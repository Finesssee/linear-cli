use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::json;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::api::{resolve_team_id, LinearClient};

/// Get default directory to scan for local projects (cross-platform)
fn get_default_code_dir() -> String {
    dirs::home_dir()
        .map(|p| p.join("code").to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string())
}

#[derive(Subcommand)]
pub enum SyncCommands {
    /// Compare local folders with Linear projects
    #[command(after_help = r#"EXAMPLES:
    linear sync status                         # Compare ~/code with Linear
    linear sy status -d /path/to/code          # Custom directory
    linear sy status --missing-only            # Show only missing projects"#)]
    Status {
        /// Directory to scan for local projects (default: ~/code)
        #[arg(short, long)]
        directory: Option<String>,
        /// Show only missing projects (not in Linear)
        #[arg(short, long)]
        missing_only: bool,
    },
    /// Create Linear projects from local folders that don't exist in Linear
    #[command(after_help = r#"EXAMPLES:
    linear sync push -t ENG                    # Create projects for all folders
    linear sy push -t ENG --dry-run            # Preview without creating
    linear sy push -t ENG -o proj1,proj2       # Only specific folders"#)]
    Push {
        /// Directory to scan for local projects (default: ~/code)
        #[arg(short, long)]
        directory: Option<String>,
        /// Team name or ID to create projects in
        #[arg(short, long)]
        team: String,
        /// Only push specific folders (comma-separated)
        #[arg(short, long)]
        only: Option<String>,
        /// Dry run - show what would be created without creating
        #[arg(long)]
        dry_run: bool,
    },
}

/// Represents a local folder that could be a project
#[derive(Debug, Clone)]
struct LocalProject {
    name: String,
    path: String,
    has_git: bool,
}

/// Represents a Linear project for comparison
#[derive(Debug, Clone)]
struct LinearProject {
    #[allow(dead_code)]
    id: String,
    name: String,
    #[allow(dead_code)]
    url: Option<String>,
}

/// Sync status for a project
#[derive(Debug)]
enum SyncStatus {
    /// Exists in both local and Linear
    Synced {
        local: LocalProject,
        remote: LinearProject,
    },
    /// Exists locally but not in Linear
    LocalOnly(LocalProject),
    /// Exists in Linear but not locally
    RemoteOnly(LinearProject),
}

pub async fn handle(cmd: SyncCommands) -> Result<()> {
    match cmd {
        SyncCommands::Status {
            directory,
            missing_only,
        } => status_command(directory, missing_only).await,
        SyncCommands::Push {
            directory,
            team,
            only,
            dry_run,
        } => push_command(directory, team, only, dry_run).await,
    }
}

/// Scan a directory for local project folders
fn scan_local_projects(dir: &str) -> Result<Vec<LocalProject>> {
    let path = Path::new(dir);

    if !path.exists() {
        anyhow::bail!("Directory does not exist: {}", dir);
    }

    if !path.is_dir() {
        anyhow::bail!("Path is not a directory: {}", dir);
    }

    let mut projects = Vec::new();

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();

        // Only consider directories
        if !entry_path.is_dir() {
            continue;
        }

        // Skip hidden directories
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }

        // Check if it has a .git folder
        let git_path = entry_path.join(".git");
        let has_git = git_path.exists();

        projects.push(LocalProject {
            name: name.clone(),
            path: entry_path.to_string_lossy().to_string(),
            has_git,
        });
    }

    // Sort alphabetically
    projects.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    Ok(projects)
}

/// Fetch all Linear projects
async fn fetch_linear_projects(client: &LinearClient) -> Result<Vec<LinearProject>> {
    let query = r#"
        query {
            projects(first: 250) {
                nodes {
                    id
                    name
                    url
                }
            }
        }
    "#;

    let result = client.query(query, None).await?;

    let projects = result["data"]["projects"]["nodes"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|p| LinearProject {
            id: p["id"].as_str().unwrap_or("").to_string(),
            name: p["name"].as_str().unwrap_or("").to_string(),
            url: p["url"].as_str().map(|s| s.to_string()),
        })
        .collect();

    Ok(projects)
}

/// Compare local projects with Linear projects
fn compare_projects(local: Vec<LocalProject>, remote: Vec<LinearProject>) -> Vec<SyncStatus> {
    let mut statuses = Vec::new();

    // Create a set of remote project names (case-insensitive)
    let _remote_names: HashSet<String> = remote.iter().map(|p| p.name.to_lowercase()).collect();

    // Create a map of local names for quick lookup
    let local_names: HashSet<String> = local.iter().map(|p| p.name.to_lowercase()).collect();

    // Check local projects
    for local_proj in &local {
        let name_lower = local_proj.name.to_lowercase();

        if let Some(remote_proj) = remote.iter().find(|r| r.name.to_lowercase() == name_lower) {
            statuses.push(SyncStatus::Synced {
                local: local_proj.clone(),
                remote: remote_proj.clone(),
            });
        } else {
            statuses.push(SyncStatus::LocalOnly(local_proj.clone()));
        }
    }

    // Check for remote-only projects
    for remote_proj in &remote {
        let name_lower = remote_proj.name.to_lowercase();
        if !local_names.contains(&name_lower) {
            statuses.push(SyncStatus::RemoteOnly(remote_proj.clone()));
        }
    }

    statuses
}

