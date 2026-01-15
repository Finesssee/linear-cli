use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::json;
use std::process::Command;
use tabled::{Table, Tabled};

use crate::api::{resolve_team_id, LinearClient};
use crate::OutputFormat;

use super::templates;

#[derive(Subcommand)]
pub enum IssueCommands {
    /// List issues
    #[command(alias = "ls")]
    #[command(after_help = r#"EXAMPLES:
    linear issues list                         # List all issues
    linear i list -t ENG                       # Filter by team
    linear i list -t ENG -s "In Progress"      # Filter by team and status
    linear i list --assignee me                # Show my assigned issues
    linear i list --project "My Project"       # Filter by project name
    linear i list --output json                # Output as JSON"#)]
    List {
        /// Filter by team name or ID
        #[arg(short, long)]
        team: Option<String>,
        /// Filter by state name or ID
        #[arg(short, long)]
        state: Option<String>,
        /// Filter by assignee (user ID, name, email, or "me")
        #[arg(short, long)]
        assignee: Option<String>,
        /// Filter by project name
        #[arg(long)]
        project: Option<String>,
        /// Include archived issues
        #[arg(long)]
        archived: bool,
        /// Maximum number of issues to return
        #[arg(short, long, default_value = "50")]
        limit: u32,
    },
    /// Get issue details
    #[command(after_help = r#"EXAMPLES:
    linear issues get LIN-123                  # View issue by identifier
    linear i get abc123-uuid                   # View issue by ID
    linear i get LIN-123 --output json         # Output as JSON"#)]
    Get {
        /// Issue ID or identifier (e.g., "LIN-123")
        id: String,
    },
    /// Create a new issue
    #[command(after_help = r#"EXAMPLES:
    linear issues create "Fix bug" -t ENG      # Create with title and team
    linear i create "Feature" -t ENG -p 2      # Create with high priority
    linear i create "Task" -t ENG -a me        # Assign to yourself
    linear i create "Bug" -t ENG -s "Backlog"  # Set initial status"#)]
    Create {
        /// Issue title
        title: String,
        /// Team name or ID (can be provided via template)
        #[arg(short, long)]
        team: Option<String>,
        /// Issue description (markdown)
        #[arg(short, long)]
        description: Option<String>,
        /// Priority (0=none, 1=urgent, 2=high, 3=normal, 4=low)
        #[arg(short, long)]
        priority: Option<i32>,
        /// State name or ID
        #[arg(short, long)]
        state: Option<String>,
        /// Assignee (user ID, name, email, or "me")
        #[arg(short, long)]
        assignee: Option<String>,
        /// Labels to add (can be specified multiple times)
        #[arg(short, long)]
        labels: Vec<String>,
        /// Template name to use for default values
        #[arg(long)]
        template: Option<String>,
    },
    /// Update an existing issue
    #[command(after_help = r#"EXAMPLES:
    linear issues update LIN-123 -s Done       # Mark as done
    linear i update LIN-123 -T "New title"     # Change title
    linear i update LIN-123 -p 1               # Set to urgent priority
    linear i update LIN-123 -a me              # Assign to yourself"#)]
    Update {
        /// Issue ID
        id: String,
        /// New title
        #[arg(short = 'T', long)]
        title: Option<String>,
        /// New description
        #[arg(short, long)]
        description: Option<String>,
        /// New priority (0=none, 1=urgent, 2=high, 3=normal, 4=low)
        #[arg(short, long)]
        priority: Option<i32>,
        /// New state name or ID
        #[arg(short, long)]
        state: Option<String>,
        /// New assignee (user ID, name, email, or "me")
        #[arg(short, long)]
        assignee: Option<String>,
    },
    /// Delete an issue
    #[command(after_help = r#"EXAMPLES:
    linear issues delete LIN-123               # Delete with confirmation
    linear i delete LIN-123 --force            # Delete without confirmation"#)]
    Delete {
        /// Issue ID
        id: String,
        /// Skip confirmation
        #[arg(short, long)]
        force: bool,
    },
    /// Start working on an issue (set to In Progress and assign to me)
    #[command(after_help = r#"EXAMPLES:
    linear issues start LIN-123                # Start working on issue
    linear i start LIN-123 --checkout          # Start and checkout git branch
    linear i start LIN-123 -c -b feature/fix   # Start with custom branch"#)]
    Start {
        /// Issue ID or identifier (e.g., "LIN-123")
        id: String,
        /// Checkout a git branch for the issue
        #[arg(short, long)]
        checkout: bool,
        /// Custom branch name (optional, uses issue's branch name by default)
        #[arg(short, long)]
        branch: Option<String>,
    },
    /// Stop working on an issue (return to backlog state)
    #[command(after_help = r#"EXAMPLES:
    linear issues stop LIN-123                 # Stop working on issue
    linear i stop LIN-123 --unassign           # Stop and unassign"#)]
    Stop {
        /// Issue ID or identifier (e.g., "LIN-123")
        id: String,
        /// Unassign the issue
        #[arg(short, long)]
        unassign: bool,
    },
}

