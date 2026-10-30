//! `git_branch` and `git_status`. The branch comes straight from `.git/HEAD` (no
//! process); the status is one `git status --porcelain=v2 --branch` call.

use std::path::{Path, PathBuf};

use super::super::format::Vars;
use super::super::style::Seg;
use super::super::{Cfg, Context, util};
use super::{finish, sub_format};

#[derive(Debug, Clone)]
pub struct Repo {
    pub root: PathBuf,
    pub git_dir: PathBuf,
    pub common_dir: PathBuf,
}

fn discover(start: &Path) -> Option<Repo> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        let dot = d.join(".git");
        if let Ok(meta) = std::fs::metadata(&dot) {
            let git_dir = if meta.is_dir() {
                dot
            } else {
                // Worktrees and submodules: `.git` is a file saying `gitdir: <path>`.
                let text = std::fs::read_to_string(&dot).ok()?;
                let p = PathBuf::from(text.trim().strip_prefix("gitdir:")?.trim());
                if p.is_absolute() { p } else { d.join(p) }
            };
            let common_dir = std::fs::read_to_string(git_dir.join("commondir"))
                .ok()
                .map(|c| git_dir.join(c.trim()))
                .unwrap_or_else(|| git_dir.clone());
            return Some(Repo { root: d.to_path_buf(), git_dir, common_dir });
        }
        dir = d.parent();
    }
    None
}

pub fn repo(ctx: &Context) -> Option<&Repo> {
    ctx.repo.get_or_init(|| discover(&ctx.cwd)).as_ref()
}

enum Head {
    Branch(String),
    Detached,
}

fn head(r: &Repo) -> Option<Head> {
    let text = std::fs::read_to_string(r.git_dir.join("HEAD")).ok()?;
    Some(match text.trim().strip_prefix("ref: refs/heads/") {
        Some(b) => Head::Branch(b.to_string()),
        None => Head::Detached,
    })
}

/// `(remote, branch)` for a local branch, from `[branch "x"]` in `.git/config`.
fn upstream(r: &Repo, branch: &str) -> Option<(String, String)> {
    let text = std::fs::read_to_string(r.common_dir.join("config")).ok()?;
    let header = format!("[branch \"{branch}\"]");
    let (mut inside, mut remote, mut merge) = (false, None, None);
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            inside = line == header;
        } else if inside && let Some((k, v)) = line.split_once('=') {
            match k.trim() {
                "remote" => remote = Some(v.trim().to_string()),
                "merge" => merge = Some(v.trim().trim_start_matches("refs/heads/").to_string()),
                _ => {}
            }
        }
    }
    Some((remote?, merge?))
}

pub fn branch(ctx: &Context, cfg: Cfg) -> Option<Vec<Seg>> {
    let r = repo(ctx)?;
    let name = match head(r)? {
        Head::Branch(b) => b,
        Head::Detached if cfg.bool("only_attached", false) => return None,
        Head::Detached => "HEAD".to_string(),
    };
    if cfg.strs("ignore_branches", &[]).contains(&name) {
        return None;
    }
    let max = cfg.int("truncation_length", i64::MAX).max(0) as usize;
    let shown = if name.chars().count() > max {
        name.chars().take(max).collect::<String>() + &cfg.str("truncation_symbol", "…")
    } else {
        name.clone()
    };
    let mut v = Vars::new();
    v.text("branch", shown);
    if let Some((remote, rb)) = upstream(r, &name)
        && (rb != name || cfg.bool("always_show_remote", false))
    {
        v.text("remote_name", remote).text("remote_branch", rb);
    }
    finish(ctx, "git_branch", cfg, "on [$symbol$branch(:$remote_branch)]($style) ", "bold purple", "\u{e0a0} ", &mut v)
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Status {
    pub upstream: bool,
    pub ahead: u32,
    pub behind: u32,
    pub conflicted: u32,
    pub deleted: u32,
    pub renamed: u32,
    pub modified: u32,
    pub typechanged: u32,
    pub staged: u32,
    pub untracked: u32,
    pub stashed: u32,
}

pub fn parse_porcelain(text: &str) -> Status {
    let mut s = Status::default();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("# branch.ab ") {
            let mut it = rest.split_whitespace();
            s.ahead = it.next().and_then(|a| a.trim_start_matches('+').parse().ok()).unwrap_or(0);
            s.behind = it.next().and_then(|b| b.trim_start_matches('-').parse().ok()).unwrap_or(0);
        } else if line.starts_with("# branch.upstream ") {
            s.upstream = true;
        } else if line.starts_with("? ") {
            s.untracked += 1;
        } else if line.starts_with("u ") {
            s.conflicted += 1;
        } else if let Some(rest) = line.strip_prefix("1 ").or_else(|| line.strip_prefix("2 ")) {
            let mut xy = rest.chars();
            let (x, y) = (xy.next().unwrap_or('.'), xy.next().unwrap_or('.'));
            if x == 'R' {
                s.renamed += 1;
            } else if x != '.' {
                s.staged += 1;
            }
            if x == 'D' || y == 'D' {
                s.deleted += 1;
            }
            if y == 'M' {
                s.modified += 1;
            }
            if y == 'T' {
                s.typechanged += 1;
            }
        }
    }
    s
}