/// Display sync status
async fn status_command(directory: Option<String>, missing_only: bool) -> Result<()> {
    let dir = directory.unwrap_or_else(get_default_code_dir);

    println!("{}", "Sync Status".bold());
    println!("{}", "─".repeat(60));
    println!("Scanning: {}", dir.cyan());
    println!();

    // Scan local projects
    let local_projects = scan_local_projects(&dir)?;
    println!("Found {} local folders", local_projects.len());

    // Fetch Linear projects
    let client = LinearClient::new()?;
    let linear_projects = fetch_linear_projects(&client).await?;
    println!("Found {} Linear projects", linear_projects.len());
    println!();

    // Compare
    let statuses = compare_projects(local_projects, linear_projects);

    // Count stats
    let synced_count = statuses
        .iter()
        .filter(|s| matches!(s, SyncStatus::Synced { .. }))
        .count();
    let local_only_count = statuses
        .iter()
        .filter(|s| matches!(s, SyncStatus::LocalOnly(_)))
        .count();
    let remote_only_count = statuses
        .iter()
        .filter(|s| matches!(s, SyncStatus::RemoteOnly(_)))
        .count();

    // Display results
    if !missing_only {
        // Show synced projects
        let synced: Vec<_> = statuses
            .iter()
            .filter_map(|s| match s {
                SyncStatus::Synced { local, remote } => Some((local, remote)),
                _ => None,
            })
            .collect();

        if !synced.is_empty() {
            println!("{} {} Synced:", "[OK]".green(), synced.len());
            for (local, _remote) in synced {
                let git_indicator = if local.has_git {
                    "[git]".dimmed()
                } else {
                    "".dimmed()
                };
                println!("  {} {} {}", "+".green(), local.name, git_indicator);
            }
            println!();
        }
    }

    // Show local-only projects (missing from Linear)
    let local_only: Vec<_> = statuses
        .iter()
        .filter_map(|s| match s {
            SyncStatus::LocalOnly(local) => Some(local),
            _ => None,
        })
        .collect();

    if !local_only.is_empty() {
        println!(
            "{} {} Local only (not in Linear):",
            "[MISSING]".yellow(),
            local_only.len()
        );
        for local in local_only {
            let git_indicator = if local.has_git {
                "[git]".dimmed()
            } else {
                "".dimmed()
            };
            println!(
                "  {} {} {}",
                "!".yellow(),
                local.name.yellow(),
                git_indicator
            );
        }
        println!();
    }

    // Show remote-only projects (not found locally)
    if !missing_only {
        let remote_only: Vec<_> = statuses
            .iter()
            .filter_map(|s| match s {
                SyncStatus::RemoteOnly(remote) => Some(remote),
                _ => None,
            })
            .collect();

        if !remote_only.is_empty() {
            println!(
                "{} {} Linear only (not found locally):",
                "[REMOTE]".blue(),
                remote_only.len()
            );
            for remote in remote_only {
                println!("  {} {}", "-".blue(), remote.name.blue());
            }
            println!();
        }
    }

    // Summary
    println!("{}", "─".repeat(60));
    println!(
        "Summary: {} synced, {} local only, {} remote only",
        synced_count.to_string().green(),
        local_only_count.to_string().yellow(),
        remote_only_count.to_string().blue()
    );

    if local_only_count > 0 {
        println!();
        println!(
            "{} Run {} to create Linear projects for local folders",
            "Tip:".dimmed(),
            "linear sync push --team <TEAM>".cyan()
        );
    }

    Ok(())
}

