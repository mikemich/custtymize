//! Renders the working config with the prompt engine on a background thread, inside the
//! sandbox scenarios.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use toml_edit::DocumentMut;

use crate::engine::{self, Context, Shell};
use crate::error::{Error, Result};
use crate::sandbox::{Sandbox, Scenario};

const DEBOUNCE: Duration = Duration::from_millis(60);
/// Shown by `cmd_duration` so it has something to display.
const FAKE_DURATION_MS: u64 = 4242;

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
    pub sandbox: Option<Sandbox>,
}

impl Runner {
    pub fn spawn(env: Env) -> Runner {
        let (tx, req_rx) = mpsc::channel::<Request>();
        let (out_tx, rx) = mpsc::channel::<Outcome>();
        thread::spawn(move || {
            while let Ok(mut req) = req_rx.recv() {
                // Debounce: keep taking newer requests until things go quiet.
                loop {
                    match req_rx.recv_timeout(DEBOUNCE) {
                        Ok(newer) => req = newer,
                        Err(RecvTimeoutError::Timeout) => break,
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
                if out_tx.send(render_all(&env, &req)).is_err() {
                    return;
                }
            }
        });
        Runner { tx, rx }
    }

    pub fn request(&self, req: Request) {
        // A send error means the worker died; the UI shows the preview as stale.
        let _ = self.tx.send(req);
    }
}

/// The engine context for a scenario: its directory and environment, with the user's
/// own ssh / virtualenv state masked so the preview shows what the scenario says.
fn context(env: &Env, scenario: Scenario, status: i32, width: u16, doc: &DocumentMut) -> Context {
    let (cwd, extra) = match &env.sandbox {
        Some(sb) => sb.context(scenario),
        None => (std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")), vec![]),
    };
    let mut ctx = Context::new(cwd);
    ctx.use_version_cache_file();
    for k in ["SSH_CONNECTION", "SSH_TTY", "SSH_CLIENT", "VIRTUAL_ENV"] {
        ctx.set_env(k, None);
    }
    for (k, v) in extra {
        ctx.set_env(k, Some(v));
    }
    ctx.status = status;
    ctx.duration_ms = Some(FAKE_DURATION_MS);
    ctx.width = width;
    ctx.timeout = engine::config_timeout(doc);
    ctx
}

fn render_all(env: &Env, req: &Request) -> Outcome {
    let start = Instant::now();
    let mut out = Outcome {
        id: req.id,
        success: PromptOutput::default(),
        failure: PromptOutput::default(),
        elapsed: Duration::ZERO,
        warnings: Vec::new(),
        error: None,
    };
    let doc: DocumentMut = match req.toml.parse() {
        Ok(d) => d,
        Err(e) => {
            out.error = Some(format!("invalid TOML: {e}"));
            return out;
        }
    };
    let (errors, warnings) = engine::validate(&doc);
    out.error = errors.into_iter().next();
    out.warnings = warnings;
    // Success and failure render in parallel (each runs its modules in parallel too).
    let (ok, failed) = thread::scope(|s| {
        let a = s.spawn(|| engine::render(&doc, &context(env, req.scenario, 0, req.width, &doc), Shell::None));
        let b = engine::render(&doc, &context(env, req.scenario, 1, req.width, &doc), Shell::None);
        (a.join().unwrap_or_default(), b)
    });
    for w in ok.warnings.iter().chain(&failed.warnings) {
        if !out.warnings.contains(w) {
            out.warnings.push(w.clone());
        }
    }
    out.success = PromptOutput { left: ok.left, right: ok.right };
    out.failure = PromptOutput { left: failed.left, right: failed.right };
    out.elapsed = start.elapsed();
    out
}

/// Renders a config once, on the calling thread.
pub fn render_sync(env: &Env, toml: &str, scenario: Scenario, width: u16) -> Outcome {
    render_all(env, &Request { id: 0, toml: toml.to_string(), scenario, width })
}

/// Validates TOML text: it must parse and every format string must be valid. Returns
/// warnings (e.g. modules the engine doesn't implement) on success.
pub fn validate(toml: &str) -> Result<Vec<String>> {
    let doc: DocumentMut = toml.parse().map_err(|e: toml_edit::TomlError| Error::InvalidToml(e.to_string()))?;
    let (errors, warnings) = engine::validate(&doc);
    match errors.into_iter().next() {
        Some(e) => Err(Error::Rejected(e)),
        None => Ok(warnings),
    }
}
