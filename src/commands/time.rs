use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::json;
use tabled::{Table, Tabled};

use crate::api::LinearClient;
use crate::display_options;
use crate::output::{print_json, OutputOptions};
use crate::text::truncate;

#[derive(Subcommand)]
pub enum TimeCommands {
    /// Log time spent on an issue
    #[command(after_help = r#"EXAMPLES:
    linear time log LIN-123 2h                 # Log 2 hours
    linear time log LIN-123 30m                # Log 30 minutes
    linear time log LIN-123 1h30m              # Log 1.5 hours"#)]
    Log {
        /// Issue ID or identifier (e.g., "LIN-123")
        issue: String,
        /// Time spent (e.g., "2h", "30m", "1h30m")
        duration: String,
        /// Optional description of work done
        #[arg(short, long)]
        description: Option<String>,
    },
    /// List time entries
    #[command(after_help = r#"EXAMPLES:
    linear time list                           # List recent time entries
    linear time list -i LIN-123                # List for specific issue
    linear time list --output json             # Output as JSON"#)]
    List {
        /// Filter by issue ID or identifier
        #[arg(short, long)]
        issue: Option<String>,
        /// Maximum number of entries to show
        #[arg(short, long, default_value = "20")]
        limit: u32,
    },
    /// Delete a time entry
    Delete {
        /// Time entry ID
        id: String,
        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
    },
}

#[derive(Tabled)]
struct TimeEntryRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Issue")]
    issue: String,
    #[tabled(rename = "Duration")]
    duration: String,
    #[tabled(rename = "Date")]
    date: String,
    #[tabled(rename = "User")]
    user: String,
}

pub async fn handle(cmd: TimeCommands, output: &OutputOptions) -> Result<()> {
    match cmd {
        TimeCommands::Log {
            issue,
            duration,
            description,
        } => log_time(&issue, &duration, description).await,
        TimeCommands::List { issue, limit } => list_time_entries(issue, limit, output).await,
        TimeCommands::Delete { id, force } => delete_time_entry(&id, force).await,
    }
}

/// Parse duration string like "2h", "30m", "1h30m" into minutes
fn parse_duration(duration: &str) -> Result<i32> {
    let duration = duration.to_lowercase();
    let mut total_minutes = 0;
    let mut current_num = String::new();

    for c in duration.chars() {
        if c.is_ascii_digit() {
            current_num.push(c);
        } else if c == 'h' {
            let hours: i32 = current_num.parse().unwrap_or(0);
            total_minutes += hours * 60;
            current_num.clear();
        } else if c == 'm' {
            let minutes: i32 = current_num.parse().unwrap_or(0);
            total_minutes += minutes;
            current_num.clear();
        }
    }

    // If just a number, treat as minutes
    if !current_num.is_empty() {
        total_minutes += current_num.parse::<i32>().unwrap_or(0);
    }

    if total_minutes == 0 {
        anyhow::bail!("Invalid duration format. Use format like '2h', '30m', or '1h30m'");
    }

    Ok(total_minutes)
}

/// Format minutes into human-readable duration
fn format_duration(minutes: i32) -> String {
    let hours = minutes / 60;
    let mins = minutes % 60;
    if hours > 0 && mins > 0 {
        format!("{}h {}m", hours, mins)
    } else if hours > 0 {
        format!("{}h", hours)
    } else {
        format!("{}m", mins)
    }
}

async fn log_time(issue_id: &str, duration: &str, description: Option<String>) -> Result<()> {
    let minutes = parse_duration(duration)?;
    let client = LinearClient::new()?;

    // First, resolve the issue to get its UUID
    let query = r#"
        query($id: String!) {
            issue(id: $id) {
                id
                identifier
                title
            }
        }
    "#;

    let result = client.query(query, Some(json!({ "id": issue_id }))).await?;
    let issue = &result["data"]["issue"];

    if issue.is_null() {
        anyhow::bail!("Issue not found: {}", issue_id);
    }

    let issue_uuid = issue["id"].as_str().unwrap_or("");
    let identifier = issue["identifier"].as_str().unwrap_or(issue_id);
    let title = issue["title"].as_str().unwrap_or("");

    // Create time entry using timeScheduleCreate mutation
    // Note: Linear's API uses timeScheduleCreate for logging time
    let mutation = r#"
        mutation CreateTimeEntry($issueId: String!, $duration: Int!, $description: String) {
            timeScheduleCreate(
                input: {
                    issueId: $issueId
                    duration: $duration
                    description: $description
                }
            ) {
                success
                timeSchedule {
                    id
                }
            }
        }
    "#;

    let variables = json!({
        "issueId": issue_uuid,
        "duration": minutes,
        "description": description
    });

    let result = client.mutate(mutation, Some(variables)).await;

    match result {
        Ok(data) => {
            if data["data"]["timeScheduleCreate"]["success"].as_bool() == Some(true) {
                println!(
                    "{} Logged {} on {} {}",
                    "+".green(),
                    format_duration(minutes).cyan(),
                    identifier.cyan(),
                    title.dimmed()
                );
            } else {
                // Time tracking might not be enabled or different API
                println!(
                    "{} Time tracking may not be available for your Linear workspace.",
                    "!".yellow()
                );
                println!(
                    "Attempted to log {} on {}",
                    format_duration(minutes),
                    identifier
                );
            }
        }
        Err(_) => {
            // Fallback message - Linear's time tracking API varies by plan
            println!(
                "{} Time tracking API not available. This feature requires Linear's time tracking add-on.",
                "!".yellow()
            );
        }
    }

    Ok(())
}