#[derive(Tabled)]
struct IssueRow {
    #[tabled(rename = "ID")]
    identifier: String,
    #[tabled(rename = "Title")]
    title: String,
    #[tabled(rename = "State")]
    state: String,
    #[tabled(rename = "Priority")]
    priority: String,
    #[tabled(rename = "Assignee")]
    assignee: String,
}

pub async fn handle(cmd: IssueCommands, output: OutputFormat) -> Result<()> {
    match cmd {
        IssueCommands::List {
            team,
            state,
            assignee,
            project,
            archived,
            limit,
        } => list_issues(team, state, assignee, project, archived, limit, output).await,
        IssueCommands::Get { id } => get_issue(&id, output).await,
        IssueCommands::Create {
            title,
            team,
            description,
            priority,
            state,
            assignee,
            labels,
            template,
        } => {
            // Load template if specified
            let tpl = if let Some(ref tpl_name) = template {
                templates::get_template(tpl_name)?
                    .ok_or_else(|| anyhow::anyhow!("Template not found: {}", tpl_name))?
            } else {
                templates::IssueTemplate {
                    name: String::new(),
                    title_prefix: None,
                    description: None,
                    default_priority: None,
                    default_labels: vec![],
                    team: None,
                }
            };

            // Team from CLI arg takes precedence, then template, then error
            let final_team = team
                .or(tpl.team.clone())
                .ok_or_else(|| anyhow::anyhow!("--team is required (or use a template with a default team)"))?;

            // Build title with optional prefix from template
            let final_title = if let Some(ref prefix) = tpl.title_prefix {
                format!("{} {}", prefix, title)
            } else {
                title
            };

            // Merge template defaults with CLI args (CLI takes precedence)
            let final_description = description.or(tpl.description.clone());
            let final_priority = priority.or(tpl.default_priority);

            // Merge labels: template labels + CLI labels
            let mut final_labels = tpl.default_labels.clone();
            final_labels.extend(labels);

            create_issue(
                &final_title,
                &final_team,
                final_description,
                final_priority,
                state,
                assignee,
                final_labels,
                output,
            )
            .await
        }
        IssueCommands::Update {
            id,
            title,
            description,
            priority,
            state,
            assignee,
        } => update_issue(&id, title, description, priority, state, assignee, output).await,
        IssueCommands::Delete { id, force } => delete_issue(&id, force).await,
        IssueCommands::Start {
            id,
            checkout,
            branch,
        } => start_issue(&id, checkout, branch).await,
        IssueCommands::Stop { id, unassign } => stop_issue(&id, unassign).await,
    }
}

fn priority_to_string(priority: Option<i64>) -> String {
    match priority {
        Some(0) => "-".to_string(),
        Some(1) => "Urgent".red().to_string(),
        Some(2) => "High".yellow().to_string(),
        Some(3) => "Normal".to_string(),
        Some(4) => "Low".dimmed().to_string(),
        _ => "-".to_string(),
    }
}

