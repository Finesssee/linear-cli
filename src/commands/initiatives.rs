use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use tabled::{Table, Tabled};

use crate::api::LinearClient;
use crate::output::{print_json, print_json_owned, OutputOptions};
use crate::pagination::PaginationOptions;
use crate::text::truncate;
use crate::types::Initiative;
use crate::DISPLAY_OPTIONS;
use colored::Colorize;

#[derive(Subcommand, Debug)]
pub enum InitiativeCommands {
    /// List all initiatives
    List,
    /// Get initiative details
    Get {
        /// Initiative ID
        id: String,
    },
    /// Create a new initiative
    Create {
        /// Initiative name
        name: String,
        /// Description
        #[arg(short, long)]
        description: Option<String>,
        /// Status
        #[arg(short, long)]
        status: Option<String>,
    },
    /// Update an existing initiative
    Update {
        /// Initiative ID
        id: String,
        /// New name
        #[arg(short, long)]
        name: Option<String>,
        /// New description
        #[arg(short, long)]
        description: Option<String>,
        /// New status
        #[arg(short, long)]
        status: Option<String>,
        /// Preview without updating (dry run)
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Tabled)]
struct InitiativeRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Progress")]
    progress: String,
    #[tabled(rename = "Projects")]
    project_count: String,
}

pub async fn handle(
    cmd: InitiativeCommands,
    output: &OutputOptions,
    _pagination: &PaginationOptions,
) -> Result<()> {
    match cmd {
        InitiativeCommands::List => list_initiatives(output).await,
        InitiativeCommands::Get { id } => get_initiative(&id, output).await,
        InitiativeCommands::Create {
            name,
            description,
            status,
        } => create_initiative(&name, description, status, output).await,
        InitiativeCommands::Update {
            id,
            name,
            description,
            status,
            dry_run,
        } => {
            let dry_run = dry_run || output.dry_run;
            update_initiative(&id, name, description, status, dry_run, output).await
        }
    }
}

async fn list_initiatives(output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query {
            initiatives(first: 100) {
                nodes {
                    id
                    name
                    description
                    status
                    sortOrder
                    projects {
                        nodes {
                            id
                        }
                    }
                }
            }
        }
    "#;

    let result = client.query(query, None).await?;
    let initiatives = &result["data"]["initiatives"]["nodes"];

    if output.is_json() {
        print_json(initiatives, output)?;
    } else {
        let display = DISPLAY_OPTIONS.get().cloned().unwrap_or_default();
        let max_width = display.max_width(40);

        let rows: Vec<InitiativeRow> = initiatives
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|v| {
                let i = serde_json::from_value::<Initiative>(v.clone()).ok()?;
                let progress = format!(
                    "{}%",
                    (v["progress"].as_f64().unwrap_or(0.0) * 100.0) as i32
                );
                let project_count = v["projects"]["nodes"]
                    .as_array()
                    .map(|a| a.len().to_string())
                    .unwrap_or_else(|| "0".to_string());
                Some(InitiativeRow {
                    id: i.id,
                    name: truncate(&i.name, max_width),
                    status: i.status.as_deref().unwrap_or("-").to_string(),
                    progress,
                    project_count,
                })
            })
            .collect();

        if rows.is_empty() {
            println!("No initiatives found");
        } else {
            println!("{}", Table::new(rows));
        }
    }

    Ok(())
}

async fn get_initiative(id: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($id: String!) {
            initiative(id: $id) {
                id
                name
                description
                status
                sortOrder
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
    let initiative = &result["data"]["initiative"];

    if initiative.is_null() {
        anyhow::bail!("Initiative not found: {}", id);
    }

    print_json(initiative, output)?;
    Ok(())
}

async fn create_initiative(
    name: &str,
    description: Option<String>,
    status: Option<String>,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;

    let mut input = json!({ "name": name });
    if let Some(d) = description {
        input["description"] = json!(d);
    }
    if let Some(s) = status {
        input["status"] = json!(s);
    }

    let mutation = r#"
        mutation($input: InitiativeCreateInput!) {
            initiativeCreate(input: $input) {
                success
                initiative { id name }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "input": input })))
        .await?;

    if result["data"]["initiativeCreate"]["success"].as_bool() == Some(true) {
        let initiative = &result["data"]["initiativeCreate"]["initiative"];
        if output.is_json() || output.has_template() {
            print_json(initiative, output)?;
            return Ok(());
        }
        println!(
            "{} Created initiative: {}",
            "+".green(),
            initiative["name"].as_str().unwrap_or("")
        );
        println!("  ID: {}", initiative["id"].as_str().unwrap_or(""));
    } else {
        anyhow::bail!("Failed to create initiative");
    }

    Ok(())
}

async fn update_initiative(
    id: &str,
    name: Option<String>,
    description: Option<String>,
    status: Option<String>,
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
    if let Some(s) = status {
        input["status"] = json!(s);
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
            println!("{}", "[DRY RUN] Would update initiative:".yellow().bold());
            println!("  ID: {}", id);
        }
        return Ok(());
    }

    let mutation = r#"
        mutation($id: String!, $input: InitiativeUpdateInput!) {
            initiativeUpdate(id: $id, input: $input) {
                success
                initiative { id name }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "id": id, "input": input })))
        .await?;

    if result["data"]["initiativeUpdate"]["success"].as_bool() == Some(true) {
        if output.is_json() || output.has_template() {
            print_json(&result["data"]["initiativeUpdate"]["initiative"], output)?;
            return Ok(());
        }
        println!("{} Initiative updated", "+".green());
    } else {
        anyhow::bail!("Failed to update initiative");
    }

    Ok(())
}
