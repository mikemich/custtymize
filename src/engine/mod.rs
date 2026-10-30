//! The prompt engine: renders a starship-compatible config without starship.
//!
//! `render` is what the shell runs on every prompt (via `custtymize prompt`) and what the
//! editor's live preview calls in-process, so the preview is exactly the real prompt.

pub mod format;
pub mod init;
mod modules;
pub mod style;
pub mod util;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use toml_edit::{DocumentMut, Item, TableLike};

use format::{Node, Val};
use style::{Seg, to_ansi};

pub use modules::SUPPORTED;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shell {
    Zsh,
    Bash,
    Fish,
    /// Raw ANSI with no shell escape wrappers (previews, `--print-preview`).
    None,
}

impl Shell {
    pub fn parse(s: &str) -> Option<Shell> {
        Some(match s {
            "zsh" => Shell::Zsh,
            "bash" => Shell::Bash,
            "fish" => Shell::Fish,
            "none" | "" => Shell::None,
            _ => return None,
        })
    }
}

/// Starship's default `format` (its `$all`, minus modules this engine doesn't implement).
pub const DEFAULT_FORMAT: &str = "$username$hostname$kubernetes$directory$git_branch$git_commit$git_state$git_status\
$docker_context$package$bun$dart$deno$golang$java$kotlin$lua$nodejs$perl$php$python$ruby$rust$swift$terraform$zig\
$memory_usage$aws$custom$sudo$cmd_duration$line_break$jobs$battery$time$status$os$character";

/// Everything a prompt render needs to know about the shell's situation.
pub struct Context {
    pub cwd: PathBuf,
    pub home: Option<PathBuf>,
    pub status: i32,
    pub duration_ms: Option<u64>,
    pub jobs: u32,
    pub width: u16,
    pub keymap: String,
    pub timeout: Duration,
    /// Overrides on top of the process environment; `None` unsets a variable.
    env: HashMap<String, Option<String>>,
    pub(crate) listing: OnceLock<modules::Listing>,
    pub(crate) repo: OnceLock<Option<modules::git::Repo>>,
    pub(crate) git_status: OnceLock<Option<modules::git::Status>>,
    pub(crate) versions: util::VersionCache,
    warnings: Mutex<Vec<String>>,
}

impl Context {
    pub fn new(cwd: PathBuf) -> Context {
        Context {
            cwd,
            home: dirs::home_dir(),
            status: 0,
            duration_ms: None,
            jobs: 0,
            width: 80,
            keymap: String::new(),
            timeout: Duration::from_millis(500),
            env: HashMap::new(),
            listing: OnceLock::new(),
            repo: OnceLock::new(),
            git_status: OnceLock::new(),
            versions: util::VersionCache::new(None),
            warnings: Mutex::new(Vec::new()),
        }
    }

    /// The directory the shell is in: `$PWD` when it names the same place as the real
    /// cwd (keeps symlinked paths as the user typed them), the real cwd otherwise.
    pub fn from_shell() -> Context {
        let real = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let cwd = std::env::var_os("PWD")
            .map(PathBuf::from)
            .filter(|p| p.canonicalize().ok() == real.canonicalize().ok())
            .unwrap_or(real);
        let mut ctx = Context::new(cwd);
        ctx.use_version_cache_file();
        ctx
    }

    /// Persists tool versions between renders in the cache dir.
    pub fn use_version_cache_file(&mut self) {
        self.versions = util::VersionCache::new(crate::sandbox::cache_dir().ok().map(|d| d.join("versions.tsv")));
    }

    pub fn set_env(&mut self, key: &str, val: Option<String>) {
        self.env.insert(key.to_string(), val);
    }

    pub fn var(&self, key: &str) -> Option<String> {
        match self.env.get(key) {
            Some(v) => v.clone(),
            None => std::env::var(key).ok(),
        }
        .filter(|v| !v.is_empty())
    }

    pub fn apply_env(&self, cmd: &mut Command) {
        for (k, v) in &self.env {
            match v {
                Some(v) => cmd.env(k, v),
                None => cmd.env_remove(k),
            };
        }
    }

    pub fn warn(&self, msg: String) {
        let mut w = self.warnings.lock().unwrap_or_else(|e| e.into_inner());
        if !w.contains(&msg) {
            w.push(msg);
        }
    }

    pub fn home_dir(&self) -> Option<PathBuf> {
        self.var("HOME").map(PathBuf::from).or_else(|| self.home.clone())
    }
}

/// Read access to one module's table (`[git_branch]`, `[custom.vpn]`).
#[derive(Clone, Copy)]
pub struct Cfg<'a> {
    t: Option<&'a dyn TableLike>,
}

impl<'a> Cfg<'a> {
    pub fn of(doc: &'a DocumentMut, name: &str) -> Cfg<'a> {
        let mut table: Option<&dyn TableLike> = Some(doc.as_table());
        for part in name.split('.') {
            table = table.and_then(|t| t.get(part)).and_then(Item::as_table_like);
        }
        Cfg { t: table }
    }

    pub fn get(&self, key: &str) -> Option<&'a Item> {
        self.t.and_then(|t| t.get(key))
    }

