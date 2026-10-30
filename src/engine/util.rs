//! Running external commands with a timeout, PATH lookup and the version cache.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::Context;

pub struct Output {
    pub stdout: String,
    pub stderr: String,
    pub ok: bool,
}

/// Runs a program in the prompt's directory, killing it after `ctx.timeout`.
pub fn run(ctx: &Context, prog: &Path, args: &[&str]) -> Option<Output> {
    let mut cmd = Command::new(prog);
    cmd.args(args).current_dir(&ctx.cwd).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    ctx.apply_env(&mut cmd);
    let mut child = cmd.spawn().ok()?;
    let mut so = child.stdout.take()?;
    let mut se = child.stderr.take()?;
    // Drain both pipes on threads so a chatty command can't block on a full pipe.
    let ho = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = so.read_to_end(&mut b);
        b
    });
    let he = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = se.read_to_end(&mut b);
        b
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) if start.elapsed() > ctx.timeout => {
                let _ = child.kill();
                let _ = child.wait();
                ctx.warn(format!("`{}` timed out after {} ms", prog.display(), ctx.timeout.as_millis()));
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_micros(300)),
            Err(_) => return None,
        }
    };
    let stdout = String::from_utf8_lossy(&ho.join().unwrap_or_default()).into_owned();
    let stderr = String::from_utf8_lossy(&he.join().unwrap_or_default()).into_owned();
    Some(Output { stdout, stderr, ok: status.success() })
}

/// Finds an executable on the prompt's PATH.
pub fn which(ctx: &Context, name: &str) -> Option<PathBuf> {
    if name.contains('/') {
        return Some(PathBuf::from(name));
    }
    let path = ctx.var("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(name)).find(|p| is_executable(p))
}

fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

pub fn mtime(p: &Path) -> u64 {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The nearest of `names` walking up from `start`, as `path:mtime`.
pub fn nearest_pin(start: &Path, names: &[&str]) -> String {
    let mut dir = Some(start);
    while let Some(d) = dir {
        for n in names {
            let p = d.join(n);
            if p.exists() {
                return format!("{}:{}", p.display(), mtime(&p));
            }
        }
        dir = d.parent();
    }
    String::new()
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Caches `<tool> --version` output across prompts. Keys include the binary's path and
/// mtime plus any version-pin files (`.nvmrc`, `rust-toolchain.toml`, …), so upgrading a
/// tool or switching projects invalidates the entry. Shims get a short TTL on top,
/// since they pick a version from places we don't watch.
pub struct VersionCache {
    path: Option<PathBuf>,
    entries: Mutex<Option<HashMap<String, (String, u64)>>>,
    dirty: AtomicBool,
}

const TTL_EXACT: u64 = 7 * 24 * 3600;
const TTL_SHIM: u64 = 3600;

impl VersionCache {
    pub fn new(path: Option<PathBuf>) -> VersionCache {
        VersionCache { path, entries: Mutex::new(None), dirty: AtomicBool::new(false) }
    }

    fn with<R>(&self, f: impl FnOnce(&mut HashMap<String, (String, u64)>) -> R) -> R {
        let mut guard = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let map = guard.get_or_insert_with(|| {
            let mut m = HashMap::new();
            if let Some(text) = self.path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()) {
                for line in text.lines() {
                    let mut it = line.splitn(3, '\t');
                    if let (Some(k), Some(t), Some(v)) = (it.next(), it.next(), it.next())
                        && let Ok(t) = t.parse()
                    {
                        m.insert(k.to_string(), (v.to_string(), t));
                    }
                }
            }
            m
        });
        f(map)
    }

    pub fn get(&self, key: &str, shim: bool) -> Option<String> {
        let ttl = if shim { TTL_SHIM } else { TTL_EXACT };
        self.with(|m| m.get(key).filter(|(_, t)| now_secs().saturating_sub(*t) < ttl).map(|(v, _)| v.clone()))
    }

    pub fn put(&self, key: String, value: String) {
        self.with(|m| m.insert(key, (value, now_secs())));
        self.dirty.store(true, Ordering::Relaxed);
    }

    pub fn save(&self) {
        let (Some(path), true) = (&self.path, self.dirty.load(Ordering::Relaxed)) else { return };
        let text = self.with(|m| {
            let cutoff = now_secs().saturating_sub(TTL_EXACT);
            m.iter()
                .filter(|(_, (_, t))| *t > cutoff)
                .map(|(k, (v, t))| format!("{}\t{t}\t{}\n", k.replace(['\t', '\n'], " "), v.replace(['\t', '\n'], " ")))
                .collect::<String>()
        });
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = path.with_extension(format!("tmp{}", std::process::id()));
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, path);
        }
    }
}

/// Splits `1.2.3-rc1` into the variables `version_format` can use.
pub fn version_vars(raw: &str) -> [(&'static str, String); 4] {
    let mut parts = raw.split(['.', '-', '+']);
    [
        ("raw", raw.to_string()),
        ("major", parts.next().unwrap_or("").to_string()),
        ("minor", parts.next().unwrap_or("").to_string()),
        ("patch", parts.next().unwrap_or("").to_string()),
    ]
}

/// The first thing in `text` that looks like a version number (`1.2.3`, `v26.10.0`).
pub fn find_version(text: &str) -> Option<String> {
    text.split(|c: char| c.is_whitespace() || c == '"' || c == '(' || c == ',')
        .map(|w| w.trim_start_matches(['v', 'V']).trim_start_matches("go"))
        .find(|w| w.chars().next().is_some_and(|c| c.is_ascii_digit()) && w.contains('.'))
        .map(|w| w.trim_end_matches(['.', ':', ';']).to_string())
}

/// Human duration the way starship prints it: `4s`, `1m2s`, `1h0m0s`, `350ms`.
pub fn humanize_ms(ms: u64, show_ms: bool) -> String {
    let (d, h, m, s, rest) = (ms / 86_400_000, ms / 3_600_000 % 24, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000);
    let mut out = String::new();
    let mut started = false;
    for (v, unit) in [(d, "d"), (h, "h"), (m, "m"), (s, "s")] {
        if v > 0 || started {
            out += &format!("{v}{unit}");
            started = true;
        }
    }
    if show_ms || out.is_empty() {
        out += &format!("{rest}ms");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(find_version("Python 3.15.1").as_deref(), Some("3.15.1"));
        assert_eq!(find_version("v26.10.0").as_deref(), Some("26.10.0"));
        assert_eq!(find_version("go version go1.26.3 darwin/arm64").as_deref(), Some("1.26.3"));
        assert_eq!(find_version("rustc 1.95.0 (abc 2026-01-01)").as_deref(), Some("1.95.0"));
        assert_eq!(find_version("openjdk version \"21.0.2\" 2024-01-16").as_deref(), Some("21.0.2"));
        assert_eq!(find_version("Lua 5.4.7  Copyright (C) 1994-2024").as_deref(), Some("5.4.7"));
        assert_eq!(version_vars("1.2.3")[2].1, "2");
    }

    #[test]
    fn durations() {
        assert_eq!(humanize_ms(4242, false), "4s");
        assert_eq!(humanize_ms(4242, true), "4s242ms");
        assert_eq!(humanize_ms(62_000, false), "1m2s");
        assert_eq!(humanize_ms(3_600_000, false), "1h0m0s");
        assert_eq!(humanize_ms(350, false), "350ms");
    }
}
