use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::{json, Value};
use tabled::{Table, Tabled};

use crate::api::{resolve_project_id, LinearClient};
use crate::display_options;
use crate::output::{
    ensure_non_empty, filter_values, print_json, print_json_owned, sort_values, OutputOptions,
};
use crate::text::truncate;

#[derive(Subcommand)]
pub enum MilestoneCommands {
    /// List milestones for a project
    #[command(alias = "ls")]
    List {
        /// Project name or ID
        #[arg(short, long)]
        project: String,
    },
    /// Get milestone details
    Get {
        /// Milestone ID
        id: String,
    },
    /// Create a new project milestone
    Create {
        /// Milestone name
        name: String,
        /// Project name or ID
        #[arg(short, long)]
        project: String,
        /// Description
        #[arg(short, long)]
        description: Option<String>,
        /// Target date (today, +2w, 2024-06-01, etc.)
        #[arg(long)]
        target_date: Option<String>,
        /// Sort order
        #[arg(long)]
        sort_order: Option<f64>,
    },
    /// Update a project milestone
    Update {
        /// Milestone ID
        id: String,
        /// New name
        #[arg(short, long)]
        name: Option<String>,
        /// New description
        #[arg(short, long)]
        description: Option<String>,
        /// New target date
        #[arg(long)]
        target_date: Option<String>,
        /// New sort order
        #[arg(long)]
        sort_order: Option<f64>,
    },
    /// Delete a project milestone
    Delete {
        /// Milestone ID
        id: String,
        /// Skip confirmation
        #[arg(long)]
        force: bool,
    },
}

#[derive(Tabled)]
struct MilestoneRow {
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Target Date")]
    target_date: String,
    #[tabled(rename = "ID")]
    id: String,
}

pub async fn handle(cmd: MilestoneCommands, output: &OutputOptions) -> Result<()> {
    match cmd {
        MilestoneCommands::List { project } => list_milestones(&project, output).await,
        MilestoneCommands::Get { id } => get_milestone(&id, output).await,
        MilestoneCommands::Create {
            name,
            project,
            description,
            target_date,
            sort_order,
        } => {
            create_milestone(
                &name,
                &project,
                description,
                target_date,
                sort_order,
                output,
            )
            .await
        }
        MilestoneCommands::Update {
            id,
            name,
            description,
            target_date,
            sort_order,
        } => update_milestone(&id, name, description, target_date, sort_order, output).await,
        MilestoneCommands::Delete { id, force } => delete_milestone(&id, force).await,
    }
}

async fn list_milestones(project: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;
    let project_id = resolve_project_id(&client, project, &output.cache).await?;

    let query = r#"
        query($projectId: String!) {
            project(id: $projectId) {
                projectMilestones {
                    nodes {
                        id
                        name
                        description
                        targetDate
                        sortOrder
                        createdAt
                        updatedAt
                    }
                }
            }
        }
    "#;

    let result = client
        .query(query, Some(json!({ "projectId": project_id })))
        .await?;
    let empty = vec![];
    let milestones = result["data"]["project"]["projectMilestones"]["nodes"]
        .as_array()
        .unwrap_or(&empty)
        .clone();

    if output.is_json() || output.has_template() {
        print_json_owned(json!(milestones), output)?;
        return Ok(());
    }

    let mut milestones = milestones;
    filter_values(&mut milestones, &output.filters);
    if let Some(sort_key) = output.json.sort.as_deref() {
        sort_values(&mut milestones, sort_key, output.json.order);
    }

    ensure_non_empty(&milestones, output)?;
    if milestones.is_empty() {
        println!("No milestones found.");
        return Ok(());
    }

    let name_width = display_options().max_width(40);
    let rows: Vec<MilestoneRow> = milestones
        .iter()
        .map(|m| MilestoneRow {
            name: truncate(m["name"].as_str().unwrap_or(""), name_width),
            target_date: m["targetDate"].as_str().unwrap_or("-").to_string(),
            id: m["id"].as_str().unwrap_or("").to_string(),
        })
        .collect();

    let table = Table::new(rows).to_string();
    println!("{}", table);
    println!("\n{} milestones", milestones.len());

    Ok(())
}

async fn get_milestone(id: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($id: String!) {
            projectMilestone(id: $id) {
                id
                name
                description
                targetDate
                sortOrder
                createdAt
                updatedAt
                project {
                    id
                    name
                }
            }
        }
    "#;

    let result = client.query(query, Some(json!({ "id": id }))).await?;
    let milestone = &result["data"]["projectMilestone"];

    if milestone.is_null() {
        anyhow::bail!("Milestone not found: {}", id);
    }

    if output.is_json() || output.has_template() {
        print_json(milestone, output)?;
        return Ok(());
    }

    println!("{}", milestone["name"].as_str().unwrap_or("").bold());
    println!("{}", "-".repeat(40));

    if let Some(desc) = milestone["description"].as_str() {
        if !desc.is_empty() {
            println!("Description: {}", desc);
        }
    }
    println!(
        "Target Date: {}",
        milestone["targetDate"].as_str().unwrap_or("-")
    );
    if let Some(project) = milestone["project"]["name"].as_str() {
        println!("Project: {}", project);
    }
    if let Some(created) = milestone["createdAt"].as_str() {
        println!("Created: {}", created);
    }
    println!("ID: {}", milestone["id"].as_str().unwrap_or(""));

    Ok(())
}

