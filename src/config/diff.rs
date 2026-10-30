//! A small line diff (LCS) for the install review. Configs are a few hundred lines at most.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiffLine {
    Same(String),
    Added(String),
    Removed(String),
}

pub fn lines(old: &str, new: &str) -> Vec<DiffLine> {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    // lcs[i][j] = LCS length of a[i..] and b[j..]
    let mut lcs = vec![vec![0u32; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lcs[i][j] = if a[i] == b[j] { lcs[i + 1][j + 1] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut out = Vec::new();
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            out.push(DiffLine::Same(a[i].to_string()));
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            out.push(DiffLine::Removed(a[i].to_string()));
            i += 1;
        } else {
            out.push(DiffLine::Added(b[j].to_string()));
            j += 1;
        }
    }
    out.extend(a[i..].iter().map(|l| DiffLine::Removed(l.to_string())));
    out.extend(b[j..].iter().map(|l| DiffLine::Added(l.to_string())));
    out
}

/// (added, removed)
pub fn stats(d: &[DiffLine]) -> (usize, usize) {
    d.iter().fold((0, 0), |(a, r), l| match l {
        DiffLine::Added(_) => (a + 1, r),
        DiffLine::Removed(_) => (a, r + 1),
        DiffLine::Same(_) => (a, r),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_marks_changes() {
        let d = lines("a\nb\nc\n", "a\nB\nc\nd\n");
        assert_eq!(
            d,
            vec![
                DiffLine::Same("a".into()),
                DiffLine::Removed("b".into()),
                DiffLine::Added("B".into()),
                DiffLine::Same("c".into()),
                DiffLine::Added("d".into()),
            ]
        );
        assert_eq!(stats(&d), (2, 1));
        assert_eq!(stats(&lines("x\n", "x\n")), (0, 0));
    }
}
