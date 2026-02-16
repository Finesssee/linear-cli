use anyhow::{Context, Result};
use futures::StreamExt;
use reqwest::header::HeaderMap;
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use std::time::Duration;

use crate::cache::{Cache, CacheOptions, CacheType};
use crate::config;
use crate::error::CliError;
use crate::pagination::{paginate_nodes, PaginationOptions};
use crate::retry::{with_retry, RetryConfig};
use crate::text::is_uuid;
use std::sync::OnceLock;

const LINEAR_API_URL: &str = "https://api.linear.app/graphql";

/// Configuration for generic ID resolution
struct ResolverConfig<'a> {
    cache_type: CacheType,
    filtered_query: &'a str,
    filtered_var_name: &'a str,
    filtered_nodes_path: &'a [&'a str],
    paginated_query: &'a str,
    paginated_nodes_path: &'a [&'a str],
    paginated_page_info_path: &'a [&'a str],
    not_found_msg: &'a str,
}

/// Generic ID resolver that handles cache, filtered query, and paginated fallback
async fn resolve_id<F>(
    client: &LinearClient,
    input: &str,
    cache_opts: &CacheOptions,
    config: &ResolverConfig<'_>,
    finder: F,
) -> Result<String>
where
    F: Fn(&[Value], &str) -> Option<String>,
{
    // Check cache first
    if !cache_opts.no_cache {
        let cache = Cache::with_ttl(cache_opts.effective_ttl_seconds())?;
        if let Some(cached) = cache
            .get(config.cache_type)
            .and_then(|data| data.as_array().cloned())
        {
            if let Some(id) = finder(&cached, input) {
                return Ok(id);
            }
        }
    }

    // Try filtered query first (fast path)
    let result = client
        .query(
            config.filtered_query,
            Some(json!({ config.filtered_var_name: input })),
        )
        .await?;
    let empty = vec![];
    let nodes = get_nested_array(&result, config.filtered_nodes_path).unwrap_or(&empty);

    if let Some(id) = finder(nodes, input) {
        return Ok(id);
    }

    // Fallback: paginate through all items
    let pagination = PaginationOptions {
        all: true,
        page_size: Some(250),
        ..Default::default()
    };
    let all_items = paginate_nodes(
        client,
        config.paginated_query,
        serde_json::Map::new(),
        config.paginated_nodes_path,
        config.paginated_page_info_path,
        &pagination,
        250,
    )
    .await?;

    if !cache_opts.no_cache {
        let cache = Cache::with_ttl(cache_opts.effective_ttl_seconds())?;
        let _ = cache.set(config.cache_type, json!(all_items));
    }

    if let Some(id) = finder(&all_items, input) {
        return Ok(id);
    }

    anyhow::bail!("{}", config.not_found_msg)
}

/// Helper to get nested array from JSON value
fn get_nested_array<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Vec<Value>> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_array()
}

/// Build a CliError from HTTP status code and headers
fn http_error(status: StatusCode, headers: &HeaderMap, context: &str) -> CliError {
    let retry_after = headers
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    let request_id = headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_string());
    let details = json!({
        "status": status.as_u16(),
        "reason": status.canonical_reason().unwrap_or("Unknown error"),
        "request_id": request_id,
    });

    let err = match status.as_u16() {
        401 => CliError::auth("Authentication failed - check your API key"),
        403 => CliError::auth(format!("Access denied - {}", context)),
        404 => CliError::not_found(format!("{} not found", context)),
        429 => CliError::rate_limited("Rate limit exceeded").with_retry_after(retry_after),
        _ => CliError::general(format!(
            "HTTP {} {}",
            status.as_u16(),
            details["reason"].as_str().unwrap_or("Unknown error")
        )),
    };
    err.with_details(details)
}

/// Resolves a team key (like "SCW") or name to a team UUID.
/// If the input is already a UUID (36 characters with dashes), returns it as-is.
pub async fn resolve_team_id(
    client: &LinearClient,
    team: &str,
    cache_opts: &CacheOptions,
) -> Result<String> {
    if is_uuid(team) {
        return Ok(team.to_string());
    }

    let config = ResolverConfig {
        cache_type: CacheType::Teams,
        filtered_query: r#"
            query($team: String!) {
                teams(first: 50, filter: { or: [{ key: { eqIgnoreCase: $team } }, { name: { eqIgnoreCase: $team } }] }) {
                    nodes { id key name }
                }
            }
        "#,
        filtered_var_name: "team",
        filtered_nodes_path: &["data", "teams", "nodes"],
        paginated_query: r#"
            query($first: Int, $after: String) {
                teams(first: $first, after: $after) {
                    nodes { id key name }
                    pageInfo { hasNextPage endCursor }
                }
            }
        "#,
        paginated_nodes_path: &["data", "teams", "nodes"],
        paginated_page_info_path: &["data", "teams", "pageInfo"],
        not_found_msg: &format!(
            "Team not found: {}. Use linear-cli t list to see available teams.",
            team
        ),
    };

    resolve_id(client, team, cache_opts, &config, find_team_id).await
}

