use anyhow::Result;
use colored::Colorize;
use console::Term;
use dialoguer::{Confirm, Input, Select};
use serde_json::json;
use tabled::{Table, Tabled};

use crate::api::LinearClient;

#[derive(Debug, Clone)]
struct Team {
    id: String,
    name: String,
    key: String,
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
}

enum MenuAction {
    CreateIssue,
    ListIssues,
    ViewIssue,
    UpdateIssue,
    SwitchTeam,
    Exit,
}

impl std::fmt::Display for MenuAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MenuAction::CreateIssue => write!(f, "Create Issue"),
            MenuAction::ListIssues => write!(f, "List Issues"),
            MenuAction::ViewIssue => write!(f, "View Issue"),
            MenuAction::UpdateIssue => write!(f, "Update Issue"),
            MenuAction::SwitchTeam => write!(f, "Switch Team"),
            MenuAction::Exit => write!(f, "Exit"),
        }
    }
}

pub async fn run() -> Result<()> {
    let term = Term::stdout();
    let client = LinearClient::new()?;

    println!("{}", "Linear CLI - Interactive Mode".cyan().bold());
    println!("{}", "=".repeat(40));

    // Fetch available teams
    let teams = fetch_teams(&client).await?;
    if teams.is_empty() {
        println!("No teams found. Please check your API key.");
        return Ok(());
    }

    // Select initial team
    let mut current_team = select_team(&teams)?;
    println!(
        "\n{} Selected team: {} ({})",
        "+".green(),
        current_team.name,
        current_team.key
    );

    loop {
        println!("\n{}", "-".repeat(40));
        println!(
            "Current team: {} {}",
            current_team.name.cyan(),
            format!("({})", current_team.key).dimmed()
        );

        let actions = vec![
            MenuAction::CreateIssue,
            MenuAction::ListIssues,
            MenuAction::ViewIssue,
            MenuAction::UpdateIssue,
            MenuAction::SwitchTeam,
            MenuAction::Exit,
        ];

        let action_strings: Vec<String> = actions.iter().map(|a| a.to_string()).collect();

        let selection = Select::new()
            .with_prompt("Select action")
            .items(&action_strings)
            .default(0)
            .interact_on(&term)?;

        match actions.get(selection).unwrap() {
            MenuAction::CreateIssue => {
                if let Err(e) = create_issue_interactive(&client, &current_team).await {
                    println!("{} Error: {}", "!".red(), e);
                }
            }
            MenuAction::ListIssues => {
                if let Err(e) = list_issues_interactive(&client, &current_team).await {
                    println!("{} Error: {}", "!".red(), e);
                }
            }
            MenuAction::ViewIssue => {
                if let Err(e) = view_issue_interactive(&client).await {
                    println!("{} Error: {}", "!".red(), e);
                }
            }
            MenuAction::UpdateIssue => {
                if let Err(e) = update_issue_interactive(&client).await {
                    println!("{} Error: {}", "!".red(), e);
                }
            }
            MenuAction::SwitchTeam => {
                current_team = select_team(&teams)?;
                println!(
                    "{} Switched to team: {} ({})",
                    "+".green(),
                    current_team.name,
                    current_team.key
                );
            }
            MenuAction::Exit => {
                println!("Goodbye!");
                break;
            }
        }
    }

    Ok(())
}

async fn fetch_teams(client: &LinearClient) -> Result<Vec<Team>> {
    let query = r#"
        query {
            teams(first: 100) {
                nodes {
                    id
                    name
                    key
                }
            }
        }
    "#;

    let result = client.query(query, None).await?;
    let empty = vec![];
    let teams_json = result["data"]["teams"]["nodes"]
        .as_array()
        .unwrap_or(&empty);

    let teams: Vec<Team> = teams_json
        .iter()
        .map(|t| Team {
            id: t["id"].as_str().unwrap_or("").to_string(),
            name: t["name"].as_str().unwrap_or("").to_string(),
            key: t["key"].as_str().unwrap_or("").to_string(),
        })
        .collect();

    Ok(teams)
}

