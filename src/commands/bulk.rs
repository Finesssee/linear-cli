use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use futures::future::join_all;
use serde_json::json;

use crate::api::LinearClient;

#[derive(Subcommand)]
pub enum BulkCommands {
    /// Update the state of multiple issues
    #[command(alias = "state")]
    #[command(after_help = r#"EXAMPLES:
    linear bulk update-state Done -i LIN-1,LIN-2,LIN-3
    linear b state "In Progress" -i LIN-1,LIN-2"#)]
    UpdateState {
        /// The new state name or ID
        state: String,
        /// Comma-separated list of issue IDs (e.g., "LIN-1,LIN-2,LIN-3")
        #[arg(short, long, value_delimiter = ',')]
        issues: Vec<String>,
    },
    /// Assign multiple issues to a user
    #[command(after_help = r#"EXAMPLES:
    linear bulk assign me -i LIN-1,LIN-2,LIN-3
    linear b assign john@example.com -i LIN-1,LIN-2"#)]
    Assign {
        /// The user to assign (user ID, name, email, or "me")
        user: String,
        /// Comma-separated list of issue IDs (e.g., "LIN-1,LIN-2,LIN-3")
        #[arg(short, long, value_delimiter = ',')]
        issues: Vec<String>,
    },
    /// Add a label to multiple issues
    #[command(after_help = r#"EXAMPLES:
    linear bulk label "Bug" -i LIN-1,LIN-2,LIN-3
    linear b label LABEL_ID -i LIN-1,LIN-2"#)]
    Label {
        /// The label name or ID to add
        label: String,
        /// Comma-separated list of issue IDs (e.g., "LIN-1,LIN-2,LIN-3")
        #[arg(short, long, value_delimiter = ',')]
        issues: Vec<String>,
    },
    /// Unassign multiple issues
    #[command(after_help = r#"EXAMPLES:
    linear bulk unassign -i LIN-1,LIN-2,LIN-3"#)]
    Unassign {
        /// Comma-separated list of issue IDs (e.g., "LIN-1,LIN-2,LIN-3")
        #[arg(short, long, value_delimiter = ',')]
        issues: Vec<String>,
    },
}

/// Result of a single bulk operation
#[derive(Debug)]
struct BulkResult {
    issue_id: String,
    success: bool,
    identifier: Option<String>,
    error: Option<String>,
}

/// Check if a string looks like a UUID (contains dashes and is 36 characters)
fn is_uuid(s: &str) -> bool {
    s.len() == 36 && s.chars().filter(|c| *c == '-').count() == 4
}

/// Resolve a user identifier to a UUID.
/// Handles "me", UUIDs, names, and emails.
async fn resolve_user_id(client: &LinearClient, user: &str) -> Result<String> {
    // Handle "me" - get the current viewer's ID
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

    // If already a UUID, return as-is
    if is_uuid(user) {
        return Ok(user.to_string());
    }

    // Try to find user by name or email
    let query = r#"
        query {
            users(first: 100) {
                nodes {
                    id
                    name
                    email
                }
            }
        }
    "#;

    let result = client.query(query, None).await?;
    let empty = vec![];
    let users = result["data"]["users"]["nodes"]
        .as_array()
        .unwrap_or(&empty);

    // Try to match by name (case-insensitive) or email
    for u in users {
        let name = u["name"].as_str().unwrap_or("");
        let email = u["email"].as_str().unwrap_or("");

        if name.eq_ignore_ascii_case(user) || email.eq_ignore_ascii_case(user) {
            if let Some(id) = u["id"].as_str() {
                return Ok(id.to_string());
            }
        }
    }

    anyhow::bail!("User not found: {}", user)
}

