//! Turns a [`Model`] into Starship TOML, editing a `toml_edit` document in place so
//! comments, key order and settings we don't manage survive.

use toml_edit::{DocumentMut, Item, Table, value};

use crate::model::catalog::SymbolMode;
use crate::model::color::parse_style;
use crate::model::{Cap, Layout, Model, OptValue, Segment, Shape, Side};

pub mod glyph {
    pub const ARROW_R: &str = "\u{e0b0}";
    pub const THIN_R: &str = "\u{e0b1}";
    pub const ARROW_L: &str = "\u{e0b2}";
    pub const THIN_L: &str = "\u{e0b3}";
    pub const ROUND_L: &str = "\u{e0b6}";
    pub const ROUND_R: &str = "\u{e0b4}";
    pub const SLANT_L: &str = "\u{e0ba}";
    pub const SLANT_R: &str = "\u{e0bc}";
    pub const GRADIENT_L: &str = "░▒▓";
    pub const GRADIENT_R: &str = "▓▒░";
}

/// Escapes text so Starship's format parser treats it literally.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '[' | ']' | '(' | ')' | '$' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// The `style` value for a segment under the current shape.
pub fn style_string(seg: &Segment, shape: Shape) -> String {
    let mut parts = Vec::new();
    if seg.bold {
        parts.push("bold".to_string());
    }
    if seg.italic {
        parts.push("italic".to_string());
    }
    parts.push(format!("fg:{}", seg.fg.hex()));
    if shape != Shape::Plain {
        parts.push(format!("bg:{}", seg.bg.hex()));
    }
    parts.join(" ")
}

/// The module body with the symbol resolved according to the module's [`SymbolMode`].
fn body(seg: &Segment) -> String {
    let def = seg.def();
    match def.symbol_mode {
        SymbolMode::Inline => def.body.replace("$symbol", &escape(seg.effective_symbol())),
        SymbolMode::Native | SymbolMode::None => def.body.to_string(),
    }
}

/// The module-level `format` for one segment.
///
/// Every format is wrapped in a conditional group `( … )` so a module that renders
/// nothing (no git repo, no venv) also hides its padding and caps.
pub fn module_format(seg: &Segment, shape: Shape) -> String {
    if let Some(f) = &seg.format_override {
        return f.clone();
    }
    let f = generated_format(seg, shape);
    match seg.def().style_key() {
        Some("style") => f,
        // Modules whose style lives under another name (or nowhere) get it inlined.
        _ => f.replace("]($style)", &format!("]({})", style_string(seg, shape))),
    }
}

fn generated_format(seg: &Segment, shape: Shape) -> String {
    let b = body(seg);
    let bg = seg.bg.hex();
    match shape {
        Shape::Powerline | Shape::Flat => format!("([ {b} ]($style))"),
        Shape::Rounded => format!(
            "([{l}](fg:{bg})[ {b} ]($style)[{r}](fg:{bg}) )",
            l = glyph::ROUND_L,
            r = glyph::ROUND_R
        ),
        Shape::Slanted => format!(
            "([{l}](fg:{bg})[ {b} ]($style)[{r}](fg:{bg}))",
            l = glyph::SLANT_L,
            r = glyph::SLANT_R
        ),
        Shape::Plain => format!("([{b}]($style) )"),
    }
}

