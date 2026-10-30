//! Left pane: the ordered module list.

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::theme::Theme;
use crate::app::{App, Focus};
use crate::model::{Layout, Shape, Side};

pub fn draw(f: &mut Frame, area: Rect, app: &App, t: &Theme) {
    let focused = app.focus == Focus::Modules;
    let enabled = app.model.segments.iter().filter(|s| s.enabled).count();
    let block = super::pane(t, "01", "modules", focused).title_bottom(
        Line::from(vec![
            Span::styled(" ■", Style::new().fg(t.ok)),
            Span::styled(" on ", t.dim()),
            Span::styled("□", t.dim()),
            Span::styled(" off ", t.dim()),
            Span::styled("⇥", Style::new().fg(t.info)),
            Span::styled(" right · ", t.dim()),
            Span::styled(format!("{enabled}/{} ", app.model.segments.len()), t.dim()),
        ])
        .alignment(Alignment::Right),
    );
    let inner = block.inner(area);
    f.render_widget(block, area);
    super::scanlines(f.buffer_mut(), inner, t);

    let h = inner.height as usize;
    let n = app.model.segments.len();
    let offset = if app.sel >= h { app.sel + 1 - h } else { 0 }.min(n.saturating_sub(h));
    let name_w = (inner.width as usize).saturating_sub(14).max(6);
    let plain = app.model.globals.shape == Shape::Plain;
    let split = app.model.globals.layout == Layout::RightPrompt;

    let lines: Vec<Line> = app
        .model
        .segments
        .iter()
        .enumerate()
        .skip(offset)
        .take(h)
        .map(|(i, s)| {
            let selected = i == app.sel;
            let row_style = if selected && focused {
                Style::new().bg(t.sel_bg)
            } else if selected {
                Style::new().bg(t.faint)
            } else {
                Style::new()
            };
            let marker = if selected { Span::styled("▌", Style::new().fg(t.accent2)) } else { Span::raw(" ") };
            let idx = Span::styled(format!("{:02} ", i + 1), t.faint());
            let check = if s.enabled {
                Span::styled("■ ", Style::new().fg(t.ok))
            } else {
                Span::styled("□ ", t.dim())
            };
            let (label, label_style) = match (&s.literal, s.raw) {
                (Some(l), _) => (format!("“{l}”"), t.dim().add_modifier(Modifier::ITALIC)),
                (None, true) => (format!("{} ⟨raw⟩", s.name), t.dim()),
                _ if s.enabled => (s.name.clone(), t.text().add_modifier(if selected { Modifier::BOLD } else { Modifier::empty() })),
                _ => (s.name.clone(), t.dim()),
            };
            let name = Span::styled(format!("{:<w$}", super::ellipsize(&label, name_w), w = name_w), label_style);
            let sw = if s.raw || s.is_character() {
                Span::raw("    ")
            } else if plain {
                Span::styled(" Aa ", Style::new().fg(s.fg.into()).bg(t.screen))
            } else {
                Span::styled(" Aa ", Style::new().fg(s.fg.into()).bg(s.bg.into()))
            };
            let side = match s.side {
                Side::Right if split => Span::styled(" ⇥", Style::new().fg(t.info)),
                Side::Right => Span::styled(" ⇥", t.faint()),
                Side::Left => Span::raw("  "),
            };
            Line::from(vec![marker, idx, check, name, sw, side]).style(row_style)
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}
