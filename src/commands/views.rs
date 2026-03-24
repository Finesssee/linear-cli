use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::json;
use tabled::{Table, Tabled};

use crate::api::{resolve_team_id, resolve_view_id, LinearClient};
use crate::display_options;
use crate::output::{
    ensure_non_empty, filter_values, print_json, print_json_owned, sort_values, OutputOptions,
};
use crate::pagination::paginate_nodes;
use crate::text::truncate;
use crate::types::CustomView;

#[derive(Subcommand)]
pub enum ViewCommands {
    /// List custom views
    #[command(alias = "ls")]
    #[command(after_help = r#"EXAMPLES:
    linear views list                       # List all custom views
    linear v list --shared                  # List shared views only
    linear v list --team ENG                # Filter by team"#)]
    List {
        /// Filter by team name or ID
        #[arg(short, long)]
        team: Option<String>,
        /// Show only shared views
        #[arg(long)]
        shared: bool,
    },
    /// Get custom view details
    #[command(after_help = r#"EXAMPLES:
    linear views get "My View"              # Get by name
    linear v get VIEW_ID                    # Get by ID"#)]
    Get {
        /// View name or ID
        name_or_id: String,
    },
    /// Create a custom view
    #[command(after_help = r#"EXAMPLES:
    linear views create "Bug Triage"        # Create a personal view
    linear v create "Sprint View" --shared  # Create a shared view
    linear v create "Team Bugs" -t ENG --filter-json filters.json"#)]
    Create {
        /// View name
        name: String,
        /// View description
        #[arg(short, long)]
        description: Option<String>,
        /// Team name or ID to scope the view to
        #[arg(short, long)]
        team: Option<String>,
        /// Make the view shared (visible to all workspace members)
        #[arg(long)]
        shared: bool,
        /// Path to JSON file with filter data, or "-" for stdin
        #[arg(long)]
        filter_json: Option<String>,
        /// View icon
        #[arg(long)]
        icon: Option<String>,
        /// View color (hex)
        #[arg(long)]
        color: Option<String>,
    },
    /// Update a custom view
    #[command(after_help = r#"EXAMPLES:
    linear views update "My View" --name "New Name"
    linear v update VIEW_ID --shared --description "Updated""#)]
    Update {
        /// View name or ID
        name_or_id: String,
        /// New name
        #[arg(short, long)]
        name: Option<String>,
        /// New description
        #[arg(short, long)]
        description: Option<String>,
        /// Set shared visibility
        #[arg(long)]
        shared: Option<bool>,
        /// Path to JSON file with filter data, or "-" for stdin
        #[arg(long)]
        filter_json: Option<String>,
    },
    /// Delete a custom view
    #[command(after_help = r#"EXAMPLES:
    linear views delete "My View"           # Delete with confirmation
    linear v delete VIEW_ID --force         # Delete without confirmation"#)]
    Delete {
        /// View name or ID
        name_or_id: String,
        /// Skip confirmation
        #[arg(long)]
        force: bool,
    },
}

#[derive(Tabled)]
struct ViewRow {
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Shared")]
    shared: String,
    #[tabled(rename = "Owner")]
    owner: String,
    #[tabled(rename = "Team")]
    team: String,
    #[tabled(rename = "Updated")]
    updated: String,
    #[tabled(rename = "ID")]
    id: String,
}

pub async fn handle(cmd: ViewCommands, output: &OutputOptions) -> Result<()> {
    match cmd {
        ViewCommands::List { team, shared } => list_views(team, shared, output).await,
        ViewCommands::Get { name_or_id } => get_view(&name_or_id, output).await,
        ViewCommands::Create {
            name,
            description,
            team,
            shared,
            filter_json,
            icon,
            color,
        } => {
            create_view(
                &name,
                description,
                team,
                shared,
                filter_json,
                icon,
                color,
                output,
            )
            .await
        }
        ViewCommands::Update {
            name_or_id,
            name,
            description,
            shared,
            filter_json,
        } => update_view(&name_or_id, name, description, shared, filter_json, output).await,
        ViewCommands::Delete { name_or_id, force } => delete_view(&name_or_id, force, output).await,
    }
}

