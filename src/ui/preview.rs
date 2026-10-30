//! The live preview pane: the prompt engine's output, success and error states.

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;

use super::theme::Theme;
use crate::app::{App, RenderedPrompt};

pub(super) const LABEL_W: u16 = 11;
const SPINNER: [&str; 8] = ["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"];

fn lines_of(p: &RenderedPrompt) -> u16 {
    p.left.lines.len().max(1) as u16
}

pub fn height(app: &App) -> u16 {
    let (a, b) = match &app.preview {
        Some(p) => (lines_of(&p.success), lines_of(&p.failure)),
        None => (1, 1),
    };
    2 + a + 1 + b
}

pub fn draw(f: &mut Frame, area: Rect, app: &mut App, t: &Theme) {
    let spin = SPINNER[(app.started.elapsed().as_millis() / 80) as usize % SPINNER.len()];
    // Border flashes briefly when a fresh render lands.
    let fresh = app.preview.as_ref().is_some_and(|p| p.received.elapsed().as_millis() < 160);
    let mut block = super::pane(t, "00", "live preview", fresh);
    let meta = match &app.preview {
        Some(p) => format!(" {} · {}ms ", app.scenario.label(), p.elapsed.as_millis()),
        None => format!(" {} ", app.scenario.label()),
    };
    block = block.title(
        Line::from(vec![
            Span::styled(if app.preview_pending { format!("{spin} ") } else { "  ".into() }, Style::new().fg(t.warn)),
            Span::styled(meta, t.dim()),
            Span::styled("s", Style::new().fg(t.accent2)),
            Span::styled(":scenario ", t.faint()),
        ])
        .alignment(Alignment::Right),
    );
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.width <= LABEL_W + 4 || inner.height == 0 {
        return;
    }
    app.set_preview_width(inner.width - LABEL_W - 1);

    let Some(p) = &app.preview else {
        f.render_widget(Paragraph::new(Line::styled(format!(" {spin} warming up the preview reactor…"), t.dim())), inner);
        return;
    };
    let ok_h = lines_of(&p.success);
    let fail_h = lines_of(&p.failure);
    let ok_area = Rect::new(inner.x, inner.y, inner.width, ok_h);
    let div = Rect::new(inner.x, inner.y + ok_h, inner.width, 1);
    let fail_area = Rect::new(inner.x, inner.y + ok_h + 1, inner.width, fail_h.min(inner.height.saturating_sub(ok_h + 1)));

    draw_state(f, ok_area, &p.success, "✔ $?=0", Style::new().fg(t.ok), t);
    f.render_widget(Paragraph::new(Line::styled("┄".repeat(inner.width as usize), t.faint())), div);
    draw_state(f, fail_area, &p.failure, "✘ $?=1", Style::new().fg(t.err), t);

    if let Some(e) = &p.error {
        let w = (e.chars().count() as u16 + 2).min(inner.width);
        let r = Rect::new(inner.right() - w, inner.y, w, 1);
        f.render_widget(Paragraph::new(Line::styled(e.clone(), Style::new().fg(t.err).add_modifier(Modifier::BOLD))), r);
    }
}

pub(super) fn draw_state(f: &mut Frame, area: Rect, prompt: &RenderedPrompt, label: &str, label_style: Style, t: &Theme) {
    if area.height == 0 {
        return;
    }
    let label_area = Rect::new(area.x, area.y, LABEL_W, area.height);
    let screen = Rect::new(area.x + LABEL_W, area.y, area.width - LABEL_W, area.height);
    f.render_widget(Paragraph::new(Line::styled(format!(" {label}"), label_style.add_modifier(Modifier::BOLD))), label_area);

    // The "terminal" behind the prompt, with a faint dot grid.
    let buf = f.buffer_mut();
    for y in screen.top()..screen.bottom() {
        for x in screen.left()..screen.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_bg(t.screen);
                if (x - screen.left()) % 4 == 3 {
                    cell.set_symbol("·").set_fg(t.faint);
                }
            }
        }
    }
    let text: Text = prompt.left.clone();
    f.render_widget(Paragraph::new(text), screen);
    // The right prompt sits on the last line of the left one.
    if !prompt.right.lines.is_empty() {
        let last = screen.y + (prompt.left.lines.len().max(1) as u16 - 1).min(screen.height - 1);
        let row = Rect::new(screen.x, last, screen.width.saturating_sub(1), 1);
        let right_line = prompt.right.lines.first().cloned().unwrap_or_default();
        let w = right_line.width() as u16;
        let left_w = prompt.left.lines.last().map(|l| l.width() as u16).unwrap_or(0);
        // Like zsh: a right prompt that would collide with the left one isn't drawn.
        if w > 0 && left_w + 1 + w <= row.width {
            let r = Rect::new(row.right() - w, row.y, w, 1);
            f.render_widget(Paragraph::new(right_line), r);
        }
    }
}