fn stash_count(r: &Repo) -> u32 {
    std::fs::read_to_string(r.common_dir.join("logs/refs/stash")).map(|t| t.lines().count() as u32).unwrap_or(0)
}

fn load_status(ctx: &Context) -> Option<Status> {
    let r = repo(ctx)?;
    let git = util::which(ctx, "git")?;
    let out = util::run(ctx, &git, &["--no-optional-locks", "status", "--porcelain=v2", "--branch"])?;
    if !out.ok {
        ctx.warn(format!("git status: {}", out.stderr.trim()));
        return None;
    }
    let mut s = parse_porcelain(&out.stdout);
    s.stashed = stash_count(r);
    Some(s)
}

pub fn status(ctx: &Context, cfg: Cfg) -> Option<Vec<Seg>> {
    let s = ctx.git_status.get_or_init(|| load_status(ctx)).as_ref()?;
    let piece = |key: &str, default: &str, count: u32| -> Vec<Seg> {
        if count == 0 {
            return Vec::new();
        }
        let mut cv = Vars::new();
        cv.text("count", count.to_string());
        sub_format(ctx, "git_status", &cfg.str(key, default), &cv)
    };
    let mut v = Vars::new();
    for (key, default, n) in [
        ("conflicted", "=", s.conflicted),
        ("stashed", "\\$", s.stashed),
        ("deleted", "✘", s.deleted),
        ("renamed", "»", s.renamed),
        ("modified", "!", s.modified),
        ("typechanged", "", s.typechanged),
        ("staged", "+", s.staged),
        ("untracked", "?", s.untracked),
    ] {
        v.segs(key, piece(key, default, n));
    }
    let ab = if s.ahead > 0 && s.behind > 0 {
        let mut dv = Vars::new();
        dv.text("ahead_count", s.ahead.to_string()).text("behind_count", s.behind.to_string());
        sub_format(ctx, "git_status", &cfg.str("diverged", "⇕"), &dv)
    } else if s.ahead > 0 {
        piece("ahead", "⇡", s.ahead)
    } else if s.behind > 0 {
        piece("behind", "⇣", s.behind)
    } else if s.upstream {
        sub_format(ctx, "git_status", &cfg.str("up_to_date", ""), &Vars::new())
    } else {
        Vec::new()
    };
    v.segs("ahead_behind", ab);
    let all_fmt = "$conflicted$stashed$deleted$renamed$modified$typechanged$staged$untracked";
    let all = sub_format(ctx, "git_status", &cfg.str("all_status", all_fmt), &v);
    v.segs("all_status", all);
    finish(ctx, "git_status", cfg, "([\\[$all_status$ahead_behind\\]]($style) )", "bold red", "", &mut v)
}

