use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use tabled::{Table, Tabled};

use crate::api::LinearClient;

#[derive(Subcommand)]
pub enum SearchCommands {
    /// Search issues by query string
    Issues {
        /// Search query string
        query: String,
        /// Maximum number of results
        #[arg(short, long, default_value = "50")]
        limit: u32,
        /// Include archived issues
        #[arg(short, long)]
        archived: bool,
    },
    /// Search projects by query string
    Projects {
        /// Search query string
        query: String,
        /// Maximum number of results
        #[arg(short, long, default_value = "50")]
        limit: u32,
        /// Include archived projects
        #[arg(short, long)]
        archived: bool,
    },
}

#[derive(Tabled)]
struct IssueRow {
    #[tabled(rename = "Identifier")]
    identifier: String,
    #[tabled(rename = "Title")]
    title: String,
    #[tabled(rename = "State")]
    state: String,
    #[tabled(rename = "Priority")]
    priority: String,
    #[tabled(rename = "ID")]
    id: String,
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

pub async fn handle(cmd: SearchCommands) -> Result<()> {
    match cmd {
        SearchCommands::Issues { query, limit, archived } => {
            search_issues(&query, limit, archived).await
        }
        SearchCommands::Projects { query, limit, archived } => {
            search_projects(&query, limit, archived).await
        }
    }
}

async fn search_issues(query: &str, limit: u32, include_archived: bool) -> Result<()> {
    let client = LinearClient::new()?;

    let graphql_query = r#"
        query($first: Int!, $includeArchived: Boolean, $filter: IssueFilter) {
            issues(first: $first, includeArchived: $includeArchived, filter: $filter) {
                nodes {
                    id
                    identifier
                    title
                    priority
                    state { name }
                }
            }
        }
    "#;

    let variables = json!({
        "first": limit,
        "includeArchived": include_archived,
        "filter": {
            "or": [
                { "title": { "containsIgnoreCase": query } },
                { "description": { "containsIgnoreCase": query } }
            ]
        }
    });

    let result = client.query(graphql_query, Some(variables)).await?;

    let empty = vec![];
    let issues = result["data"]["issues"]["nodes"]
        .as_array()
        .unwrap_or(&empty);

    if issues.is_empty() {
        println!("No issues found matching: {}", query);
        return Ok(());
    }

    let rows: Vec<IssueRow> = issues
        .iter()
        .map(|issue| {
            let priority = match issue["priority"].as_i64() {
                Some(0) => "-".to_string(),
                Some(1) => "Urgent".to_string(),
                Some(2) => "High".to_string(),
                Some(3) => "Normal".to_string(),
                Some(4) => "Low".to_string(),
                _ => "-".to_string(),
            };

            IssueRow {
                identifier: issue["identifier"].as_str().unwrap_or("").to_string(),
                title: truncate_string(issue["title"].as_str().unwrap_or(""), 50),
                state: issue["state"]["name"].as_str().unwrap_or("-").to_string(),
                priority,
                id: issue["id"].as_str().unwrap_or("").to_string(),
            }
        })
        .collect();

    let table = Table::new(rows).to_string();
    println!("{}", table);
    println!("\n{} issues found", issues.len());

    Ok(())
}

async fn search_projects(query: &str, limit: u32, include_archived: bool) -> Result<()> {
    let client = LinearClient::new()?;

    let graphql_query = r#"
        query($first: Int!, $includeArchived: Boolean, $filter: ProjectFilter) {
            projects(first: $first, includeArchived: $includeArchived, filter: $filter) {
                nodes {
                    id
                    name
                    status { name }
                    labels { nodes { name } }
                }
            }
        }
    "#;

    let variables = json!({
        "first": limit,
        "includeArchived": include_archived,
        "filter": {
            "name": { "containsIgnoreCase": query }
        }
    });

    let result = client.query(graphql_query, Some(variables)).await?;

    let empty = vec![];
    let projects = result["data"]["projects"]["nodes"]
        .as_array()
        .unwrap_or(&empty);

    if projects.is_empty() {
        println!("No projects found matching: {}", query);
        return Ok(());
    }

    let rows: Vec<ProjectRow> = projects
        .iter()
        .map(|p| {
            let labels: Vec<String> = p["labels"]["nodes"]
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .map(|l| l["name"].as_str().unwrap_or("").to_string())
                .collect();

            ProjectRow {
                name: p["name"].as_str().unwrap_or("").to_string(),
                status: p["status"]["name"].as_str().unwrap_or("-").to_string(),
                labels: if labels.is_empty() { "-".to_string() } else { labels.join(", ") },
                id: p["id"].as_str().unwrap_or("").to_string(),
            }
        })
        .collect();

    let table = Table::new(rows).to_string();
    println!("{}", table);
    println!("\n{} projects found", projects.len());

    Ok(())
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
