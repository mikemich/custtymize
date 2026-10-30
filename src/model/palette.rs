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
    },
    Palette {
        name: "tokyo-night",
        accents: &[c(0x7aa2f7), c(0xbb9af7), c(0x7dcfff), c(0x9ece6a), c(0xe0af68), c(0xf7768e), c(0x2ac3de), c(0x414868)],
        dark: c(0x1a1b26),
        light: c(0xc0caf5),
        success: c(0x9ece6a),
        error: c(0xf7768e),
    },
    Palette {
        name: "catppuccin-mocha",
        accents: &[c(0xcba6f7), c(0xf38ba8), c(0xfab387), c(0xf9e2af), c(0xa6e3a1), c(0x94e2d5), c(0x89b4fa), c(0xb4befe)],
        dark: c(0x1e1e2e),
        light: c(0xcdd6f4),
        success: c(0xa6e3a1),
        error: c(0xf38ba8),
    },
    Palette {
        name: "gruvbox",
        accents: &[c(0xd65d0e), c(0xd79921), c(0x689d6a), c(0x458588), c(0xb16286), c(0x98971a), c(0xcc241d), c(0x665c54)],
        dark: c(0x282828),
        light: c(0xfbf1c7),
        success: c(0xb8bb26),
        error: c(0xfb4934),
    },
    Palette {
        name: "phosphor",
        accents: &[c(0x33ff66), c(0x1fcc4d), c(0x15993a), c(0x0e6b29), c(0x0a4d1d)],
        dark: c(0x001a08),
        light: c(0x7dff9e),
        success: c(0x33ff66),
        error: c(0xb3ffc6),
    },
    Palette {
        name: "amber",
        accents: &[c(0xffb000), c(0xe69c00), c(0xb37700), c(0x805500), c(0x523600)],
        dark: c(0x1a1000),
        light: c(0xffd27a),
        success: c(0xffb000),
        error: c(0xff6a00),
    },
    Palette {
        name: "synthwave",
        accents: &[c(0xff2a6d), c(0xd300c5), c(0x7700ff), c(0x05d9e8), c(0xfee440), c(0x1a1a40)],
        dark: c(0x0d0221),
        light: c(0xf5f5ff),
        success: c(0x05d9e8),
        error: c(0xff2a6d),
    },
    Palette {
        name: "nord",
        accents: &[c(0x5e81ac), c(0x81a1c1), c(0x88c0d0), c(0xa3be8c), c(0xebcb8b), c(0xd08770), c(0xb48ead), c(0x4c566a)],
        dark: c(0x2e3440),
        light: c(0xeceff4),
        success: c(0xa3be8c),
        error: c(0xbf616a),
    },
    Palette {
        name: "one-dark",
        accents: &[c(0x61afef), c(0xc678dd), c(0x56b6c2), c(0x98c379), c(0xe5c07b), c(0xe06c75), c(0xd19a66), c(0x3e4451)],
        dark: c(0x282c34),
        light: c(0xabb2bf),
        success: c(0x98c379),
        error: c(0xe06c75),
    },
    Palette {
        name: "rose-pine",
        accents: &[c(0xc4a7e7), c(0xebbcba), c(0x9ccfd8), c(0xf6c177), c(0x31748f), c(0xeb6f92), c(0x524f67), c(0x26233a)],
        dark: c(0x191724),
        light: c(0xe0def4),
        success: c(0x9ccfd8),
        error: c(0xeb6f92),
    },
    Palette {
        name: "everforest",
        accents: &[c(0xa7c080), c(0x83c092), c(0x7fbbb3), c(0xdbbc7f), c(0xe69875), c(0xd699b6), c(0xe67e80), c(0x475258)],
        dark: c(0x2d353b),
        light: c(0xd3c6aa),
        success: c(0xa7c080),
        error: c(0xe67e80),
    },
    Palette {
        name: "kanagawa",
        accents: &[c(0x7e9cd8), c(0x957fb8), c(0x7aa89f), c(0x98bb6c), c(0xe6c384), c(0xffa066), c(0xd27e99), c(0x363646)],
        dark: c(0x1f1f28),
        light: c(0xdcd7ba),
        success: c(0x98bb6c),
        error: c(0xe82424),
    },
    Palette {
        name: "monokai",
        accents: &[c(0xf92672), c(0xa6e22e), c(0x66d9ef), c(0xfd971f), c(0xae81ff), c(0xe6db74), c(0x75715e), c(0x49483e)],
        dark: c(0x272822),
        light: c(0xf8f8f2),
        success: c(0xa6e22e),
        error: c(0xf92672),
    },
    Palette {
        name: "solarized",
        accents: &[c(0x268bd2), c(0x2aa198), c(0x859900), c(0xb58900), c(0xcb4b16), c(0xd33682), c(0x6c71c4), c(0x073642)],
        dark: c(0x002b36),
        light: c(0xfdf6e3),
        success: c(0x859900),
        error: c(0xdc322f),
    },
    Palette {
        name: "github-dark",
        accents: &[c(0x58a6ff), c(0xbc8cff), c(0x39c5cf), c(0x3fb950), c(0xd29922), c(0xff7b72), c(0x8b949e), c(0x30363d)],
        dark: c(0x0d1117),
        light: c(0xe6edf3),
        success: c(0x3fb950),
        error: c(0xf85149),
    },
    Palette {
        name: "ayu",
        accents: &[c(0xffb454), c(0x59c2ff), c(0xaad94c), c(0xd2a6ff), c(0x95e6cb), c(0xf07178), c(0xe6b673), c(0x1f2430)],
        dark: c(0x0b0e14),
        light: c(0xbfbdb6),
        success: c(0xaad94c),
        error: c(0xd95757),
    },
    Palette {
        name: "cyberpunk",
        accents: &[c(0xfcee0a), c(0x00f0ff), c(0xff003c), c(0x711c91), c(0x0abdc6), c(0xea00d9), c(0x133e7c), c(0x091833)],
        dark: c(0x000b1e),
        light: c(0xf2f2f2),
        success: c(0x00f0ff),
        error: c(0xff003c),
    },
    Palette {
        name: "sakura",
        accents: &[c(0xffb7c5), c(0xf4a7b9), c(0xc9a0dc), c(0xa8d8ea), c(0xfcd5ce), c(0xb5ead7), c(0xe2c2ff), c(0x5c4b51)],
        dark: c(0x2a2024),
        light: c(0xfff5f7),
        success: c(0xb5ead7),
        error: c(0xff6b81),
    },
    Palette {
        name: "ice",
        accents: &[c(0x8fbcbb), c(0xa3d5ff), c(0x6ec1e4), c(0x4a90c2), c(0x2e6f9e), c(0x1d4e73), c(0xc9e7f5), c(0x22303c)],
        dark: c(0x0f1a24),
        light: c(0xeaf6ff),
        success: c(0x6ec1e4),
        error: c(0xff8a8a),
    },
    Palette {
        name: "greyscale",
        accents: &[c(0xe0e0e0), c(0xb0b0b0), c(0x8a8a8a), c(0x6a6a6a), c(0x4a4a4a), c(0x333333)],
        dark: c(0x161616),
        light: c(0xf0f0f0),
        success: c(0xe0e0e0),
        error: c(0xff5f5f),
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

    #[test]
    fn palettes_are_distinct_and_findable() {
        let mut names: Vec<_> = PALETTES.iter().map(|p| p.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), PALETTES.len());
        for p in PALETTES {
            assert_eq!(find(p.name).map(|f| f.name), Some(p.name));
            assert!(!p.accents.is_empty());
        }
    }
}
