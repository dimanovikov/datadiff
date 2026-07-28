//! Semantic tree diff.

use crate::value::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Removed,
    Modified,
    Unchanged,
}

#[derive(Debug, Clone)]
pub struct Change {
    pub kind: ChangeKind,
    /// Data path, e.g. `services.web.replicas` or `users[id=4217].email`.
    pub path: String,
    pub old: Option<Value>,
    pub new: Option<Value>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub added: usize,
    pub removed: usize,
    pub modified: usize,
}

impl Summary {
    pub fn total(&self) -> usize {
        self.added + self.removed + self.modified
    }
}

/// Compute the diff between two value trees. When `key` is set, arrays whose
/// elements are all objects carrying that field are matched by key value
/// instead of by index.
pub fn diff(old: &Value, new: &Value, key: Option<&str>) -> Vec<Change> {
    let mut changes = Vec::new();
    diff_value(old, new, "$", key, &mut changes);
    changes
}

pub fn summarize(changes: &[Change]) -> Summary {
    let mut s = Summary::default();
    for c in changes {
        match c.kind {
            ChangeKind::Added => s.added += 1,
            ChangeKind::Removed => s.removed += 1,
            ChangeKind::Modified => s.modified += 1,
            ChangeKind::Unchanged => {}
        }
    }
    s
}

fn diff_value(old: &Value, new: &Value, path: &str, key: Option<&str>, out: &mut Vec<Change>) {
    match (old, new) {
        (Value::Object(a), Value::Object(b)) => diff_objects(a, b, path, key, out),
        (Value::Array(a), Value::Array(b)) => diff_arrays(a, b, path, key, out),
        _ => {
            if old.semantically_eq(new) {
                out.push(Change {
                    kind: ChangeKind::Unchanged,
                    path: path.to_string(),
                    old: Some(old.clone()),
                    new: Some(new.clone()),
                });
            } else {
                out.push(Change {
                    kind: ChangeKind::Modified,
                    path: path.to_string(),
                    old: Some(old.clone()),
                    new: Some(new.clone()),
                });
            }
        }
    }
}

fn diff_objects(
    a: &std::collections::BTreeMap<String, Value>,
    b: &std::collections::BTreeMap<String, Value>,
    path: &str,
    key: Option<&str>,
    out: &mut Vec<Change>,
) {
    for (k, v) in a {
        let child = join_path(path, k);
        match b.get(k) {
            Some(nv) => diff_value(v, nv, &child, key, out),
            None => out.push(Change {
                kind: ChangeKind::Removed,
                path: child,
                old: Some(v.clone()),
                new: None,
            }),
        }
    }
    for (k, v) in b {
        if !a.contains_key(k) {
            out.push(Change {
                kind: ChangeKind::Added,
                path: join_path(path, k),
                old: None,
                new: Some(v.clone()),
            });
        }
    }
}

fn diff_arrays(a: &[Value], b: &[Value], path: &str, key: Option<&str>, out: &mut Vec<Change>) {
    if let Some(k) = key {
        if let (Some(ma), Some(mb)) = (key_map(a, k), key_map(b, k)) {
            return diff_arrays_by_key(a, b, &ma, &mb, path, k, out);
        }
    }
    // Index-based matching.
    let common = a.len().min(b.len());
    for i in 0..common {
        diff_value(&a[i], &b[i], &format!("{path}[{i}]"), key, out);
    }
    for (i, v) in a.iter().enumerate().skip(common) {
        out.push(Change {
            kind: ChangeKind::Removed,
            path: format!("{path}[{i}]"),
            old: Some(v.clone()),
            new: None,
        });
    }
    for (i, v) in b.iter().enumerate().skip(common) {
        out.push(Change {
            kind: ChangeKind::Added,
            path: format!("{path}[{i}]"),
            old: None,
            new: Some(v.clone()),
        });
    }
}

/// Build `key value -> index` for an array. Returns `None` (fall back to
/// index matching) if any element is not an object, lacks the key field, the
/// key value is not a scalar, or key values are duplicated.
fn key_map(arr: &[Value], key: &str) -> Option<std::collections::BTreeMap<String, usize>> {
    let mut map = std::collections::BTreeMap::new();
    for (i, item) in arr.iter().enumerate() {
        let Value::Object(obj) = item else {
            return None;
        };
        let kv = obj.get(key)?;
        let text = scalar_key_text(kv)?;
        if map.insert(text, i).is_some() {
            return None; // duplicate key value
        }
    }
    Some(map)
}

/// Render a scalar key value for use in paths (`users[id=4217]`).
fn scalar_key_text(v: &Value) -> Option<String> {
    match v {
        Value::Null => Some("null".to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_json_string()),
        Value::String(s) => Some(s.clone()),
        Value::Array(_) | Value::Object(_) => None,
    }
}

fn diff_arrays_by_key(
    a: &[Value],
    b: &[Value],
    ma: &std::collections::BTreeMap<String, usize>,
    mb: &std::collections::BTreeMap<String, usize>,
    path: &str,
    key: &str,
    out: &mut Vec<Change>,
) {
    // Matched and added entries follow the new array's order.
    for (k, &j) in mb {
        let child = format!("{path}[{key}={k}]");
        match ma.get(k) {
            Some(&i) => diff_value(&a[i], &b[j], &child, Some(key), out),
            None => out.push(Change {
                kind: ChangeKind::Added,
                path: child,
                old: None,
                new: Some(b[j].clone()),
            }),
        }
    }
    // Removed entries follow the old array's order.
    for (k, &i) in ma {
        if !mb.contains_key(k) {
            out.push(Change {
                kind: ChangeKind::Removed,
                path: format!("{path}[{key}={k}]"),
                old: Some(a[i].clone()),
                new: None,
            });
        }
    }
}

fn join_path(base: &str, segment: &str) -> String {
    if base == "$" {
        segment.to_string()
    } else {
        format!("{base}.{segment}")
    }
}
