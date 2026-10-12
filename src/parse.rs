//! Format detection and parsing into the internal `Value` tree.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{bail, Context};

use crate::value::{from_serializable, Number, Value};

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
/// every data row becomes one object. A column whose cells all read as
/// numbers holds numbers, so numeric comparisons work the same as in
/// JSON/YAML; every other cell stays a string (see [`numeric_columns`]).
fn parse_csv(contents: &str) -> anyhow::Result<Value> {
    let table = read_csv(contents)?;
    let numeric = numeric_columns([&table]);
    Ok(table.into_value(&numeric))
}

/// A CSV file read as text: the header row and the data rows, before any
/// cell is turned into a number.
pub struct CsvTable {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl CsvTable {
    /// Key for column `j`: its header, or `columnN` past the header row.
    fn column_name(&self, j: usize) -> String {
        self.headers
            .get(j)
            .cloned()
            .unwrap_or_else(|| format!("column{}", j + 1))
    }

    /// One object per row. Cells of a `numeric` column become numbers; every
    /// other cell stays text.
    pub fn into_value(self, numeric: &BTreeSet<String>) -> Value {
        let rows = self
            .rows
            .iter()
            .map(|row| {
                let obj = row
                    .iter()
                    .enumerate()
                    .map(|(j, field)| {
                        let key = self.column_name(j);
                        let number = numeric
                            .contains(&key)
                            .then(|| csv_number(field.trim()))
                            .flatten();
                        let value = match number {
                            Some(n) => Value::Number(n),
                            None if field.trim().is_empty() => Value::String(String::new()),
                            None => Value::String(field.clone()),
                        };
                        (key, value)
                    })
                    .collect::<BTreeMap<_, _>>();
                Value::Object(obj)
            })
            .collect();
        Value::Array(rows)
    }
}

pub fn read_csv(contents: &str) -> anyhow::Result<CsvTable> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(contents.as_bytes());
    let headers = rdr
        .headers()
        .context("invalid CSV: cannot read header row")?
        .iter()
        .map(str::to_string)
        .collect();
    let mut rows = Vec::new();
    for (i, record) in rdr.records().enumerate() {
        let record = record.with_context(|| format!("invalid CSV at row {}", i + 2))?;
        rows.push(record.iter().map(str::to_string).collect());
    }
    Ok(CsvTable { headers, rows })
}

/// Columns whose every non-empty cell, in all of `tables`, reads as a number
/// without losing what was written. Deciding per column keeps a code like
/// `1E5` as text among other codes. Deciding over both versions of a file at
/// once keeps one `N/A` from making a column text on one side only, which
/// would report every cell in it as changed (`100` → `"100"`).
pub fn numeric_columns<'a>(tables: impl IntoIterator<Item = &'a CsvTable>) -> BTreeSet<String> {
    let mut numeric = BTreeMap::new();
    for table in tables {
        for row in &table.rows {
            for (j, field) in row.iter().enumerate() {
                let field = field.trim();
                if field.is_empty() {
                    continue;
                }
                let all_numbers = numeric.entry(table.column_name(j)).or_insert(true);
                *all_numbers &= csv_number(field).is_some();
            }
        }
    }
    numeric
        .into_iter()
        .filter_map(|(name, all_numbers)| all_numbers.then_some(name))
        .collect()
}

/// The number a trimmed CSV cell holds, if reading it as one loses nothing.
fn csv_number(field: &str) -> Option<Number> {
    if !is_plain_number(field) {
        return None;
    }
    if let Ok(i) = field.parse::<i64>() {
        return Some(Number::Int(i));
    }
    // An integer too long for i64 would lose digits as a float.
    let is_integer = field
        .trim_start_matches(['+', '-'])
        .bytes()
        .all(|b| b.is_ascii_digit());
    if is_integer {
        return None;
    }
    field
        .parse::<f64>()
        .ok()
        .filter(|f| f.is_finite())
        .map(Number::Float)
}

/// Whether a CSV cell can be read as a number without losing what was
/// written: only digits, sign, point and exponent (so `NaN` and `inf` stay
/// text), and no leading zero before another digit (so `00544` stays an
/// identifier rather than becoming 544).
fn is_plain_number(s: &str) -> bool {
    let numeric_chars = s
        .bytes()
        .all(|b| b.is_ascii_digit() || matches!(b, b'+' | b'-' | b'.' | b'e' | b'E'));
    let unsigned = s.trim_start_matches(['+', '-']).as_bytes();
    let leading_zero = unsigned.len() > 1 && unsigned[0] == b'0' && unsigned[1].is_ascii_digit();
    numeric_chars && !leading_zero
}
