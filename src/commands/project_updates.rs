use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::json;
use tabled::{Table, Tabled};

use crate::api::{resolve_project_id, LinearClient};
use crate::display_options;
use crate::output::{
    ensure_non_empty, filter_values, print_json, print_json_owned, sort_values, OutputOptions,
};
use crate::text::truncate;

#[derive(Subcommand)]
pub enum ProjectUpdateCommands {
    /// List updates for a project
    #[command(alias = "ls")]
    List {
        /// Project ID, name, or slug
        project: String,
    },
    /// Get a specific update
    Get {
        /// Update ID
        id: String,
    },
    /// Create a project update
    Create {
        /// Project ID, name, or slug
        project: String,
        /// Update body (markdown)
        #[arg(short, long)]
        body: String,
        /// Project health: onTrack, atRisk, offTrack
        #[arg(short = 'H', long)]
        health: Option<String>,
    },
    /// Update a project update
    Update {
        /// Update ID
        id: String,
        /// New body
        #[arg(short, long)]
        body: Option<String>,
        /// New health status
        #[arg(short = 'H', long)]
        health: Option<String>,
    },
    /// Archive a project update
    Archive {
        /// Update ID
        id: String,
    },
    /// Unarchive a project update
    Unarchive {
        /// Update ID
        id: String,
    },
}

#[derive(Tabled)]
struct UpdateRow {
    #[tabled(rename = "Health")]
    health: String,
    #[tabled(rename = "Author")]
    author: String,
    #[tabled(rename = "Date")]
    date: String,
    #[tabled(rename = "Body")]
    body: String,
    #[tabled(rename = "ID")]
    id: String,
}

fn format_health(health: Option<&str>) -> String {
    match health {
        Some("onTrack") => "On Track".green().to_string(),
        Some("atRisk") => "At Risk".yellow().to_string(),
        Some("offTrack") => "Off Track".red().to_string(),
        Some(other) => other.to_string(),
        None => "-".to_string(),
    }
}

pub async fn handle(cmd: ProjectUpdateCommands, output: &OutputOptions) -> Result<()> {
    match cmd {
        ProjectUpdateCommands::List { project } => list_updates(&project, output).await,
        ProjectUpdateCommands::Get { id } => get_update(&id, output).await,
        ProjectUpdateCommands::Create {
            project,
            body,
            health,
        } => create_update(&project, &body, health, output).await,
        ProjectUpdateCommands::Update { id, body, health } => {
            update_update(&id, body, health, output).await
        }
        ProjectUpdateCommands::Archive { id } => archive_update(&id, output).await,
        ProjectUpdateCommands::Unarchive { id } => unarchive_update(&id, output).await,
    }
}

