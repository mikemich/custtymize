//! Modal overlays: globals, colour picker, glyph picker, help, TOML viewer, quit confirm.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

use super::theme::Theme;
use crate::app::{App, BackupBrowser, ColorPicker, GLYPH_COLS, GlobalRow, InstallKind, InstallReview, Mode, RenderedPrompt};
use crate::config::diff::{self, DiffLine};
use crate::model::catalog::GLYPHS;
use crate::model::color::Rgb;
use crate::model::palette::PALETTES;

pub fn draw(f: &mut Frame, area: Rect, app: &App, t: &Theme) {
    match &app.mode {
        Mode::Globals(row) => globals(f, area, app, *row, t),
        Mode::Edit(e) if e.from_globals => globals(f, area, app, 3, t),
        Mode::Picker(p) => picker(f, area, app, p, t),
        Mode::Glyphs(g) => glyphs(f, area, g.idx, t),
        Mode::Help(scroll) => help(f, area, *scroll, t),
        Mode::TomlView(scroll) => toml_view(f, area, app, *scroll, t),
        Mode::ConfirmQuit => confirm(f, area, t),
        Mode::Review(r) => review(f, area, app, r, t),
        Mode::Backups(b) => backups(f, area, b, t),
        _ => {}
    }
}

fn frame(f: &mut Frame, r: Rect, num: &str, title: &str, t: &Theme) -> Rect {
    // Drop shadow.
    let shadow = Rect::new(r.x + 1, r.y + 1, r.width, r.height).intersection(f.area());
    f.render_widget(Clear, shadow);
    f.render_widget(Paragraph::new("").style(Style::new().bg(t.faint)), shadow);
    f.render_widget(Clear, r);
    let block = super::pane(t, num, title, true).style(Style::new().bg(t.bg));
    let inner = block.inner(r);
    f.render_widget(block, r);
    inner
}

fn globals(f: &mut Frame, area: Rect, app: &App, row: usize, t: &Theme) {
    let r = super::centered(area, 60, GlobalRow::ALL.len() as u16 + 5);
    let inner = frame(f, r, "G", "global options", t);
    let g = &app.model.globals;
    let mut lines = vec![Line::raw("")];
    for (i, gr) in GlobalRow::ALL.iter().enumerate() {
        let sel = i == row;
        let editing = matches!(&app.mode, Mode::Edit(e) if e.from_globals) && *gr == GlobalRow::PromptChar;
        let value: Vec<Span> = match gr {
            _ if editing => match &app.mode {
                Mode::Edit(e) => super::inspector::edit_spans(&e.buf, e.cursor, app, t),
                _ => vec![],
            },
            GlobalRow::Layout => vec![Span::raw(g.layout.label())],
            GlobalRow::Shape => vec![Span::raw(g.shape.label())],
            GlobalRow::Cap => vec![Span::raw(g.cap.label())],
            GlobalRow::PromptChar => vec![Span::styled(g.prompt_char.clone(), Style::new().fg(g.success.into()).add_modifier(Modifier::BOLD))],
            GlobalRow::Success => vec![super::swatch(g.success), Span::raw(format!(" {}", g.success))],
            GlobalRow::Error => vec![super::swatch(g.error), Span::raw(format!(" {}", g.error))],
            GlobalRow::Palette => {
                let mut v = vec![Span::raw(format!("{:<18}", g.palette))];
                if let Some(p) = crate::model::palette::find(g.palette) {
                    v.extend(p.accents.iter().map(|c| Span::styled("▮", Style::new().fg((*c).into()))));
                }
                v
            }
            GlobalRow::AddNewline => vec![Span::raw(if g.add_newline { "yes" } else { "no" })],
        };
        let cycles = !matches!(gr, GlobalRow::PromptChar | GlobalRow::Success | GlobalRow::Error);
        let mut spans = vec![
            Span::styled(if sel { " ❯ " } else { "   " }, Style::new().fg(t.accent2)),
            Span::styled(format!("{:<20}", gr.label()), if sel { t.text().add_modifier(Modifier::BOLD) } else { t.dim() }),
        ];
        if sel && cycles {
            spans.push(Span::styled("◂ ", Style::new().fg(t.accent2)));
        } else {
            spans.push(Span::raw("  "));
        }
        spans.extend(value);
        if sel && cycles {
            spans.push(Span::styled(" ▸", Style::new().fg(t.accent2)));
        }
        lines.push(Line::from(spans).style(if sel { Style::new().bg(t.sel_bg) } else { Style::new() }));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled("   h/l cycle · ⏎ edit · esc close", t.faint()));
    f.render_widget(Paragraph::new(lines), inner);
}

