use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::json;
use tabled::{Table, Tabled};

use crate::api::LinearClient;
use crate::display_options;
use crate::input::read_ids_from_stdin;
use crate::output::{ensure_non_empty, filter_values, print_json, sort_values, OutputOptions};
use crate::pagination::paginate_nodes;
use crate::text::truncate;

#[derive(Subcommand)]
pub enum CommentCommands {
    /// List comments for an issue
    #[command(alias = "ls")]
    List {
        /// Issue ID(s). Use "-" to read from stdin.
        issue_ids: Vec<String>,
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
        CommentCommands::List { issue_ids } => list_comments(&issue_ids, output).await,
        CommentCommands::Create {
            issue_id,
            body,
            parent_id,
        } => create_comment(&issue_id, &body, parent_id).await,
    }
}

async fn list_comments(issue_ids: &[String], output: &OutputOptions) -> Result<()> {
    let final_ids = read_ids_from_stdin(issue_ids.to_vec());

    if final_ids.is_empty() {
        anyhow::bail!("No issue IDs provided. Provide IDs or pipe them via stdin.");
    }

    let client = LinearClient::new()?;
    let pagination = output.pagination.with_default_limit(100);
    let mut issues = Vec::new();
    for id in &final_ids {
        let mut issue = fetch_issue_meta(&client, id).await?;
        if issue.is_null() {
            if !output.is_json() && !output.has_template() {
                eprintln!("{} Issue not found: {}", "!".yellow(), id);
            }
            continue;
        }
        let comments = fetch_issue_comments(&client, id, &pagination).await?;
        issue["comments"] = json!({ "nodes": comments });
        issues.push(issue);
    }

    // JSON output - return raw data for LLM consumption
    if output.is_json() || output.has_template() {
        if issues.len() == 1 {
            print_json(&issues[0], output)?;
        } else {
            print_json(&serde_json::json!(issues), output)?;
        }
        return Ok(());
    }

    for (idx, issue) in issues.iter().enumerate() {
        if idx > 0 {
            println!();
        }
        let identifier = issue["identifier"].as_str().unwrap_or("");
        let title = issue["title"].as_str().unwrap_or("");

        println!("{} {}", identifier.bold(), title);
        println!("{}", "-".repeat(50));

        let mut comments = issue["comments"]["nodes"]
            .as_array()
            .cloned()
            .unwrap_or_default();

        filter_values(&mut comments, &output.filters);

        if let Some(sort_key) = output.json.sort.as_deref() {
            sort_values(&mut comments, sort_key, output.json.order);
        }

        ensure_non_empty(&comments, output)?;
        if comments.is_empty() {
            println!("No comments found for this issue.");
            continue;
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
    }

    Ok(())
}

async fn fetch_issue_meta(client: &LinearClient, issue_id: &str) -> Result<serde_json::Value> {
    let query = r#"
        query($issueId: String!) {
            issue(id: $issueId) {
                id
                identifier
                title
            }
        }
    "#;

    let result = client
        .query(query, Some(json!({ "issueId": issue_id })))
        .await?;
    Ok(result["data"]["issue"].clone())
}

async fn fetch_issue_comments(
    client: &LinearClient,
    issue_id: &str,
    pagination: &crate::pagination::PaginationOptions,
) -> Result<Vec<serde_json::Value>> {
    let query = r#"
        query($issueId: String!, $first: Int, $after: String, $last: Int, $before: String) {
            issue(id: $issueId) {
                comments(first: $first, after: $after, last: $last, before: $before) {
                    nodes {
                        id
                        body
                        createdAt
                        user { name email }
                        parent { id }
                    }
                    pageInfo {
                        hasNextPage
                        endCursor
                        hasPreviousPage
                        startCursor
                    }
                }
            }
        }
    "#;

    let mut vars = serde_json::Map::new();
    vars.insert("issueId".to_string(), json!(issue_id));

    paginate_nodes(
        client,
        query,
        vars,
        &["data", "issue", "comments", "nodes"],
        &["data", "issue", "comments", "pageInfo"],
        pagination,
        100,
    )
    .await
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
