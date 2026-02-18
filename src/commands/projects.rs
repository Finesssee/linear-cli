use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::json;
use tabled::{Table, Tabled};

use crate::api::{resolve_project_id, resolve_team_id, LinearClient};
use crate::cache::{Cache, CacheType};
use crate::display_options;
use crate::input::read_ids_from_stdin;
use crate::output::{
    ensure_non_empty, filter_values, print_json, print_json_owned, sort_values, OutputOptions,
};
use crate::pagination::paginate_nodes;
use crate::text::truncate;
use crate::types::Project;

#[derive(Subcommand)]
pub enum ProjectCommands {
    /// List all projects
    #[command(alias = "ls")]
    #[command(after_help = r#"EXAMPLES:
    linear projects list                       # List all projects
    linear p list --archived                   # Include archived projects
    linear p list --output json                # Output as JSON"#)]
    List {
        /// Show archived projects
        #[arg(short, long)]
        archived: bool,
        /// Apply a saved custom view's project filters
        #[arg(long)]
        view: Option<String>,
    },
    /// Get project details
    #[command(after_help = r#"EXAMPLES:
    linear projects get PROJECT_ID             # View by ID
    linear p get "Q1 Roadmap"                  # View by name
    linear p get PROJECT_ID --output json      # Output as JSON
    linear p get ID1 ID2 ID3                   # Get multiple projects
    echo "PROJECT_ID" | linear p get -         # Read ID from stdin"#)]
    Get {
        /// Project ID(s) or name(s). Use "-" to read from stdin.
        ids: Vec<String>,
    },
    /// Open project in browser
    Open {
        /// Project ID or name
        id: String,
    },
    /// Create a new project
    #[command(after_help = r##"EXAMPLES:
    linear projects create "Q1 Roadmap" -t ENG # Create project
    linear p create "Feature" -t ENG -d "Desc" # With description
    linear p create "UI" -t ENG -c "#FF5733"   # With color"##)]
    Create {
        /// Project name
        name: String,
        /// Team name or ID
        #[arg(short, long)]
        team: String,
        /// Project description
        #[arg(short, long)]
        description: Option<String>,
        /// Project color (hex)
        #[arg(short, long)]
        color: Option<String>,
    },
    /// Update a project
    #[command(after_help = r#"EXAMPLES:
    linear projects update ID -n "New Name"    # Rename project
    linear p update ID -d "New description"    # Update description"#)]
    Update {
        /// Project ID
        id: String,
        /// New name
        #[arg(short, long)]
        name: Option<String>,
        /// New description
        #[arg(short, long)]
        description: Option<String>,
        /// New color (hex)
        #[arg(short, long)]
        color: Option<String>,
        /// New icon
        #[arg(short, long)]
        icon: Option<String>,
        /// Preview without updating (dry run)
        #[arg(long)]
        dry_run: bool,
    },
    /// Delete a project
    #[command(after_help = r#"EXAMPLES:
    linear projects delete PROJECT_ID          # Delete with confirmation
    linear p delete PROJECT_ID --force         # Delete without confirmation"#)]
    Delete {
        /// Project ID
        id: String,
        /// Skip confirmation
        #[arg(short, long)]
        force: bool,
    },
    /// Add labels to a project
    #[command(after_help = r#"EXAMPLES:
    linear projects add-labels ID LABEL_ID     # Add one label
    linear p add-labels ID L1 L2 L3            # Add multiple labels"#)]
    AddLabels {
        /// Project ID
        id: String,
        /// Label IDs to add
        #[arg(required = true)]
        labels: Vec<String>,
    },
    /// List project members
    Members {
        /// Project ID or name
        id: String,
    },
}

#[derive(Tabled)]
struct ProjectRow {
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Labels")]
    labels: String,
    #[tabled(rename = "ID")]
    id: String,
}

