use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::{json, Map, Value};
use std::io::{self, BufRead};
use tabled::{Table, Tabled};

use crate::api::{
    resolve_label_id, resolve_state_id, resolve_team_id, resolve_user_id, LinearClient,
};
use crate::display_options;
use crate::input::read_ids_from_stdin;
use crate::output::{ensure_non_empty, filter_values, print_json, sort_values, OutputOptions};
use crate::pagination::{paginate_nodes, stream_nodes};
use crate::priority::priority_to_string;
use crate::text::truncate;
use crate::vcs::{generate_branch_name, run_git_command};
use crate::AgentOptions;

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
    },
    /// Get issue details
    #[command(after_help = r#"EXAMPLES:
    linear issues get LIN-123                  # View issue by identifier
    linear i get abc123-uuid                   # View issue by ID
    linear i get LIN-1 LIN-2 LIN-3             # Get multiple issues
    linear i get LIN-123 --output json         # Output as JSON
    echo "LIN-123" | linear i get -            # Read ID from stdin (piping)"#)]
    Get {
        /// Issue ID(s) or identifier(s). Use "-" to read from stdin.
        ids: Vec<String>,
    },
    /// Create a new issue
    #[command(after_help = r#"EXAMPLES:
    linear issues create "Fix bug" -t ENG      # Create with title and team
    linear i create "Feature" -t ENG -p 2      # Create with high priority
    linear i create "Task" -t ENG -a me        # Assign to yourself
    linear i create "Task" -t ENG --due +3d    # Due in 3 days
    linear i create "Bug" -t ENG --dry-run     # Preview without creating"#)]
    Create {
        /// Issue title
        title: String,
        /// Team name or ID (can be provided via template)
        #[arg(short, long)]
        team: Option<String>,
        /// Issue description (markdown). Use "-" to read from stdin.
        #[arg(short, long)]
        description: Option<String>,
        /// JSON input for issue fields. Use "-" to read from stdin.
        #[arg(long)]
        data: Option<String>,
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
        /// Due date (today, tomorrow, +3d, +1w, or YYYY-MM-DD)
        #[arg(long)]
        due: Option<String>,
        /// Estimate in points (e.g., 1, 2, 3, 5, 8)
        #[arg(short, long)]
        estimate: Option<f64>,
        /// Template name to use for default values
        #[arg(long)]
        template: Option<String>,
        /// Preview without creating (dry run)
        #[arg(long)]
        dry_run: bool,
    },
    /// Update an existing issue
    #[command(after_help = r#"EXAMPLES:
    linear issues update LIN-123 -s Done       # Mark as done
    linear i update LIN-123 -T "New title"     # Change title
    linear i update LIN-123 -p 1               # Set to urgent priority
    linear i update LIN-123 --due tomorrow     # Due tomorrow
    linear i update LIN-123 -a me              # Assign to yourself
    linear i update LIN-123 -l bug -l urgent   # Add labels"#)]
    Update {
        /// Issue ID
        id: String,
        /// New title
        #[arg(short = 'T', long)]
        title: Option<String>,
        /// New description
        #[arg(short, long)]
        description: Option<String>,
        /// JSON input for issue fields. Use "-" to read from stdin.
        #[arg(long)]
        data: Option<String>,
        /// New priority (0=none, 1=urgent, 2=high, 3=normal, 4=low)
        #[arg(short, long)]
        priority: Option<i32>,
        /// New state name or ID
        #[arg(short, long)]
        state: Option<String>,
        /// New assignee (user ID, name, email, or "me")
        #[arg(short, long)]
        assignee: Option<String>,
        /// Labels to set (can be specified multiple times)
        #[arg(short, long)]
        labels: Vec<String>,
        /// Due date (today, tomorrow, +3d, +1w, YYYY-MM-DD, or "none" to clear)
        #[arg(long)]
        due: Option<String>,
        /// Estimate in points (e.g., 1, 2, 3, 5, 8, or 0 to clear)
        #[arg(short, long)]
        estimate: Option<f64>,
        /// Preview without updating (dry run)
        #[arg(long)]
        dry_run: bool,
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

pub async fn handle(
    cmd: IssueCommands,
    output: &OutputOptions,
    agent_opts: AgentOptions,
) -> Result<()> {
    match cmd {
        IssueCommands::List {
            team,
            state,
            assignee,
            project,
            archived,
        } => list_issues(team, state, assignee, project, archived, output, agent_opts).await,
        IssueCommands::Get { ids } => {
            // Support reading from stdin if no IDs provided or if "-" is passed
            let final_ids = read_ids_from_stdin(ids);
            if final_ids.is_empty() {
                anyhow::bail!(
                    "No issue IDs provided. Provide IDs as arguments or pipe them via stdin."
                );
            }
            get_issues(&final_ids, output).await
        }
        IssueCommands::Create {
            title,
            team,
            description,
            data,
            priority,
            state,
            assignee,
            labels,
            due,
            estimate,
            template,
            dry_run,
        } => {
            let dry_run = dry_run || output.dry_run || agent_opts.dry_run;
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
            let data_json = read_json_data(data.as_deref())?;
            let data_team = data_json.as_ref().and_then(|v| {
                v.get("team")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string())
            });
            let data_team_id = data_json.as_ref().and_then(|v| {
                v.get("teamId")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string())
            });
            let final_team = team
                .or(tpl.team.clone())
                .or(data_team)
                .or(data_team_id)
                .ok_or_else(|| {
                    anyhow::anyhow!("--team is required (or use a template with a default team)")
                })?;

            // Build title with optional prefix from template
            let final_title = if let Some(ref prefix) = tpl.title_prefix {
                format!("{} {}", prefix, title)
            } else {
                title
            };

            // Merge template defaults with CLI args (CLI takes precedence)
            // Support reading description from stdin if "-" is passed
            if data.as_deref() == Some("-") && description.as_deref() == Some("-") {
                anyhow::bail!("--data - and --description - cannot both read from stdin");
            }

            let final_description = match description.as_deref() {
                Some("-") => {
                    let stdin = io::stdin();
                    let lines: Vec<String> = stdin.lock().lines().map_while(Result::ok).collect();
                    Some(lines.join("\n"))
                }
                Some(d) => Some(d.to_string()),
                None => tpl.description.clone(),
            };
            let final_priority = priority.or(tpl.default_priority);

            // Merge labels: template labels + CLI labels
            let mut final_labels = tpl.default_labels.clone();
            final_labels.extend(labels);

            create_issue(
                &final_title,
                &final_team,
                data_json,
                final_description,
                final_priority,
                state,
                assignee,
                final_labels,
                due,
                estimate,
                output,
                agent_opts,
                dry_run,
            )
            .await
        }
        IssueCommands::Update {
            id,
            title,
            description,
            data,
            priority,
            state,
            assignee,
            labels,
            due,
            estimate,
            dry_run,
        } => {
            let dry_run = dry_run || output.dry_run || agent_opts.dry_run;
            if data.as_deref() == Some("-") && description.as_deref() == Some("-") {
                anyhow::bail!("--data - and --description - cannot both read from stdin");
            }

            let data_json = read_json_data(data.as_deref())?;
            // Support reading description from stdin if "-" is passed
            let final_description = match description.as_deref() {
                Some("-") => {
                    let stdin = io::stdin();
                    let lines: Vec<String> = stdin.lock().lines().map_while(Result::ok).collect();
                    Some(lines.join("\n"))
                }
                Some(d) => Some(d.to_string()),
                None => None,
            };
            update_issue(
                &id,
                title,
                final_description,
                data_json,
                priority,
                state,
                assignee,
                labels,
                due,
                estimate,
                dry_run,
                output,
                agent_opts,
            )
            .await
        }
        IssueCommands::Delete { id, force } => delete_issue(&id, force, agent_opts).await,
        IssueCommands::Start {
            id,
            checkout,
            branch,
        } => start_issue(&id, checkout, branch, agent_opts).await,
        IssueCommands::Stop { id, unassign } => stop_issue(&id, unassign, agent_opts).await,
    }
}