async fn list_issues(
    team: Option<String>,
    state: Option<String>,
    assignee: Option<String>,
    project: Option<String>,
    include_archived: bool,
    limit: u32,
    output: OutputFormat,
) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($team: String, $state: String, $assignee: String, $project: String, $includeArchived: Boolean, $limit: Int) {
            issues(
                first: $limit,
                includeArchived: $includeArchived,
                filter: {
                    team: { name: { eqIgnoreCase: $team } },
                    state: { name: { eqIgnoreCase: $state } },
                    assignee: { name: { eqIgnoreCase: $assignee } },
                    project: { name: { eqIgnoreCase: $project } }
                }
            ) {
                nodes {
                    id
                    identifier
                    title
                    priority
                    state { name }
                    assignee { name }
                }
            }
        }
    "#;

    let mut variables = json!({
        "includeArchived": include_archived,
        "limit": limit
    });

    if let Some(t) = team {
        variables["team"] = json!(t);
    }
    if let Some(s) = state {
        variables["state"] = json!(s);
    }
    if let Some(a) = assignee {
        variables["assignee"] = json!(a);
    }
    if let Some(p) = project {
        variables["project"] = json!(p);
    }

    let result = client.query(query, Some(variables)).await?;

    // Handle JSON output
    if matches!(output, OutputFormat::Json) {
        println!(
            "{}",
            serde_json::to_string_pretty(&result["data"]["issues"]["nodes"])?
        );
        return Ok(());
    }

    let empty = vec![];
    let issues = result["data"]["issues"]["nodes"]
        .as_array()
        .unwrap_or(&empty);

    if issues.is_empty() {
        println!("No issues found.");
        return Ok(());
    }

    let rows: Vec<IssueRow> = issues
        .iter()
        .map(|issue| IssueRow {
            identifier: issue["identifier"].as_str().unwrap_or("").to_string(),
            title: {
                let t = issue["title"].as_str().unwrap_or("");
                if t.len() > 50 {
                    format!("{}...", &t[..47])
                } else {
                    t.to_string()
                }
            },
            state: issue["state"]["name"].as_str().unwrap_or("-").to_string(),
            priority: priority_to_string(issue["priority"].as_i64()),
            assignee: issue["assignee"]["name"]
                .as_str()
                .unwrap_or("-")
                .to_string(),
        })
        .collect();

    let table = Table::new(rows).to_string();
    println!("{}", table);
    println!("\n{} issues", issues.len());

    Ok(())
}

