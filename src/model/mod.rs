//! The editable prompt model: an ordered list of segments plus global options.

pub mod catalog;
pub mod color;
pub mod palette;
pub mod presets;

use std::collections::BTreeMap;

use catalog::{ModuleDef, SymbolMode};
use color::Rgb;
use palette::Palette;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Left,
    Right,
}

#[derive(Clone, PartialEq, Debug)]
pub enum OptValue {
    Bool(bool),
    Int(i64),
    Str(String),
}

impl OptValue {
    pub fn display(&self) -> String {
        match self {
            OptValue::Bool(b) => b.to_string(),
            OptValue::Int(i) => i.to_string(),
            OptValue::Str(s) => quote(s),
        }
    }
}

/// Double-quotes for display without Rust's `\u{…}` escaping, so glyphs stay readable.
pub fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// One Starship module as it appears in the prompt.
#[derive(Clone, PartialEq, Debug)]
pub struct Segment {
    /// Starship module name (`git_branch`, `custom.foo`, …).
    pub name: String,
    /// A module from the existing config that we don't know how to style.
    /// It is emitted verbatim in `format` and its table is never touched.
    pub raw: bool,
    pub enabled: bool,
    pub side: Side,
    /// `None` means "use the catalog default".
    pub symbol: Option<String>,
    pub fg: Rgb,
    pub bg: Rgb,
    pub bold: bool,
    pub italic: bool,
    /// A hand-written module `format`, used verbatim instead of the generated one.
    pub format_override: Option<String>,
    /// Module specific keys that have been set (others keep Starship's defaults).
    pub options: BTreeMap<String, OptValue>,
    /// Literal text from the top-level `format` (e.g. `[@](#727169)`), emitted verbatim.
    pub literal: Option<String>,
}

impl Segment {
    pub fn new(name: &str) -> Segment {
        let def = catalog::lookup(name);
        Segment {
            name: name.to_string(),
            raw: false,
            enabled: def.is_some_and(|d| d.starter),
            side: if def.is_some_and(|d| d.right_by_default) { Side::Right } else { Side::Left },
            symbol: None,
            fg: color::dracula::BG,
            bg: color::dracula::PURPLE,
            bold: false,
            italic: false,
            format_override: None,
            options: BTreeMap::new(),
            literal: None,
        }
    }

    pub fn literal(text: &str) -> Segment {
        Segment { literal: Some(text.to_string()), ..Segment::raw("text") }
    }

    pub fn raw(name: &str) -> Segment {
        Segment { raw: true, enabled: true, ..Segment::new(name) }
    }

    pub fn def(&self) -> &'static ModuleDef {
        catalog::lookup(&self.name).unwrap_or(&catalog::GENERIC)
    }

    pub fn is_character(&self) -> bool {
        self.name == "character"
    }

    /// The symbol that will actually be rendered.
    pub fn effective_symbol(&self) -> &str {
        self.symbol.as_deref().unwrap_or(self.def().default_symbol)
    }

    pub fn symbol_editable(&self) -> bool {
        !self.raw && self.def().symbol_mode != SymbolMode::None
    }

    /// Token used in a top-level `format` string.
    pub fn token(&self) -> String {
        if let Some(l) = &self.literal {
            return l.clone();
        }
        if self.name.contains('.') { format!("${{{}}}", self.name) } else { format!("${}", self.name) }
    }
}

