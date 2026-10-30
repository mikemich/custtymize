//! Best-effort reconstruction of a [`Model`] from an existing starship.toml.

use std::path::Path;

use toml_edit::{DocumentMut, Item};

use crate::error::{Error, Result};
use crate::model::catalog::{self, OptKind, SymbolMode};
use crate::model::color::parse_style;
use crate::model::palette;
use crate::model::{Cap, Layout, Model, OptValue, Segment, Shape, Side};

use super::generate::glyph;

pub struct Loaded {
    pub doc: DocumentMut,
    pub model: Model,
    pub existed: bool,
    pub notes: Vec<String>,
}

pub fn load(path: &Path) -> Result<Loaded> {
    if !path.exists() {
        return Ok(Loaded {
            doc: DocumentMut::new(),
            model: Model::default(),
            existed: false,
            notes: vec![format!("{} not found, starting from defaults", path.display())],
        });
    }
    let text = std::fs::read_to_string(path).map_err(|e| Error::io(path, e))?;
    let doc: DocumentMut = text
        .parse()
        .map_err(|e: toml_edit::TomlError| Error::Parse { path: path.to_path_buf(), message: e.to_string() })?;
    let (model, notes) = model_from_doc(&doc);
    Ok(Loaded { doc, model, existed: true, notes })
}

#[derive(Clone, Debug, PartialEq)]
pub enum FmtItem {
    Token(String),
    Text(String),
}

/// Splits a top-level format into module tokens and the literal text between them.
pub fn format_items(fmt: &str) -> Vec<FmtItem> {
    let chars: Vec<char> = fmt.chars().collect();
    let mut out = Vec::new();
    let mut text = String::new();
    let mut i = 0;
    let flush = |text: &mut String, out: &mut Vec<FmtItem>| {
        if !text.is_empty() {
            out.push(FmtItem::Text(std::mem::take(text)));
        }
    };
    while i < chars.len() {
        match chars[i] {
            '\\' => {
                text.push('\\');
                if let Some(c) = chars.get(i + 1) {
                    text.push(*c);
                }
                i += 2;
            }
            '$' if chars.get(i + 1) == Some(&'{') => {
                let start = i + 2;
                let mut j = start;
                while j < chars.len() && chars[j] != '}' {
                    j += 1;
                }
                flush(&mut text, &mut out);
                out.push(FmtItem::Token(chars[start..j.min(chars.len())].iter().collect()));
                i = j + 1;
            }
            '$' if chars.get(i + 1).is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_') => {
                let start = i + 1;
                let mut j = start;
                while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '_') {
                    j += 1;
                }
                flush(&mut text, &mut out);
                out.push(FmtItem::Token(chars[start..j].iter().collect()));
                i = j;
            }
            c => {
                text.push(c);
                i += 1;
            }
        }
    }
    flush(&mut text, &mut out);
    out
}

/// Extracts `$name` / `${name}` tokens from a format string, in order.
#[cfg(test)]
pub fn format_tokens(fmt: &str) -> Vec<String> {
    let chars: Vec<char> = fmt.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 2,
            '$' => {
                i += 1;
                if chars.get(i) == Some(&'{') {
                    let start = i + 1;
                    while i < chars.len() && chars[i] != '}' {
                        i += 1;
                    }
                    out.push(chars[start..i.min(chars.len())].iter().collect());
                    i += 1;
                } else {
                    let start = i;
                    while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                        i += 1;
                    }
                    if i > start {
                        out.push(chars[start..i].iter().collect());
                    }
                }
            }
            _ => i += 1,
        }
    }
    out
}

/// Style groups `](...)` in a format string, e.g. `fg:#282a36 bg:#bd93f9`.
fn style_groups(fmt: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = fmt;
    while let Some(p) = rest.find("](") {
        let after = &rest[p + 2..];
        match after.find(')') {
            Some(end) => {
                out.push(after[..end].to_string());
                rest = &after[end..];
            }
            None => break,
        }
    }
    out
}

