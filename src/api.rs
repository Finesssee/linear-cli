use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::{json, Value};

use crate::config;
use crate::error::CliError;

const LINEAR_API_URL: &str = "https://api.linear.app/graphql";

/// Resolves a team key (like "SCW") or name to a team UUID.
/// If the input is already a UUID (36 characters with dashes), returns it as-is.
pub async fn resolve_team_id(client: &LinearClient, team: &str) -> Result<String> {
    // If already a UUID (36 chars with dashes pattern), return as-is
    if team.len() == 36 && team.chars().filter(|c| *c == '-').count() == 4 {
        return Ok(team.to_string());
    }

    // Query to find team by key or name
    let query = r#"
        query {
            teams(first: 100) {
                nodes {
                    id
                    key
                    name
                }
            }
        }
    "#;

    let result = client.query(query, None).await?;
    let empty = vec![];
    let teams = result["data"]["teams"]["nodes"]
        .as_array()
        .unwrap_or(&empty);

    // First try exact key match (case-insensitive)
    if let Some(team_data) = teams
        .iter()
        .find(|t| t["key"].as_str().map(|k| k.eq_ignore_ascii_case(team)) == Some(true))
    {
        if let Some(id) = team_data["id"].as_str() {
            return Ok(id.to_string());
        }
    }

    // Then try exact name match (case-insensitive)
    if let Some(team_data) = teams
        .iter()
        .find(|t| t["name"].as_str().map(|n| n.eq_ignore_ascii_case(team)) == Some(true))
    {
        if let Some(id) = team_data["id"].as_str() {
            return Ok(id.to_string());
        }
    }

    anyhow::bail!(
        "Team not found: '{}'. Use 'linear-cli t list' to see available teams.",
        team
    )
}

#[derive(Clone)]
pub struct LinearClient {
    client: Client,
    api_key: String,
}

impl LinearClient {
    pub fn new() -> Result<Self> {
        let api_key = config::get_api_key()?;
        Ok(Self {
            client: Client::new(),
            api_key,
        })
    }

    pub async fn query(&self, query: &str, variables: Option<Value>) -> Result<Value> {
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
            let details = json!({
                "status": status.as_u16(),
                "reason": status.canonical_reason().unwrap_or("Unknown error"),
            });
            let err = match status.as_u16() {
                401 => CliError::new(3, "Authentication failed - check your API key"),
                403 => CliError::new(3, "Access denied - insufficient permissions"),
                404 => CliError::new(2, "Resource not found"),
                429 => CliError::new(4, "Rate limit exceeded").with_retry_after(retry_after),
                _ => CliError::new(1, format!("HTTP {} {}", status.as_u16(), details["reason"].as_str().unwrap_or("Unknown error"))),
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
        self.query(mutation, variables).await
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
            let details = json!({
                "status": status.as_u16(),
                "reason": status.canonical_reason().unwrap_or("Unknown error"),
            });
            let err = match status.as_u16() {
                401 => CliError::new(3, "Authentication failed - check your API key"),
                403 => CliError::new(3, "Access denied to this upload"),
                404 => CliError::new(2, "Upload not found"),
                429 => CliError::new(4, "Rate limit exceeded").with_retry_after(retry_after),
                _ => CliError::new(1, format!("HTTP {} {}", status.as_u16(), details["reason"].as_str().unwrap_or("Unknown error"))),
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
