use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::{json, Value};
use tabled::{Table, Tabled};

use crate::api::{resolve_team_id, LinearClient};
use crate::cache::{Cache, CacheType};
use crate::display_options;
use crate::input::read_ids_from_stdin;
use crate::output::{
    ensure_non_empty, filter_values, print_json, print_json_owned, sort_values, OutputOptions,
};
use crate::pagination::paginate_nodes;
use crate::text::truncate;
use crate::types::Team;

fn safe_terminal_value(value: &str) -> String {
    crate::text::sanitize_terminal_text(value)
}

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
    /// List members of a team
    Members {
        /// Team key, name, or ID (e.g., "ENG")
        team: String,
    },
    /// Create a new team
    Create {
        /// Team name (required)
        name: String,
        /// Team key (2-5 uppercase letters)
        #[arg(short, long)]
        key: Option<String>,
        /// Team description
        #[arg(short, long)]
        description: Option<String>,
        /// Team icon
        #[arg(long)]
        icon: Option<String>,
        /// Team color (hex)
        #[arg(long)]
        color: Option<String>,
        /// Make team private
        #[arg(long)]
        private: bool,
        /// Team timezone
        #[arg(long)]
        timezone: Option<String>,
    },
    /// Update an existing team
    Update {
        /// Team ID, key, or name
        id: String,
        /// New name
        #[arg(short, long)]
        name: Option<String>,
        /// New description
        #[arg(short, long)]
        description: Option<String>,
        /// New icon
        #[arg(long)]
        icon: Option<String>,
        /// New color (hex)
        #[arg(long)]
        color: Option<String>,
        /// Make private
        #[arg(long)]
        private: Option<bool>,
        /// Set timezone
        #[arg(long)]
        timezone: Option<String>,
    },
    /// Delete a team
    Delete {
        /// Team ID, key, or name
        id: String,
        /// Skip confirmation
        #[arg(long)]
        force: bool,
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
            let final_ids = read_ids_from_stdin(ids);
            if final_ids.is_empty() {
                anyhow::bail!("No team IDs provided. Provide IDs or pipe them via stdin.");
            }
            get_teams(&final_ids, output).await
        }
        TeamCommands::Members { team } => list_members(&team, output).await,
        TeamCommands::Create {
            name,
            key,
            description,
            icon,
            color,
            private,
            timezone,
        } => {
            create_team(
                &name,
                key,
                description,
                icon,
                color,
                private,
                timezone,
                output,
            )
            .await
        }
        TeamCommands::Update {
            id,
            name,
            description,
            icon,
            color,
            private,
            timezone,
        } => {
            update_team(
                &id,
                name,
                description,
                icon,
                color,
                private,
                timezone,
                output,
            )
            .await
        }
        TeamCommands::Delete { id, force } => delete_team(&id, force, output).await,
    }
}