fn select_team(teams: &[Team]) -> Result<Team> {
    let team_names: Vec<String> = teams
        .iter()
        .map(|t| format!("{} ({})", t.name, t.key))
        .collect();

    let selection = Select::new()
        .with_prompt("Select a team")
        .items(&team_names)
        .default(0)
        .interact()?;

    Ok(teams[selection].clone())
}

async fn create_issue_interactive(client: &LinearClient, team: &Team) -> Result<()> {
    println!("\n{}", "Create New Issue".cyan().bold());

    let title: String = Input::new()
        .with_prompt("Title")
        .interact_text()?;

    if title.trim().is_empty() {
        println!("Title cannot be empty.");
        return Ok(());
    }

    let description: String = Input::new()
        .with_prompt("Description (optional)")
        .allow_empty(true)
        .interact_text()?;

    let priority_options = vec!["None", "Urgent", "High", "Normal", "Low"];
    let priority_selection = Select::new()
        .with_prompt("Priority")
        .items(&priority_options)
        .default(3)
        .interact()?;

    let priority = match priority_selection {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 3,
        4 => 4,
        _ => 0,
    };

    let confirm = Confirm::new()
        .with_prompt("Create this issue?")
        .default(true)
        .interact()?;

    if !confirm {
        println!("Cancelled.");
        return Ok(());
    }

    let mut input = json!({
        "title": title,
        "teamId": team.id
    });

    if !description.is_empty() {
        input["description"] = json!(description);
    }
    if priority > 0 {
        input["priority"] = json!(priority);
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
        let issue_title = issue["title"].as_str().unwrap_or("");
        println!(
            "\n{} Created issue: {} {}",
            "+".green(),
            identifier.cyan(),
            issue_title
        );
        println!("  URL: {}", issue["url"].as_str().unwrap_or(""));
    } else {
        anyhow::bail!("Failed to create issue");
    }

    Ok(())
}

async fn list_issues_interactive(client: &LinearClient, team: &Team) -> Result<()> {
    println!("\n{}", "Issues".cyan().bold());

    let query = r#"
        query($teamId: String!) {
            team(id: $teamId) {
                issues(first: 25) {
                    nodes {
                        id
                        identifier
                        title
                        priority
                        state { name }
                    }
                }
            }
        }
    "#;

    let result = client
        .query(query, Some(json!({ "teamId": team.id })))
        .await?;

    let empty = vec![];
    let issues = result["data"]["team"]["issues"]["nodes"]
        .as_array()
        .unwrap_or(&empty);

    if issues.is_empty() {
        println!("No issues found for team {}.", team.name);
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
        })
        .collect();

    let table = Table::new(rows).to_string();
    println!("{}", table);
    println!("\n{} issues (showing up to 25)", issues.len());

    Ok(())
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

async fn view_issue_interactive(client: &LinearClient) -> Result<()> {
    println!("\n{}", "View Issue".cyan().bold());

    let issue_id: String = Input::new()
        .with_prompt("Issue ID (e.g., LIN-123)")
        .interact_text()?;

    if issue_id.trim().is_empty() {
        println!("Issue ID cannot be empty.");
        return Ok(());
    }

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
                assignee { name email }
                labels { nodes { name } }
            }
        }
    "#;

    let result = client
        .query(query, Some(json!({ "id": issue_id })))
        .await?;
    let issue = &result["data"]["issue"];

    if issue.is_null() {
        println!("Issue not found: {}", issue_id);
        return Ok(());
    }

    let identifier = issue["identifier"].as_str().unwrap_or("");
    let title = issue["title"].as_str().unwrap_or("");
    println!("\n{} {}", identifier.cyan().bold(), title.bold());
    println!("{}", "-".repeat(60));

    if let Some(desc) = issue["description"].as_str() {
        if !desc.is_empty() {
            let truncated = if desc.len() > 200 {
                format!("{}...", &desc[..197])
            } else {
                desc.to_string()
            };
            println!("\n{}\n", truncated);
        }
    }

    println!(
        "State:    {}",
        issue["state"]["name"].as_str().unwrap_or("-")
    );
    println!("Priority: {}", priority_to_string(issue["priority"].as_i64()));
    println!(
        "Team:     {}",
        issue["team"]["name"].as_str().unwrap_or("-")
    );

    if let Some(assignee) = issue["assignee"]["name"].as_str() {
        println!("Assignee: {}", assignee);
    } else {
        println!("Assignee: -");
    }

    let labels = issue["labels"]["nodes"].as_array();
    if let Some(labels) = labels {
        if !labels.is_empty() {
            let label_names: Vec<&str> = labels
                .iter()
                .filter_map(|l| l["name"].as_str())
                .collect();
            println!("Labels:   {}", label_names.join(", "));
        }
    }

    println!("\nURL: {}", issue["url"].as_str().unwrap_or("-"));

    Ok(())
}

