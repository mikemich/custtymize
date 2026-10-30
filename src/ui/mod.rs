//! Rendering. Everything here is a pure function of `&App` (plus the preview width hint).

mod boot;
mod inspector;
mod log;
mod matrix;
mod modules;
mod popups;
mod preview;
mod status;
pub mod theme;

use std::time::Instant;

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};

use crate::app::{App, Mode};
use crate::model::color::Rgb;
use theme::Theme;

pub fn draw(f: &mut Frame, app: &mut App) {
    let t = Theme::for_palette(app.model.globals.palette);
    let area = f.area();
    f.render_widget(Block::new().style(Style::new().bg(t.bg).fg(t.fg)), area);

    match &app.mode {
        Mode::Boot => boot::draw(f, area, app, &t),
        Mode::Matrix(m) => matrix::draw(f, area, m, &t),
        _ => draw_main(f, area, app, &t),
    }

    if !app.truecolor {
        downsample(f.buffer_mut());
    }
}

fn draw_main(f: &mut Frame, area: Rect, app: &mut App, t: &Theme) {
    let preview_h = preview::height(app);
    let log_h = if area.height > 34 { 9 } else { 6 };
    let [header, prev, main, logs, status] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(preview_h),
        Constraint::Min(8),
        Constraint::Length(log_h),
        Constraint::Length(1),
    ])
    .areas(area);
    let list_w = 36.min(area.width / 2);
    let [list, insp] = Layout::horizontal([Constraint::Length(list_w), Constraint::Min(20)]).areas(main);

    draw_header(f, header, app, t);
    preview::draw(f, prev, app, t);
    modules::draw(f, list, app, t);
    inspector::draw(f, insp, app, t);
    log::draw(f, logs, app, t);
    status::draw(f, status, app, t);
    popups::draw(f, area, app, t);
}

fn draw_header(f: &mut Frame, area: Rect, app: &App, t: &Theme) {
    let blink = blink_on(app.started, 900);
    let clock = chrono::Local::now().format("%H:%M:%S").to_string();
    let left = Line::from(vec![
        Span::styled("▓▒░", Style::new().fg(t.accent)),
        Span::styled(" CUSTTYMIZE ", Style::new().fg(t.bg).bg(t.accent).add_modifier(Modifier::BOLD)),
        Span::styled("░▒▓", Style::new().fg(t.accent)),
        Span::styled("  shell prompt forge ", t.dim()),
        Span::styled("// ", t.faint()),
        Span::styled(format!("v{}", env!("CARGO_PKG_VERSION")), t.dim()),
    ]);
    let link = if app.preview_pending { ("◌", t.warn) } else if blink { ("●", t.ok) } else { ("○", t.ok) };
    let right = Line::from(vec![
        Span::styled(format!("engine v{} ", app.engine_version), t.dim()),
        Span::styled("│ ", t.faint()),
        Span::styled(format!("{} ", if app.truecolor { "24bit" } else { "256c" }), t.dim()),
        Span::styled("│ ", t.faint()),
        Span::styled(link.0, Style::new().fg(link.1)),
        Span::styled(" LINK ", t.dim()),
        Span::styled("│ ", t.faint()),
        Span::styled(clock, Style::new().fg(t.info)),
        Span::raw(" "),
    ]);
    f.render_widget(Paragraph::new(left), area);
    f.render_widget(Paragraph::new(right).alignment(ratatui::layout::Alignment::Right), area);
}

/// Standard pane chrome: rounded border, `┤ 01 TITLE ├` tab, focus colour.
pub fn pane<'a>(t: &Theme, num: &str, title: &str, focused: bool) -> Block<'a> {
    let border = Style::new().fg(if focused { t.focus } else { t.border });
    let title_style = if focused { t.title() } else { Style::new().fg(t.dim).add_modifier(Modifier::BOLD) };
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(border)
        .title(Line::from(vec![
            Span::styled("┤", border),
            Span::styled(format!(" {num} "), Style::new().fg(if focused { t.accent2 } else { t.faint })),
            Span::styled(title.to_uppercase(), title_style),
            Span::styled(" ├", border),
        ]))
}

/// Alternating row tint inside a pane: the CRT scanline look.
pub fn scanlines(buf: &mut Buffer, area: Rect, t: &Theme) {
    for y in area.top()..area.bottom() {
        if y % 2 == 1 {
            buf.set_style(Rect::new(area.x, y, area.width, 1), Style::new().bg(t.scan));
        }
    }
}

pub fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h)
}

pub fn blink_on(since: Instant, period_ms: u128) -> bool {
    (since.elapsed().as_millis() / period_ms).is_multiple_of(2)
}

pub fn swatch(c: Rgb) -> Span<'static> {
    Span::styled("██", Style::new().fg(c.into()))
}

/// Truncates to `w` display columns, adding an ellipsis.
pub fn ellipsize(s: &str, w: usize) -> String {
    let count = s.chars().count();
    if count <= w {
        return s.to_string();
    }
    let mut out: String = s.chars().take(w.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Maps every truecolour cell to the xterm-256 palette (for terminals without 24-bit colour).
fn downsample(buf: &mut Buffer) {
    let map = |c: Color| match c {
        Color::Rgb(r, g, b) => Color::Indexed(Rgb(r, g, b).to_ansi256()),
        other => other,
    };
    for cell in buf.content.iter_mut() {
        cell.fg = map(cell.fg);
        cell.bg = map(cell.bg);
    }
}

/// Minimal TOML highlighter for the snippet and full-file views.
pub fn toml_line(line: &str, t: &Theme) -> Line<'static> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return Line::styled(line.to_string(), t.dim().add_modifier(Modifier::ITALIC));
    }
    if trimmed.starts_with('[') {
        return Line::styled(line.to_string(), Style::new().fg(t.accent2).add_modifier(Modifier::BOLD));
    }
    match line.split_once('=') {
        Some((k, v)) => Line::from(vec![
            Span::styled(k.to_string(), Style::new().fg(t.info)),
            Span::styled("=", t.dim()),
            Span::styled(v.to_string(), Style::new().fg(t.warn)),
        ]),
        None => Line::styled(line.to_string(), t.text()),
    }
}
