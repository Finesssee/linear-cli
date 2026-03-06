use anyhow::Result;
use clap::{Subcommand, ValueHint};
use csv::Writer;
use serde_json::json;
use std::borrow::Cow;
use std::io::Write;

use crate::api::LinearClient;
use crate::output::OutputOptions;
use crate::pagination::{paginate_nodes, stream_nodes, PaginationOptions};
use colored::Colorize;

#[cfg(unix)]
fn create_private_file(path: &str) -> Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;

    Ok(std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?)
}

#[cfg(not(unix))]
fn create_private_file(path: &str) -> Result<std::fs::File> {
    Ok(std::fs::File::create(path)?)
}

#[cfg(unix)]
fn write_private_string(path: &str, contents: &str) -> Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(contents.as_bytes())?;
    file.flush()?;
    Ok(())
}

#[cfg(not(unix))]
fn write_private_string(path: &str, contents: &str) -> Result<()> {
    std::fs::write(path, contents)?;
    Ok(())
}

fn sanitize_csv_cell(value: &str) -> Cow<'_, str> {
    match value.chars().next() {
        Some('=' | '+' | '-' | '@' | '\t' | '\r') => Cow::Owned(format!("'{}", value)),
        _ => Cow::Borrowed(value),
    }
}

#[derive(Subcommand, Debug)]
pub enum ExportCommands {
    /// Export issues to CSV
    Csv {
        /// Team key to export
        #[arg(short, long)]
        team: Option<String>,
        /// Output file (default: stdout)
        #[arg(short, long, value_hint = ValueHint::FilePath)]
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
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        file: Option<String>,
        /// Limit number of issues (default: 250, ignored with --all)
        #[arg(long)]
        limit: Option<usize>,
        /// Export all matching issues
        #[arg(long)]
        all: bool,
    },
    /// Export issues to JSON file
    Json {
        /// Team key to export
        #[arg(short, long)]
        team: Option<String>,
        /// Output file (default: stdout)
        #[arg(short, long, value_hint = ValueHint::FilePath)]
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
        /// Pretty-print JSON output
        #[arg(long)]
        pretty: bool,
    },
    /// Export projects to CSV
    ProjectsCsv {
        /// Output file (default: stdout)
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        file: Option<String>,
        /// Include archived projects
        #[arg(long)]
        archived: bool,
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
        ExportCommands::Markdown {
            team,
            file,
            limit,
            all,
        } => export_markdown(team, file, limit, all).await,
        ExportCommands::Json {
            team,
            file,
            include_completed,
            limit,
            all,
            pretty,
        } => export_json(team, file, include_completed, limit, all, pretty).await,
        ExportCommands::ProjectsCsv { file, archived } => {
            export_projects_csv(file, archived).await
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

    let mut pagination = PaginationOptions {
        page_size: Some(250),
        ..Default::default()
    };
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
            create_private_file(path)?,
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
                        sanitize_csv_cell(issue["identifier"].as_str().unwrap_or("")).as_ref(),
                        sanitize_csv_cell(issue["title"].as_str().unwrap_or("")).as_ref(),
                        sanitize_csv_cell(issue["state"]["name"].as_str().unwrap_or("")).as_ref(),
                        &issue["priority"].as_i64().unwrap_or(0).to_string(),
                        &issue["estimate"].as_f64().unwrap_or(0.0).to_string(),
                        sanitize_csv_cell(issue["dueDate"].as_str().unwrap_or("")).as_ref(),
                        sanitize_csv_cell(issue["assignee"]["name"].as_str().unwrap_or("")).as_ref(),
                        sanitize_csv_cell(issue["team"]["key"].as_str().unwrap_or("")).as_ref(),
                        sanitize_csv_cell(issue["project"]["name"].as_str().unwrap_or("")).as_ref(),
                        sanitize_csv_cell(issue["cycle"]["name"].as_str().unwrap_or("")).as_ref(),
                        sanitize_csv_cell(&labels.join("; ")).as_ref(),
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

    if let Some(ref path) = file {
        eprintln!("Exported {} issues to {}", total, path);
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

    let mut pagination = PaginationOptions {
        page_size: Some(250),
        ..Default::default()
    };
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
        Box::new(create_private_file(path)?)
    } else {
        Box::new(std::io::stdout())
    };

    writeln!(output, "# Issues Export\n")?;
    writeln!(
        output,
        "Generated: {}\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")
    )?;

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

    if let Some(ref path) = file {
        eprintln!("Exported {} issues to {}", issues.len(), path);
    }

    Ok(())
}

async fn export_json(
    team: Option<String>,
    file: Option<String>,
    include_completed: bool,
    limit: Option<usize>,
    all: bool,
    pretty: bool,
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

    let mut pagination = PaginationOptions {
        page_size: Some(250),
        ..Default::default()
    };
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

    // Flatten issue objects for easier re-import
    let flattened: Vec<serde_json::Value> = issues
        .iter()
        .map(|issue| {
            let labels: Vec<&str> = issue["labels"]["nodes"]
                .as_array()
                .map(|a| a.iter().filter_map(|l| l["name"].as_str()).collect())
                .unwrap_or_default();

            json!({
                "identifier": issue["identifier"],
                "title": issue["title"],
                "description": issue["description"],
                "priority": issue["priority"],
                "estimate": issue["estimate"],
                "dueDate": issue["dueDate"],
                "status": issue["state"]["name"],
                "statusType": issue["state"]["type"],
                "assignee": issue["assignee"]["name"],
                "assigneeEmail": issue["assignee"]["email"],
                "team": issue["team"]["key"],
                "teamName": issue["team"]["name"],
                "project": issue["project"]["name"],
                "cycleNumber": issue["cycle"]["number"],
                "cycleName": issue["cycle"]["name"],
                "labels": labels,
                "createdAt": issue["createdAt"],
                "updatedAt": issue["updatedAt"],
            })
        })
        .collect();

    let json_output = if pretty {
        serde_json::to_string_pretty(&flattened)?
    } else {
        serde_json::to_string(&flattened)?
    };

    if let Some(ref path) = file {
        write_private_string(path, &json_output)?;
        eprintln!("Exported {} issues to {}", flattened.len(), path);
    } else {
        println!("{}", json_output);
    }

    Ok(())
}

async fn export_projects_csv(file: Option<String>, include_archived: bool) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($includeArchived: Boolean, $first: Int, $after: String, $last: Int, $before: String) {
            projects(first: $first, after: $after, last: $last, before: $before, includeArchived: $includeArchived) {
                nodes {
                    id
                    name
                    description
                    state
                    priority
                    progress
                    startDate
                    targetDate
                    url
                    createdAt
                    updatedAt
                    lead { name email }
                    teams { nodes { key name } }
                    members { nodes { name } }
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

    let pagination = PaginationOptions {
        page_size: Some(50),
        all: true,
        ..Default::default()
    };

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

    let mut wtr: Writer<Box<dyn Write>> = if let Some(ref path) = file {
        Writer::from_writer(Box::new(create_private_file(path)?))
    } else {
        Writer::from_writer(Box::new(std::io::stdout()))
    };

    wtr.write_record([
        "Name",
        "State",
        "Priority",
        "Progress",
        "Start Date",
        "Target Date",
        "Lead",
        "Teams",
        "Members",
        "Created",
        "Updated",
        "URL",
    ])?;

    for project in &projects {
        let teams: Vec<&str> = project["teams"]["nodes"]
            .as_array()
            .map(|a| a.iter().filter_map(|t| t["key"].as_str()).collect())
            .unwrap_or_default();

        let members: Vec<&str> = project["members"]["nodes"]
            .as_array()
            .map(|a| a.iter().filter_map(|m| m["name"].as_str()).collect())
            .unwrap_or_default();

        let progress = project["progress"]
            .as_f64()
            .map(|p| format!("{:.0}%", p * 100.0))
            .unwrap_or_default();

        wtr.write_record([
            sanitize_csv_cell(project["name"].as_str().unwrap_or("")).as_ref(),
            sanitize_csv_cell(project["state"].as_str().unwrap_or("")).as_ref(),
            &project["priority"].as_i64().unwrap_or(0).to_string(),
            sanitize_csv_cell(&progress).as_ref(),
            sanitize_csv_cell(project["startDate"].as_str().unwrap_or("")).as_ref(),
            sanitize_csv_cell(project["targetDate"].as_str().unwrap_or("")).as_ref(),
            sanitize_csv_cell(project["lead"]["name"].as_str().unwrap_or("")).as_ref(),
            sanitize_csv_cell(&teams.join("; ")).as_ref(),
            sanitize_csv_cell(&members.join("; ")).as_ref(),
            &project["createdAt"]
                .as_str()
                .unwrap_or("")
                .chars()
                .take(10)
                .collect::<String>(),
            &project["updatedAt"]
                .as_str()
                .unwrap_or("")
                .chars()
                .take(10)
                .collect::<String>(),
            sanitize_csv_cell(project["url"].as_str().unwrap_or("")).as_ref(),
        ])?;
    }

    wtr.flush()?;

    if let Some(ref path) = file {
        eprintln!(
            "{}",
            format!("Exported {} projects to {}", projects.len(), path).green()
        );
    }

    Ok(())
}
