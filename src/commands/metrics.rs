use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use crate::api::{resolve_team_id, LinearClient};
use crate::output::{print_json_owned, OutputOptions};

#[derive(Subcommand, Debug)]
pub enum MetricsCommands {
    /// Show cycle metrics (velocity, burndown)
    Cycle {
        /// Cycle ID or number
        id: String,
        /// Team key (required if using cycle number)
        #[arg(short, long)]
        team: Option<String>,
    },
    /// Show project progress metrics
    Project {
        /// Project ID or slug
        id: String,
    },
    /// Show team velocity over time
    Velocity {
        /// Team key or ID
        team: String,
        /// Number of cycles to include
        #[arg(short, long, default_value = "5")]
        cycles: usize,
    },
}

pub async fn handle(cmd: MetricsCommands, output: &OutputOptions) -> Result<()> {
    match cmd {
        MetricsCommands::Cycle { id, team } => cycle_metrics(&id, team, output).await,
        MetricsCommands::Project { id } => project_metrics(&id, output).await,
        MetricsCommands::Velocity { team, cycles } => velocity_metrics(&team, cycles, output).await,
    }
}

async fn cycle_metrics(id: &str, team: Option<String>, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    // If --team is provided and id looks like a cycle number, resolve by team + number
    let cycle_id = if let Some(ref t) = team {
        if id.parse::<u64>().is_ok() {
            let team_id = resolve_team_id(&client, t, &output.cache).await?;
            let lookup_query = r#"
                query($teamId: String!) {
                    team(id: $teamId) {
                        cycles { nodes { id number } }
                    }
                }
            "#;
            let result = client
                .query(lookup_query, Some(json!({ "teamId": team_id })))
                .await?;
            let num: u64 = id.parse().unwrap();
            result["data"]["team"]["cycles"]["nodes"]
                .as_array()
                .and_then(|arr| {
                    arr.iter()
                        .find(|c| c["number"].as_u64() == Some(num))
                        .and_then(|c| c["id"].as_str().map(|s| s.to_string()))
                })
                .ok_or_else(|| anyhow::anyhow!("Cycle {} not found for team {}", id, t))?
        } else {
            id.to_string()
        }
    } else {
        id.to_string()
    };

    let query = r#"
        query($id: String!) {
            cycle(id: $id) {
                id
                number
                name
                startsAt
                endsAt
                progress
                scopeHistory
                completedScopeHistory
                issues {
                    nodes {
                        id
                        identifier
                        title
                        estimate
                        state {
                            name
                            type
                        }
                    }
                }
            }
        }
    "#;

    let result = client.query(query, Some(json!({ "id": cycle_id }))).await?;
    let cycle = &result["data"]["cycle"];

    if cycle.is_null() {
        anyhow::bail!("Cycle not found: {}", id);
    }

    if output.is_json() {
        // Add computed metrics
        let issues = cycle["issues"]["nodes"].as_array();
        let total_issues = issues.map(|a| a.len()).unwrap_or(0);
        let completed = issues
            .map(|a| {
                a.iter()
                    .filter(|i| i["state"]["type"].as_str() == Some("completed"))
                    .count()
            })
            .unwrap_or(0);
        let total_points: f64 = issues
            .map(|a| a.iter().filter_map(|i| i["estimate"].as_f64()).sum())
            .unwrap_or(0.0);
        let completed_points: f64 = issues
            .map(|a| {
                a.iter()
                    .filter(|i| i["state"]["type"].as_str() == Some("completed"))
                    .filter_map(|i| i["estimate"].as_f64())
                    .sum()
            })
            .unwrap_or(0.0);

        let metrics = json!({
            "cycle": cycle,
            "metrics": {
                "total_issues": total_issues,
                "completed_issues": completed,
                "completion_rate": if total_issues > 0 { (completed as f64 / total_issues as f64 * 100.0).round() } else { 0.0 },
                "total_points": total_points,
                "completed_points": completed_points,
                "velocity": completed_points,
            }
        });
        print_json_owned(metrics, output)?;
    } else {
        let issues = cycle["issues"]["nodes"].as_array();
        let total = issues.map(|a| a.len()).unwrap_or(0);
        let completed = issues
            .map(|a| {
                a.iter()
                    .filter(|i| i["state"]["type"].as_str() == Some("completed"))
                    .count()
            })
            .unwrap_or(0);
        let progress = cycle["progress"].as_f64().unwrap_or(0.0) * 100.0;

        println!("Cycle: {}", cycle["name"].as_str().unwrap_or(id));
        println!("Progress: {:.1}%", progress);
        println!("Issues: {}/{} completed", completed, total);
        println!(
            "Period: {} to {}",
            cycle["startsAt"]
                .as_str()
                .unwrap_or("-")
                .chars()
                .take(10)
                .collect::<String>(),
            cycle["endsAt"]
                .as_str()
                .unwrap_or("-")
                .chars()
                .take(10)
                .collect::<String>()
        );
    }

    Ok(())
}

