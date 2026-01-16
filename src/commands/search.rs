use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use tabled::{Table, Tabled};

use crate::api::LinearClient;
use crate::display_options;
use crate::output::{print_json, OutputOptions};
use crate::text::truncate;

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

pub async fn handle(cmd: SearchCommands, output: &OutputOptions) -> Result<()> {
    match cmd {
        SearchCommands::Issues {
            query,
            limit,
            archived,
        } => search_issues(&query, limit, archived, output).await,
        SearchCommands::Projects {
            query,
            limit,
            archived,
        } => search_projects(&query, limit, archived, output).await,
    }
}

async fn search_issues(
    query: &str,
    limit: u32,
    include_archived: bool,
    output: &OutputOptions,
) -> Result<()> {
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

    if output.is_json() {
        print_json(&serde_json::json!(issues), &output.json)?;
        return Ok(());
    }

    if issues.is_empty() {
        println!("No issues found matching: {}", query);
        return Ok(());
    }

    let width = display_options().max_width(50);
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
                title: truncate(issue["title"].as_str().unwrap_or(""), width),
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

async fn search_projects(
    query: &str,
    limit: u32,
    include_archived: bool,
    output: &OutputOptions,
) -> Result<()> {
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

    if output.is_json() {
        print_json(&serde_json::json!(projects), &output.json)?;
        return Ok(());
    }

    if projects.is_empty() {
        println!("No projects found matching: {}", query);
        return Ok(());
    }

    let name_width = display_options().max_width(40);
    let label_width = display_options().max_width(40);
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
                name: truncate(p["name"].as_str().unwrap_or(""), name_width),
                status: p["status"]["name"].as_str().unwrap_or("-").to_string(),
                labels: if labels.is_empty() {
                    "-".to_string()
                } else {
                    truncate(&labels.join(", "), label_width)
                },
                id: p["id"].as_str().unwrap_or("").to_string(),
            }
        })
        .collect();

    let table = Table::new(rows).to_string();
    println!("{}", table);
    println!("\n{} projects found", projects.len());

    Ok(())
}
