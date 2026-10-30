//! Backup-then-write for the live config.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// Follows a symlinked config (dotfiles repos) so we write the real file, not over the link.
fn resolve(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Copies the existing config to `<path>.bak-<timestamp>`. Returns the backup path, or
/// `None` when there was nothing to back up.
pub fn backup(path: &Path) -> Result<Option<PathBuf>> {
    if !path.exists() {
        return Ok(None);
    }
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let mut name = path.as_os_str().to_owned();
    name.push(format!(".bak-{stamp}"));
    let mut bak = PathBuf::from(name);
    // Two saves in the same second: don't clobber the earlier backup.
    let mut n = 1;
    while bak.exists() {
        let mut name = path.as_os_str().to_owned();
        name.push(format!(".bak-{stamp}-{n}"));
        bak = PathBuf::from(name);
        n += 1;
    }
    std::fs::copy(path, &bak).map_err(|e| Error::io(&bak, e))?;
    Ok(Some(bak))
}

/// Writes atomically: temp file in the same directory, then rename.
pub fn write_atomic(path: &Path, contents: &str) -> Result<()> {
    let target = resolve(path);
    let dir = target.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
    std::fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
    let tmp = dir.join(format!(
        ".{}.custtymize-{}",
        target.file_name().and_then(|n| n.to_str()).unwrap_or("starship.toml"),
        std::process::id()
    ));
    std::fs::write(&tmp, contents).map_err(|e| Error::io(&tmp, e))?;
    std::fs::rename(&tmp, &target).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        Error::io(&target, e)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_then_write() {
        let dir = std::env::temp_dir().join(format!("custtymize-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let cfg = dir.join("starship.toml");
        assert!(backup(&cfg).expect("no-op backup").is_none());
        std::fs::write(&cfg, "old = 1\n").expect("seed");
        let b1 = backup(&cfg).expect("backup").expect("path");
        let b2 = backup(&cfg).expect("backup").expect("path");
        assert_ne!(b1, b2);
        write_atomic(&cfg, "new = 2\n").expect("write");
        assert_eq!(std::fs::read_to_string(&cfg).expect("read"), "new = 2\n");
        assert_eq!(std::fs::read_to_string(&b1).expect("read"), "old = 1\n");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
