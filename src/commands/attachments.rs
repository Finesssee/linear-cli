use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::json;
use tabled::{Table, Tabled};

use crate::api::LinearClient;
use crate::display_options;
use crate::output::{
    ensure_non_empty, filter_values, print_json, print_json_owned, sort_values, OutputOptions,
};
use crate::text::truncate;

#[derive(Subcommand)]
pub enum AttachmentCommands {
    /// List attachments for an issue
    #[command(alias = "ls")]
    List {
        /// Issue ID or identifier (e.g., SCW-123)
        issue: String,
    },
    /// Get attachment details
    Get {
        /// Attachment ID
        id: String,
    },
    /// Create an attachment on an issue
    Create {
        /// Issue ID or identifier
        issue: String,
        /// Attachment title
        #[arg(short = 'T', long)]
        title: String,
        /// Attachment URL
        #[arg(short, long)]
        url: String,
        /// Subtitle/description
        #[arg(short, long)]
        subtitle: Option<String>,
        /// Icon URL
        #[arg(long)]
        icon_url: Option<String>,
    },
    /// Update an attachment
    Update {
        /// Attachment ID
        id: String,
        /// New title
        #[arg(short = 'T', long)]
        title: Option<String>,
        /// New URL
        #[arg(short, long)]
        url: Option<String>,
        /// New subtitle
        #[arg(short, long)]
        subtitle: Option<String>,
    },
    /// Delete an attachment
    #[command(alias = "rm")]
    Delete {
        /// Attachment ID
        id: String,
        /// Skip confirmation
        #[arg(short, long)]
        force: bool,
    },
    /// Link a URL to an issue
    #[command(alias = "link")]
    LinkUrl {
        /// Issue ID or identifier
        issue: String,
        /// URL to link
        url: String,
        /// Link title
        #[arg(short = 'T', long)]
        title: Option<String>,
    },
}

#[derive(Tabled)]
struct AttachmentRow {
    #[tabled(rename = "Title")]
    title: String,
    #[tabled(rename = "URL")]
    url: String,
    #[tabled(rename = "Source")]
    source: String,
    #[tabled(rename = "ID")]
    id: String,
}

pub async fn handle(cmd: AttachmentCommands, output: &OutputOptions) -> Result<()> {
    match cmd {
        AttachmentCommands::List { issue } => list_attachments(&issue, output).await,
        AttachmentCommands::Get { id } => get_attachment(&id, output).await,
        AttachmentCommands::Create {
            issue,
            title,
            url,
            subtitle,
            icon_url,
        } => create_attachment(&issue, &title, &url, subtitle, icon_url, output).await,
        AttachmentCommands::Update {
            id,
            title,
            url,
            subtitle,
        } => update_attachment(&id, title, url, subtitle, output).await,
        AttachmentCommands::Delete { id, force } => delete_attachment(&id, force).await,
        AttachmentCommands::LinkUrl { issue, url, title } => {
            link_url(&issue, &url, title, output).await
        }
    }
}

async fn resolve_issue_uuid(client: &LinearClient, issue: &str) -> Result<String> {
    let query = r#"
        query($id: String!) {
            issue(id: $id) {
                id
            }
        }
    "#;

    let result = client.query(query, Some(json!({ "id": issue }))).await?;
    let id = result["data"]["issue"]["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Issue not found: {}", issue))?;
    Ok(id.to_string())
}

