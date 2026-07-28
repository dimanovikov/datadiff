//! Internal data tree that all supported formats are parsed into.

use std::collections::BTreeMap;

/// A single numeric value. Integers and floats are unified so that `1` and
/// `1.0` compare as equal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Number {
    Int(i64),
    UInt(u64),
    Float(f64),
}

impl Number {
    /// Numeric value as `f64` for cross-representation comparison.
    pub fn as_f64(&self) -> f64 {
        match self {
            Number::Int(i) => *i as f64,
            Number::UInt(u) => *u as f64,
            Number::Float(f) => *f,
        }
    }

    /// JSON-style rendering: `1` stays `1`, `1.5` stays `1.5`, `1.0` renders `1.0`.
    pub fn to_json_string(self) -> String {
        match self {
            Number::Int(i) => i.to_string(),
            Number::UInt(u) => u.to_string(),
            Number::Float(f) => {
                if f.is_finite() && f.fract() == 0.0 && f.abs() < 1e15 {
                    format!("{f:.1}")
                } else {
                    f.to_string()
                }
            }
        }
    }
}

/// Unified value tree. Object entries are stored in a `BTreeMap` so key order
/// is irrelevant by construction.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

impl Value {
    /// Render the value in JSON representation (for diff output).
    pub fn to_json_string(&self) -> String {
        match self {
            Value::Null => "null".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Number(n) => n.to_json_string(),
            Value::String(s) => serde_json::to_string(s).unwrap_or_else(|_| format!("\"{s}\"")),
            Value::Array(_) | Value::Object(_) => {
                let v = self.to_serde_json();
                serde_json::to_string(&v).unwrap_or_default()
            }
        }
    }

    /// Convert to `serde_json::Value` for serialization of compound values.
    pub fn to_serde_json(&self) -> serde_json::Value {
        match self {
            Value::Null => serde_json::Value::Null,
            Value::Bool(b) => serde_json::Value::Bool(*b),
            Value::Number(Number::Int(i)) => serde_json::Value::from(*i),
            Value::Number(Number::UInt(u)) => serde_json::Value::from(*u),
            Value::Number(Number::Float(f)) => serde_json::Number::from_f64(*f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            Value::String(s) => serde_json::Value::String(s.clone()),
            Value::Array(items) => {
                serde_json::Value::Array(items.iter().map(Value::to_serde_json).collect())
            }
            Value::Object(map) => serde_json::Value::Object(
                map.iter()
                    .map(|(k, v)| (k.clone(), v.to_serde_json()))
                    .collect(),
            ),
        }
    }

    /// Build a `Value` from a `serde_json::Value`.
    pub fn from_serde_json(v: serde_json::Value) -> Value {
        match v {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(b) => Value::Bool(b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Value::Number(Number::Int(i))
                } else if let Some(u) = n.as_u64() {
                    Value::Number(Number::UInt(u))
                } else {
                    Value::Number(Number::Float(n.as_f64().unwrap_or(f64::NAN)))
                }
            }
            serde_json::Value::String(s) => Value::String(s),
            serde_json::Value::Array(items) => {
                Value::Array(items.into_iter().map(Value::from_serde_json).collect())
            }
            serde_json::Value::Object(map) => Value::Object(
                map.into_iter()
                    .map(|(k, v)| (k, Value::from_serde_json(v)))
                    .collect(),
            ),
        }
    }

    /// Semantic equality: numbers compare across int/float representations.
    pub fn semantically_eq(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Number(a), Value::Number(b)) => numbers_eq(a, b),
            _ => self == other,
        }
    }
}

/// Compare two numbers so that `1 == 1.0`.
pub fn numbers_eq(a: &Number, b: &Number) -> bool {
    match (a, b) {
        (Number::Int(x), Number::Int(y)) => x == y,
        (Number::UInt(x), Number::UInt(y)) => x == y,
        (Number::Int(x), Number::UInt(y)) => *x >= 0 && *x as u64 == *y,
        (Number::UInt(x), Number::Int(y)) => *y >= 0 && *x == *y as u64,
        _ => a.as_f64() == b.as_f64(),
    }
}

/// Serialize any `serde::Serialize` value into our tree via JSON as the
/// common intermediate (works for serde_yml and toml outputs).
pub fn from_serializable<T: serde::Serialize>(v: &T) -> anyhow::Result<Value> {
    let json = serde_json::to_value(v)?;
    Ok(Value::from_serde_json(json))
}
