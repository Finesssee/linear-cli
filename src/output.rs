use anyhow::Result;
use clap::ValueEnum;
use serde_json::{Map, Value};
use std::cmp::Ordering;

use crate::OutputFormat;

#[derive(Debug, Clone, Copy, Default, ValueEnum, PartialEq)]
pub enum SortOrder {
    #[default]
    Asc,
    Desc,
}

#[derive(Debug, Clone)]
pub struct JsonOutputOptions {
    pub compact: bool,
    pub fields: Option<Vec<String>>,
    pub sort: Option<String>,
    pub order: SortOrder,
    pub default_sort: bool,
}

impl JsonOutputOptions {
    pub fn new(
        compact: bool,
        fields: Option<Vec<String>>,
        sort: Option<String>,
        order: SortOrder,
        default_sort: bool,
    ) -> Self {
        Self {
            compact,
            fields,
            sort,
            order,
            default_sort,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OutputOptions {
    pub format: OutputFormat,
    pub json: JsonOutputOptions,
}

impl OutputOptions {
    pub fn is_json(&self) -> bool {
        self.format == OutputFormat::Json
    }
}

pub fn print_json(value: &Value, opts: &JsonOutputOptions) -> Result<()> {
    let mut out = value.clone();
    apply_sort(&mut out, opts);
    if let Some(fields) = opts.fields.as_ref() {
        out = select_fields(&out, fields);
    }

    let text = if opts.compact {
        serde_json::to_string(&out)?
    } else {
        serde_json::to_string_pretty(&out)?
    };
    println!("{}", text);
    Ok(())
}

fn apply_sort(value: &mut Value, opts: &JsonOutputOptions) {
    let Value::Array(items) = value else { return };

    let sort_key = opts
        .sort
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            if opts.default_sort {
                default_sort_key(items)
            } else {
                None
            }
        });

    let Some(key) = sort_key else { return };

    let mut indexed: Vec<(usize, Value)> = items.drain(..).enumerate().collect();
    indexed.sort_by(|(idx_a, a), (idx_b, b)| {
        let ord = compare_json_field(a, b, &key);
        let ord = match opts.order {
            SortOrder::Asc => ord,
            SortOrder::Desc => ord.reverse(),
        };
        if ord == Ordering::Equal {
            idx_a.cmp(idx_b)
        } else {
            ord
        }
    });
    *items = indexed.into_iter().map(|(_, v)| v).collect();
}

fn default_sort_key(items: &[Value]) -> Option<String> {
    if items.iter().any(|v| has_object_key(v, "identifier")) {
        return Some("identifier".to_string());
    }
    if items.iter().any(|v| has_object_key(v, "id")) {
        return Some("id".to_string());
    }
    None
}

fn has_object_key(value: &Value, key: &str) -> bool {
    value.as_object().and_then(|obj| obj.get(key)).is_some()
}

fn compare_json_field(a: &Value, b: &Value, key: &str) -> Ordering {
    let a_key = extract_sort_key(a, key);
    let b_key = extract_sort_key(b, key);
    a_key.cmp(&b_key)
}

fn extract_sort_key(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::String(s)) => s.to_lowercase(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

fn select_fields(value: &Value, fields: &[String]) -> Value {
    match value {
        Value::Array(items) => {
            Value::Array(items.iter().map(|v| select_fields(v, fields)).collect())
        }
        Value::Object(_) => {
            let mut out = Map::new();
            for path in fields {
                let parts: Vec<&str> = path.split('.').filter(|p| !p.is_empty()).collect();
                if parts.is_empty() {
                    continue;
                }
                if let Some(field_value) = get_path(value, &parts) {
                    set_path(&mut out, &parts, field_value);
                }
            }
            Value::Object(out)
        }
        _ => value.clone(),
    }
}

fn get_path(value: &Value, parts: &[&str]) -> Option<Value> {
    let mut current = value;
    for part in parts {
        current = current.get(*part)?;
    }
    Some(current.clone())
}

fn set_path(out: &mut Map<String, Value>, parts: &[&str], value: Value) {
    if parts.is_empty() {
        return;
    }
    if parts.len() == 1 {
        out.insert(parts[0].to_string(), value);
        return;
    }
    let entry = out
        .entry(parts[0].to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if let Value::Object(ref mut map) = entry {
        set_path(map, &parts[1..], value);
    }
}