/// Resolve a state name to a UUID for a given team.
async fn resolve_state_id(client: &LinearClient, team_id: &str, state: &str) -> Result<String> {
    // If already a UUID, return as-is
    if is_uuid(state) {
        return Ok(state.to_string());
    }

    // Fetch team states
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

    // Try to match by name (case-insensitive)
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

/// Resolve a label name to a UUID.
async fn resolve_label_id(client: &LinearClient, label: &str) -> Result<String> {
    // If already a UUID, return as-is
    if is_uuid(label) {
        return Ok(label.to_string());
    }

    // Fetch all labels
    let query = r#"
        query {
            issueLabels(first: 250) {
                nodes {
                    id
                    name
                }
            }
        }
    "#;

    let result = client.query(query, None).await?;
    let empty = vec![];
    let labels = result["data"]["issueLabels"]["nodes"]
        .as_array()
        .unwrap_or(&empty);

    // Try to match by name (case-insensitive)
    for l in labels {
        let name = l["name"].as_str().unwrap_or("");
        if name.eq_ignore_ascii_case(label) {
            if let Some(id) = l["id"].as_str() {
                return Ok(id.to_string());
            }
        }
    }

    anyhow::bail!("Label not found: {}", label)
}

/// Get issue details including UUID and team ID from identifier (e.g., "LIN-123")
async fn get_issue_info(
    client: &LinearClient,
    issue_id: &str,
) -> Result<(String, String, Option<String>)> {
    let query = r#"
        query($id: String!) {
            issue(id: $id) {
                id
                identifier
                team {
                    id
                }
            }
        }
    "#;

    let result = client.query(query, Some(json!({ "id": issue_id }))).await?;
    let issue = &result["data"]["issue"];

    if issue.is_null() {
        anyhow::bail!("Issue not found: {}", issue_id);
    }

    let uuid = issue["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Failed to get issue ID"))?
        .to_string();

    let team_id = issue["team"]["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Failed to get team ID"))?
        .to_string();

    let identifier = issue["identifier"].as_str().map(|s| s.to_string());

    Ok((uuid, team_id, identifier))
}

pub async fn handle(cmd: BulkCommands) -> Result<()> {
    match cmd {
        BulkCommands::UpdateState { state, issues } => bulk_update_state(&state, issues).await,
        BulkCommands::Assign { user, issues } => bulk_assign(&user, issues).await,
        BulkCommands::Label { label, issues } => bulk_label(&label, issues).await,
        BulkCommands::Unassign { issues } => bulk_unassign(issues).await,
    }
}

async fn bulk_update_state(state: &str, issues: Vec<String>) -> Result<()> {
    if issues.is_empty() {
        println!("No issues specified.");
        return Ok(());
    }

    println!(
        "{} Updating state to '{}' for {} issues...",
        ">>".cyan(),
        state,
        issues.len()
    );

    let client = LinearClient::new()?;
    let state_owned = state.to_string();

    let futures: Vec<_> = issues
        .iter()
        .map(|issue_id| {
            let client = &client;
            let state = &state_owned;
            let id = issue_id.clone();
            async move { update_issue_state(client, &id, state).await }
        })
        .collect();

    let results = join_all(futures).await;
    print_summary(&results, "state updated");

    Ok(())
}

async fn bulk_assign(user: &str, issues: Vec<String>) -> Result<()> {
    if issues.is_empty() {
        println!("No issues specified.");
        return Ok(());
    }

    println!(
        "{} Assigning {} issues to '{}'...",
        ">>".cyan(),
        issues.len(),
        user
    );

    let client = LinearClient::new()?;

    // Resolve the user ID once upfront
    let user_id = match resolve_user_id(&client, user).await {
        Ok(id) => id,
        Err(e) => {
            println!("{} Failed to resolve user '{}': {}", "x".red(), user, e);
            return Ok(());
        }
    };

    let futures: Vec<_> = issues
        .iter()
        .map(|issue_id| {
            let client = &client;
            let user_id = &user_id;
            let id = issue_id.clone();
            async move { update_issue_assignee(client, &id, Some(user_id)).await }
        })
        .collect();

    let results = join_all(futures).await;
    print_summary(&results, "assigned");

    Ok(())
}

