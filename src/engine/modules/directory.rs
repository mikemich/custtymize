//! `directory`: the working directory, contracted to the repo or `~` and truncated.

use std::path::Path;

use super::super::format::Vars;
use super::super::style::Seg;
use super::super::{Cfg, Context};
use super::{finish, git};

/// Keeps the last `n` components; `n == 0` keeps everything.
fn truncate(path: &str, n: usize) -> String {
    if n == 0 || path == "/" {
        return path.to_string();
    }
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    if parts.len() <= n {
        return path.to_string();
    }
    parts[parts.len() - n..].join("/")
}

fn joined(base: &str, rest: &Path) -> String {
    let rest = rest.to_string_lossy();
    if rest.is_empty() { base.to_string() } else { format!("{base}/{rest}") }
}

pub fn display_path(ctx: &Context, cfg: Cfg) -> String {
    let home = ctx.home_dir();
    let home_sym = cfg.str("home_symbol", "~");
    let cwd = &ctx.cwd;
    let repo_root = git::repo(ctx).map(|r| r.root.clone());

    let mut path = match (&repo_root, &home) {
        (Some(root), home) if cfg.bool("truncate_to_repo", true) && Some(root) != home.as_ref() => {
            let name = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            joined(&name, cwd.strip_prefix(root).unwrap_or(Path::new("")))
        }
        (_, Some(h)) if cwd.starts_with(h) => joined(&home_sym, cwd.strip_prefix(h).unwrap_or(Path::new(""))),
        _ => cwd.to_string_lossy().into_owned(),
    };
    for (from, to) in cfg.table("substitutions").entries() {
        if let Some(to) = to.as_str() {
            path = path.replace(from, to);
        }
    }
    let n = cfg.int("truncation_length", 3).max(0) as usize;
    let truncated = truncate(&path, n);
    let symbol = cfg.str("truncation_symbol", "");
    if !(truncated.starts_with(&home_sym) || truncated.starts_with('/')) {
        format!("{symbol}{truncated}")
    } else {
        truncated
    }
}

fn writable(p: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let Ok(c) = std::ffi::CString::new(p.as_os_str().as_bytes()) else { return true };
    // SAFETY: `c` is a valid NUL-terminated path for the duration of the call.
    unsafe { libc::access(c.as_ptr(), libc::W_OK) == 0 }
}

pub fn render(ctx: &Context, cfg: Cfg) -> Option<Vec<Seg>> {
    let mut v = Vars::new();
    v.text("path", display_path(ctx, cfg));
    v.text("read_only", if writable(&ctx.cwd) { String::new() } else { cfg.str("read_only", "🔒") });
    v.text("read_only_style", cfg.str("read_only_style", "red"));
    finish(ctx, "directory", cfg, "[$path]($style)[$read_only]($read_only_style) ", "cyan bold", "", &mut v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncation() {
        assert_eq!(truncate("~/a/b/c", 3), "a/b/c");
        assert_eq!(truncate("~/a/b", 3), "~/a/b");
        assert_eq!(truncate("/usr/local/bin/x", 2), "bin/x");
        assert_eq!(truncate("/", 3), "/");
        assert_eq!(truncate("/a/b/c/d", 0), "/a/b/c/d");
    }

    #[test]
    fn home_and_symbol() {
        let doc: toml_edit::DocumentMut = "[directory]\ntruncation_symbol = '…/'\n".parse().expect("toml");
        let mut ctx = Context::new("/tmp/custtymize-nowhere/x/y/z/w".into());
        ctx.set_env("HOME", Some("/tmp/custtymize-nowhere".into()));
        assert_eq!(display_path(&ctx, Cfg::of(&doc, "directory")), "…/y/z/w");
        let ctx2 = {
            let mut c = Context::new("/tmp/custtymize-nowhere/x".into());
            c.set_env("HOME", Some("/tmp/custtymize-nowhere".into()));
            c
        };
        assert_eq!(display_path(&ctx2, Cfg::of(&doc, "directory")), "~/x");
    }
}
