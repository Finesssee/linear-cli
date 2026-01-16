use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::{json, Value};
use std::io::{self, BufRead};
use tabled::{Table, Tabled};

use crate::api::LinearClient;
use crate::cache::{Cache, CacheType};
use crate::display_options;
use crate::output::{print_json, OutputOptions};
use crate::text::truncate;

#[derive(Subcommand)]
pub enum TeamCommands {
    /// List all teams
    #[command(alias = "ls")]
    List,
    /// Get team details
    Get {
        /// Team ID(s), key(s), or name(s). Use "-" to read from stdin.
        ids: Vec<String>,
    },
}

#[derive(Tabled)]
struct TeamRow {
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Key")]
    key: String,
    #[tabled(rename = "ID")]
    id: String,
}

pub async fn handle(cmd: TeamCommands, output: &OutputOptions) -> Result<()> {
    match cmd {
        TeamCommands::List => list_teams(output).await,
        TeamCommands::Get { ids } => {
            let final_ids: Vec<String> = if ids.is_empty() || (ids.len() == 1 && ids[0] == "-") {
                let stdin = io::stdin();
                stdin
                    .lock()
                    .lines()
                    .map_while(Result::ok)
                    .filter(|l| !l.trim().is_empty())
                    .map(|l| l.trim().to_string())
                    .collect()
            } else {
                ids
            };
            if final_ids.is_empty() {
                anyhow::bail!("No team IDs provided. Provide IDs or pipe them via stdin.");
            }
            get_teams(&final_ids, output).await
        }
    }
}

async fn list_teams(output: &OutputOptions) -> Result<()> {
    let cache = Cache::new()?;

    // Try to get teams from cache first
    let teams_data: Value = if let Some(cached) = cache.get(CacheType::Teams) {
        cached
    } else {
        // Fetch from API
        let client = LinearClient::new()?;

        let query = r#"
            query {
                teams(first: 100) {
                    nodes {
                        id
                        name
                        key
                    }
                }
            }
        "#;

        let result = client.query(query, None).await?;
        let data = result["data"]["teams"]["nodes"].clone();

        // Cache the result
        let _ = cache.set(CacheType::Teams, data.clone());
        data
    };

    // Handle JSON output
    if output.is_json() {
        print_json(&teams_data, &output.json)?;
        return Ok(());
    }

    let empty = vec![];
    let teams = teams_data.as_array().unwrap_or(&empty);

    if teams.is_empty() {
        println!("No teams found.");
        return Ok(());
    }

    let width = display_options().max_width(30);
    let rows: Vec<TeamRow> = teams
        .iter()
        .map(|t| TeamRow {
            name: truncate(t["name"].as_str().unwrap_or(""), width),
            key: t["key"].as_str().unwrap_or("").to_string(),
            id: t["id"].as_str().unwrap_or("").to_string(),
        })
        .collect();

    let table = Table::new(rows).to_string();
    println!("{}", table);
    println!("\n{} teams", teams.len());

    Ok(())
}

async fn get_team(id: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($id: String!) {
            team(id: $id) {
                id
                name
                key
                description
                icon
                color
                private
                timezone
                issueCount
                createdAt
                updatedAt
            }
        }
    "#;

    let result = client.query(query, Some(json!({ "id": id }))).await?;
    let team = &result["data"]["team"];

    if team.is_null() {
        anyhow::bail!("Team not found: {}", id);
    }

    // Handle JSON output
    if output.is_json() {
        print_json(team, &output.json)?;
        return Ok(());
    }

    println!("{}", team["name"].as_str().unwrap_or("").bold());
    println!("{}", "-".repeat(40));

    println!("Key: {}", team["key"].as_str().unwrap_or("-"));

    if let Some(desc) = team["description"].as_str() {
        if !desc.is_empty() {
            println!("Description: {}", desc);
        }
    }

    println!("Private: {}", team["private"].as_bool().unwrap_or(false));

    if let Some(timezone) = team["timezone"].as_str() {
        println!("Timezone: {}", timezone);
    }

    if let Some(issue_count) = team["issueCount"].as_i64() {
        println!("Issue Count: {}", issue_count);
    }

    if let Some(color) = team["color"].as_str() {
        println!("Color: {}", color);
    }

    if let Some(icon) = team["icon"].as_str() {
        println!("Icon: {}", icon);
    }

    println!("ID: {}", team["id"].as_str().unwrap_or("-"));

    if let Some(created_at) = team["createdAt"].as_str() {
        println!("Created: {}", created_at);
    }

    if let Some(updated_at) = team["updatedAt"].as_str() {
        println!("Updated: {}", updated_at);
    }

    Ok(())
}

async fn get_teams(ids: &[String], output: &OutputOptions) -> Result<()> {
    if ids.len() == 1 {
        return get_team(&ids[0], output).await;
    }

    let client = LinearClient::new()?;

    let futures: Vec<_> = ids
        .iter()
        .map(|id| {
            let client = client.clone();
            let id = id.clone();
            async move {
                let query = r#"
                    query($id: String!) {
                        team(id: $id) {
                            id
                            name
                            key
                            private
                        }
                    }
                "#;
                let result = client.query(query, Some(json!({ "id": id }))).await;
                (id, result)
            }
        })
        .collect();

    let results = futures::future::join_all(futures).await;

    if output.is_json() {
        let teams: Vec<_> = results
            .iter()
            .filter_map(|(_, r)| {
                r.as_ref().ok().and_then(|data| {
                    let team = &data["data"]["team"];
                    if !team.is_null() {
                        Some(team.clone())
                    } else {
                        None
                    }
                })
            })
            .collect();
        print_json(&serde_json::json!(teams), &output.json)?;
        return Ok(());
    }

    let width = display_options().max_width(30);
    for (id, result) in results {
        match result {
            Ok(data) => {
                let team = &data["data"]["team"];
                if team.is_null() {
                    eprintln!("{} Team not found: {}", "!".yellow(), id);
                } else {
                    let name = truncate(team["name"].as_str().unwrap_or("-"), width);
                    let key = team["key"].as_str().unwrap_or("-");
                    let private = team["private"].as_bool().unwrap_or(false);
                    println!("{} ({}) private={} id={}", name.cyan(), key, private, id);
                }
            }
            Err(e) => {
                eprintln!("{} Error fetching {}: {}", "!".red(), id, e);
            }
        }
    }

    Ok(())
}
