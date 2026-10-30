//! Throwaway directories the preview renders the prompt inside, so git, language
//! and path modules have something to show.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Error, Result};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scenario {
    DirtyRepo,
    CleanRepo,
    DeepPath,
    Ssh,
    Home,
}

impl Scenario {
    pub const ALL: [Scenario; 5] =
        [Scenario::DirtyRepo, Scenario::CleanRepo, Scenario::DeepPath, Scenario::Ssh, Scenario::Home];

    pub fn label(&self) -> &'static str {
        match self {
            Scenario::DirtyRepo => "dirty repo",
            Scenario::CleanRepo => "clean repo",
            Scenario::DeepPath => "deep path",
            Scenario::Ssh => "ssh session",
            Scenario::Home => "home dir",
        }
    }

    pub fn next(&self) -> Scenario {
        let i = Self::ALL.iter().position(|s| s == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    pub fn parse(s: &str) -> Option<Scenario> {
        let s = s.to_ascii_lowercase();
        Self::ALL.iter().copied().find(|sc| sc.label().starts_with(&s) || sc.label().replace(' ', "-") == s)
    }
}

#[derive(Clone, Debug)]
pub struct Sandbox {
    pub root: PathBuf,
    pub dirty: PathBuf,
    pub clean: PathBuf,
    pub deep: PathBuf,
    pub home: PathBuf,
}

impl Sandbox {
    /// The working directory and extra environment for a scenario.
    pub fn context(&self, sc: Scenario) -> (PathBuf, Vec<(&'static str, String)>) {
        let venv = ("VIRTUAL_ENV", self.root.join("neon-venv").display().to_string());
        match sc {
            Scenario::DirtyRepo => (self.dirty.clone(), vec![venv]),
            Scenario::CleanRepo => (self.clean.clone(), vec![]),
            Scenario::DeepPath => (self.deep.clone(), vec![venv]),
            Scenario::Ssh => (
                self.dirty.clone(),
                vec![("SSH_CONNECTION", "10.13.37.1 51337 10.13.37.2 22".into()), ("SSH_TTY", "/dev/ttys013".into())],
            ),
            Scenario::Home => (self.home.clone(), vec![]),
        }
    }
}

fn write(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    std::fs::write(path, contents).map_err(|e| Error::io(path, e))
}

fn git(dir: &Path, args: &[&str]) -> Result<()> {
    let out = Command::new("git")
        .args(["-c", "user.name=custtymize", "-c", "user.email=custtymize@localhost", "-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| Error::io(dir, e))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(Error::Msg(format!("git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim())))
    }
}

pub fn cache_dir() -> Result<PathBuf> {
    let base = dirs::cache_dir().or_else(|| dirs::home_dir().map(|h| h.join(".cache"))).ok_or(Error::NoHome)?;
    let dir = base.join("custtymize");
    std::fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
    Ok(dir)
}

/// Creates (once) and returns the sandbox. Cheap after the first run.
pub fn ensure() -> Result<Sandbox> {
    let root = cache_dir()?.join("playground");
    let dirty = root.join("neon-city");
    let clean = root.join("clean-room");
    let deep = dirty.join("src/core/engine/warp-drive");
    let home = dirs::home_dir().ok_or(Error::NoHome)?;
    let sb = Sandbox { root: root.clone(), dirty: dirty.clone(), clean: clean.clone(), deep: deep.clone(), home };

    let marker = root.join(".ready-v1");
    if marker.exists() {
        return Ok(sb);
    }
    if root.exists() {
        std::fs::remove_dir_all(&root).map_err(|e| Error::io(&root, e))?;
    }

    write(&dirty.join("Cargo.toml"), "[package]\nname = \"neon-city\"\nversion = \"0.13.37\"\nedition = \"2024\"\n")?;
    write(&dirty.join("package.json"), "{ \"name\": \"neon-city\", \"version\": \"0.13.37\" }\n")?;
    write(&dirty.join("go.mod"), "module neon.city/grid\n\ngo 1.23\n")?;
    write(&dirty.join("requirements.txt"), "rich\n")?;
    write(&dirty.join("README.md"), "# neon city\n")?;
    write(&deep.join("core.rs"), "fn main() {}\n")?;
    git(&dirty, &["init", "-q", "-b", "feature/neon-dreams"])?;
    git(&dirty, &["add", "-A"])?;
    git(&dirty, &["commit", "-q", "-m", "jack in"])?;
    // Leave it dirty: one modified, one staged, one untracked.
    write(&dirty.join("README.md"), "# neon city\n\nnow with more neon\n")?;
    write(&dirty.join("staged.txt"), "staged\n")?;
    git(&dirty, &["add", "staged.txt"])?;
    write(&dirty.join("untracked.log"), "?\n")?;

    write(&clean.join("Cargo.toml"), "[package]\nname = \"clean-room\"\nversion = \"1.0.0\"\nedition = \"2024\"\n")?;
    git(&clean, &["init", "-q", "-b", "main"])?;
    git(&clean, &["add", "-A"])?;
    git(&clean, &["commit", "-q", "-m", "pristine"])?;

    std::fs::create_dir_all(root.join("neon-venv")).map_err(|e| Error::io(&root, e))?;
    write(&marker, "ok\n")?;
    Ok(sb)
}
