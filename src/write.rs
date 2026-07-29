//! Serialize the value tree back into a supported file format.

use anyhow::{bail, Context};

use crate::parse::Format;
use crate::value::Value;

/// Render the tree in the given format. Not every tree fits every format:
/// TOML needs an object at the root (and no nulls), CSV needs an array of
/// flat objects, and XML writing is not supported at all.
pub fn write(value: &Value, format: Format) -> anyhow::Result<String> {
    match format {
        Format::Json => Ok(serde_json::to_string_pretty(&value.to_serde_json())?),
        Format::Yaml => {
            serde_yml::to_string(&value.to_serde_json()).context("failed to write YAML")
        }
        Format::Toml => write_toml(value),
        Format::Csv => write_csv(value),
        Format::Xml => {
            bail!("writing XML is not supported (the attribute/text mapping is lossy)")
        }
    }
}

fn write_toml(value: &Value) -> anyhow::Result<String> {
    let Value::Object(_) = value else {
        bail!("cannot write TOML: the document root is not an object");
    };
    toml::to_string_pretty(&value.to_serde_json()).context("failed to write TOML (nulls?)")
}

fn write_csv(value: &Value) -> anyhow::Result<String> {
    let Value::Array(rows) = value else {
        bail!("cannot write CSV: the document root is not an array");
    };
    // Header: union of all keys, in first-seen order.
    let mut headers: Vec<String> = Vec::new();
    for row in rows {
        let Value::Object(obj) = row else {
            bail!("cannot write CSV: every row must be an object");
        };
        for (k, v) in obj {
            if matches!(v, Value::Array(_) | Value::Object(_)) {
                bail!("cannot write CSV: field '{k}' is not a scalar");
            }
            if !headers.contains(k) {
                headers.push(k.clone());
            }
        }
    }
    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record(&headers)?;
    for row in rows {
        let Value::Object(obj) = row else {
            unreachable!("rows are validated above");
        };
        let record: Vec<String> = headers.iter().map(|h| csv_scalar(obj.get(h))).collect();
        wtr.write_record(&record)?;
    }
    let bytes = wtr.into_inner()?;
    Ok(String::from_utf8(bytes)?)
}

/// Render a scalar for a CSV cell; missing values and nulls are empty.
fn csv_scalar(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => n.to_json_string(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(_) | Value::Object(_)) => {
            unreachable!("non-scalar fields are rejected above")
        }
    }
}