macro_rules! cycling_enum {
    ($ty:ident { $($variant:ident => $label:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub enum $ty { $($variant),+ }
        impl $ty {
            pub const ALL: &'static [$ty] = &[$($ty::$variant),+];
            pub fn label(&self) -> &'static str {
                match self { $($ty::$variant => $label),+ }
            }
            pub fn cycle(&self, step: i32) -> $ty {
                let i = Self::ALL.iter().position(|v| v == self).unwrap_or(0) as i32;
                let n = Self::ALL.len() as i32;
                Self::ALL[((i + step).rem_euclid(n)) as usize]
            }
            pub fn parse(s: &str) -> Option<$ty> {
                let k: String = s.to_ascii_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
                if k.is_empty() { return None; }
                Self::ALL.iter().copied().find(|v| {
                    let l: String = v.label().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
                    l.starts_with(&k)
                })
            }
        }
    };
}

cycling_enum!(Layout {
    OneLine => "one line",
    TwoLine => "two lines",
    RightPrompt => "right prompt",
});

cycling_enum!(Shape {
    Powerline => "powerline",
    Rounded => "rounded",
    Slanted => "slanted",
    Flat => "flat",
    Plain => "plain text",
});

cycling_enum!(Cap {
    Gradient => "gradient",
    Rounded => "rounded",
    None => "none",
});

cycling_enum!(Glyphs {
    Nerd => "nerd font",
    Unicode => "unicode",
    Ascii => "ascii",
});

impl Shape {
    /// Powerline, rounded and slanted are drawn with Nerd Font glyphs.
    pub fn needs_nerd_font(&self) -> bool {
        matches!(self, Shape::Powerline | Shape::Rounded | Shape::Slanted)
    }
}

impl Glyphs {
    pub fn blurb(&self) -> &'static str {
        match self {
            Glyphs::Nerd => "Nerd Font icons and powerline shapes (needs a Nerd Font)",
            Glyphs::Unicode => "standard Unicode and emoji: works in any modern terminal font",
            Glyphs::Ascii => "plain ASCII words: works everywhere, even over old ssh",
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Globals {
    pub layout: Layout,
    pub shape: Shape,
    pub cap: Cap,
    pub prompt_char: String,
    pub success: Rgb,
    pub error: Rgb,
    pub add_newline: bool,
    pub palette: &'static str,
    pub glyphs: Glyphs,
}

impl Default for Globals {
    fn default() -> Self {
        let p = palette::default_palette();
        Globals {
            layout: Layout::OneLine,
            shape: Shape::Powerline,
            cap: Cap::Gradient,
            prompt_char: "❯".into(),
            success: p.success,
            error: p.error,
            add_newline: true,
            palette: p.name,
            glyphs: Glyphs::Nerd,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Model {
    pub segments: Vec<Segment>,
    pub globals: Globals,
}

impl Default for Model {
    fn default() -> Self {
        let mut m = Model {
            segments: catalog::CATALOG
                .iter()
                .filter(|d| d.starter)
                .map(|d| Segment::new(d.name))
                .collect(),
            globals: Globals::default(),
        };
        // A tasteful starting point: the common modules on, the noisy ones off.
        for s in &mut m.segments {
            s.enabled = matches!(
                s.name.as_str(),
                "directory" | "git_branch" | "git_status" | "nodejs" | "rust" | "python" | "cmd_duration" | "character"
            );
        }
        m.apply_palette(palette::default_palette());
        m
    }
}

impl Model {
    /// Enabled, styled (non-raw, non-character) segments in display order.
    pub fn styled(&self) -> impl Iterator<Item = &Segment> {
        self.segments.iter().filter(|s| s.enabled && !s.raw && !s.is_character())
    }

    pub fn character(&self) -> Option<&Segment> {
        self.segments.iter().find(|s| s.is_character())
    }

    /// Recolours every segment from a preset (disabled ones too, so toggling them on looks right).
    pub fn apply_palette(&mut self, p: &'static Palette) {
        let plain = self.globals.shape == Shape::Plain;
        let mut i = 0;
        // Enabled segments get the first accents so the visible prompt always uses the
        // palette's lead colours; disabled ones continue the cycle afterwards.
        let order: Vec<usize> = (0..self.segments.len())
            .filter(|&k| self.segments[k].enabled)
            .chain((0..self.segments.len()).filter(|&k| !self.segments[k].enabled))
            .collect();
        for k in order {
            let s = &mut self.segments[k];
            if s.raw || s.is_character() {
                continue;
            }
            let accent = p.accents[i % p.accents.len()];
            i += 1;
            if plain {
                s.fg = accent;
                s.bg = p.dark;
            } else {
                s.bg = accent;
                s.fg = p.text_on(accent);
            }
        }
        self.globals.success = p.success;
        self.globals.error = p.error;
        self.globals.palette = p.name;
    }

    /// Switching to/from plain text swaps which colour is the "accent" so nothing turns invisible.
    pub fn set_shape(&mut self, shape: Shape) {
        let was_plain = self.globals.shape == Shape::Plain;
        let now_plain = shape == Shape::Plain;
        self.globals.shape = shape;
        if was_plain != now_plain {
            let p = palette::find(self.globals.palette).unwrap_or(palette::default_palette());
            for s in self.segments.iter_mut().filter(|s| !s.raw && !s.is_character()) {
                if now_plain {
                    s.fg = s.bg;
                } else {
                    s.bg = s.fg;
                    s.fg = p.text_on(s.bg);
                }
            }
        }
    }

    /// Switches every module symbol (that the user hasn't customised) to a glyph set, and
    /// drops shapes and caps the set can't draw.
    pub fn set_glyphs(&mut self, g: Glyphs) {
        self.globals.glyphs = g;
        for s in self.segments.iter_mut().filter(|s| !s.raw && s.literal.is_none() && !s.is_character()) {
            let def = s.def();
            if def.symbol_mode == SymbolMode::None {
                continue;
            }
            let (uni, ascii) = catalog::plain_symbols(&s.name);
            let known = [def.default_symbol, uni, ascii];
            if s.symbol.as_deref().is_some_and(|x| !known.contains(&x)) {
                continue;
            }
            s.symbol = match g {
                Glyphs::Nerd => None,
                Glyphs::Unicode => Some(uni.to_string()),
                Glyphs::Ascii => Some(ascii.to_string()),
            };
        }
        if ["❯", ">"].contains(&self.globals.prompt_char.as_str()) {
            self.globals.prompt_char = if g == Glyphs::Ascii { ">" } else { "❯" }.into();
        }
        // starship's own git_status and read-only markers aren't ASCII.
        const ASCII_OPTS: &[(&str, &str, &str)] = &[
            ("git_status", "ahead", "^"),
            ("git_status", "behind", "v"),
            ("git_status", "diverged", "<>"),
            ("git_status", "deleted", "x"),
            ("git_status", "renamed", ">"),
            ("directory", "read_only", " ro"),
        ];
        for (module, key, val) in ASCII_OPTS {
            if let Some(s) = self.find_mut(module) {
                let ours = s.options.get(*key) == Some(&OptValue::Str(val.to_string()));
                if g == Glyphs::Ascii && !s.options.contains_key(*key) {
                    s.options.insert(key.to_string(), OptValue::Str(val.to_string()));
                } else if g != Glyphs::Ascii && ours {
                    s.options.remove(*key);
                }
            }
        }
        // ASCII means ASCII: spell out an ellipsis the user's own options may use.
        if g == Glyphs::Ascii {
            for s in &mut self.segments {
                for v in s.options.values_mut() {
                    if let OptValue::Str(text) = v
                        && !text.is_ascii()
                    {
                        *text = text.replace('…', "...");
                    }
                }
            }
        }
        if g != Glyphs::Nerd {
            if self.globals.shape.needs_nerd_font() {
                self.set_shape(Shape::Flat);
            }
            if self.globals.cap == Cap::Rounded || (g == Glyphs::Ascii && self.globals.cap == Cap::Gradient) {
                self.globals.cap = Cap::None;
            }
        }
    }

    /// Moves the segment at `idx` by `delta`, keeping `character` pinned to the end.
    /// Returns the new index.
    pub fn move_segment(&mut self, idx: usize, delta: i32) -> usize {
        let last_movable = self
            .segments
            .iter()
            .rposition(|s| !s.is_character())
            .unwrap_or(0);
        if idx >= self.segments.len() || self.segments[idx].is_character() {
            return idx;
        }
        let target = (idx as i32 + delta).clamp(0, last_movable as i32) as usize;
        let seg = self.segments.remove(idx);
        self.segments.insert(target, seg);
        target
    }

    pub fn find(&self, name: &str) -> Option<&Segment> {
        self.segments.iter().find(|s| s.name == name)
    }

    pub fn find_mut(&mut self, name: &str) -> Option<&mut Segment> {
        self.segments.iter_mut().find(|s| s.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn character_stays_last() {
        let mut m = Model::default();
        let last = m.segments.len() - 1;
        assert!(m.segments[last].is_character());
        let moved = m.move_segment(0, 100);
        assert_eq!(moved, last - 1);
        assert!(m.segments[last].is_character());
        assert_eq!(m.move_segment(last, -3), last);
    }

    #[test]
    fn enum_parse_and_cycle() {
        assert_eq!(Shape::parse("power"), Some(Shape::Powerline));
        assert_eq!(Shape::parse("plain"), Some(Shape::Plain));
        assert_eq!(Layout::parse("right"), Some(Layout::RightPrompt));
        assert_eq!(Cap::None.cycle(1), Cap::Gradient);
        assert_eq!(Cap::Gradient.cycle(-1), Cap::None);
    }

    #[test]
    fn glyph_sets_swap_symbols_and_shapes() {
        let mut m = Model::default();
        m.set_glyphs(Glyphs::Ascii);
        assert_eq!(m.find("nodejs").and_then(|s| s.symbol.clone()).as_deref(), Some("node "));
        assert_eq!(m.globals.prompt_char, ">");
        assert_eq!(m.globals.shape, Shape::Flat);
        assert_eq!(m.globals.cap, Cap::None);
        // A customised symbol survives.
        if let Some(s) = m.find_mut("rust") {
            s.symbol = Some("R!".into());
        }
        m.set_glyphs(Glyphs::Nerd);
        assert_eq!(m.find("nodejs").and_then(|s| s.symbol.clone()), None);
        assert_eq!(m.find("rust").and_then(|s| s.symbol.clone()).as_deref(), Some("R!"));
        assert!(m.find("git_status").is_some_and(|s| s.options.is_empty()));
        assert_eq!(m.globals.prompt_char, "❯");
    }

    #[test]
    fn plain_shape_swaps_accents() {
        let mut m = Model::default();
        let bg = m.find("directory").map(|s| s.bg);
        m.set_shape(Shape::Plain);
        assert_eq!(m.find("directory").map(|s| s.fg), bg);
    }
}
