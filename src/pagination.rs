use anyhow::Result;
use serde_json::{Map, Value};
use std::future::Future;

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

/// Stream paginated results, calling a handler for each batch of nodes.
///
/// This is memory-efficient for large exports because it processes each page
/// as it arrives rather than accumulating all results in memory.
///
/// The handler receives each batch of nodes and can process them immediately
/// (e.g., write to CSV). Returns the total number of items processed.
///
/// # Arguments
///
/// * `client` - The Linear API client
/// * `query` - GraphQL query with pagination variables
/// * `variables` - Initial query variables
/// * `nodes_path` - JSON path to the nodes array in the response
/// * `page_info_path` - JSON path to the pageInfo object in the response
/// * `options` - Pagination options (limit, page_size, all, etc.)
/// * `default_page_size` - Default page size if not specified in options
/// * `handler` - Async function called with each batch of nodes
///
/// # Example
///
/// ```ignore
/// let count = stream_nodes(
///     &client,
///     query,
///     vars,
///     &["data", "issues", "nodes"],
///     &["data", "issues", "pageInfo"],
///     &pagination,
///     250,
///     |batch| async {
///         for issue in batch {
///             wtr.write_record(&[...])?;
///         }
///         Ok(())
///     },
/// ).await?;
/// ```
pub async fn stream_nodes<F, Fut>(
    client: &LinearClient,
    query: &str,
    mut variables: Map<String, Value>,
    nodes_path: &[&str],
    page_info_path: &[&str],
    options: &PaginationOptions,
    default_page_size: usize,
    mut handler: F,
) -> Result<usize>
where
    F: FnMut(Vec<Value>) -> Fut,
    Fut: Future<Output = Result<()>>,
{
    let mut total: usize = 0;
    let limit = if options.all { None } else { options.limit };
    let mut after = options.after.clone();

    let page_size = options.effective_page_size(default_page_size);

    loop {
        // Calculate batch size respecting limit
        let batch_size = limit
            .map(|l| l.saturating_sub(total).min(page_size))
            .unwrap_or(page_size)
            .max(1);

        // If we've already hit the limit, stop
        if let Some(l) = limit {
            if total >= l {
                break;
            }
        }

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

        let result = client
            .query(query, Some(Value::Object(variables.clone())))
            .await?;

        let mut nodes = get_path(&result, nodes_path)
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        // Truncate if we'd exceed the limit
        if let Some(l) = limit {
            let remaining = l.saturating_sub(total);
            if nodes.len() > remaining {
                nodes.truncate(remaining);
            }
        }

        let count = nodes.len();
        if count == 0 {
            break;
        }

        total += count;

        // Process this batch
        handler(nodes).await?;

        // Check if we've hit the limit
        if let Some(l) = limit {
            if total >= l {
                break;
            }
        }

        // Check for more pages (only if we want all or have a limit we haven't reached)
        if !options.all && options.limit.is_none() {
            break;
        }

        let page_info = get_path(&result, page_info_path).and_then(|v| v.as_object());
        let Some(page_info) = page_info else { break };

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
    }

    Ok(total)
}