async fn get_issue(id: &str, output: OutputFormat) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($id: String!) {
            issue(id: $id) {
                id
                identifier
                title
                description
                priority
                url
                createdAt
                updatedAt
                state { name }
                team { name }
                assignee { name email }
                labels { nodes { name color } }
                project { name }
                parent { identifier title }
            }
        }
    "#;

    let result = client.query(query, Some(json!({ "id": id }))).await?;
    let issue = &result["data"]["issue"];

    if issue.is_null() {
        anyhow::bail!("Issue not found: {}", id);
    }

    // Handle JSON output
    if matches!(output, OutputFormat::Json) {
        println!("{}", serde_json::to_string_pretty(issue)?);
        return Ok(());
    }

    let identifier = issue["identifier"].as_str().unwrap_or("");
    let title = issue["title"].as_str().unwrap_or("");
    println!("{} {}", identifier.cyan().bold(), title.bold());
    println!("{}", "-".repeat(60));

    if let Some(desc) = issue["description"].as_str() {
        if !desc.is_empty() {
            println!("\n{}", desc);
            println!();
        }
    }

    println!(
        "State:    {}",
        issue["state"]["name"].as_str().unwrap_or("-")
    );
    println!(
        "Priority: {}",
        priority_to_string(issue["priority"].as_i64())
    );
    println!(
        "Team:     {}",
        issue["team"]["name"].as_str().unwrap_or("-")
    );

    if let Some(assignee) = issue["assignee"]["name"].as_str() {
        let email = issue["assignee"]["email"].as_str().unwrap_or("");
        if !email.is_empty() {
            println!("Assignee: {} ({})", assignee, email.dimmed());
        } else {
            println!("Assignee: {}", assignee);
        }
    } else {
        println!("Assignee: -");
    }

    if let Some(project) = issue["project"]["name"].as_str() {
        println!("Project:  {}", project);
    }

    if let Some(parent) = issue["parent"]["identifier"].as_str() {
        let parent_title = issue["parent"]["title"].as_str().unwrap_or("");
        println!("Parent:   {} {}", parent, parent_title.dimmed());
    }

    let labels = issue["labels"]["nodes"].as_array();
    if let Some(labels) = labels {
        if !labels.is_empty() {
            let label_names: Vec<&str> = labels.iter().filter_map(|l| l["name"].as_str()).collect();
            println!("Labels:   {}", label_names.join(", "));
        }
    }

    println!("\nURL: {}", issue["url"].as_str().unwrap_or("-"));
    println!("ID:  {}", issue["id"].as_str().unwrap_or("-"));

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn create_issue(
    title: &str,
    team: &str,
    description: Option<String>,
    priority: Option<i32>,
    state: Option<String>,
    assignee: Option<String>,
    labels: Vec<String>,
    output: OutputFormat,
) -> Result<()> {
    let client = LinearClient::new()?;

    // Determine the final team (CLI arg takes precedence, then template, then error)
    let final_team = team;

    // Resolve team key/name to UUID
    let team_id = resolve_team_id(&client, final_team).await?;

    // Build the title with optional prefix from template
    let final_title = title.to_string();

    let mut input = json!({
        "title": final_title,
        "teamId": team_id
    });

    // CLI args override template values
    if let Some(desc) = description {
        input["description"] = json!(desc);
    }
    if let Some(p) = priority {
        input["priority"] = json!(p);
    }
    if let Some(s) = state {
        input["stateId"] = json!(s);
    }
    if let Some(a) = assignee {
        input["assigneeId"] = json!(a);
    }
    if !labels.is_empty() {
        // Merge with template labels if present
        let existing: Vec<String> = input["labelIds"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let mut all_labels = existing;
        all_labels.extend(labels);
        input["labelIds"] = json!(all_labels);
    }

    let mutation = r#"
        mutation($input: IssueCreateInput!) {
            issueCreate(input: $input) {
                success
                issue {
                    id
                    identifier
                    title
                    url
                }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "input": input })))
        .await?;

    if result["data"]["issueCreate"]["success"].as_bool() == Some(true) {
        let issue = &result["data"]["issueCreate"]["issue"];

        // Handle JSON output
        if matches!(output, OutputFormat::Json) {
            println!("{}", serde_json::to_string_pretty(issue)?);
            return Ok(());
        }

        let identifier = issue["identifier"].as_str().unwrap_or("");
        let issue_title = issue["title"].as_str().unwrap_or("");
        println!(
            "{} Created issue: {} {}",
            "+".green(),
            identifier.cyan(),
            issue_title
        );
        println!("  ID:  {}", issue["id"].as_str().unwrap_or(""));
        println!("  URL: {}", issue["url"].as_str().unwrap_or(""));
    } else {
        anyhow::bail!("Failed to create issue");
    }

    Ok(())
}

async fn update_issue(
    id: &str,
    title: Option<String>,
    description: Option<String>,
    priority: Option<i32>,
    state: Option<String>,
    assignee: Option<String>,
    output: OutputFormat,
) -> Result<()> {
    let client = LinearClient::new()?;

    let mut input = json!({});

    if let Some(t) = title {
        input["title"] = json!(t);
    }
    if let Some(d) = description {
        input["description"] = json!(d);
    }
    if let Some(p) = priority {
        input["priority"] = json!(p);
    }
    if let Some(s) = state {
        input["stateId"] = json!(s);
    }
    if let Some(a) = assignee {
        input["assigneeId"] = json!(a);
    }

    if input.as_object().map(|o| o.is_empty()).unwrap_or(true) {
        println!("No updates specified.");
        return Ok(());
    }

    let mutation = r#"
        mutation($id: String!, $input: IssueUpdateInput!) {
            issueUpdate(id: $id, input: $input) {
                success
                issue {
                    identifier
                    title
                }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "id": id, "input": input })))
        .await?;

    if result["data"]["issueUpdate"]["success"].as_bool() == Some(true) {
        let issue = &result["data"]["issueUpdate"]["issue"];

        // Handle JSON output
        if matches!(output, OutputFormat::Json) {
            println!("{}", serde_json::to_string_pretty(issue)?);
            return Ok(());
        }

        println!(
            "{} Updated issue: {} {}",
            "+".green(),
            issue["identifier"].as_str().unwrap_or(""),
            issue["title"].as_str().unwrap_or("")
        );
    } else {
        anyhow::bail!("Failed to update issue");
    }

    Ok(())
}