/// The commit HEAD points at (following a symbolic ref through loose refs and
/// packed-refs).
fn head_sha(r: &Repo) -> Option<String> {
    let head = std::fs::read_to_string(r.git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    let Some(refname) = head.strip_prefix("ref: ") else { return Some(head.to_string()) };
    if let Ok(sha) = std::fs::read_to_string(r.common_dir.join(refname)) {
        return Some(sha.trim().to_string());
    }
    let packed = std::fs::read_to_string(r.common_dir.join("packed-refs")).ok()?;
    packed.lines().find_map(|l| l.strip_suffix(refname).map(|s| s.trim().to_string()))
}

/// `git_commit`: the short hash, by default only when HEAD is detached.
pub fn commit(ctx: &Context, cfg: Cfg) -> Option<Vec<Seg>> {
    let r = repo(ctx)?;
    let detached = matches!(head(r)?, Head::Detached);
    if cfg.bool("only_detached", true) && !detached {
        return None;
    }
    let sha = head_sha(r)?;
    let len = cfg.int("commit_hash_length", 7).clamp(1, 40) as usize;
    let mut v = Vars::new();
    v.text("hash", sha.chars().take(len).collect::<String>()).text("tag", "");
    finish(ctx, "git_commit", cfg, "[\\($hash$tag\\)]($style) ", "green bold", "", &mut v)
}

/// `git_state`: an in-progress rebase, merge, cherry-pick, revert or bisect.
pub fn state(ctx: &Context, cfg: Cfg) -> Option<Vec<Seg>> {
    let r = repo(ctx)?;
    let g = &r.git_dir;
    let read_num = |p: PathBuf| std::fs::read_to_string(p).ok().map(|t| t.trim().to_string()).unwrap_or_default();
    let (state, cur, total) = if g.join("rebase-merge").is_dir() {
        let d = g.join("rebase-merge");
        (cfg.str("rebase", "REBASING"), read_num(d.join("msgnum")), read_num(d.join("end")))
    } else if g.join("rebase-apply").is_dir() {
        let d = g.join("rebase-apply");
        let label = if d.join("applying").exists() { cfg.str("am", "AM") } else { cfg.str("rebase", "REBASING") };
        (label, read_num(d.join("next")), read_num(d.join("last")))
    } else if g.join("MERGE_HEAD").exists() {
        (cfg.str("merge", "MERGING"), String::new(), String::new())
    } else if g.join("CHERRY_PICK_HEAD").exists() {
        (cfg.str("cherry_pick", "CHERRY-PICKING"), String::new(), String::new())
    } else if g.join("REVERT_HEAD").exists() {
        (cfg.str("revert", "REVERTING"), String::new(), String::new())
    } else if g.join("BISECT_LOG").exists() {
        (cfg.str("bisect", "BISECTING"), String::new(), String::new())
    } else {
        return None;
    };
    let mut v = Vars::new();
    v.text("state", state).text("progress_current", cur).text("progress_total", total);
    finish(ctx, "git_state", cfg, "\\([$state( $progress_current/$progress_total)]($style)\\) ", "bold yellow", "", &mut v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain() {
        let text = "# branch.oid abc\n# branch.head main\n# branch.upstream origin/main\n# branch.ab +2 -1\n\
1 .M N... 100644 100644 100644 a b README.md\n1 A. N... 000000 100644 100644 0 b new.txt\n\
2 R. N... 100644 100644 100644 a b R100 new\told\n1 D. N... 100644 000000 000000 a 0 gone\n\
u UU N... 1 2 3 4 h1 h2 h3 x\n? untracked.log\n? other\n";
        let s = parse_porcelain(text);
        assert_eq!((s.ahead, s.behind, s.upstream), (2, 1, true));
        assert_eq!((s.modified, s.staged, s.renamed, s.deleted, s.conflicted, s.untracked), (1, 2, 1, 1, 1, 2));
    }
}
