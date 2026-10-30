//! Complete prompt designs: which modules, in what order, plus shape, theme and glyphs.

use super::{Cap, Glyphs, Layout, Model, Segment, Shape, Side, palette};

pub struct Preset {
    pub name: &'static str,
    pub blurb: &'static str,
    /// Modules in display order (`character` is added last automatically).
    pub modules: &'static [(&'static str, Side)],
    pub palette: &'static str,
    pub shape: Shape,
    pub cap: Cap,
    pub layout: Layout,
    pub glyphs: Glyphs,
    pub prompt_char: &'static str,
    pub add_newline: bool,
}

use Side::{Left as L, Right as R};

pub static PRESETS: &[Preset] = &[
    Preset {
        name: "neon-powerline",
        blurb: "Dracula powerline with a gradient cap: the custtymize look",
        modules: &[("directory", L), ("git_branch", L), ("git_status", L), ("nodejs", L), ("python", L), ("rust", L), ("cmd_duration", L), ("time", R)],
        palette: "dracula",
        shape: Shape::Powerline,
        cap: Cap::Gradient,
        layout: Layout::RightPrompt,
        glyphs: Glyphs::Nerd,
        prompt_char: "❯",
        add_newline: true,
    },
    Preset {
        name: "pastel-pills",
        blurb: "Catppuccin rounded pills on one line, prompt below",
        modules: &[("os", L), ("directory", L), ("git_branch", L), ("git_status", L), ("nodejs", L), ("python", L), ("rust", L), ("golang", L), ("cmd_duration", L)],
        palette: "catppuccin-mocha",
        shape: Shape::Rounded,
        cap: Cap::None,
        layout: Layout::TwoLine,
        glyphs: Glyphs::Nerd,
        prompt_char: "❯",
        add_newline: true,
    },
    Preset {
        name: "tokyo-slant",
        blurb: "Tokyo Night slanted segments with the clock on the right",
        modules: &[("username", L), ("directory", L), ("git_branch", L), ("git_status", L), ("package", L), ("cmd_duration", R), ("time", R)],
        palette: "tokyo-night",
        shape: Shape::Slanted,
        cap: Cap::None,
        layout: Layout::RightPrompt,
        glyphs: Glyphs::Nerd,
        prompt_char: "❯",
        add_newline: true,
    },
    Preset {
        name: "zen",
        blurb: "Minimal two-line text prompt: path and branch, nothing else",
        modules: &[("directory", L), ("git_branch", L), ("git_status", L), ("cmd_duration", L)],
        palette: "rose-pine",
        shape: Shape::Plain,
        cap: Cap::None,
        layout: Layout::TwoLine,
        glyphs: Glyphs::Unicode,
        prompt_char: "❯",
        add_newline: true,
    },
    Preset {
        name: "one-liner",
        blurb: "Everything on one short line; no blank line between prompts",
        modules: &[("directory", L), ("git_branch", L), ("git_status", L)],
        palette: "nord",
        shape: Shape::Plain,
        cap: Cap::None,
        layout: Layout::OneLine,
        glyphs: Glyphs::Nerd,
        prompt_char: "›",
        add_newline: false,
    },
    Preset {
        name: "full-stack",
        blurb: "Every dev module: languages, docker, cloud, k8s, jobs, status",
        modules: &[
            ("username", L), ("hostname", L), ("directory", L), ("git_branch", L), ("git_status", L), ("package", L),
            ("nodejs", L), ("python", L), ("rust", L), ("golang", L), ("java", L), ("docker_context", L),
            ("jobs", L), ("cmd_duration", R), ("kubernetes", R), ("aws", R), ("status", R), ("time", R),
        ],
        palette: "gruvbox",
        shape: Shape::Powerline,
        cap: Cap::Rounded,
        layout: Layout::RightPrompt,
        glyphs: Glyphs::Nerd,
        prompt_char: "❯",
        add_newline: true,
    },
    Preset {
        name: "hacker-terminal",
        blurb: "Phosphor-green flat blocks, like a 1983 mainframe",
        modules: &[("username", L), ("hostname", L), ("directory", L), ("git_branch", L), ("git_status", L), ("cmd_duration", L), ("status", L)],
        palette: "phosphor",
        shape: Shape::Flat,
        cap: Cap::Gradient,
        layout: Layout::TwoLine,
        glyphs: Glyphs::Unicode,
        prompt_char: "$",
        add_newline: true,
    },
    Preset {
        name: "synthwave-sunset",
        blurb: "Neon synthwave pills with battery and clock on the right",
        modules: &[("directory", L), ("git_branch", L), ("git_status", L), ("nodejs", L), ("python", L), ("battery", R), ("cmd_duration", R), ("time", R)],
        palette: "synthwave",
        shape: Shape::Rounded,
        cap: Cap::None,
        layout: Layout::RightPrompt,
        glyphs: Glyphs::Nerd,
        prompt_char: "❯",
        add_newline: true,
    },
    Preset {
        name: "cyber-slant",
        blurb: "Cyberpunk yellow-and-cyan slants, two lines",
        modules: &[("directory", L), ("git_branch", L), ("git_status", L), ("rust", L), ("nodejs", L), ("python", L), ("cmd_duration", L)],
        palette: "cyberpunk",
        shape: Shape::Slanted,
        cap: Cap::None,
        layout: Layout::TwoLine,
        glyphs: Glyphs::Nerd,
        prompt_char: "❯",
        add_newline: true,
    },
    Preset {
        name: "plain-ascii",
        blurb: "No special fonts at all: plain ASCII words, safe anywhere",
        modules: &[("username", L), ("directory", L), ("git_branch", L), ("git_status", L), ("python", L), ("nodejs", L), ("cmd_duration", L)],
        palette: "greyscale",
        shape: Shape::Plain,
        cap: Cap::None,
        layout: Layout::OneLine,
        glyphs: Glyphs::Ascii,
        prompt_char: ">",
        add_newline: true,
    },
    Preset {
        name: "amber-crt",
        blurb: "Amber monochrome flat blocks with the time up front",
        modules: &[("time", L), ("directory", L), ("git_branch", L), ("git_status", L), ("cmd_duration", L)],
        palette: "amber",
        shape: Shape::Flat,
        cap: Cap::Gradient,
        layout: Layout::OneLine,
        glyphs: Glyphs::Unicode,
        prompt_char: "❯",
        add_newline: true,
    },
    Preset {
        name: "kanagawa-wave",
        blurb: "Kanagawa powerline with language versions on the right",
        modules: &[("directory", L), ("git_branch", L), ("git_status", L), ("nodejs", R), ("python", R), ("rust", R), ("golang", R)],
        palette: "kanagawa",
        shape: Shape::Powerline,
        cap: Cap::None,
        layout: Layout::RightPrompt,
        glyphs: Glyphs::Nerd,
        prompt_char: "❯",
        add_newline: true,
    },
];

