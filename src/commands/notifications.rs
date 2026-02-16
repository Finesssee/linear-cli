use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::json;
use tabled::{Table, Tabled};

use crate::api::LinearClient;
use crate::display_options;
use crate::output::{ensure_non_empty, filter_values, print_json, sort_values, OutputOptions};
use crate::pagination::{paginate_nodes, PaginationOptions};
use crate::text::truncate;

#[derive(Subcommand)]
pub enum NotificationCommands {
    /// List unread notifications
    #[command(alias = "ls")]
    List {
        /// Include read notifications
        #[arg(short, long)]
        all: bool,
    },
    /// Mark a notification as read
    Read {
        /// Notification ID
        id: String,
    },
    /// Mark all notifications as read
    #[command(alias = "ra")]
    ReadAll,
    /// Show unread notification count
    Count,
}

#[derive(Tabled)]
struct NotificationRow {
    #[tabled(rename = "Type")]
    notification_type: String,
    #[tabled(rename = "Issue")]
    issue: String,
    #[tabled(rename = "Title")]
    title: String,
    #[tabled(rename = "Created")]
    created_at: String,
    #[tabled(rename = "ID")]
    id: String,
}

pub async fn handle(cmd: NotificationCommands, output: &OutputOptions) -> Result<()> {
    match cmd {
        NotificationCommands::List { all } => list_notifications(all, output).await,
        NotificationCommands::Read { id } => mark_as_read(&id).await,
        NotificationCommands::ReadAll => mark_all_as_read().await,
        NotificationCommands::Count => show_count(output).await,
    }
}

fn format_notification_type(notification_type: &str) -> String {
    match notification_type {
        "issueComment" => "Comment".cyan().to_string(),
        "issueMention" => "Mention".yellow().to_string(),
        "issueAssignment" => "Assigned".green().to_string(),
        "issueStatusChanged" => "Status".blue().to_string(),
        "issuePriorityChanged" => "Priority".magenta().to_string(),
        "issueNewComment" => "New Comment".cyan().to_string(),
        "issueSubscribed" => "Subscribed".dimmed().to_string(),
        "issueDue" => "Due".red().to_string(),
        "projectUpdate" => "Project".white().to_string(),
        _ => notification_type.to_string(),
    }
}

async fn list_notifications(include_all: bool, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($first: Int, $after: String, $last: Int, $before: String) {
            notifications(first: $first, after: $after, last: $last, before: $before) {
                nodes {
                    id
                    type
                    createdAt
                    readAt
                    ... on IssueNotification {
                        issue {
                            identifier
                            title
                        }
                        comment {
                            body
                        }
                        actor {
                            name
                        }
                    }
                    ... on ProjectNotification {
                        project {
                            name
                        }
                    }
                }
                pageInfo {
                    hasNextPage
                    endCursor
                    hasPreviousPage
                    startCursor
                }
            }
        }
    "#;

    let pagination = output.pagination.with_default_limit(50);
    let notifications = paginate_nodes(
        &client,
        query,
        serde_json::Map::new(),
        &["data", "notifications", "nodes"],
        &["data", "notifications", "pageInfo"],
        &pagination,
        50,
    )
    .await?;

    let mut filtered: Vec<_> = if include_all {
        notifications.clone()
    } else {
        notifications
            .iter()
            .filter(|n| n["readAt"].is_null())
            .cloned()
            .collect()
    };

    filter_values(&mut filtered, &output.filters);

    if let Some(sort_key) = output.json.sort.as_deref() {
        sort_values(&mut filtered, sort_key, output.json.order);
    }

    if output.is_json() || output.has_template() {
        print_json(&serde_json::json!(filtered), output)?;
        return Ok(());
    }

    ensure_non_empty(&filtered, output)?;
    if filtered.is_empty() {
        if include_all {
            println!("No notifications found.");
        } else {
            println!("{} No unread notifications.", "+".green());
        }
        return Ok(());
    }

    let unread_count = notifications
        .iter()
        .filter(|n| n["readAt"].is_null())
        .count();

    println!(
        "{} {} unread notification{}",
        "Notifications".bold(),
        unread_count.to_string().cyan(),
        if unread_count == 1 { "" } else { "s" }
    );
    println!("{}", "-".repeat(60));

    let width = display_options().max_width(40);
    let rows: Vec<NotificationRow> = filtered
        .iter()
        .map(|n| {
            let notification_type = n["type"].as_str().unwrap_or("unknown");
            let issue_identifier = n["issue"]["identifier"].as_str().unwrap_or("-");
            let issue_title = n["issue"]["title"].as_str().unwrap_or("");

            let truncated_title = truncate(issue_title, width);

            let created_at = n["createdAt"]
                .as_str()
                .unwrap_or("")
                .split('T')
                .next()
                .unwrap_or("-")
                .to_string();

            let id = n["id"].as_str().unwrap_or("").to_string();
            let short_id = if id.len() > 8 {
                format!("{}...", &id[..8])
            } else {
                id
            };

            NotificationRow {
                notification_type: format_notification_type(notification_type),
                issue: issue_identifier.to_string(),
                title: truncated_title,
                created_at,
                id: short_id,
            }
        })
        .collect();

    let table = Table::new(rows).to_string();
    println!("{}", table);
    println!("\n{} notifications shown", filtered.len());

    Ok(())
}