/// Resolve a user identifier to a UUID.
/// Handles "me", UUIDs, names, and emails.
pub async fn resolve_user_id(
    client: &LinearClient,
    user: &str,
    cache_opts: &CacheOptions,
) -> Result<String> {
    if user.eq_ignore_ascii_case("me") {
        let query = r#"query { viewer { id } }"#;
        let result = client.query(query, None).await?;
        let user_id = result["data"]["viewer"]["id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Could not fetch current user ID"))?;
        return Ok(user_id.to_string());
    }

    if is_uuid(user) {
        return Ok(user.to_string());
    }

    let config = ResolverConfig {
        cache_type: CacheType::Users,
        filtered_query: r#"
            query($user: String!) {
                users(first: 50, filter: { or: [{ name: { eqIgnoreCase: $user } }, { email: { eqIgnoreCase: $user } }] }) {
                    nodes { id name email }
                }
            }
        "#,
        filtered_var_name: "user",
        filtered_nodes_path: &["data", "users", "nodes"],
        paginated_query: r#"
            query($first: Int, $after: String) {
                users(first: $first, after: $after) {
                    nodes { id name email }
                    pageInfo { hasNextPage endCursor }
                }
            }
        "#,
        paginated_nodes_path: &["data", "users", "nodes"],
        paginated_page_info_path: &["data", "users", "pageInfo"],
        not_found_msg: &format!("User not found: {}", user),
    };

    resolve_id(client, user, cache_opts, &config, find_user_id).await
}

/// Resolve a label name to a UUID.
pub async fn resolve_label_id(
    client: &LinearClient,
    label: &str,
    cache_opts: &CacheOptions,
) -> Result<String> {
    if is_uuid(label) {
        return Ok(label.to_string());
    }

    let config = ResolverConfig {
        cache_type: CacheType::Labels,
        filtered_query: r#"
            query($label: String!) {
                issueLabels(first: 50, filter: { name: { eqIgnoreCase: $label } }) {
                    nodes { id name }
                }
            }
        "#,
        filtered_var_name: "label",
        filtered_nodes_path: &["data", "issueLabels", "nodes"],
        paginated_query: r#"
            query($first: Int, $after: String) {
                issueLabels(first: $first, after: $after) {
                    nodes { id name }
                    pageInfo { hasNextPage endCursor }
                }
            }
        "#,
        paginated_nodes_path: &["data", "issueLabels", "nodes"],
        paginated_page_info_path: &["data", "issueLabels", "pageInfo"],
        not_found_msg: &format!("Label not found: {}", label),
    };

    resolve_id(client, label, cache_opts, &config, find_label_id).await
}

/// Resolve a project name or slug to a UUID.
pub async fn resolve_project_id(
    client: &LinearClient,
    project: &str,
    cache_opts: &CacheOptions,
) -> Result<String> {
    if is_uuid(project) {
        return Ok(project.to_string());
    }

    let config = ResolverConfig {
        cache_type: CacheType::Projects,
        filtered_query: r#"
            query($project: String!) {
                projects(first: 50, filter: { name: { eqIgnoreCase: $project } }) {
                    nodes { id name slugId }
                }
            }
        "#,
        filtered_var_name: "project",
        filtered_nodes_path: &["data", "projects", "nodes"],
        paginated_query: r#"
            query($first: Int, $after: String) {
                projects(first: $first, after: $after) {
                    nodes { id name slugId }
                    pageInfo { hasNextPage endCursor }
                }
            }
        "#,
        paginated_nodes_path: &["data", "projects", "nodes"],
        paginated_page_info_path: &["data", "projects", "pageInfo"],
        not_found_msg: &format!(
            "Project not found: {}. Use linear-cli p list to see available projects.",
            project
        ),
    };

    resolve_id(client, project, cache_opts, &config, find_project_id).await
}

/// Resolve a state name to a UUID for a given team.
/// States are team-scoped in Linear, so the team_id must be provided.
pub async fn resolve_state_id(client: &LinearClient, team_id: &str, state: &str) -> Result<String> {
    if is_uuid(state) {
        return Ok(state.to_string());
    }

    let query = r#"
        query($teamId: String!) {
            team(id: $teamId) {
                states {
                    nodes {
                        id
                        name
                    }
                }
            }
        }
    "#;

    let result = client
        .query(query, Some(json!({ "teamId": team_id })))
        .await?;
    let empty = vec![];
    let states = result["data"]["team"]["states"]["nodes"]
        .as_array()
        .unwrap_or(&empty);

    for s in states {
        let name = s["name"].as_str().unwrap_or("");
        if name.eq_ignore_ascii_case(state) {
            if let Some(id) = s["id"].as_str() {
                return Ok(id.to_string());
            }
        }
    }

    anyhow::bail!("State '{}' not found for team", state)
}

fn find_team_id(teams: &[Value], team: &str) -> Option<String> {
    if let Some(team_data) = teams
        .iter()
        .find(|t| t["key"].as_str().map(|k| k.eq_ignore_ascii_case(team)) == Some(true))
    {
        if let Some(id) = team_data["id"].as_str() {
            return Some(id.to_string());
        }
    }

    if let Some(team_data) = teams
        .iter()
        .find(|t| t["name"].as_str().map(|n| n.eq_ignore_ascii_case(team)) == Some(true))
    {
        if let Some(id) = team_data["id"].as_str() {
            return Some(id.to_string());
        }
    }

    None
}