/// Builds the left-to-right run of segments for one side of the prompt,
/// chaining separator colours between neighbours where the shape needs it.
pub fn segment_run(segs: &[&Segment], shape: Shape, cap: Cap, side: Side) -> String {
    let mut out = String::new();
    if segs.is_empty() {
        return out;
    }
    let chained = matches!(shape, Shape::Powerline | Shape::Flat);
    let first = segs[0];
    let last = segs[segs.len() - 1];

    if chained && side == Side::Left {
        match cap {
            Cap::Gradient => out += &format!("[{}](fg:{})", glyph::GRADIENT_L, first.bg.hex()),
            Cap::Rounded => out += &format!("[{}](fg:{})", glyph::ROUND_L, first.bg.hex()),
            Cap::None => {}
        }
    }

    for (i, seg) in segs.iter().enumerate() {
        if shape == Shape::Powerline {
            match (side, i) {
                (Side::Left, 0) => {}
                (Side::Left, _) => {
                    let prev = segs[i - 1];
                    if prev.bg == seg.bg {
                        out += &format!("[{}](fg:{} bg:{})", glyph::THIN_R, prev.fg.hex(), seg.bg.hex());
                    } else {
                        out += &format!("[{}](fg:{} bg:{})", glyph::ARROW_R, prev.bg.hex(), seg.bg.hex());
                    }
                }
                (Side::Right, 0) => out += &format!("[{}](fg:{})", glyph::ARROW_L, seg.bg.hex()),
                (Side::Right, _) => {
                    let prev = segs[i - 1];
                    if prev.bg == seg.bg {
                        out += &format!("[{}](fg:{} bg:{})", glyph::THIN_L, seg.fg.hex(), seg.bg.hex());
                    } else {
                        out += &format!("[{}](fg:{} bg:{})", glyph::ARROW_L, seg.bg.hex(), prev.bg.hex());
                    }
                }
            }
        }
        out += &seg.token();
    }

    if chained {
        match (shape, side, cap) {
            (Shape::Powerline, Side::Left, _) => {
                out += &format!("[{}](fg:{})", glyph::ARROW_R, last.bg.hex())
            }
            (Shape::Flat, Side::Left, Cap::Gradient) => {
                out += &format!("[{}](fg:{})", glyph::GRADIENT_R, last.bg.hex())
            }
            (Shape::Flat, Side::Left, Cap::Rounded) => {
                out += &format!("[{}](fg:{})", glyph::ROUND_R, last.bg.hex())
            }
            (_, Side::Right, Cap::Gradient) => {
                out += &format!("[{}](fg:{})", glyph::GRADIENT_R, last.bg.hex())
            }
            (_, Side::Right, Cap::Rounded) => {
                out += &format!("[{}](fg:{})", glyph::ROUND_R, last.bg.hex())
            }
            _ => {}
        }
    }
    out
}

/// Top-level `format` and `right_format`.
pub fn top_formats(model: &Model) -> (String, Option<String>) {
    let g = &model.globals;
    let split = g.layout == Layout::RightPrompt;

    // Raw (unmanaged) segments are emitted in place but can't take part in colour chaining,
    // so each run of styled segments between raw ones is chained independently.
    let build = |want: Option<Side>| -> String {
        let mut out = String::new();
        let mut run: Vec<&Segment> = Vec::new();
        let side = want.unwrap_or(Side::Left);
        for seg in model.segments.iter().filter(|s| s.enabled && !s.is_character()) {
            if let Some(w) = want
                && seg.side != w {
                    continue;
                }
            if seg.raw {
                out += &segment_run(&run, g.shape, g.cap, side);
                run.clear();
                out += &seg.token();
            } else {
                run.push(seg);
            }
        }
        out += &segment_run(&run, g.shape, g.cap, side);
        out
    };

    let mut left = if split { build(Some(Side::Left)) } else { build(None) };
    let has_char = model.character().is_some_and(|c| c.enabled);
    if has_char {
        match g.layout {
            Layout::TwoLine => left += "$line_break$character",
            _ if g.shape == Shape::Plain || left.is_empty() => left += "$character",
            _ => left += " $character",
        }
    } else if g.layout == Layout::TwoLine {
        left += "$line_break";
    }

    let right = if split {
        let r = build(Some(Side::Right));
        if r.is_empty() { None } else { Some(r) }
    } else {
        None
    };
    (left, right)
}

fn table_mut<'a>(doc: &'a mut DocumentMut, name: &str) -> &'a mut Table {
    // `custom.foo` lives at [custom.foo]
    let mut cur = doc.as_table_mut();
    for part in name.split('.') {
        let entry = cur.entry(part).or_insert_with(|| {
            let mut t = Table::new();
            t.set_implicit(false);
            Item::Table(t)
        });
        if !entry.is_table() {
            *entry = Item::Table(Table::new());
        }
        cur = match entry.as_table_mut() {
            Some(t) => t,
            None => unreachable!("entry was just made a table"),
        };
    }
    cur
}

