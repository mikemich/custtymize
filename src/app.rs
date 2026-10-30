//! Application state, input handling and commands.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use ansi_to_tui::IntoText;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Color;
use ratatui::text::Text;
use toml_edit::DocumentMut;

use crate::config::{diff, generate, library, load, save};
use crate::model::catalog::{self, OptDef, OptKind};
use crate::model::color::{Rgb, dracula};
use crate::model::palette::{self, PALETTES};
use crate::model::presets::{self, PRESETS};
use crate::model::{Cap, Glyphs, Layout, Model, OptValue, Segment, Shape, Side, quote};
use crate::preview::{self, Outcome, PromptOutput, Runner};
use crate::sandbox::Scenario;

const UNDO_LIMIT: usize = 256;
const LOG_LIMIT: usize = 400;
pub const BOOT_DURATION: Duration = Duration::from_millis(600);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Focus {
    Modules,
    Inspector,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Field {
    Enabled,
    Side,
    Symbol,
    Fg,
    Bg,
    Bold,
    Italic,
    Format,
    Opt(&'static OptDef),
    PromptChar,
    Success,
    Error,
    Literal,
}

impl Field {
    pub fn label(&self) -> &'static str {
        match self {
            Field::Enabled => "enabled",
            Field::Side => "side",
            Field::Symbol => "symbol",
            Field::Fg => "fg",
            Field::Bg => "bg",
            Field::Bold => "bold",
            Field::Italic => "italic",
            Field::Format => "format",
            Field::Opt(o) => o.key,
            Field::PromptChar => "char",
            Field::Success => "success",
            Field::Error => "error",
            Field::Literal => "text",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GlobalRow {
    Layout,
    Shape,
    Cap,
    PromptChar,
    Success,
    Error,
    Palette,
    Glyphs,
    AddNewline,
}

impl GlobalRow {
    pub const ALL: [GlobalRow; 9] = [
        GlobalRow::Layout,
        GlobalRow::Shape,
        GlobalRow::Glyphs,
        GlobalRow::Cap,
        GlobalRow::PromptChar,
        GlobalRow::Success,
        GlobalRow::Error,
        GlobalRow::Palette,
        GlobalRow::AddNewline,
    ];
    pub fn label(&self) -> &'static str {
        match self {
            GlobalRow::Layout => "layout",
            GlobalRow::Shape => "segment shape",
            GlobalRow::Cap => "left cap",
            GlobalRow::PromptChar => "prompt char",
            GlobalRow::Success => "success colour",
            GlobalRow::Error => "error colour",
            GlobalRow::Palette => "colour theme",
            GlobalRow::Glyphs => "glyphs (font)",
            GlobalRow::AddNewline => "blank line before",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ColorTarget {
    Fg,
    Bg,
    Success,
    Error,
}

#[derive(Clone, Debug)]
pub enum EditTarget {
    Symbol,
    Format,
    Opt(&'static OptDef),
    PromptChar,
    Literal,
}

#[derive(Clone, Debug)]
pub struct TextEdit {
    pub buf: Vec<char>,
    pub cursor: usize,
    pub target: EditTarget,
    pub from_globals: bool,
}

impl TextEdit {
    fn new(initial: &str, target: EditTarget, from_globals: bool) -> TextEdit {
        let buf: Vec<char> = initial.chars().collect();
        TextEdit { cursor: buf.len(), buf, target, from_globals }
    }
    pub fn text(&self) -> String {
        self.buf.iter().collect()
    }
    /// Shared line-editing keys. Returns true if the key was consumed.
    fn key(&mut self, k: &KeyEvent) -> bool {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        match k.code {
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(self.buf.len()),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.buf.len(),
            KeyCode::Char('a') if ctrl => self.cursor = 0,
            KeyCode::Char('e') if ctrl => self.cursor = self.buf.len(),
            KeyCode::Char('u') if ctrl => {
                self.buf.drain(..self.cursor);
                self.cursor = 0;
            }
            KeyCode::Char('w') if ctrl => {
                let mut start = self.cursor;
                while start > 0 && self.buf[start - 1] == ' ' {
                    start -= 1;
                }
                while start > 0 && self.buf[start - 1] != ' ' {
                    start -= 1;
                }
                self.buf.drain(start..self.cursor);
                self.cursor = start;
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.buf.remove(self.cursor);
                }
            }
            KeyCode::Delete => {
                if self.cursor < self.buf.len() {
                    self.buf.remove(self.cursor);
                }
            }
            KeyCode::Char(c) if !ctrl => {
                self.buf.insert(self.cursor, c);
                self.cursor += 1;
            }
            _ => return false,
        }
        true
    }
}

#[derive(Clone, Debug)]
pub struct ColorPicker {
    pub target: ColorTarget,
    pub original: Model,
    pub rows: Vec<(String, Vec<(String, Rgb)>)>,
    pub row: usize,
    pub col: usize,
    pub hex: Option<String>,
    pub from_globals: bool,
}

impl ColorPicker {
    pub fn selected(&self) -> Option<Rgb> {
        if let Some(h) = &self.hex {
            return Rgb::parse(h);
        }
        self.rows.get(self.row).and_then(|(_, r)| r.get(self.col)).map(|(_, c)| *c)
    }
}

#[derive(Clone, Debug)]
pub struct GlyphPicker {
    pub idx: usize,
}

pub const GLYPH_COLS: usize = 8;

#[derive(Clone, Debug)]
pub struct MatrixState {
    pub drops: Vec<(f32, f32, u16)>,
    pub started: Instant,
}

#[derive(Clone, Debug)]
pub enum InstallKind {
    /// The config generated from the current edits.
    Generated,
    /// Putting an old backup back in place.
    Restore(PathBuf),
}

/// Everything shown on the "install prompt?" screen. Nothing is written until confirmed.
#[derive(Clone, Debug)]
pub struct InstallReview {
    pub kind: InstallKind,
    pub toml: String,
    /// The config currently on disk, rendered (None if there is no file yet).
    pub before: Option<(RenderedPrompt, RenderedPrompt)>,
    pub after: (RenderedPrompt, RenderedPrompt),
    pub diff: Vec<diff::DiffLine>,
    pub warnings: Vec<String>,
    pub quit_after: bool,
    pub scroll: u16,
}

#[derive(Clone, Debug)]
pub struct BackupEntry {
    pub path: PathBuf,
    pub modified: String,
    pub size: u64,
}

#[derive(Clone, Debug)]
pub struct BackupBrowser {
    pub entries: Vec<BackupEntry>,
    pub idx: usize,
    /// Rendered prompt for `entries[preview_for]`.
    pub preview: Option<(RenderedPrompt, RenderedPrompt)>,
    pub preview_for: Option<usize>,
    pub preview_error: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GalleryKind {
    Presets,
    Themes,
    Saved,
}

impl GalleryKind {
    pub fn title(&self) -> &'static str {
        match self {
            GalleryKind::Presets => "presets // complete prompt designs",
            GalleryKind::Themes => "themes // colour palettes",
            GalleryKind::Saved => "saved configs",
        }
    }
}

#[derive(Clone, Debug)]
pub struct GalleryItem {
    pub name: String,
    pub blurb: String,
    pub swatches: Vec<Rgb>,
    pub path: Option<PathBuf>,
}

/// A list of things to apply (presets, themes, saved configs) with a live preview of the
/// highlighted one rendered against the current design.
#[derive(Clone, Debug)]
pub struct Gallery {
    pub kind: GalleryKind,
    pub items: Vec<GalleryItem>,
    pub idx: usize,
    pub preview: Option<(RenderedPrompt, RenderedPrompt)>,
    pub preview_for: Option<usize>,
    pub error: Option<String>,
    /// `d` was pressed once on this saved config; a second `d` deletes it.
    pub confirm_delete: bool,
}

#[derive(Clone, Debug)]
pub enum Mode {
    Boot,
    Normal,
    Command,
    Edit(TextEdit),
    Picker(ColorPicker),
    Glyphs(GlyphPicker),
    Globals(usize),
    Help(u16),
    TomlView(u16),
    Matrix(MatrixState),
    ConfirmQuit,
    Review(Box<InstallReview>),
    Backups(BackupBrowser),
    Gallery(Box<Gallery>),
    Welcome,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tag {
    Sys,
    Ok,
    Write,
    Warn,
    Err,
    Cmd,
    Preview,
    Backup,
}

impl Tag {
    pub fn label(&self) -> &'static str {
        match self {
            Tag::Sys => "sys",
            Tag::Ok => " ok",
            Tag::Write => "write",
            Tag::Warn => "warn",
            Tag::Err => "err",
            Tag::Cmd => "cmd",
            Tag::Preview => "prv",
            Tag::Backup => "bak",
        }
    }
}

#[derive(Clone, Debug)]
pub struct LogLine {
    pub tag: Tag,
    pub text: String,
    pub at: Instant,
    pub clock: String,
}

/// Starship output converted to ratatui text, ready to draw.
#[derive(Clone, Debug, Default)]
pub struct RenderedPrompt {
    pub left: Text<'static>,
    pub right: Text<'static>,
}

#[derive(Clone, Debug)]
pub struct PreviewState {
    pub success: RenderedPrompt,
    pub failure: RenderedPrompt,
    pub elapsed: Duration,
    pub error: Option<String>,
    pub warnings: Vec<String>,
    pub received: Instant,
}

pub struct BootLine {
    pub text: String,
    pub status: Result<String, String>,
}

pub struct App {
    pub model: Model,
    pub saved: Model,
    base_model: Model,
    base_doc: DocumentMut,
    disk_text: Option<String>,
    pub config_path: PathBuf,
    undo: Vec<Model>,
    redo: Vec<Model>,

    pub mode: Mode,
    pub focus: Focus,
    pub sel: usize,
    pub field: usize,

    pub cmdline: TextEdit,
    cmd_history: Vec<String>,
    history_pos: Option<usize>,
    completion: Option<(Vec<String>, usize)>,

    pub log: VecDeque<LogLine>,
    pub boot: Vec<BootLine>,
    pub started: Instant,
    pub quit: bool,

    env: Option<preview::Env>,
    runner: Option<Runner>,
    preview_id: u64,
    pub preview_pending: bool,
    pub preview: Option<PreviewState>,
    pub preview_width: u16,
    pub scenario: Scenario,
    pub engine_version: String,
    imported: Option<String>,
    /// Show the welcome screen once the boot sequence ends.
    welcome_pending: bool,

    pub truecolor: bool,
    pub flash: Option<(String, Instant)>,
}

pub struct AppInit {
    pub config_path: PathBuf,
    pub loaded: load::Loaded,
    pub env: Option<preview::Env>,
    /// Text of a starship config being imported (not yet saved to `config_path`).
    pub imported: Option<String>,
    /// No custtymize config yet: greet the user with the three steps.
    pub first_run: bool,
    pub sandbox_error: Option<String>,
    pub truecolor: bool,
    pub skip_boot: bool,
}

fn home_relative(p: &Path) -> String {
    match dirs::home_dir() {
        Some(h) if p.starts_with(&h) => format!("~/{}", p.strip_prefix(&h).map(|r| r.display().to_string()).unwrap_or_default()),
        _ => p.display().to_string(),
    }
}

impl App {
    pub fn new(init: AppInit) -> App {
        let AppInit { config_path, loaded, env, imported, first_run, sandbox_error, truecolor, skip_boot } = init;
        let disk_text = std::fs::read_to_string(&config_path).ok();
        let runner = env.clone().map(Runner::spawn);

        let mut boot = Vec::new();
        boot.push(BootLine {
            text: "warming prompt engine".into(),
            status: Ok(format!("{} modules", crate::engine::SUPPORTED.len())),
        });
        boot.push(BootLine {
            text: format!("mounting {}", home_relative(&config_path)),
            status: Ok(if loaded.existed { format!("{} keys", loaded.doc.as_table().len()) } else if imported.is_some() { "import".into() } else { "new".into() }),
        });
        boot.push(BootLine {
            text: "building sandbox repo".into(),
            status: match &sandbox_error {
                None => Ok("neon-city".into()),
                Some(e) => Err(e.chars().take(24).collect()),
            },
        });
        for s in loaded.model.segments.iter().filter(|s| s.enabled && s.literal.is_none()) {
            boot.push(BootLine { text: format!("initialising module {}", s.name), status: Ok(String::new()) });
        }
        boot.push(BootLine { text: format!("loading palette {}", loaded.model.globals.palette), status: Ok(String::new()) });
        boot.push(BootLine { text: "engaging preview reactor".into(), status: Ok(String::new()) });

        let mut app = App {
            saved: loaded.model.clone(),
            base_model: loaded.model.clone(),
            model: loaded.model,
            base_doc: loaded.doc,
            disk_text,
            config_path,
            undo: Vec::new(),
            redo: Vec::new(),
            mode: match (skip_boot, first_run) {
                (false, _) => Mode::Boot,
                (true, true) => Mode::Welcome,
                (true, false) => Mode::Normal,
            },
            focus: Focus::Modules,
            sel: 0,
            field: 0,
            cmdline: TextEdit::new("", EditTarget::Literal, false),
            cmd_history: Vec::new(),
            history_pos: None,
            completion: None,
            log: VecDeque::new(),
            boot,
            started: Instant::now(),
            quit: false,
            env,
            runner,
            preview_id: 0,
            preview_pending: false,
            preview: None,
            preview_width: 100,
            scenario: Scenario::DirtyRepo,
            engine_version: env!("CARGO_PKG_VERSION").to_string(),
            imported,
            welcome_pending: first_run && !skip_boot,
            truecolor,
            flash: None,
        };
        app.log(Tag::Sys, format!("custtymize {} online", env!("CARGO_PKG_VERSION")));
        app.log(Tag::Sys, format!("config: {}", home_relative(&app.config_path)));
        for n in loaded.notes {
            app.log(Tag::Warn, n);
        }
        if let Some(e) = sandbox_error {
            app.log(Tag::Warn, format!("sandbox unavailable ({e}); previewing in the current dir"));
        }
        app.log(Tag::Sys, "press ? for help, : for commands".into());
        app.request_preview();
        app
    }

    // ───────────────────────────── helpers ─────────────────────────────

    pub fn log(&mut self, tag: Tag, text: String) {
        self.log.push_back(LogLine { tag, text, at: Instant::now(), clock: chrono::Local::now().format("%H:%M:%S").to_string() });
        while self.log.len() > LOG_LIMIT {
            self.log.pop_front();
        }
    }

    pub fn imported_from_starship(&self) -> bool {
        self.imported.is_some()
    }

    pub fn dirty(&self) -> bool {
        self.model != self.saved
    }

    pub fn config_display(&self) -> String {
        home_relative(&self.config_path)
    }

    pub fn selected(&self) -> Option<&Segment> {
        self.model.segments.get(self.sel)
    }

    pub fn palette(&self) -> &'static palette::Palette {
        palette::find(self.model.globals.palette).unwrap_or(palette::default_palette())
    }

    pub fn fields(&self) -> Vec<Field> {
        let Some(s) = self.selected() else { return vec![] };
        if s.literal.is_some() {
            return vec![Field::Enabled, Field::Side, Field::Literal];
        }
        if s.raw {
            return vec![Field::Enabled, Field::Side];
        }
        if s.is_character() {
            return vec![Field::Enabled, Field::PromptChar, Field::Success, Field::Error];
        }
        let mut f = vec![Field::Enabled, Field::Side];
        if s.symbol_editable() {
            f.push(Field::Symbol);
        }
        f.extend([Field::Fg, Field::Bg, Field::Bold, Field::Italic, Field::Format]);
        f.extend(s.def().options.iter().map(Field::Opt));
        f
    }

    pub fn generated_toml(&self) -> String {
        generate::render(&self.model, &self.base_doc, Some(&self.base_model))
    }

    pub fn disk_text(&self) -> Option<&str> {
        self.disk_text.as_deref()
    }

    pub fn can_undo(&self) -> (usize, usize) {
        (self.undo.len(), self.redo.len())
    }

    /// Records an edit for undo, logs it, and refreshes the preview. No-op edits are dropped.
    fn commit(&mut self, before: Model, tag: Tag, msg: String) {
        if before == self.model {
            return;
        }
        self.undo.push(before);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.log(tag, msg);
        self.request_preview();
    }

    /// Swaps in a whole new model as one undoable step.
    pub fn replace_model(&mut self, model: Model, why: &str) {
        let before = std::mem::replace(&mut self.model, model);
        self.commit(before, Tag::Write, why.to_string());
    }

    fn edit_selected(&mut self, f: impl FnOnce(&mut Segment, &Model) -> Option<String>) {
        let before = self.model.clone();
        let snapshot = self.model.clone();
        let Some(seg) = self.model.segments.get_mut(self.sel) else { return };
        if let Some(msg) = f(seg, &snapshot) {
            self.commit(before, Tag::Write, msg);
        }
    }

    fn style_msg(&self, name: &str) -> String {
        match self.model.find(name) {
            Some(s) => {
                let key = s.def().style_key().unwrap_or("style");
                format!("{name}.{key} = \"{}\"", generate::style_string(s, self.model.globals.shape))
            }
            None => name.to_string(),
        }
    }

    pub fn request_preview(&mut self) {
        let Some(runner) = &self.runner else { return };
        self.preview_id += 1;
        self.preview_pending = true;
        runner.request(preview::Request {
            id: self.preview_id,
            toml: generate::render(&self.model, &self.base_doc, Some(&self.base_model)),
            scenario: self.scenario,
            width: self.preview_width,
        });
    }

    pub fn set_preview_width(&mut self, w: u16) {
        if w != self.preview_width && w > 10 {
            self.preview_width = w;
            self.request_preview();
        }
    }

    /// Per-frame housekeeping: preview results, boot timeout.
    pub fn tick(&mut self) {
        let mut latest: Option<Outcome> = None;
        if let Some(r) = &self.runner {
            while let Ok(o) = r.rx.try_recv() {
                latest = Some(o);
            }
        }
        if let Some(o) = latest {
            if o.id == self.preview_id {
                self.preview_pending = false;
            }
            self.accept_preview(o);
        }
        if matches!(self.mode, Mode::Boot) && self.started.elapsed() > BOOT_DURATION + Duration::from_millis(350) {
            self.mode = Mode::Normal;
        }
        if self.welcome_pending && matches!(self.mode, Mode::Normal) {
            self.welcome_pending = false;
            self.mode = Mode::Welcome;
        }
        if let Mode::Matrix(m) = &mut self.mode {
            for d in &mut m.drops {
                d.0 += d.1;
            }
        }
    }

    fn accept_preview(&mut self, o: Outcome) {
        let conv = ansi_text;
        let new_warnings: Vec<String> = o
            .warnings
            .iter()
            .filter(|w| self.preview.as_ref().is_none_or(|p| !p.warnings.contains(w)))
            .cloned()
            .collect();
        for w in new_warnings {
            self.log(Tag::Warn, w);
        }
        if let Some(e) = &o.error
            && self.preview.as_ref().is_none_or(|p| p.error.as_ref() != Some(e)) {
                self.log(Tag::Err, format!("preview: {e}"));
            }
        self.preview = Some(PreviewState {
            success: RenderedPrompt { left: conv(&o.success.left), right: conv(&o.success.right) },
            failure: RenderedPrompt { left: conv(&o.failure.left), right: conv(&o.failure.right) },
            elapsed: o.elapsed,
            error: o.error,
            warnings: o.warnings,
            received: Instant::now(),
        });
    }

    // ───────────────────────────── input ─────────────────────────────

    pub fn on_key(&mut self, k: KeyEvent) {
        if k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL) {
            self.request_quit(false);
            return;
        }
        let mode = std::mem::replace(&mut self.mode, Mode::Normal);
        self.mode = match mode {
            Mode::Boot => Mode::Normal,
            Mode::Normal => {
                self.key_normal(k);
                return;
            }
            Mode::Command => {
                self.mode = Mode::Command;
                self.key_command(k);
                return;
            }
            Mode::Edit(e) => self.key_edit(e, k),
            Mode::Picker(p) => self.key_picker(p, k),
            Mode::Glyphs(g) => self.key_glyphs(g, k),
            Mode::Globals(row) => self.key_globals(row, k),
            Mode::Help(scroll) => match k.code {
                KeyCode::Char('j') | KeyCode::Down => Mode::Help(scroll.saturating_add(1)),
                KeyCode::Char('k') | KeyCode::Up => Mode::Help(scroll.saturating_sub(1)),
                _ => Mode::Normal,
            },
            Mode::TomlView(scroll) => match k.code {
                KeyCode::Char('j') | KeyCode::Down => Mode::TomlView(scroll.saturating_add(1)),
                KeyCode::Char('k') | KeyCode::Up => Mode::TomlView(scroll.saturating_sub(1)),
                KeyCode::PageDown | KeyCode::Char(' ') => Mode::TomlView(scroll.saturating_add(15)),
                KeyCode::PageUp => Mode::TomlView(scroll.saturating_sub(15)),
                KeyCode::Char('g') => Mode::TomlView(0),
                KeyCode::Char('y') => {
                    self.yank();
                    Mode::TomlView(scroll)
                }
                _ => Mode::Normal,
            },
            Mode::Matrix(_) => {
                self.log(Tag::Sys, "wake up, neo… back to work".into());
                Mode::Normal
            }
            Mode::Review(r) => self.key_review(*r, k),
            Mode::Backups(b) => self.key_backups(b, k),
            Mode::Gallery(g) => self.key_gallery(*g, k),
            Mode::Welcome => match k.code {
                KeyCode::Char('P') => {
                    self.open_gallery(GalleryKind::Presets);
                    std::mem::replace(&mut self.mode, Mode::Normal)
                }
                _ => Mode::Normal,
            },
            Mode::ConfirmQuit => match k.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.quit = true;
                    Mode::Normal
                }
                KeyCode::Char('w') | KeyCode::Char('W') => {
                    self.begin_install(false, true);
                    std::mem::replace(&mut self.mode, Mode::Normal)
                }
                _ => {
                    self.log(Tag::Sys, "quit aborted".into());
                    Mode::Normal
                }
            },
        };
    }

    fn request_quit(&mut self, force: bool) {
        if force || !self.dirty() {
            self.quit = true;
        } else {
            self.mode = Mode::ConfirmQuit;
        }
    }

    fn key_normal(&mut self, k: KeyEvent) {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let n = self.model.segments.len();
        match k.code {
            KeyCode::Char(':') => {
                self.cmdline = TextEdit::new("", EditTarget::Literal, false);
                self.history_pos = None;
                self.completion = None;
                self.mode = Mode::Command;
            }
            KeyCode::Char('s') if ctrl => self.begin_install(false, false),
            KeyCode::Char('r') if ctrl => self.redo(),
            KeyCode::Char('u') => self.undo(),
            KeyCode::Char('q') => self.request_quit(false),
            KeyCode::Char('?') | KeyCode::F(1) => self.mode = Mode::Help(0),
            KeyCode::Char('g') => self.mode = Mode::Globals(0),
            KeyCode::Char('t') => self.mode = Mode::TomlView(0),
            KeyCode::Char('y') => self.yank(),
            KeyCode::Char('s') => self.cycle_scenario(),
            KeyCode::Char('S') => self.cycle_shape(1),
            KeyCode::Char('L') => self.set_layout(self.model.globals.layout.cycle(1)),
            KeyCode::Char('C') => self.set_cap(self.model.globals.cap.cycle(1)),
            KeyCode::Char('P') => self.open_gallery(GalleryKind::Presets),
            KeyCode::Char('T') => self.open_gallery(GalleryKind::Themes),
            KeyCode::Char('O') => self.open_gallery(GalleryKind::Saved),
            KeyCode::Char('F') => self.set_glyphs(self.model.globals.glyphs.cycle(1)),
            KeyCode::Char(']') => self.cycle_palette(1),
            KeyCode::Char('[') => self.cycle_palette(-1),
            KeyCode::Char('M') => self.start_matrix(),
            KeyCode::Char('B') => self.open_backups(),
            KeyCode::Char('a') => {
                self.cmdline = TextEdit::new("add ", EditTarget::Literal, false);
                self.mode = Mode::Command;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.focus = if self.focus == Focus::Modules { Focus::Inspector } else { Focus::Modules };
                self.field = self.field.min(self.fields().len().saturating_sub(1));
            }
            _ if self.focus == Focus::Modules => match k.code {
                KeyCode::Char('j') | KeyCode::Down => self.sel = (self.sel + 1).min(n.saturating_sub(1)),
                KeyCode::Char('k') | KeyCode::Up => self.sel = self.sel.saturating_sub(1),
                KeyCode::Home => self.sel = 0,
                KeyCode::End | KeyCode::Char('G') => self.sel = n.saturating_sub(1),
                KeyCode::Char(' ') => self.toggle_selected(),
                KeyCode::Char('J') => self.move_selected(1),
                KeyCode::Char('K') => self.move_selected(-1),
                KeyCode::Char('R') => self.toggle_side(),
                KeyCode::Char('D') | KeyCode::Delete => self.remove_selected(),
                KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                    self.focus = Focus::Inspector;
                    self.field = 0;
                }
                _ => {}
            },
            _ => match k.code {
                KeyCode::Char('j') | KeyCode::Down => self.field = (self.field + 1).min(self.fields().len().saturating_sub(1)),
                KeyCode::Char('k') | KeyCode::Up => self.field = self.field.saturating_sub(1),
                KeyCode::Esc | KeyCode::Char('h') | KeyCode::Left => self.focus = Focus::Modules,
                KeyCode::Enter | KeyCode::Char(' ') => self.activate_field(),
                KeyCode::Char('p')
                    if self.fields().get(self.field) == Some(&Field::Symbol) => {
                        self.mode = Mode::Glyphs(GlyphPicker { idx: 0 });
                    }
                KeyCode::Backspace | KeyCode::Char('x') => self.reset_field(),
                KeyCode::Char('J') => self.move_selected(1),
                KeyCode::Char('K') => self.move_selected(-1),
                _ => {}
            },
        }
    }

    fn activate_field(&mut self) {
        let Some(field) = self.fields().get(self.field).copied() else { return };
        let Some(seg) = self.selected().cloned() else { return };
        match field {
            Field::Enabled => self.toggle_selected(),
            Field::Side => self.toggle_side(),
            Field::Bold | Field::Italic => {
                let before = self.model.clone();
                if let Some(s) = self.model.segments.get_mut(self.sel) {
                    if field == Field::Bold {
                        s.bold = !s.bold;
                    } else {
                        s.italic = !s.italic;
                    }
                }
                let msg = self.style_msg(&seg.name);
                self.commit(before, Tag::Write, msg);
            }
            Field::Symbol => {
                self.mode = Mode::Edit(TextEdit::new(seg.effective_symbol(), EditTarget::Symbol, false))
            }
            Field::Format => {
                let f = generate::module_format(&seg, self.model.globals.shape);
                self.mode = Mode::Edit(TextEdit::new(&f, EditTarget::Format, false));
            }
            Field::Literal => {
                self.mode = Mode::Edit(TextEdit::new(seg.literal.as_deref().unwrap_or(""), EditTarget::Literal, false))
            }
            Field::Opt(o) => {
                if o.kind == OptKind::Bool {
                    let cur = match seg.options.get(o.key) {
                        Some(OptValue::Bool(b)) => *b,
                        _ => o.default == "true",
                    };
                    let key = o.key;
                    self.edit_selected(|s, _| {
                        s.options.insert(key.to_string(), OptValue::Bool(!cur));
                        Some(format!("{}.{key} = {}", s.name, !cur))
                    });
                } else {
                    let cur = seg.options.get(o.key).map(|v| match v {
                        OptValue::Str(s) => s.clone(),
                        other => other.display(),
                    });
                    let init = cur.unwrap_or_else(|| o.default.to_string());
                    self.mode = Mode::Edit(TextEdit::new(&init, EditTarget::Opt(o), false));
                }
            }
            Field::PromptChar => {
                self.mode = Mode::Edit(TextEdit::new(&self.model.globals.prompt_char, EditTarget::PromptChar, false))
            }
            Field::Fg => self.open_picker(ColorTarget::Fg, false),
            Field::Bg => self.open_picker(ColorTarget::Bg, false),
            Field::Success => self.open_picker(ColorTarget::Success, false),
            Field::Error => self.open_picker(ColorTarget::Error, false),
        }
    }

    fn reset_field(&mut self) {
        let Some(field) = self.fields().get(self.field).copied() else { return };
        self.edit_selected(|s, _| match field {
            Field::Symbol if s.symbol.is_some() => {
                s.symbol = None;
                Some(format!("{}.symbol reset to default", s.name))
            }
            Field::Format if s.format_override.is_some() => {
                s.format_override = None;
                Some(format!("{}.format → generated", s.name))
            }
            Field::Opt(o) if s.options.contains_key(o.key) => {
                s.options.remove(o.key);
                Some(format!("{}.{} reset to default", s.name, o.key))
            }
            _ => None,
        });
    }

    fn key_edit(&mut self, mut e: TextEdit, k: KeyEvent) -> Mode {
        let back = |from_globals: bool| if from_globals { Mode::Globals(3) } else { Mode::Normal };
        match k.code {
            KeyCode::Esc => back(e.from_globals),
            KeyCode::Enter => {
                self.apply_edit(&e);
                back(e.from_globals)
            }
            KeyCode::Tab if matches!(e.target, EditTarget::Symbol) => Mode::Glyphs(GlyphPicker { idx: 0 }),
            _ => {
                e.key(&k);
                Mode::Edit(e)
            }
        }
    }

    fn apply_edit(&mut self, e: &TextEdit) {
        let text = e.text();
        match &e.target {
            EditTarget::PromptChar => {
                if text.is_empty() {
                    self.log(Tag::Err, "prompt char can't be empty".into());
                    return;
                }
                let before = self.model.clone();
                self.model.globals.prompt_char = text.clone();
                self.commit(before, Tag::Write, format!("character.success_symbol = \"[{text}](bold {})\"", self.model.globals.success));
            }
            EditTarget::Symbol => self.edit_selected(|s, _| {
                s.symbol = Some(text.clone());
                Some(format!("{}.symbol = {}", s.name, quote(&text)))
            }),
            EditTarget::Literal => self.edit_selected(|s, _| {
                s.literal = Some(text.clone());
                Some(format!("format literal = {}", quote(&text)))
            }),
            EditTarget::Format => {
                let shape = self.model.globals.shape;
                self.edit_selected(|s, _| {
                    let generated = {
                        let mut t = s.clone();
                        t.format_override = None;
                        generate::module_format(&t, shape)
                    };
                    s.format_override = if text == generated { None } else { Some(text.clone()) };
                    Some(format!("{}.format = '{text}'", s.name))
                });
            }
            EditTarget::Opt(o) => {
                let value = match o.kind {
                    OptKind::Int => match text.trim().parse::<i64>() {
                        Ok(i) => OptValue::Int(i),
                        Err(_) => {
                            self.log(Tag::Err, format!("{}: {} is not an integer", o.key, quote(&text)));
                            return;
                        }
                    },
                    OptKind::Bool => OptValue::Bool(matches!(text.trim(), "true" | "yes" | "1" | "on")),
                    OptKind::Str => OptValue::Str(text.clone()),
                };
                let key = o.key;
                self.edit_selected(|s, _| {
                    let msg = format!("{}.{key} = {}", s.name, value.display());
                    s.options.insert(key.to_string(), value);
                    Some(msg)
                });
            }
        }
    }

    fn open_picker(&mut self, target: ColorTarget, from_globals: bool) {
        let p = self.palette();
        let mut rows: Vec<(String, Vec<(String, Rgb)>)> = Vec::new();
        rows.push(("dracula".into(), dracula::SWATCHES.iter().map(|(n, c)| (n.to_string(), *c)).collect()));
        let mut pal: Vec<(String, Rgb)> = p.accents.iter().enumerate().map(|(i, c)| (format!("accent {}", i + 1), *c)).collect();
        pal.extend([("dark".into(), p.dark), ("light".into(), p.light), ("success".into(), p.success), ("error".into(), p.error)]);
        rows.push((p.name.into(), pal));
        for other in PALETTES.iter().filter(|o| o.name != p.name) {
            rows.push((other.name.into(), other.accents.iter().map(|c| (c.hex(), *c)).collect()));
        }
        rows.push((
            "greys".into(),
            (0..10).map(|i| {
                let v = (i as f32 / 9.0 * 255.0) as u8;
                (format!("grey {i}"), Rgb(v, v, v))
            }).collect(),
        ));
        let current = self.current_color(target);
        let (mut row, mut col) = (0, 0);
        'find: for (r, (_, sw)) in rows.iter().enumerate() {
            for (c, (_, rgb)) in sw.iter().enumerate() {
                if Some(*rgb) == current {
                    row = r;
                    col = c;
                    break 'find;
                }
            }
        }
        self.mode = Mode::Picker(ColorPicker { target, original: self.model.clone(), rows, row, col, hex: None, from_globals });
    }

    fn current_color(&self, t: ColorTarget) -> Option<Rgb> {
        match t {
            ColorTarget::Fg => self.selected().map(|s| s.fg),
            ColorTarget::Bg => self.selected().map(|s| s.bg),
            ColorTarget::Success => Some(self.model.globals.success),
            ColorTarget::Error => Some(self.model.globals.error),
        }
    }

    fn set_color(&mut self, t: ColorTarget, c: Rgb) {
        match t {
            ColorTarget::Fg => {
                if let Some(s) = self.model.segments.get_mut(self.sel) {
                    s.fg = c;
                }
            }
            ColorTarget::Bg => {
                if let Some(s) = self.model.segments.get_mut(self.sel) {
                    s.bg = c;
                }
            }
            ColorTarget::Success => self.model.globals.success = c,
            ColorTarget::Error => self.model.globals.error = c,
        }
    }

    fn key_picker(&mut self, mut p: ColorPicker, k: KeyEvent) -> Mode {
        let back = if p.from_globals { Mode::Globals(if p.target == ColorTarget::Success { 4 } else { 5 }) } else { Mode::Normal };
        if let Some(hex) = &mut p.hex {
            match k.code {
                KeyCode::Esc => p.hex = None,
                KeyCode::Backspace => {
                    hex.pop();
                    if hex.is_empty() {
                        p.hex = None;
                    }
                }
                KeyCode::Char(c) if c.is_ascii_hexdigit() && hex.len() < 7 => hex.push(c.to_ascii_lowercase()),
                KeyCode::Enter => return self.finish_picker(p, back),
                _ => {}
            }
            if let Some(c) = p.selected() {
                self.set_color(p.target, c);
                self.request_preview();
            }
            return Mode::Picker(p);
        }
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.model = p.original.clone();
                self.request_preview();
                return back;
            }
            KeyCode::Enter | KeyCode::Char(' ') => return self.finish_picker(p, back),
            KeyCode::Char('#') => p.hex = Some("#".into()),
            KeyCode::Left | KeyCode::Char('h') => p.col = p.col.saturating_sub(1),
            KeyCode::Right | KeyCode::Char('l') => p.col += 1,
            KeyCode::Up | KeyCode::Char('k') => p.row = p.row.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => p.row = (p.row + 1).min(p.rows.len().saturating_sub(1)),
            _ => {}
        }
        let len = p.rows.get(p.row).map(|r| r.1.len()).unwrap_or(1);
        p.col = p.col.min(len.saturating_sub(1));
        // Live preview while browsing; Esc restores.
        if let Some(c) = p.selected() {
            self.set_color(p.target, c);
            self.request_preview();
        }
        Mode::Picker(p)
    }

    fn finish_picker(&mut self, p: ColorPicker, back: Mode) -> Mode {
        let Some(c) = p.selected() else {
            self.log(Tag::Err, "not a valid hex colour".into());
            return Mode::Picker(p);
        };
        self.model = p.original.clone();
        let before = self.model.clone();
        self.set_color(p.target, c);
        let msg = match p.target {
            ColorTarget::Fg | ColorTarget::Bg => {
                let name = self.selected().map(|s| s.name.clone()).unwrap_or_default();
                self.style_msg(&name)
            }
            ColorTarget::Success => format!("character.success_symbol = \"[{}](bold {c})\"", self.model.globals.prompt_char),
            ColorTarget::Error => format!("character.error_symbol = \"[{}](bold {c})\"", self.model.globals.prompt_char),
        };
        self.commit(before, Tag::Write, msg);
        back
    }

    fn key_glyphs(&mut self, mut g: GlyphPicker, k: KeyEvent) -> Mode {
        let n = catalog::GLYPHS.len();
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => return Mode::Normal,
            KeyCode::Left | KeyCode::Char('h') => g.idx = g.idx.saturating_sub(1),
            KeyCode::Right | KeyCode::Char('l') => g.idx = (g.idx + 1).min(n - 1),
            KeyCode::Up | KeyCode::Char('k') => g.idx = g.idx.saturating_sub(GLYPH_COLS),
            KeyCode::Down | KeyCode::Char('j') => g.idx = (g.idx + GLYPH_COLS).min(n - 1),
            KeyCode::Enter | KeyCode::Char(' ') => {
                let glyph = catalog::GLYPHS[g.idx].0;
                let character = self.selected().is_some_and(|s| s.is_character());
                if character {
                    let before = self.model.clone();
                    self.model.globals.prompt_char = glyph.to_string();
                    self.commit(before, Tag::Write, format!("character glyph = {}", quote(glyph)));
                } else {
                    self.edit_selected(|s, _| {
                        // Keep the catalog's spacing convention (most symbols carry a trailing space).
                        let spaced = if s.def().default_symbol.ends_with(' ') { format!("{glyph} ") } else { glyph.to_string() };
                        let msg = format!("{}.symbol = {}", s.name, quote(&spaced));
                        s.symbol = Some(spaced);
                        Some(msg)
                    });
                }
                return Mode::Normal;
            }
            _ => {}
        }
        Mode::Glyphs(g)
    }

    fn key_globals(&mut self, row: usize, k: KeyEvent) -> Mode {
        let r = GlobalRow::ALL[row.min(GlobalRow::ALL.len() - 1)];
        let step = match k.code {
            KeyCode::Left | KeyCode::Char('h') => -1,
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter | KeyCode::Char(' ') => 1,
            KeyCode::Up | KeyCode::Char('k') => return Mode::Globals(row.saturating_sub(1)),
            KeyCode::Down | KeyCode::Char('j') => return Mode::Globals((row + 1).min(GlobalRow::ALL.len() - 1)),
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('g') => return Mode::Normal,
            _ => return Mode::Globals(row),
        };
        self.mode = Mode::Globals(row);
        match r {
            GlobalRow::Layout => self.set_layout(self.model.globals.layout.cycle(step)),
            GlobalRow::Shape => self.cycle_shape(step),
            GlobalRow::Cap => self.set_cap(self.model.globals.cap.cycle(step)),
            GlobalRow::Palette => self.cycle_palette(step),
            GlobalRow::Glyphs => self.set_glyphs(self.model.globals.glyphs.cycle(step)),
            GlobalRow::AddNewline => {
                let before = self.model.clone();
                self.model.globals.add_newline = !self.model.globals.add_newline;
                self.commit(before, Tag::Write, format!("add_newline = {}", self.model.globals.add_newline));
            }
            GlobalRow::PromptChar => {
                return Mode::Edit(TextEdit::new(&self.model.globals.prompt_char, EditTarget::PromptChar, true));
            }
            GlobalRow::Success => {
                self.open_picker(ColorTarget::Success, true);
                return std::mem::replace(&mut self.mode, Mode::Normal);
            }
            GlobalRow::Error => {
                self.open_picker(ColorTarget::Error, true);
                return std::mem::replace(&mut self.mode, Mode::Normal);
            }
        }
        std::mem::replace(&mut self.mode, Mode::Normal)
    }

    fn key_command(&mut self, k: KeyEvent) {
        match k.code {
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Enter => {
                let line = self.cmdline.text();
                self.mode = Mode::Normal;
                if !line.trim().is_empty() {
                    self.cmd_history.push(line.clone());
                    self.run_command(&line);
                }
            }
            KeyCode::Backspace if self.cmdline.buf.is_empty() => self.mode = Mode::Normal,
            KeyCode::Up | KeyCode::Down => {
                if self.cmd_history.is_empty() {
                    return;
                }
                let last = self.cmd_history.len() - 1;
                let pos = match (self.history_pos, k.code) {
                    (None, KeyCode::Up) => Some(last),
                    (Some(p), KeyCode::Up) => Some(p.saturating_sub(1)),
                    (Some(p), KeyCode::Down) if p < last => Some(p + 1),
                    _ => None,
                };
                self.history_pos = pos;
                let text = pos.and_then(|p| self.cmd_history.get(p)).cloned().unwrap_or_default();
                self.cmdline = TextEdit::new(&text, EditTarget::Literal, false);
            }
            KeyCode::Tab => self.complete(),
            _ => {
                self.completion = None;
                self.cmdline.key(&k);
            }
        }
    }

    pub fn completions_for(&self, line: &str) -> Vec<String> {
        let (cmd, arg) = match line.split_once(' ') {
            Some((c, a)) => (c, Some(a)),
            None => (line, None),
        };
        match arg {
            None => COMMANDS.iter().map(|(c, _)| *c).filter(|c| c.starts_with(cmd)).map(|c| c.to_string()).collect(),
            Some(a) => {
                let opts: Vec<String> = match cmd {
                    "preset" | "palette" => PALETTES.iter().map(|p| p.name.to_string()).collect(),
                    "shape" => Shape::ALL.iter().map(|s| s.label().replace(' ', "-")).collect(),
                    "layout" => Layout::ALL.iter().map(|s| s.label().replace(' ', "-")).collect(),
                    "cap" => Cap::ALL.iter().map(|s| s.label().to_string()).collect(),
                    "scenario" => Scenario::ALL.iter().map(|s| s.label().replace(' ', "-")).collect(),
                    "colors" => vec!["true".into(), "256".into()],
                    "add" => catalog::CATALOG
                        .iter()
                        .filter(|d| self.model.find(d.name).is_none())
                        .map(|d| d.name.to_string())
                        .collect(),
                    "set" => self
                        .model
                        .segments
                        .iter()
                        .filter(|s| !s.raw)
                        .flat_map(|s| {
                            ["fg", "bg", "symbol", "bold", "italic", "side", "enabled", "format"]
                                .iter()
                                .map(|k| format!("{}.{k}", s.name))
                                .chain(s.def().options.iter().map(|o| format!("{}.{}", s.name, o.key)))
                                .collect::<Vec<_>>()
                        })
                        .collect(),
                    _ => vec![],
                };
                opts.into_iter().filter(|o| o.starts_with(a)).map(|o| format!("{cmd} {o}")).collect()
            }
        }
    }

    fn complete(&mut self) {
        let (matches, idx) = match self.completion.take() {
            Some((m, i)) if !m.is_empty() => {
                let i = (i + 1) % m.len();
                (m, i)
            }
            _ => (self.completions_for(&self.cmdline.text()), 0),
        };
        if let Some(m) = matches.get(idx) {
            let needs_space = !m.contains(' ') && COMMANDS.iter().any(|(c, h)| c == m && h.contains('<'));
            let text = if needs_space { format!("{m} ") } else { m.clone() };
            self.cmdline = TextEdit::new(&text, EditTarget::Literal, false);
        }
        self.completion = Some((matches, idx));
    }

    // ───────────────────────────── actions ─────────────────────────────

    fn toggle_selected(&mut self) {
        self.edit_selected(|s, _| {
            s.enabled = !s.enabled;
            Some(format!("{} {}", s.name, if s.enabled { "ENABLED" } else { "disabled" }))
        });
    }

    fn toggle_side(&mut self) {
        let layout = self.model.globals.layout;
        self.edit_selected(|s, _| {
            if s.is_character() {
                return None;
            }
            s.side = if s.side == Side::Left { Side::Right } else { Side::Left };
            let note = if layout != Layout::RightPrompt { " (only used with the right-prompt layout)" } else { "" };
            Some(format!("{} → {} side{note}", s.name, if s.side == Side::Left { "left" } else { "right" }))
        });
    }

    fn move_selected(&mut self, delta: i32) {
        let before = self.model.clone();
        if self.selected().is_some_and(|s| s.is_character()) {
            self.log(Tag::Warn, "character is pinned to the end of the prompt".into());
            return;
        }
        let name = self.selected().map(|s| s.name.clone()).unwrap_or_default();
        let new = self.model.move_segment(self.sel, delta);
        self.sel = new;
        let (fmt, _) = generate::top_formats(&self.model);
        self.commit(before, Tag::Write, format!("{name} → #{:02} · format = \"{fmt}\"", new + 1));
    }

    fn remove_selected(&mut self) {
        let Some(seg) = self.selected().cloned() else { return };
        if seg.is_character() {
            self.log(Tag::Warn, "can't remove character (toggle it off instead)".into());
            return;
        }
        let before = self.model.clone();
        self.model.segments.remove(self.sel);
        self.sel = self.sel.min(self.model.segments.len().saturating_sub(1));
        self.commit(before, Tag::Write, format!("removed {} (u to undo)", seg.name));
    }

    fn add_module(&mut self, name: &str) {
        if name.is_empty() {
            self.log(Tag::Err, "usage: :add <module>".into());
            return;
        }
        if let Some(i) = self.model.segments.iter().position(|s| s.name == name) {
            self.sel = i;
            if !self.model.segments[i].enabled {
                self.toggle_selected();
            } else {
                self.log(Tag::Warn, format!("{name} is already in the prompt"));
            }
            return;
        }
        let before = self.model.clone();
        let p = self.palette();
        let mut seg = Segment::new(name);
        seg.enabled = true;
        let used = self.model.styled().count();
        let accent = p.accents[used % p.accents.len()];
        if self.model.globals.shape == Shape::Plain {
            seg.fg = accent;
            seg.bg = p.dark;
        } else {
            seg.bg = accent;
            seg.fg = p.text_on(accent);
        }
        let at = self.model.segments.iter().position(|s| s.is_character()).unwrap_or(self.model.segments.len());
        self.model.segments.insert(at, seg);
        self.sel = at;
        let generic = if catalog::lookup(name).is_none() { " (not in catalog: generic body)" } else { "" };
        self.commit(before, Tag::Write, format!("added {name}{generic}"));
    }

    fn undo(&mut self) {
        match self.undo.pop() {
            Some(prev) => {
                self.redo.push(std::mem::replace(&mut self.model, prev));
                self.sel = self.sel.min(self.model.segments.len().saturating_sub(1));
                self.log(Tag::Cmd, format!("undo ({} left)", self.undo.len()));
                self.request_preview();
            }
            None => self.log(Tag::Warn, "already at oldest change".into()),
        }
    }

    fn redo(&mut self) {
        match self.redo.pop() {
            Some(next) => {
                self.undo.push(std::mem::replace(&mut self.model, next));
                self.sel = self.sel.min(self.model.segments.len().saturating_sub(1));
                self.log(Tag::Cmd, format!("redo ({} left)", self.redo.len()));
                self.request_preview();
            }
            None => self.log(Tag::Warn, "already at newest change".into()),
        }
    }

    /// Next shape the current glyph set can draw.
    fn cycle_shape(&mut self, step: i32) {
        let mut shape = self.model.globals.shape.cycle(step);
        if self.model.globals.glyphs != Glyphs::Nerd {
            while shape.needs_nerd_font() {
                shape = shape.cycle(step);
            }
        }
        self.set_shape(shape);
    }

    fn set_shape(&mut self, shape: Shape) {
        if shape.needs_nerd_font() && self.model.globals.glyphs != Glyphs::Nerd {
            self.log(Tag::Err, format!("{} needs Nerd Font glyphs — press F to switch glyphs to nerd font", shape.label()));
            return;
        }
        let before = self.model.clone();
        let overrides = self.model.segments.iter().filter(|s| s.format_override.is_some()).count();
        self.model.set_shape(shape);
        // Picking a shape means "generate my module formats".
        for s in &mut self.model.segments {
            s.format_override = None;
        }
        let extra = if overrides > 0 { format!(" · regenerated {overrides} hand-written module formats") } else { String::new() };
        self.commit(before, Tag::Write, format!("shape = {}{extra}", shape.label()));
    }

    fn set_glyphs(&mut self, g: Glyphs) {
        let before = self.model.clone();
        self.model.set_glyphs(g);
        let shape_note = if before.globals.shape != self.model.globals.shape {
            format!(" · shape → {} (no Nerd Font shapes)", self.model.globals.shape.label())
        } else {
            String::new()
        };
        self.commit(before, Tag::Write, format!("glyphs = {}: {}{shape_note}", g.label(), g.blurb()));
    }

    fn set_layout(&mut self, layout: Layout) {
        let before = self.model.clone();
        self.model.globals.layout = layout;
        let (_, right) = generate::top_formats(&self.model);
        let msg = match (layout, right) {
            (Layout::RightPrompt, None) => "layout = right prompt (press R on a module to send it right)".to_string(),
            (Layout::RightPrompt, Some(r)) => format!("right_format = \"{r}\""),
            (l, _) => format!("layout = {}", l.label()),
        };
        self.commit(before, Tag::Write, msg);
    }

    fn set_cap(&mut self, cap: Cap) {
        let before = self.model.clone();
        self.model.globals.cap = cap;
        let note = if matches!(self.model.globals.shape, Shape::Powerline | Shape::Flat) { "" } else { " (caps apply to powerline/flat shapes)" };
        self.commit(before, Tag::Write, format!("left cap = {}{note}", cap.label()));
    }

    fn cycle_palette(&mut self, step: i32) {
        // From a hand-rolled ("custom") palette, start the cycle at the first preset.
        let next = match PALETTES.iter().position(|p| p.name == self.model.globals.palette) {
            Some(i) => &PALETTES[(i as i32 + step).rem_euclid(PALETTES.len() as i32) as usize],
            None => &PALETTES[0],
        };
        self.apply_theme(next);
    }

    fn apply_theme(&mut self, p: &'static palette::Palette) {
        let before = self.model.clone();
        self.model.apply_palette(p);
        self.commit(before, Tag::Write, format!("theme {} applied to {} segments", p.name, self.model.styled().count()));
    }

    fn apply_design(&mut self, p: &'static presets::Preset) {
        let before = self.model.clone();
        self.model.apply_design(p);
        self.sel = 0;
        self.commit(before, Tag::Write, format!("preset {}: {}", p.name, p.blurb));
    }

    fn library_dir(&self) -> PathBuf {
        library::dir(&self.config_path)
    }

    fn save_named(&mut self, name: &str, overwrite: bool) {
        if name.is_empty() {
            self.log(Tag::Err, "usage: :save <name>   (then O or :open to browse saved configs)".into());
            return;
        }
        let toml = self.generated_toml();
        if self.validated(&toml).is_none() {
            return;
        }
        match library::save(&self.library_dir(), name, &toml, overwrite) {
            Ok(p) => {
                self.log(Tag::Ok, format!("saved as {} — O to browse saved configs", home_relative(&p)));
                self.flash = Some(("SAVED".into(), Instant::now()));
            }
            Err(e) => self.log(Tag::Err, e),
        }
    }

    pub fn open_gallery(&mut self, kind: GalleryKind) {
        let items: Vec<GalleryItem> = match kind {
            GalleryKind::Presets => PRESETS
                .iter()
                .map(|p| GalleryItem {
                    name: p.name.into(),
                    blurb: format!("{} · {} · {}", p.blurb, p.shape.label(), p.glyphs.label()),
                    swatches: palette::find(p.palette).map(|pal| pal.accents.iter().take(6).copied().collect()).unwrap_or_default(),
                    path: None,
                })
                .collect(),
            GalleryKind::Themes => PALETTES
                .iter()
                .map(|p| GalleryItem {
                    name: p.name.into(),
                    blurb: String::new(),
                    swatches: p.accents.to_vec(),
                    path: None,
                })
                .collect(),
            GalleryKind::Saved => library::list(&self.library_dir())
                .into_iter()
                .map(|e| GalleryItem { name: e.name, blurb: format!("saved {}", e.modified), swatches: vec![], path: Some(e.path) })
                .collect(),
        };
        if items.is_empty() {
            self.log(Tag::Warn, "no saved configs yet — :save <name> stores the current design".into());
            return;
        }
        let idx = match kind {
            GalleryKind::Themes => items.iter().position(|i| i.name == self.model.globals.palette).unwrap_or(0),
            _ => 0,
        };
        let mut g = Gallery { kind, items, idx, preview: None, preview_for: None, error: None, confirm_delete: false };
        self.render_gallery_preview(&mut g);
        self.mode = Mode::Gallery(Box::new(g));
    }

    /// The design the highlighted gallery item would produce, as TOML.
    fn gallery_candidate(&self, g: &Gallery) -> Result<String, String> {
        let item = g.items.get(g.idx).ok_or("nothing selected")?;
        let mut m = self.model.clone();
        match g.kind {
            GalleryKind::Presets => m.apply_design(presets::find(&item.name).ok_or("unknown preset")?),
            GalleryKind::Themes => m.apply_palette(palette::find(&item.name).ok_or("unknown theme")?),
            GalleryKind::Saved => {
                let path = item.path.as_ref().ok_or("no file")?;
                return std::fs::read_to_string(path).map_err(|e| e.to_string());
            }
        }
        Ok(generate::render(&m, &self.base_doc, Some(&self.base_model)))
    }

    fn render_gallery_preview(&mut self, g: &mut Gallery) {
        if g.preview_for == Some(g.idx) {
            return;
        }
        g.preview_for = Some(g.idx);
        g.preview = None;
        g.error = None;
        let Some(env) = self.env.clone() else { return };
        match self.gallery_candidate(g) {
            Ok(toml) => {
                let o = preview::render_sync(&env, &toml, self.scenario, self.preview_width);
                g.error = o.error.clone();
                g.preview = Some((rendered(&o.success), rendered(&o.failure)));
            }
            Err(e) => g.error = Some(e),
        }
    }

    fn key_gallery(&mut self, mut g: Gallery, k: KeyEvent) -> Mode {
        let n = g.items.len();
        let delete_armed = std::mem::take(&mut g.confirm_delete);
        match k.code {
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Tab => g.idx = (g.idx + 1) % n.max(1),
            KeyCode::Char('k') | KeyCode::Up | KeyCode::BackTab => g.idx = (g.idx + n.max(1) - 1) % n.max(1),
            KeyCode::Home | KeyCode::Char('g') => g.idx = 0,
            KeyCode::End | KeyCode::Char('G') => g.idx = n.saturating_sub(1),
            KeyCode::Enter | KeyCode::Char(' ') => {
                let Some(item) = g.items.get(g.idx).cloned() else { return Mode::Normal };
                match g.kind {
                    GalleryKind::Presets => {
                        if let Some(p) = presets::find(&item.name) {
                            self.apply_design(p);
                        }
                    }
                    GalleryKind::Themes => {
                        if let Some(p) = palette::find(&item.name) {
                            self.apply_theme(p);
                        }
                    }
                    GalleryKind::Saved => {
                        let Some(path) = item.path else { return Mode::Normal };
                        match load::load(&path) {
                            Ok(l) => self.replace_model(l.model, &format!("opened saved config {} (ctrl-s to install it)", item.name)),
                            Err(e) => self.log(Tag::Err, format!("can't open {}: {e}", item.name)),
                        }
                    }
                }
                return Mode::Normal;
            }
            KeyCode::Char('d') if g.kind == GalleryKind::Saved => {
                if delete_armed {
                    if let Some(path) = g.items.get(g.idx).and_then(|i| i.path.clone()) {
                        match std::fs::remove_file(&path) {
                            Ok(()) => self.log(Tag::Ok, format!("deleted saved config {}", home_relative(&path))),
                            Err(e) => self.log(Tag::Err, format!("delete failed: {e}")),
                        }
                    }
                    g.items.remove(g.idx);
                    if g.items.is_empty() {
                        return Mode::Normal;
                    }
                    g.idx = g.idx.min(g.items.len() - 1);
                    g.preview_for = None;
                } else {
                    g.confirm_delete = true;
                }
            }
            KeyCode::Esc | KeyCode::Char('q') => return Mode::Normal,
            _ => {}
        }
        self.render_gallery_preview(&mut g);
        Mode::Gallery(Box::new(g))
    }

    fn cycle_scenario(&mut self) {
        self.scenario = self.scenario.next();
        self.log(Tag::Preview, format!("scenario → {}", self.scenario.label()));
        self.request_preview();
    }

    fn start_matrix(&mut self) {
        let drops = (0..400).map(|_| (-(fastrand::f32() * 60.0), 0.2 + fastrand::f32() * 0.6, 4 + fastrand::u16(..20))).collect();
        self.mode = Mode::Matrix(MatrixState { drops, started: Instant::now() });
    }

    fn randomize(&mut self) {
        let before = self.model.clone();
        let plain = self.model.globals.shape == Shape::Plain;
        let p = self.palette();
        let base = fastrand::f32() * 360.0;
        let golden = 137.508;
        let mut i = 0.0;
        for s in self.model.segments.iter_mut().filter(|s| !s.raw && !s.is_character()) {
            let c = hsl((base + i * golden) % 360.0, 0.55 + fastrand::f32() * 0.35, 0.55 + fastrand::f32() * 0.2);
            i += 1.0;
            if plain {
                s.fg = c;
            } else {
                s.bg = c;
                s.fg = p.text_on(c);
            }
        }
        self.commit(before, Tag::Write, "randomised segment colours (golden-angle hues)".into());
    }

    fn yank(&mut self) {
        let toml = self.generated_toml();
        let res = std::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut c| {
                use std::io::Write;
                if let Some(stdin) = c.stdin.as_mut() {
                    stdin.write_all(toml.as_bytes())?;
                }
                c.wait()
            });
        match res {
            Ok(st) if st.success() => self.log(Tag::Ok, format!("yanked {} bytes of TOML to the clipboard", toml.len())),
            Ok(st) => self.log(Tag::Err, format!("pbcopy exited with {st}")),
            Err(e) => self.log(Tag::Err, format!("pbcopy: {e}")),
        }
    }

    /// Validates the generated TOML, logging warnings. `None` means "don't write".
    fn validated(&mut self, toml: &str) -> Option<()> {
        self.log(Tag::Sys, "validating: toml parse + format check…".into());
        match preview::validate(toml) {
            Ok(warnings) => {
                for w in warnings {
                    self.log(Tag::Warn, w);
                }
                Some(())
            }
            Err(e) => {
                self.log(Tag::Err, format!("refusing to save: {e}"));
                None
            }
        }
    }

    /// Writes a validated copy somewhere other than the live config (no review needed).
    fn export(&mut self, target: &Path) -> bool {
        let toml = self.generated_toml();
        if self.validated(&toml).is_none() {
            return false;
        }
        self.write_with_backup(target, &toml)
    }

    fn write_with_backup(&mut self, target: &Path, toml: &str) -> bool {
        match save::backup(target) {
            Ok(Some(b)) => self.log(Tag::Backup, format!("previous setup backed up → {}", home_relative(&b))),
            Ok(None) => {}
            Err(e) => {
                self.log(Tag::Err, format!("backup failed, not saving: {e}"));
                return false;
            }
        }
        if let Err(e) = save::write_atomic(target, toml) {
            self.log(Tag::Err, format!("write failed: {e}"));
            return false;
        }
        self.log(Tag::Ok, format!("wrote {} bytes → {}", toml.len(), home_relative(target)));
        true
    }

    /// First half of saving to the live config: validate, render the current and new
    /// prompts, diff them, and open the review screen. Nothing is written yet.
    pub fn begin_install(&mut self, force: bool, quit_after: bool) {
        if !force {
            let on_disk = std::fs::read_to_string(&self.config_path).ok();
            if on_disk != self.disk_text {
                self.log(Tag::Err, "config changed on disk since it was loaded — :w! to overwrite, :reload! to discard".into());
                return;
            }
        }
        let toml = self.generated_toml();
        self.open_review(InstallKind::Generated, toml, quit_after);
    }

    fn open_review(&mut self, kind: InstallKind, toml: String, quit_after: bool) {
        if self.validated(&toml).is_none() {
            return;
        }
        let Some(env) = self.env.clone() else { return };
        // Before the first save, "current" is the starship config being imported.
        let current = std::fs::read_to_string(&self.config_path).ok().or_else(|| self.imported.clone());
        let (scenario, width) = (self.scenario, self.preview_width);
        let (before, after) = std::thread::scope(|sc| {
            let b = current.as_deref().map(|c| sc.spawn(|| preview::render_sync(&env, c, scenario, width)));
            let a = preview::render_sync(&env, &toml, scenario, width);
            (b.and_then(|h| h.join().ok()), a)
        });
        let diff = diff::lines(current.as_deref().unwrap_or(""), &toml);
        let (added, removed) = diff::stats(&diff);
        self.log(Tag::Sys, format!("review: +{added} −{removed} lines · y to install, esc to cancel"));
        self.mode = Mode::Review(Box::new(InstallReview {
            kind,
            before: before.map(|o| (rendered(&o.success), rendered(&o.failure))),
            after: (rendered(&after.success), rendered(&after.failure)),
            warnings: after.warnings,
            toml,
            diff,
            quit_after,
            scroll: 0,
        }));
    }

    fn key_review(&mut self, mut r: InstallReview, k: KeyEvent) -> Mode {
        match k.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                self.confirm_install(&r);
                Mode::Normal
            }
            KeyCode::Char('j') | KeyCode::Down => {
                r.scroll = r.scroll.saturating_add(1);
                Mode::Review(Box::new(r))
            }
            KeyCode::Char('k') | KeyCode::Up => {
                r.scroll = r.scroll.saturating_sub(1);
                Mode::Review(Box::new(r))
            }
            KeyCode::PageDown | KeyCode::Char(' ') => {
                r.scroll = r.scroll.saturating_add(10);
                Mode::Review(Box::new(r))
            }
            KeyCode::PageUp => {
                r.scroll = r.scroll.saturating_sub(10);
                Mode::Review(Box::new(r))
            }
            _ => {
                self.log(Tag::Sys, "install cancelled — nothing written".into());
                Mode::Normal
            }
        }
    }

    fn confirm_install(&mut self, r: &InstallReview) {
        let target = self.config_path.clone();
        if !self.write_with_backup(&target, &r.toml) {
            return;
        }
        match &r.kind {
            InstallKind::Generated => {
                match r.toml.parse::<DocumentMut>() {
                    Ok(doc) => self.base_doc = doc,
                    Err(e) => self.log(Tag::Warn, format!("re-parse after save failed: {e}")),
                }
                self.base_model = self.model.clone();
                self.saved = self.model.clone();
                self.disk_text = Some(r.toml.clone());
                self.imported = None;
            }
            InstallKind::Restore(from) => {
                self.log(Tag::Ok, format!("restored {}", home_relative(from)));
                self.reload(true);
            }
        }
        self.flash = Some(("INSTALLED".into(), Instant::now()));
        if std::env::var_os("CUSTTYMIZE_SHELL").is_some() {
            self.log(Tag::Ok, "press enter in your shell to see it".into());
        } else {
            self.log(Tag::Warn, "not active in this shell yet: add  eval \"$(custtymize init zsh)\"  to ~/.zshrc".into());
        }
        if r.quit_after {
            self.quit = true;
        }
    }

    /// Backups next to the live config: our timestamped ones and any other `*.bak*`.
    pub fn list_backups(&self) -> Vec<BackupEntry> {
        let Some(dir) = self.config_path.parent() else { return vec![] };
        let Some(stem) = self.config_path.file_name().and_then(|n| n.to_str()) else { return vec![] };
        let prefix = format!("{stem}.bak");
        let mut out: Vec<(std::time::SystemTime, BackupEntry)> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.file_name().to_str().is_some_and(|n| n.starts_with(&prefix)))
            .filter_map(|e| {
                let meta = e.metadata().ok()?;
                let mtime = meta.modified().ok()?;
                let modified = chrono::DateTime::<chrono::Local>::from(mtime).format("%Y-%m-%d %H:%M").to_string();
                Some((mtime, BackupEntry { path: e.path(), modified, size: meta.len() }))
            })
            .collect();
        out.sort_by_key(|e| std::cmp::Reverse(e.0));
        out.into_iter().map(|(_, e)| e).collect()
    }

    fn open_backups(&mut self) {
        let entries = self.list_backups();
        if entries.is_empty() {
            self.log(Tag::Warn, format!("no backups next to {}", self.config_display()));
            return;
        }
        let mut b = BackupBrowser { entries, idx: 0, preview: None, preview_for: None, preview_error: None };
        self.render_backup_preview(&mut b);
        self.mode = Mode::Backups(b);
    }

    fn render_backup_preview(&mut self, b: &mut BackupBrowser) {
        if b.preview_for == Some(b.idx) {
            return;
        }
        b.preview_for = Some(b.idx);
        b.preview = None;
        b.preview_error = None;
        let (Some(env), Some(entry)) = (self.env.clone(), b.entries.get(b.idx)) else { return };
        match std::fs::read_to_string(&entry.path) {
            Ok(text) => {
                let o = preview::render_sync(&env, &text, self.scenario, self.preview_width);
                b.preview_error = o.error.clone();
                b.preview = Some((rendered(&o.success), rendered(&o.failure)));
            }
            Err(e) => b.preview_error = Some(e.to_string()),
        }
    }

    fn key_backups(&mut self, mut b: BackupBrowser, k: KeyEvent) -> Mode {
        match k.code {
            KeyCode::Char('j') | KeyCode::Down => b.idx = (b.idx + 1).min(b.entries.len().saturating_sub(1)),
            KeyCode::Char('k') | KeyCode::Up => b.idx = b.idx.saturating_sub(1),
            KeyCode::Enter | KeyCode::Char('r') => {
                let Some(entry) = b.entries.get(b.idx).cloned() else { return Mode::Normal };
                if self.dirty() {
                    self.log(Tag::Err, "unsaved edits — save or :reset them before restoring a backup".into());
                    return Mode::Backups(b);
                }
                return match std::fs::read_to_string(&entry.path) {
                    Ok(text) => {
                        self.open_review(InstallKind::Restore(entry.path), text, false);
                        // Validation failure leaves us in Normal mode with the reason logged.
                        std::mem::replace(&mut self.mode, Mode::Normal)
                    }
                    Err(e) => {
                        self.log(Tag::Err, format!("can't read backup: {e}"));
                        Mode::Backups(b)
                    }
                };
            }
            KeyCode::Char('l') | KeyCode::Char('o') => {
                // Load into the editor without touching the live file.
                let Some(entry) = b.entries.get(b.idx).cloned() else { return Mode::Normal };
                match load::load(&entry.path) {
                    Ok(l) => {
                        self.replace_model(l.model, &format!("loaded {} into the editor (not saved)", home_relative(&entry.path)));
                        return Mode::Normal;
                    }
                    Err(e) => self.log(Tag::Err, format!("can't load backup: {e}")),
                }
            }
            _ => return Mode::Normal,
        }
        self.render_backup_preview(&mut b);
        Mode::Backups(b)
    }

    fn reload(&mut self, force: bool) {
        if self.dirty() && !force {
            self.log(Tag::Err, "unsaved changes — :reload! to discard them".into());
            return;
        }
        match load::load(&self.config_path) {
            Ok(l) => {
                self.disk_text = std::fs::read_to_string(&self.config_path).ok();
                let before = self.model.clone();
                self.model = l.model.clone();
                self.saved = l.model.clone();
                self.base_model = l.model;
                self.base_doc = l.doc;
                self.sel = self.sel.min(self.model.segments.len().saturating_sub(1));
                for n in l.notes {
                    self.log(Tag::Warn, n);
                }
                if before != self.model {
                    self.undo.push(before);
                }
                self.log(Tag::Ok, format!("reloaded {}", self.config_display()));
                self.request_preview();
            }
            Err(e) => self.log(Tag::Err, format!("reload failed: {e}")),
        }
    }

    fn set_command(&mut self, arg: &str) {
        let (lhs, value) = match arg.trim().split_once(char::is_whitespace) {
            Some((l, v)) => (l, v.trim()),
            None => (arg.trim(), ""),
        };
        let Some((module, key)) = lhs.split_once('.') else {
            self.log(Tag::Err, "usage: :set <module>.<key> <value>".into());
            return;
        };
        let Some(idx) = self.model.segments.iter().position(|s| s.name == module) else {
            self.log(Tag::Err, format!("no module {module} (try :add {module})"));
            return;
        };
        let shape = self.model.globals.shape;
        let before = self.model.clone();
        let seg = &mut self.model.segments[idx];
        let truthy = |v: &str| matches!(v, "true" | "on" | "yes" | "1" | "");
        let result: Result<(), String> = match key {
            "fg" | "bg" | "color" => match Rgb::parse_any(value) {
                Some(c) => {
                    if key == "bg" { seg.bg = c } else { seg.fg = c }
                    Ok(())
                }
                None => Err(format!("{} is not a colour", quote(value))),
            },
            "symbol" => {
                seg.symbol = Some(value.trim_matches('"').to_string());
                Ok(())
            }
            "bold" => {
                seg.bold = truthy(value);
                Ok(())
            }
            "italic" => {
                seg.italic = truthy(value);
                Ok(())
            }
            "enabled" => {
                seg.enabled = truthy(value);
                Ok(())
            }
            "side" => match value {
                "left" | "l" => {
                    seg.side = Side::Left;
                    Ok(())
                }
                "right" | "r" => {
                    seg.side = Side::Right;
                    Ok(())
                }
                _ => Err("side is left or right".into()),
            },
            "format" => {
                seg.format_override = if value.is_empty() { None } else { Some(value.trim_matches('\'').to_string()) };
                Ok(())
            }
            other => match seg.def().options.iter().find(|o| o.key == other) {
                Some(o) => {
                    let v = match o.kind {
                        OptKind::Bool => Ok(OptValue::Bool(truthy(value))),
                        OptKind::Int => value.parse().map(OptValue::Int).map_err(|_| format!("{} is not an integer", quote(value))),
                        OptKind::Str => Ok(OptValue::Str(value.trim_matches('"').to_string())),
                    };
                    v.map(|v| {
                        seg.options.insert(o.key.to_string(), v);
                    })
                }
                None => Err(format!("{module} has no option {other}")),
            },
        };
        match result {
            Err(e) => {
                self.model = before;
                self.log(Tag::Err, e);
            }
            Ok(()) => {
                let msg = match key {
                    "fg" | "bg" | "color" | "bold" | "italic" => self.style_msg(module),
                    _ => format!("{module}.{key} = {}", quote(value)),
                };
                let _ = shape;
                self.sel = idx;
                self.commit(before, Tag::Write, msg);
            }
        }
    }

    pub fn run_command(&mut self, line: &str) {
        let line = line.trim().trim_start_matches(':');
        let (cmd, arg) = match line.split_once(char::is_whitespace) {
            Some((c, a)) => (c, a.trim()),
            None => (line, ""),
        };
        self.log(Tag::Cmd, format!(":{line}"));
        match cmd {
            "w" | "write" | "install" => {
                let path = if arg.is_empty() { None } else { Some(expand(arg)) };
                match path {
                    Some(p) if p != self.config_path => {
                        self.export(&p);
                    }
                    _ => self.begin_install(false, false),
                }
            }
            "w!" => self.begin_install(true, false),
            "q" | "quit" => self.request_quit(false),
            "q!" | "quit!" => self.request_quit(true),
            "wq" | "x" => self.begin_install(false, true),
            "wq!" | "x!" => self.begin_install(true, true),
            "preset" | "presets" => match presets::find(arg) {
                _ if arg.is_empty() => self.open_gallery(GalleryKind::Presets),
                Some(p) => self.apply_design(p),
                None => self.log(
                    Tag::Err,
                    format!("unknown preset {}; try: {}", quote(arg), PRESETS.iter().map(|p| p.name).collect::<Vec<_>>().join(", ")),
                ),
            },
            "theme" | "themes" | "palette" => match palette::find(arg) {
                _ if arg.is_empty() => self.open_gallery(GalleryKind::Themes),
                Some(p) => self.apply_theme(p),
                None => self.log(
                    Tag::Err,
                    format!("unknown theme {}; try: {}", quote(arg), PALETTES.iter().map(|p| p.name).collect::<Vec<_>>().join(", ")),
                ),
            },
            "glyphs" | "font" | "fonts" => match Glyphs::parse(arg) {
                Some(g) => self.set_glyphs(g),
                None if arg.is_empty() => self.set_glyphs(self.model.globals.glyphs.cycle(1)),
                None => self.log(Tag::Err, "glyph sets: nerd unicode ascii".into()),
            },
            "save" => self.save_named(arg, false),
            "save!" => self.save_named(arg, true),
            "open" | "saved" | "load" => {
                if arg.is_empty() {
                    self.open_gallery(GalleryKind::Saved);
                } else {
                    let path = library::sanitize(arg).map(|n| self.library_dir().join(format!("{n}.toml")));
                    match path.map(|p| (load::load(&p), p)) {
                        Some((Ok(l), p)) if p.exists() => {
                            self.replace_model(l.model, &format!("opened saved config {arg} (ctrl-s to install it)"))
                        }
                        _ => self.log(Tag::Err, format!("no saved config {} — O lists them", quote(arg))),
                    }
                }
            }
            "welcome" => self.mode = Mode::Welcome,
            "undo" | "u" => self.undo(),
            "redo" | "r" => self.redo(),
            "export" | "e" => {
                if arg.is_empty() {
                    self.log(Tag::Err, "usage: :export <path>".into());
                } else {
                    self.export(&expand(arg));
                }
            }
            "shape" => match Shape::parse(arg) {
                Some(s) => self.set_shape(s),
                None => self.log(Tag::Err, "shapes: powerline rounded slanted flat plain".into()),
            },
            "layout" => match Layout::parse(arg) {
                Some(l) => self.set_layout(l),
                None => self.log(Tag::Err, "layouts: one two right".into()),
            },
            "cap" => match Cap::parse(arg) {
                Some(c) => self.set_cap(c),
                None => self.log(Tag::Err, "caps: gradient rounded none".into()),
            },
            "char" => {
                if arg.is_empty() {
                    self.log(Tag::Err, "usage: :char <glyph>".into());
                } else {
                    let before = self.model.clone();
                    self.model.globals.prompt_char = arg.to_string();
                    self.commit(before, Tag::Write, format!("prompt char = {}", quote(arg)));
                }
            }
            "scenario" => match Scenario::parse(arg) {
                Some(s) => {
                    self.scenario = s;
                    self.log(Tag::Preview, format!("scenario → {}", s.label()));
                    self.request_preview();
                }
                None if arg.is_empty() => self.cycle_scenario(),
                None => self.log(Tag::Err, "scenarios: dirty-repo clean-repo deep-path ssh-session home-dir".into()),
            },
            "set" => self.set_command(arg),
            "add" => self.add_module(arg),
            "rm" | "remove" | "delete" => {
                if !arg.is_empty()
                    && let Some(i) = self.model.segments.iter().position(|s| s.name == arg) {
                        self.sel = i;
                    }
                self.remove_selected();
            }
            "toml" | "view" => self.mode = Mode::TomlView(0),
            "backups" | "restore" => self.open_backups(),
            "help" | "h" => self.mode = Mode::Help(0),
            "random" | "glitch" => self.randomize(),
            "matrix" => self.start_matrix(),
            "reload" => self.reload(false),
            "reload!" => self.reload(true),
            "reset" => {
                let before = self.model.clone();
                self.model = self.saved.clone();
                self.commit(before, Tag::Write, "reset to last saved state".into());
            }
            "yank" | "y" => self.yank(),
            "clear" => self.log.clear(),
            "colors" | "colours" => {
                self.truecolor = match arg {
                    "true" | "24" | "24bit" | "truecolor" => true,
                    "256" => false,
                    _ => !self.truecolor,
                };
                self.log(Tag::Sys, format!("colour mode: {}", if self.truecolor { "truecolor" } else { "256" }));
            }
            "newline" => {
                let before = self.model.clone();
                self.model.globals.add_newline = !self.model.globals.add_newline;
                self.commit(before, Tag::Write, format!("add_newline = {}", self.model.globals.add_newline));
            }
            "whoami" => self.log(Tag::Sys, "a ghost in the shell".into()),
            "hack" => {
                for l in [
                    "bypassing mainframe firewall… [ OK ]",
                    "decrypting ~/.config with rot26… [ OK ]",
                    "uploading prompt to the gibson… [ OK ]",
                    "just kidding. nothing left this machine.",
                ] {
                    self.log(Tag::Sys, l.into());
                }
            }
            "" => {}
            other => self.log(Tag::Err, format!("E492: not an editor command: {other} (try :help)")),
        }
    }
}

