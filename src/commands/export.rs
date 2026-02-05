use anyhow::Result;
use clap::Subcommand;
use csv::Writer;
use serde_json::json;
use std::io::Write;

use crate::api::LinearClient;
use crate::output::OutputOptions;
use crate::pagination::{paginate_nodes, stream_nodes, PaginationOptions};

#[derive(Subcommand, Debug)]
pub enum ExportCommands {
    /// Export issues to CSV
    Csv {
        /// Team key to export
        #[arg(short, long)]
        team: Option<String>,
        /// Output file (default: stdout)
        #[arg(short, long)]
        file: Option<String>,
        /// Include completed issues
        #[arg(long)]
        include_completed: bool,
        /// Limit number of issues (default: 250, ignored with --all)
        #[arg(long)]
        limit: Option<usize>,
        /// Export all matching issues
        #[arg(long)]
        all: bool,
    },
    /// Export issues to Markdown
    Markdown {
        /// Team key to export
        #[arg(short, long)]
        team: Option<String>,
        /// Output file (default: stdout)
        #[arg(short, long)]
        file: Option<String>,
        /// Limit number of issues (default: 250, ignored with --all)
        #[arg(long)]
        limit: Option<usize>,
        /// Export all matching issues
        #[arg(long)]
        all: bool,
    },
}

pub async fn handle(cmd: ExportCommands, _output: &OutputOptions) -> Result<()> {
    match cmd {
        ExportCommands::Csv {
            team,
            file,
            include_completed,
            limit,
            all,
        } => export_csv(team, file, include_completed, limit, all).await,
        ExportCommands::Markdown { team, file, limit, all } => {
            export_markdown(team, file, limit, all).await
        }
    }
}

