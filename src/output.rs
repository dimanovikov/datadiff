//! Diff rendering: colored line-oriented text and machine-readable JSON.

use colored::Colorize;

use crate::diff::{Change, ChangeKind, Summary};
use crate::value::Value;

/// Output format selected by `--output`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum OutputFormat {
    /// Human-readable colored lines (default).
    #[default]
    Text,
    /// Machine-readable JSON document.
    Json,
    /// JSON Patch (RFC 6902) document, for external patch tooling.
    Patch,
}

/// Render one change line. Unchanged entries are skipped unless
/// `show_unchanged` is set. Returns `None` for skipped entries.
pub fn render_change(change: &Change, show_unchanged: bool, color: bool) -> Option<String> {
    let line = match change.kind {
        ChangeKind::Modified => {
            let old = change
                .old
                .as_ref()
                .map(|v| v.to_json_string())
                .unwrap_or_default();
            let new = change
                .new
                .as_ref()
                .map(|v| v.to_json_string())
                .unwrap_or_default();
            format!("~ {}: {old} → {new}", change.path)
                .yellow()
                .to_string()
        }
        ChangeKind::Added => {
            let new = change
                .new
                .as_ref()
                .map(|v| v.to_json_string())
                .unwrap_or_default();
            format!("+ {}: {new}", change.path).green().to_string()
        }
        ChangeKind::Removed => {
            let old = change
                .old
                .as_ref()
                .map(|v| v.to_json_string())
                .unwrap_or_default();
            format!("- {}: {old}", change.path).red().to_string()
        }
        ChangeKind::Unchanged => {
            if !show_unchanged {
                return None;
            }
            format!("= {}", change.path).dimmed().to_string()
        }
    };
    let _ = color; // colors are controlled globally via colored::control
    Some(line)
}

pub fn render_summary(summary: &Summary) -> String {
    format!(
        "{} changes ({} added, {} removed, {} modified)",
        summary.total(),
        summary.added,
        summary.removed,
        summary.modified
    )
}

/// Render the whole diff as one JSON document:
/// `{ "changes": [...], "summary": { ... } }`. Unchanged entries are
/// included only when `show_unchanged` is set (mirrors the text output).
pub fn render_json(changes: &[Change], summary: &Summary, show_unchanged: bool) -> String {
    let changes_json: Vec<serde_json::Value> = changes
        .iter()
        .filter(|c| show_unchanged || c.kind != ChangeKind::Unchanged)
        .map(|c| {
            serde_json::json!({
                "type": change_kind_str(c.kind),
                "path": c.path,
                "old": c.old.as_ref().map(|v| v.to_serde_json()).unwrap_or(serde_json::Value::Null),
                "new": c.new.as_ref().map(|v| v.to_serde_json()).unwrap_or(serde_json::Value::Null),
            })
        })
        .collect();
    let doc = serde_json::json!({
        "changes": changes_json,
        "summary": {
            "added": summary.added,
            "removed": summary.removed,
            "modified": summary.modified,
            "total": summary.total(),
        },
    });
    serde_json::to_string_pretty(&doc).unwrap_or_default()
}

fn change_kind_str(kind: ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Added => "added",
        ChangeKind::Removed => "removed",
        ChangeKind::Modified => "modified",
        ChangeKind::Unchanged => "unchanged",
    }
}

/// Render the diff as a JSON Patch (RFC 6902) document. `old` is the
/// baseline tree, needed to resolve key-lookup paths (`users[id=4217]`)
/// to the numeric array indices JSON Pointer requires. Unchanged entries
/// have no RFC 6902 operation and are always skipped.
pub fn render_json_patch(changes: &[Change], old: &Value) -> String {
    let mut replaces: Vec<serde_json::Value> = Vec::new();
    let mut removes: Vec<(String, usize, serde_json::Value)> = Vec::new();
    let mut adds: Vec<serde_json::Value> = Vec::new();

    for change in changes {
        let tokens = resolve_pointer(&change.path, old);
        let pointer = tokens_to_pointer(&tokens);
        match change.kind {
            ChangeKind::Modified => replaces.push(serde_json::json!({
                "op": "replace",
                "path": pointer,
                "value": change.new.as_ref().map(Value::to_serde_json).unwrap_or(serde_json::Value::Null),
            })),
            ChangeKind::Removed => {
                // Sort key: parent pointer + descending index, so that
                // removing several elements of one array does not shift
                // the indices of the removals that follow.
                let parent = tokens_to_pointer(&tokens[..tokens.len().saturating_sub(1)]);
                let index = match tokens.last() {
                    Some(PtrToken::Index(i)) => *i,
                    _ => 0,
                };
                removes.push((parent, index, serde_json::json!({
                    "op": "remove",
                    "path": pointer,
                })));
            }
            ChangeKind::Added => adds.push(serde_json::json!({
                "op": "add",
                "path": pointer,
                "value": change.new.as_ref().map(Value::to_serde_json).unwrap_or(serde_json::Value::Null),
            })),
            ChangeKind::Unchanged => {}
        }
    }

    removes.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    let ops: Vec<serde_json::Value> = replaces
        .into_iter()
        .chain(removes.into_iter().map(|(_, _, op)| op))
        .chain(adds)
        .collect();
    serde_json::to_string_pretty(&ops).unwrap_or_default()
}

/// One resolved JSON Pointer token.
enum PtrToken {
    /// Object key, already escaped (`~0` / `~1`).
    Key(String),
    /// Array index.
    Index(usize),
    /// RFC 6902 `-`: append to the array (a keyed element absent in the
    /// old tree has no index there).
    Append,
}

/// Translate a data path (`users[id=4217].email`) into JSON Pointer
/// tokens, resolving key lookups against the old tree.
fn resolve_pointer(path: &str, old: &Value) -> Vec<PtrToken> {
    let segments = crate::patch::parse_path(path).unwrap_or_default();
    let mut tokens = Vec::with_capacity(segments.len());
    let mut node: Option<&Value> = Some(old);
    for seg in &segments {
        match seg {
            crate::patch::Segment::Field(name) => {
                tokens.push(PtrToken::Key(escape_pointer_token(name)));
                node = node.and_then(|n| match n {
                    Value::Object(map) => map.get(name),
                    _ => None,
                });
            }
            crate::patch::Segment::Index(i) => {
                tokens.push(PtrToken::Index(*i));
                node = node.and_then(|n| match n {
                    Value::Array(items) => items.get(*i),
                    _ => None,
                });
            }
            crate::patch::Segment::KeyLookup { key, value } => {
                let resolved = node.and_then(|n| match n {
                    Value::Array(items) => items
                        .iter()
                        .position(|item| crate::patch::key_matches(item, key, value))
                        .map(|i| (i, items)),
                    _ => None,
                });
                match resolved {
                    Some((i, items)) => {
                        tokens.push(PtrToken::Index(i));
                        node = items.get(i);
                    }
                    None => {
                        tokens.push(PtrToken::Append);
                        node = None;
                    }
                }
            }
        }
    }
    tokens
}

/// JSON Pointer escaping of a single reference token (RFC 6901).
fn escape_pointer_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

/// Join tokens into a JSON Pointer string; the root is the empty string.
fn tokens_to_pointer(tokens: &[PtrToken]) -> String {
    let mut pointer = String::new();
    for token in tokens {
        pointer.push('/');
        match token {
            PtrToken::Key(k) => pointer.push_str(k),
            PtrToken::Index(i) => pointer.push_str(&i.to_string()),
            PtrToken::Append => pointer.push('-'),
        }
    }
    pointer
}
