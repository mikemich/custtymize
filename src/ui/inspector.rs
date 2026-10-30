//! Right pane: fields of the selected module plus its generated TOML.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use super::theme::Theme;
use crate::app::{App, Field, Focus, Mode};
use crate::config::generate;
use crate::model::catalog::OptKind;
use crate::model::{Segment, Side};

const LABEL_W: usize = 20;

pub fn draw(f: &mut Frame, area: Rect, app: &App, t: &Theme) {
    let focused = app.focus == Focus::Inspector;
    let Some(seg) = app.selected() else {
        f.render_widget(super::pane(t, "02", "inspector", focused), area);
        return;
    };
    let title = format!("inspector :: {}", if seg.literal.is_some() { "text" } else { seg.name.as_str() });
    let block = super::pane(t, "02", &title, focused);
    let inner = block.inner(area);
    f.render_widget(block, area);
    super::scanlines(f.buffer_mut(), inner, t);

    let fields = app.fields();
    let snippet = generate::module_snippet(&app.model, seg);
    let fields_h = fields.len() as u16 + 3;
    let [top, bottom] = Layout::vertical([Constraint::Length(fields_h), Constraint::Min(0)]).areas(inner);

    let mut lines = Vec::new();
    let blurb = if seg.literal.is_some() {
        "Literal text from your top-level format (kept verbatim)"
    } else if seg.raw {
        "Module custtymize doesn't manage — its table is left untouched"
    } else {
        seg.def().blurb
    };
    lines.push(Line::from(vec![Span::styled(" ▸ ", Style::new().fg(t.accent2)), Span::styled(blurb, t.dim().add_modifier(Modifier::ITALIC))]));
    lines.push(Line::raw(""));
    for (i, field) in fields.iter().enumerate() {
        let selected = focused && i == app.field;
        let editing = match &app.mode {
            Mode::Edit(e) if selected && !e.from_globals => Some(e),
            _ => None,
        };
        let marker = if selected { Span::styled(" ❯ ", Style::new().fg(t.accent2)) } else { Span::raw("   ") };
        let dots = LABEL_W.saturating_sub(field.label().chars().count() + 1);
        let label = Span::styled(format!("{} ", field.label()), if selected { t.text().add_modifier(Modifier::BOLD) } else { t.dim() });
        let leader = Span::styled(format!("{} ", "·".repeat(dots)), t.faint());
        let mut spans = vec![marker, label, leader];
        match editing {
            Some(e) => spans.extend(edit_spans(&e.buf, e.cursor, app, t)),
            None => spans.extend(value_spans(app, seg, *field, t)),
        }
        let style = if selected { Style::new().bg(t.sel_bg) } else { Style::new() };
        lines.push(Line::from(spans).style(style));
    }
    // Contextual hint for the focused field.
    let hint = match fields.get(app.field) {
        Some(Field::Opt(o)) => format!("{} — default {}", o.help, if o.default.is_empty() { "\"\"" } else { o.default }),
        Some(Field::Symbol) => "⏎ edit · p glyph picker · x reset".into(),
        Some(Field::Fg | Field::Bg | Field::Success | Field::Error) => "⏎ colour picker (live preview) · # hex".into(),
        Some(Field::Format) => "⏎ edit raw format · x back to generated".into(),
        _ => "⏎/␣ toggle · x reset".into(),
    };
    if focused {
        lines.push(Line::styled(format!("   {hint}"), t.faint()));
    }
    f.render_widget(Paragraph::new(lines), top);

    if bottom.height > 2 {
        let mut out = vec![Line::from(vec![
            Span::styled(" ── ", t.faint()),
            Span::styled("generated toml", t.dim()),
            Span::styled(format!(" {}", "─".repeat(bottom.width.saturating_sub(20) as usize)), t.faint()),
        ])];
        out.extend(snippet.lines().map(|l| {
            let mut line = super::toml_line(&format!(" {l}"), t);
            line.spans.truncate(8);
            line
        }));
        f.render_widget(Paragraph::new(out).wrap(Wrap { trim: false }), bottom);
    }
}

fn on_off(b: bool, t: &Theme) -> Vec<Span<'static>> {
    if b {
        vec![Span::styled("■ on", Style::new().fg(t.ok))]
    } else {
        vec![Span::styled("□ off", t.dim())]
    }
}

fn color_spans(c: crate::model::color::Rgb, t: &Theme) -> Vec<Span<'static>> {
    vec![super::swatch(c), Span::styled(format!(" {}", c.hex()), t.text())]
}

fn value_spans(app: &App, seg: &Segment, field: Field, t: &Theme) -> Vec<Span<'static>> {
    let g = &app.model.globals;
    match field {
        Field::Enabled => on_off(seg.enabled, t),
        Field::Side => vec![Span::styled(
            match seg.side {
                Side::Left => "◂ left",
                Side::Right => "right ▸",
            },
            t.text(),
        )],
        Field::Symbol => {
            let mut v = vec![Span::styled(format!("“{}”", seg.effective_symbol()), Style::new().fg(seg.bg.into()).add_modifier(Modifier::BOLD))];
            if seg.symbol.is_none() {
                v.push(Span::styled("  (default)", t.faint()));
            }
            v
        }
        Field::Fg => color_spans(seg.fg, t),
        Field::Bg => {
            let mut v = color_spans(seg.bg, t);
            if g.shape == crate::model::Shape::Plain {
                v.push(Span::styled("  (unused in plain text)", t.faint()));
            }
            v
        }
        Field::Bold => on_off(seg.bold, t),
        Field::Italic => on_off(seg.italic, t),
        Field::Format => {
            let f = generate::module_format(seg, g.shape);
            let tag = if seg.format_override.is_some() {
                Span::styled("custom ", Style::new().fg(t.bg).bg(t.warn))
            } else {
                Span::styled("auto ", Style::new().fg(t.bg).bg(t.info))
            };
            vec![tag, Span::raw(" "), Span::styled(super::ellipsize(&f, 48), t.text())]
        }
        Field::Opt(o) => match seg.options.get(o.key) {
            Some(v) => vec![Span::styled(v.display(), Style::new().fg(t.warn))],
            None => {
                let d = match o.kind {
                    OptKind::Str => format!("{:?}", o.default),
                    _ => o.default.to_string(),
                };
                vec![Span::styled(d, t.faint()), Span::styled("  (starship default)", t.faint())]
            }
        },
        Field::PromptChar => vec![Span::styled(g.prompt_char.clone(), Style::new().fg(g.success.into()).add_modifier(Modifier::BOLD))],
        Field::Success => color_spans(g.success, t),
        Field::Error => color_spans(g.error, t),
        Field::Literal => vec![Span::styled(seg.literal.clone().unwrap_or_default(), t.text())],
    }
}

pub fn edit_spans(buf: &[char], cursor: usize, app: &App, t: &Theme) -> Vec<Span<'static>> {
    let before: String = buf[..cursor.min(buf.len())].iter().collect();
    let at: String = buf.get(cursor).map(|c| c.to_string()).unwrap_or_else(|| " ".into());
    let after: String = buf.get(cursor + 1..).map(|s| s.iter().collect()).unwrap_or_default();
    let cursor_style = if super::blink_on(app.started, 530) {
        Style::new().fg(t.bg).bg(t.fg)
    } else {
        Style::new().fg(t.fg).add_modifier(Modifier::UNDERLINED)
    };
    vec![
        Span::styled(before, Style::new().fg(t.ok)),
        Span::styled(at, cursor_style),
        Span::styled(after, Style::new().fg(t.ok)),
    ]
}