#[allow(clippy::too_many_arguments)]
async fn list_issues(
    team: Option<String>,
    state: Option<String>,
    assignee: Option<String>,
    project: Option<String>,
    include_archived: bool,
    output: &OutputOptions,
    _agent_opts: AgentOptions,
) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($team: String, $state: String, $assignee: String, $project: String, $includeArchived: Boolean, $first: Int, $after: String, $last: Int, $before: String) {
            issues(
                first: $first,
                after: $after,
                last: $last,
                before: $before,
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
                pageInfo {
                    hasNextPage
                    endCursor
                    hasPreviousPage
                    startCursor
                }
            }
        }
    "#;

    let mut variables = Map::new();
    variables.insert("includeArchived".to_string(), json!(include_archived));

    if let Some(t) = team {
        variables.insert("team".to_string(), json!(t));
    }
    if let Some(s) = state {
        variables.insert("state".to_string(), json!(s));
    }
    if let Some(a) = assignee {
        variables.insert("assignee".to_string(), json!(a));
    }
    if let Some(p) = project {
        variables.insert("project".to_string(), json!(p));
    }

    let pagination = output.pagination.with_default_limit(50);

    // For NDJSON, use streaming to avoid buffering all results
    if output.is_ndjson() {
        let mut count = 0;
        stream_nodes(
            &client,
            query,
            variables,
            &["data", "issues", "nodes"],
            &["data", "issues", "pageInfo"],
            &pagination,
            50,
            |batch| {
                count += batch.len();
                async move {
                    for issue in batch {
                        println!("{}", serde_json::to_string(&issue)?);
                    }
                    Ok(())
                }
            },
        )
        .await?;

        return Ok(());
    }

    // For other formats, use paginate_nodes (need all results for sorting/filtering/tables)
    let issues = paginate_nodes(
        &client,
        query,
        variables,
        &["data", "issues", "nodes"],
        &["data", "issues", "pageInfo"],
        &pagination,
        50,
    )
    .await?;

    if output.is_json() || output.has_template() {
        print_json(&serde_json::json!(issues), output)?;
        return Ok(());
    }

    let mut issues = issues;
    filter_values(&mut issues, &output.filters);

    if let Some(sort_key) = output.json.sort.as_deref() {
        sort_values(&mut issues, sort_key, output.json.order);
    }

    ensure_non_empty(&issues, output)?;
    if issues.is_empty() {
        println!("No issues found.");
        return Ok(());
    }

    let width = display_options().max_width(50);
    let rows: Vec<IssueRow> = issues
        .iter()
        .map(|issue| IssueRow {
            identifier: issue["identifier"].as_str().unwrap_or("").to_string(),
            title: truncate(issue["title"].as_str().unwrap_or(""), width),
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

/// Get multiple issues (supports batch fetching with concurrency limit)
async fn get_issues(ids: &[String], output: &OutputOptions) -> Result<()> {
    // Handle single ID (most common case)
    if ids.len() == 1 {
        return get_issue(&ids[0], output).await;
    }

    let client = LinearClient::new()?;

    // Limit concurrent requests to avoid rate limiting and socket exhaustion
    use futures::stream::{self, StreamExt};
    const MAX_CONCURRENT: usize = 10;

    let results: Vec<_> = stream::iter(ids.iter().cloned())
        .map(|id| {
            let client = client.clone();
            async move {
                let query = r#"
                    query($id: String!) {
                        issue(id: $id) {
                            id
                            identifier
                            title
                            description
                            priority
                            url
                            state { name }
                            team { name }
                            assignee { name }
                        }
                    }
                "#;
                let result = client.query(query, Some(json!({ "id": id }))).await;
                (id, result)
            }
        })
        .buffer_unordered(MAX_CONCURRENT)
        .collect()
        .await;

    // JSON output: array of issues
    if output.is_json() || output.has_template() {
        let issues: Vec<_> = results
            .iter()
            .filter_map(|(_, r)| {
                r.as_ref().ok().and_then(|data| {
                    let issue = &data["data"]["issue"];
                    if !issue.is_null() {
                        Some(issue.clone())
                    } else {
                        None
                    }
                })
            })
            .collect();
        print_json(&serde_json::json!(issues), output)?;
        return Ok(());
    }

    // Table output
    for (id, result) in results {
        match result {
            Ok(data) => {
                let issue = &data["data"]["issue"];
                if issue.is_null() {
                    eprintln!("{} Issue not found: {}", "!".yellow(), id);
                } else {
                    let identifier = issue["identifier"].as_str().unwrap_or("");
                    let title = issue["title"].as_str().unwrap_or("");
                    let state = issue["state"]["name"].as_str().unwrap_or("-");
                    let priority = priority_to_string(issue["priority"].as_i64());
                    println!("{} {} [{}] {}", identifier.cyan(), title, state, priority);
                }
            }
            Err(e) => {
                eprintln!("{} Error fetching {}: {}", "!".red(), id, e);
            }
        }
    }

    Ok(())
}

async fn get_issue(id: &str, output: &OutputOptions) -> Result<()> {
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
    if output.is_json() || output.has_template() {
        print_json(issue, output)?;
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
    data_json: Option<Value>,
    description: Option<String>,
    priority: Option<i32>,
    state: Option<String>,
    assignee: Option<String>,
    labels: Vec<String>,
    due: Option<String>,
    estimate: Option<f64>,
    output: &OutputOptions,
    agent_opts: AgentOptions,
    dry_run: bool,
) -> Result<()> {
    let client = LinearClient::new()?;

    // Determine the final team (CLI arg takes precedence, then template, then error)
    let final_team = team;

    // Resolve team key/name to UUID
    let team_id = resolve_team_id(&client, final_team, &output.cache).await?;

    // Build the title with optional prefix from template
    let final_title = title.to_string();

    let mut input = match data_json {
        Some(Value::Object(map)) => Value::Object(map),
        Some(_) => anyhow::bail!("--data must be a JSON object"),
        None => json!({}),
    };

    input["title"] = json!(final_title);
    input["teamId"] = json!(team_id);

    // CLI args override template values
    if let Some(ref desc) = description {
        input["description"] = json!(desc);
    }
    if let Some(p) = priority {
        input["priority"] = json!(p);
    }
    if let Some(ref s) = state {
        if dry_run {
            input["stateId"] = json!(s);
        } else {
            let state_id = resolve_state_id(&client, &team_id, s).await?;
            input["stateId"] = json!(state_id);
        }
    }
    if let Some(ref a) = assignee {
        // Resolve user name/email to UUID (skip during dry-run to avoid API calls)
        if dry_run {
            input["assigneeId"] = json!(a);
        } else {
            let assignee_id = resolve_user_id(&client, a, &output.cache).await?;
            input["assigneeId"] = json!(assignee_id);
        }
    }
    if !labels.is_empty() {
        // Resolve label names to UUIDs (skip during dry-run to avoid API calls)
        if dry_run {
            let mut label_ids: Vec<String> = input["labelIds"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            label_ids.extend(labels.clone());
            input["labelIds"] = json!(label_ids);
        } else {
            let mut label_ids: Vec<String> = input["labelIds"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            for label in &labels {
                let label_id = resolve_label_id(&client, label, &output.cache).await?;
                label_ids.push(label_id);
            }
            input["labelIds"] = json!(label_ids);
        }
    }
    if let Some(ref d) = due {
        // Parse due date shorthand
        if let Some(parsed) = crate::dates::parse_due_date(d) {
            input["dueDate"] = json!(parsed);
        } else {
            // Assume it's already a valid date format
            input["dueDate"] = json!(d);
        }
    }
    if let Some(e) = estimate {
        input["estimate"] = json!(e);
    }

    // Dry run: show what would be created without actually creating
    if dry_run {
        if output.is_json() || output.has_template() {
            print_json(
                &json!({
                    "dry_run": true,
                    "would_create": {
                        "title": final_title,
                        "team": final_team,
                        "teamId": team_id,
                        "description": description,
                        "priority": priority,
                        "state": state,
                        "assignee": assignee,
                        "labels": labels,
                        "dueDate": due,
                        "estimate": estimate,
                    }
                }),
                output,
            )?;
        } else {
            println!("{}", "[DRY RUN] Would create issue:".yellow().bold());
            println!("  Title:       {}", final_title);
            println!("  Team:        {} ({})", final_team, team_id);
            if let Some(ref desc) = description {
                let preview = if desc.len() > 50 {
                    format!("{}...", &desc[..50])
                } else {
                    desc.clone()
                };
                println!("  Description: {}", preview);
            }
            if let Some(p) = priority {
                println!("  Priority:    {}", p);
            }
            if let Some(ref s) = state {
                println!("  State:       {}", s);
            }
            if let Some(ref a) = assignee {
                println!("  Assignee:    {}", a);
            }
            if !labels.is_empty() {
                println!("  Labels:      {}", labels.join(", "));
            }
            if let Some(ref d) = due {
                println!("  Due:         {}", d);
            }
            if let Some(e) = estimate {
                println!("  Estimate:    {}", e);
            }
        }
        return Ok(());
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
        let identifier = issue["identifier"].as_str().unwrap_or("");

        // --id-only: Just output the identifier for chaining
        if agent_opts.id_only {
            println!("{}", identifier);
            return Ok(());
        }

        // Handle JSON output
        if output.is_json() || output.has_template() {
            print_json(issue, output)?;
            return Ok(());
        }

        // Quiet mode: minimal output
        if agent_opts.quiet {
            println!("{}", identifier);
            return Ok(());
        }

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

#[allow(clippy::too_many_arguments)]
async fn update_issue(
    id: &str,
    title: Option<String>,
    description: Option<String>,
    data_json: Option<Value>,
    priority: Option<i32>,
    state: Option<String>,
    assignee: Option<String>,
    labels: Vec<String>,
    due: Option<String>,
    estimate: Option<f64>,
    dry_run: bool,
    output: &OutputOptions,
    agent_opts: AgentOptions,
) -> Result<()> {
    let client = LinearClient::new()?;

    let mut input = match data_json {
        Some(Value::Object(map)) => Value::Object(map),
        Some(_) => anyhow::bail!("--data must be a JSON object"),
        None => json!({}),
    };

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
        if dry_run {
            input["stateId"] = json!(s);
        } else {
            // Fetch the issue's team ID to resolve state name
            let team_query = r#"
                query($id: String!) {
                    issue(id: $id) {
                        team { id }
                    }
                }
            "#;
            let team_result = client.query(team_query, Some(json!({ "id": id }))).await?;
            let issue_team_id = team_result["data"]["issue"]["team"]["id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Could not determine team for issue {}", id))?;
            let state_id = resolve_state_id(&client, issue_team_id, &s).await?;
            input["stateId"] = json!(state_id);
        }
    }
    if let Some(a) = assignee {
        // Resolve user name/email to UUID (skip during dry-run to avoid API calls)
        if dry_run {
            input["assigneeId"] = json!(a);
        } else {
            let assignee_id = resolve_user_id(&client, &a, &output.cache).await?;
            input["assigneeId"] = json!(assignee_id);
        }
    }
    if !labels.is_empty() {
        // Resolve label names to UUIDs (skip during dry-run to avoid API calls)
        if dry_run {
            input["labelIds"] = json!(labels);
        } else {
            let mut label_ids = Vec::new();
            for label in &labels {
                let label_id = resolve_label_id(&client, label, &output.cache).await?;
                label_ids.push(label_id);
            }
            input["labelIds"] = json!(label_ids);
        }
    }
    if let Some(ref d) = due {
        // Support clearing due date with "none"
        if d.eq_ignore_ascii_case("none") || d.eq_ignore_ascii_case("clear") {
            input["dueDate"] = json!(null);
        } else if let Some(parsed) = crate::dates::parse_due_date(d) {
            input["dueDate"] = json!(parsed);
        } else {
            input["dueDate"] = json!(d);
        }
    }
    if let Some(e) = estimate {
        // 0 clears the estimate
        if e == 0.0 {
            input["estimate"] = json!(null);
        } else {
            input["estimate"] = json!(e);
        }
    }

    if input.as_object().map(|o| o.is_empty()).unwrap_or(true) {
        if !agent_opts.quiet {
            println!("No updates specified.");
        }
        return Ok(());
    }

    if dry_run {
        if output.is_json() || output.has_template() {
            print_json(
                &json!({
                    "dry_run": true,
                    "would_update": {
                        "id": id,
                        "input": input,
                    }
                }),
                output,
            )?;
        } else {
            println!("{}", "[DRY RUN] Would update issue:".yellow().bold());
            println!("  ID: {}", id);
        }
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
        let identifier = issue["identifier"].as_str().unwrap_or("");

        // --id-only: Just output the identifier
        if agent_opts.id_only {
            println!("{}", identifier);
            return Ok(());
        }

        // Handle JSON output
        if output.is_json() || output.has_template() {
            print_json(issue, output)?;
            return Ok(());
        }

        // Quiet mode
        if agent_opts.quiet {
            println!("{}", identifier);
            return Ok(());
        }

        println!(
            "{} Updated issue: {} {}",
            "+".green(),
            identifier,
            issue["title"].as_str().unwrap_or("")
        );
    } else {
        anyhow::bail!("Failed to update issue");
    }

    Ok(())
}

fn read_json_data(data: Option<&str>) -> Result<Option<Value>> {
    let Some(data) = data else { return Ok(None) };
    let raw = if data == "-" {
        let stdin = io::stdin();
        let lines: Vec<String> = stdin.lock().lines().map_while(Result::ok).collect();
        lines.join("\n")
    } else {
        data.to_string()
    };
    let value: Value = serde_json::from_str(&raw)?;
    Ok(Some(value))
}

async fn delete_issue(id: &str, force: bool, agent_opts: AgentOptions) -> Result<()> {
    if !force && !agent_opts.quiet {
        let confirm = dialoguer::Confirm::new()
            .with_prompt(format!("Delete issue {}? This cannot be undone", id))
            .default(false)
            .interact()?;

        if !confirm {
            println!("Cancelled.");
            return Ok(());
        }
    } else if !force && agent_opts.quiet {
        // In quiet mode without force, require --force
        anyhow::bail!("Use --force to delete in quiet mode");
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
        if !agent_opts.quiet {
            println!("{} Issue deleted", "+".green());
        }
    } else {
        anyhow::bail!("Failed to delete issue");
    }

    Ok(())
}

// Git helper functions for start command

fn branch_exists(branch: &str) -> bool {
    run_git_command(&["rev-parse", "--verify", branch]).is_ok()
}

async fn start_issue(
    id: &str,
    checkout: bool,
    custom_branch: Option<String>,
    agent_opts: AgentOptions,
) -> Result<()> {
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
        let updated_id = updated["identifier"].as_str().unwrap_or("");

        if agent_opts.id_only {
            println!("{}", updated_id);
        } else if !agent_opts.quiet {
            println!(
                "{} Started issue: {} {}",
                "+".green(),
                updated_id.cyan(),
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
        }
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

        if !agent_opts.quiet {
            println!();
        }
        if branch_exists(&branch_name) {
            if !agent_opts.quiet {
                println!("Checking out existing branch: {}", branch_name.green());
            }
            run_git_command(&["checkout", &branch_name])?;
        } else {
            if !agent_opts.quiet {
                println!("Creating and checking out branch: {}", branch_name.green());
            }
            run_git_command(&["checkout", "-b", &branch_name])?;
        }

        let current = run_git_command(&["rev-parse", "--abbrev-ref", "HEAD"])?;
        if !agent_opts.quiet {
            println!("{} Now on branch: {}", "+".green(), current);
        }
    }

    Ok(())
}

async fn stop_issue(id: &str, unassign: bool, agent_opts: AgentOptions) -> Result<()> {
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
        let updated_id = updated["identifier"].as_str().unwrap_or("");

        if agent_opts.id_only {
            println!("{}", updated_id);
        } else if !agent_opts.quiet {
            println!(
                "{} Stopped issue: {} {}",
                "+".green(),
                updated_id.cyan(),
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
        }
    } else {
        anyhow::bail!("Failed to stop issue");
    }

    Ok(())
}
