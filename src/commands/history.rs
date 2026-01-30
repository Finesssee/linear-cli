use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use tabled::{Table, Tabled};

use crate::api::LinearClient;
use crate::output::{print_json, OutputOptions};
use crate::text::truncate;
use crate::DISPLAY_OPTIONS;

#[derive(Subcommand, Debug)]
pub enum HistoryCommands {
    /// Show issue activity history
    Issue {
        /// Issue identifier (e.g., LIN-123)
        id: String,
        /// Limit number of entries
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },
}

#[derive(Tabled)]
struct HistoryRow {
    #[tabled(rename = "Date")]
    date: String,
    #[tabled(rename = "Actor")]
    actor: String,
    #[tabled(rename = "Action")]
    action: String,
    #[tabled(rename = "Details")]
    details: String,
}

pub async fn handle(cmd: HistoryCommands, output: &OutputOptions) -> Result<()> {
    match cmd {
        HistoryCommands::Issue { id, limit } => issue_history(&id, limit, output).await,
    }
}

async fn issue_history(id: &str, limit: usize, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    // First get the issue ID if identifier provided
    let issue_query = r#"
        query($id: String!) {
            issue(id: $id) {
                id
                identifier
                title
                history(first: 50) {
                    nodes {
                        id
                        createdAt
                        actor { name }
                        fromState { name }
                        toState { name }
                        fromAssignee { name }
                        toAssignee { name }
                        fromPriority
                        toPriority
                        fromEstimate
                        toEstimate
                        addedLabels { nodes { name } }
                        removedLabels { nodes { name } }
                        relationChanges {
                            type
                            issue { identifier }
                        }
                    }
                }
            }
        }
    "#;

    let result = client.query(issue_query, Some(json!({ "id": id }))).await?;
    let issue = &result["data"]["issue"];

    if issue.is_null() {
        anyhow::bail!("Issue not found: {}", id);
    }

    let history = issue["history"]["nodes"].as_array().cloned().unwrap_or_default();

    if output.is_json() {
        print_json(&json!({
            "issue": {
                "id": issue["id"],
                "identifier": issue["identifier"],
                "title": issue["title"],
            },
            "history": history.iter().take(limit).collect::<Vec<_>>()
        }), output)?;
    } else {
        let display = DISPLAY_OPTIONS.get().cloned().unwrap_or_default();
        let max_width = display.max_width(30);

        println!(
            "History for {} - {}\n",
            issue["identifier"].as_str().unwrap_or(id),
            issue["title"].as_str().unwrap_or("")
        );

        let rows: Vec<HistoryRow> = history
            .iter()
            .take(limit)
            .map(|h| {
                let mut action = String::new();
                let mut details = String::new();

                // State change
                if !h["fromState"].is_null() || !h["toState"].is_null() {
                    action = "Status".to_string();
                    details = format!(
                        "{} -> {}",
                        h["fromState"]["name"].as_str().unwrap_or("-"),
                        h["toState"]["name"].as_str().unwrap_or("-")
                    );
                }
                // Assignee change
                else if !h["fromAssignee"].is_null() || !h["toAssignee"].is_null() {
                    action = "Assignee".to_string();
                    details = format!(
                        "{} -> {}",
                        h["fromAssignee"]["name"].as_str().unwrap_or("Unassigned"),
                        h["toAssignee"]["name"].as_str().unwrap_or("Unassigned")
                    );
                }
                // Priority change
                else if !h["fromPriority"].is_null() || !h["toPriority"].is_null() {
                    action = "Priority".to_string();
                    details = format!(
                        "{} -> {}",
                        h["fromPriority"].as_i64().unwrap_or(0),
                        h["toPriority"].as_i64().unwrap_or(0)
                    );
                }
                // Estimate change
                else if !h["fromEstimate"].is_null() || !h["toEstimate"].is_null() {
                    action = "Estimate".to_string();
                    details = format!(
                        "{} -> {}",
                        h["fromEstimate"].as_f64().unwrap_or(0.0),
                        h["toEstimate"].as_f64().unwrap_or(0.0)
                    );
                }
                // Labels added
                else if let Some(labels) = h["addedLabels"]["nodes"].as_array() {
                    if !labels.is_empty() {
                        action = "Labels +".to_string();
                        details = labels
                            .iter()
                            .filter_map(|l| l["name"].as_str())
                            .collect::<Vec<_>>()
                            .join(", ");
                    }
                }
                // Labels removed
                else if let Some(labels) = h["removedLabels"]["nodes"].as_array() {
                    if !labels.is_empty() {
                        action = "Labels -".to_string();
                        details = labels
                            .iter()
                            .filter_map(|l| l["name"].as_str())
                            .collect::<Vec<_>>()
                            .join(", ");
                    }
                }

                if action.is_empty() {
                    action = "Update".to_string();
                }

                HistoryRow {
                    date: h["createdAt"]
                        .as_str()
                        .unwrap_or("-")
                        .chars()
                        .take(16)
                        .collect::<String>()
                        .replace('T', " "),
                    actor: truncate(
                        h["actor"]["name"].as_str().unwrap_or("System"),
                        max_width,
                    ),
                    action,
                    details: truncate(&details, max_width),
                }
            })
            .filter(|r| !r.action.is_empty())
            .collect();

        if rows.is_empty() {
            println!("No history found");
        } else {
            println!("{}", Table::new(rows));
        }
    }

    Ok(())
}