async fn delete_issue(id: &str, force: bool) -> Result<()> {
    if !force {
        let confirm = dialoguer::Confirm::new()
            .with_prompt(format!("Delete issue {}? This cannot be undone", id))
            .default(false)
            .interact()?;

        if !confirm {
            println!("Cancelled.");
            return Ok(());
        }
    }

    let client = LinearClient::new()?;

    let mutation = r#"
        mutation($id: String!) {
            issueDelete(id: $id) {
                success
            }
        }
    "#;

    let result = client.mutate(mutation, Some(json!({ "id": id }))).await?;

    if result["data"]["issueDelete"]["success"].as_bool() == Some(true) {
        println!("{} Issue deleted", "+".green());
    } else {
        anyhow::bail!("Failed to delete issue");
    }

    Ok(())
}

// Git helper functions for start command
fn run_git_command(args: &[&str]) -> Result<String> {
    let output = Command::new("git").args(args).output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Git command failed: {}", stderr.trim());
    }
}

fn branch_exists(branch: &str) -> bool {
    run_git_command(&["rev-parse", "--verify", branch]).is_ok()
}

fn generate_branch_name(identifier: &str, title: &str) -> String {
    // Convert title to kebab-case for branch name
    let slug: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    // Truncate if too long
    let slug = if slug.len() > 50 {
        slug[..50].trim_end_matches('-').to_string()
    } else {
        slug
    };

    format!("{}/{}", identifier.to_lowercase(), slug)
}

async fn start_issue(id: &str, checkout: bool, custom_branch: Option<String>) -> Result<()> {
    let client = LinearClient::new()?;

    // First, get the issue details including team info to find the "started" state
    let query = r#"
        query($id: String!) {
            issue(id: $id) {
                id
                identifier
                title
                branchName
                team {
                    id
                    states {
                        nodes {
                            id
                            name
                            type
                        }
                    }
                }
            }
            viewer {
                id
            }
        }
    "#;

    let result = client.query(query, Some(json!({ "id": id }))).await?;
    let issue = &result["data"]["issue"];

    if issue.is_null() {
        anyhow::bail!("Issue not found: {}", id);
    }

    let identifier = issue["identifier"].as_str().unwrap_or("");
    let title = issue["title"].as_str().unwrap_or("");
    let linear_branch = issue["branchName"].as_str().unwrap_or("").to_string();

    // Get current user ID
    let viewer_id = result["data"]["viewer"]["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Could not fetch current user ID"))?;

    // Find a "started" type state (In Progress)
    let empty = vec![];
    let states = issue["team"]["states"]["nodes"]
        .as_array()
        .unwrap_or(&empty);

    let started_state = states
        .iter()
        .find(|s| s["type"].as_str() == Some("started"));

    let state_id = match started_state {
        Some(s) => s["id"].as_str().unwrap_or(""),
        None => anyhow::bail!("No 'started' state found for this team"),
    };

    let state_name = started_state
        .and_then(|s| s["name"].as_str())
        .unwrap_or("In Progress");

    // Update the issue: set state to "In Progress" and assign to current user
    let input = json!({
        "stateId": state_id,
        "assigneeId": viewer_id
    });

    let mutation = r#"
        mutation($id: String!, $input: IssueUpdateInput!) {
            issueUpdate(id: $id, input: $input) {
                success
                issue {
                    identifier
                    title
                    state { name }
                    assignee { name }
                }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "id": id, "input": input })))
        .await?;

    if result["data"]["issueUpdate"]["success"].as_bool() == Some(true) {
        let updated = &result["data"]["issueUpdate"]["issue"];
        println!(
            "{} Started issue: {} {}",
            "+".green(),
            updated["identifier"].as_str().unwrap_or("").cyan(),
            updated["title"].as_str().unwrap_or("")
        );
        println!(
            "  State:    {}",
            updated["state"]["name"].as_str().unwrap_or(state_name)
        );
        println!(
            "  Assignee: {}",
            updated["assignee"]["name"].as_str().unwrap_or("me")
        );
    } else {
        anyhow::bail!("Failed to start issue");
    }

    // Optionally checkout a git branch
    if checkout {
        let branch_name = custom_branch
            .or(if linear_branch.is_empty() {
                None
            } else {
                Some(linear_branch)
            })
            .unwrap_or_else(|| generate_branch_name(identifier, title));

        println!();
        if branch_exists(&branch_name) {
            println!("Checking out existing branch: {}", branch_name.green());
            run_git_command(&["checkout", &branch_name])?;
        } else {
            println!("Creating and checking out branch: {}", branch_name.green());
            run_git_command(&["checkout", "-b", &branch_name])?;
        }

        let current = run_git_command(&["rev-parse", "--abbrev-ref", "HEAD"])?;
        println!("{} Now on branch: {}", "+".green(), current);
    }

    Ok(())
}