async fn update_issue_interactive(client: &LinearClient) -> Result<()> {
    println!("\n{}", "Update Issue".cyan().bold());

    let issue_id: String = Input::new()
        .with_prompt("Issue ID (e.g., LIN-123)")
        .interact_text()?;

    if issue_id.trim().is_empty() {
        println!("Issue ID cannot be empty.");
        return Ok(());
    }

    // First fetch the issue to show current state and get the UUID
    let query = r#"
        query($id: String!) {
            issue(id: $id) {
                id
                identifier
                title
                priority
                state { name }
            }
        }
    "#;

    let result = client
        .query(query, Some(json!({ "id": issue_id })))
        .await?;
    let issue = &result["data"]["issue"];

    if issue.is_null() {
        println!("Issue not found: {}", issue_id);
        return Ok(());
    }

    // Get the UUID for the update mutation
    let issue_uuid = issue["id"].as_str().unwrap_or(&issue_id);
    let current_title = issue["title"].as_str().unwrap_or("");
    let identifier = issue["identifier"].as_str().unwrap_or("");
    println!(
        "\nCurrent: {} {}",
        identifier.cyan(),
        current_title
    );

    let update_options = vec!["Title", "Priority", "Cancel"];
    let selection = Select::new()
        .with_prompt("What to update?")
        .items(&update_options)
        .default(0)
        .interact()?;

    let mut input = json!({});

    match selection {
        0 => {
            // Update title
            let new_title: String = Input::new()
                .with_prompt("New title")
                .with_initial_text(current_title)
                .interact_text()?;

            if !new_title.is_empty() && new_title != current_title {
                input["title"] = json!(new_title);
            }
        }
        1 => {
            // Update priority
            let priority_options = vec!["None", "Urgent", "High", "Normal", "Low"];
            let current_priority = issue["priority"].as_i64().unwrap_or(0) as usize;
            let priority_selection = Select::new()
                .with_prompt("New priority")
                .items(&priority_options)
                .default(current_priority)
                .interact()?;

            input["priority"] = json!(priority_selection);
        }
        _ => {
            println!("Cancelled.");
            return Ok(());
        }
    }

    if input.as_object().map(|o| o.is_empty()).unwrap_or(true) {
        println!("No changes made.");
        return Ok(());
    }

    let confirm = Confirm::new()
        .with_prompt("Apply changes?")
        .default(true)
        .interact()?;

    if !confirm {
        println!("Cancelled.");
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

    // Use the UUID for the update mutation
    let result = client
        .mutate(mutation, Some(json!({ "id": issue_uuid, "input": input })))
        .await?;

    if result["data"]["issueUpdate"]["success"].as_bool() == Some(true) {
        let updated = &result["data"]["issueUpdate"]["issue"];
        println!(
            "\n{} Updated issue: {} {}",
            "+".green(),
            updated["identifier"].as_str().unwrap_or(""),
            updated["title"].as_str().unwrap_or("")
        );
    } else {
        anyhow::bail!("Failed to update issue");
    }

    Ok(())
}
