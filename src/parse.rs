//! Format detection and parsing into the internal `Value` tree.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Context};

use crate::value::{from_serializable, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    Json,
    Yaml,
    Csv,
    Toml,
    Xml,
}

/// Detect the format from a file extension.
pub fn detect_format(path: &Path) -> anyhow::Result<Format> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "json" => Ok(Format::Json),
        "yaml" | "yml" => Ok(Format::Yaml),
        "csv" => Ok(Format::Csv),
        "toml" => Ok(Format::Toml),
        "xml" => Ok(Format::Xml),
        other => {
            bail!("cannot detect format from extension '.{other}' (use --format to specify it)")
        }
    }
}

/// Parse file contents of the given format into the value tree.
pub fn parse(contents: &str, format: Format) -> anyhow::Result<Value> {
    match format {
        Format::Json => parse_json(contents),
        Format::Yaml => parse_yaml(contents),
        Format::Csv => parse_csv(contents),
        Format::Toml => parse_toml(contents),
        Format::Xml => parse_xml(contents),
    }
}

fn parse_json(contents: &str) -> anyhow::Result<Value> {
    let v: serde_json::Value = serde_json::from_str(contents).context("invalid JSON")?;
    Ok(Value::from_serde_json(v))
}

fn parse_yaml(contents: &str) -> anyhow::Result<Value> {
    let v: serde_yml::Value = serde_yml::from_str(contents).context("invalid YAML")?;
    from_serializable(&v).context("failed to convert YAML")
}

fn parse_toml(contents: &str) -> anyhow::Result<Value> {
    let v: toml::Value = toml::from_str(contents).context("invalid TOML")?;
    from_serializable(&v).context("failed to convert TOML")
}

/// Parse XML into the value tree via quick-xml's serde mapping: element
/// names become keys, attributes become keys with an `@` prefix, and
/// repeated sibling elements become arrays.
fn parse_xml(contents: &str) -> anyhow::Result<Value> {
    let v: serde_json::Value = quick_xml::de::from_str(contents).context("invalid XML")?;
    Ok(Value::from_serde_json(v))
}

/// Parse CSV into an array of objects: the header row provides the keys and
/// every data row becomes one object. Values are kept as strings except that
/// values that look like numbers are parsed as numbers, so numeric
/// comparisons work the same as in JSON/YAML.
fn parse_csv(contents: &str) -> anyhow::Result<Value> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(contents.as_bytes());
    let headers: Vec<String> = rdr
        .headers()
        .context("invalid CSV: cannot read header row")?
        .iter()
        .map(|h| h.to_string())
        .collect();

    let mut rows = Vec::new();
    for (i, record) in rdr.records().enumerate() {
        let record = record.with_context(|| format!("invalid CSV at row {}", i + 2))?;
        let mut obj = BTreeMap::new();
        for (j, field) in record.iter().enumerate() {
            let key = headers
                .get(j)
                .cloned()
                .unwrap_or_else(|| format!("column{}", j + 1));
            obj.insert(key, csv_field_to_value(field));
        }
        rows.push(Value::Object(obj));
    }
    Ok(Value::Array(rows))
}

fn csv_field_to_value(field: &str) -> Value {
    let trimmed = field.trim();
    if trimmed.is_empty() {
        return Value::String(String::new());
    }
    if let Ok(i) = trimmed.parse::<i64>() {
        return Value::Number(crate::value::Number::Int(i));
    }
    if let Ok(f) = trimmed.parse::<f64>() {
        return Value::Number(crate::value::Number::Float(f));
    }
    Value::String(field.to_string())
}