pub const COMMANDS: &[(&str, &str)] = &[
    ("w", "[path]   review before/after, back up, install (or save a copy)"),
    ("backups", "         browse, preview and restore previous setups"),
    ("w!", "         save even if the file changed on disk"),
    ("q", "         quit (asks if unsaved)"),
    ("q!", "         quit without saving"),
    ("wq", "         save and quit"),
    ("x", "         save and quit"),
    ("preset", "[name]   apply a complete design (no name: gallery)"),
    ("theme", "[name]   apply a colour theme (no name: gallery)"),
    ("glyphs", "<g>      nerd|unicode|ascii (what your font can draw)"),
    ("save", "<name>   save the current design to your library"),
    ("open", "[name]   open a saved design (no name: gallery)"),
    ("welcome", "         show the getting-started screen"),
    ("undo", "         undo"),
    ("redo", "         redo"),
    ("export", "<path>   validate and write the TOML elsewhere"),
    ("shape", "<s>      powerline|rounded|slanted|flat|plain"),
    ("layout", "<l>      one|two|right"),
    ("cap", "<c>      gradient|rounded|none"),
    ("char", "<glyph>  prompt character"),
    ("scenario", "<s>      preview context"),
    ("set", "<m.k> <v> set any field, e.g. git_branch.bg #ff79c6"),
    ("add", "<module>  add a module (catalog or custom.<name>)"),
    ("rm", "[module]  remove a module"),
    ("toml", "         view the generated TOML"),
    ("yank", "         copy the generated TOML (pbcopy)"),
    ("random", "         randomise colours"),
    ("reload", "         reload from disk"),
    ("reset", "         back to the last saved state"),
    ("newline", "         toggle add_newline"),
    ("colors", "<true|256> UI colour depth"),
    ("clear", "         clear the log"),
    ("matrix", "         …"),
    ("help", "         keybindings"),
];

