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
pub enum RoadmapCommands {
    /// List all roadmaps
    List,
    /// Get roadmap details
    Get {
        /// Roadmap ID
        id: String,
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
    _pagination: &PaginationOptions,
) -> Result<()> {
    match cmd {
        RoadmapCommands::List => list_roadmaps(output).await,
        RoadmapCommands::Get { id } => get_roadmap(&id, output).await,
    }
}

async fn list_roadmaps(output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query {
            roadmaps(first: 100) {
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

    let result = client.query(query, None).await?;
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
            .map(|r| RoadmapRow {
                id: r["id"].as_str().unwrap_or("-").to_string(),
                name: truncate(r["name"].as_str().unwrap_or("-"), max_width),
                description: truncate(r["description"].as_str().unwrap_or("-"), max_width),
                project_count: r["projects"]["nodes"]
                    .as_array()
                    .map(|a| a.len().to_string())
                    .unwrap_or_else(|| "0".to_string()),
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
