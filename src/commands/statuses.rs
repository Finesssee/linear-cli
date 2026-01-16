use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::{json, Value};
use tabled::{Table, Tabled};

use crate::api::{resolve_team_id, LinearClient};
use crate::cache::{Cache, CacheType};
use crate::display_options;
use crate::output::{print_json, OutputOptions};
use crate::text::truncate;

#[derive(Subcommand)]
pub enum StatusCommands {
    /// List all issue statuses for a team
    #[command(alias = "ls")]
    List {
        /// Team name or ID
        #[arg(short, long)]
        team: String,
    },
    /// Get details of a specific status
    Get {
        /// Status name or ID
        id: String,
        /// Team name or ID
        #[arg(short, long)]
        team: String,
    },
}

#[derive(Tabled)]
struct StatusRow {
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Type")]
    status_type: String,
    #[tabled(rename = "Color")]
    color: String,
    #[tabled(rename = "Position")]
    position: String,
    #[tabled(rename = "ID")]
    id: String,
}

pub async fn handle(cmd: StatusCommands, output: &OutputOptions) -> Result<()> {
    match cmd {
        StatusCommands::List { team } => list_statuses(&team, output).await,
        StatusCommands::Get { id, team } => get_status(&id, &team, output).await,
    }
}

async fn list_statuses(team: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;
    let cache = Cache::new()?;

    // Resolve team key/name to UUID
    let team_id = resolve_team_id(&client, team).await?;

    // Try to get statuses from cache first
    let (team_name, states): (String, Vec<Value>) =
        if let Some(cached) = cache.get_keyed(CacheType::Statuses, &team_id) {
            let name = cached["team_name"].as_str().unwrap_or("").to_string();
            let states_data = cached["states"].as_array().cloned().unwrap_or_default();
            (name, states_data)
        } else {
            // Fetch from API
            let query = r#"
                query($teamId: String!) {
                    team(id: $teamId) {
                        id
                        name
                        states {
                            nodes {
                                id
                                name
                                type
                                color
                                position
                                description
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

            let name = team_data["name"].as_str().unwrap_or("").to_string();
            let states_data = team_data["states"]["nodes"]
                .as_array()
                .cloned()
                .unwrap_or_default();

            // Cache the result
            let cache_data = json!({
                "team_name": name,
                "states": states_data
            });
            let _ = cache.set_keyed(CacheType::Statuses, &team_id, cache_data);

            (name, states_data)
        };

    if states.is_empty() {
        if output.is_json() {
            print_json(&json!({"statuses": [], "team": team_name}), &output.json)?;
            return Ok(());
        }
        println!("No statuses found for team '{}'.", team_name);
        return Ok(());
    }

    if output.is_json() {
        print_json(
            &json!({
                "team": team_name,
                "statuses": states
            }),
            &output.json,
        )?;
        return Ok(());
    }

    println!(
        "{}",
        format!("Issue statuses for team '{}'", team_name).bold()
    );
    println!("{}", "-".repeat(50));

    let width = display_options().max_width(30);
    let rows: Vec<StatusRow> = states
        .iter()
        .map(|s| {
            let status_type = s["type"].as_str().unwrap_or("");
            let type_colored = match status_type {
                "completed" => status_type.green().to_string(),
                "started" => status_type.yellow().to_string(),
                "canceled" | "cancelled" => status_type.red().to_string(),
                "backlog" => status_type.dimmed().to_string(),
                "unstarted" => status_type.cyan().to_string(),
                _ => status_type.to_string(),
            };

            StatusRow {
                name: truncate(s["name"].as_str().unwrap_or(""), width),
                status_type: type_colored,
                color: s["color"].as_str().unwrap_or("").to_string(),
                position: s["position"]
                    .as_f64()
                    .map(|p| format!("{:.0}", p))
                    .unwrap_or("-".to_string()),
                id: s["id"].as_str().unwrap_or("").to_string(),
            }
        })
        .collect();

    let table = Table::new(rows).to_string();
    println!("{}", table);
    println!("\n{} statuses", states.len());

    Ok(())
}

async fn get_status(id: &str, team: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    // Resolve team key/name to UUID
    let team_id = resolve_team_id(&client, team).await?;

    // First get all states for the team and find the matching one
    let query = r#"
        query($teamId: String!) {
            team(id: $teamId) {
                id
                name
                states {
                    nodes {
                        id
                        name
                        type
                        color
                        position
                        description
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

    let empty = vec![];
    let states = team_data["states"]["nodes"].as_array().unwrap_or(&empty);

    // Find matching status by ID or name
    let status = states.iter().find(|s| {
        s["id"].as_str() == Some(id)
            || s["name"].as_str().map(|n| n.to_lowercase()) == Some(id.to_lowercase())
    });

    match status {
        Some(s) => {
            if output.is_json() {
                print_json(s, &output.json)?;
                return Ok(());
            }

            println!("{}", s["name"].as_str().unwrap_or("").bold());
            println!("{}", "-".repeat(40));
            println!("Type: {}", s["type"].as_str().unwrap_or("-"));
            println!("Color: {}", s["color"].as_str().unwrap_or("-"));
            println!(
                "Position: {}",
                s["position"]
                    .as_f64()
                    .map(|p| format!("{:.0}", p))
                    .unwrap_or("-".to_string())
            );
            if let Some(desc) = s["description"].as_str() {
                if !desc.is_empty() {
                    println!("Description: {}", desc);
                }
            }
            println!("ID: {}", s["id"].as_str().unwrap_or("-"));
            Ok(())
        }
        None => {
            anyhow::bail!("Status not found: {}", id);
        }
    }
}