fn expand(p: &str) -> PathBuf {
    match (p.strip_prefix("~/"), dirs::home_dir()) {
        (Some(rest), Some(h)) => h.join(rest),
        _ => PathBuf::from(p),
    }
}

/// Starship ANSI output → ratatui text. `\e[49m`/`\e[39m` mean "terminal default":
/// those become "inherit" so the preview screen colour shows through.
pub fn ansi_text(s: &str) -> Text<'static> {
    let trimmed = s.trim_start_matches('\n');
    let mut text = trimmed.as_bytes().into_text().unwrap_or_else(|_| Text::raw(trimmed.to_string()));
    for line in &mut text.lines {
        for span in &mut line.spans {
            if span.style.bg == Some(Color::Reset) {
                span.style.bg = None;
            }
            if span.style.fg == Some(Color::Reset) {
                span.style.fg = None;
            }
        }
    }
    text
}

fn rendered(p: &PromptOutput) -> RenderedPrompt {
    RenderedPrompt { left: ansi_text(&p.left), right: ansi_text(&p.right) }
}

fn hsl(h: f32, s: f32, l: f32) -> Rgb {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let f = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgb(f(r), f(g), f(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::catalog::SymbolMode;

    #[test]
    fn hsl_primaries() {
        assert_eq!(hsl(0.0, 1.0, 0.5), Rgb(255, 0, 0));
        assert_eq!(hsl(120.0, 1.0, 0.5), Rgb(0, 255, 0));
    }

    #[test]
    fn symbol_mode_is_known_for_catalog() {
        for d in catalog::CATALOG {
            if d.symbol_mode == SymbolMode::Inline {
                assert!(d.body.contains("$symbol"), "{}", d.name);
            }
        }
    }
}
