use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::json;
use tabled::{Table, Tabled};

use crate::api::LinearClient;
use crate::display_options;
use crate::output::{print_json, OutputOptions};
use crate::text::truncate;

#[derive(Subcommand)]
pub enum LabelCommands {
    /// List labels
    #[command(alias = "ls")]
    #[command(after_help = r#"EXAMPLES:
    linear labels list                         # List project labels
    linear l list --type issue                 # List issue labels
    linear l list --output json                # Output as JSON"#)]
    List {
        /// Label type: issue or project
        #[arg(short, long, default_value = "project")]
        r#type: String,
    },
    /// Create a new label
    #[command(after_help = r##"EXAMPLES:
    linear labels create "Feature"             # Create project label
    linear l create "Bug" --type issue         # Create issue label
    linear l create "UI" -c "#FF5733"          # With custom color
    linear l create "Sub" -p PARENT_ID         # As child of parent"##)]
    Create {
        /// Label name
        name: String,
        /// Label type: issue or project
        #[arg(short, long, default_value = "project")]
        r#type: String,
        /// Label color (hex)
        #[arg(short, long, default_value = "#6B7280")]
        color: String,
        /// Parent label ID (for grouped labels)
        #[arg(short, long)]
        parent: Option<String>,
    },
    /// Delete a label
    #[command(after_help = r#"EXAMPLES:
    linear labels delete LABEL_ID              # Delete with confirmation
    linear l delete LABEL_ID --force           # Delete without confirmation
    linear l delete LABEL_ID --type issue      # Delete issue label"#)]
    Delete {
        /// Label ID
        id: String,
        /// Label type: issue or project
        #[arg(short, long, default_value = "project")]
        r#type: String,
        /// Skip confirmation
        #[arg(short, long)]
        force: bool,
    },
}

#[derive(Tabled)]
struct LabelRow {
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Group")]
    group: String,
    #[tabled(rename = "Color")]
    color: String,
    #[tabled(rename = "ID")]
    id: String,
}

pub async fn handle(cmd: LabelCommands, output: &OutputOptions) -> Result<()> {
    match cmd {
        LabelCommands::List { r#type } => list_labels(&r#type, output).await,
        LabelCommands::Create {
            name,
            r#type,
            color,
            parent,
        } => create_label(&name, &r#type, &color, parent, output).await,
        LabelCommands::Delete { id, r#type, force } => delete_label(&id, &r#type, force).await,
    }
}

async fn list_labels(label_type: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let query = if label_type == "project" {
        r#"
            query {
                projectLabels(first: 100) {
                    nodes {
                        id
                        name
                        color
                        parent { name }
                    }
                }
            }
        "#
    } else {
        r#"
            query {
                issueLabels(first: 100) {
                    nodes {
                        id
                        name
                        color
                        parent { name }
                    }
                }
            }
        "#
    };

    let result = client.query(query, None).await?;

    let key = if label_type == "project" {
        "projectLabels"
    } else {
        "issueLabels"
    };

    // Handle JSON output
    if output.is_json() {
        print_json(&result["data"][key]["nodes"], &output.json)?;
        return Ok(());
    }

    let empty = vec![];
    let labels = result["data"][key]["nodes"].as_array().unwrap_or(&empty);

    if labels.is_empty() {
        println!("No {} labels found.", label_type);
        return Ok(());
    }

    let width = display_options().max_width(30);
    let rows: Vec<LabelRow> = labels
        .iter()
        .map(|l| LabelRow {
            name: truncate(l["name"].as_str().unwrap_or(""), width),
            group: truncate(l["parent"]["name"].as_str().unwrap_or("-"), width),
            color: l["color"].as_str().unwrap_or("").to_string(),
            id: l["id"].as_str().unwrap_or("").to_string(),
        })
        .collect();

    let table = Table::new(rows).to_string();
    println!("{}", table);
    println!("\n{} {} labels", labels.len(), label_type);

    Ok(())
}

async fn create_label(
    name: &str,
    label_type: &str,
    color: &str,
    parent: Option<String>,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;

    let mut input = json!({
        "name": name,
        "color": color
    });

    if let Some(p) = parent {
        input["parentId"] = json!(p);
    }

    let mutation = if label_type == "project" {
        r#"
            mutation($input: ProjectLabelCreateInput!) {
                projectLabelCreate(input: $input) {
                    success
                    projectLabel { id name color }
                }
            }
        "#
    } else {
        r#"
            mutation($input: IssueLabelCreateInput!) {
                issueLabelCreate(input: $input) {
                    success
                    issueLabel { id name color }
                }
            }
        "#
    };

    let result = client
        .mutate(mutation, Some(json!({ "input": input })))
        .await?;

    let key = if label_type == "project" {
        "projectLabelCreate"
    } else {
        "issueLabelCreate"
    };
    let label_key = if label_type == "project" {
        "projectLabel"
    } else {
        "issueLabel"
    };

    if result["data"][key]["success"].as_bool() == Some(true) {
        let label = &result["data"][key][label_key];

        // Handle JSON output
        if output.is_json() {
            print_json(label, &output.json)?;
            return Ok(());
        }

        println!(
            "{} Created {} label: {}",
            "+".green(),
            label_type,
            label["name"].as_str().unwrap_or("")
        );
        println!("  ID: {}", label["id"].as_str().unwrap_or(""));
    } else {
        anyhow::bail!("Failed to create label");
    }

    Ok(())
}

async fn delete_label(id: &str, label_type: &str, force: bool) -> Result<()> {
    if !force {
        println!(
            "Are you sure you want to delete {} label {}?",
            label_type, id
        );
        println!("Use --force to skip this prompt.");
        return Ok(());
    }

    let client = LinearClient::new()?;

    let mutation = if label_type == "project" {
        r#"
            mutation($id: String!) {
                projectLabelDelete(id: $id) {
                    success
                }
            }
        "#
    } else {
        r#"
            mutation($id: String!) {
                issueLabelDelete(id: $id) {
                    success
                }
            }
        "#
    };

    let result = client.mutate(mutation, Some(json!({ "id": id }))).await?;

    let key = if label_type == "project" {
        "projectLabelDelete"
    } else {
        "issueLabelDelete"
    };

    if result["data"][key]["success"].as_bool() == Some(true) {
        println!("{} Label deleted", "+".green());
    } else {
        anyhow::bail!("Failed to delete label");
    }

    Ok(())
}