async fn list_time_entries(
    issue_filter: Option<String>,
    limit: u32,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;

    // Query time schedules
    let query = if issue_filter.is_some() {
        r#"
            query($issueId: String!, $limit: Int!) {
                issue(id: $issueId) {
                    id
                    identifier
                    timeSchedules(first: $limit) {
                        nodes {
                            id
                            duration
                            createdAt
                            description
                            user { name }
                        }
                    }
                }
            }
        "#
    } else {
        r#"
            query($limit: Int!) {
                timeSchedules(first: $limit) {
                    nodes {
                        id
                        duration
                        createdAt
                        description
                        issue { identifier }
                        user { name }
                    }
                }
            }
        "#
    };

    let variables = if let Some(ref issue_id) = issue_filter {
        json!({ "issueId": issue_id, "limit": limit })
    } else {
        json!({ "limit": limit })
    };

    let result = client.query(query, Some(variables)).await;

    match result {
        Ok(data) => {
            let entries = if issue_filter.is_some() {
                &data["data"]["issue"]["timeSchedules"]["nodes"]
            } else {
                &data["data"]["timeSchedules"]["nodes"]
            };

            if let Some(entries) = entries.as_array() {
                if output.is_json() {
                    print_json(&serde_json::Value::Array(entries.clone()), &output.json)?;
                    return Ok(());
                }

                if entries.is_empty() {
                    println!("No time entries found.");
                    return Ok(());
                }

                let issue_width = display_options().max_width(20);
                let user_width = display_options().max_width(30);
                let rows: Vec<TimeEntryRow> = entries
                    .iter()
                    .map(|e| {
                        let duration_mins = e["duration"].as_i64().unwrap_or(0) as i32;
                        TimeEntryRow {
                            id: e["id"].as_str().unwrap_or("").chars().take(8).collect(),
                            issue: truncate(
                                e["issue"]["identifier"].as_str().unwrap_or("-"),
                                issue_width,
                            ),
                            duration: format_duration(duration_mins),
                            date: e["createdAt"]
                                .as_str()
                                .unwrap_or("")
                                .chars()
                                .take(10)
                                .collect(),
                            user: truncate(e["user"]["name"].as_str().unwrap_or("-"), user_width),
                        }
                    })
                    .collect();

                let table = Table::new(rows).to_string();
                println!("{}", table);
            } else {
                println!(
                    "{} Time tracking may not be available for your workspace.",
                    "!".yellow()
                );
            }
        }
        Err(_) => {
            println!(
                "{} Time tracking API not available. This feature requires Linear's time tracking add-on.",
                "!".yellow()
            );
        }
    }

    Ok(())
}

async fn delete_time_entry(id: &str, force: bool) -> Result<()> {
    if !force {
        let confirm = dialoguer::Confirm::new()
            .with_prompt(format!("Delete time entry {}?", id))
            .default(false)
            .interact()?;

        if !confirm {
            println!("Cancelled.");
            return Ok(());
        }
    }

    let client = LinearClient::new()?;

    let mutation = r#"
        mutation DeleteTimeEntry($id: String!) {
            timeScheduleDelete(id: $id) {
                success
            }
        }
    "#;

    let result = client.mutate(mutation, Some(json!({ "id": id }))).await?;

    if result["data"]["timeScheduleDelete"]["success"].as_bool() == Some(true) {
        println!("{} Time entry deleted", "+".green());
    } else {
        anyhow::bail!("Failed to delete time entry");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration_hours() {
        assert_eq!(parse_duration("2h").unwrap(), 120);
    }

    #[test]
    fn test_parse_duration_minutes() {
        assert_eq!(parse_duration("30m").unwrap(), 30);
    }

    #[test]
    fn test_parse_duration_combined() {
        assert_eq!(parse_duration("1h30m").unwrap(), 90);
    }

    #[test]
    fn test_parse_duration_just_number() {
        assert_eq!(parse_duration("45").unwrap(), 45);
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(90), "1h 30m");
        assert_eq!(format_duration(60), "1h");
        assert_eq!(format_duration(45), "45m");
    }
}