pub async fn handle(cmd: ProjectCommands, output: &OutputOptions) -> Result<()> {
    match cmd {
        ProjectCommands::List { archived, view } => list_projects(archived, view, output).await,
        ProjectCommands::Get { ids } => {
            let final_ids = read_ids_from_stdin(ids);
            if final_ids.is_empty() {
                anyhow::bail!("No project IDs provided. Provide IDs or pipe them via stdin.");
            }
            get_projects(&final_ids, output).await
        }
        ProjectCommands::Open { id } => open_project(&id, &output.cache).await,
        ProjectCommands::Create {
            name,
            team,
            description,
            color,
        } => create_project(&name, &team, description, color, output).await,
        ProjectCommands::Update {
            id,
            name,
            description,
            color,
            icon,
            dry_run,
        } => {
            let dry_run = dry_run || output.dry_run;
            update_project(&id, name, description, color, icon, dry_run, output).await
        }
        ProjectCommands::Delete { id, force } => delete_project(&id, force).await,
        ProjectCommands::AddLabels { id, labels } => add_labels(&id, labels, output).await,
        ProjectCommands::Members { id } => list_project_members(&id, output).await,
    }
}

async fn list_projects(
    include_archived: bool,
    view: Option<String>,
    output: &OutputOptions,
) -> Result<()> {
    // If --view is specified, use the view's project filter
    if let Some(ref view_name) = view {
        let client = LinearClient::new()?;
        let filter_data =
            super::views::fetch_view_project_filter(&client, view_name, &output.cache).await?;

        let query = r#"
            query($filter: ProjectFilter, $includeArchived: Boolean, $first: Int, $after: String, $last: Int, $before: String) {
                projects(first: $first, after: $after, last: $last, before: $before, includeArchived: $includeArchived, filter: $filter) {
                    nodes {
                        id
                        name
                        state
                        url
                        startDate
                        targetDate
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

        let mut vars = serde_json::Map::new();
        vars.insert("includeArchived".to_string(), json!(include_archived));
        vars.insert("filter".to_string(), filter_data);

        let pagination = output.pagination.with_default_limit(50);
        let mut projects = paginate_nodes(
            &client,
            query,
            vars,
            &["data", "projects", "nodes"],
            &["data", "projects", "pageInfo"],
            &pagination,
            50,
        )
        .await?;

        if output.is_json() || output.has_template() {
            print_json_owned(serde_json::json!(projects), output)?;
            return Ok(());
        }

        filter_values(&mut projects, &output.filters);
        if let Some(sort_key) = output.json.sort.as_deref() {
            sort_values(&mut projects, sort_key, output.json.order);
        }

        ensure_non_empty(&projects, output)?;
        if projects.is_empty() {
            println!("No projects found matching view filters.");
            return Ok(());
        }

        return print_project_table(&projects);
    }

    let can_use_cache = !output.cache.no_cache
        && !include_archived
        && output.pagination.after.is_none()
        && output.pagination.before.is_none()
        && !output.pagination.all
        && output.pagination.page_size.is_none()
        && output.pagination.limit.is_none();

    let cached: Vec<serde_json::Value> = if can_use_cache {
        let cache = Cache::with_ttl(output.cache.effective_ttl_seconds())?;
        cache
            .get(CacheType::Projects)
            .and_then(|data| data.as_array().cloned())
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let mut projects = if !cached.is_empty() {
        cached
    } else {
        let client = LinearClient::new()?;

        // Simplified query to reduce GraphQL complexity (was exceeding 10000 limit)
        let query = r#"
            query($includeArchived: Boolean, $first: Int, $after: String, $last: Int, $before: String) {
                projects(first: $first, after: $after, last: $last, before: $before, includeArchived: $includeArchived) {
                    nodes {
                        id
                        name
                        state
                        url
                        startDate
                        targetDate
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

        let mut vars = serde_json::Map::new();
        vars.insert("includeArchived".to_string(), json!(include_archived));

        let pagination = output.pagination.with_default_limit(50);
        let projects = paginate_nodes(
            &client,
            query,
            vars,
            &["data", "projects", "nodes"],
            &["data", "projects", "pageInfo"],
            &pagination,
            50,
        )
        .await?;

        if can_use_cache {
            let cache = Cache::with_ttl(output.cache.effective_ttl_seconds())?;
            let _ = cache.set(CacheType::Projects, serde_json::json!(projects.clone()));
        }

        projects
    };

    if output.is_json() || output.has_template() {
        print_json_owned(serde_json::json!(projects), output)?;
        return Ok(());
    }

    filter_values(&mut projects, &output.filters);
    if let Some(sort_key) = output.json.sort.as_deref() {
        sort_values(&mut projects, sort_key, output.json.order);
    }

    ensure_non_empty(&projects, output)?;
    if projects.is_empty() {
        println!("No projects found.");
        return Ok(());
    }

    let width = display_options().max_width(50);
    let rows: Vec<ProjectRow> = projects
        .iter()
        .filter_map(|v| serde_json::from_value::<Project>(v.clone()).ok())
        .map(|p| ProjectRow {
            name: truncate(&p.name, width),
            status: p.state.unwrap_or_else(|| "-".to_string()),
            labels: "-".to_string(),
            id: p.id,
        })
        .collect();

    let table = Table::new(rows).to_string();
    println!("{}", table);
    println!("\n{} projects", projects.len());

    Ok(())
}

fn print_project_table(projects: &[serde_json::Value]) -> Result<()> {
    let width = display_options().max_width(50);
    let rows: Vec<ProjectRow> = projects
        .iter()
        .filter_map(|v| serde_json::from_value::<Project>(v.clone()).ok())
        .map(|p| ProjectRow {
            name: truncate(&p.name, width),
            status: p.state.unwrap_or_else(|| "-".to_string()),
            labels: "-".to_string(),
            id: p.id,
        })
        .collect();

    let table = Table::new(rows).to_string();
    println!("{}", table);
    println!("\n{} projects", projects.len());

    Ok(())
}

async fn open_project(id: &str, cache: &crate::cache::CacheOptions) -> Result<()> {
    let client = LinearClient::new()?;
    let resolved_id = resolve_project_id(&client, id, cache).await?;

    let query = r#"
        query($id: String!) {
            project(id: $id) {
                name
                url
            }
        }
    "#;
    let result = client.query(query, Some(json!({ "id": resolved_id }))).await?;
    let project = &result["data"]["project"];

    if project.is_null() {
        anyhow::bail!("Project not found: {}", id);
    }

    let url = project["url"].as_str().unwrap_or("");
    if url.is_empty() {
        anyhow::bail!("No URL for project: {}", id);
    }

    let name = project["name"].as_str().unwrap_or(id);
    println!("Opening project '{}' in browser...", name);
    open::that(url)?;
    Ok(())
}

async fn get_project(id: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;
    let resolved_id = resolve_project_id(&client, id, &output.cache).await?;

    let query = r#"
        query($id: String!) {
            project(id: $id) {
                id
                name
                description
                icon
                color
                url
                status { name }
                labels { nodes { id name color parent { name } } }
            }
        }
    "#;

    let result = client
        .query(query, Some(json!({ "id": resolved_id })))
        .await?;
    let project = &result["data"]["project"];

    if project.is_null() {
        anyhow::bail!("Project not found: {}", id);
    }

    // Handle JSON output
    if output.is_json() || output.has_template() {
        print_json(project, output)?;
        return Ok(());
    }

    let proj: Project = serde_json::from_value(project.clone())?;

    println!("{}", proj.name.bold());
    println!("{}", "-".repeat(40));

    if let Some(desc) = &proj.description {
        if !desc.is_empty() {
            println!(
                "Description: {}",
                desc.chars().take(100).collect::<String>()
            );
        }
    }

    println!(
        "Status: {}",
        proj.status.as_ref().map(|s| s.name.as_str()).unwrap_or("-")
    );
    println!("Color: {}", proj.color.as_deref().unwrap_or("-"));
    println!("Icon: {}", proj.icon.as_deref().unwrap_or("-"));
    println!("URL: {}", proj.url.as_deref().unwrap_or("-"));
    println!("ID: {}", proj.id);

    if let Some(label_conn) = &proj.labels {
        if !label_conn.nodes.is_empty() {
            println!("\nLabels:");
            for label in &label_conn.nodes {
                let parent_name = label.parent.as_ref().map(|p| p.name.as_str()).unwrap_or("");
                if parent_name.is_empty() {
                    println!("  - {}", label.name);
                } else {
                    println!("  - {} > {}", parent_name.dimmed(), label.name);
                }
            }
        }
    }

    Ok(())
}

async fn get_projects(ids: &[String], output: &OutputOptions) -> Result<()> {
    if ids.len() == 1 {
        return get_project(&ids[0], output).await;
    }

    let client = LinearClient::new()?;

    use futures::stream::{self, StreamExt};
    let cache_opts = output.cache;
    let results: Vec<_> = stream::iter(ids.iter())
        .map(|id| {
            let client = client.clone();
            let id = id.clone();
            async move {
                let resolved = resolve_project_id(&client, &id, &cache_opts)
                    .await
                    .unwrap_or_else(|_| id.clone());
                let query = r#"
                    query($id: String!) {
                        project(id: $id) {
                            id
                            name
                            description
                            status { name }
                            url
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
        let projects: Vec<_> = results
            .iter()
            .filter_map(|(_, r)| {
                r.as_ref().ok().and_then(|data| {
                    let project = &data["data"]["project"];
                    if !project.is_null() {
                        Some(project.clone())
                    } else {
                        None
                    }
                })
            })
            .collect();
        print_json_owned(serde_json::json!(projects), output)?;
        return Ok(());
    }

    let width = display_options().max_width(50);
    for (id, result) in results {
        match result {
            Ok(data) => {
                let project = &data["data"]["project"];
                if project.is_null() {
                    eprintln!("{} Project not found: {}", "!".yellow(), id);
                } else if let Ok(proj) = serde_json::from_value::<Project>(project.clone()) {
                    let name = truncate(&proj.name, width);
                    let status = proj.status.as_ref().map(|s| s.name.as_str()).unwrap_or("-");
                    println!("{} [{}] {}", name.cyan(), status, id);
                } else {
                    eprintln!("{} Failed to parse project: {}", "!".yellow(), id);
                }
            }
            Err(e) => {
                eprintln!("{} Error fetching {}: {}", "!".red(), id, e);
            }
        }
    }

    Ok(())
}

async fn create_project(
    name: &str,
    team: &str,
    description: Option<String>,
    color: Option<String>,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;

    // Resolve team key/name to UUID
    let team_id = resolve_team_id(&client, team, &output.cache).await?;

    let mut input = json!({
        "name": name,
        "teamIds": [team_id]
    });

    if let Some(desc) = description {
        input["description"] = json!(desc);
    }
    if let Some(c) = color {
        input["color"] = json!(c);
    }

    let mutation = r#"
        mutation($input: ProjectCreateInput!) {
            projectCreate(input: $input) {
                success
                project { id name url }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "input": input })))
        .await?;

    if result["data"]["projectCreate"]["success"].as_bool() == Some(true) {
        let project = &result["data"]["projectCreate"]["project"];

        // Handle JSON output
        if output.is_json() || output.has_template() {
            print_json(project, output)?;
            return Ok(());
        }

        println!(
            "{} Created project: {}",
            "+".green(),
            project["name"].as_str().unwrap_or("")
        );
        println!("  ID: {}", project["id"].as_str().unwrap_or(""));
        println!("  URL: {}", project["url"].as_str().unwrap_or(""));

        // Invalidate projects cache after successful create
        let _ = Cache::new().and_then(|c| c.clear_type(CacheType::Projects));
    } else {
        anyhow::bail!("Failed to create project");
    }

    Ok(())
}

async fn update_project(
    id: &str,
    name: Option<String>,
    description: Option<String>,
    color: Option<String>,
    icon: Option<String>,
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
    if let Some(c) = color {
        input["color"] = json!(c);
    }
    if let Some(i) = icon {
        input["icon"] = json!(i);
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
                    "would_update": {
                        "id": id,
                        "input": input,
                    }
                }),
                output,
            )?;
        } else {
            println!("{}", "[DRY RUN] Would update project:".yellow().bold());
            println!("  ID: {}", id);
        }
        return Ok(());
    }

    let mutation = r#"
        mutation($id: String!, $input: ProjectUpdateInput!) {
            projectUpdate(id: $id, input: $input) {
                success
                project { id name }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "id": id, "input": input })))
        .await?;

    if result["data"]["projectUpdate"]["success"].as_bool() == Some(true) {
        let project = &result["data"]["projectUpdate"]["project"];

        // Handle JSON output
        if output.is_json() || output.has_template() {
            print_json(project, output)?;
            return Ok(());
        }

        println!("{} Project updated", "+".green());

        // Invalidate projects cache after successful update
        let _ = Cache::new().and_then(|c| c.clear_type(CacheType::Projects));
    } else {
        anyhow::bail!("Failed to update project");
    }

    Ok(())
}

async fn delete_project(id: &str, force: bool) -> Result<()> {
    if !force {
        println!("Are you sure you want to delete project {}?", id);
        println!("This action cannot be undone. Use --force to skip this prompt.");
        return Ok(());
    }

    let client = LinearClient::new()?;

    let mutation = r#"
        mutation($id: String!) {
            projectDelete(id: $id) {
                success
            }
        }
    "#;

    let result = client.mutate(mutation, Some(json!({ "id": id }))).await?;

    if result["data"]["projectDelete"]["success"].as_bool() == Some(true) {
        println!("{} Project deleted", "+".green());

        // Invalidate projects cache after successful delete
        let _ = Cache::new().and_then(|c| c.clear_type(CacheType::Projects));
    } else {
        anyhow::bail!("Failed to delete project");
    }

    Ok(())
}

async fn add_labels(id: &str, label_ids: Vec<String>, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let mutation = r#"
        mutation($id: String!, $input: ProjectUpdateInput!) {
            projectUpdate(id: $id, input: $input) {
                success
                project {
                    name
                    labels { nodes { name } }
                }
            }
        }
    "#;

    let input = json!({ "labelIds": label_ids });
    let result = client
        .mutate(mutation, Some(json!({ "id": id, "input": input })))
        .await?;

    if result["data"]["projectUpdate"]["success"].as_bool() == Some(true) {
        let project = &result["data"]["projectUpdate"]["project"];

        // Handle JSON output
        if output.is_json() || output.has_template() {
            print_json(project, output)?;
            return Ok(());
        }

        let empty = vec![];
        let labels: Vec<&str> = project["labels"]["nodes"]
            .as_array()
            .unwrap_or(&empty)
            .iter()
            .filter_map(|l| l["name"].as_str())
            .collect();
        println!("{} Labels updated: {}", "+".green(), labels.join(", "));
    } else {
        anyhow::bail!("Failed to add labels");
    }

    Ok(())
}

async fn list_project_members(id: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;
    let project_id = resolve_project_id(&client, id, &output.cache).await?;

    let query = r#"
        query($id: String!) {
            project(id: $id) {
                name
                members(first: 100) {
                    nodes {
                        id
                        name
                        email
                        displayName
                    }
                }
            }
        }
    "#;

    let result = client
        .query(query, Some(json!({ "id": project_id })))
        .await?;
    let project = &result["data"]["project"];

    if project.is_null() {
        anyhow::bail!("Project not found: {}", id);
    }

    let members = project["members"]["nodes"]
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

    let proj_name = project["name"].as_str().unwrap_or(id);
    let width = display_options().max_width(30);

    #[derive(Tabled)]
    struct MemberRow {
        #[tabled(rename = "Name")]
        name: String,
        #[tabled(rename = "Email")]
        email: String,
    }

    let rows: Vec<MemberRow> = members
        .iter()
        .map(|m| MemberRow {
            name: truncate(
                m["displayName"]
                    .as_str()
                    .or_else(|| m["name"].as_str())
                    .unwrap_or(""),
                width,
            ),
            email: m["email"].as_str().unwrap_or("").to_string(),
        })
        .collect();

    let table = Table::new(rows).to_string();
    println!("Project: {}\n", proj_name.bold());
    println!("{}", table);
    println!("\n{} members", members.len());

    Ok(())
}