fn picker(f: &mut Frame, area: Rect, app: &App, p: &ColorPicker, t: &Theme) {
    let target = match p.target {
        crate::app::ColorTarget::Fg => format!("{}.fg", app.selected().map(|s| s.name.as_str()).unwrap_or("?")),
        crate::app::ColorTarget::Bg => format!("{}.bg", app.selected().map(|s| s.name.as_str()).unwrap_or("?")),
        crate::app::ColorTarget::Success => "character.success".into(),
        crate::app::ColorTarget::Error => "character.error".into(),
    };
    let r = super::centered(area, 74, p.rows.len() as u16 + 8);
    let inner = frame(f, r, "C", &format!("colour // {target}"), t);
    let mut lines = vec![Line::raw("")];
    for (ri, (name, sw)) in p.rows.iter().enumerate() {
        let mut spans = vec![Span::styled(format!(" {:<17}", super::ellipsize(name, 16)), if ri == p.row { t.text() } else { t.dim() })];
        for (ci, (_, c)) in sw.iter().enumerate() {
            let sel = ri == p.row && ci == p.col && p.hex.is_none();
            let color: ratatui::style::Color = (*c).into();
            if sel {
                spans.push(Span::styled("[", Style::new().fg(t.fg).add_modifier(Modifier::BOLD)));
                spans.push(Span::styled("██", Style::new().fg(color)));
                spans.push(Span::styled("]", Style::new().fg(t.fg).add_modifier(Modifier::BOLD)));
            } else {
                spans.push(Span::styled(" ██ ", Style::new().fg(color)));
            }
        }
        lines.push(Line::from(spans));
    }
    lines.push(Line::raw(""));
    let cur: Option<Rgb> = p.selected();
    let label = p.rows.get(p.row).and_then(|(_, r)| r.get(p.col)).map(|(n, _)| n.clone()).unwrap_or_default();
    let mut info = vec![Span::raw(" ")];
    match (&p.hex, cur) {
        (Some(h), c) => {
            info.push(Span::styled("hex › ", Style::new().fg(t.accent2)));
            info.push(Span::styled(h.clone(), t.text().add_modifier(Modifier::BOLD)));
            info.push(Span::styled("█", Style::new().fg(t.fg)));
            if let Some(c) = c {
                info.push(Span::raw("  "));
                info.push(Span::styled("  sample  ", Style::new().bg(c.into()).fg(text_on(c))));
            } else {
                info.push(Span::styled("  (incomplete)", t.faint()));
            }
        }
        (None, Some(c)) => {
            info.push(Span::styled("  sample  ", Style::new().bg(c.into()).fg(text_on(c))));
            info.push(Span::styled(format!("  {}  {label}", c.hex()), t.text()));
        }
        (None, None) => {}
    }
    lines.push(Line::from(info));
    lines.push(Line::styled(" hjkl move · # type hex · ⏎ apply · esc revert (preview is live)", t.faint()));
    f.render_widget(Paragraph::new(lines), inner);
}

fn text_on(c: Rgb) -> ratatui::style::Color {
    if c.is_light() { Rgb(0x28, 0x2a, 0x36).into() } else { Rgb(0xf8, 0xf8, 0xf2).into() }
}