async fn list_updates(project: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;
    let project_id = resolve_project_id(&client, project, &output.cache).await?;

    let query = r#"
        query($projectId: String!) {
            project(id: $projectId) {
                name
                projectUpdates(first: 50) {
                    nodes {
                        id
                        body
                        health
                        createdAt
                        user { name }
                    }
                }
            }
        }
    "#;

    let result = client
        .query(query, Some(json!({ "projectId": project_id })))
        .await?;
    let project_data = &result["data"]["project"];

    if project_data.is_null() {
        anyhow::bail!("Project not found: {}", project);
    }

    let project_name = project_data["name"].as_str().unwrap_or(project);
    let updates = project_data["projectUpdates"]["nodes"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    if output.is_json() || output.has_template() {
        print_json_owned(
            json!({
                "project": project_name,
                "updates": updates
            }),
            output,
        )?;
        return Ok(());
    }

    if updates.is_empty() {
        println!("No updates found for project '{}'.", project_name);
        return Ok(());
    }

    let mut filtered: Vec<serde_json::Value> = updates;
    filter_values(&mut filtered, &output.filters);

    if let Some(sort_key) = output.json.sort.as_deref() {
        sort_values(&mut filtered, sort_key, output.json.order);
    }

    ensure_non_empty(&filtered, output)?;
    if filtered.is_empty() {
        println!("No updates match the filter criteria.");
        return Ok(());
    }

    let width = display_options().max_width(50);
    let rows: Vec<UpdateRow> = filtered
        .iter()
        .map(|u| UpdateRow {
            health: format_health(u["health"].as_str()),
            author: truncate(
                u["user"]["name"].as_str().unwrap_or("-"),
                display_options().max_width(20),
            ),
            date: u["createdAt"]
                .as_str()
                .map(|s| s.get(..10).unwrap_or(s).to_string())
                .unwrap_or_else(|| "-".to_string()),
            body: truncate(u["body"].as_str().unwrap_or(""), width),
            id: u["id"].as_str().unwrap_or("").to_string(),
        })
        .collect();

    println!(
        "{}",
        format!("Project Updates for '{}'", project_name).bold()
    );
    println!("{}", "-".repeat(40));

    let rows_len = rows.len();
    let table = Table::new(rows).to_string();
    println!("{}", table);
    println!("\n{} updates shown", rows_len);

    Ok(())
}

async fn get_update(id: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($id: String!) {
            projectUpdate(id: $id) {
                id
                body
                health
                createdAt
                updatedAt
                url
                project { name }
                user { name }
            }
        }
    "#;

    let result = client.query(query, Some(json!({ "id": id }))).await?;
    let raw = &result["data"]["projectUpdate"];

    if raw.is_null() {
        anyhow::bail!("Project update not found: {}", id);
    }

    if output.is_json() || output.has_template() {
        print_json(raw, output)?;
        return Ok(());
    }

    println!("{}", "Project Update".bold());
    println!("{}", "-".repeat(40));

    if let Some(project_name) = raw["project"]["name"].as_str() {
        println!("Project: {}", project_name);
    }
    if let Some(author) = raw["user"]["name"].as_str() {
        println!("Author: {}", author);
    }
    println!("Health: {}", format_health(raw["health"].as_str()));
    println!(
        "Created: {}",
        raw["createdAt"]
            .as_str()
            .map(|s| s.get(..10).unwrap_or(s))
            .unwrap_or("-")
    );
    if let Some(url) = raw["url"].as_str() {
        println!("URL: {}", url);
    }
    println!("ID: {}", id);

    if let Some(body) = raw["body"].as_str() {
        if !body.is_empty() {
            println!("\n{}", body);
        }
    }

    Ok(())
}

async fn create_update(
    project: &str,
    body: &str,
    health: Option<String>,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;
    let project_id = resolve_project_id(&client, project, &output.cache).await?;

    let mut input = json!({
        "projectId": project_id,
        "body": body,
    });
    if let Some(h) = &health {
        input["health"] = json!(h);
    }

    let mutation = r#"
        mutation($input: ProjectUpdateCreateInput!) {
            projectUpdateCreate(input: $input) {
                success
                projectUpdate { id health url }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "input": input })))
        .await?;

    if result["data"]["projectUpdateCreate"]["success"].as_bool() == Some(true) {
        let update = &result["data"]["projectUpdateCreate"]["projectUpdate"];
        if output.is_json() || output.has_template() {
            print_json(update, output)?;
            return Ok(());
        }
        println!("{} Project update created", "+".green());
        println!("  ID: {}", update["id"].as_str().unwrap_or(""));
        if let Some(url) = update["url"].as_str() {
            println!("  URL: {}", url);
        }
    } else {
        anyhow::bail!("Failed to create project update");
    }

    Ok(())
}

async fn update_update(
    id: &str,
    body: Option<String>,
    health: Option<String>,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;

    let mut input = json!({});
    if let Some(b) = body {
        input["body"] = json!(b);
    }
    if let Some(h) = health {
        input["health"] = json!(h);
    }

    if input.as_object().map(|o| o.is_empty()).unwrap_or(true) {
        println!("No updates specified.");
        return Ok(());
    }

    let mutation = r#"
        mutation($id: String!, $input: ProjectUpdateUpdateInput!) {
            projectUpdateUpdate(id: $id, input: $input) {
                success
                projectUpdate { id health }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "id": id, "input": input })))
        .await?;

    if result["data"]["projectUpdateUpdate"]["success"].as_bool() == Some(true) {
        let update = &result["data"]["projectUpdateUpdate"]["projectUpdate"];
        if output.is_json() || output.has_template() {
            print_json(update, output)?;
            return Ok(());
        }
        println!("{} Project update updated", "+".green());
    } else {
        anyhow::bail!("Failed to update project update");
    }

    Ok(())
}

async fn archive_update(id: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let mutation = r#"
        mutation($id: String!) {
            projectUpdateArchive(id: $id) {
                success
            }
        }
    "#;

    let result = client.mutate(mutation, Some(json!({ "id": id }))).await?;

    let success = result["data"]["projectUpdateArchive"]["success"]
        .as_bool()
        .unwrap_or(false);

    if success {
        if output.is_json() || output.has_template() {
            print_json_owned(json!({ "archived": id }), output)?;
            return Ok(());
        }
        println!("{} Project update archived", "+".green());
    } else {
        anyhow::bail!("Failed to archive project update {}", id);
    }

    Ok(())
}

async fn unarchive_update(id: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let mutation = r#"
        mutation($id: String!) {
            projectUpdateUnarchive(id: $id) {
                success
            }
        }
    "#;

    let result = client.mutate(mutation, Some(json!({ "id": id }))).await?;

    let success = result["data"]["projectUpdateUnarchive"]["success"]
        .as_bool()
        .unwrap_or(false);

    if success {
        if output.is_json() || output.has_template() {
            print_json_owned(json!({ "unarchived": id }), output)?;
            return Ok(());
        }
        println!("{} Project update unarchived", "+".green());
    } else {
        anyhow::bail!("Failed to unarchive project update {}", id);
    }

    Ok(())
}