/// Push local projects to Linear
async fn push_command(
    directory: Option<String>,
    team: String,
    only: Option<String>,
    dry_run: bool,
) -> Result<()> {
    let dir = directory.unwrap_or_else(get_default_code_dir);

    // Create client early to resolve team ID
    let client = LinearClient::new()?;

    // Resolve team key/name to UUID
    let team_id = resolve_team_id(&client, &team).await?;

    if dry_run {
        println!(
            "{}",
            "[DRY RUN] No projects will be created".yellow().bold()
        );
        println!();
    }

    println!("{}", "Push to Linear".bold());
    println!("{}", "─".repeat(60));
    println!("Source: {}", dir.cyan());
    println!("Team: {}", team.cyan());
    println!();

    // Scan local projects
    let local_projects = scan_local_projects(&dir)?;

    // Fetch Linear projects
    let linear_projects = fetch_linear_projects(&client).await?;

    // Compare to find missing
    let statuses = compare_projects(local_projects, linear_projects);

    // Get local-only projects
    let mut to_create: Vec<&LocalProject> = statuses
        .iter()
        .filter_map(|s| match s {
            SyncStatus::LocalOnly(local) => Some(local),
            _ => None,
        })
        .collect();

    // Filter by --only if specified
    if let Some(only_list) = &only {
        let only_names: HashSet<String> = only_list
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .collect();

        to_create.retain(|p| only_names.contains(&p.name.to_lowercase()));
    }

    if to_create.is_empty() {
        println!(
            "{} All local projects already exist in Linear",
            "[OK]".green()
        );
        return Ok(());
    }

    println!("Projects to create: {}", to_create.len());
    println!();

    let mut created_count = 0;
    let mut failed_count = 0;

    for project in to_create {
        print!("  {} {} ... ", ">".cyan(), project.name);

        if dry_run {
            println!("{}", "[would create]".yellow());
            created_count += 1;
            continue;
        }

        match create_linear_project(&client, &project.name, &team_id, &project.path).await {
            Ok(url) => {
                println!("{}", "[created]".green());
                if let Some(url) = url {
                    println!("    {}", url.dimmed());
                }
                created_count += 1;
            }
            Err(e) => {
                println!("{} {}", "[failed]".red(), e.to_string().red());
                failed_count += 1;
            }
        }
    }

    println!();
    println!("{}", "─".repeat(60));

    if dry_run {
        println!(
            "Would create {} projects",
            created_count.to_string().green()
        );
    } else {
        println!(
            "Created: {}, Failed: {}",
            created_count.to_string().green(),
            failed_count.to_string().red()
        );
    }

    Ok(())
}

/// Create a single Linear project
async fn create_linear_project(
    client: &LinearClient,
    name: &str,
    team: &str,
    local_path: &str,
) -> Result<Option<String>> {
    let description = format!("Local project synced from: {}", local_path);

    let input = json!({
        "name": name,
        "teamIds": [team],
        "description": description
    });

    let mutation = r#"
        mutation($input: ProjectCreateInput!) {
            projectCreate(input: $input) {
                success
                project {
                    id
                    name
                    url
                }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "input": input })))
        .await?;

    if result["data"]["projectCreate"]["success"].as_bool() == Some(true) {
        let url = result["data"]["projectCreate"]["project"]["url"]
            .as_str()
            .map(|s| s.to_string());
        Ok(url)
    } else {
        let errors = &result["errors"];
        if !errors.is_null() {
            anyhow::bail!("API error: {}", errors);
        }
        anyhow::bail!("Failed to create project");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_projects_synced() {
        let local = vec![LocalProject {
            name: "my-project".to_string(),
            path: "/code/my-project".to_string(),
            has_git: true,
        }];

        let remote = vec![LinearProject {
            id: "123".to_string(),
            name: "my-project".to_string(),
            url: Some("https://linear.app/...".to_string()),
        }];

        let statuses = compare_projects(local, remote);
        assert_eq!(statuses.len(), 1);
        assert!(matches!(statuses[0], SyncStatus::Synced { .. }));
    }

    #[test]
    fn test_compare_projects_local_only() {
        let local = vec![LocalProject {
            name: "new-project".to_string(),
            path: "/code/new-project".to_string(),
            has_git: false,
        }];

        let remote = vec![];

        let statuses = compare_projects(local, remote);
        assert_eq!(statuses.len(), 1);
        assert!(matches!(statuses[0], SyncStatus::LocalOnly(_)));
    }

    #[test]
    fn test_compare_projects_remote_only() {
        let local = vec![];

        let remote = vec![LinearProject {
            id: "456".to_string(),
            name: "archived-project".to_string(),
            url: None,
        }];

        let statuses = compare_projects(local, remote);
        assert_eq!(statuses.len(), 1);
        assert!(matches!(statuses[0], SyncStatus::RemoteOnly(_)));
    }

    #[test]
    fn test_compare_projects_case_insensitive() {
        let local = vec![LocalProject {
            name: "MyProject".to_string(),
            path: "/code/MyProject".to_string(),
            has_git: true,
        }];

        let remote = vec![LinearProject {
            id: "789".to_string(),
            name: "myproject".to_string(),
            url: None,
        }];

        let statuses = compare_projects(local, remote);
        // Should match case-insensitively
        let synced = statuses
            .iter()
            .filter(|s| matches!(s, SyncStatus::Synced { .. }))
            .count();
        assert_eq!(synced, 1);
    }
}