fn find_user_id(users: &[Value], user: &str) -> Option<String> {
    for u in users {
        let name = u["name"].as_str().unwrap_or("");
        let email = u["email"].as_str().unwrap_or("");
        if name.eq_ignore_ascii_case(user) || email.eq_ignore_ascii_case(user) {
            if let Some(id) = u["id"].as_str() {
                return Some(id.to_string());
            }
        }
    }
    None
}

fn find_label_id(labels: &[Value], label: &str) -> Option<String> {
    for l in labels {
        let name = l["name"].as_str().unwrap_or("");
        if name.eq_ignore_ascii_case(label) {
            if let Some(id) = l["id"].as_str() {
                return Some(id.to_string());
            }
        }
    }
    None
}

fn find_project_id(projects: &[Value], project: &str) -> Option<String> {
    // Match by name (case-insensitive)
    for p in projects {
        let name = p["name"].as_str().unwrap_or("");
        if name.eq_ignore_ascii_case(project) {
            if let Some(id) = p["id"].as_str() {
                return Some(id.to_string());
            }
        }
    }
    // Match by slugId
    for p in projects {
        let slug = p["slugId"].as_str().unwrap_or("");
        if slug.eq_ignore_ascii_case(project) {
            if let Some(id) = p["id"].as_str() {
                return Some(id.to_string());
            }
        }
    }
    None
}
#[derive(Clone)]
pub struct LinearClient {
    client: Client,
    api_key: String,
    retry: RetryConfig,
}

impl LinearClient {
    pub fn new() -> Result<Self> {
        let retry = default_retry_config();
        let api_key = config::get_api_key()?;
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .user_agent(format!("linear-cli/{}", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            client,
            api_key,
            retry,
        })
    }

    pub fn new_with_retry(retry_count: u32) -> Result<Self> {
        let api_key = config::get_api_key()?;
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .user_agent(format!("linear-cli/{}", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            client,
            api_key,
            retry: RetryConfig::new(retry_count),
        })
    }

    pub fn with_api_key(api_key: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .user_agent(format!("linear-cli/{}", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            client,
            api_key,
            retry: default_retry_config(),
        })
    }

    pub async fn query(&self, query: &str, variables: Option<Value>) -> Result<Value> {
        with_retry(&self.retry, || {
            let vars = variables.clone();
            async move { self.query_once(query, vars).await }
        })
        .await
    }

    async fn query_once(&self, query: &str, variables: Option<Value>) -> Result<Value> {
        let body = match variables {
            Some(vars) => json!({ "query": query, "variables": vars }),
            None => json!({ "query": query }),
        };

        let response = self
            .client
            .post(LINEAR_API_URL)
            .header("Content-Type", "application/json")
            .header("Authorization", &self.api_key)
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        let headers = response.headers().clone();

        // Check HTTP status before parsing JSON to avoid confusing errors
        if !status.is_success() {
            // Try to get error details from response body
            let body = response.text().await.unwrap_or_default();
            let details = if let Ok(json) = serde_json::from_str::<Value>(&body) {
                json
            } else {
                json!({ "body": body })
            };
            let mut err = http_error(status, &headers, "resource");
            if !body.is_empty() {
                err = err.with_details(details);
            }
            return Err(err.into());
        }

        let result: Value = response.json().await?;

        if let Some(errors) = result.get("errors") {
            return Err(CliError::general("GraphQL error")
                .with_details(errors.clone())
                .into());
        }

        Ok(result)
    }

    pub async fn mutate(&self, mutation: &str, variables: Option<Value>) -> Result<Value> {
        // Mutations are retried - Linear API is idempotent for creates/updates
        self.query(mutation, variables).await
    }

    /// Stream response bytes directly to a writer (for large downloads)
    pub async fn fetch_to_writer(
        &self,
        url: &str,
        writer: &mut impl std::io::Write,
    ) -> Result<u64> {
        let response = self
            .client
            .get(url)
            .header("Authorization", &self.api_key)
            .send()
            .await
            .context("Failed to connect to Linear uploads")?;

        let status = response.status();
        let headers = response.headers().clone();
        if !status.is_success() {
            return Err(http_error(status, &headers, "upload").into());
        }

        let mut total: u64 = 0;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Failed to read response chunk")?;
            writer.write_all(&chunk).context("Failed to write chunk")?;
            total += chunk.len() as u64;
        }
        Ok(total)
    }
}

static DEFAULT_RETRY: OnceLock<RetryConfig> = OnceLock::new();

pub fn set_default_retry(retry_count: u32) {
    let config = if retry_count == 0 {
        RetryConfig::no_retry()
    } else {
        RetryConfig::new(retry_count)
    };
    let _ = DEFAULT_RETRY.set(config);
}

fn default_retry_config() -> RetryConfig {
    DEFAULT_RETRY.get().copied().unwrap_or_default()
}