fn glyphs(f: &mut Frame, area: Rect, idx: usize, t: &Theme) {
    let rows = GLYPHS.len().div_ceil(GLYPH_COLS) as u16;
    let r = super::centered(area, 8 + GLYPH_COLS as u16 * 6, rows + 6);
    let inner = frame(f, r, "Ⅎ", "glyph picker (nerd font)", t);
    let mut lines = vec![Line::raw("")];
    for (ri, chunk) in GLYPHS.chunks(GLYPH_COLS).enumerate() {
        let mut spans = vec![Span::raw("  ")];
        for (ci, (g, _)) in chunk.iter().enumerate() {
            let sel = ri * GLYPH_COLS + ci == idx;
            let style = if sel { Style::new().fg(t.bg).bg(t.accent2).add_modifier(Modifier::BOLD) } else { t.text() };
            spans.push(Span::styled(format!("  {g}  "), style));
            spans.push(Span::raw(" "));
        }
        lines.push(Line::from(spans));
    }
    lines.push(Line::raw(""));
    if let Some((g, name)) = GLYPHS.get(idx) {
        let cp = g.chars().next().map(|c| format!("U+{:04X}", c as u32)).unwrap_or_default();
        lines.push(Line::from(vec![
            Span::styled(format!("  {g}  "), Style::new().fg(t.accent).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{name}  "), t.text()),
            Span::styled(cp, t.dim()),
            Span::styled("   ⏎ use · esc cancel", t.faint()),
        ]));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

const KEYS: &[(&str, &str)] = &[
    ("j/k ↑/↓", "move selection"),
    ("tab", "switch modules ⇄ inspector"),
    ("space", "toggle module on/off"),
    ("J / K", "move module down/up in format"),
    ("R", "send module to the left/right prompt"),
    ("D", "remove module · a add module"),
    ("⏎", "edit field / open picker"),
    ("x / ⌫", "reset field to default"),
    ("p", "glyph picker (on symbol)"),
    ("g", "global options"),
    ("S L C P", "cycle shape / layout / cap / preset"),
    ("s", "cycle preview scenario"),
    ("u / ^r", "undo / redo"),
    ("^s", "install: review before/after, then backup → write"),
    ("B", "browse & restore backups"),
    ("t", "view generated TOML"),
    ("y", "yank TOML to clipboard"),
    ("q", "quit"),
    ("M", "???"),
];

fn help(f: &mut Frame, area: Rect, scroll: u16, t: &Theme) {
    let r = super::centered(area, 92, 32);
    let inner = frame(f, r, "?", "help // keybindings & commands", t);
    let mut lines = vec![Line::styled(" KEYS", t.title())];
    for (k, v) in KEYS {
        lines.push(Line::from(vec![Span::styled(format!("  {k:<10}"), Style::new().fg(t.accent2)), Span::styled(*v, t.text())]));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled(" COMMANDS", t.title()));
    for (c, h) in crate::app::COMMANDS {
        lines.push(Line::from(vec![Span::styled(format!("  :{c:<9}"), Style::new().fg(t.info)), Span::styled(*h, t.text())]));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled(" presets: ".to_string() + &PALETTES.iter().map(|p| p.name).collect::<Vec<_>>().join(" · "), t.dim()));
    f.render_widget(Paragraph::new(lines).scroll((scroll, 0)), inner);
}

fn toml_view(f: &mut Frame, area: Rect, app: &App, scroll: u16, t: &Theme) {
    let r = Rect::new(area.x + 4, area.y + 2, area.width.saturating_sub(8), area.height.saturating_sub(4));
    let toml = app.generated_toml();
    let disk: Vec<&str> = app.disk_text().map(|d| d.lines().collect()).unwrap_or_default();
    let changed = toml.lines().filter(|l| !disk.contains(l)).count();
    let inner = frame(f, r, "T", &format!("generated toml // {changed} changed lines vs disk"), t);
    let lines: Vec<Line> = toml
        .lines()
        .enumerate()
        .map(|(i, l)| {
            let mark = if disk.contains(&l) { Span::styled("  ", t.faint()) } else { Span::styled("+ ", Style::new().fg(t.ok).add_modifier(Modifier::BOLD)) };
            let mut line = super::toml_line(l, t);
            line.spans.insert(0, mark);
            line.spans.insert(0, Span::styled(format!("{:>4} ", i + 1), t.faint()));
            line
        })
        .collect();
    f.render_widget(Paragraph::new(lines).scroll((scroll, 0)), inner);
}

fn confirm(f: &mut Frame, area: Rect, t: &Theme) {
    let r = super::centered(area, 52, 7);
    let inner = frame(f, r, "!", "unsaved changes", t);
    let lines = vec![
        Line::raw(""),
        Line::styled("  You have unsaved changes. Abandon them?", t.text()),
        Line::raw(""),
        Line::from(vec![
            Span::styled("  y", Style::new().fg(t.err).add_modifier(Modifier::BOLD)),
            Span::styled(" discard & quit   ", t.dim()),
            Span::styled("w", Style::new().fg(t.ok).add_modifier(Modifier::BOLD)),
            Span::styled(" save & quit   ", t.dim()),
            Span::styled("esc", Style::new().fg(t.accent2)),
            Span::styled(" cancel", t.dim()),
        ]),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}

/// Draws a success/failure pair of prompts, returning the rows used.
fn prompt_pair(f: &mut Frame, area: Rect, pair: &(RenderedPrompt, RenderedPrompt), t: &Theme) -> u16 {
    let h0 = pair.0.left.lines.len().max(1) as u16;
    let h1 = pair.1.left.lines.len().max(1) as u16;
    let a = Rect::new(area.x, area.y, area.width, h0.min(area.height));
    super::preview::draw_state(f, a, &pair.0, "✔ $?=0", Style::new().fg(t.ok), t);
    if area.height > h0 {
        let b = Rect::new(area.x, area.y + h0, area.width, h1.min(area.height - h0));
        super::preview::draw_state(f, b, &pair.1, "✘ $?=1", Style::new().fg(t.err), t);
    }
    h0 + h1
}

fn pair_height(p: &(RenderedPrompt, RenderedPrompt)) -> u16 {
    p.0.left.lines.len().max(1) as u16 + p.1.left.lines.len().max(1) as u16
}

fn heading(text: String, style: Style, t: &Theme) -> Line<'static> {
    Line::from(vec![Span::styled(" ▍", style), Span::styled(text, style.add_modifier(Modifier::BOLD)), Span::styled(" ", t.faint())])
}

fn review(f: &mut Frame, area: Rect, app: &App, r: &InstallReview, t: &Theme) {
    let outer = Rect::new(area.x + 2, area.y + 1, area.width.saturating_sub(4), area.height.saturating_sub(2));
    let title = match &r.kind {
        InstallKind::Generated => format!("install prompt → {}", app.config_display()),
        InstallKind::Restore(p) => format!("restore {} → {}", p.file_name().and_then(|n| n.to_str()).unwrap_or("backup"), app.config_display()),
    };
    let inner = frame(f, outer, "!", &title, t);
    let mut y = inner.y;
    let line = |f: &mut Frame, y: &mut u16, l: Line<'static>| {
        if *y < inner.bottom() {
            f.render_widget(Paragraph::new(l), Rect::new(inner.x, *y, inner.width, 1));
            *y += 1;
        }
    };

    line(f, &mut y, heading("BEFORE · what your shell shows now".into(), Style::new().fg(t.dim), t));
    match &r.before {
        Some(p) => {
            let h = pair_height(p).min(inner.bottom().saturating_sub(y));
            prompt_pair(f, Rect::new(inner.x, y, inner.width, h), p, t);
            y += h;
        }
        None => line(f, &mut y, Line::styled("   (no config file yet: defaults)", t.dim())),
    }
    line(f, &mut y, Line::raw(""));
    line(f, &mut y, heading("AFTER · what it will show".into(), Style::new().fg(t.ok), t));
    let h = pair_height(&r.after).min(inner.bottom().saturating_sub(y));
    prompt_pair(f, Rect::new(inner.x, y, inner.width, h), &r.after, t);
    y += h;
    line(f, &mut y, Line::raw(""));

    let (added, removed) = diff::stats(&r.diff);
    line(
        f,
        &mut y,
        Line::from(vec![
            Span::styled(" ▍", Style::new().fg(t.info)),
            Span::styled("CHANGES ", Style::new().fg(t.info).add_modifier(Modifier::BOLD)),
            Span::styled(format!("+{added}"), Style::new().fg(t.ok)),
            Span::styled(" ", t.dim()),
            Span::styled(format!("−{removed}"), Style::new().fg(t.err)),
            Span::styled("  (j/k to scroll)", t.faint()),
        ]),
    );

    // Footer: backup notice + keys, pinned to the bottom.
    let footer_h = 3 + r.warnings.len().min(2) as u16;
    let diff_h = inner.bottom().saturating_sub(y).saturating_sub(footer_h);
    let changed: Vec<Line> = r
        .diff
        .iter()
        .filter(|l| !matches!(l, DiffLine::Same(_)))
        .map(|l| match l {
            DiffLine::Added(s) => Line::from(vec![Span::styled("  + ", Style::new().fg(t.ok).add_modifier(Modifier::BOLD)), Span::styled(s.clone(), Style::new().fg(t.ok))]),
            DiffLine::Removed(s) => Line::from(vec![Span::styled("  − ", Style::new().fg(t.err).add_modifier(Modifier::BOLD)), Span::styled(s.clone(), Style::new().fg(t.err))]),
            DiffLine::Same(_) => Line::raw(""),
        })
        .collect();
    let changed = if changed.is_empty() { vec![Line::styled("    (identical to what's on disk)", t.dim())] } else { changed };
    f.render_widget(Paragraph::new(changed).scroll((r.scroll, 0)), Rect::new(inner.x, y, inner.width, diff_h));

    let mut fy = inner.bottom().saturating_sub(footer_h);
    for w in r.warnings.iter().take(2) {
        line(f, &mut fy, Line::styled(format!(" ⚠ {}", super::ellipsize(w, inner.width as usize - 4)), Style::new().fg(t.warn)));
    }
    let backup = if std::path::Path::new(&app.config_path).exists() {
        format!(" ⛁ your current config is copied to {}.bak-{} first", app.config_display(), chrono::Local::now().format("%Y%m%d-%H%M%S"))
    } else {
        " ⛁ no existing config to back up".to_string()
    };
    line(f, &mut fy, Line::styled(backup, Style::new().fg(t.warn)));
    line(f, &mut fy, Line::raw(""));
    line(
        f,
        &mut fy,
        Line::from(vec![
            Span::styled(" y", Style::new().fg(t.ok).add_modifier(Modifier::BOLD)),
            Span::styled(if r.quit_after { " install & quit   " } else { " install   " }, t.dim()),
            Span::styled("esc", Style::new().fg(t.accent2).add_modifier(Modifier::BOLD)),
            Span::styled(" cancel (nothing is written)", t.dim()),
        ]),
    );
}

fn backups(f: &mut Frame, area: Rect, b: &BackupBrowser, t: &Theme) {
    // Below the list: heading, the prompt pair, a gap and the key line.
    let below = b.preview.as_ref().map(pair_height).unwrap_or(1) + 3;
    // Size to the content: list (up to 12 rows) plus a gap, what goes below it, and borders.
    let want = b.entries.len().clamp(1, 12) as u16 + 1 + below + 2;
    let h = want.min(area.height.saturating_sub(2));
    let outer = Rect::new(area.x + 2, area.y + (area.height - h) / 2, area.width.saturating_sub(4), h);
    let inner = frame(f, outer, "B", &format!("backups // {} found", b.entries.len()), t);
    let list_h = inner.height.saturating_sub(below);

    let offset = b.idx.saturating_sub(list_h.saturating_sub(1) as usize);
    let rows: Vec<Line> = b
        .entries
        .iter()
        .enumerate()
        .skip(offset)
        .take(list_h as usize)
        .map(|(i, e)| {
            let sel = i == b.idx;
            let name = e.path.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string();
            Line::from(vec![
                Span::styled(if sel { " ❯ " } else { "   " }, Style::new().fg(t.accent2)),
                Span::styled(format!("{:<44}", super::ellipsize(&name, 43)), if sel { t.text().add_modifier(Modifier::BOLD) } else { t.text() }),
                Span::styled(format!("{}  ", e.modified), t.dim()),
                Span::styled(format!("{:>6} B", e.size), t.faint()),
            ])
            .style(if sel { Style::new().bg(t.sel_bg) } else { Style::new() })
        })
        .collect();
    f.render_widget(Paragraph::new(rows), Rect::new(inner.x, inner.y, inner.width, list_h));

    let mut y = inner.y + list_h;
    f.render_widget(Paragraph::new(heading("PREVIEW".into(), Style::new().fg(t.info), t)), Rect::new(inner.x, y, inner.width, 1));
    y += 1;
    match (&b.preview, &b.preview_error) {
        (_, Some(e)) => {
            f.render_widget(Paragraph::new(Line::styled(format!("   ✘ {}", super::ellipsize(e, inner.width as usize - 6)), Style::new().fg(t.err))), Rect::new(inner.x, y, inner.width, 1));
        }
        (Some(p), None) => {
            let h = pair_height(p).min(inner.bottom().saturating_sub(y + 1));
            prompt_pair(f, Rect::new(inner.x, y, inner.width, h), p, t);
        }
        _ => {}
    }
    let keys = Line::from(vec![
        Span::styled(" ⏎", Style::new().fg(t.ok).add_modifier(Modifier::BOLD)),
        Span::styled(" restore (review first, current config backed up)   ", t.dim()),
        Span::styled("l", Style::new().fg(t.accent2).add_modifier(Modifier::BOLD)),
        Span::styled(" load into editor   ", t.dim()),
        Span::styled("esc", Style::new().fg(t.accent2).add_modifier(Modifier::BOLD)),
        Span::styled(" close", t.dim()),
    ]);
    f.render_widget(Paragraph::new(keys), Rect::new(inner.x, inner.bottom() - 1, inner.width, 1));
}
