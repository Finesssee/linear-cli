use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use futures::stream::{self, StreamExt};
use serde_json::json;

use crate::api::{resolve_team_id, LinearClient};
use crate::output::{print_json, print_json_owned, OutputOptions};

#[derive(Subcommand)]
pub enum SprintCommands {
    /// Show current sprint status and progress
    Status {
        /// Team key, name, or ID
        #[arg(short, long)]
        team: String,
    },
    /// Show sprint progress (completion %)
    Progress {
        /// Team key, name, or ID
        #[arg(short, long)]
        team: String,
    },
    /// List issues planned for next cycle
    Plan {
        /// Team key, name, or ID
        #[arg(short, long)]
        team: String,
    },
    /// Move incomplete issues from current cycle to next
    CarryOver {
        /// Team key, name, or ID
        #[arg(short, long)]
        team: String,
        /// Skip confirmation
        #[arg(short, long)]
        force: bool,
    },
}

pub async fn handle(cmd: SprintCommands, output: &OutputOptions) -> Result<()> {
    match cmd {
        SprintCommands::Status { team } => sprint_status(&team, output).await,
        SprintCommands::Progress { team } => sprint_progress(&team, output).await,
        SprintCommands::Plan { team } => sprint_plan(&team, output).await,
        SprintCommands::CarryOver { team, force } => sprint_carry_over(&team, force, output).await,
    }
}

