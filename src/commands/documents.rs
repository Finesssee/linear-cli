use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::json;
use tabled::{Table, Tabled};

use crate::api::LinearClient;
use crate::display_options;
use crate::input::read_ids_from_stdin;
use crate::output::{
    ensure_non_empty, filter_values, print_json, print_json_owned, sort_values, OutputOptions,
};
use crate::pagination::paginate_nodes;
use crate::text::truncate;
use crate::types::Document;

#[derive(Subcommand)]
pub enum DocumentCommands {
    /// List all documents
    #[command(alias = "ls")]
    List {
        /// Filter by project ID
        #[arg(short, long)]
        project: Option<String>,
        /// Include archived documents
        #[arg(short, long)]
        archived: bool,
    },
    /// Get document details and content
    Get {
        /// Document ID(s) or slug(s). Use "-" to read from stdin.
        ids: Vec<String>,
    },
    /// Create a new document
    Create {
        /// Document title
        title: String,
        /// Project name or ID to associate the document with
        #[arg(short, long)]
        project: String,
        /// Document content (Markdown)
        #[arg(short, long)]
        content: Option<String>,
        /// Document icon (e.g., ":page_facing_up:")
        #[arg(short, long)]
        icon: Option<String>,
        /// Icon color (hex color code)
        #[arg(long)]
        color: Option<String>,
    },
    /// Update an existing document
    Update {
        /// Document ID
        id: String,
        /// New title
        #[arg(short, long)]
        title: Option<String>,
        /// New content (Markdown)
        #[arg(short, long)]
        content: Option<String>,
        /// New icon
        #[arg(short, long)]
        icon: Option<String>,
        /// New color (hex)
        #[arg(long)]
        color: Option<String>,
        /// New project ID
        #[arg(short, long)]
        project: Option<String>,
        /// Preview without updating (dry run)
        #[arg(long)]
        dry_run: bool,
    },
    /// Delete a document
    Delete {
        /// Document ID
        id: String,
        /// Skip confirmation
        #[arg(long)]
        force: bool,
        /// Preview without deleting (dry run)
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Tabled)]
struct DocumentRow {
    #[tabled(rename = "Title")]
    title: String,
    #[tabled(rename = "Project")]
    project: String,
    #[tabled(rename = "Updated")]
    updated: String,
    #[tabled(rename = "ID")]
    id: String,
}

pub async fn handle(cmd: DocumentCommands, output: &OutputOptions) -> Result<()> {
    match cmd {
        DocumentCommands::List { project, archived } => {
            list_documents(project, archived, output).await
        }
        DocumentCommands::Get { ids } => {
            let final_ids = read_ids_from_stdin(ids);
            if final_ids.is_empty() {
                anyhow::bail!("No document IDs provided. Provide IDs or pipe them via stdin.");
            }
            get_documents(&final_ids, output).await
        }
        DocumentCommands::Create {
            title,
            project,
            content,
            icon,
            color,
        } => create_document(&title, &project, content, icon, color, output).await,
        DocumentCommands::Update {
            id,
            title,
            content,
            icon,
            color,
            project,
            dry_run,
        } => {
            let dry_run = dry_run || output.dry_run;
            update_document(&id, title, content, icon, color, project, dry_run, output).await
        }
        DocumentCommands::Delete { id, force, dry_run } => {
            let dry_run = dry_run || output.dry_run;
            delete_document(&id, force, dry_run, output).await
        }
    }
}