fn table<'a>(doc: &'a DocumentMut, name: &str) -> Option<&'a toml_edit::Table> {
    let mut cur = doc.as_table();
    for part in name.split('.') {
        cur = cur.get(part)?.as_table()?;
    }
    Some(cur)
}

fn get_str<'a>(t: Option<&'a toml_edit::Table>, key: &str) -> Option<&'a str> {
    t?.get(key)?.as_str()
}

pub fn model_from_doc(doc: &DocumentMut) -> (Model, Vec<String>) {
    let mut notes = Vec::new();
    let mut model = Model::default();
    let root = doc.as_table();
    let fmt = root.get("format").and_then(Item::as_str).map(str::to_string);
    let right = root.get("right_format").and_then(Item::as_str).map(str::to_string);

    let Some(fmt) = fmt else {
        notes.push("no top-level `format`; using the default module set".into());
        return (model, notes);
    };

    let pal = palette::default_palette();
    let left_items = format_items(&fmt);
    let right_items = right.as_deref().map(format_items).unwrap_or_default();
    let mut segments: Vec<Segment> = Vec::new();
    let mut accent_i = 0;

    for (item, side) in left_items
        .iter()
        .map(|t| (t, Side::Left))
        .chain(right_items.iter().map(|t| (t, Side::Right)))
    {
        let tok = match item {
            // Whitespace is re-created by the generator; anything else is kept verbatim.
            FmtItem::Text(t) if t.trim().is_empty() => continue,
            FmtItem::Text(t) => {
                let mut s = Segment::literal(t);
                s.side = side;
                segments.push(s);
                continue;
            }
            FmtItem::Token(t) => t,
        };
        if matches!(tok.as_str(), "line_break" | "fill" | "all") || segments.iter().any(|s| &s.name == tok) {
            if tok == "fill" || tok == "all" {
                notes.push(format!("`${tok}` in format is not managed and will be dropped on save"));
            }
            continue;
        }
        let Some(def) = catalog::lookup(tok) else {
            notes.push(format!("kept unknown module `{tok}` verbatim"));
            let mut s = Segment::raw(tok);
            s.side = side;
            segments.push(s);
            continue;
        };
        let t = table(doc, tok);
        let mut seg = Segment::new(def.name);
        seg.enabled = true;
        seg.side = side;

        // Keep a hand-written module format until the user picks a shape.
        seg.format_override = get_str(t, "format").map(String::from);
        if def.symbol_mode == SymbolMode::Native
            && let Some(sym) = get_str(t, "symbol")
                && sym != def.default_symbol {
                    seg.symbol = Some(sym.to_string());
                }
        for o in def.options {
            let Some(v) = t.and_then(|t| t.get(o.key)) else { continue };
            let ov = match o.kind {
                OptKind::Bool => v.as_bool().map(OptValue::Bool),
                OptKind::Int => v.as_integer().map(OptValue::Int),
                OptKind::Str => v.as_str().map(|s| OptValue::Str(s.into())),
            };
            if let Some(ov) = ov {
                seg.options.insert(o.key.to_string(), ov);
            }
        }

        // Colours: prefer `style`, fall back to the first styled group in the module format.
        let mut ps = parse_style(def.style_key().and_then(|k| get_str(t, k)).unwrap_or(""));
        if let Some(f) = get_str(t, "format") {
            let groups = style_groups(f);
            let with_bg = groups.iter().map(|g| parse_style(g)).find(|p| p.bg.is_some());
            if let Some(g) = with_bg.or_else(|| groups.iter().map(|g| parse_style(g)).find(|p| p.fg.is_some())) {
                if ps.bg.is_none() {
                    ps.bg = g.bg;
                }
                if ps.fg.is_none() || g.bg.is_some() {
                    ps.fg = g.fg.or(ps.fg);
                }
                ps.bold |= g.bold;
            }
        }
        let accent = pal.accents[accent_i % pal.accents.len()];
        accent_i += 1;
        if ps.bg.is_none() && ps.fg.is_none() {
            // No colour information at all: treat it like a plain-text accent.
            ps.fg = Some(accent);
        }
        seg.bg = ps.bg.unwrap_or(match ps.fg {
            Some(fg) if ps.bg.is_none() => pal.text_on(fg),
            _ => accent,
        });
        seg.fg = ps.fg.unwrap_or_else(|| pal.text_on(seg.bg));
        seg.bold = ps.bold;
        seg.italic = ps.italic;
        segments.push(seg);
    }

    // Keep everything else from the catalog's starter set around, switched off.
    for def in catalog::CATALOG.iter().filter(|d| d.starter) {
        if !segments.iter().any(|s| s.name == def.name) {
            let mut s = Segment::new(def.name);
            s.enabled = false;
            if let Some(d) = model.find(def.name) {
                s.fg = d.fg;
                s.bg = d.bg;
            }
            segments.push(s);
        }
    }
    // Character goes last.
    if let Some(i) = segments.iter().position(|s| s.is_character()) {
        let c = segments.remove(i);
        segments.push(c);
    }

    // Globals.
    let g = &mut model.globals;
    let all_text: String = std::iter::once(fmt.clone())
        .chain(segments.iter().filter_map(|s| get_str(table(doc, &s.name), "format").map(String::from)))
        .collect();
    let any_bg = segments.iter().any(|s| s.enabled && !s.raw)
        && segments
            .iter()
            .filter(|s| !s.raw && !s.is_character())
            .any(|s| get_str(table(doc, &s.name), "format").is_some_and(|f| f.contains("bg:"))
                || get_str(table(doc, &s.name), "style").is_some_and(|f| f.contains("bg:")));
    g.shape = if fmt.contains(glyph::ARROW_R) || fmt.contains(glyph::ARROW_L) {
        Shape::Powerline
    } else if all_text.contains(glyph::SLANT_L) || all_text.contains(glyph::SLANT_R) {
        Shape::Slanted
    } else if all_text.contains(glyph::ROUND_L) || all_text.contains(glyph::ROUND_R) {
        Shape::Rounded
    } else if any_bg {
        Shape::Flat
    } else {
        Shape::Plain
    };
    g.cap = if fmt.starts_with(&format!("[{}", glyph::GRADIENT_L)) {
        Cap::Gradient
    } else if fmt.starts_with(&format!("[{}", glyph::ROUND_L)) {
        Cap::Rounded
    } else {
        Cap::None
    };
    g.layout = if right.as_deref().is_some_and(|r| !r.trim().is_empty()) {
        Layout::RightPrompt
    } else if fmt.contains("$line_break") || fmt.contains('\n') {
        Layout::TwoLine
    } else {
        Layout::OneLine
    };
    // Colours came from the file, not from one of our presets.
    g.palette = "custom";
    if let Some(b) = root.get("add_newline").and_then(Item::as_bool) {
        g.add_newline = b;
    }
    let ch = table(doc, "character");
    if let Some(s) = get_str(ch, "success_symbol")
        && let Some((sym, style)) = split_styled(s) {
            g.prompt_char = sym;
            if let Some(c) = parse_style(&style).fg {
                g.success = c;
            }
        }
    if let Some(c) = get_str(ch, "error_symbol").and_then(split_styled).and_then(|(_, st)| parse_style(&st).fg) {
        g.error = c;
    }
    model.segments = segments;
    (model, notes)
}

