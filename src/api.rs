use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::{json, Value};

use crate::cache::{Cache, CacheOptions, CacheType};
use crate::config;
use crate::error::CliError;
use crate::retry::{with_retry, RetryConfig};
use crate::text::is_uuid;
use std::sync::OnceLock;

const LINEAR_API_URL: &str = "https://api.linear.app/graphql";

/// Resolves a team key (like "SCW") or name to a team UUID.
/// If the input is already a UUID (36 characters with dashes), returns it as-is.
pub async fn resolve_team_id(client: &LinearClient, team: &str, cache_opts: &CacheOptions) -> Result<String> {
    if is_uuid(team) {
        return Ok(team.to_string());
    }

    if !cache_opts.no_cache {
        let cache = Cache::new()?;
        if let Some(cached) = cache.get(CacheType::Teams).and_then(|data| data.as_array().cloned()) {
            if let Some(id) = find_team_id(&cached, team) {
                return Ok(id);
            }
        }
    }

    let query = r#"
        query($team: String!) {
            teams(first: 50, filter: { or: [{ key: { eqIgnoreCase: $team } }, { name: { eqIgnoreCase: $team } }] }) {
                nodes {
                    id
                    key
                    name
                }
            }
        }
    "#;

    let result = client.query(query, Some(json!({ "team": team }))).await?;
    let empty = vec![];
    let teams = result["data"]["teams"]["nodes"].as_array().unwrap_or(&empty);

    if let Some(id) = find_team_id(teams, team) {
        if !cache_opts.no_cache {
            let cache = Cache::with_ttl(cache_opts.effective_ttl_seconds())?;
            let _ = cache.set(CacheType::Teams, json!(teams));
        }
        return Ok(id);
    }

    let query_all = r#"
        query {
            teams(first: 500) {
                nodes {
                    id
                    key
                    name
                }
            }
        }
    "#;

    let result = client.query(query_all, None).await?;
    let teams = result["data"]["teams"]["nodes"].as_array().unwrap_or(&empty);

    if !cache_opts.no_cache {
        let cache = Cache::with_ttl(cache_opts.effective_ttl_seconds())?;
        let _ = cache.set(CacheType::Teams, json!(teams));
    }

    if let Some(id) = find_team_id(teams, team) {
        return Ok(id);
    }

    anyhow::bail!("Team not found: {}. Use linear-cli t list to see available teams.", team)
}

/// Resolve a user identifier to a UUID.
/// Handles "me", UUIDs, names, and emails.
pub async fn resolve_user_id(client: &LinearClient, user: &str, cache_opts: &CacheOptions) -> Result<String> {
    if user.eq_ignore_ascii_case("me") {
        let query = r#"
            query {
                viewer {
                    id
                }
            }
        "#;
        let result = client.query(query, None).await?;
        let user_id = result["data"]["viewer"]["id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Could not fetch current user ID"))?;
        return Ok(user_id.to_string());
    }

    if is_uuid(user) {
        return Ok(user.to_string());
    }

    if !cache_opts.no_cache {
        let cache = Cache::new()?;
        if let Some(cached) = cache.get(CacheType::Users).and_then(|data| data.as_array().cloned()) {
            if let Some(id) = find_user_id(&cached, user) {
                return Ok(id);
            }
        }
    }

    let query = r#"
        query($user: String!) {
            users(first: 50, filter: { or: [{ name: { eqIgnoreCase: $user } }, { email: { eqIgnoreCase: $user } }] }) {
                nodes {
                    id
                    name
                    email
                }
            }
        }
    "#;

    let result = client.query(query, Some(json!({ "user": user }))).await?;
    let empty = vec![];
    let users = result["data"]["users"]["nodes"].as_array().unwrap_or(&empty);

    if let Some(id) = find_user_id(users, user) {
        if !cache_opts.no_cache {
            let cache = Cache::with_ttl(cache_opts.effective_ttl_seconds())?;
            let _ = cache.set(CacheType::Users, json!(users));
        }
        return Ok(id);
    }

    let query_all = r#"
        query {
            users(first: 500) {
                nodes {
                    id
                    name
                    email
                }
            }
        }
    "#;

    let result = client.query(query_all, None).await?;
    let users = result["data"]["users"]["nodes"].as_array().unwrap_or(&empty);

    if !cache_opts.no_cache {
        let cache = Cache::with_ttl(cache_opts.effective_ttl_seconds())?;
        let _ = cache.set(CacheType::Users, json!(users));
    }

    if let Some(id) = find_user_id(users, user) {
        return Ok(id);
    }

    anyhow::bail!("User not found: {}", user)
}

