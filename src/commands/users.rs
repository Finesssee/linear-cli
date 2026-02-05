use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::{json, Value};
use tabled::{Table, Tabled};

use crate::api::{resolve_team_id, LinearClient};
use crate::cache::{Cache, CacheType};
use crate::display_options;
use crate::output::{ensure_non_empty, filter_values, print_json, sort_values, OutputOptions};
use crate::pagination::paginate_nodes;
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
    let can_use_cache = !output.cache.no_cache
        && output.pagination.after.is_none()
        && output.pagination.before.is_none()
        && !output.pagination.all
        && output.pagination.page_size.is_none()
        && output.pagination.limit.is_none();

    let users: Vec<Value> = if let Some(ref team_key) = team {
        let client = LinearClient::new()?;
        let team_id = resolve_team_id(&client, team_key, &output.cache).await?;
        let pagination = output.pagination.with_default_limit(100);
        let query = r#"
            query($teamId: String!, $first: Int, $after: String, $last: Int, $before: String) {
                team(id: $teamId) {
                    members(first: $first, after: $after, last: $last, before: $before) {
                        nodes {
                            id
                            name
                            email
                        }
                        pageInfo {
                            hasNextPage
                            endCursor
                            hasPreviousPage
                            startCursor
                        }
                    }
                }
            }
        "#;

        let mut vars = serde_json::Map::new();
        vars.insert("teamId".to_string(), json!(team_id));

        paginate_nodes(
            &client,
            query,
            vars,
            &["data", "team", "members", "nodes"],
            &["data", "team", "members", "pageInfo"],
            &pagination,
            100,
        )
        .await?
    } else {
        let cached: Vec<Value> = if can_use_cache {
            let cache = Cache::new()?;
            cache
                .get(CacheType::Users)
                .and_then(|data| data.as_array().cloned())
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        if !cached.is_empty() {
            cached
        } else {
            let client = LinearClient::new()?;
            let pagination = output.pagination.with_default_limit(100);
            let query = r#"
                query($first: Int, $after: String, $last: Int, $before: String) {
                    users(first: $first, after: $after, last: $last, before: $before) {
                        nodes {
                            id
                            name
                            email
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

            let users = paginate_nodes(
                &client,
                query,
                serde_json::Map::new(),
                &["data", "users", "nodes"],
                &["data", "users", "pageInfo"],
                &pagination,
                100,
            )
            .await?;

            if !output.cache.no_cache {
                let cache = Cache::with_ttl(output.cache.effective_ttl_seconds())?;
                let _ = cache.set(CacheType::Users, serde_json::json!(users.clone()));
            }

            users
        }
    };

    if output.is_json() || output.has_template() {
        print_json(&serde_json::json!(users), output)?;
        return Ok(());
    }

    let mut users = users;
    filter_values(&mut users, &output.filters);
    if let Some(sort_key) = output.json.sort.as_deref() {
        sort_values(&mut users, sort_key, output.json.order);
    }

    ensure_non_empty(&users, output)?;
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

    if output.is_json() || output.has_template() {
        print_json(user, output)?;
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