fn has_table(doc: &DocumentMut, name: &str) -> bool {
    let mut cur = doc.as_table();
    for part in name.split('.') {
        match cur.get(part).and_then(Item::as_table) {
            Some(t) => cur = t,
            None => return false,
        }
    }
    true
}

fn same_value(existing: &Item, v: &OptValue, key: &str) -> bool {
    match v {
        OptValue::Bool(b) => existing.as_bool() == Some(*b),
        OptValue::Int(i) => existing.as_integer() == Some(*i),
        OptValue::Str(s) => match existing.as_str() {
            Some(e) if e == s => true,
            // `#e46876` and `fg:#e46876` are the same style; don't churn the file.
            Some(e) if key.starts_with("style") => parse_style(e) == parse_style(s),
            _ => false,
        },
    }
}

/// Sets a key, leaving it alone if it already holds an equivalent value.
fn set_opt(t: &mut Table, key: &str, v: &OptValue) {
    if t.get(key).is_some_and(|e| same_value(e, v, key)) {
        return;
    }
    let item = match v {
        OptValue::Bool(b) => value(*b),
        OptValue::Int(i) => value(*i),
        OptValue::Str(s) => value(s.as_str()),
    };
    // Keep any trailing comment/decoration on an existing key.
    match t.get_mut(key) {
        Some(existing) if existing.is_value() => {
            let decor = existing.as_value().map(|v| v.decor().clone());
            *existing = item;
            if let (Some(d), Some(v)) = (decor, existing.as_value_mut()) {
                *v.decor_mut() = d;
            }
        }
        _ => {
            t.insert(key, item);
        }
    }
}

fn set_str(t: &mut Table, key: &str, s: &str) {
    set_opt(t, key, &OptValue::Str(s.to_string()));
}

/// Writes the model into `doc`. Only keys custtymize owns are touched, and when `base`
/// (the model as loaded from `doc`) is given, anything unchanged since then is left alone,
/// so a no-op save is byte-identical.
pub fn apply(model: &Model, base: Option<&Model>, doc: &mut DocumentMut) {
    let g = &model.globals;
    let (fmt, right) = top_formats(model);
    let top_changed = base.is_none_or(|b| top_formats(b) != (fmt.clone(), right.clone()));
    let shape_changed = base.is_none_or(|b| b.globals.shape != g.shape);
    let char_changed = base.is_none_or(|b| {
        (&b.globals.prompt_char, b.globals.success, b.globals.error) != (&g.prompt_char, g.success, g.error)
            || b.character().map(|c| c.enabled) != model.character().map(|c| c.enabled)
    });

    let root = doc.as_table_mut();
    if base.is_none_or(|b| b.globals.add_newline != g.add_newline) {
        set_opt(root, "add_newline", &OptValue::Bool(g.add_newline));
    }
    if top_changed {
        set_str(root, "format", &fmt);
        match right {
            Some(r) => set_str(root, "right_format", &r),
            None => {
                root.remove("right_format");
            }
        }
    }

    for seg in &model.segments {
        if seg.raw {
            continue;
        }
        let def = seg.def();
        if seg.is_character() {
            if !seg.enabled || !char_changed {
                continue;
            }
            let t = table_mut(doc, "character");
            let ch = escape(&g.prompt_char);
            set_str(t, "success_symbol", &format!("[{ch}](bold {})", g.success.hex()));
            set_str(t, "error_symbol", &format!("[{ch}](bold {})", g.error.hex()));
            continue;
        }
        let untouched = base.and_then(|b| b.find(&seg.name)).is_some_and(|b| b == seg) && !shape_changed;
        if untouched {
            continue;
        }
        if !seg.enabled {
            // Not in `format`, so it won't render; only flip `disabled` if the user
            // already has a table for it.
            if has_table(doc, &seg.name) && def.off_by_default {
                table_mut(doc, &seg.name).insert("disabled", value(true));
            }
            continue;
        }
        let t = table_mut(doc, &seg.name);
        if def.off_by_default {
            set_opt(t, "disabled", &OptValue::Bool(false));
        } else if t.get("disabled").and_then(|v| v.as_bool()) == Some(true) {
            t.remove("disabled");
        }
        if let Some(key) = def.style_key() {
            set_str(t, key, &style_string(seg, g.shape));
        }
        // Generated formats always carry our symbol so the preview matches the inspector;
        // hand-written ones keep starship's default unless the user picked one.
        if def.symbol_mode == SymbolMode::Native && (seg.symbol.is_some() || seg.format_override.is_none()) {
            set_str(t, "symbol", seg.effective_symbol());
        }
        set_str(t, "format", &module_format(seg, g.shape));
        for (k, v) in &seg.options {
            set_opt(t, k, v);
        }
    }
}