/// Resolve a label name to a UUID.
pub async fn resolve_label_id(client: &LinearClient, label: &str, cache_opts: &CacheOptions) -> Result<String> {
    if is_uuid(label) {
        return Ok(label.to_string());
    }

    if !cache_opts.no_cache {
        let cache = Cache::new()?;
        if let Some(cached) = cache.get(CacheType::Labels).and_then(|data| data.as_array().cloned()) {
            if let Some(id) = find_label_id(&cached, label) {
                return Ok(id);
            }
        }
    }

    let query = r#"
        query($label: String!) {
            issueLabels(first: 50, filter: { name: { eqIgnoreCase: $label } }) {
                nodes {
                    id
                    name
                }
            }
        }
    "#;

    let result = client.query(query, Some(json!({ "label": label }))).await?;
    let empty = vec![];
    let labels = result["data"]["issueLabels"]["nodes"].as_array().unwrap_or(&empty);

    if let Some(id) = find_label_id(labels, label) {
        if !cache_opts.no_cache {
            let cache = Cache::with_ttl(cache_opts.effective_ttl_seconds())?;
            let _ = cache.set(CacheType::Labels, json!(labels));
        }
        return Ok(id);
    }

    let query_all = r#"
        query {
            issueLabels(first: 500) {
                nodes {
                    id
                    name
                }
            }
        }
    "#;

    let result = client.query(query_all, None).await?;
    let labels = result["data"]["issueLabels"]["nodes"].as_array().unwrap_or(&empty);

    if !cache_opts.no_cache {
        let cache = Cache::with_ttl(cache_opts.effective_ttl_seconds())?;
        let _ = cache.set(CacheType::Labels, json!(labels));
    }

    if let Some(id) = find_label_id(labels, label) {
        return Ok(id);
    }

    anyhow::bail!("Label not found: {}", label)
}

fn find_team_id(teams: &[Value], team: &str) -> Option<String> {
    if let Some(team_data) = teams.iter().find(|t| t["key"].as_str().map(|k| k.eq_ignore_ascii_case(team)) == Some(true)) {
        if let Some(id) = team_data["id"].as_str() {
            return Some(id.to_string());
        }
    }

    if let Some(team_data) = teams.iter().find(|t| t["name"].as_str().map(|n| n.eq_ignore_ascii_case(team)) == Some(true)) {
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
        Ok(Self {
            client: Client::new(),
            api_key,
            retry,
        })
    }

    pub fn new_with_retry(retry_count: u32) -> Result<Self> {
        let api_key = config::get_api_key()?;
        Ok(Self {
            client: Client::new(),
            api_key,
            retry: RetryConfig::new(retry_count),
        })
    }

    pub fn with_api_key(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            retry: default_retry_config(),
        }
    }

    pub async fn query(&self, query: &str, variables: Option<Value>) -> Result<Value> {
        let vars = variables.clone();
        with_retry(&self.retry, || {
            let vars = vars.clone();
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
        let result: Value = response.json().await?;

        if !status.is_success() {
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
                401 => CliError::new(3, "Authentication failed - check your API key"),
                403 => CliError::new(3, "Access denied - insufficient permissions"),
                404 => CliError::new(2, "Resource not found"),
                429 => CliError::new(4, "Rate limit exceeded").with_retry_after(retry_after),
                _ => CliError::new(
                    1,
                    format!(
                        "HTTP {} {}",
                        status.as_u16(),
                        details["reason"].as_str().unwrap_or("Unknown error")
                    ),
                ),
            };
            return Err(err.with_details(details).into());
        }

        if let Some(errors) = result.get("errors") {
            return Err(CliError::new(1, "GraphQL error")
                .with_details(errors.clone())
                .into());
        }

        Ok(result)
    }

    pub async fn mutate(&self, mutation: &str, variables: Option<Value>) -> Result<Value> {
        self.query_once(mutation, variables).await
    }

    /// Fetch raw bytes from a URL with authorization header (for Linear uploads)
    pub async fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let response = self
            .client
            .get(url)
            .header("Authorization", &self.api_key)
            .send()
            .await
            .context("Failed to connect to Linear uploads")?;

        let status = response.status();
        if !status.is_success() {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok());
            let request_id = response
                .headers()
                .get("x-request-id")
                .and_then(|v| v.to_str().ok())
                .map(|v| v.to_string());
            let details = json!({
                "status": status.as_u16(),
                "reason": status.canonical_reason().unwrap_or("Unknown error"),
                "request_id": request_id,
            });
            let err = match status.as_u16() {
                401 => CliError::new(3, "Authentication failed - check your API key"),
                403 => CliError::new(3, "Access denied to this upload"),
                404 => CliError::new(2, "Upload not found"),
                429 => CliError::new(4, "Rate limit exceeded").with_retry_after(retry_after),
                _ => CliError::new(
                    1,
                    format!(
                        "HTTP {} {}",
                        status.as_u16(),
                        details["reason"].as_str().unwrap_or("Unknown error")
                    ),
                ),
            };
            return Err(err.with_details(details).into());
        }

        let bytes: Vec<u8> = response
            .bytes()
            .await
            .context("Failed to read response body")?
            .to_vec();
        Ok(bytes)
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
