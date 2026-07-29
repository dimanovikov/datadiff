//! Path-pattern matching for `--fail-on` risk policies.

/// Does `pattern` select `path`? A pattern matches when it equals the path,
/// is a dot-segment prefix of it (`spec` matches `spec.replicas`), or its
/// segments contain `*` globs (`*.image`, `containers[name=*].image`).
pub fn matches(pattern: &str, path: &str) -> bool {
    let pat: Vec<&str> = pattern.split('.').collect();
    let pth: Vec<&str> = path.split('.').collect();
    if pat.len() > pth.len() {
        return false;
    }
    pat.iter()
        .zip(pth.iter())
        .all(|(p, s)| segment_matches(p, s))
}

fn segment_matches(pattern: &str, segment: &str) -> bool {
    if !pattern.contains('*') {
        return pattern == segment;
    }
    glob_matches(pattern.as_bytes(), segment.as_bytes())
}

/// Glob where `*` matches any run of characters (no other wildcards).
fn glob_matches(pat: &[u8], s: &[u8]) -> bool {
    let (mut p, mut t) = (0, 0);
    let (mut star, mut mark) = (usize::MAX, 0);
    while t < s.len() {
        if p < pat.len() && pat[p] == s[t] {
            p += 1;
            t += 1;
        } else if p < pat.len() && pat[p] == b'*' {
            star = p;
            mark = t;
            p += 1;
        } else if star != usize::MAX {
            p = star + 1;
            mark += 1;
            t = mark;
        } else {
            return false;
        }
    }
    while p < pat.len() && pat[p] == b'*' {
        p += 1;
    }
    p == pat.len()
}
