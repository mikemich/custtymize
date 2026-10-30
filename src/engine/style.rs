//! Style strings (`bold fg:#282a36 bg:purple`) and their ANSI rendering.

use super::Shell;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Color {
    /// One of the 16 terminal palette colours (0-7 normal, 8-15 bright).
    Ansi(u8),
    Fixed(u8),
    Rgb(u8, u8, u8),
    /// Resolved against the previous segment when the prompt is serialized.
    PrevFg,
    PrevBg,
}

pub const BOLD: u16 = 1;
pub const DIMMED: u16 = 1 << 1;
pub const ITALIC: u16 = 1 << 2;
pub const UNDERLINE: u16 = 1 << 3;
pub const BLINK: u16 = 1 << 4;
pub const INVERTED: u16 = 1 << 5;
pub const HIDDEN: u16 = 1 << 6;
pub const STRIKE: u16 = 1 << 7;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub attrs: u16,
}

impl Style {
    pub fn is_plain(&self) -> bool {
        self.fg.is_none() && self.bg.is_none() && self.attrs == 0
    }

    /// `self` drawn inside `parent`: own colours win, attributes accumulate.
    pub fn inherit(self, parent: Style) -> Style {
        Style { fg: self.fg.or(parent.fg), bg: self.bg.or(parent.bg), attrs: self.attrs | parent.attrs }
    }
}

fn named(name: &str) -> Option<u8> {
    Some(match name {
        "black" => 0,
        "red" => 1,
        "green" => 2,
        "yellow" => 3,
        "blue" => 4,
        "purple" | "magenta" => 5,
        "cyan" => 6,
        "white" => 7,
        _ => return None,
    })
}

pub fn parse_color(s: &str) -> Option<Color> {
    let s = s.to_ascii_lowercase();
    if let Some(hex) = s.strip_prefix('#') {
        let full: String = match hex.len() {
            3 => hex.chars().flat_map(|c| [c, c]).collect(),
            6 => hex.to_string(),
            _ => return None,
        };
        let v = u32::from_str_radix(&full, 16).ok()?;
        return Some(Color::Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8));
    }
    if let Ok(n) = s.parse::<u8>() {
        return Some(Color::Fixed(n));
    }
    match s.as_str() {
        "prev_fg" => return Some(Color::PrevFg),
        "prev_bg" => return Some(Color::PrevBg),
        _ => {}
    }
    if let Some(b) = s.strip_prefix("bright-") {
        return named(b).map(|n| Color::Ansi(n + 8));
    }
    named(&s).map(Color::Ansi)
}

/// Parses a style string. Unknown words are collected in `bad` rather than failing.
pub fn parse_style(s: &str, bad: &mut Vec<String>) -> Style {
    let mut st = Style::default();
    for tok in s.split_whitespace() {
        let lower = tok.to_ascii_lowercase();
        let attr = match lower.as_str() {
            "bold" => BOLD,
            "dimmed" => DIMMED,
            "italic" => ITALIC,
            "underline" => UNDERLINE,
            "blink" => BLINK,
            "inverted" => INVERTED,
            "hidden" => HIDDEN,
            "strikethrough" => STRIKE,
            "none" => {
                st = Style::default();
                continue;
            }
            _ => 0,
        };
        if attr != 0 {
            st.attrs |= attr;
            continue;
        }
        let (slot, val) = if let Some(v) = lower.strip_prefix("fg:") {
            (&mut st.fg, v)
        } else if let Some(v) = lower.strip_prefix("bg:") {
            (&mut st.bg, v)
        } else {
            (&mut st.fg, lower.as_str())
        };
        if val == "none" {
            *slot = None;
        } else if let Some(c) = parse_color(val) {
            *slot = Some(c);
        } else {
            bad.push(tok.to_string());
        }
    }
    st
}

fn color_code(c: Color, bg: bool, out: &mut Vec<String>) {
    let base = if bg { 40 } else { 30 };
    match c {
        Color::Ansi(n) if n < 8 => out.push((base + n as u16).to_string()),
        Color::Ansi(n) => out.push((base + 60 + (n as u16 - 8)).to_string()),
        Color::Fixed(n) => out.push(format!("{};5;{n}", base + 8)),
        Color::Rgb(r, g, b) => out.push(format!("{};2;{r};{g};{b}", base + 8)),
        // Unresolved: there was no previous segment to take a colour from.
        Color::PrevFg | Color::PrevBg => {}
    }
}

