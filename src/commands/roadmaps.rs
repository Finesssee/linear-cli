use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::json;
use tabled::{Table, Tabled};

use crate::api::LinearClient;
use crate::output::{print_json, print_json_owned, OutputOptions};
use crate::pagination::PaginationOptions;
use crate::text::truncate;
use crate::types::Roadmap;
use crate::DISPLAY_OPTIONS;

#[derive(Subcommand, Debug)]
pub enum RoadmapCommands {
    /// List all roadmaps
    List,
    /// Get roadmap details
    Get {
        /// Roadmap ID
        id: String,
    },
    /// Create a new roadmap
    Create {
        /// Roadmap name
        name: String,
        /// Description
        #[arg(short, long)]
        description: Option<String>,
    },
    /// Update an existing roadmap
    Update {
        /// Roadmap ID
        id: String,
        /// New name
        #[arg(short, long)]
        name: Option<String>,
        /// New description
        #[arg(short, long)]
        description: Option<String>,
        /// Preview without updating (dry run)
        #[arg(long)]
        dry_run: bool,
    },
    /// Delete a roadmap
    Delete {
        /// Roadmap ID
        id: String,
        /// Skip confirmation
        #[arg(long)]
        force: bool,
    },
}

#[derive(Tabled)]
struct RoadmapRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Description")]
    description: String,
    #[tabled(rename = "Projects")]
    project_count: String,
}

pub async fn handle(
    cmd: RoadmapCommands,
    output: &OutputOptions,
    pagination: &PaginationOptions,
) -> Result<()> {
    match cmd {
        RoadmapCommands::List => list_roadmaps(output, pagination).await,
        RoadmapCommands::Get { id } => get_roadmap(&id, output).await,
        RoadmapCommands::Create { name, description } => {
            create_roadmap(&name, description, output).await
        }
        RoadmapCommands::Update {
            id,
            name,
            description,
            dry_run,
        } => {
            let dry_run = dry_run || output.dry_run;
            update_roadmap(&id, name, description, dry_run, output).await
        }
        RoadmapCommands::Delete { id, force } => delete_roadmap(&id, force).await,
    }
}

async fn list_roadmaps(output: &OutputOptions, pagination: &PaginationOptions) -> Result<()> {
    let client = LinearClient::new()?;
    let pagination = pagination.with_default_limit(250);
    let limit = pagination.limit.unwrap_or(250);

    let query = r#"
        query($first: Int) {
            roadmaps(first: $first) {
                nodes {
                    id
                    name
                    description
                    slugId
                    projects {
                        nodes {
                            id
                        }
                    }
                }
            }
        }
    "#;

    let mut variables = serde_json::Map::new();
    variables.insert("first".to_string(), json!(limit));
    let result = client.query(query, Some(serde_json::Value::Object(variables))).await?;
    let roadmaps = &result["data"]["roadmaps"]["nodes"];

    if output.is_json() {
        print_json(roadmaps, output)?;
    } else {
        let display = DISPLAY_OPTIONS.get().cloned().unwrap_or_default();
        let max_width = display.max_width(40);

        let rows: Vec<RoadmapRow> = roadmaps
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|v| {
                let r = serde_json::from_value::<Roadmap>(v.clone()).ok()?;
                let project_count = v["projects"]["nodes"]
                    .as_array()
                    .map(|a| a.len().to_string())
                    .unwrap_or_else(|| "0".to_string());
                Some(RoadmapRow {
                    id: r.id,
                    name: truncate(&r.name, max_width),
                    description: truncate(r.description.as_deref().unwrap_or("-"), max_width),
                    project_count,
                })
            })
            .collect();

        if rows.is_empty() {
            println!("No roadmaps found");
        } else {
            println!("{}", Table::new(rows));
        }
    }

    Ok(())
}

async fn get_roadmap(id: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($id: String!) {
            roadmap(id: $id) {
                id
                name
                description
                slugId
                createdAt
                updatedAt
                projects {
                    nodes {
                        id
                        name
                        state
                        progress
                    }
                }
            }
        }
    "#;

    let result = client.query(query, Some(json!({ "id": id }))).await?;
    let roadmap = &result["data"]["roadmap"];

    if roadmap.is_null() {
        anyhow::bail!("Roadmap not found: {}", id);
    }

    print_json(roadmap, output)?;
    Ok(())
}

async fn create_roadmap(
    name: &str,
    description: Option<String>,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;

    let mut input = json!({ "name": name });
    if let Some(d) = description {
        input["description"] = json!(d);
    }

    let mutation = r#"
        mutation($input: RoadmapCreateInput!) {
            roadmapCreate(input: $input) {
                success
                roadmap { id name }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "input": input })))
        .await?;

    if result["data"]["roadmapCreate"]["success"].as_bool() == Some(true) {
        let roadmap = &result["data"]["roadmapCreate"]["roadmap"];
        if output.is_json() || output.has_template() {
            print_json(roadmap, output)?;
            return Ok(());
        }
        println!(
            "{} Created roadmap: {}",
            "+".green(),
            roadmap["name"].as_str().unwrap_or("")
        );
        println!("  ID: {}", roadmap["id"].as_str().unwrap_or(""));
    } else {
        anyhow::bail!("Failed to create roadmap");
    }

    Ok(())
}

async fn update_roadmap(
    id: &str,
    name: Option<String>,
    description: Option<String>,
    dry_run: bool,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;

    let mut input = json!({});
    if let Some(n) = name {
        input["name"] = json!(n);
    }
    if let Some(d) = description {
        input["description"] = json!(d);
    }

    if input.as_object().map(|o| o.is_empty()).unwrap_or(true) {
        println!("No updates specified.");
        return Ok(());
    }

    if dry_run {
        if output.is_json() || output.has_template() {
            print_json_owned(
                json!({
                    "dry_run": true,
                    "would_update": { "id": id, "input": input }
                }),
                output,
            )?;
        } else {
            println!("{}", "[DRY RUN] Would update roadmap:".yellow().bold());
            println!("  ID: {}", id);
        }
        return Ok(());
    }

    let mutation = r#"
        mutation($id: String!, $input: RoadmapUpdateInput!) {
            roadmapUpdate(id: $id, input: $input) {
                success
                roadmap { id name }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "id": id, "input": input })))
        .await?;

    if result["data"]["roadmapUpdate"]["success"].as_bool() == Some(true) {
        if output.is_json() || output.has_template() {
            print_json(&result["data"]["roadmapUpdate"]["roadmap"], output)?;
            return Ok(());
        }
        println!("{} Roadmap updated", "+".green());
    } else {
        anyhow::bail!("Failed to update roadmap");
    }

    Ok(())
}

async fn delete_roadmap(id: &str, force: bool) -> Result<()> {
    if !force && !crate::is_yes() {
        anyhow::bail!("Delete requires --force flag. Use: linear roadmaps delete {} --force", id);
    }

    let client = LinearClient::new()?;

    let mutation = r#"
        mutation($id: String!) {
            roadmapDelete(id: $id) {
                success
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "id": id })))
        .await?;

    let success = result["data"]["roadmapDelete"]["success"]
        .as_bool()
        .unwrap_or(false);

    if success {
        println!("Roadmap {} deleted.", id);
    } else {
        anyhow::bail!("Failed to delete roadmap {}", id);
    }

    Ok(())
}
