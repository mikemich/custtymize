//! Module implementations. Defaults follow starship's, so existing starship configs
//! render the same way.

mod directory;
pub mod git;
mod lang;
mod sys;

use std::collections::HashSet;

use toml_edit::DocumentMut;

use super::format::{self, Vars};
use super::style::Seg;
use super::{Cfg, Context};

pub const SUPPORTED: &[&str] = &[
    "aws", "battery", "bun", "character", "cmd_duration", "dart", "deno", "directory", "docker_context",
    "git_branch", "git_commit", "git_state", "git_status", "golang", "hostname", "java", "jobs", "kotlin",
    "kubernetes", "line_break", "lua", "memory_usage", "nodejs", "os", "package", "perl", "php", "python",
    "ruby", "rust", "status", "sudo", "swift", "terraform", "time", "username", "zig",
];

pub fn is_supported(name: &str) -> bool {
    SUPPORTED.contains(&name) || name.starts_with("custom.")
}

/// Whether `[module].key` holds a format string (checked by `validate`).
pub fn is_format_key(module: &str, key: &str) -> bool {
    key == "format"
        || (module == "character" && key.ends_with("_symbol"))
        || (module == "git_status"
            && matches!(
                key,
                "conflicted" | "ahead" | "behind" | "diverged" | "up_to_date" | "untracked" | "stashed"
                    | "modified" | "staged" | "renamed" | "deleted" | "typechanged"
            ))
}

/// The prompt directory's entries, read once and shared by every detector.
#[derive(Default)]
pub struct Listing {
    pub files: HashSet<String>,
    pub exts: HashSet<String>,
    pub folders: HashSet<String>,
}

impl Listing {
    fn read(ctx: &Context) -> Listing {
        let mut l = Listing::default();
        let Ok(rd) = std::fs::read_dir(&ctx.cwd) else { return l };
        for e in rd.flatten().take(5000) {
            let name = e.file_name().to_string_lossy().into_owned();
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                l.folders.insert(name);
            } else {
                if let Some((_, ext)) = name.rsplit_once('.')
                    && !ext.is_empty()
                {
                    l.exts.insert(ext.to_string());
                }
                l.files.insert(name);
            }
        }
        l
    }
}

pub fn listing(ctx: &Context) -> &Listing {
    ctx.listing.get_or_init(|| Listing::read(ctx))
}

/// Starship's `detect_files` / `detect_extensions` / `detect_folders` check.
pub fn detected(ctx: &Context, cfg: Cfg, files: &[&str], exts: &[&str], folders: &[&str]) -> bool {
    let l = listing(ctx);
    cfg.strs("detect_files", files).iter().any(|f| l.files.contains(f))
        || cfg.strs("detect_extensions", exts).iter().any(|e| l.exts.contains(e))
        || cfg.strs("detect_folders", folders).iter().any(|f| l.folders.contains(f))
}

/// Renders a module's `format` with `vars`, plus the usual `$style` and `$symbol`.
pub fn finish(ctx: &Context, name: &str, cfg: Cfg, def_format: &str, def_style: &str, def_symbol: &str, vars: &mut Vars) -> Option<Vec<Seg>> {
    if vars.get("style").is_none() {
        vars.text("style", cfg.str("style", def_style));
    }
    if vars.get("symbol").is_none() {
        vars.text("symbol", cfg.str("symbol", def_symbol));
    }
    let fmt = cfg.str("format", def_format);
    match format::render(&fmt, vars) {
        Ok(segs) => Some(segs),
        Err(e) => {
            ctx.warn(format!("[{name}] {e}"));
            None
        }
    }
}

/// Renders an option that is itself a format (e.g. `success_symbol = "[❯](bold green)"`).
pub fn sub_format(ctx: &Context, name: &str, src: &str, vars: &Vars) -> Vec<Seg> {
    format::render(src, vars).unwrap_or_else(|e| {
        ctx.warn(format!("[{name}] {e}"));
        Vec::new()
    })
}

pub fn render(name: &str, ctx: &Context, doc: &DocumentMut) -> Option<Vec<Seg>> {
    let cfg = Cfg::of(doc, name);
    let off_by_default = matches!(name, "time" | "os" | "status" | "sudo" | "kubernetes" | "memory_usage");
    if cfg.bool("disabled", off_by_default) {
        return None;
    }
    if let Some(custom) = name.strip_prefix("custom.") {
        return sys::custom(ctx, custom, cfg);
    }
    match name {
        "directory" => directory::render(ctx, cfg),
        "git_branch" => git::branch(ctx, cfg),
        "git_status" => git::status(ctx, cfg),
        "git_commit" => git::commit(ctx, cfg),
        "git_state" => git::state(ctx, cfg),
        "line_break" => Some(vec![Seg { text: "\n".into(), style: Default::default() }]),
        "package" => lang::package(ctx, cfg),
        n if lang::SPECS.iter().any(|s| s.name == n) => lang::render(ctx, n, cfg),
        _ => sys::render(ctx, name, cfg),
    }
}
