use anyhow::Result;
use serde_json::{Map, Value};

use crate::api::LinearClient;
use crate::json_path::get_path;

#[derive(Debug, Clone, Default)]
pub struct PaginationOptions {
    pub limit: Option<usize>,
    pub after: Option<String>,
    pub before: Option<String>,
    pub page_size: Option<usize>,
    pub all: bool,
}

impl PaginationOptions {
    pub fn with_default_limit(&self, default_limit: usize) -> Self {
        let mut options = self.clone();
        if !options.all && options.limit.is_none() {
            options.limit = Some(default_limit);
        }
        options
    }

    pub fn effective_page_size(&self, default_page_size: usize) -> usize {
        self.page_size.unwrap_or(default_page_size).max(1)
    }
}


pub async fn paginate_nodes(
    client: &LinearClient,
    query: &str,
    mut variables: Map<String, Value>,
    nodes_path: &[&str],
    page_info_path: &[&str],
    options: &PaginationOptions,
    default_page_size: usize,
) -> Result<Vec<Value>> {
    let mut items: Vec<Value> = Vec::new();
    let remaining = if options.all { None } else { options.limit };
    let mut after = options.after.clone();
    let mut before = options.before.clone();
    let mut forward = before.is_none();

    if options.after.is_some() && options.before.is_some() {
        before = None;
        forward = true;
    }

    let page_size = options.effective_page_size(default_page_size);

    loop {
        let batch_size = remaining
            .map(|r| r.saturating_sub(items.len()).min(page_size))
            .unwrap_or(page_size)
            .max(1);

        if forward {
            variables.insert(
                "first".to_string(),
                Value::Number(serde_json::Number::from(batch_size as u64)),
            );
            if let Some(ref cursor) = after {
                variables.insert("after".to_string(), Value::String(cursor.clone()));
            } else {
                variables.remove("after");
            }
            variables.remove("last");
            variables.remove("before");
        } else {
            variables.insert(
                "last".to_string(),
                Value::Number(serde_json::Number::from(batch_size as u64)),
            );
            if let Some(ref cursor) = before {
                variables.insert("before".to_string(), Value::String(cursor.clone()));
            } else {
                variables.remove("before");
            }
            variables.remove("first");
            variables.remove("after");
        }

        let result = client
            .query(query, Some(Value::Object(variables.clone())))
            .await?;

        let nodes = get_path(&result, nodes_path)
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        items.extend(nodes);

        if let Some(limit) = remaining {
            if limit <= items.len() {
                items.truncate(limit);
                break;
            }
        }

        if !options.all && options.limit.is_none() {
            break;
        }

        let page_info = get_path(&result, page_info_path).and_then(|v| v.as_object());
        let Some(page_info) = page_info else { break };

        if forward {
            let has_next = page_info
                .get("hasNextPage")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if !has_next {
                break;
            }
            after = page_info
                .get("endCursor")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            if after.is_none() {
                break;
            }
        } else {
            let has_prev = page_info
                .get("hasPreviousPage")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if !has_prev {
                break;
            }
            before = page_info
                .get("startCursor")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            if before.is_none() {
                break;
            }
        }
    }

    Ok(items)
}