async fn list_views(team: Option<String>, shared_only: bool, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($first: Int, $after: String) {
            customViews(first: $first, after: $after) {
                nodes {
                    id name description icon color shared slugId modelName
                    filterData projectFilterData
                    owner { id name }
                    team { id key name }
                    createdAt updatedAt
                }
                pageInfo { hasNextPage endCursor }
            }
        }
    "#;

    let vars = serde_json::Map::new();
    let pagination = output.pagination.with_default_limit(100);
    let mut views = paginate_nodes(
        &client,
        query,
        vars,
        &["data", "customViews", "nodes"],
        &["data", "customViews", "pageInfo"],
        &pagination,
        100,
    )
    .await?;

    // Filter by shared
    if shared_only {
        views.retain(|v| v["shared"].as_bool() == Some(true));
    }

    // Filter by team
    if let Some(ref team_filter) = team {
        let team_id = resolve_team_id(&client, team_filter, &output.cache).await?;
        views.retain(|v| v["team"]["id"].as_str() == Some(team_id.as_str()));
    }

    if output.is_json() || output.has_template() {
        print_json_owned(json!(views), output)?;
        return Ok(());
    }

    filter_values(&mut views, &output.filters);

    if let Some(sort_key) = output.json.sort.as_deref() {
        sort_values(&mut views, sort_key, output.json.order);
    }

    ensure_non_empty(&views, output)?;
    if views.is_empty() {
        println!("No custom views found.");
        return Ok(());
    }

    let width = display_options().max_width(30);
    let rows: Vec<ViewRow> = views
        .iter()
        .map(|v| {
            let updated = v["updatedAt"]
                .as_str()
                .unwrap_or("")
                .chars()
                .take(10)
                .collect::<String>();

            ViewRow {
                name: truncate(v["name"].as_str().unwrap_or(""), width),
                shared: if v["shared"].as_bool() == Some(true) {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
                owner: truncate(v["owner"]["name"].as_str().unwrap_or("-"), width),
                team: truncate(v["team"]["name"].as_str().unwrap_or("-"), width),
                updated,
                id: v["id"].as_str().unwrap_or("").to_string(),
            }
        })
        .collect();

    let table = Table::new(rows).to_string();
    println!("{}", table);
    println!("\n{} views", views.len());

    Ok(())
}

async fn get_view(name_or_id: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;
    let view_id = resolve_view_id(&client, name_or_id, &output.cache).await?;

    let query = r#"
        query($id: String!) {
            customView(id: $id) {
                id name description icon color shared slugId modelName
                filterData projectFilterData
                owner { id name }
                team { id key name }
                createdAt updatedAt
            }
        }
    "#;

    let result = client.query(query, Some(json!({ "id": view_id }))).await?;
    let view = &result["data"]["customView"];

    if view.is_null() {
        anyhow::bail!("Custom view not found: {}", name_or_id);
    }

    if output.is_json() || output.has_template() {
        print_json(view, output)?;
        return Ok(());
    }

    let cv: CustomView = serde_json::from_value(view.clone())?;

    println!("{}", cv.name.bold());
    println!("{}", "-".repeat(40));

    if let Some(desc) = &cv.description {
        println!("Description: {}", desc);
    }

    println!("Shared: {}", if cv.shared { "Yes" } else { "No" });

    if let Some(owner) = &cv.owner {
        println!("Owner: {}", owner.name);
    }

    if let Some(team) = &cv.team {
        println!("Team: {} ({})", team.name, team.key);
    }

    if let Some(icon) = &cv.icon {
        println!("Icon: {}", icon);
    }

    if let Some(color) = &cv.color {
        println!("Color: {}", color);
    }

    if let Some(model) = &cv.model_name {
        println!("Model: {}", model);
    }

    println!("ID: {}", cv.id);

    if let Some(created) = &cv.created_at {
        println!("Created: {}", created.chars().take(10).collect::<String>());
    }

    if let Some(updated) = &cv.updated_at {
        println!("Updated: {}", updated.chars().take(10).collect::<String>());
    }

    if let Some(ref filter) = cv.filter_data {
        if !filter.is_null() {
            println!("\n{}", "Filter Data".bold());
            println!("{}", "-".repeat(40));
            println!("{}", serde_json::to_string_pretty(filter)?);
        }
    }

    if let Some(ref filter) = cv.project_filter_data {
        if !filter.is_null() {
            println!("\n{}", "Project Filter Data".bold());
            println!("{}", "-".repeat(40));
            println!("{}", serde_json::to_string_pretty(filter)?);
        }
    }

    Ok(())
}

fn read_filter_json(path: &str) -> Result<serde_json::Value> {
    let content = if path == "-" {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        buf
    } else {
        std::fs::read_to_string(path)?
    };
    let value: serde_json::Value = serde_json::from_str(&content)?;
    Ok(value)
}

#[allow(clippy::too_many_arguments)]
async fn create_view(
    name: &str,
    description: Option<String>,
    team: Option<String>,
    shared: bool,
    filter_json: Option<String>,
    icon: Option<String>,
    color: Option<String>,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;

    let mut input = json!({
        "name": name,
        "shared": shared,
    });

    if let Some(desc) = description {
        input["description"] = json!(desc);
    }

    if let Some(ref t) = team {
        let team_id = resolve_team_id(&client, t, &output.cache).await?;
        input["teamId"] = json!(team_id);
    }

    if let Some(ref path) = filter_json {
        let filter = read_filter_json(path)?;
        input["filterData"] = filter;
    }

    if let Some(i) = icon {
        input["icon"] = json!(i);
    }

    if let Some(c) = color {
        input["color"] = json!(c);
    }

    if output.dry_run {
        if output.is_json() || output.has_template() {
            print_json_owned(
                json!({
                    "dry_run": true,
                    "would_create": { "input": input }
                }),
                output,
            )?;
        } else {
            println!("{}", "[DRY RUN] Would create custom view:".yellow().bold());
            println!("  Name: {}", name);
        }
        return Ok(());
    }

    let mutation = r#"
        mutation($input: CustomViewCreateInput!) {
            customViewCreate(input: $input) {
                success
                customView { id name shared }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "input": input })))
        .await?;

    if result["data"]["customViewCreate"]["success"].as_bool() == Some(true) {
        let view = &result["data"]["customViewCreate"]["customView"];
        if output.is_json() || output.has_template() {
            print_json(view, output)?;
            return Ok(());
        }
        println!(
            "{} Created custom view: {}",
            "+".green(),
            view["name"].as_str().unwrap_or("")
        );
        println!("  ID: {}", view["id"].as_str().unwrap_or(""));
        println!(
            "  Shared: {}",
            if view["shared"].as_bool() == Some(true) {
                "Yes"
            } else {
                "No"
            }
        );
    } else {
        anyhow::bail!("Failed to create custom view");
    }

    Ok(())
}