async fn export_csv(
    team: Option<String>,
    file: Option<String>,
    include_completed: bool,
    limit: Option<usize>,
    all: bool,
) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($filter: IssueFilter, $first: Int, $after: String, $last: Int, $before: String) {
            issues(first: $first, after: $after, last: $last, before: $before, filter: $filter) {
                nodes {
                    identifier
                    title
                    description
                    priority
                    estimate
                    dueDate
                    createdAt
                    updatedAt
                    state { name type }
                    assignee { name email }
                    team { key name }
                    labels { nodes { name } }
                    project { name }
                    cycle { number name }
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

    let mut filter = json!({});
    if let Some(ref t) = team {
        filter["team"] = json!({ "key": { "eq": t } });
    }
    if !include_completed {
        filter["state"] = json!({ "type": { "neq": "completed" } });
    }

    let mut vars = serde_json::Map::new();
    vars.insert("filter".to_string(), filter);

    let mut pagination = PaginationOptions::default();
    pagination.page_size = Some(250);
    if all {
        pagination.all = true;
    } else {
        pagination.limit = Some(limit.unwrap_or(250));
    }

    // Use RefCell to allow mutable access to the writer from the closure
    use std::cell::RefCell;
    use std::rc::Rc;

    let wtr: Rc<RefCell<Writer<Box<dyn Write>>>> = if let Some(ref path) = file {
        Rc::new(RefCell::new(Writer::from_writer(Box::new(
            std::fs::File::create(path)?,
        ))))
    } else {
        Rc::new(RefCell::new(Writer::from_writer(Box::new(
            std::io::stdout(),
        ))))
    };

    // Write CSV header
    wtr.borrow_mut().write_record([
        "Identifier",
        "Title",
        "Status",
        "Priority",
        "Estimate",
        "Due Date",
        "Assignee",
        "Team",
        "Project",
        "Cycle",
        "Labels",
        "Created",
        "Updated",
    ])?;

    // Stream pages and write rows as they arrive
    let wtr_clone = Rc::clone(&wtr);
    let total = stream_nodes(
        &client,
        query,
        vars,
        &["data", "issues", "nodes"],
        &["data", "issues", "pageInfo"],
        &pagination,
        250,
        |batch| {
            let wtr = Rc::clone(&wtr_clone);
            async move {
                let mut writer = wtr.borrow_mut();
                for issue in &batch {
                    let labels: Vec<&str> = issue["labels"]["nodes"]
                        .as_array()
                        .map(|a| a.iter().filter_map(|l| l["name"].as_str()).collect())
                        .unwrap_or_default();

                    writer.write_record([
                        issue["identifier"].as_str().unwrap_or(""),
                        issue["title"].as_str().unwrap_or(""),
                        issue["state"]["name"].as_str().unwrap_or(""),
                        &issue["priority"].as_i64().unwrap_or(0).to_string(),
                        &issue["estimate"].as_f64().unwrap_or(0.0).to_string(),
                        issue["dueDate"].as_str().unwrap_or(""),
                        issue["assignee"]["name"].as_str().unwrap_or(""),
                        issue["team"]["key"].as_str().unwrap_or(""),
                        issue["project"]["name"].as_str().unwrap_or(""),
                        issue["cycle"]["name"].as_str().unwrap_or(""),
                        &labels.join("; "),
                        &issue["createdAt"]
                            .as_str()
                            .unwrap_or("")
                            .chars()
                            .take(10)
                            .collect::<String>(),
                        &issue["updatedAt"]
                            .as_str()
                            .unwrap_or("")
                            .chars()
                            .take(10)
                            .collect::<String>(),
                    ])?;
                }
                Ok(())
            }
        },
    )
    .await?;

    wtr.borrow_mut().flush()?;

    if file.is_some() {
        eprintln!("Exported {} issues to {}", total, file.unwrap());
    }

    Ok(())
}

async fn export_markdown(
    team: Option<String>,
    file: Option<String>,
    limit: Option<usize>,
    all: bool,
) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($filter: IssueFilter, $first: Int, $after: String, $last: Int, $before: String) {
            issues(first: $first, after: $after, last: $last, before: $before, filter: $filter) {
                nodes {
                    identifier
                    title
                    description
                    priority
                    state { name }
                    assignee { name }
                    team { key }
                    labels { nodes { name } }
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

    let mut filter = json!({ "state": { "type": { "neq": "completed" } } });
    if let Some(ref t) = team {
        filter["team"] = json!({ "key": { "eq": t } });
    }

    let mut vars = serde_json::Map::new();
    vars.insert("filter".to_string(), filter);

    let mut pagination = PaginationOptions::default();
    pagination.page_size = Some(250);
    if all {
        pagination.all = true;
    } else {
        pagination.limit = Some(limit.unwrap_or(250));
    }

    let issues = paginate_nodes(
        &client,
        query,
        vars,
        &["data", "issues", "nodes"],
        &["data", "issues", "pageInfo"],
        &pagination,
        250,
    )
    .await?;

    let mut output: Box<dyn Write> = if let Some(ref path) = file {
        Box::new(std::fs::File::create(path)?)
    } else {
        Box::new(std::io::stdout())
    };

    writeln!(output, "# Issues Export\n")?;
    writeln!(output, "Generated: {}\n", chrono::Utc::now().format("%Y-%m-%d %H:%M UTC"))?;

    // Group by status
    let mut by_status: std::collections::HashMap<String, Vec<&serde_json::Value>> =
        std::collections::HashMap::new();
    for issue in &issues {
        let status = issue["state"]["name"]
            .as_str()
            .unwrap_or("Unknown")
            .to_string();
        by_status.entry(status).or_default().push(issue);
    }

    for (status, status_issues) in by_status {
        writeln!(output, "## {}\n", status)?;
        for issue in status_issues {
            let labels: Vec<&str> = issue["labels"]["nodes"]
                .as_array()
                .map(|a| a.iter().filter_map(|l| l["name"].as_str()).collect())
                .unwrap_or_default();
            let label_str = if labels.is_empty() {
                String::new()
            } else {
                format!(" `{}`", labels.join("` `"))
            };

            writeln!(
                output,
                "- **{}** {}{}",
                issue["identifier"].as_str().unwrap_or(""),
                issue["title"].as_str().unwrap_or(""),
                label_str
            )?;

            if let Some(assignee) = issue["assignee"]["name"].as_str() {
                writeln!(output, "  - Assignee: {}", assignee)?;
            }
        }
        writeln!(output)?;
    }

    if file.is_some() {
        eprintln!("Exported {} issues to {}", issues.len(), file.unwrap());
    }

    Ok(())
}