async fn list_teams(output: &OutputOptions) -> Result<()> {
    let can_use_cache = !output.cache.no_cache
        && output.pagination.after.is_none()
        && output.pagination.before.is_none()
        && !output.pagination.all
        && output.pagination.page_size.is_none()
        && output.pagination.limit.is_none();

    let cached: Vec<Value> = if can_use_cache {
        let cache = Cache::with_ttl(output.cache.effective_ttl_seconds())?;
        cache
            .get(CacheType::Teams)
            .and_then(|data| data.as_array().cloned())
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let teams = if !cached.is_empty() {
        cached
    } else {
        let client = LinearClient::new()?;
        let pagination = output.pagination.with_default_limit(100);
        let query = r#"
            query($first: Int, $after: String, $last: Int, $before: String) {
                teams(first: $first, after: $after, last: $last, before: $before) {
                    nodes {
                        id
                        name
                        key
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

        let teams = paginate_nodes(
            &client,
            query,
            serde_json::Map::new(),
            &["data", "teams", "nodes"],
            &["data", "teams", "pageInfo"],
            &pagination,
            100,
        )
        .await?;

        if !output.cache.no_cache {
            let cache = Cache::with_ttl(output.cache.effective_ttl_seconds())?;
            let _ = cache.set(CacheType::Teams, serde_json::json!(teams.clone()));
        }

        teams
    };

    if output.is_json() || output.has_template() {
        print_json_owned(serde_json::json!(teams), output)?;
        return Ok(());
    }

    let mut teams = teams;
    filter_values(&mut teams, &output.filters);

    if let Some(sort_key) = output.json.sort.as_deref() {
        sort_values(&mut teams, sort_key, output.json.order);
    }

    ensure_non_empty(&teams, output)?;
    if teams.is_empty() {
        println!("No teams found.");
        return Ok(());
    }

    let width = display_options().max_width(30);
    let rows: Vec<TeamRow> = teams
        .iter()
        .filter_map(|v| serde_json::from_value::<Team>(v.clone()).ok())
        .map(|t| TeamRow {
            name: truncate(safe_terminal_value(&t.name).as_str(), width),
            key: safe_terminal_value(&t.key),
            id: t.id,
        })
        .collect();

    let table = Table::new(rows).to_string();
    println!("{}", table);
    println!("\n{} teams", teams.len());

    Ok(())
}

async fn get_team(id: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;
    let resolved_id = resolve_team_id(&client, id, &output.cache).await?;

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

    let result = client
        .query(query, Some(json!({ "id": resolved_id })))
        .await?;
    let raw = &result["data"]["team"];

    if raw.is_null() {
        anyhow::bail!("Team not found: {}", id);
    }

    if output.is_json() || output.has_template() {
        print_json(raw, output)?;
        return Ok(());
    }

    let team: Team = serde_json::from_value(raw.clone())?;

    println!("{}", safe_terminal_value(&team.name).bold());
    println!("{}", "-".repeat(40));

    println!("Key: {}", safe_terminal_value(&team.key));

    if let Some(desc) = &team.description {
        if !desc.is_empty() {
            println!("Description: {}", safe_terminal_value(desc));
        }
    }

    println!("Private: {}", team.private.unwrap_or(false));

    if let Some(timezone) = &team.timezone {
        println!("Timezone: {}", safe_terminal_value(timezone));
    }

    if let Some(issue_count) = team.issue_count {
        println!("Issue Count: {}", issue_count);
    }

    if let Some(color) = &team.color {
        println!("Color: {}", safe_terminal_value(color));
    }

    if let Some(icon) = &team.icon {
        println!("Icon: {}", safe_terminal_value(icon));
    }

    println!("ID: {}", team.id);

    if let Some(created_at) = &team.created_at {
        println!("Created: {}", created_at);
    }

    if let Some(updated_at) = &team.updated_at {
        println!("Updated: {}", updated_at);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_terminal_value_removes_escape_sequences() {
        assert_eq!(safe_terminal_value("team\u{1b}]52;c;ZXZpbA==\u{7}"), "team");
    }
}

async fn get_teams(ids: &[String], output: &OutputOptions) -> Result<()> {
    if ids.len() == 1 {
        return get_team(&ids[0], output).await;
    }

    let client = LinearClient::new()?;

    use futures::stream::{self, StreamExt};
    let cache_opts = output.cache;
    let results: Vec<_> = stream::iter(ids.iter())
        .map(|id| {
            let client = client.clone();
            let id = id.clone();
            async move {
                let resolved = resolve_team_id(&client, &id, &cache_opts)
                    .await
                    .unwrap_or_else(|_| id.clone());
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
                let result = client.query(query, Some(json!({ "id": resolved }))).await;
                (id, result)
            }
        })
        .buffer_unordered(10)
        .collect()
        .await;

    if output.is_json() || output.has_template() {
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
        print_json_owned(serde_json::json!(teams), output)?;
        return Ok(());
    }

    let width = display_options().max_width(30);
    for (id, result) in results {
        match result {
            Ok(data) => {
                let raw = &data["data"]["team"];
                if raw.is_null() {
                    eprintln!("{} Team not found: {}", "!".yellow(), id);
                } else if let Ok(team) = serde_json::from_value::<Team>(raw.clone()) {
                    let name = truncate(safe_terminal_value(&team.name).as_str(), width);
                    println!(
                        "{} ({}) private={} id={}",
                        name.cyan(),
                        safe_terminal_value(&team.key),
                        team.private.unwrap_or(false),
                        id
                    );
                } else {
                    eprintln!("{} Failed to parse team: {}", "!".yellow(), id);
                }
            }
            Err(e) => {
                eprintln!("{} Error fetching {}: {}", "!".red(), id, e);
            }
        }
    }

    Ok(())
}

#[derive(Tabled)]
struct MemberRow {
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Email")]
    email: String,
    #[tabled(rename = "Role")]
    role: String,
    #[tabled(rename = "Active")]
    active: String,
}

async fn list_members(team: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;
    let team_id = resolve_team_id(&client, team, &output.cache).await?;

    let query = r#"
        query($id: String!) {
            team(id: $id) {
                name
                members(first: 100) {
                    nodes {
                        id
                        name
                        email
                        admin
                        active
                        displayName
                    }
                }
            }
        }
    "#;

    let result = client.query(query, Some(json!({ "id": team_id }))).await?;
    let team_data = &result["data"]["team"];

    if team_data.is_null() {
        anyhow::bail!("Team not found: {}", team);
    }

    let members = team_data["members"]["nodes"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    if output.is_json() || output.has_template() {
        print_json_owned(serde_json::json!(members), output)?;
        return Ok(());
    }

    let mut members = members;
    filter_values(&mut members, &output.filters);

    if let Some(sort_key) = output.json.sort.as_deref() {
        sort_values(&mut members, sort_key, output.json.order);
    }

    ensure_non_empty(&members, output)?;
    if members.is_empty() {
        println!("No members found.");
        return Ok(());
    }

    let team_name = team_data["name"].as_str().unwrap_or(team);
    let width = display_options().max_width(30);
    let rows: Vec<MemberRow> = members
        .iter()
        .map(|m| MemberRow {
            name: truncate(
                safe_terminal_value(
                    m["displayName"]
                        .as_str()
                        .or_else(|| m["name"].as_str())
                        .unwrap_or(""),
                )
                .as_str(),
                width,
            ),
            email: safe_terminal_value(m["email"].as_str().unwrap_or("")),
            role: if m["admin"].as_bool() == Some(true) {
                "Admin".to_string()
            } else {
                "Member".to_string()
            },
            active: if m["active"].as_bool() == Some(true) {
                "Yes".to_string()
            } else {
                "No".to_string()
            },
        })
        .collect();

    let table = Table::new(rows).to_string();
    println!("Team: {}\n", safe_terminal_value(team_name).bold());
    println!("{}", table);
    println!("\n{} members", members.len());

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn create_team(
    name: &str,
    key: Option<String>,
    description: Option<String>,
    icon: Option<String>,
    color: Option<String>,
    private: bool,
    timezone: Option<String>,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;

    let mut input = json!({ "name": name });
    if let Some(k) = &key {
        input["key"] = json!(k);
    }
    if let Some(d) = &description {
        input["description"] = json!(d);
    }
    if let Some(i) = &icon {
        input["icon"] = json!(i);
    }
    if let Some(c) = &color {
        input["color"] = json!(c);
    }
    if private {
        input["private"] = json!(true);
    }
    if let Some(tz) = &timezone {
        input["timezone"] = json!(tz);
    }

    let mutation = r#"
        mutation($input: TeamCreateInput!) {
            teamCreate(input: $input) {
                success
                team { id name key }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "input": input })))
        .await?;

    if result["data"]["teamCreate"]["success"].as_bool() == Some(true) {
        let team = &result["data"]["teamCreate"]["team"];
        if output.is_json() || output.has_template() {
            print_json(team, output)?;
            return Ok(());
        }
        let display_name = team["name"].as_str().unwrap_or(name);
        let display_key = team["key"].as_str().unwrap_or("");
        println!(
            "{} Created team: {} ({})",
            "+".green(),
            safe_terminal_value(display_name),
            safe_terminal_value(display_key)
        );
        println!("  ID: {}", team["id"].as_str().unwrap_or(""));
    } else {
        anyhow::bail!("Failed to create team");
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn update_team(
    id: &str,
    name: Option<String>,
    description: Option<String>,
    icon: Option<String>,
    color: Option<String>,
    private: Option<bool>,
    timezone: Option<String>,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;
    let resolved_id = resolve_team_id(&client, id, &output.cache).await?;

    let mut input = json!({});
    if let Some(n) = name {
        input["name"] = json!(n);
    }
    if let Some(d) = description {
        input["description"] = json!(d);
    }
    if let Some(i) = icon {
        input["icon"] = json!(i);
    }
    if let Some(c) = color {
        input["color"] = json!(c);
    }
    if let Some(p) = private {
        input["private"] = json!(p);
    }
    if let Some(tz) = timezone {
        input["timezone"] = json!(tz);
    }

    if input.as_object().map(|o| o.is_empty()).unwrap_or(true) {
        println!("No updates specified.");
        return Ok(());
    }

    let mutation = r#"
        mutation($id: String!, $input: TeamUpdateInput!) {
            teamUpdate(id: $id, input: $input) {
                success
                team { id name key }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "id": resolved_id, "input": input })))
        .await?;

    if result["data"]["teamUpdate"]["success"].as_bool() == Some(true) {
        let team = &result["data"]["teamUpdate"]["team"];
        if output.is_json() || output.has_template() {
            print_json(team, output)?;
            return Ok(());
        }
        println!("{} Team updated", "+".green());
        println!("  ID: {}", team["id"].as_str().unwrap_or(""));
        println!(
            "  Name: {}",
            safe_terminal_value(team["name"].as_str().unwrap_or(""))
        );
        println!(
            "  Key: {}",
            safe_terminal_value(team["key"].as_str().unwrap_or(""))
        );
    } else {
        anyhow::bail!("Failed to update team");
    }

    Ok(())
}

async fn delete_team(id: &str, force: bool, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;
    let resolved_id = resolve_team_id(&client, id, &output.cache).await?;

    if !force && !crate::is_yes() {
        anyhow::bail!(
            "Delete requires --force flag. Use: linear teams delete {} --force",
            id
        );
    }

    let mutation = r#"
        mutation($id: String!) {
            teamDelete(id: $id) {
                success
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "id": resolved_id })))
        .await?;

    let success = result["data"]["teamDelete"]["success"]
        .as_bool()
        .unwrap_or(false);

    if success {
        println!("Team {} deleted.", id);
    } else {
        anyhow::bail!("Failed to delete team {}", id);
    }

    Ok(())
}
