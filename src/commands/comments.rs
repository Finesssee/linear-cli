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
pub enum CommentCommands {
    /// List comments for an issue
    #[command(alias = "ls")]
    List {
        /// Issue ID to list comments for
        issue_id: String,
    },
    /// Create a new comment on an issue
    Create {
        /// Issue ID to comment on
        issue_id: String,
        /// Comment body (Markdown supported)
        #[arg(short, long)]
        body: String,
        /// Parent comment ID to reply to (optional)
        #[arg(short, long)]
        parent_id: Option<String>,
    },
}

#[derive(Tabled)]
struct CommentRow {
    #[tabled(rename = "Author")]
    author: String,
    #[tabled(rename = "Created")]
    created_at: String,
    #[tabled(rename = "Body")]
    body: String,
    #[tabled(rename = "ID")]
    id: String,
}

pub async fn handle(cmd: CommentCommands, output: &OutputOptions) -> Result<()> {
    match cmd {
        CommentCommands::List { issue_id } => list_comments(&issue_id, output).await,
        CommentCommands::Create {
            issue_id,
            body,
            parent_id,
        } => create_comment(&issue_id, &body, parent_id).await,
    }
}

async fn list_comments(issue_id: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($issueId: String!) {
            issue(id: $issueId) {
                id
                identifier
                title
                comments {
                    nodes {
                        id
                        body
                        createdAt
                        user { name email }
                        parent { id }
                    }
                }
            }
        }
    "#;

    let result = client
        .query(query, Some(json!({ "issueId": issue_id })))
        .await?;
    let issue = &result["data"]["issue"];

    if issue.is_null() {
        anyhow::bail!("Issue not found: {}", issue_id);
    }

    // JSON output - return raw data for LLM consumption
    if output.is_json() {
        print_json(issue, &output.json)?;
        return Ok(());
    }

    let identifier = issue["identifier"].as_str().unwrap_or("");
    let title = issue["title"].as_str().unwrap_or("");

    println!("{} {}", identifier.bold(), title);
    println!("{}", "-".repeat(50));

    let empty = vec![];
    let comments = issue["comments"]["nodes"].as_array().unwrap_or(&empty);

    if comments.is_empty() {
        println!("No comments found for this issue.");
        return Ok(());
    }

    let width = display_options().max_width(60);
    let rows: Vec<CommentRow> = comments
        .iter()
        .map(|c| {
            let body = c["body"].as_str().unwrap_or("");
            let truncated_body = truncate(body, width);

            let created_at = c["createdAt"]
                .as_str()
                .unwrap_or("")
                .split('T')
                .next()
                .unwrap_or("-")
                .to_string();

            CommentRow {
                author: c["user"]["name"].as_str().unwrap_or("Unknown").to_string(),
                created_at,
                body: truncated_body.replace('\n', " "),
                id: c["id"].as_str().unwrap_or("").to_string(),
            }
        })
        .collect();

    let table = Table::new(rows).to_string();
    println!("{}", table);
    println!("\n{} comments", comments.len());

    Ok(())
}

async fn create_comment(issue_id: &str, body: &str, parent_id: Option<String>) -> Result<()> {
    let client = LinearClient::new()?;

    let mut input = json!({
        "issueId": issue_id,
        "body": body
    });

    if let Some(pid) = parent_id {
        input["parentId"] = json!(pid);
    }

    let mutation = r#"
        mutation($input: CommentCreateInput!) {
            commentCreate(input: $input) {
                success
                comment {
                    id
                    body
                    createdAt
                    user { name }
                    issue { identifier title }
                }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "input": input })))
        .await?;

    if result["data"]["commentCreate"]["success"].as_bool() == Some(true) {
        let comment = &result["data"]["commentCreate"]["comment"];
        let issue_identifier = comment["issue"]["identifier"].as_str().unwrap_or("");
        let issue_title = comment["issue"]["title"].as_str().unwrap_or("");

        println!(
            "{} Comment added to {} {}",
            "+".green(),
            issue_identifier,
            issue_title
        );
        println!("  ID: {}", comment["id"].as_str().unwrap_or(""));
        println!(
            "  Author: {}",
            comment["user"]["name"].as_str().unwrap_or("")
        );

        let body_preview = comment["body"]
            .as_str()
            .unwrap_or("")
            .chars()
            .take(80)
            .collect::<String>();
        if !body_preview.is_empty() {
            println!("  Body: {}", body_preview.dimmed());
        }
    } else {
        anyhow::bail!("Failed to create comment");
    }

    Ok(())
}