async fn update_view(
    name_or_id: &str,
    name: Option<String>,
    description: Option<String>,
    shared: Option<bool>,
    filter_json: Option<String>,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;
    let view_id = resolve_view_id(&client, name_or_id, &output.cache).await?;

    let mut input = json!({});
    if let Some(n) = name {
        input["name"] = json!(n);
    }
    if let Some(d) = description {
        input["description"] = json!(d);
    }
    if let Some(s) = shared {
        input["shared"] = json!(s);
    }
    if let Some(ref path) = filter_json {
        let filter = read_filter_json(path)?;
        input["filterData"] = filter;
    }

    if input.as_object().map(|o| o.is_empty()).unwrap_or(true) {
        println!("No updates specified.");
        return Ok(());
    }

    if output.dry_run {
        if output.is_json() || output.has_template() {
            print_json_owned(
                json!({
                    "dry_run": true,
                    "would_update": {
                        "id": view_id,
                        "input": input,
                    }
                }),
                output,
            )?;
        } else {
            println!("{}", "[DRY RUN] Would update custom view:".yellow().bold());
            println!("  ID: {}", view_id);
        }
        return Ok(());
    }

    let mutation = r#"
        mutation($id: String!, $input: CustomViewUpdateInput!) {
            customViewUpdate(id: $id, input: $input) {
                success
                customView { id name }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "id": view_id, "input": input })))
        .await?;

    if result["data"]["customViewUpdate"]["success"].as_bool() == Some(true) {
        if output.is_json() || output.has_template() {
            print_json(&result["data"]["customViewUpdate"]["customView"], output)?;
            return Ok(());
        }
        println!("{} Custom view updated", "+".green());
    } else {
        anyhow::bail!("Failed to update custom view");
    }

    Ok(())
}

