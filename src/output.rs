//! Diff rendering: colored line-oriented text and machine-readable JSON.

use colored::Colorize;

use crate::diff::{Change, ChangeKind, Summary};

/// Output format selected by `--output`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum OutputFormat {
    /// Human-readable colored lines (default).
    #[default]
    Text,
    /// Machine-readable JSON document.
    Json,
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