/// The SGR escape for a style whose `prev_*` colours are already resolved.
pub fn sgr(st: &Style) -> String {
    let mut codes = Vec::new();
    for (bit, code) in [(BOLD, 1), (DIMMED, 2), (ITALIC, 3), (UNDERLINE, 4), (BLINK, 5), (INVERTED, 7), (HIDDEN, 8), (STRIKE, 9)] {
        if st.attrs & bit != 0 {
            codes.push(code.to_string());
        }
    }
    if let Some(c) = st.fg {
        color_code(c, false, &mut codes);
    }
    if let Some(c) = st.bg {
        color_code(c, true, &mut codes);
    }
    if codes.is_empty() { String::new() } else { format!("\x1b[{}m", codes.join(";")) }
}

/// A run of text in one style.
#[derive(Clone, Debug, PartialEq)]
pub struct Seg {
    pub text: String,
    pub style: Style,
}

pub fn segs_text(segs: &[Seg]) -> String {
    segs.iter().map(|s| s.text.as_str()).collect()
}

/// Wraps an escape sequence so the shell knows it takes no columns.
fn wrap(esc: &str, shell: Shell) -> String {
    match shell {
        Shell::Zsh => format!("%{{{esc}%}}"),
        // Readline's RL_PROMPT_START_IGNORE / END; works from inside a variable, unlike `\[ \]`.
        Shell::Bash => format!("\x01{esc}\x02"),
        Shell::Fish | Shell::None => esc.to_string(),
    }
}

fn escape_text(text: &str, shell: Shell) -> String {
    match shell {
        Shell::Zsh => text.replace('%', "%%"),
        _ => text.to_string(),
    }
}

/// Serializes styled segments to ANSI for a shell, resolving `prev_fg` / `prev_bg`.
pub fn to_ansi(segs: &[Seg], shell: Shell) -> String {
    let reset = wrap("\x1b[0m", shell);
    let mut out = String::new();
    let mut current = Style::default();
    let (mut prev_fg, mut prev_bg) = (None, None);
    for seg in segs.iter().filter(|s| !s.text.is_empty()) {
        let resolve = |c: Option<Color>| match c {
            Some(Color::PrevFg) => prev_fg,
            Some(Color::PrevBg) => prev_bg,
            other => other,
        };
        let st = Style { fg: resolve(seg.style.fg), bg: resolve(seg.style.bg), attrs: seg.style.attrs };
        if st != current {
            if !current.is_plain() {
                out += &reset;
            }
            if !st.is_plain() {
                out += &wrap(&sgr(&st), shell);
            }
            current = st;
        }
        out += &escape_text(&seg.text, shell);
        prev_fg = st.fg;
        prev_bg = st.bg;
    }
    if !current.is_plain() {
        out += &reset;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_styles() {
        let mut bad = vec![];
        let st = parse_style("bold fg:#282a36 bg:purple", &mut bad);
        assert_eq!(st.fg, Some(Color::Rgb(0x28, 0x2a, 0x36)));
        assert_eq!(st.bg, Some(Color::Ansi(5)));
        assert_eq!(st.attrs, BOLD);
        assert_eq!(parse_style("208 bold", &mut bad).fg, Some(Color::Fixed(208)));
        assert_eq!(parse_style("bright-green", &mut bad).fg, Some(Color::Ansi(10)));
        assert!(bad.is_empty());
        parse_style("bold sparkly", &mut bad);
        assert_eq!(bad, vec!["sparkly"]);
    }

    #[test]
    fn ansi_and_shell_wrapping() {
        let segs = vec![
            Seg { text: "a".into(), style: parse_style("red bg:#000000", &mut vec![]) },
            Seg { text: "\u{e0b0}".into(), style: parse_style("fg:prev_bg bg:blue", &mut vec![]) },
            Seg { text: "50%".into(), style: Style::default() },
        ];
        assert_eq!(to_ansi(&segs, Shell::None), "\x1b[31;48;2;0;0;0ma\x1b[0m\x1b[38;2;0;0;0;44m\u{e0b0}\x1b[0m50%");
        let z = to_ansi(&segs, Shell::Zsh);
        assert!(z.starts_with("%{\x1b[31;48;2;0;0;0m%}a%{\x1b[0m%}"), "{z:?}");
        assert!(z.ends_with("50%%"));
        assert!(to_ansi(&segs, Shell::Bash).starts_with("\x01\x1b[31;"));
    }
}