    pub fn opt_str(&self, key: &str) -> Option<&'a str> {
        self.get(key).and_then(Item::as_str)
    }

    pub fn str(&self, key: &str, default: &str) -> String {
        self.opt_str(key).unwrap_or(default).to_string()
    }

    pub fn int(&self, key: &str, default: i64) -> i64 {
        self.get(key).and_then(Item::as_integer).unwrap_or(default)
    }

    pub fn bool(&self, key: &str, default: bool) -> bool {
        self.get(key).and_then(Item::as_bool).unwrap_or(default)
    }

    pub fn strs(&self, key: &str, default: &[&str]) -> Vec<String> {
        match self.get(key).and_then(Item::as_array) {
            Some(a) => a.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
            None => default.iter().map(|s| s.to_string()).collect(),
        }
    }

    pub fn table(&self, key: &str) -> Cfg<'a> {
        Cfg { t: self.get(key).and_then(Item::as_table_like) }
    }

    pub fn entries(&self) -> Vec<(&'a str, &'a Item)> {
        self.t.map(|t| t.iter().collect()).unwrap_or_default()
    }
}

#[derive(Debug, Default)]
pub struct Rendered {
    pub left: String,
    pub right: String,
    pub timings: Vec<(String, Duration)>,
    pub warnings: Vec<String>,
}

/// Replaces `$all` with the default module list, minus modules named elsewhere.
fn expand_all(fmt: &str, others: &str, doc: &DocumentMut) -> String {
    if !fmt.contains("$all") && !fmt.contains("$custom") {
        return fmt.to_string();
    }
    let mut named = Vec::new();
    for src in [fmt, others] {
        if let Ok(nodes) = format::parse(src) {
            format::variables(&nodes, &mut named);
        }
    }
    let all: String = DEFAULT_FORMAT
        .split('$')
        .filter(|m| !m.is_empty() && !named.iter().any(|n| n == m))
        .map(|m| format!("${m}"))
        .collect();
    // `$custom` (inside `$all` or on its own) means every `[custom.*]` not named elsewhere.
    let customs: String = doc
        .get("custom")
        .and_then(Item::as_table_like)
        .map(|t| {
            t.iter()
                .map(|(k, _)| format!("custom.{k}"))
                .filter(|k| !named.contains(k))
                .map(|k| format!("${{{k}}}"))
                .collect()
        })
        .unwrap_or_default();
    fmt.replace("$all", &all).replace("$custom", &customs)
}

fn parse_top(src: &str, what: &str, ctx: &Context) -> Vec<Node> {
    format::parse(src).unwrap_or_else(|e| {
        ctx.warn(format!("{what}: {e}"));
        Vec::new()
    })
}

/// Renders the left and right prompts.
pub fn render(doc: &DocumentMut, ctx: &Context, shell: Shell) -> Rendered {
    let root = doc.as_table();
    let left_src = root.get("format").and_then(Item::as_str).unwrap_or(DEFAULT_FORMAT);
    let right_src = root.get("right_format").and_then(Item::as_str).unwrap_or("");
    let left_nodes = parse_top(&expand_all(left_src, right_src, doc), "format", ctx);
    let right_nodes = parse_top(&expand_all(right_src, left_src, doc), "right_format", ctx);

    let mut names = Vec::new();
    format::variables(&left_nodes, &mut names);
    format::variables(&right_nodes, &mut names);
    names.dedup();
    let mut seen = std::collections::HashSet::new();
    names.retain(|n| seen.insert(n.clone()));

    // Modules run in parallel; git and version lookups are the slow parts.
    let results: Vec<(String, Option<Vec<Seg>>, Duration)> = std::thread::scope(|s| {
        let handles: Vec<_> = names
            .iter()
            .map(|name| {
                s.spawn(move || {
                    let start = Instant::now();
                    let out = modules::render(name, ctx, doc);
                    (name.clone(), out, start.elapsed())
                })
            })
            .collect();
        handles.into_iter().filter_map(|h| h.join().ok()).collect()
    });

    let outputs: HashMap<&str, &Vec<Seg>> =
        results.iter().filter_map(|(n, o, _)| o.as_ref().map(|o| (n.as_str(), o))).collect();
    let lookup = |k: &str| outputs.get(k).map(|segs| Val::Segs((*segs).clone()));
    let mut bad = Vec::new();
    let mut left = format::eval(&left_nodes, &lookup, &mut bad);
    let right = format::eval(&right_nodes, &lookup, &mut bad);
    for b in bad {
        ctx.warn(format!("unknown style word `{b}` in format"));
    }
    if root.get("add_newline").and_then(Item::as_bool).unwrap_or(true) {
        left.insert(0, Seg { text: "\n".into(), style: Default::default() });
    }

    ctx.versions.save();
    let warnings = ctx.warnings.lock().map(|w| w.clone()).unwrap_or_default();
    Rendered {
        left: to_ansi(&left, shell),
        right: to_ansi(&right, shell),
        timings: results.into_iter().map(|(n, _, d)| (n, d)).collect(),
        warnings,
    }
}