async fn list_documents(
    project_id: Option<String>,
    include_archived: bool,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($includeArchived: Boolean, $first: Int, $after: String, $last: Int, $before: String) {
            documents(first: $first, after: $after, last: $last, before: $before, includeArchived: $includeArchived) {
                nodes {
                    id
                    title
                    updatedAt
                    project { id name }
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

    let pagination = output.pagination.with_default_limit(100);
    let documents = paginate_nodes(
        &client,
        query,
        vars,
        &["data", "documents", "nodes"],
        &["data", "documents", "pageInfo"],
        &pagination,
        100,
    )
    .await?;

    // Filter by project if specified
    let mut filtered_docs: Vec<_> = if let Some(ref pid) = project_id {
        documents
            .iter()
            .filter(|d| {
                d["project"]["id"].as_str() == Some(pid.as_str())
                    || d["project"]["name"].as_str().map(|n| n.to_lowercase())
                        == Some(pid.to_lowercase())
            })
            .cloned()
            .collect()
    } else {
        documents
    };

    if output.is_json() || output.has_template() {
        print_json_owned(serde_json::json!(filtered_docs), output)?;
        return Ok(());
    }

    filter_values(&mut filtered_docs, &output.filters);

    if let Some(sort_key) = output.json.sort.as_deref() {
        sort_values(&mut filtered_docs, sort_key, output.json.order);
    }

    ensure_non_empty(&filtered_docs, output)?;
    if filtered_docs.is_empty() {
        println!("No documents found.");
        return Ok(());
    }

    let width = display_options().max_width(40);
    let rows: Vec<DocumentRow> = filtered_docs
        .iter()
        .map(|d| {
            let updated = d["updatedAt"]
                .as_str()
                .unwrap_or("")
                .chars()
                .take(10)
                .collect::<String>();

            DocumentRow {
                title: truncate(d["title"].as_str().unwrap_or(""), width),
                project: truncate(d["project"]["name"].as_str().unwrap_or("-"), width),
                updated,
                id: d["id"].as_str().unwrap_or("").to_string(),
            }
        })
        .collect();

    let table = Table::new(rows).to_string();
    println!("{}", table);
    println!("\n{} documents", filtered_docs.len());

    Ok(())
}

async fn get_document(id: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($id: String!) {
            document(id: $id) {
                id
                title
                content
                icon
                color
                url
                createdAt
                updatedAt
                creator { name email }
                project { id name }
            }
        }
    "#;

    let result = client.query(query, Some(json!({ "id": id }))).await?;
    let document = &result["data"]["document"];

    if document.is_null() {
        anyhow::bail!("Document not found: {}", id);
    }

    if output.is_json() || output.has_template() {
        print_json(document, output)?;
        return Ok(());
    }

    let doc: Document = serde_json::from_value(document.clone())?;

    println!("{}", doc.title.bold());
    println!("{}", "-".repeat(40));

    if let Some(proj) = &doc.project {
        println!("Project: {}", proj.name);
    }

    if let Some(creator) = &doc.creator {
        println!("Creator: {}", creator.name);
    }

    if let Some(icon) = &doc.icon {
        println!("Icon: {}", icon);
    }

    if let Some(color) = &doc.color {
        println!("Color: {}", color);
    }

    println!("URL: {}", doc.url.as_deref().unwrap_or("-"));
    println!("ID: {}", doc.id);

    if let Some(created) = &doc.created_at {
        println!("Created: {}", created.chars().take(10).collect::<String>());
    }

    if let Some(updated) = &doc.updated_at {
        println!("Updated: {}", updated.chars().take(10).collect::<String>());
    }

    // Display content
    if let Some(content) = &doc.content {
        println!("\n{}", "Content".bold());
        println!("{}", "-".repeat(40));
        println!("{}", content);
    }

    Ok(())
}

async fn get_documents(ids: &[String], output: &OutputOptions) -> Result<()> {
    if ids.len() == 1 {
        return get_document(&ids[0], output).await;
    }

    if output.is_json() || output.has_template() {
        let client = LinearClient::new()?;
        let mut docs: Vec<serde_json::Value> = Vec::new();
        for id in ids {
            let query = r#"
                query($id: String!) {
                    document(id: $id) {
                        id
                        title
                        content
                        icon
                        color
                        url
                        createdAt
                        updatedAt
                        creator { name email }
                        project { id name }
                    }
                }
            "#;
            let result = client.query(query, Some(json!({ "id": id }))).await?;
            let document = &result["data"]["document"];
            if !document.is_null() {
                docs.push(document.clone());
            }
        }
        print_json_owned(serde_json::json!(docs), output)?;
        return Ok(());
    }

    for (idx, id) in ids.iter().enumerate() {
        if idx > 0 {
            println!();
        }
        get_document(id, output).await?;
    }

    Ok(())
}

async fn create_document(
    title: &str,
    project: &str,
    content: Option<String>,
    icon: Option<String>,
    color: Option<String>,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;

    let mut input = json!({
        "title": title,
        "projectId": project
    });

    if let Some(c) = content {
        input["content"] = json!(c);
    }
    if let Some(i) = icon {
        input["icon"] = json!(i);
    }
    if let Some(col) = color {
        input["color"] = json!(col);
    }

    let mutation = r#"
        mutation($input: DocumentCreateInput!) {
            documentCreate(input: $input) {
                success
                document { id title url }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "input": input })))
        .await?;

    if result["data"]["documentCreate"]["success"].as_bool() == Some(true) {
        let document = &result["data"]["documentCreate"]["document"];
        if output.is_json() || output.has_template() {
            print_json(document, output)?;
            return Ok(());
        }
        println!(
            "{} Created document: {}",
            "+".green(),
            document["title"].as_str().unwrap_or("")
        );
        println!("  ID: {}", document["id"].as_str().unwrap_or(""));
        println!("  URL: {}", document["url"].as_str().unwrap_or(""));
    } else {
        anyhow::bail!("Failed to create document");
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn update_document(
    id: &str,
    title: Option<String>,
    content: Option<String>,
    icon: Option<String>,
    color: Option<String>,
    project: Option<String>,
    dry_run: bool,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;

    let mut input = json!({});
    if let Some(t) = title {
        input["title"] = json!(t);
    }
    if let Some(c) = content {
        input["content"] = json!(c);
    }
    if let Some(i) = icon {
        input["icon"] = json!(i);
    }
    if let Some(col) = color {
        input["color"] = json!(col);
    }
    if let Some(p) = project {
        input["projectId"] = json!(p);
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
            println!("{}", "[DRY RUN] Would update document:".yellow().bold());
            println!("  ID: {}", id);
        }
        return Ok(());
    }

    let mutation = r#"
        mutation($id: String!, $input: DocumentUpdateInput!) {
            documentUpdate(id: $id, input: $input) {
                success
                document { id title }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "id": id, "input": input })))
        .await?;

    if result["data"]["documentUpdate"]["success"].as_bool() == Some(true) {
        if output.is_json() || output.has_template() {
            print_json(&result["data"]["documentUpdate"]["document"], output)?;
            return Ok(());
        }
        println!("{} Document updated", "+".green());
    } else {
        anyhow::bail!("Failed to update document");
    }

    Ok(())
}

