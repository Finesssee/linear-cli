use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::{json, Value};
use tabled::{Table, Tabled};

use crate::api::LinearClient;
use crate::cache::{Cache, CacheType};
use crate::display_options;
use crate::output::{
    ensure_non_empty, filter_values, print_json, print_json_owned, sort_values, OutputOptions,
};
use crate::pagination::paginate_nodes;
use crate::text::truncate;
use crate::types::Label;

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
    /// Update a label's name or color
    Update {
        /// Label ID
        id: String,
        /// Label type: issue or project
        #[arg(short, long, default_value = "project")]
        r#type: String,
        /// New label name
        #[arg(short, long)]
        name: Option<String>,
        /// New color (hex, e.g. "#FF5733")
        #[arg(short, long)]
        color: Option<String>,
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
        LabelCommands::Update {
            id,
            r#type,
            name,
            color,
        } => update_label(&id, &r#type, name, color).await,
    }
}

async fn list_labels(label_type: &str, output: &OutputOptions) -> Result<()> {
    let can_use_cache = !output.cache.no_cache
        && output.pagination.after.is_none()
        && output.pagination.before.is_none()
        && !output.pagination.all
        && output.pagination.page_size.is_none()
        && output.pagination.limit.is_none();

    let cached: Vec<Value> = if can_use_cache {
        let cache = Cache::with_ttl(output.cache.effective_ttl_seconds())?;
        cache
            .get_keyed(CacheType::Labels, label_type)
            .and_then(|data| data.as_array().cloned())
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let mut labels = if !cached.is_empty() {
        cached
    } else {
        let client = LinearClient::new()?;

        let query = if label_type == "project" {
            r#"
                query($first: Int, $after: String, $last: Int, $before: String) {
                    projectLabels(first: $first, after: $after, last: $last, before: $before) {
                        nodes {
                            id
                            name
                            color
                            parent { name }
                        }
                        pageInfo {
                            hasNextPage
                            endCursor
                            hasPreviousPage
                            startCursor
                        }
                    }
                }
            "#
        } else {
            r#"
                query($first: Int, $after: String, $last: Int, $before: String) {
                    issueLabels(first: $first, after: $after, last: $last, before: $before) {
                        nodes {
                            id
                            name
                            color
                            parent { name }
                        }
                        pageInfo {
                            hasNextPage
                            endCursor
                            hasPreviousPage
                            startCursor
                        }
                    }
                }
            "#
        };

        let key = if label_type == "project" {
            "projectLabels"
        } else {
            "issueLabels"
        };

        let pagination = output.pagination.with_default_limit(100);
        let labels = paginate_nodes(
            &client,
            query,
            serde_json::Map::new(),
            &["data", key, "nodes"],
            &["data", key, "pageInfo"],
            &pagination,
            100,
        )
        .await?;

        if can_use_cache {
            let cache = Cache::with_ttl(output.cache.effective_ttl_seconds())?;
            let _ = cache.set_keyed(
                CacheType::Labels,
                label_type,
                serde_json::json!(labels.clone()),
            );
        }

        labels
    };

    if output.is_json() || output.has_template() {
        print_json_owned(serde_json::json!(labels), output)?;
        return Ok(());
    }

    filter_values(&mut labels, &output.filters);
    if let Some(sort_key) = output.json.sort.as_deref() {
        sort_values(&mut labels, sort_key, output.json.order);
    }

    ensure_non_empty(&labels, output)?;
    if labels.is_empty() {
        println!("No {} labels found.", label_type);
        return Ok(());
    }

    let width = display_options().max_width(30);
    let rows: Vec<LabelRow> = labels
        .iter()
        .filter_map(|v| serde_json::from_value::<Label>(v.clone()).ok())
        .map(|l| LabelRow {
            name: truncate(&l.name, width),
            group: truncate(
                l.parent.as_ref().map(|p| p.name.as_str()).unwrap_or("-"),
                width,
            ),
            color: l.color.unwrap_or_default(),
            id: l.id,
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
        if output.is_json() || output.has_template() {
            print_json(label, output)?;
            return Ok(());
        }

        println!(
            "{} Created {} label: {}",
            "+".green(),
            label_type,
            label["name"].as_str().unwrap_or("")
        );
        println!("  ID: {}", label["id"].as_str().unwrap_or(""));

        // Invalidate labels cache after successful create
        let _ = Cache::new().and_then(|c| c.clear_type(CacheType::Labels));
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

        // Invalidate labels cache after successful delete
        let _ = Cache::new().and_then(|c| c.clear_type(CacheType::Labels));
    } else {
        anyhow::bail!("Failed to delete label");
    }

    Ok(())
}

async fn update_label(
    id: &str,
    label_type: &str,
    name: Option<String>,
    color: Option<String>,
) -> Result<()> {
    if name.is_none() && color.is_none() {
        println!("No updates specified. Use --name or --color.");
        return Ok(());
    }

    let client = LinearClient::new()?;

    let mut input = json!({});
    if let Some(n) = &name {
        input["name"] = json!(n);
    }
    if let Some(c) = &color {
        input["color"] = json!(c);
    }

    let mutation = if label_type == "project" {
        r#"
            mutation($id: String!, $input: ProjectLabelUpdateInput!) {
                projectLabelUpdate(id: $id, input: $input) {
                    success
                    projectLabel { id name color }
                }
            }
        "#
    } else {
        r#"
            mutation($id: String!, $input: IssueLabelUpdateInput!) {
                issueLabelUpdate(id: $id, input: $input) {
                    success
                    issueLabel { id name color }
                }
            }
        "#
    };

    let result = client
        .mutate(mutation, Some(json!({ "id": id, "input": input })))
        .await?;

    let key = if label_type == "project" {
        "projectLabelUpdate"
    } else {
        "issueLabelUpdate"
    };
    let label_key = if label_type == "project" {
        "projectLabel"
    } else {
        "issueLabel"
    };

    if result["data"][key]["success"].as_bool() == Some(true) {
        let label = &result["data"][key][label_key];
        println!(
            "{} Updated {} label: {}",
            "+".green(),
            label_type,
            label["name"].as_str().unwrap_or("")
        );

        let _ = Cache::new().and_then(|c| c.clear_type(CacheType::Labels));
    } else {
        anyhow::bail!("Failed to update label");
    }

    Ok(())
}