pub fn find(name: &str) -> Option<&'static Preset> {
    let n = name.trim().to_ascii_lowercase().replace(' ', "-");
    PRESETS.iter().find(|p| p.name == n).or_else(|| PRESETS.iter().find(|p| !n.is_empty() && p.name.starts_with(&n)))
}

impl Model {
    /// Replaces the whole design with a preset. Modules not in the preset are kept but
    /// switched off, so their settings come back if you re-enable them.
    pub fn apply_design(&mut self, p: &Preset) {
        let mut rest = std::mem::take(&mut self.segments);
        let mut segs = Vec::new();
        for (name, side) in p.modules {
            let mut s = match rest.iter().position(|s| s.name == *name && s.literal.is_none()) {
                Some(i) => rest.remove(i),
                None => Segment::new(name),
            };
            s.enabled = true;
            s.side = *side;
            s.format_override = None;
            segs.push(s);
        }
        let character = rest.iter().position(|s| s.is_character()).map(|i| rest.remove(i));
        for mut s in rest {
            s.enabled = false;
            s.format_override = None;
            segs.push(s);
        }
        let mut ch = character.unwrap_or_else(|| Segment::new("character"));
        ch.enabled = true;
        segs.push(ch);
        self.segments = segs;

        self.globals.layout = p.layout;
        self.globals.add_newline = p.add_newline;
        // Glyphs first (it may change the shape), then shape, then colours.
        self.globals.shape = Shape::Flat;
        self.set_glyphs(p.glyphs);
        self.globals.prompt_char = p.prompt_char.to_string();
        self.globals.cap = p.cap;
        self.set_shape(p.shape);
        self.apply_palette(palette::find(p.palette).unwrap_or(palette::default_palette()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::generate;

    #[test]
    fn every_preset_generates_a_valid_config() {
        for p in PRESETS {
            let mut m = Model::default();
            m.apply_design(p);
            assert_eq!(m.globals.shape, p.shape, "{}", p.name);
            assert_eq!(m.globals.glyphs, p.glyphs, "{}", p.name);
            assert!(m.segments.last().is_some_and(|s| s.is_character() && s.enabled));
            let toml = generate::render(&m, &toml_edit::DocumentMut::new(), None);
            let doc: toml_edit::DocumentMut = toml.parse().unwrap_or_else(|e| panic!("{}: {e}\n{toml}", p.name));
            let (errors, warnings) = crate::engine::validate(&doc);
            assert!(errors.is_empty() && warnings.is_empty(), "{}: {errors:?} {warnings:?}\n{toml}", p.name);
            if p.glyphs == Glyphs::Ascii {
                assert!(toml.is_ascii(), "{} should be pure ASCII:\n{toml}", p.name);
            }
            if p.glyphs != Glyphs::Nerd {
                assert!(!toml.chars().any(|c| matches!(c as u32, 0xe000..=0xf8ff | 0xf0000..)), "{} uses Nerd glyphs:\n{toml}", p.name);
            }
            // Reloading recognises the design.
            let (back, _) = crate::config::load::model_from_doc(&doc);
            assert_eq!(back.globals.shape, p.shape, "{}", p.name);
            assert_eq!(back.globals.layout, p.layout, "{}", p.name);
            assert_eq!(back.globals.palette, p.palette, "{}", p.name);
            assert_eq!(back.globals.glyphs == Glyphs::Ascii, p.glyphs == Glyphs::Ascii, "{}", p.name);
        }
    }

    #[test]
    fn find_by_prefix() {
        assert_eq!(find("zen").map(|p| p.name), Some("zen"));
        assert_eq!(find("Plain ASCII").map(|p| p.name), Some("plain-ascii"));
        assert_eq!(find("hack").map(|p| p.name), Some("hacker-terminal"));
        assert!(find("").is_none());
    }
}
