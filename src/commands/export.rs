use anyhow::Result;
use clap::Subcommand;
use serde_json::json;
use std::io::Write;

use crate::api::LinearClient;
use crate::output::OutputOptions;

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
    },
    /// Export issues to Markdown
    Markdown {
        /// Team key to export
        #[arg(short, long)]
        team: Option<String>,
        /// Output file (default: stdout)
        #[arg(short, long)]
        file: Option<String>,
    },
}

pub async fn handle(cmd: ExportCommands, _output: &OutputOptions) -> Result<()> {
    match cmd {
        ExportCommands::Csv {
            team,
            file,
            include_completed,
        } => export_csv(team, file, include_completed).await,
        ExportCommands::Markdown { team, file } => export_markdown(team, file).await,
    }
}

async fn export_csv(
    team: Option<String>,
    file: Option<String>,
    include_completed: bool,
) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($filter: IssueFilter) {
            issues(first: 250, filter: $filter) {
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

    let result = client
        .query(query, Some(json!({ "filter": filter })))
        .await?;
    let issues = result["data"]["issues"]["nodes"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let mut output: Box<dyn Write> = if let Some(ref path) = file {
        Box::new(std::fs::File::create(path)?)
    } else {
        Box::new(std::io::stdout())
    };

    // Write CSV header
    writeln!(
        output,
        "Identifier,Title,Status,Priority,Estimate,Due Date,Assignee,Team,Project,Cycle,Labels,Created,Updated"
    )?;

    // Write rows
    for issue in &issues {
        let labels: Vec<&str> = issue["labels"]["nodes"]
            .as_array()
            .map(|a| a.iter().filter_map(|l| l["name"].as_str()).collect())
            .unwrap_or_default();

        writeln!(
            output,
            "\"{}\",\"{}\",\"{}\",{},{},\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"",
            issue["identifier"].as_str().unwrap_or(""),
            issue["title"].as_str().unwrap_or("").replace('"', "\"\""),
            issue["state"]["name"].as_str().unwrap_or(""),
            issue["priority"].as_i64().unwrap_or(0),
            issue["estimate"].as_f64().unwrap_or(0.0),
            issue["dueDate"].as_str().unwrap_or(""),
            issue["assignee"]["name"].as_str().unwrap_or(""),
            issue["team"]["key"].as_str().unwrap_or(""),
            issue["project"]["name"].as_str().unwrap_or(""),
            issue["cycle"]["name"].as_str().unwrap_or(""),
            labels.join("; "),
            issue["createdAt"].as_str().unwrap_or("").chars().take(10).collect::<String>(),
            issue["updatedAt"].as_str().unwrap_or("").chars().take(10).collect::<String>(),
        )?;
    }

    if file.is_some() {
        eprintln!("Exported {} issues to {}", issues.len(), file.unwrap());
    }

    Ok(())
}

async fn export_markdown(team: Option<String>, file: Option<String>) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($filter: IssueFilter) {
            issues(first: 250, filter: $filter) {
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
            }
        }
    "#;

    let mut filter = json!({ "state": { "type": { "neq": "completed" } } });
    if let Some(ref t) = team {
        filter["team"] = json!({ "key": { "eq": t } });
    }

    let result = client
        .query(query, Some(json!({ "filter": filter })))
        .await?;
    let issues = result["data"]["issues"]["nodes"]
        .as_array()
        .cloned()
        .unwrap_or_default();

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
