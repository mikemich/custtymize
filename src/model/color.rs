//! 24-bit colours, hex parsing and Starship style strings.

use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// Parses `#rrggbb`, `rrggbb` or `#rgb`.
    pub fn parse(s: &str) -> Option<Rgb> {
        let h = s.trim().trim_start_matches('#');
        let expanded: String = match h.len() {
            3 => h.chars().flat_map(|c| [c, c]).collect(),
            6 => h.to_string(),
            _ => return None,
        };
        let v = u32::from_str_radix(&expanded, 16).ok()?;
        Some(Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
    }

    /// Parses a hex value or one of Starship's named ANSI colours
    /// (mapped onto their Dracula equivalents).
    pub fn parse_any(s: &str) -> Option<Rgb> {
        let s = s.trim().to_ascii_lowercase();
        let named = match s.trim_start_matches("bright-") {
            "black" => Some("#21222c"),
            "red" => Some("#ff5555"),
            "green" => Some("#50fa7b"),
            "yellow" => Some("#f1fa8c"),
            "blue" => Some("#bd93f9"),
            "purple" => Some("#bd93f9"),
            "cyan" => Some("#8be9fd"),
            "white" => Some("#f8f8f2"),
            _ => None,
        };
        match named {
            Some(h) => Rgb::parse(h),
            None if s.starts_with('#') => Rgb::parse(&s),
            None => None,
        }
    }

    pub fn hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }

    /// Relative luminance (sRGB, 0..1).
    pub fn luminance(&self) -> f32 {
        fn ch(c: u8) -> f32 {
            let c = c as f32 / 255.0;
            if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        }
        0.2126 * ch(self.0) + 0.7152 * ch(self.1) + 0.0722 * ch(self.2)
    }

    pub fn is_light(&self) -> bool {
        self.luminance() > 0.35
    }

    /// Nearest xterm-256 index (for terminals without truecolour).
    pub fn to_ansi256(self) -> u8 {
        let Rgb(r, g, b) = self;
        // Greyscale ramp when the channels are close together.
        if r.abs_diff(g) < 10 && g.abs_diff(b) < 10 {
            let avg = (r as u16 + g as u16 + b as u16) / 3;
            if avg < 8 {
                return 16;
            }
            if avg > 238 {
                return 231;
            }
            return 232 + ((avg - 8) * 24 / 231) as u8;
        }
        let q = |c: u8| -> u8 {
            if c < 48 { 0 } else if c < 115 { 1 } else { ((c as u16 - 35) / 40) as u8 }
        };
        16 + 36 * q(r) + 6 * q(g) + q(b)
    }

    pub fn mix(&self, other: Rgb, t: f32) -> Rgb {
        let l = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Rgb(l(self.0, other.0), l(self.1, other.1), l(self.2, other.2))
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.hex())
    }
}

impl From<Rgb> for ratatui::style::Color {
    fn from(c: Rgb) -> Self {
        ratatui::style::Color::Rgb(c.0, c.1, c.2)
    }
}

/// The parsed pieces of a Starship style string such as `bold fg:#282a36 bg:#bd93f9`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParsedStyle {
    pub fg: Option<Rgb>,
    pub bg: Option<Rgb>,
    pub bold: bool,
    pub italic: bool,
}

pub fn parse_style(s: &str) -> ParsedStyle {
    let mut out = ParsedStyle::default();
    for tok in s.split_whitespace() {
        let lower = tok.to_ascii_lowercase();
        if lower == "bold" {
            out.bold = true;
        } else if lower == "italic" {
            out.italic = true;
        } else if let Some(c) = lower.strip_prefix("fg:") {
            out.fg = Rgb::parse_any(c).or(out.fg);
        } else if let Some(c) = lower.strip_prefix("bg:") {
            out.bg = Rgb::parse_any(c).or(out.bg);
        } else if let Some(c) = Rgb::parse_any(&lower) {
            out.fg = Some(c);
        }
    }
    out
}

/// Dracula reference colours, used for UI chrome and the colour picker.
pub mod dracula {
    use super::Rgb;
    pub const BG: Rgb = Rgb(0x28, 0x2a, 0x36);
    pub const CURRENT: Rgb = Rgb(0x44, 0x47, 0x5a);
    pub const FG: Rgb = Rgb(0xf8, 0xf8, 0xf2);
    pub const COMMENT: Rgb = Rgb(0x62, 0x72, 0xa4);
    pub const CYAN: Rgb = Rgb(0x8b, 0xe9, 0xfd);
    pub const GREEN: Rgb = Rgb(0x50, 0xfa, 0x7b);
    pub const ORANGE: Rgb = Rgb(0xff, 0xb8, 0x6c);
    pub const PINK: Rgb = Rgb(0xff, 0x79, 0xc6);
    pub const PURPLE: Rgb = Rgb(0xbd, 0x93, 0xf9);
    pub const RED: Rgb = Rgb(0xff, 0x55, 0x55);
    pub const YELLOW: Rgb = Rgb(0xf1, 0xfa, 0x8c);
    pub const DARKER: Rgb = Rgb(0x21, 0x22, 0x2c);

    pub const SWATCHES: [(&str, Rgb); 12] = [
        ("background", BG),
        ("darker", DARKER),
        ("current", CURRENT),
        ("comment", COMMENT),
        ("foreground", FG),
        ("cyan", CYAN),
        ("green", GREEN),
        ("orange", ORANGE),
        ("pink", PINK),
        ("purple", PURPLE),
        ("red", RED),
        ("yellow", YELLOW),
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        assert_eq!(Rgb::parse("#bd93f9"), Some(Rgb(0xbd, 0x93, 0xf9)));
        assert_eq!(Rgb::parse("fff"), Some(Rgb(255, 255, 255)));
        assert_eq!(Rgb::parse("#12"), None);
        assert_eq!(Rgb(1, 2, 255).hex(), "#0102ff");
    }

    #[test]
    fn style_parsing() {
        let s = parse_style("bold fg:#282a36 bg:#bd93f9");
        assert!(s.bold);
        assert_eq!(s.fg, Some(dracula::BG));
        assert_eq!(s.bg, Some(dracula::PURPLE));
        assert_eq!(parse_style("bold green").fg, Some(dracula::GREEN));
    }

    #[test]
    fn ansi256_is_sane() {
        assert_eq!(Rgb(0, 0, 0).to_ansi256(), 16);
        assert_eq!(Rgb(255, 0, 0).to_ansi256(), 196);
        assert_eq!(Rgb(255, 255, 255).to_ansi256(), 231);
    }
}
