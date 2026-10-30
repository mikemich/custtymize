//! UI chrome colours. Dracula by default; the monochrome presets re-skin everything.

use ratatui::style::{Color, Modifier, Style};

use crate::model::color::{Rgb, dracula};

#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub bg: Color,
    pub scan: Color,
    pub screen: Color,
    pub fg: Color,
    pub dim: Color,
    pub faint: Color,
    pub border: Color,
    pub focus: Color,
    pub accent: Color,
    pub accent2: Color,
    pub ok: Color,
    pub warn: Color,
    pub err: Color,
    pub info: Color,
    pub sel_bg: Color,
}

fn c(r: Rgb) -> Color {
    r.into()
}

impl Theme {
    pub fn dracula() -> Theme {
        Theme {
            bg: c(Rgb(0x1b, 0x1c, 0x25)),
            scan: c(Rgb(0x1f, 0x20, 0x2a)),
            screen: c(dracula::BG),
            fg: c(dracula::FG),
            dim: c(dracula::COMMENT),
            faint: c(Rgb(0x34, 0x37, 0x46)),
            border: c(Rgb(0x44, 0x47, 0x5a)),
            focus: c(dracula::PURPLE),
            accent: c(dracula::PURPLE),
            accent2: c(dracula::PINK),
            ok: c(dracula::GREEN),
            warn: c(dracula::ORANGE),
            err: c(dracula::RED),
            info: c(dracula::CYAN),
            sel_bg: c(dracula::CURRENT),
        }
    }

    /// A single-hue CRT theme built from one phosphor colour.
    pub fn mono(hue: Rgb) -> Theme {
        let black = Rgb(0x05, 0x07, 0x05);
        let at = |t: f32| c(black.mix(hue, t));
        Theme {
            bg: at(0.0),
            scan: at(0.04),
            screen: at(0.07),
            fg: c(hue),
            dim: at(0.5),
            faint: at(0.18),
            border: at(0.35),
            focus: c(hue),
            accent: c(hue),
            accent2: c(hue.mix(Rgb(255, 255, 255), 0.4)),
            ok: c(hue),
            warn: c(hue.mix(Rgb(255, 255, 255), 0.3)),
            err: c(hue.mix(Rgb(255, 255, 255), 0.6)),
            info: at(0.8),
            sel_bg: at(0.22),
        }
    }

    pub fn for_palette(name: &str) -> Theme {
        match name {
            "phosphor" => Theme::mono(Rgb(0x33, 0xff, 0x66)),
            "amber" => Theme::mono(Rgb(0xff, 0xb0, 0x00)),
            _ => Theme::dracula(),
        }
    }

    pub fn text(&self) -> Style {
        Style::new().fg(self.fg)
    }
    pub fn dim(&self) -> Style {
        Style::new().fg(self.dim)
    }
    pub fn faint(&self) -> Style {
        Style::new().fg(self.faint)
    }
    pub fn title(&self) -> Style {
        Style::new().fg(self.accent).add_modifier(Modifier::BOLD)
    }
}
