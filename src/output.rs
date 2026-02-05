use anyhow::Result;
use clap::ValueEnum;
use serde_json::{Map, Value};
use std::cmp::Ordering;
use std::sync::OnceLock;

use regex::Regex;

use crate::cache::CacheOptions;
use crate::error::CliError;
use crate::json_path::get_path;
use crate::pagination::PaginationOptions;
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
    pub format_template: Option<String>,
    pub filters: Vec<FilterExpr>,
    pub fail_on_empty: bool,
    pub pagination: PaginationOptions,
    pub cache: CacheOptions,
    pub dry_run: bool,
}

impl OutputOptions {
    pub fn is_json(&self) -> bool {
        matches!(self.format, OutputFormat::Json | OutputFormat::Ndjson)
    }

    pub fn is_ndjson(&self) -> bool {
        self.format == OutputFormat::Ndjson
    }

    pub fn has_template(&self) -> bool {
        self.format_template
            .as_deref()
            .map(|t| !t.trim().is_empty())
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone)]
pub enum FilterOp {
    Eq,
    NotEq,
    Contains,
}

#[derive(Debug, Clone)]
pub struct FilterExpr {
    pub path: Vec<String>,
    pub op: FilterOp,
    pub value: String,
}

pub fn parse_filters(filters: &[String]) -> Result<Vec<FilterExpr>> {
    filters
        .iter()
        .filter(|f| !f.trim().is_empty())
        .map(|f| parse_filter(f))
        .collect()
}

fn parse_filter(input: &str) -> Result<FilterExpr> {
    let trimmed = input.trim();
    let (path, op, value) = if let Some((left, right)) = trimmed.split_once("!=") {
        (left, FilterOp::NotEq, right)
    } else if let Some((left, right)) = trimmed.split_once("~=") {
        (left, FilterOp::Contains, right)
    } else if let Some((left, right)) = trimmed.split_once('=') {
        (left, FilterOp::Eq, right)
    } else {
        anyhow::bail!(
            "Invalid filter '{}'. Use field=value, field!=value, or field~=value",
            input
        );
    };

    let path_parts: Vec<String> = path
        .split('.')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .map(|p| p.to_string())
        .collect();

    if path_parts.is_empty() {
        anyhow::bail!("Invalid filter '{}': missing field path", input);
    }

    Ok(FilterExpr {
        path: path_parts,
        op,
        value: value.trim().to_string(),
    })
}

pub fn print_json(value: &Value, output: &OutputOptions) -> Result<()> {
    let mut out = value.clone();
    apply_filters(&mut out, &output.filters);
    apply_sort(&mut out, &output.json);
    if let Some(fields) = output.json.fields.as_ref() {
        out = select_fields(&out, fields);
    }

    if output.fail_on_empty {
        if let Value::Array(items) = &out {
            if items.is_empty() {
                return Err(CliError::new(2, "No results found").into());
            }
        }
    }

    if let Some(template) = output.format_template.as_deref() {
        return print_template(&out, template);
    }

    if output.is_ndjson() {
        return print_ndjson(&out);
    }

    let text = if output.json.compact {
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

fn apply_filters(value: &mut Value, filters: &[FilterExpr]) {
    if filters.is_empty() {
        return;
    }
    let Value::Array(items) = value else { return };
    let filtered: Vec<Value> = items
        .iter()
        .filter(|item| matches_filters(item, filters))
        .cloned()
        .collect();
    *items = filtered;
}

pub fn filter_values(values: &mut Vec<Value>, filters: &[FilterExpr]) {
    if filters.is_empty() {
        return;
    }
    values.retain(|value| matches_filters(value, filters));
}

fn matches_filters(value: &Value, filters: &[FilterExpr]) -> bool {
    filters.iter().all(|filter| {
        let mut current = value;
        for part in &filter.path {
            match current.get(part.as_str()) {
                Some(next) => current = next,
                None => return false,
            }
        }
        let actual = value_to_string(current).to_lowercase();
        let expected = filter.value.to_lowercase();
        match filter.op {
            FilterOp::Eq => actual == expected,
            FilterOp::NotEq => actual != expected,
            FilterOp::Contains => actual.contains(&expected),
        }
    })
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        other => other.to_string(),
    }
}

fn template_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\{\{\s*\.?([a-zA-Z0-9_.-]*)\s*\}\}").unwrap())
}

pub fn print_template(value: &Value, template: &str) -> Result<()> {
    match value {
        Value::Array(items) => {
            for item in items {
                println!("{}", render_template(template, item));
            }
        }
        _ => {
            println!("{}", render_template(template, value));
        }
    }
    Ok(())
}

pub fn ensure_non_empty(values: &[Value], output: &OutputOptions) -> Result<()> {
    if output.fail_on_empty && values.is_empty() {
        return Err(CliError::new(2, "No results found").into());
    }
    Ok(())
}

fn render_template(template: &str, value: &Value) -> String {
    template_regex()
        .replace_all(template, |caps: &regex::Captures| {
            let path = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            if path.is_empty() || path == "." {
                return value_to_string(value);
            }
            let parts: Vec<&str> = path.split('.').filter(|p| !p.is_empty()).collect();
            match get_path(value, &parts) {
                Some(found) => value_to_string(found),
                None => String::new(),
            }
        })
        .to_string()
}

fn print_ndjson(value: &Value) -> Result<()> {
    match value {
        Value::Array(items) => {
            for item in items {
                println!("{}", serde_json::to_string(item)?);
            }
        }
        _ => {
            println!("{}", serde_json::to_string(value)?);
        }
    }
    Ok(())
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

pub fn sort_values(values: &mut Vec<Value>, key: &str, order: SortOrder) {
    let mut indexed: Vec<(usize, Value)> = values.drain(..).enumerate().collect();
    indexed.sort_by(|(idx_a, a), (idx_b, b)| {
        let ord = compare_json_field(a, b, key);
        let ord = match order {
            SortOrder::Asc => ord,
            SortOrder::Desc => ord.reverse(),
        };
        if ord == Ordering::Equal {
            idx_a.cmp(idx_b)
        } else {
            ord
        }
    });
    *values = indexed.into_iter().map(|(_, v)| v).collect();
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
                    set_path(&mut out, &parts, field_value.clone());
                }
            }
            Value::Object(out)
        }
        _ => value.clone(),
    }
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

