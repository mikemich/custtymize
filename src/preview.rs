//! Runs the real `starship prompt` against the working config on a background thread.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use crate::error::{Error, Result};
use crate::sandbox::{Sandbox, Scenario};

const DEBOUNCE: Duration = Duration::from_millis(100);
const TIMEOUT: Duration = Duration::from_secs(5);
/// Fed to `--cmd-duration` so `cmd_duration` has something to show.
const FAKE_DURATION_MS: &str = "4242";

#[derive(Clone, Debug)]
pub struct Request {
    pub id: u64,
    pub toml: String,
    pub scenario: Scenario,
    pub width: u16,
}

/// Raw ANSI output of one prompt state.
#[derive(Clone, Debug, Default)]
pub struct PromptOutput {
    pub left: String,
    pub right: String,
}

#[derive(Clone, Debug)]
pub struct Outcome {
    pub id: u64,
    pub success: PromptOutput,
    pub failure: PromptOutput,
    pub elapsed: Duration,
    pub warnings: Vec<String>,
    pub error: Option<String>,
}

pub struct Runner {
    tx: Sender<Request>,
    pub rx: Receiver<Outcome>,
}

#[derive(Clone)]
pub struct Env {
    pub starship: PathBuf,
    pub sandbox: Option<Sandbox>,
    pub work_dir: PathBuf,
}

