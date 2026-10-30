//! Colour presets for prompt segments.

use super::color::Rgb;

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub name: &'static str,
    /// Segment backgrounds, assigned to enabled segments in order (cycled).
    pub accents: &'static [Rgb],
    /// Text colour on light accents.
    pub dark: Rgb,
    /// Text colour on dark accents.
    pub light: Rgb,
    pub success: Rgb,
    pub error: Rgb,
    /// Monochrome themes also re-skin the custtymize UI itself.
    pub mono: bool,
}

impl Palette {
    pub fn text_on(&self, bg: Rgb) -> Rgb {
        if bg.is_light() { self.dark } else { self.light }
    }
}

const fn c(hex: u32) -> Rgb {
    Rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub static PALETTES: &[Palette] = &[
    Palette {
        name: "dracula",
        accents: &[c(0xbd93f9), c(0xff79c6), c(0x8be9fd), c(0x50fa7b), c(0xffb86c), c(0xf1fa8c), c(0x6272a4), c(0x44475a)],
        dark: c(0x282a36),
        light: c(0xf8f8f2),
        success: c(0x50fa7b),
        error: c(0xff5555),
        mono: false,
    },
    Palette {
        name: "tokyo-night",
        accents: &[c(0x7aa2f7), c(0xbb9af7), c(0x7dcfff), c(0x9ece6a), c(0xe0af68), c(0xf7768e), c(0x2ac3de), c(0x414868)],
        dark: c(0x1a1b26),
        light: c(0xc0caf5),
        success: c(0x9ece6a),
        error: c(0xf7768e),
        mono: false,
    },
    Palette {
        name: "catppuccin-mocha",
        accents: &[c(0xcba6f7), c(0xf38ba8), c(0xfab387), c(0xf9e2af), c(0xa6e3a1), c(0x94e2d5), c(0x89b4fa), c(0xb4befe)],
        dark: c(0x1e1e2e),
        light: c(0xcdd6f4),
        success: c(0xa6e3a1),
        error: c(0xf38ba8),
        mono: false,
    },
    Palette {
        name: "gruvbox",
        accents: &[c(0xd65d0e), c(0xd79921), c(0x689d6a), c(0x458588), c(0xb16286), c(0x98971a), c(0xcc241d), c(0x665c54)],
        dark: c(0x282828),
        light: c(0xfbf1c7),
        success: c(0xb8bb26),
        error: c(0xfb4934),
        mono: false,
    },
    Palette {
        name: "phosphor",
        accents: &[c(0x33ff66), c(0x1fcc4d), c(0x15993a), c(0x0e6b29), c(0x0a4d1d)],
        dark: c(0x001a08),
        light: c(0x7dff9e),
        success: c(0x33ff66),
        error: c(0xb3ffc6),
        mono: true,
    },
    Palette {
        name: "amber",
        accents: &[c(0xffb000), c(0xe69c00), c(0xb37700), c(0x805500), c(0x523600)],
        dark: c(0x1a1000),
        light: c(0xffd27a),
        success: c(0xffb000),
        error: c(0xff6a00),
        mono: true,
    },
    Palette {
        name: "synthwave",
        accents: &[c(0xff2a6d), c(0xd300c5), c(0x7700ff), c(0x05d9e8), c(0xfee440), c(0x1a1a40)],
        dark: c(0x0d0221),
        light: c(0xf5f5ff),
        success: c(0x05d9e8),
        error: c(0xff2a6d),
        mono: false,
    },
    Palette {
        name: "nord",
        accents: &[c(0x5e81ac), c(0x81a1c1), c(0x88c0d0), c(0xa3be8c), c(0xebcb8b), c(0xd08770), c(0xb48ead), c(0x4c566a)],
        dark: c(0x2e3440),
        light: c(0xeceff4),
        success: c(0xa3be8c),
        error: c(0xbf616a),
        mono: false,
    },
];

/// Looks up a preset by name, accepting a few spellings (`tokyo night`, `catppuccin`, `amber-crt`…).
pub fn find(name: &str) -> Option<&'static Palette> {
    let n: String = name
        .to_ascii_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    if n.is_empty() {
        return None;
    }
    PALETTES.iter().find(|p| {
        let pn: String = p.name.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        pn == n || pn.starts_with(&n) || n.starts_with(&pn)
    })
}

pub fn default_palette() -> &'static Palette {
    &PALETTES[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_accepts_aliases() {
        assert_eq!(find("Tokyo Night").map(|p| p.name), Some("tokyo-night"));
        assert_eq!(find("catppuccin").map(|p| p.name), Some("catppuccin-mocha"));
        assert_eq!(find("amber-crt").map(|p| p.name), Some("amber"));
        assert_eq!(find("phosphor green").map(|p| p.name), Some("phosphor"));
        assert!(find("nope").is_none());
    }
}