/// Renders the model on top of `base` (the loaded config) and returns TOML text.
pub fn render(model: &Model, base_doc: &DocumentMut, base_model: Option<&Model>) -> String {
    let mut doc = base_doc.clone();
    apply(model, base_model, &mut doc);
    doc.to_string()
}

/// TOML snippet for a single module, shown in the inspector.
pub fn module_snippet(model: &Model, seg: &Segment) -> String {
    if seg.raw {
        return format!("# [{}] is not managed by custtymize\n# (kept verbatim)", seg.name);
    }
    let mut doc = DocumentMut::new();
    let mut solo = model.clone();
    solo.segments.retain(|s| s.name == seg.name);
    if let Some(s) = solo.segments.first_mut() {
        s.enabled = true;
    }
    apply(&solo, None, &mut doc);
    let root = doc.as_table_mut();
    for k in ["add_newline", "format", "right_format"] {
        root.remove(k);
    }
    doc.to_string().trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::color::Rgb;
    use crate::model::{Globals, Segment};

    const A: Rgb = Rgb(0xbd, 0x93, 0xf9);
    const B: Rgb = Rgb(0xff, 0x79, 0xc6);
    const DARK: Rgb = Rgb(0x28, 0x2a, 0x36);

    fn seg(name: &str, bg: Rgb) -> Segment {
        Segment { enabled: true, side: Side::Left, bg, fg: DARK, ..Segment::new(name) }
    }

    fn model(segs: Vec<Segment>, shape: Shape, cap: Cap, layout: Layout) -> Model {
        Model { segments: segs, globals: Globals { shape, cap, layout, ..Globals::default() } }
    }

    #[test]
    fn powerline_chains_separator_colours() {
        let a = seg("directory", A);
        let b = seg("git_branch", B);
        let run = segment_run(&[&a, &b], Shape::Powerline, Cap::None, Side::Left);
        assert_eq!(
            run,
            format!("$directory[{}](fg:#bd93f9 bg:#ff79c6)$git_branch[{}](fg:#ff79c6)", glyph::ARROW_R, glyph::ARROW_R)
        );
    }

    #[test]
    fn same_background_uses_thin_separator() {
        let a = seg("git_branch", A);
        let b = seg("git_status", A);
        let run = segment_run(&[&a, &b], Shape::Powerline, Cap::None, Side::Left);
        assert!(run.contains(&format!("[{}](fg:#282a36 bg:#bd93f9)", glyph::THIN_R)), "{run}");
        assert!(!run.contains(&format!("[{}](fg:#bd93f9 bg:#bd93f9)", glyph::ARROW_R)));
    }

    #[test]
    fn gradient_cap_takes_first_segment_colour() {
        let a = seg("directory", A);
        let b = seg("git_branch", B);
        let run = segment_run(&[&a, &b], Shape::Powerline, Cap::Gradient, Side::Left);
        assert!(run.starts_with("[░▒▓](fg:#bd93f9)$directory"), "{run}");
    }

    #[test]
    fn right_side_powerline_points_left() {
        let a = seg("time", A);
        let b = seg("cmd_duration", B);
        let run = segment_run(&[&a, &b], Shape::Powerline, Cap::None, Side::Right);
        assert_eq!(
            run,
            format!("[{l}](fg:#bd93f9)$time[{l}](fg:#ff79c6 bg:#bd93f9)$cmd_duration", l = glyph::ARROW_L)
        );
    }

    #[test]
    fn self_contained_shapes_need_no_chaining() {
        let a = seg("directory", A);
        let b = seg("git_branch", B);
        for shape in [Shape::Rounded, Shape::Slanted, Shape::Plain] {
            assert_eq!(segment_run(&[&a, &b], shape, Cap::Gradient, Side::Left), "$directory$git_branch");
        }
        let f = module_format(&a, Shape::Rounded);
        assert_eq!(f, format!("([{}](fg:#bd93f9)[ $path$read_only ]($style)[{}](fg:#bd93f9) )", glyph::ROUND_L, glyph::ROUND_R));
    }

    #[test]
    fn module_formats_hide_when_empty() {
        let s = seg("git_status", A);
        for shape in Shape::ALL {
            let f = module_format(&s, *shape);
            assert!(f.starts_with('(') && f.trim_end().ends_with(')'), "{shape:?}: {f}");
        }
    }

    #[test]
    fn inline_symbols_are_escaped() {
        let mut s = seg("cmd_duration", A);
        s.symbol = Some("[t]".into());
        assert_eq!(module_format(&s, Shape::Flat), "([ \\[t\\]$duration ]($style))");
    }

    #[test]
    fn style_string_respects_plain() {
        let mut s = seg("rust", A);
        s.bold = true;
        assert_eq!(style_string(&s, Shape::Powerline), "bold fg:#282a36 bg:#bd93f9");
        assert_eq!(style_string(&s, Shape::Plain), "bold fg:#282a36");
    }

    #[test]
    fn layouts_place_character_and_right_prompt() {
        let mut t = seg("time", B);
        t.side = Side::Right;
        let segs = vec![seg("directory", A), t, Segment { enabled: true, ..Segment::new("character") }];

        let (l, r) = top_formats(&model(segs.clone(), Shape::Flat, Cap::None, Layout::OneLine));
        assert_eq!(l, "$directory$time $character");
        assert!(r.is_none());

        let (l, _) = top_formats(&model(segs.clone(), Shape::Flat, Cap::None, Layout::TwoLine));
        assert_eq!(l, "$directory$time$line_break$character");

        let (l, r) = top_formats(&model(segs, Shape::Flat, Cap::None, Layout::RightPrompt));
        assert_eq!(l, "$directory $character");
        assert_eq!(r.as_deref(), Some("$time"));
    }

    #[test]
    fn raw_segments_break_the_chain() {
        let segs = vec![seg("directory", A), Segment::raw("custom.foo"), seg("git_branch", B)];
        let (l, _) = top_formats(&model(segs, Shape::Powerline, Cap::None, Layout::OneLine));
        assert_eq!(
            l,
            format!("$directory[{r}](fg:#bd93f9)${{custom.foo}}$git_branch[{r}](fg:#ff79c6)", r = glyph::ARROW_R)
        );
    }

    #[test]
    fn apply_preserves_comments_and_unknown_keys() {
        let src = "# my precious comment\ncommand_timeout = 800\n\n[directory]\ntruncation_length = 3 # keep three\n\n[php]\ndisabled = true\n";
        let base: DocumentMut = src.parse().expect("parse");
        let m = model(vec![seg("directory", A)], Shape::Powerline, Cap::None, Layout::OneLine);
        let out = render(&m, &base, None);
        assert!(out.contains("# my precious comment"));
        assert!(out.contains("command_timeout = 800"));
        assert!(out.contains("truncation_length = 3 # keep three"));
        assert!(out.contains("[php]\ndisabled = true"));
        assert!(out.contains("style = \"fg:#282a36 bg:#bd93f9\""));
        let reparsed: Result<DocumentMut, _> = out.parse();
        assert!(reparsed.is_ok());
    }

    #[test]
    fn off_by_default_modules_get_enabled() {
        let m = model(vec![seg("time", A)], Shape::Flat, Cap::None, Layout::OneLine);
        let out = render(&m, &DocumentMut::new(), None);
        assert!(out.contains("[time]"));
        assert!(out.contains("disabled = false"));
    }

    #[test]
    fn character_symbols_use_globals() {
        let mut m = model(vec![Segment { enabled: true, ..Segment::new("character") }], Shape::Flat, Cap::None, Layout::OneLine);
        m.globals.prompt_char = "λ".into();
        let out = render(&m, &DocumentMut::new(), None);
        assert!(out.contains("success_symbol = \"[λ](bold #50fa7b)\""), "{out}");
        assert!(out.contains("error_symbol = \"[λ](bold #ff5555)\""));
    }
}