async fn list_attachments(issue: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($id: String!) {
            issue(id: $id) {
                identifier
                title
                attachments(first: 50) {
                    nodes {
                        id
                        title
                        subtitle
                        url
                        sourceType
                        createdAt
                    }
                }
            }
        }
    "#;

    let result = client.query(query, Some(json!({ "id": issue }))).await?;
    let issue_data = &result["data"]["issue"];

    if issue_data.is_null() {
        anyhow::bail!("Issue not found: {}", issue);
    }

    let mut attachments = issue_data["attachments"]["nodes"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    if output.is_json() || output.has_template() {
        print_json_owned(
            json!({
                "issue": issue_data["identifier"],
                "title": issue_data["title"],
                "attachments": attachments
            }),
            output,
        )?;
        return Ok(());
    }

    let identifier = issue_data["identifier"].as_str().unwrap_or("");
    let title = issue_data["title"].as_str().unwrap_or("");

    println!("{} {}", identifier.bold(), title);
    println!("{}", "-".repeat(50));

    filter_values(&mut attachments, &output.filters);

    if let Some(sort_key) = output.json.sort.as_deref() {
        sort_values(&mut attachments, sort_key, output.json.order);
    }

    ensure_non_empty(&attachments, output)?;
    if attachments.is_empty() {
        println!("No attachments found for this issue.");
        return Ok(());
    }

    let width = display_options().max_width(40);
    let rows: Vec<AttachmentRow> = attachments
        .iter()
        .map(|v| AttachmentRow {
            title: truncate(v["title"].as_str().unwrap_or("-"), width),
            url: truncate(v["url"].as_str().unwrap_or("-"), width),
            source: v["sourceType"].as_str().unwrap_or("-").to_string(),
            id: v["id"].as_str().unwrap_or("").to_string(),
        })
        .collect();

    let rows_len = rows.len();
    let table = Table::new(rows).to_string();
    println!("{}", table);
    println!("\n{} attachments", rows_len);

    Ok(())
}

async fn get_attachment(id: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($id: String!) {
            attachment(id: $id) {
                id
                title
                subtitle
                url
                sourceType
                metadata
                createdAt
                updatedAt
                issue { identifier }
            }
        }
    "#;

    let result = client.query(query, Some(json!({ "id": id }))).await?;
    let raw = &result["data"]["attachment"];

    if raw.is_null() {
        anyhow::bail!("Attachment not found: {}", id);
    }

    if output.is_json() || output.has_template() {
        print_json(raw, output)?;
        return Ok(());
    }

    let title = raw["title"].as_str().unwrap_or("-");
    println!("{}", title.bold());
    println!("{}", "-".repeat(40));

    if let Some(issue_id) = raw["issue"]["identifier"].as_str() {
        println!("Issue: {}", issue_id);
    }
    if let Some(subtitle) = raw["subtitle"].as_str() {
        if !subtitle.is_empty() {
            println!("Subtitle: {}", subtitle);
        }
    }
    if let Some(url) = raw["url"].as_str() {
        println!("URL: {}", url);
    }
    if let Some(source) = raw["sourceType"].as_str() {
        println!("Source: {}", source);
    }
    println!(
        "Created: {}",
        raw["createdAt"]
            .as_str()
            .map(|s| s.get(..10).unwrap_or(s))
            .unwrap_or("-")
    );
    println!(
        "Updated: {}",
        raw["updatedAt"]
            .as_str()
            .map(|s| s.get(..10).unwrap_or(s))
            .unwrap_or("-")
    );
    println!("ID: {}", id);

    Ok(())
}

async fn create_attachment(
    issue: &str,
    title: &str,
    url: &str,
    subtitle: Option<String>,
    icon_url: Option<String>,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;
    let issue_id = resolve_issue_uuid(&client, issue).await?;

    let mut input = json!({
        "issueId": issue_id,
        "title": title,
        "url": url
    });
    if let Some(s) = &subtitle {
        input["subtitle"] = json!(s);
    }
    if let Some(icon) = &icon_url {
        input["iconUrl"] = json!(icon);
    }

    let mutation = r#"
        mutation($input: AttachmentCreateInput!) {
            attachmentCreate(input: $input) {
                success
                attachment { id title url }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "input": input })))
        .await?;

    if result["data"]["attachmentCreate"]["success"].as_bool() == Some(true) {
        let attachment = &result["data"]["attachmentCreate"]["attachment"];
        if output.is_json() || output.has_template() {
            print_json(attachment, output)?;
            return Ok(());
        }
        println!(
            "{} Attachment created: {}",
            "+".green(),
            attachment["title"].as_str().unwrap_or("")
        );
        println!("  ID: {}", attachment["id"].as_str().unwrap_or(""));
        println!("  URL: {}", attachment["url"].as_str().unwrap_or(""));
    } else {
        anyhow::bail!("Failed to create attachment");
    }

    Ok(())
}

async fn update_attachment(
    id: &str,
    title: Option<String>,
    url: Option<String>,
    subtitle: Option<String>,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;

    let mut input = json!({});
    if let Some(t) = title {
        input["title"] = json!(t);
    }
    if let Some(u) = url {
        input["url"] = json!(u);
    }
    if let Some(s) = subtitle {
        input["subtitle"] = json!(s);
    }

    if input.as_object().map(|o| o.is_empty()).unwrap_or(true) {
        println!("No updates specified.");
        return Ok(());
    }

    let mutation = r#"
        mutation($id: String!, $input: AttachmentUpdateInput!) {
            attachmentUpdate(id: $id, input: $input) {
                success
                attachment { id title url }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "id": id, "input": input })))
        .await?;

    if result["data"]["attachmentUpdate"]["success"].as_bool() == Some(true) {
        let attachment = &result["data"]["attachmentUpdate"]["attachment"];
        if output.is_json() || output.has_template() {
            print_json(attachment, output)?;
            return Ok(());
        }
        println!("{} Attachment updated", "+".green());
        println!("  ID: {}", attachment["id"].as_str().unwrap_or(""));
    } else {
        anyhow::bail!("Failed to update attachment");
    }

    Ok(())
}

async fn delete_attachment(id: &str, force: bool) -> Result<()> {
    if !force && !crate::is_yes() {
        anyhow::bail!(
            "Delete requires --force flag. Use: linear attachments delete {} --force",
            id
        );
    }

    let client = LinearClient::new()?;

    let mutation = r#"
        mutation($id: String!) {
            attachmentDelete(id: $id) {
                success
            }
        }
    "#;

    let result = client.mutate(mutation, Some(json!({ "id": id }))).await?;

    if result["data"]["attachmentDelete"]["success"]
        .as_bool()
        .unwrap_or(false)
    {
        println!("{} Attachment deleted", "+".green());
    } else {
        anyhow::bail!("Failed to delete attachment {}", id);
    }

    Ok(())
}

async fn link_url(
    issue: &str,
    url: &str,
    title: Option<String>,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;
    let issue_id = resolve_issue_uuid(&client, issue).await?;

    let mut vars = json!({
        "issueId": issue_id,
        "url": url
    });
    if let Some(t) = &title {
        vars["title"] = json!(t);
    }

    let mutation = r#"
        mutation($issueId: String!, $url: String!, $title: String) {
            attachmentLinkURL(issueId: $issueId, url: $url, title: $title) {
                success
                attachment { id title url }
            }
        }
    "#;

    let result = client.mutate(mutation, Some(vars)).await?;

    if result["data"]["attachmentLinkURL"]["success"].as_bool() == Some(true) {
        let attachment = &result["data"]["attachmentLinkURL"]["attachment"];
        if output.is_json() || output.has_template() {
            print_json(attachment, output)?;
            return Ok(());
        }
        println!("{} URL linked to issue", "+".green());
        println!("  ID: {}", attachment["id"].as_str().unwrap_or(""));
        println!("  Title: {}", attachment["title"].as_str().unwrap_or("-"));
        println!("  URL: {}", attachment["url"].as_str().unwrap_or(""));
    } else {
        anyhow::bail!("Failed to link URL to issue");
    }

    Ok(())
}
