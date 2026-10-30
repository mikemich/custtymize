//! The scrolling action log, with a typewriter effect on fresh lines.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::theme::Theme;
use crate::app::{App, Tag};

/// Characters revealed per millisecond-ish.
const TYPE_MS_PER_CHAR: u128 = 4;

pub fn draw(f: &mut Frame, area: Rect, app: &App, t: &Theme) {
    let (u, r) = app.can_undo();
    let block = super::pane(t, "03", "log", false).title_bottom(
        Line::styled(format!(" undo {u} · redo {r} "), t.faint()).alignment(ratatui::layout::Alignment::Right),
    );
    let inner = block.inner(area);
    f.render_widget(block, area);
    super::scanlines(f.buffer_mut(), inner, t);

    let h = inner.height as usize;
    let start = app.log.len().saturating_sub(h);
    let lines: Vec<Line> = app
        .log
        .iter()
        .skip(start)
        .map(|l| {
            let tag_style = match l.tag {
                Tag::Write => Style::new().fg(t.accent2),
                Tag::Ok => Style::new().fg(t.ok),
                Tag::Warn => Style::new().fg(t.warn),
                Tag::Err => Style::new().fg(t.err).add_modifier(Modifier::BOLD),
                Tag::Cmd => Style::new().fg(t.info),
                Tag::Preview => Style::new().fg(t.accent),
                Tag::Backup => Style::new().fg(t.warn),
                Tag::Sys => t.dim(),
            };
            let total = l.text.chars().count();
            let shown = ((l.at.elapsed().as_millis() / TYPE_MS_PER_CHAR) as usize + 1).min(total);
            let text: String = l.text.chars().take(shown).collect();
            let mut spans = vec![
                Span::styled(format!(" {} ", l.clock), t.faint()),
                Span::styled(format!("[{:>5}]", l.tag.label()), tag_style),
                Span::raw(" "),
                Span::styled(text, if l.tag == Tag::Err { Style::new().fg(t.err) } else { t.text() }),
            ];
            if shown < total {
                spans.push(Span::styled("█", Style::new().fg(t.accent)));
            }
            Line::from(spans)
        })
        .collect();
    f.render_widget(Paragraph::new(lines), inner);
}