async fn delete_document(
    id: &str,
    force: bool,
    dry_run: bool,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;

    if dry_run {
        if output.is_json() || output.has_template() {
            print_json_owned(
                json!({
                    "dry_run": true,
                    "would_delete": { "id": id }
                }),
                output,
            )?;
        } else {
            println!("{}", "[DRY RUN] Would delete document:".yellow().bold());
            println!("  ID: {}", id);
        }
        return Ok(());
    }

    if !force {
        use dialoguer::Confirm;
        let confirm = Confirm::new()
            .with_prompt(format!("Delete document {}?", id))
            .default(false)
            .interact()?;
        if !confirm {
            println!("Cancelled.");
            return Ok(());
        }
    }

    let mutation = r#"
        mutation($id: String!) {
            documentDelete(id: $id) {
                success
            }
        }
    "#;

    let result = client.mutate(mutation, Some(json!({ "id": id }))).await?;

    if result["data"]["documentDelete"]["success"].as_bool() == Some(true) {
        if output.is_json() || output.has_template() {
            print_json_owned(json!({ "deleted": true, "id": id }), output)?;
            return Ok(());
        }
        println!("{} Document deleted: {}", "-".red(), id);
    } else {
        anyhow::bail!("Failed to delete document");
    }

    Ok(())
}