async fn bulk_label(label: &str, issues: Vec<String>) -> Result<()> {
    if issues.is_empty() {
        println!("No issues specified.");
        return Ok(());
    }

    println!(
        "{} Adding label '{}' to {} issues...",
        ">>".cyan(),
        label,
        issues.len()
    );

    let client = LinearClient::new()?;

    // Resolve the label ID once upfront
    let label_id = match resolve_label_id(&client, label).await {
        Ok(id) => id,
        Err(e) => {
            println!("{} Failed to resolve label '{}': {}", "x".red(), label, e);
            return Ok(());
        }
    };

    let futures: Vec<_> = issues
        .iter()
        .map(|issue_id| {
            let client = &client;
            let label_id = &label_id;
            let id = issue_id.clone();
            async move { add_label_to_issue(client, &id, label_id).await }
        })
        .collect();

    let results = join_all(futures).await;
    print_summary(&results, "labeled");

    Ok(())
}

async fn bulk_unassign(issues: Vec<String>) -> Result<()> {
    if issues.is_empty() {
        println!("No issues specified.");
        return Ok(());
    }

    println!("{} Unassigning {} issues...", ">>".cyan(), issues.len());

    let client = LinearClient::new()?;

    let futures: Vec<_> = issues
        .iter()
        .map(|issue_id| {
            let client = &client;
            let id = issue_id.clone();
            async move { update_issue_assignee(client, &id, None).await }
        })
        .collect();

    let results = join_all(futures).await;
    print_summary(&results, "unassigned");

    Ok(())
}

async fn update_issue_state(client: &LinearClient, issue_id: &str, state: &str) -> BulkResult {
    // First, get issue UUID and team ID
    let (uuid, team_id, identifier) = match get_issue_info(client, issue_id).await {
        Ok(info) => info,
        Err(e) => {
            return BulkResult {
                issue_id: issue_id.to_string(),
                success: false,
                identifier: None,
                error: Some(e.to_string()),
            };
        }
    };

    // Resolve state name to UUID for this team
    let state_id = match resolve_state_id(client, &team_id, state).await {
        Ok(id) => id,
        Err(e) => {
            return BulkResult {
                issue_id: issue_id.to_string(),
                success: false,
                identifier,
                error: Some(e.to_string()),
            };
        }
    };

    let mutation = r#"
        mutation($id: String!, $input: IssueUpdateInput!) {
            issueUpdate(id: $id, input: $input) {
                success
                issue {
                    identifier
                    title
                }
            }
        }
    "#;

    let input = json!({ "stateId": state_id });

    match client
        .mutate(mutation, Some(json!({ "id": uuid, "input": input })))
        .await
    {
        Ok(result) => {
            if result["data"]["issueUpdate"]["success"].as_bool() == Some(true) {
                let identifier = result["data"]["issueUpdate"]["issue"]["identifier"]
                    .as_str()
                    .map(|s| s.to_string());
                BulkResult {
                    issue_id: issue_id.to_string(),
                    success: true,
                    identifier,
                    error: None,
                }
            } else {
                BulkResult {
                    issue_id: issue_id.to_string(),
                    success: false,
                    identifier: None,
                    error: Some("Update failed".to_string()),
                }
            }
        }
        Err(e) => BulkResult {
            issue_id: issue_id.to_string(),
            success: false,
            identifier: None,
            error: Some(e.to_string()),
        },
    }
}

async fn update_issue_assignee(
    client: &LinearClient,
    issue_id: &str,
    assignee_id: Option<&str>,
) -> BulkResult {
    // First, get issue UUID
    let (uuid, _team_id, identifier) = match get_issue_info(client, issue_id).await {
        Ok(info) => info,
        Err(e) => {
            return BulkResult {
                issue_id: issue_id.to_string(),
                success: false,
                identifier: None,
                error: Some(e.to_string()),
            };
        }
    };

    let mutation = r#"
        mutation($id: String!, $input: IssueUpdateInput!) {
            issueUpdate(id: $id, input: $input) {
                success
                issue {
                    identifier
                    title
                }
            }
        }
    "#;

    let input = match assignee_id {
        Some(id) => json!({ "assigneeId": id }),
        None => json!({ "assigneeId": null }),
    };

    match client
        .mutate(mutation, Some(json!({ "id": uuid, "input": input })))
        .await
    {
        Ok(result) => {
            if result["data"]["issueUpdate"]["success"].as_bool() == Some(true) {
                let identifier = result["data"]["issueUpdate"]["issue"]["identifier"]
                    .as_str()
                    .map(|s| s.to_string())
                    .or(identifier);
                BulkResult {
                    issue_id: issue_id.to_string(),
                    success: true,
                    identifier,
                    error: None,
                }
            } else {
                BulkResult {
                    issue_id: issue_id.to_string(),
                    success: false,
                    identifier,
                    error: Some("Update failed".to_string()),
                }
            }
        }
        Err(e) => BulkResult {
            issue_id: issue_id.to_string(),
            success: false,
            identifier,
            error: Some(e.to_string()),
        },
    }
}

