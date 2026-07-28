//! Colored, line-oriented diff rendering.

use colored::Colorize;

use crate::diff::{Change, ChangeKind, Summary};

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