/// `command_timeout` from the config, in the same units starship uses (ms).
pub fn config_timeout(doc: &DocumentMut) -> Duration {
    let ms = doc.as_table().get("command_timeout").and_then(Item::as_integer).unwrap_or(500);
    Duration::from_millis(ms.clamp(10, 10_000) as u64)
}

/// Checks a config without rendering it: format strings must parse (errors) and modules
/// should be ones this engine knows (warnings).
pub fn validate(doc: &DocumentMut) -> (Vec<String>, Vec<String>) {
    let (mut errors, mut warnings) = (Vec::new(), Vec::new());
    let root = doc.as_table();
    let mut names = Vec::new();
    for key in ["format", "right_format"] {
        if let Some(src) = root.get(key).and_then(Item::as_str) {
            match format::parse(src) {
                Ok(nodes) => format::variables(&nodes, &mut names),
                Err(e) => errors.push(format!("{key}: {e}")),
            }
        }
    }
    for name in &names {
        if name == "all" || name == "custom" || modules::is_supported(name) {
            continue;
        }
        let w = format!("module `{name}` isn't supported by the custtymize engine; it will render empty");
        if !warnings.contains(&w) {
            warnings.push(w);
        }
    }
    // Every string option that is itself a format.
    for (name, item) in root.iter() {
        let Some(t) = item.as_table_like() else { continue };
        let mut check = |label: String, src: &str| {
            if let Err(e) = format::parse(src) {
                errors.push(format!("[{label}] {e}"));
            }
        };
        let mut visit = |prefix: &str, t: &dyn TableLike| {
            for (k, v) in t.iter() {
                if let Some(s) = v.as_str()
                    && modules::is_format_key(prefix, k)
                {
                    check(format!("{prefix}.{k}"), s);
                }
            }
        };
        if name == "custom" {
            for (cname, ct) in t.iter() {
                if let Some(ct) = ct.as_table_like() {
                    visit(&format!("custom.{cname}"), ct);
                }
            }
        } else {
            visit(name, t);
        }
    }
    (errors, warnings)
}

/// Where the engine reads its config: `$CUSTTYMIZE_CONFIG`, then
/// `~/.config/custtymize/prompt.toml`, falling back to starship's config so `init` works
/// before anything has been saved.
pub fn config_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CUSTTYMIZE_CONFIG") {
        return Some(PathBuf::from(p));
    }
    let own = own_config_path()?;
    if own.exists() {
        return Some(own);
    }
    let starship = std::env::var_os("STARSHIP_CONFIG")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".config/starship.toml")))?;
    Some(if starship.exists() { starship } else { own })
}

pub fn own_config_path() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join(".config").join("custtymize").join("prompt.toml"))
}

/// Parses a config file; a missing file is an empty config (all defaults).
pub fn load_doc(path: &Path) -> Result<DocumentMut, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => text.parse::<DocumentMut>().map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(DocumentMut::new()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(s: &str) -> DocumentMut {
        s.parse().expect("toml")
    }

    fn ctx_in_empty_dir() -> Context {
        let dir = std::env::temp_dir().join(format!("custtymize-engine-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let mut ctx = Context::new(dir);
        ctx.set_env("HOME", Some("/nonexistent-home".into()));
        ctx
    }

    #[test]
    fn renders_custom_modules_and_character() {
        let d = doc(
            "add_newline = false\nformat = '[>](bold red) ${custom.hi}$character'\n\
             [custom.hi]\ncommand = 'echo hello'\nwhen = true\nformat = '[$output]($style) '\n\
             [character]\nerror_symbol = '[x](red)'\n",
        );
        let mut ctx = ctx_in_empty_dir();
        let r = render(&d, &ctx, Shell::None);
        assert_eq!(r.left, "\x1b[1;31m>\x1b[0m \x1b[1;32mhello\x1b[0m \x1b[1;32m❯\x1b[0m ");
        ctx.status = 1;
        let r = render(&d, &ctx, Shell::Zsh);
        assert!(r.left.ends_with("%{\x1b[31m%}x%{\x1b[0m%} "), "{:?}", r.left);
    }

    #[test]
    fn all_expands_without_duplicates() {
        let d = doc("format = '$character$all'\n");
        let expanded = expand_all("$character$all", "", &d);
        assert_eq!(expanded.matches("$character").count(), 1);
        assert!(expanded.contains("$directory$git_branch"));
    }

    #[test]
    fn validation_reports_errors_and_unknown_modules() {
        let (errors, warnings) = validate(&doc("format = '$directory$kotlinx$character'\n[directory]\nformat = '[$path('\n"));
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].starts_with("[directory.format]"));
        assert!(warnings.iter().any(|w| w.contains("kotlinx")));
        assert_eq!(validate(&doc("format = '$all'\n[custom.x]\ncommand='true'\n")), (vec![], vec![]));
    }
}
