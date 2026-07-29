//! Apply a JSON patch (the format produced by `--output json`) to a value
//! tree.
//!
//! Known limitation: object keys containing `.`, `[` or `]` cannot be
//! represented in our path strings, so changes on such paths cannot be
//! patched.

use anyhow::{bail, Context};

use crate::diff::scalar_key_text;
use crate::value::Value;

/// One path segment, parsed from strings like `users[id=4217].email`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    Field(String),
    Index(usize),
    KeyLookup { key: String, value: String },
}

/// Apply every change of a patch document (`--output json` format) to the
/// tree. Unchanged entries are ignored; a change that cannot be applied is
/// an error — no silent skips.
pub fn apply_patch(tree: &mut Value, patch: &serde_json::Value) -> anyhow::Result<()> {
    let changes = patch
        .get("changes")
        .and_then(|c| c.as_array())
        .context("invalid patch: missing \"changes\" array")?;
    for change in changes {
        let kind = change
            .get("type")
            .and_then(|t| t.as_str())
            .context("invalid patch: change without \"type\"")?;
        let path = change
            .get("path")
            .and_then(|p| p.as_str())
            .context("invalid patch: change without \"path\"")?;
        match kind {
            "added" | "modified" => {
                let new = change
                    .get("new")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                set_at_path(tree, path, Value::from_serde_json(new))?;
            }
            "removed" => remove_at_path(tree, path)?,
            "unchanged" => {}
            other => bail!("invalid patch: unknown change type '{other}'"),
        }
    }
    Ok(())
}

/// Parse a data path into segments. A bare `$` (or empty path) is the root.
fn parse_path(path: &str) -> anyhow::Result<Vec<Segment>> {
    let mut segments = Vec::new();
    let mut rest = path;
    if let Some(stripped) = rest.strip_prefix('$') {
        rest = stripped.strip_prefix('.').unwrap_or(stripped);
    }
    while !rest.is_empty() {
        // Field name runs up to the next '.' or '['.
        let field_end = rest.find(['.', '[']).unwrap_or(rest.len());
        let field = &rest[..field_end];
        if !field.is_empty() {
            segments.push(Segment::Field(field.to_string()));
        }
        rest = &rest[field_end..];
        if rest.is_empty() {
            break;
        }
        if rest.starts_with('.') {
            rest = &rest[1..];
            if rest.is_empty() {
                bail!("invalid path '{path}': trailing '.'");
            }
            continue;
        }
        // Bracket group(s): [3] or [key=value].
        while rest.starts_with('[') {
            let close = rest
                .find(']')
                .with_context(|| format!("invalid path '{path}': unclosed '['"))?;
            let inner = &rest[1..close];
            if let Some((k, v)) = inner.split_once('=') {
                segments.push(Segment::KeyLookup {
                    key: k.to_string(),
                    value: v.to_string(),
                });
            } else if let Ok(i) = inner.parse::<usize>() {
                segments.push(Segment::Index(i));
            } else {
                bail!("invalid path '{path}': bad bracket '[{inner}]'");
            }
            rest = &rest[close + 1..];
        }
        if let Some(stripped) = rest.strip_prefix('.') {
            rest = stripped;
            if rest.is_empty() {
                bail!("invalid path '{path}': trailing '.'");
            }
        } else if !rest.is_empty() {
            bail!("invalid path '{path}': expected '.' before '{rest}'");
        }
    }
    Ok(segments)
}

fn set_at_path(tree: &mut Value, path: &str, new: Value) -> anyhow::Result<()> {
    let segments = parse_path(path)?;
    let Some((last, parents)) = segments.split_last() else {
        *tree = new; // root replacement
        return Ok(());
    };
    let mut node = &mut *tree;
    for seg in parents {
        node = navigate_mut(node, seg, path)?;
    }
    match last {
        Segment::Field(name) => {
            let Value::Object(map) = node else {
                bail!("cannot set '{path}': parent is not an object");
            };
            map.insert(name.clone(), new);
        }
        Segment::Index(i) => {
            let Value::Array(items) = node else {
                bail!("cannot set '{path}': parent is not an array");
            };
            if *i < items.len() {
                items[*i] = new;
            } else if *i == items.len() {
                items.push(new);
            } else {
                bail!("cannot set '{path}': index {i} is out of range");
            }
        }
        Segment::KeyLookup { key, value } => {
            let Value::Array(items) = node else {
                bail!("cannot set '{path}': parent is not an array");
            };
            match items.iter_mut().find(|item| key_matches(item, key, value)) {
                Some(slot) => *slot = new,
                None => items.push(new), // added element
            }
        }
    }
    Ok(())
}

fn remove_at_path(tree: &mut Value, path: &str) -> anyhow::Result<()> {
    let segments = parse_path(path)?;
    let Some((last, parents)) = segments.split_last() else {
        bail!("cannot remove the document root");
    };
    let mut node = &mut *tree;
    for seg in parents {
        node = navigate_mut(node, seg, path)?;
    }
    match last {
        Segment::Field(name) => {
            let Value::Object(map) = node else {
                bail!("cannot remove '{path}': parent is not an object");
            };
            if map.remove(name).is_none() {
                bail!("cannot remove '{path}': no key '{name}'");
            }
        }
        Segment::Index(i) => {
            let Value::Array(items) = node else {
                bail!("cannot remove '{path}': parent is not an array");
            };
            if *i >= items.len() {
                bail!("cannot remove '{path}': index {i} is out of range");
            }
            items.remove(*i);
        }
        Segment::KeyLookup { key, value } => {
            let Value::Array(items) = node else {
                bail!("cannot remove '{path}': parent is not an array");
            };
            let pos = items
                .iter()
                .position(|item| key_matches(item, key, value))
                .with_context(|| {
                    format!("cannot remove '{path}': no element with {key}={value}")
                })?;
            items.remove(pos);
        }
    }
    Ok(())
}

/// Navigate one segment down the tree, with patch-oriented error messages.
fn navigate_mut<'a>(
    node: &'a mut Value,
    seg: &Segment,
    path: &str,
) -> anyhow::Result<&'a mut Value> {
    match seg {
        Segment::Field(name) => match node {
            Value::Object(map) => map
                .get_mut(name)
                .with_context(|| format!("cannot resolve '{path}': no key '{name}'")),
            _ => bail!("cannot resolve '{path}': '{name}' is not inside an object"),
        },
        Segment::Index(i) => match node {
            Value::Array(items) => items
                .get_mut(*i)
                .with_context(|| format!("cannot resolve '{path}': index {i} is out of range")),
            _ => bail!("cannot resolve '{path}': [{i}] is not inside an array"),
        },
        Segment::KeyLookup { key, value } => match node {
            Value::Array(items) => items
                .iter_mut()
                .find(|item| key_matches(item, key, value))
                .with_context(|| format!("cannot resolve '{path}': no element with {key}={value}")),
            _ => bail!("cannot resolve '{path}': [{key}={value}] is not inside an array"),
        },
    }
}

/// Does `item` carry the field `key` whose scalar path text equals `value`?
fn key_matches(item: &Value, key: &str, value: &str) -> bool {
    let Value::Object(obj) = item else {
        return false;
    };
    obj.get(key).and_then(scalar_key_text).as_deref() == Some(value)
}
