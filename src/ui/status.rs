//! Bottom status bar and the `:` command line.

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

use super::theme::Theme;
use crate::app::{App, COMMANDS, Focus, Mode};

pub fn draw(f: &mut Frame, area: Rect, app: &App, t: &Theme) {
    if matches!(app.mode, Mode::Command) {
        draw_cmdline(f, area, app, t);
        return;
    }
    let (mode, color) = match &app.mode {
        Mode::Normal | Mode::Boot => ("NORMAL", t.accent),
        Mode::Command => ("COMMAND", t.accent2),
        Mode::Edit(_) => ("INSERT", t.ok),
        Mode::Picker(_) => ("COLOUR", t.warn),
        Mode::Glyphs(_) => ("GLYPH", t.warn),
        Mode::Globals(_) => ("GLOBAL", t.info),
        Mode::Help(_) => ("HELP", t.info),
        Mode::TomlView(_) => ("TOML", t.info),
        Mode::Matrix(_) => ("WAKE UP", t.ok),
        Mode::ConfirmQuit => ("QUIT?", t.err),
        Mode::Review(_) => ("REVIEW", t.warn),
        Mode::Backups(_) => ("BACKUPS", t.info),
        Mode::Gallery(g) => (
            match g.kind {
                crate::app::GalleryKind::Presets => "PRESETS",
                crate::app::GalleryKind::Themes => "THEMES",
                crate::app::GalleryKind::Saved => "SAVED",
            },
            t.info,
        ),
        Mode::Welcome => ("WELCOME", t.ok),
    };
    let flash = app.flash.as_ref().filter(|(_, at)| at.elapsed().as_millis() < 1500);
    let dirty = if let Some((msg, _)) = flash {
        Span::styled(format!(" ✔ {msg} "), Style::new().fg(t.bg).bg(t.ok).add_modifier(Modifier::BOLD))
    } else if app.dirty() {
        let style = if super::blink_on(app.started, 700) { Style::new().fg(t.warn).add_modifier(Modifier::BOLD) } else { Style::new().fg(t.warn) };
        Span::styled(" [+] UNSAVED ", style)
    } else {
        Span::styled(" ✔ synced ", Style::new().fg(t.ok))
    };
    let g = &app.model.globals;
    let left = Line::from(vec![
        Span::styled(format!(" {mode} "), Style::new().fg(t.bg).bg(color).add_modifier(Modifier::BOLD)),
        Span::styled("\u{e0b0}", Style::new().fg(color).bg(t.scan)),
        Span::styled(format!(" {} ", ellipsize_left(&app.config_display(), 34)), t.text()),
        dirty,
        Span::styled("│ ", t.faint()),
        Span::styled(format!("{} · {} · {} · {}", g.palette, g.shape.label(), g.layout.label(), g.glyphs.label()), t.dim()),
    ]);
    let hints: &[(&str, &str)] = match (&app.mode, app.focus) {
        (Mode::Normal, Focus::Modules) => &[("␣", "on/off"), ("J/K", "reorder"), ("⏎", "edit module"), (":", "command"), ("?", "help")],
        (Mode::Normal, Focus::Inspector) => &[("j/k", "field"), ("⏎", "edit"), ("x", "reset"), ("esc", "back"), (":", "cmd"), ("?", "help")],
        (Mode::Edit(_), _) => &[("⏎", "apply"), ("esc", "cancel"), ("^u", "clear")],
        (Mode::Picker(_), _) => &[("hjkl", "move"), ("#", "hex"), ("⏎", "apply"), ("esc", "revert")],
        (Mode::Globals(_), _) => &[("j/k", "row"), ("h/l", "change"), ("esc", "close")],
        (Mode::Review(_), _) => &[("y/⏎", "install"), ("j/k", "scroll diff"), ("esc", "cancel")],
        (Mode::Backups(_), _) => &[("j/k", "pick"), ("⏎", "restore…"), ("l", "load into editor"), ("esc", "close")],
        (Mode::Gallery(_), _) => &[("j/k", "browse"), ("⏎", "apply"), ("esc", "close")],
        (Mode::ConfirmQuit, _) => &[("y", "discard & quit"), ("w", "save & quit"), ("any", "cancel")],
        _ => &[("esc", "close")],
    };
    let mut right = Vec::new();
    for (k, v) in hints {
        right.push(Span::styled(format!(" {k}"), Style::new().fg(t.accent2)));
        right.push(Span::styled(format!(" {v}"), t.dim()));
    }
    right.push(Span::raw(" "));
    f.render_widget(Paragraph::new(left).style(Style::new().bg(t.scan)), area);
    let right = Line::from(right);
    if (right.width() + 60) as u16 <= area.width {
        f.render_widget(Paragraph::new(right).alignment(Alignment::Right), area);
    }
}

fn draw_cmdline(f: &mut Frame, area: Rect, app: &App, t: &Theme) {
    let mut spans = vec![Span::styled(":", Style::new().fg(t.accent2).add_modifier(Modifier::BOLD))];
    spans.extend(super::inspector::edit_spans(&app.cmdline.buf, app.cmdline.cursor, app, t));
    f.render_widget(Paragraph::new(Line::from(spans)).style(Style::new().bg(t.scan)), area);

    // Live completion menu above the command line.
    let text = app.cmdline.text();
    let matches = app.completions_for(&text);
    if matches.is_empty() || area.y < 2 {
        return;
    }
    let show: Vec<&String> = matches.iter().take(8).collect();
    let h = show.len() as u16;
    let w = 58.min(area.width);
    let r = Rect::new(area.x, area.y.saturating_sub(h), w, h);
    f.render_widget(Clear, r);
    let lines: Vec<Line> = show
        .iter()
        .map(|m| {
            let help = COMMANDS.iter().find(|(c, _)| c == m).map(|(_, h)| *h).unwrap_or("");
            Line::from(vec![
                Span::styled(format!(" :{m:<10}"), Style::new().fg(t.accent2)),
                Span::styled(help.to_string(), t.dim()),
            ])
        })
        .collect();
    f.render_widget(Paragraph::new(lines).style(Style::new().bg(t.sel_bg)), r);
}

/// Keeps the end of a path, which is the informative part.
fn ellipsize_left(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n <= w {
        return s.to_string();
    }
    let tail: String = s.chars().skip(n - (w - 1)).collect();
    format!("…{tail}")
}