async fn mark_as_read(id: &str) -> Result<()> {
    let client = LinearClient::new()?;

    let mutation = r#"
        mutation($id: String!) {
            notificationUpdate(id: $id, input: { readAt: "now" }) {
                success
                notification {
                    id
                    readAt
                    ... on IssueNotification {
                        issue {
                            identifier
                            title
                        }
                    }
                }
            }
        }
    "#;

    let result = client.mutate(mutation, Some(json!({ "id": id }))).await?;

    if result["data"]["notificationUpdate"]["success"].as_bool() == Some(true) {
        let notification = &result["data"]["notificationUpdate"]["notification"];
        let issue_identifier = notification["issue"]["identifier"].as_str().unwrap_or("");
        let issue_title = notification["issue"]["title"].as_str().unwrap_or("");

        println!("{} Marked notification as read", "+".green());
        if !issue_identifier.is_empty() {
            println!("  Issue: {} {}", issue_identifier.cyan(), issue_title);
        }
    } else {
        anyhow::bail!("Failed to mark notification as read");
    }

    Ok(())
}


async fn mark_all_as_read() -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($first: Int, $after: String) {
            notifications(first: $first, after: $after) {
                nodes {
                    id
                    readAt
                }
                pageInfo {
                    hasNextPage
                    endCursor
                }
            }
        }
    "#;

    let pagination = PaginationOptions {
        all: true,
        ..Default::default()
    };

    let notifications = paginate_nodes(
        &client,
        query,
        serde_json::Map::new(),
        &["data", "notifications", "nodes"],
        &["data", "notifications", "pageInfo"],
        &pagination,
        100,
    )
    .await?;

    let unread: Vec<_> = notifications
        .iter()
        .filter(|n| n["readAt"].is_null())
        .filter_map(|n| n["id"].as_str())
        .collect();

    if unread.is_empty() {
        println!("{} No unread notifications to mark.", "+".green());
        return Ok(());
    }

    let count = unread.len();
    println!("Marking {} notifications as read...", count);

    let mutation = r#"
        mutation($id: String!) {
            notificationUpdate(id: $id, input: { readAt: "now" }) {
                success
            }
        }
    "#;

    // Run mutations with bounded concurrency
    use futures::stream::{self, StreamExt};
    let results: Vec<_> = stream::iter(unread.iter())
        .map(|id| {
            let client = client.clone();
            let id = id.to_string();
            async move {
                client
                    .mutate(mutation, Some(json!({ "id": id })))
                    .await
                    .is_ok()
            }
        })
        .buffer_unordered(10)
        .collect()
        .await;
    let success_count = results.iter().filter(|&&r| r).count();

    println!(
        "{} Marked {} notification{} as read",
        "+".green(),
        success_count,
        if success_count == 1 { "" } else { "s" }
    );

    if success_count < count {
        println!(
            "  {} Failed to mark {} notification{}",
            "!".yellow(),
            count - success_count,
            if count - success_count == 1 { "" } else { "s" }
        );
    }

    Ok(())
}

async fn show_count(output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($first: Int, $after: String) {
            notifications(first: $first, after: $after) {
                nodes {
                    id
                    readAt
                }
                pageInfo {
                    hasNextPage
                    endCursor
                }
            }
        }
    "#;

    let pagination = PaginationOptions {
        all: true,
        ..Default::default()
    };

    let notifications = paginate_nodes(
        &client,
        query,
        serde_json::Map::new(),
        &["data", "notifications", "nodes"],
        &["data", "notifications", "pageInfo"],
        &pagination,
        100,
    )
    .await?;

    let unread_count = notifications
        .iter()
        .filter(|n| n["readAt"].is_null())
        .count();

    if output.is_json() || output.has_template() {
        print_json(&json!({ "count": unread_count }), output)?;
        return Ok(());
    }

    if unread_count == 0 {
        println!("{} No unread notifications", "+".green());
    } else {
        println!(
            "{} {} unread notification{}",
            "!".yellow().bold(),
            unread_count.to_string().cyan().bold(),
            if unread_count == 1 { "" } else { "s" }
        );
    }

    Ok(())
}
