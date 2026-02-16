use anyhow::Result;
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;

use crate::api::LinearClient;
use crate::output::{print_json_owned, OutputOptions};

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
                    print_json_owned(
                        json!({
                            "event": "updated",
                            "issue": issue,
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                        }),
                        output,
                    )?;
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
                print_json_owned(
                    json!({
                        "event": "initial",
                        "issue": issue,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    }),
                    output,
                )?;
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

/// Watch for changes to a project and print updates
pub async fn watch_project(id: &str, interval_secs: u64, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($id: String!) {
            project(id: $id) {
                id
                name
                state
                progress
                updatedAt
                teams { nodes { key } }
            }
        }
    "#;

    let mut last_updated: Option<String> = None;
    let mut iteration = 0;

    eprintln!("Watching project {} for changes (Ctrl+C to stop)...\n", id);

    loop {
        let result = client.query(query, Some(json!({ "id": id }))).await?;
        let project = &result["data"]["project"];

        if project.is_null() {
            anyhow::bail!("Project not found: {}", id);
        }

        let current_updated = project["updatedAt"].as_str().map(|s| s.to_string());

        if last_updated.as_ref() != current_updated.as_ref() {
            if iteration > 0 {
                if output.is_json() {
                    print_json_owned(
                        json!({
                            "event": "updated",
                            "project": project,
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                        }),
                        output,
                    )?;
                } else {
                    println!(
                        "[{}] {} updated - State: {}, Progress: {:.0}%",
                        chrono::Utc::now().format("%H:%M:%S"),
                        project["name"].as_str().unwrap_or(id),
                        project["state"].as_str().unwrap_or("-"),
                        project["progress"].as_f64().unwrap_or(0.0) * 100.0,
                    );
                }
            } else if output.is_json() {
                print_json_owned(
                    json!({
                        "event": "initial",
                        "project": project,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    }),
                    output,
                )?;
            } else {
                println!(
                    "Initial state: {} - {}",
                    project["name"].as_str().unwrap_or(id),
                    project["state"].as_str().unwrap_or("")
                );
                println!(
                    "  Progress: {:.0}%",
                    project["progress"].as_f64().unwrap_or(0.0) * 100.0,
                );
            }

            last_updated = current_updated;
        }

        iteration += 1;
        sleep(Duration::from_secs(interval_secs)).await;
    }
}

/// Watch for changes to a team and print updates
pub async fn watch_team(team: &str, interval_secs: u64, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($id: String!) {
            team(id: $id) {
                id
                name
                key
                updatedAt
                activeCycle {
                    id
                    name
                    progress
                }
                issues(first: 0) {
                    __typename
                }
            }
        }
    "#;

    // Resolve team key to UUID
    let team_id = crate::api::resolve_team_id(&client, team, &output.cache).await?;

    let mut last_updated: Option<String> = None;
    let mut iteration = 0;

    eprintln!("Watching team {} for changes (Ctrl+C to stop)...\n", team);

    loop {
        let result = client.query(query, Some(json!({ "id": team_id }))).await?;
        let team_data = &result["data"]["team"];

        if team_data.is_null() {
            anyhow::bail!("Team not found: {}", team);
        }

        let current_updated = team_data["updatedAt"].as_str().map(|s| s.to_string());

        if last_updated.as_ref() != current_updated.as_ref() {
            if iteration > 0 {
                if output.is_json() {
                    print_json_owned(
                        json!({
                            "event": "updated",
                            "team": team_data,
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                        }),
                        output,
                    )?;
                } else {
                    let cycle_name = team_data["activeCycle"]["name"].as_str().unwrap_or("-");
                    let cycle_progress =
                        team_data["activeCycle"]["progress"].as_f64().unwrap_or(0.0);
                    println!(
                        "[{}] {} updated - Cycle: {} ({:.0}%)",
                        chrono::Utc::now().format("%H:%M:%S"),
                        team_data["name"].as_str().unwrap_or(team),
                        cycle_name,
                        cycle_progress * 100.0,
                    );
                }
            } else if output.is_json() {
                print_json_owned(
                    json!({
                        "event": "initial",
                        "team": team_data,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                    }),
                    output,
                )?;
            } else {
                let cycle_name = team_data["activeCycle"]["name"].as_str().unwrap_or("none");
                println!(
                    "Initial state: {} ({})",
                    team_data["name"].as_str().unwrap_or(team),
                    team_data["key"].as_str().unwrap_or(""),
                );
                println!("  Active cycle: {}", cycle_name);
            }

            last_updated = current_updated;
        }

        iteration += 1;
        sleep(Duration::from_secs(interval_secs)).await;
    }
}
