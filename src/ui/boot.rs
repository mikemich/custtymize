//! Fake-but-honest boot sequence: each line reflects a real startup check.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::theme::Theme;
use crate::app::{App, BOOT_DURATION};

const LOGO: [&str; 3] = [
    "┏━╸╻ ╻┏━┓╺┳╸╺┳╸╻ ╻┏┳┓╻╺━┓┏━╸",
    "┃  ┃ ┃┗━┓ ┃  ┃ ┗┳┛┃┃┃┃┏━┛┣╸ ",
    "┗━╸┗━┛┗━┛ ╹  ╹  ╹ ╹ ╹╹┗━╸┗━╸",
];

pub fn draw(f: &mut Frame, area: Rect, app: &App, t: &Theme) {
    let elapsed = app.started.elapsed();
    let n = app.boot.len().max(1);
    let step = BOOT_DURATION.as_millis() / n as u128;
    let visible = ((elapsed.as_millis() / step.max(1)) as usize + 1).min(n);

    let w = 72.min(area.width);
    let h = (n as u16 + 12).min(area.height);
    let r = super::centered(area, w, h);
    super::scanlines(f.buffer_mut(), area, t);

    let mut lines: Vec<Line> = LOGO
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let c = match i {
                0 => t.accent2,
                1 => t.accent,
                _ => t.info,
            };
            Line::styled(format!("  {l}"), Style::new().fg(c).add_modifier(Modifier::BOLD))
        })
        .collect();
    lines.push(Line::styled(format!("  starship prompt forge · bios v{} · (c) 2026 nobody", env!("CARGO_PKG_VERSION")), t.dim()));
    lines.push(Line::raw(""));

    for (i, b) in app.boot.iter().take(visible).enumerate() {
        let stamp = Span::styled(format!("  [{:>7.3}] ", (i as u128 * step) as f64 / 1000.0), t.faint());
        let label_w = (w as usize).saturating_sub(34);
        let dots = label_w.saturating_sub(b.text.chars().count());
        let mut spans = vec![
            stamp,
            Span::styled(super::ellipsize(&b.text, label_w), t.text()),
            Span::styled(format!(" {} ", ".".repeat(dots)), t.faint()),
        ];
        let newest = i + 1 == visible && visible < n;
        if newest {
            spans.push(Span::styled("[ .. ]", Style::new().fg(t.warn)));
        } else {
            match &b.status {
                Ok(detail) => {
                    spans.push(Span::styled("[ ", t.dim()));
                    spans.push(Span::styled("OK", Style::new().fg(t.ok).add_modifier(Modifier::BOLD)));
                    spans.push(Span::styled(" ]", t.dim()));
                    if !detail.is_empty() {
                        spans.push(Span::styled(format!(" {detail}"), t.dim()));
                    }
                }
                Err(e) => {
                    spans.push(Span::styled("[", t.dim()));
                    spans.push(Span::styled("FAIL", Style::new().fg(t.err).add_modifier(Modifier::BOLD)));
                    spans.push(Span::styled("]", t.dim()));
                    spans.push(Span::styled(format!(" {e}"), Style::new().fg(t.err)));
                }
            }
        }
        lines.push(Line::from(spans));
    }

    let pct = (elapsed.as_millis() * 100 / BOOT_DURATION.as_millis()).min(100) as usize;
    let bar_w = (w as usize).saturating_sub(16);
    let filled = bar_w * pct / 100;
    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::styled("  ", t.dim()),
        Span::styled("▰".repeat(filled), Style::new().fg(t.accent)),
        Span::styled("▱".repeat(bar_w - filled), t.faint()),
        Span::styled(format!(" {pct:>3}%"), t.text()),
    ]));
    let cursor = if super::blink_on(app.started, 300) { "█" } else { " " };
    lines.push(Line::from(vec![
        Span::styled(if pct >= 100 { "  system ready" } else { "  press any key to skip" }, t.dim()),
        Span::styled(cursor, Style::new().fg(t.accent)),
    ]));
    f.render_widget(Paragraph::new(lines), r);
}