async fn sprint_status(team: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;
    let team_id = resolve_team_id(&client, team, &output.cache).await?;

    let query = r#"
        query($teamId: String!) {
            team(id: $teamId) {
                name
                activeCycle {
                    id name number
                    startsAt endsAt
                    progress
                    scopeHistory
                    issues(first: 250) {
                        nodes {
                            id identifier title
                            state { name type }
                            priority
                            assignee { name }
                            estimate
                            createdAt
                        }
                    }
                }
            }
        }
    "#;

    let result = client
        .query(query, Some(json!({ "teamId": team_id })))
        .await?;
    let team_data = &result["data"]["team"];

    if team_data.is_null() {
        anyhow::bail!("Team not found: {}", team);
    }

    let team_name = team_data["name"].as_str().unwrap_or(team);
    let cycle = &team_data["activeCycle"];

    if cycle.is_null() {
        if output.is_json() || output.has_template() {
            print_json_owned(
                json!({ "team": team_name, "activeCycle": null }),
                output,
            )?;
        } else {
            println!("No active cycle for team '{}'.", team_name);
        }
        return Ok(());
    }

    if output.is_json() || output.has_template() {
        print_json(cycle, output)?;
        return Ok(());
    }

    let cycle_name = cycle["name"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or("(unnamed)");
    let cycle_number = cycle["number"].as_u64().unwrap_or(0);
    let progress = cycle["progress"].as_f64().unwrap_or(0.0);
    let start_date = cycle["startsAt"]
        .as_str()
        .map(|s| s.get(..10).unwrap_or(s))
        .unwrap_or("-");
    let end_date = cycle["endsAt"]
        .as_str()
        .map(|s| s.get(..10).unwrap_or(s))
        .unwrap_or("-");

    let issues = cycle["issues"]["nodes"].as_array();

    let (total, completed, in_progress, scope_change) = if let Some(issues) = issues {
        let total = issues.len();
        let completed = issues
            .iter()
            .filter(|i| i["state"]["type"].as_str() == Some("completed"))
            .count();
        let in_progress = issues
            .iter()
            .filter(|i| i["state"]["type"].as_str() == Some("started"))
            .count();

        // Scope change: compare current total to first entry in scopeHistory
        let scope_change = cycle["scopeHistory"]
            .as_array()
            .and_then(|h| h.first())
            .and_then(|v| v.as_f64())
            .map(|initial| total as i64 - initial as i64)
            .unwrap_or(0);

        (total, completed, in_progress, scope_change)
    } else {
        (0, 0, 0, 0)
    };

    println!(
        "{}",
        format!("Sprint {} - {}", cycle_number, cycle_name).bold()
    );
    println!("{}", "-".repeat(40));
    println!("Team:        {}", team_name);
    println!("Dates:       {} to {}", start_date, end_date);
    println!("Progress:    {:.0}%", progress * 100.0);
    println!();
    println!("Issues:      {}", total);
    println!("  Completed: {}", completed.to_string().green());
    println!("  In Prog:   {}", in_progress.to_string().yellow());
    println!(
        "  Remaining: {}",
        (total - completed - in_progress).to_string().dimmed()
    );

    if scope_change != 0 {
        let sign = if scope_change > 0 { "+" } else { "" };
        println!(
            "  Scope:     {} issues",
            format!("{}{}", sign, scope_change).red()
        );
    }

    // Show estimate totals if any issues have estimates
    if let Some(issues) = issues {
        let total_estimate: f64 = issues
            .iter()
            .filter_map(|i| i["estimate"].as_f64())
            .sum();
        let completed_estimate: f64 = issues
            .iter()
            .filter(|i| i["state"]["type"].as_str() == Some("completed"))
            .filter_map(|i| i["estimate"].as_f64())
            .sum();

        if total_estimate > 0.0 {
            println!();
            println!(
                "Estimates:   {:.0} / {:.0} points",
                completed_estimate, total_estimate
            );
        }
    }

    Ok(())
}

async fn sprint_progress(team: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;
    let team_id = resolve_team_id(&client, team, &output.cache).await?;

    let query = r#"
        query($teamId: String!) {
            team(id: $teamId) {
                name
                activeCycle {
                    id name number progress
                    issues(first: 250) {
                        nodes {
                            id
                            state { type }
                            estimate
                        }
                    }
                }
            }
        }
    "#;

    let result = client
        .query(query, Some(json!({ "teamId": team_id })))
        .await?;
    let team_data = &result["data"]["team"];

    if team_data.is_null() {
        anyhow::bail!("Team not found: {}", team);
    }

    let cycle = &team_data["activeCycle"];

    if cycle.is_null() {
        if output.is_json() || output.has_template() {
            print_json_owned(
                json!({ "team": team_data["name"], "activeCycle": null }),
                output,
            )?;
        } else {
            println!(
                "No active cycle for team '{}'.",
                team_data["name"].as_str().unwrap_or(team)
            );
        }
        return Ok(());
    }

    let issues = cycle["issues"]["nodes"].as_array();
    let cycle_number = cycle["number"].as_u64().unwrap_or(0);
    let progress = cycle["progress"].as_f64().unwrap_or(0.0);

    let (total, completed, in_progress, todo) = if let Some(issues) = issues {
        let total = issues.len();
        let completed = issues
            .iter()
            .filter(|i| i["state"]["type"].as_str() == Some("completed"))
            .count();
        let in_progress = issues
            .iter()
            .filter(|i| i["state"]["type"].as_str() == Some("started"))
            .count();
        let todo = total - completed - in_progress;
        (total, completed, in_progress, todo)
    } else {
        (0, 0, 0, 0)
    };

    if output.is_json() || output.has_template() {
        let total_estimate: f64 = issues
            .map(|arr| arr.iter().filter_map(|i| i["estimate"].as_f64()).sum())
            .unwrap_or(0.0);
        let completed_estimate: f64 = issues
            .map(|arr| {
                arr.iter()
                    .filter(|i| i["state"]["type"].as_str() == Some("completed"))
                    .filter_map(|i| i["estimate"].as_f64())
                    .sum()
            })
            .unwrap_or(0.0);

        print_json_owned(
            json!({
                "cycle_number": cycle_number,
                "progress": progress,
                "total": total,
                "completed": completed,
                "in_progress": in_progress,
                "todo": todo,
                "total_estimate": total_estimate,
                "completed_estimate": completed_estimate,
            }),
            output,
        )?;
        return Ok(());
    }

    // Visual progress bar
    let bar_width: usize = 20;
    let filled = (progress * bar_width as f64).round() as usize;
    let empty = bar_width.saturating_sub(filled);
    let bar = format!(
        "[{}{}]",
        "\u{2588}".repeat(filled).green(),
        "\u{2591}".repeat(empty).dimmed()
    );

    println!(
        "Sprint {}: {} {:.0}% ({}/{} issues)",
        cycle_number, bar, progress * 100.0, completed, total
    );
    println!(
        "  Completed: {}  In Progress: {}  Todo: {}",
        completed.to_string().green(),
        in_progress.to_string().yellow(),
        todo.to_string().dimmed()
    );

    // Estimate summary
    if let Some(issues) = issues {
        let total_estimate: f64 = issues
            .iter()
            .filter_map(|i| i["estimate"].as_f64())
            .sum();
        let completed_estimate: f64 = issues
            .iter()
            .filter(|i| i["state"]["type"].as_str() == Some("completed"))
            .filter_map(|i| i["estimate"].as_f64())
            .sum();

        if total_estimate > 0.0 {
            println!(
                "  Estimate: {:.0} points completed / {:.0} total",
                completed_estimate, total_estimate
            );
        }
    }

    Ok(())
}

async fn sprint_plan(team: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;
    let team_id = resolve_team_id(&client, team, &output.cache).await?;

    let query = r#"
        query($teamId: String!) {
            team(id: $teamId) {
                name
                upcomingCycles(first: 1) {
                    nodes {
                        id name number startsAt endsAt
                        issues(first: 250) {
                            nodes {
                                id identifier title priority
                                state { name }
                                assignee { name }
                                estimate
                            }
                        }
                    }
                }
            }
        }
    "#;

    let result = client
        .query(query, Some(json!({ "teamId": team_id })))
        .await?;
    let team_data = &result["data"]["team"];

    if team_data.is_null() {
        anyhow::bail!("Team not found: {}", team);
    }

    let team_name = team_data["name"].as_str().unwrap_or(team);
    let cycles = team_data["upcomingCycles"]["nodes"].as_array();

    let next_cycle = cycles.and_then(|arr| arr.first());

    if next_cycle.is_none() {
        if output.is_json() || output.has_template() {
            print_json_owned(
                json!({ "team": team_name, "nextCycle": null }),
                output,
            )?;
        } else {
            println!("No upcoming cycle for team '{}'.", team_name);
        }
        return Ok(());
    }

    let cycle = next_cycle.unwrap();

    if output.is_json() || output.has_template() {
        print_json(cycle, output)?;
        return Ok(());
    }

    let cycle_name = cycle["name"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or("(unnamed)");
    let cycle_number = cycle["number"].as_u64().unwrap_or(0);
    let start_date = cycle["startsAt"]
        .as_str()
        .map(|s| s.get(..10).unwrap_or(s))
        .unwrap_or("-");
    let end_date = cycle["endsAt"]
        .as_str()
        .map(|s| s.get(..10).unwrap_or(s))
        .unwrap_or("-");

    println!(
        "{}",
        format!("Next Sprint {} - {}", cycle_number, cycle_name).bold()
    );
    println!("{}", "-".repeat(40));
    println!("Dates: {} to {}", start_date, end_date);

    let issues = cycle["issues"]["nodes"].as_array();

    if let Some(issues) = issues {
        if issues.is_empty() {
            println!("\nNo issues planned yet.");
        } else {
            let total_estimate: f64 = issues
                .iter()
                .filter_map(|i| i["estimate"].as_f64())
                .sum();

            println!("\n{} ({} issues)", "Planned Issues:".bold(), issues.len());
            if total_estimate > 0.0 {
                println!("Total estimate: {:.0} points", total_estimate);
            }
            println!();

            for issue in issues {
                let identifier = issue["identifier"].as_str().unwrap_or("");
                let title = issue["title"].as_str().unwrap_or("");
                let state = issue["state"]["name"].as_str().unwrap_or("-");
                let assignee = issue["assignee"]["name"].as_str().unwrap_or("-");
                let estimate = issue["estimate"]
                    .as_f64()
                    .map(|e| format!(" [{:.0}p]", e))
                    .unwrap_or_default();

                println!(
                    "  {} {}{} [{}] ({})",
                    identifier.cyan(),
                    title,
                    estimate.dimmed(),
                    state,
                    assignee
                );
            }
        }
    }

    Ok(())
}

async fn sprint_carry_over(team: &str, force: bool, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;
    let team_id = resolve_team_id(&client, team, &output.cache).await?;

    // Get current cycle's incomplete issues
    let current_query = r#"
        query($teamId: String!) {
            team(id: $teamId) {
                name
                activeCycle {
                    id name number
                    issues(first: 250) {
                        nodes {
                            id identifier title
                            state { name type }
                        }
                    }
                }
            }
        }
    "#;

    let result = client
        .query(current_query, Some(json!({ "teamId": team_id })))
        .await?;
    let team_data = &result["data"]["team"];

    if team_data.is_null() {
        anyhow::bail!("Team not found: {}", team);
    }

    let team_name = team_data["name"].as_str().unwrap_or(team);
    let current_cycle = &team_data["activeCycle"];

    if current_cycle.is_null() {
        anyhow::bail!("No active cycle for team '{}'.", team_name);
    }

    // Get next cycle
    let next_query = r#"
        query($teamId: String!) {
            team(id: $teamId) {
                upcomingCycles(first: 1) {
                    nodes { id name number }
                }
            }
        }
    "#;

    let next_result = client
        .query(next_query, Some(json!({ "teamId": team_id })))
        .await?;
    let next_cycles = next_result["data"]["team"]["upcomingCycles"]["nodes"].as_array();
    let next_cycle = next_cycles
        .and_then(|arr| arr.first())
        .ok_or_else(|| anyhow::anyhow!("No upcoming cycle to carry issues over to."))?;

    let next_cycle_id = next_cycle["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Could not get next cycle ID"))?;

    // Find incomplete issues (not completed, not canceled)
    let incomplete: Vec<&serde_json::Value> = current_cycle["issues"]["nodes"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter(|i| {
                    let state_type = i["state"]["type"].as_str().unwrap_or("");
                    state_type != "completed" && state_type != "canceled"
                })
                .collect()
        })
        .unwrap_or_default();

    if incomplete.is_empty() {
        if output.is_json() || output.has_template() {
            print_json_owned(
                json!({
                    "carried_over": 0,
                    "message": "No incomplete issues to carry over"
                }),
                output,
            )?;
        } else {
            println!("No incomplete issues in the current cycle.");
        }
        return Ok(());
    }

    // Confirmation
    if !force && !crate::is_yes() {
        println!(
            "Will move {} incomplete issues from current cycle to next cycle:",
            incomplete.len()
        );
        for issue in &incomplete {
            let identifier = issue["identifier"].as_str().unwrap_or("");
            let title = issue["title"].as_str().unwrap_or("");
            let state = issue["state"]["name"].as_str().unwrap_or("-");
            println!("  {} {} [{}]", identifier.cyan(), title, state);
        }
        println!();
        anyhow::bail!(
            "Use --force or --yes to confirm. {} issues would be moved.",
            incomplete.len()
        );
    }

    // Move issues in parallel
    let issue_ids: Vec<String> = incomplete
        .iter()
        .filter_map(|i| i["id"].as_str().map(|s| s.to_string()))
        .collect();

    let mutation = r#"
        mutation($id: String!, $input: IssueUpdateInput!) {
            issueUpdate(id: $id, input: $input) {
                success
                issue { id identifier }
            }
        }
    "#;

    let results: Vec<(String, bool)> = stream::iter(issue_ids.iter())
        .map(|issue_id| {
            let client = &client;
            let id = issue_id.clone();
            let cycle_id = next_cycle_id.to_string();
            async move {
                let result = client
                    .mutate(
                        mutation,
                        Some(json!({ "id": id, "input": { "cycleId": cycle_id } })),
                    )
                    .await;
                let success = result
                    .as_ref()
                    .map(|r| {
                        r["data"]["issueUpdate"]["success"]
                            .as_bool()
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);
                (id, success)
            }
        })
        .buffer_unordered(10)
        .collect()
        .await;

    let moved = results.iter().filter(|(_, s)| *s).count();
    let failed = results.iter().filter(|(_, s)| !*s).count();

    if output.is_json() || output.has_template() {
        print_json_owned(
            json!({
                "carried_over": moved,
                "failed": failed,
                "next_cycle": next_cycle["name"],
                "next_cycle_number": next_cycle["number"],
            }),
            output,
        )?;
    } else {
        println!(
            "{} Moved {} issues to next cycle ({})",
            "+".green(),
            moved,
            next_cycle["name"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or("upcoming")
        );
        if failed > 0 {
            println!("{} {} issues failed to move", "!".red(), failed);
        }
    }

    Ok(())
}
