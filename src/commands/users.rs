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
pub enum UserCommands {
    /// List all users in the workspace
    #[command(alias = "ls")]
    List {
        /// Filter users by team name or ID
        #[arg(short, long)]
        team: Option<String>,
    },
    /// Show current user details
    Me,
}

#[derive(Tabled)]
struct UserRow {
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Email")]
    email: String,
    #[tabled(rename = "ID")]
    id: String,
}

pub async fn handle(cmd: UserCommands, output: &OutputOptions) -> Result<()> {
    match cmd {
        UserCommands::List { team } => list_users(team, output).await,
        UserCommands::Me => get_me(output).await,
    }
}

async fn list_users(team: Option<String>, output: &OutputOptions) -> Result<()> {
    let cache = Cache::new()?;

    // Only use cache for full user list (no team filter)
    let users: Vec<Value> = if let Some(ref team_key) = team {
        // Team-filtered users - always fetch from API (not cached)
        let client = LinearClient::new()?;
        let team_id = resolve_team_id(&client, team_key).await?;

        let query = r#"
            query($teamId: String!) {
                team(id: $teamId) {
                    members {
                        nodes {
                            id
                            name
                            email
                        }
                    }
                }
            }
        "#;

        let result = client
            .query(query, Some(json!({ "teamId": team_id })))
            .await?;

        result["data"]["team"]["members"]["nodes"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    } else {
        // Try cache first
        if let Some(cached) = cache.get(CacheType::Users) {
            cached.as_array().cloned().unwrap_or_default()
        } else {
            // Fetch from API
            let client = LinearClient::new()?;
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
            let data = result["data"]["users"]["nodes"].clone();

            // Cache the result
            let _ = cache.set(CacheType::Users, data.clone());
            data.as_array().cloned().unwrap_or_default()
        }
    };

    if output.is_json() {
        print_json(&serde_json::json!(users), &output.json)?;
        return Ok(());
    }

    if users.is_empty() {
        println!("No users found.");
        return Ok(());
    }

    let name_width = display_options().max_width(30);
    let email_width = display_options().max_width(40);
    let rows: Vec<UserRow> = users
        .iter()
        .map(|u| UserRow {
            name: truncate(u["name"].as_str().unwrap_or(""), name_width),
            email: truncate(u["email"].as_str().unwrap_or(""), email_width),
            id: u["id"].as_str().unwrap_or("").to_string(),
        })
        .collect();

    let table = Table::new(rows).to_string();
    println!("{}", table);
    println!("\n{} users", users.len());

    Ok(())
}

async fn get_me(output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query {
            viewer {
                id
                name
                email
                displayName
                avatarUrl
                admin
                active
                createdAt
                url
            }
        }
    "#;

    let result = client.query(query, None).await?;
    let user = &result["data"]["viewer"];

    if user.is_null() {
        anyhow::bail!("Could not fetch current user");
    }

    if output.is_json() {
        print_json(user, &output.json)?;
        return Ok(());
    }

    println!("{}", user["name"].as_str().unwrap_or("").bold());
    println!("{}", "-".repeat(40));

    if let Some(display_name) = user["displayName"].as_str() {
        if !display_name.is_empty() {
            println!("Display Name: {}", display_name);
        }
    }

    println!("Email: {}", user["email"].as_str().unwrap_or("-"));
    println!(
        "Admin: {}",
        user["admin"]
            .as_bool()
            .map(|b| if b { "Yes" } else { "No" })
            .unwrap_or("-")
    );
    println!(
        "Active: {}",
        user["active"]
            .as_bool()
            .map(|b| if b { "Yes" } else { "No" })
            .unwrap_or("-")
    );

    if let Some(created) = user["createdAt"].as_str() {
        println!("Created: {}", created);
    }

    println!("URL: {}", user["url"].as_str().unwrap_or("-"));
    println!("ID: {}", user["id"].as_str().unwrap_or("-"));

    Ok(())
}