impl Runner {
    pub fn spawn(env: Env) -> Runner {
        let (tx, req_rx) = mpsc::channel::<Request>();
        let (out_tx, rx) = mpsc::channel::<Outcome>();
        thread::spawn(move || {
            let cfg = env.work_dir.join(format!("preview-{}.toml", std::process::id()));
            while let Ok(mut req) = req_rx.recv() {
                // Debounce: keep taking newer requests until things go quiet.
                loop {
                    match req_rx.recv_timeout(DEBOUNCE) {
                        Ok(newer) => req = newer,
                        Err(RecvTimeoutError::Timeout) => break,
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
                let outcome = render_all(&env, &cfg, &req);
                if out_tx.send(outcome).is_err() {
                    return;
                }
            }
            let _ = std::fs::remove_file(&cfg);
        });
        Runner { tx, rx }
    }

    pub fn request(&self, req: Request) {
        // A send error means the worker died; the UI shows the preview as stale.
        let _ = self.tx.send(req);
    }
}

fn render_all(env: &Env, cfg: &Path, req: &Request) -> Outcome {
    let start = Instant::now();
    let mut out = Outcome {
        id: req.id,
        success: PromptOutput::default(),
        failure: PromptOutput::default(),
        elapsed: Duration::ZERO,
        warnings: Vec::new(),
        error: None,
    };
    if let Err(e) = std::fs::write(cfg, &req.toml) {
        out.error = Some(Error::io(cfg, e).to_string());
        return out;
    }
    let (cwd, extra) = match &env.sandbox {
        Some(sb) => sb.context(req.scenario),
        None => (std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")), vec![]),
    };
    // Four starship invocations in parallel: {success, failure} × {left, right}.
    let jobs = [(0, false), (1, false), (0, true), (1, true)];
    let results: Vec<Result<Run>> = thread::scope(|s| {
        let handles: Vec<_> = jobs
            .iter()
            .map(|&(status, right)| {
                let cwd = &cwd;
                let extra = &extra;
                s.spawn(move || {
                    run_prompt(&env.starship, cfg, cwd, extra, &env.work_dir, status, right, req.width)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_else(|_| Err(Error::Starship("preview thread panicked".into()))))
            .collect()
    });
    for (res, &(status, right)) in results.into_iter().zip(jobs.iter()) {
        match res {
            Ok(run) => {
                for w in stderr_blocks(&run.stderr) {
                    if !out.warnings.contains(&w) {
                        out.warnings.push(w);
                    }
                }
                let slot = if status == 0 { &mut out.success } else { &mut out.failure };
                if right { slot.right = run.stdout } else { slot.left = run.stdout }
            }
            Err(e) => {
                out.error.get_or_insert_with(|| e.to_string());
            }
        }
    }
    out.elapsed = start.elapsed();
    out
}

pub struct Run {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
}

#[allow(clippy::too_many_arguments)]
pub fn run_prompt(
    starship: &Path,
    cfg: &Path,
    cwd: &Path,
    extra_env: &[(&str, String)],
    cache: &Path,
    status: i32,
    right: bool,
    width: u16,
) -> Result<Run> {
    let mut cmd = Command::new(starship);
    cmd.arg("prompt")
        .arg(format!("--status={status}"))
        .arg(format!("--cmd-duration={FAKE_DURATION_MS}"))
        .arg(format!("--terminal-width={width}"))
        .arg("--jobs=0");
    if right {
        cmd.arg("--right");
    }
    cmd.current_dir(cwd)
        .env("STARSHIP_CONFIG", cfg)
        .env("PWD", cwd)
        // Keep starship's own log out of the user's ~/.cache/starship.
        .env("STARSHIP_CACHE", cache)
        // An unknown shell means no `%{ %}` zsh escape wrappers in the output.
        .env_remove("STARSHIP_SHELL")
        .env_remove("STARSHIP_SESSION_KEY")
        .env_remove("SSH_CONNECTION")
        .env_remove("SSH_TTY")
        .env_remove("SSH_CLIENT")
        .env_remove("VIRTUAL_ENV")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().map_err(|e| Error::StarshipMissing(format!("{}: {e}", starship.display())))?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() > TIMEOUT => {
                let _ = child.kill();
                return Err(Error::Starship(format!("timed out after {}s", TIMEOUT.as_secs())));
            }
            Ok(None) => thread::sleep(Duration::from_millis(4)),
            Err(e) => return Err(Error::Starship(e.to_string())),
        }
    }
    let output = child.wait_with_output().map_err(|e| Error::Starship(e.to_string()))?;
    Ok(Run {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        code: output.status.code().unwrap_or(-1),
    })
}

/// Locates the starship binary on PATH (or `$STARSHIP_BIN`).
pub fn find_starship() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("STARSHIP_BIN") {
        return Some(PathBuf::from(p));
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .chain([PathBuf::from("/opt/homebrew/bin"), PathBuf::from("/usr/local/bin")])
        .map(|d| d.join("starship"))
        .find(|p| p.is_file())
}

pub fn starship_version(bin: &Path) -> Option<String> {
    let out = Command::new(bin).arg("--version").output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines().next().map(|l| l.trim_start_matches("starship ").trim().to_string())
}

/// Renders a config once, on the calling thread. `tag` keeps concurrent callers' temp
/// files apart (e.g. "before" and "after" in the install review).
pub fn render_sync(env: &Env, toml: &str, scenario: Scenario, width: u16, tag: &str) -> Outcome {
    let cfg = env.work_dir.join(format!("{tag}-{}.toml", std::process::id()));
    let out = render_all(env, &cfg, &Request { id: 0, toml: toml.to_string(), scenario, width });
    let _ = std::fs::remove_file(&cfg);
    out
}

/// Validates TOML text by running starship against it once. Returns warnings on success.
pub fn validate(env: &Env, toml: &str) -> Result<Vec<String>> {
    toml.parse::<toml_edit::DocumentMut>().map_err(|e| Error::InvalidToml(e.to_string()))?;
    let cfg = env.work_dir.join(format!("validate-{}.toml", std::process::id()));
    std::fs::write(&cfg, toml).map_err(|e| Error::io(&cfg, e))?;
    let (cwd, extra) = match &env.sandbox {
        Some(sb) => sb.context(Scenario::DirtyRepo),
        None => (std::env::temp_dir(), vec![]),
    };
    let run = run_prompt(&env.starship, &cfg, &cwd, &extra, &env.work_dir, 0, false, 120);
    let _ = std::fs::remove_file(&cfg);
    let run = run?;
    let blocks = stderr_blocks(&run.stderr);
    if run.code != 0 {
        return Err(Error::Starship(format!("exit code {}: {}", run.code, blocks.join(" | "))));
    }
    // starship exits 0 even for a broken config; the problems are WARN/ERROR log blocks.
    if let Some(e) = blocks.iter().find(|b| is_fatal(b)) {
        return Err(Error::Starship(e.clone()));
    }
    Ok(blocks)
}

/// Config schema errors and unparsable module formats; timeouts and the like are not fatal.
pub fn is_fatal(block: &str) -> bool {
    block.starts_with("[ERROR]")
        || block.contains("(starship::config)")
        || block.contains("Error in module")
        || block.contains("Error parsing")
}

/// Groups starship's stderr into one string per log record (records span several lines
/// for format parse errors), with ANSI codes removed.
pub fn stderr_blocks(stderr: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in stderr.lines().map(strip_ansi) {
        if line.trim().is_empty() {
            continue;
        }
        let starts_record = line.starts_with('[') && line.contains("] - (");
        match out.last_mut() {
            Some(last) if !starts_record => {
                last.push(' ');
                last.push_str(line.trim().trim_start_matches(['|', '-', '>', ' ']).trim());
            }
            _ => out.push(line.trim().to_string()),
        }
    }
    out
}

pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_multiline_records() {
        let err = "[\u{1b}[33mWARN\u{1b}[0m] - (starship::modules::directory): Error in module `directory`:\n --> 1:8\n  |\n1 | [$path(\n  |        ^---\n  = expected variable\n[\u{1b}[33mWARN\u{1b}[0m] - (starship::utils): Executing command \"node\" timed out.\n";
        let b = stderr_blocks(err);
        assert_eq!(b.len(), 2, "{b:?}");
        assert!(b[0].starts_with("[WARN] - (starship::modules::directory)"));
        assert!(b[0].contains("expected variable"));
        assert!(is_fatal(&b[0]));
        assert!(!is_fatal(&b[1]));
        assert!(is_fatal("[WARN] - (starship::config): Error in 'Username' at 'style': Unknown key"));
    }
}