/// Splits `[❯](bold green)` into (`❯`, `bold green`).
fn split_styled(s: &str) -> Option<(String, String)> {
    let s = s.trim();
    let inner = s.strip_prefix('[')?;
    let close = inner.find("](")?;
    let sym = inner[..close].replace('\\', "");
    let style = inner[close + 2..].trim_end_matches(')').to_string();
    Some((sym, style))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::generate;
    use crate::model::color::dracula;

    const USER_CONFIG: &str = r##""$schema" = 'https://starship.rs/config-schema.json'

# One-line slanted prompt, Dracula colours.
add_newline = true
command_timeout = 800
format = "$directory$git_branch$git_status$python $character"

[character]
success_symbol = "[❯](bold #50fa7b)"
error_symbol = "[❯](bold #ff5555)"

[directory]
truncation_length = 3
truncation_symbol = "…/"
format = '[](fg:#bd93f9)[ $path$read_only ](fg:#282a36 bg:#bd93f9)[](fg:#bd93f9)'

[git_branch]
symbol = ""
format = '[](fg:#6272a4)[ $symbol $branch ](fg:#f8f8f2 bg:#6272a4)[](fg:#6272a4)'

[php]
disabled = true
"##;

    #[test]
    fn tokens() {
        assert_eq!(format_tokens("$a${custom.b}\\$c $d_e"), vec!["a", "custom.b", "d_e"]);
    }

    #[test]
    fn loads_users_config() {
        let doc: DocumentMut = USER_CONFIG.parse().expect("parse");
        let (m, _) = model_from_doc(&doc);
        let names: Vec<_> = m.segments.iter().filter(|s| s.enabled).map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["directory", "git_branch", "git_status", "python", "character"]);
        assert!(m.find("git_branch").is_some_and(|s| s.format_override.is_some()));
        let dir = m.find("directory").expect("directory");
        assert_eq!(dir.bg, dracula::PURPLE);
        assert_eq!(dir.fg, dracula::BG);
        assert_eq!(dir.options.get("truncation_length"), Some(&OptValue::Int(3)));
        assert_eq!(m.find("git_branch").map(|s| s.bg), Some(dracula::COMMENT));
        assert_eq!(m.globals.shape, Shape::Flat);
        assert_eq!(m.globals.prompt_char, "❯");
        assert!(m.segments.last().is_some_and(|s| s.is_character()));
    }

    #[test]
    fn round_trip_keeps_unknown_settings() {
        let doc: DocumentMut = USER_CONFIG.parse().expect("parse");
        let (m, _) = model_from_doc(&doc);
        let out = generate::render(&m, &doc, Some(&m));
        assert_eq!(out, USER_CONFIG, "a no-op render must be byte-identical");
        // A real edit regenerates only what changed.
        let mut edited = m.clone();
        if let Some(s) = edited.find_mut("directory") {
            s.fg = dracula::PINK;
        }
        let out = generate::render(&edited, &doc, Some(&m));
        for needle in ["command_timeout = 800", "[php]", "# One-line slanted prompt", "\"$schema\"", "truncation_symbol = \"…/\""] {
            assert!(out.contains(needle), "missing {needle:?} in\n{out}");
        }
        assert!(out.contains("style = \"fg:#ff79c6 bg:#bd93f9\""), "{out}");
        assert!(out.contains("[](fg:#6272a4)[ $symbol $branch ]"), "git_branch must be untouched:\n{out}");
        // Regenerating from the regenerated config is stable.
        let doc2: DocumentMut = out.parse().expect("reparse");
        let (m2, _) = model_from_doc(&doc2);
        assert_eq!(generate::render(&m2, &doc2, Some(&m2)), out);
    }

    #[test]
    fn literal_text_in_format_survives() {
        let src = "format = \"$username[@](#727169)[mbp](#dcd7ba)[:](#727169)$directory$character\"\n";
        let doc: DocumentMut = src.parse().expect("parse");
        let (m, _) = model_from_doc(&doc);
        assert_eq!(generate::top_formats(&m).0, "$username[@](#727169)[mbp](#dcd7ba)[:](#727169)$directory$character");
        assert_eq!(m.globals.shape, Shape::Plain);
    }

    #[test]
    fn unknown_modules_survive() {
        let doc: DocumentMut = "format = '$directory${custom.vpn}$kotlin$character'\n[custom.vpn]\ncommand = 'echo hi'\n"
            .parse()
            .expect("parse");
        let (m, notes) = model_from_doc(&doc);
        assert!(m.find("custom.vpn").is_some_and(|s| s.raw));
        assert!(notes.iter().any(|n| n.contains("kotlin")));
        let (fmt, _) = generate::top_formats(&m);
        assert!(fmt.contains("${custom.vpn}") && fmt.contains("$kotlin"), "{fmt}");
    }
}