async fn delete_view(name_or_id: &str, force: bool, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;
    let view_id = resolve_view_id(&client, name_or_id, &output.cache).await?;

    if output.dry_run {
        if output.is_json() || output.has_template() {
            print_json_owned(
                json!({
                    "dry_run": true,
                    "would_delete": { "id": view_id }
                }),
                output,
            )?;
        } else {
            println!("{}", "[DRY RUN] Would delete custom view:".yellow().bold());
            println!("  ID: {}", view_id);
        }
        return Ok(());
    }

    if !force && !crate::is_yes() {
        use dialoguer::Confirm;
        let confirm = Confirm::new()
            .with_prompt(format!("Delete custom view {}?", name_or_id))
            .default(false)
            .interact()?;
        if !confirm {
            println!("Cancelled.");
            return Ok(());
        }
    }

    let mutation = r#"
        mutation($id: String!) {
            customViewDelete(id: $id) {
                success
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "id": view_id })))
        .await?;

    if result["data"]["customViewDelete"]["success"].as_bool() == Some(true) {
        if output.is_json() || output.has_template() {
            print_json_owned(json!({ "deleted": true, "id": view_id }), output)?;
            return Ok(());
        }
        println!("{} Custom view deleted: {}", "-".red(), name_or_id);
    } else {
        anyhow::bail!("Failed to delete custom view");
    }

    Ok(())
}

/// Fetch the filterData for a custom view (used by issues list --view).
pub async fn fetch_view_filter(
    client: &LinearClient,
    view_name_or_id: &str,
    cache_opts: &crate::cache::CacheOptions,
) -> Result<serde_json::Value> {
    let view_id = resolve_view_id(client, view_name_or_id, cache_opts).await?;

    let query = r#"
        query($id: String!) {
            customView(id: $id) {
                filterData
            }
        }
    "#;

    let result = client.query(query, Some(json!({ "id": view_id }))).await?;
    let filter = &result["data"]["customView"]["filterData"];

    if filter.is_null() {
        anyhow::bail!("Custom view '{}' has no filter data", view_name_or_id);
    }

    Ok(filter.clone())
}

/// Fetch the projectFilterData for a custom view (used by projects list --view).
pub async fn fetch_view_project_filter(
    client: &LinearClient,
    view_name_or_id: &str,
    cache_opts: &crate::cache::CacheOptions,
) -> Result<serde_json::Value> {
    let view_id = resolve_view_id(client, view_name_or_id, cache_opts).await?;

    let query = r#"
        query($id: String!) {
            customView(id: $id) {
                projectFilterData
            }
        }
    "#;

    let result = client.query(query, Some(json!({ "id": view_id }))).await?;
    let filter = &result["data"]["customView"]["projectFilterData"];

    if filter.is_null() {
        anyhow::bail!(
            "Custom view '{}' has no project filter data",
            view_name_or_id
        );
    }

    Ok(filter.clone())
}