async fn project_metrics(id: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($id: String!) {
            project(id: $id) {
                id
                name
                state
                progress
                targetDate
                startDate
                issues {
                    nodes {
                        id
                        estimate
                        state { type }
                    }
                }
                projectMilestones {
                    nodes {
                        id
                        name
                        targetDate
                    }
                }
            }
        }
    "#;

    let result = client.query(query, Some(json!({ "id": id }))).await?;
    let project = &result["data"]["project"];

    if project.is_null() {
        anyhow::bail!("Project not found: {}", id);
    }

    if output.is_json() {
        let issues = project["issues"]["nodes"].as_array();
        let total = issues.map(|a| a.len()).unwrap_or(0);
        let completed = issues
            .map(|a| {
                a.iter()
                    .filter(|i| i["state"]["type"].as_str() == Some("completed"))
                    .count()
            })
            .unwrap_or(0);

        let metrics = json!({
            "project": project,
            "metrics": {
                "total_issues": total,
                "completed_issues": completed,
                "completion_rate": if total > 0 { (completed as f64 / total as f64 * 100.0).round() } else { 0.0 },
            }
        });
        print_json_owned(metrics, output)?;
    } else {
        let progress = project["progress"].as_f64().unwrap_or(0.0) * 100.0;
        println!("Project: {}", project["name"].as_str().unwrap_or(id));
        println!("State: {}", project["state"].as_str().unwrap_or("-"));
        println!("Progress: {:.1}%", progress);
    }

    Ok(())
}

async fn velocity_metrics(team: &str, cycles: usize, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;
    let team_id = resolve_team_id(&client, team, &output.cache).await?;

    let query = r#"
        query($teamId: String!, $first: Int!) {
            team(id: $teamId) {
                id
                name
                cycles(first: $first, orderBy: updatedAt) {
                    nodes {
                        id
                        number
                        name
                        progress
                        issues {
                            nodes {
                                estimate
                                state { type }
                            }
                        }
                    }
                }
            }
        }
    "#;

    let result = client
        .query(query, Some(json!({ "teamId": team_id, "first": cycles })))
        .await?;
    let team_data = &result["data"]["team"];

    if team_data.is_null() {
        anyhow::bail!("Team not found: {}", team);
    }

    let cycles_data = team_data["cycles"]["nodes"].as_array();

    if output.is_json() {
        let velocity: Vec<serde_json::Value> = cycles_data
            .unwrap_or(&vec![])
            .iter()
            .map(|c| {
                let issues = c["issues"]["nodes"].as_array();
                let points: f64 = issues
                    .map(|a| {
                        a.iter()
                            .filter(|i| i["state"]["type"].as_str() == Some("completed"))
                            .filter_map(|i| i["estimate"].as_f64())
                            .sum()
                    })
                    .unwrap_or(0.0);
                json!({
                    "cycle": c["number"],
                    "name": c["name"],
                    "velocity": points,
                })
            })
            .collect();

        let avg: f64 = velocity
            .iter()
            .filter_map(|v| v["velocity"].as_f64())
            .sum::<f64>()
            / velocity.len().max(1) as f64;

        print_json_owned(
            json!({
                "team": team_data["name"],
                "cycles": velocity,
                "average_velocity": avg.round(),
            }),
            output,
        )?;
    } else {
        println!("Team: {}", team_data["name"].as_str().unwrap_or(team));
        println!("\nVelocity by Cycle:");
        for c in cycles_data.unwrap_or(&vec![]) {
            let issues = c["issues"]["nodes"].as_array();
            let points: f64 = issues
                .map(|a| {
                    a.iter()
                        .filter(|i| i["state"]["type"].as_str() == Some("completed"))
                        .filter_map(|i| i["estimate"].as_f64())
                        .sum()
                })
                .unwrap_or(0.0);
            println!(
                "  Cycle {}: {} points",
                c["number"].as_i64().unwrap_or(0),
                points
            );
        }
    }

    Ok(())
}