async fn create_milestone(
    name: &str,
    project: &str,
    description: Option<String>,
    target_date: Option<String>,
    sort_order: Option<f64>,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;
    let project_id = resolve_project_id(&client, project, &output.cache).await?;

    let mut input = json!({
        "name": name,
        "projectId": project_id,
    });

    if let Some(desc) = description {
        input["description"] = json!(desc);
    }
    if let Some(date) = target_date {
        let parsed = crate::dates::parse_due_date(&date)
            .ok_or_else(|| anyhow::anyhow!("Invalid target date: '{}'", date))?;
        input["targetDate"] = json!(parsed);
    }
    if let Some(order) = sort_order {
        input["sortOrder"] = json!(order);
    }

    if output.dry_run {
        println!("Dry run: would create milestone");
        print_json_owned(input, output)?;
        return Ok(());
    }

    let mutation = r#"
        mutation($input: ProjectMilestoneCreateInput!) {
            projectMilestoneCreate(input: $input) {
                success
                projectMilestone {
                    id
                    name
                    targetDate
                }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "input": input })))
        .await?;
    let created = &result["data"]["projectMilestoneCreate"]["projectMilestone"];

    if created.is_null() {
        anyhow::bail!("Failed to create milestone");
    }

    if output.is_json() || output.has_template() {
        print_json(created, output)?;
    } else {
        println!(
            "Created milestone: {} ({})",
            created["name"].as_str().unwrap_or(""),
            created["id"].as_str().unwrap_or("")
        );
    }

    Ok(())
}

async fn update_milestone(
    id: &str,
    name: Option<String>,
    description: Option<String>,
    target_date: Option<String>,
    sort_order: Option<f64>,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;

    let mut input: serde_json::Map<String, Value> = serde_json::Map::new();

    if let Some(n) = name {
        input.insert("name".to_string(), json!(n));
    }
    if let Some(desc) = description {
        input.insert("description".to_string(), json!(desc));
    }
    if let Some(date) = target_date {
        let parsed = crate::dates::parse_due_date(&date)
            .ok_or_else(|| anyhow::anyhow!("Invalid target date: '{}'", date))?;
        input.insert("targetDate".to_string(), json!(parsed));
    }
    if let Some(order) = sort_order {
        input.insert("sortOrder".to_string(), json!(order));
    }

    if input.is_empty() {
        anyhow::bail!(
            "No fields to update. Specify --name, --description, --target-date, or --sort-order."
        );
    }

    if output.dry_run {
        println!("Dry run: would update milestone {}", id);
        print_json_owned(Value::Object(input), output)?;
        return Ok(());
    }

    let mutation = r#"
        mutation($id: String!, $input: ProjectMilestoneUpdateInput!) {
            projectMilestoneUpdate(id: $id, input: $input) {
                success
                projectMilestone {
                    id
                    name
                    targetDate
                }
            }
        }
    "#;

    let result = client
        .mutate(
            mutation,
            Some(json!({ "id": id, "input": Value::Object(input) })),
        )
        .await?;
    let updated = &result["data"]["projectMilestoneUpdate"]["projectMilestone"];

    if updated.is_null() {
        anyhow::bail!("Failed to update milestone");
    }

    if output.is_json() || output.has_template() {
        print_json(updated, output)?;
    } else {
        println!(
            "Updated milestone: {} ({})",
            updated["name"].as_str().unwrap_or(""),
            updated["id"].as_str().unwrap_or("")
        );
    }

    Ok(())
}

async fn delete_milestone(id: &str, force: bool) -> Result<()> {
    if !force && !crate::is_yes() {
        anyhow::bail!(
            "Delete requires --force flag. Use: linear milestones delete {} --force",
            id
        );
    }

    let client = LinearClient::new()?;

    let mutation = r#"
        mutation($id: String!) {
            projectMilestoneDelete(id: $id) {
                success
            }
        }
    "#;

    let result = client.mutate(mutation, Some(json!({ "id": id }))).await?;

    let success = result["data"]["projectMilestoneDelete"]["success"]
        .as_bool()
        .unwrap_or(false);

    if success {
        println!("Milestone {} deleted.", id);
    } else {
        anyhow::bail!("Failed to delete milestone {}", id);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_milestone_commands_exist() {
        // Verify the enum variants exist (compile-time check)
        use super::MilestoneCommands;
        let _list = MilestoneCommands::List {
            project: "test".to_string(),
        };
        let _get = MilestoneCommands::Get {
            id: "test".to_string(),
        };
    }
}