async fn stop_issue(id: &str, unassign: bool) -> Result<()> {
    let client = LinearClient::new()?;

    // First, get the issue details including team info to find the "backlog" or "unstarted" state
    let query = r#"
        query($id: String!) {
            issue(id: $id) {
                id
                identifier
                title
                team {
                    id
                    states {
                        nodes {
                            id
                            name
                            type
                        }
                    }
                }
            }
        }
    "#;

    let result = client.query(query, Some(json!({ "id": id }))).await?;
    let issue = &result["data"]["issue"];

    if issue.is_null() {
        anyhow::bail!("Issue not found: {}", id);
    }

    // Find a "backlog" or "unstarted" type state
    let empty = vec![];
    let states = issue["team"]["states"]["nodes"]
        .as_array()
        .unwrap_or(&empty);

    // Prefer backlog, fall back to unstarted
    let stop_state = states
        .iter()
        .find(|s| s["type"].as_str() == Some("backlog"))
        .or_else(|| {
            states
                .iter()
                .find(|s| s["type"].as_str() == Some("unstarted"))
        });

    let state_id = match stop_state {
        Some(s) => s["id"].as_str().unwrap_or(""),
        None => anyhow::bail!("No 'backlog' or 'unstarted' state found for this team"),
    };

    let state_name = stop_state
        .and_then(|s| s["name"].as_str())
        .unwrap_or("Backlog");

    // Build the update input
    let mut input = json!({
        "stateId": state_id
    });

    // Optionally unassign
    if unassign {
        input["assigneeId"] = json!(null);
    }

    let mutation = r#"
        mutation($id: String!, $input: IssueUpdateInput!) {
            issueUpdate(id: $id, input: $input) {
                success
                issue {
                    identifier
                    title
                    state { name }
                    assignee { name }
                }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "id": id, "input": input })))
        .await?;

    if result["data"]["issueUpdate"]["success"].as_bool() == Some(true) {
        let updated = &result["data"]["issueUpdate"]["issue"];
        println!(
            "{} Stopped issue: {} {}",
            "+".green(),
            updated["identifier"].as_str().unwrap_or("").cyan(),
            updated["title"].as_str().unwrap_or("")
        );
        println!(
            "  State:    {}",
            updated["state"]["name"].as_str().unwrap_or(state_name)
        );
        if unassign {
            println!("  Assignee: (unassigned)");
        } else if let Some(assignee) = updated["assignee"]["name"].as_str() {
            println!("  Assignee: {}", assignee);
        }
    } else {
        anyhow::bail!("Failed to stop issue");
    }

    Ok(())
}
