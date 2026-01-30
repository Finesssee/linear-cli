use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use tabled::{Table, Tabled};

use crate::api::LinearClient;
use crate::output::{print_json, OutputOptions};
use crate::pagination::PaginationOptions;
use crate::text::truncate;
use crate::DISPLAY_OPTIONS;

#[derive(Subcommand, Debug)]
pub enum InitiativeCommands {
    /// List all initiatives
    List,
    /// Get initiative details
    Get {
        /// Initiative ID
        id: String,
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
            .map(|i| InitiativeRow {
                id: i["id"].as_str().unwrap_or("-").to_string(),
                name: truncate(i["name"].as_str().unwrap_or("-"), max_width),
                status: i["status"].as_str().unwrap_or("-").to_string(),
                progress: format!(
                    "{}%",
                    (i["progress"].as_f64().unwrap_or(0.0) * 100.0) as i32
                ),
                project_count: i["projects"]["nodes"]
                    .as_array()
                    .map(|a| a.len().to_string())
                    .unwrap_or_else(|| "0".to_string()),
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
