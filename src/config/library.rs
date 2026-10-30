//! Saved configs: named snapshots in `<config dir>/saved/<name>.toml`.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub modified: String,
}

pub fn dir(config_path: &Path) -> PathBuf {
    config_path.parent().map(|p| p.join("saved")).unwrap_or_else(|| PathBuf::from("saved"))
}

/// Turns a user-typed name into a safe file stem: letters, digits, `-`, `_`, `.`.
pub fn sanitize(name: &str) -> Option<String> {
    let s: String = name
        .trim()
        .chars()
        .map(|c| if c.is_whitespace() { '-' } else { c })
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .take(48)
        .collect();
    let s = s.trim_matches('.').to_string();
    (!s.is_empty()).then_some(s)
}

/// Newest first.
pub fn list(dir: &Path) -> Vec<Entry> {
    let mut out: Vec<(std::time::SystemTime, Entry)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if path.extension().and_then(|x| x.to_str()) != Some("toml") {
                return None;
            }
            let name = path.file_stem()?.to_str()?.to_string();
            let mtime = e.metadata().ok()?.modified().ok()?;
            let modified = chrono::DateTime::<chrono::Local>::from(mtime).format("%Y-%m-%d %H:%M").to_string();
            Some((mtime, Entry { name, path, modified }))
        })
        .collect();
    out.sort_by_key(|e| std::cmp::Reverse(e.0));
    out.into_iter().map(|(_, e)| e).collect()
}

/// Writes `toml` as `<name>.toml`. Refuses to replace an existing one unless `overwrite`.
pub fn save(dir: &Path, name: &str, toml: &str, overwrite: bool) -> Result<PathBuf, String> {
    let stem = sanitize(name).ok_or_else(|| format!("{name:?} isn't a usable name"))?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join(format!("{stem}.toml"));
    if path.exists() && !overwrite {
        return Err(format!("{stem} already exists (use :save! {stem} to replace it)"));
    }
    super::save::write_atomic(&path, toml).map_err(|e| e.to_string())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_list_and_refuse_overwrite() {
        let d = std::env::temp_dir().join(format!("custtymize-lib-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        assert_eq!(sanitize("  my work prompt! "), Some("my-work-prompt".into()));
        assert_eq!(sanitize("../.."), None);
        let p = save(&d, "work", "format = '$directory'\n", false).expect("save");
        assert!(p.ends_with("work.toml"));
        assert!(save(&d, "work", "x", false).is_err());
        save(&d, "work", "format = '$character'\n", true).expect("overwrite");
        save(&d, "home", "format = '$directory'\n", false).expect("save 2");
        let names: Vec<String> = list(&d).into_iter().map(|e| e.name).collect();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"work".to_string()) && names.contains(&"home".to_string()));
        assert_eq!(std::fs::read_to_string(&p).ok().as_deref(), Some("format = '$character'\n"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
