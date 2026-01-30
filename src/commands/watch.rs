use anyhow::Result;
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;

use crate::api::LinearClient;
use crate::output::{print_json, OutputOptions};

/// Watch for changes to an issue and print updates
pub async fn watch_issue(id: &str, interval_secs: u64, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($id: String!) {
            issue(id: $id) {
                id
                identifier
                title
                updatedAt
                state { name }
                assignee { name }
                priority
                labels { nodes { name } }
            }
        }
    "#;

    let mut last_updated: Option<String> = None;
    let mut iteration = 0;

    eprintln!("Watching {} for changes (Ctrl+C to stop)...\n", id);

    loop {
        let result = client.query(query, Some(json!({ "id": id }))).await?;
        let issue = &result["data"]["issue"];

        if issue.is_null() {
            anyhow::bail!("Issue not found: {}", id);
        }

        let current_updated = issue["updatedAt"].as_str().map(|s| s.to_string());

        // Check if updated
        if last_updated.as_ref() != current_updated.as_ref() {
            if iteration > 0 {
                // Not the first iteration, so this is a change
                if output.is_json() {
                    print_json(&json!({
                        "event": "updated",
                        "issue": issue,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    }), output)?;
                } else {
                    println!(
                        "[{}] {} updated - Status: {}, Assignee: {}",
                        chrono::Utc::now().format("%H:%M:%S"),
                        issue["identifier"].as_str().unwrap_or(id),
                        issue["state"]["name"].as_str().unwrap_or("-"),
                        issue["assignee"]["name"].as_str().unwrap_or("Unassigned"),
                    );
                }
            } else if output.is_json() {
                print_json(&json!({
                    "event": "initial",
                    "issue": issue,
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                }), output)?;
            } else {
                println!(
                    "Initial state: {} - {}",
                    issue["identifier"].as_str().unwrap_or(id),
                    issue["title"].as_str().unwrap_or("")
                );
                println!(
                    "  Status: {}, Assignee: {}",
                    issue["state"]["name"].as_str().unwrap_or("-"),
                    issue["assignee"]["name"].as_str().unwrap_or("Unassigned"),
                );
            }

            last_updated = current_updated;
        }

        iteration += 1;
        sleep(Duration::from_secs(interval_secs)).await;
    }
}

/// Watch for new issues matching a filter
pub async fn watch_issues(
    team: Option<&str>,
    interval_secs: u64,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($filter: IssueFilter) {
            issues(first: 10, filter: $filter, orderBy: createdAt) {
                nodes {
                    id
                    identifier
                    title
                    createdAt
                    state { name }
                    assignee { name }
                }
            }
        }
    "#;

    let mut filter = json!({});
    if let Some(t) = team {
        filter["team"] = json!({ "key": { "eq": t } });
    }

    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut first_run = true;

    eprintln!("Watching for new issues (Ctrl+C to stop)...\n");

    loop {
        let result = client
            .query(query, Some(json!({ "filter": filter })))
            .await?;
        let issues = result["data"]["issues"]["nodes"]
            .as_array()
            .cloned()
            .unwrap_or_default();

        for issue in &issues {
            let id = issue["id"].as_str().unwrap_or("").to_string();
            if !seen_ids.contains(&id) {
                if !first_run {
                    if output.is_json() {
                        print_json(&json!({
                            "event": "new_issue",
                            "issue": issue,
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                        }), output)?;
                    } else {
                        println!(
                            "[{}] NEW: {} - {}",
                            chrono::Utc::now().format("%H:%M:%S"),
                            issue["identifier"].as_str().unwrap_or("-"),
                            issue["title"].as_str().unwrap_or("")
                        );
                    }
                }
                seen_ids.insert(id);
            }
        }

        first_run = false;
        sleep(Duration::from_secs(interval_secs)).await;
    }
}