async fn add_label_to_issue(client: &LinearClient, issue_id: &str, label_id: &str) -> BulkResult {
    // First, get existing labels for the issue (using the issue identifier/UUID)
    let query = r#"
        query($id: String!) {
            issue(id: $id) {
                id
                identifier
                labels {
                    nodes {
                        id
                    }
                }
            }
        }
    "#;

    let (uuid, identifier, existing_label_ids) =
        match client.query(query, Some(json!({ "id": issue_id }))).await {
            Ok(result) => {
                if result["data"]["issue"].is_null() {
                    return BulkResult {
                        issue_id: issue_id.to_string(),
                        success: false,
                        identifier: None,
                        error: Some("Issue not found".to_string()),
                    };
                }

                let uuid = result["data"]["issue"]["id"]
                    .as_str()
                    .unwrap_or(issue_id)
                    .to_string();

                let identifier = result["data"]["issue"]["identifier"]
                    .as_str()
                    .map(|s| s.to_string());

                let labels: Vec<String> = result["data"]["issue"]["labels"]["nodes"]
                    .as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .filter_map(|l| l["id"].as_str().map(|s| s.to_string()))
                    .collect();

                (uuid, identifier, labels)
            }
            Err(e) => {
                return BulkResult {
                    issue_id: issue_id.to_string(),
                    success: false,
                    identifier: None,
                    error: Some(e.to_string()),
                };
            }
        };

    let mut label_ids = existing_label_ids;

    // Add the new label if not already present
    if !label_ids.contains(&label_id.to_string()) {
        label_ids.push(label_id.to_string());
    }

    // Update the issue with the new label list
    let mutation = r#"
        mutation($id: String!, $input: IssueUpdateInput!) {
            issueUpdate(id: $id, input: $input) {
                success
                issue {
                    identifier
                }
            }
        }
    "#;

    let input = json!({ "labelIds": label_ids });

    match client
        .mutate(mutation, Some(json!({ "id": uuid, "input": input })))
        .await
    {
        Ok(result) => {
            if result["data"]["issueUpdate"]["success"].as_bool() == Some(true) {
                let identifier = result["data"]["issueUpdate"]["issue"]["identifier"]
                    .as_str()
                    .map(|s| s.to_string())
                    .or(identifier);
                BulkResult {
                    issue_id: issue_id.to_string(),
                    success: true,
                    identifier,
                    error: None,
                }
            } else {
                BulkResult {
                    issue_id: issue_id.to_string(),
                    success: false,
                    identifier,
                    error: Some("Update failed".to_string()),
                }
            }
        }
        Err(e) => BulkResult {
            issue_id: issue_id.to_string(),
            success: false,
            identifier,
            error: Some(e.to_string()),
        },
    }
}

fn print_summary(results: &[BulkResult], action: &str) {
    println!();

    let success_count = results.iter().filter(|r| r.success).count();
    let failure_count = results.len() - success_count;

    // Print individual results
    for result in results {
        if result.success {
            let display_id = result.identifier.as_deref().unwrap_or(&result.issue_id);
            println!("  {} {} {}", "+".green(), display_id.cyan(), action);
        } else {
            let error_msg = result.error.as_deref().unwrap_or("Unknown error");
            println!(
                "  {} {} failed: {}",
                "x".red(),
                result.issue_id.cyan(),
                error_msg.dimmed()
            );
        }
    }

    // Print summary
    println!();
    println!(
        "{} Summary: {} succeeded, {} failed",
        ">>".cyan(),
        success_count.to_string().green(),
        if failure_count > 0 {
            failure_count.to_string().red().to_string()
        } else {
            failure_count.to_string()
        }
    );
}
