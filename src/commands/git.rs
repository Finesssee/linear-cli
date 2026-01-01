use anyhow::Result;
use clap::{Subcommand, ValueEnum};
use colored::Colorize;
use serde_json::json;
use std::path::Path;
use std::process::Command;

use crate::api::LinearClient;

/// Version control system type
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Vcs {
    /// Git version control
    Git,
    /// Jujutsu (jj) version control
    Jj,
}

impl std::fmt::Display for Vcs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Vcs::Git => write!(f, "git"),
            Vcs::Jj => write!(f, "jj"),
        }
    }
}

#[derive(Subcommand)]
pub enum GitCommands {
    /// Checkout a branch for an issue (creates if doesn't exist)
    Checkout {
        /// Issue identifier (e.g., "LIN-123") or ID
        issue: String,
        /// Custom branch name (optional, uses issue's branch name by default)
        #[arg(short, long)]
        branch: Option<String>,
        /// Version control system to use (auto-detected by default)
        #[arg(long, value_enum)]
        vcs: Option<Vcs>,
    },
    /// Show the branch name for an issue
    Branch {
        /// Issue identifier (e.g., "LIN-123") or ID
        issue: String,
        /// Version control system to use (auto-detected by default)
        #[arg(long, value_enum)]
        vcs: Option<Vcs>,
    },
    /// Create a branch for an issue without checking out
    Create {
        /// Issue identifier (e.g., "LIN-123") or ID
        issue: String,
        /// Custom branch name (optional)
        #[arg(short, long)]
        branch: Option<String>,
        /// Version control system to use (auto-detected by default)
        #[arg(long, value_enum)]
        vcs: Option<Vcs>,
    },
    /// Show commits with Linear issue trailers (jj only)
    Commits {
        /// Number of commits to show
        #[arg(short, long, default_value = "10")]
        limit: usize,
        /// Version control system to use (auto-detected by default)
        #[arg(long, value_enum)]
        vcs: Option<Vcs>,
    },
    /// Create a GitHub PR from a Linear issue
    Pr {
        /// Issue identifier (e.g., "LIN-123") or ID
        issue: String,
        /// Base branch to merge into (default: main)
        #[arg(short = 'B', long, default_value = "main")]
        base: String,
        /// Create a draft PR
        #[arg(short, long)]
        draft: bool,
        /// Open the PR in the browser after creation
        #[arg(short, long)]
        web: bool,
    },
}

/// Detect which VCS is being used in the current directory
fn detect_vcs() -> Result<Vcs> {
    // First check for .jj directory
    if Path::new(".jj").exists() {
        return Ok(Vcs::Jj);
    }

    // Try running jj status to see if we're in a jj repo
    if let Ok(output) = Command::new("jj").args(["status"]).output() {
        if output.status.success() {
            return Ok(Vcs::Jj);
        }
    }

    // Check for .git directory
    if Path::new(".git").exists() {
        return Ok(Vcs::Git);
    }

    // Try running git status
    if let Ok(output) = Command::new("git").args(["status"]).output() {
        if output.status.success() {
            return Ok(Vcs::Git);
        }
    }

    anyhow::bail!("Not in a git or jj repository")
}

/// Get the VCS to use, either from the flag or auto-detected
fn get_vcs(vcs_flag: Option<Vcs>) -> Result<Vcs> {
    match vcs_flag {
        Some(vcs) => Ok(vcs),
        None => detect_vcs(),
    }
}

pub async fn handle(cmd: GitCommands) -> Result<()> {
    match cmd {
        GitCommands::Checkout { issue, branch, vcs } => {
            let vcs = get_vcs(vcs)?;
            checkout_issue(&issue, branch, vcs).await
        }
        GitCommands::Branch { issue, vcs } => {
            let vcs = get_vcs(vcs)?;
            show_branch(&issue, vcs).await
        }
        GitCommands::Create { issue, branch, vcs } => {
            let vcs = get_vcs(vcs)?;
            create_branch(&issue, branch, vcs).await
        }
        GitCommands::Commits { limit, vcs } => {
            let vcs = get_vcs(vcs)?;
            show_commits(limit, vcs).await
        }
        GitCommands::Pr {
            issue,
            base,
            draft,
            web,
        } => create_pr(&issue, &base, draft, web).await,
    }
}

async fn get_issue_info(issue_id: &str) -> Result<(String, String, String, String)> {
    let client = LinearClient::new()?;

    let query = r#"
        query($id: String!) {
            issue(id: $id) {
                id
                identifier
                title
                branchName
                url
            }
        }
    "#;

    let result = client.query(query, Some(json!({ "id": issue_id }))).await?;
    let issue = &result["data"]["issue"];

    if issue.is_null() {
        anyhow::bail!("Issue not found: {}", issue_id);
    }

    let identifier = issue["identifier"].as_str().unwrap_or("").to_string();
    let title = issue["title"].as_str().unwrap_or("").to_string();
    let branch_name = issue["branchName"].as_str().unwrap_or("").to_string();
    let url = issue["url"].as_str().unwrap_or("").to_string();

    Ok((identifier, title, branch_name, url))
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

fn run_git_command(args: &[&str]) -> Result<String> {
    let output = Command::new("git").args(args).output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Git command failed: {}", stderr.trim());
    }
}

fn run_jj_command(args: &[&str]) -> Result<String> {
    let output = Command::new("jj").args(args).output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Jujutsu command failed: {}", stderr.trim());
    }
}

fn branch_exists(branch: &str, vcs: Vcs) -> bool {
    match vcs {
        Vcs::Git => run_git_command(&["rev-parse", "--verify", branch]).is_ok(),
        Vcs::Jj => {
            // In jj, check if bookmark exists
            run_jj_command(&["bookmark", "list", branch])
                .is_ok_and(|output| output.lines().any(|line| line.starts_with(branch)))
        }
    }
}

fn get_current_branch(vcs: Vcs) -> Result<String> {
    match vcs {
        Vcs::Git => run_git_command(&["rev-parse", "--abbrev-ref", "HEAD"]),
        Vcs::Jj => {
            // Get the current change description or ID
            run_jj_command(&["log", "-r", "@", "--no-graph", "-T", "change_id.short()"])
        }
    }
}

/// Generate the commit description with Linear issue trailer
fn generate_jj_description(identifier: &str, title: &str, url: &str) -> String {
    format!(
        "{}: {}\n\nLinear-Issue: {}\nLinear-URL: {}",
        identifier, title, identifier, url
    )
}

async fn checkout_issue(issue_id: &str, custom_branch: Option<String>, vcs: Vcs) -> Result<()> {
    let (identifier, title, linear_branch, url) = get_issue_info(issue_id).await?;

    let branch_name = custom_branch
        .or(if linear_branch.is_empty() {
            None
        } else {
            Some(linear_branch)
        })
        .unwrap_or_else(|| generate_branch_name(&identifier, &title));

    println!(
        "{} {} {}",
        identifier.cyan(),
        title.dimmed(),
        format!("({})", vcs).dimmed()
    );

    match vcs {
        Vcs::Git => {
            if branch_exists(&branch_name, vcs) {
                // Branch exists, just checkout
                println!("Checking out existing branch: {}", branch_name.green());
                run_git_command(&["checkout", &branch_name])?;
            } else {
                // Create and checkout new branch
                println!("Creating and checking out branch: {}", branch_name.green());
                run_git_command(&["checkout", "-b", &branch_name])?;
            }

            let current = get_current_branch(vcs)?;
            println!("{} Now on branch: {}", "✓".green(), current);
        }
        Vcs::Jj => {
            // For jj, we create a new change with the issue info in the description
            let description = generate_jj_description(&identifier, &title, &url);

            if branch_exists(&branch_name, vcs) {
                // Bookmark exists, switch to it
                println!("Switching to existing bookmark: {}", branch_name.green());
                run_jj_command(&["edit", &branch_name])?;
            } else {
                // Create a new change with description
                println!("Creating new change for issue: {}", identifier.green());
                run_jj_command(&["new", "-m", &description])?;

                // Create a bookmark for the branch name
                println!("Creating bookmark: {}", branch_name.green());
                run_jj_command(&["bookmark", "create", &branch_name])?;
            }

            let current = get_current_branch(vcs)?;
            println!("{} Now on change: {}", "✓".green(), current);
        }
    }

    Ok(())
}

async fn show_branch(issue_id: &str, vcs: Vcs) -> Result<()> {
    let (identifier, title, linear_branch, url) = get_issue_info(issue_id).await?;

    println!(
        "{} {} {}",
        identifier.cyan().bold(),
        title,
        format!("({})", vcs).dimmed()
    );
    println!("{}", "-".repeat(50));

    if !linear_branch.is_empty() {
        println!("Linear branch: {}", linear_branch.green());
    }

    let generated = generate_branch_name(&identifier, &title);
    println!("Generated:     {}", generated.yellow());
    println!("Issue URL:     {}", url.blue());

    match vcs {
        Vcs::Git => {
            // Check if either branch exists locally
            if branch_exists(&linear_branch, vcs) {
                println!("\n{} Linear branch exists locally", "✓".green());
            } else if branch_exists(&generated, vcs) {
                println!("\n{} Generated branch exists locally", "✓".green());
            } else {
                println!("\n{} No local branch found for this issue", "!".yellow());
            }
        }
        Vcs::Jj => {
            // Check if bookmark exists
            if branch_exists(&linear_branch, vcs) {
                println!("\n{} Linear bookmark exists", "✓".green());
            } else if branch_exists(&generated, vcs) {
                println!("\n{} Generated bookmark exists", "✓".green());
            } else {
                println!("\n{} No bookmark found for this issue", "!".yellow());
            }
        }
    }

    Ok(())
}

async fn create_branch(issue_id: &str, custom_branch: Option<String>, vcs: Vcs) -> Result<()> {
    let (identifier, title, linear_branch, url) = get_issue_info(issue_id).await?;

    let branch_name = custom_branch
        .or(if linear_branch.is_empty() {
            None
        } else {
            Some(linear_branch)
        })
        .unwrap_or_else(|| generate_branch_name(&identifier, &title));

    println!(
        "{} {} {}",
        identifier.cyan(),
        title.dimmed(),
        format!("({})", vcs).dimmed()
    );

    match vcs {
        Vcs::Git => {
            if branch_exists(&branch_name, vcs) {
                println!("{} Branch already exists: {}", "!".yellow(), branch_name);
                return Ok(());
            }

            // Create branch without checking out
            run_git_command(&["branch", &branch_name])?;
            println!("{} Created branch: {}", "✓".green(), branch_name);
        }
        Vcs::Jj => {
            if branch_exists(&branch_name, vcs) {
                println!("{} Bookmark already exists: {}", "!".yellow(), branch_name);
                return Ok(());
            }

            // Create a new change with description and bookmark
            let description = generate_jj_description(&identifier, &title, &url);
            run_jj_command(&["new", "-m", &description])?;
            run_jj_command(&["bookmark", "create", &branch_name])?;

            // Go back to original change
            run_jj_command(&["prev"])?;

            println!("{} Created bookmark: {}", "✓".green(), branch_name);
        }
    }

    Ok(())
}

async fn show_commits(limit: usize, vcs: Vcs) -> Result<()> {
    match vcs {
        Vcs::Git => {
            println!(
                "{}",
                "The 'commits' subcommand is designed for jj. For git, use 'git log'.".yellow()
            );
            println!("Tip: Use --vcs jj to explicitly use jj commands.");
            Ok(())
        }
        Vcs::Jj => {
            println!("{}", "Commits with Linear issue trailers:".cyan().bold());
            println!("{}", "-".repeat(50));

            // Get commits with their descriptions
            let limit_str = limit.to_string();
            let output = run_jj_command(&[
                "log",
                "-r",
                &format!("ancestors(@, {})", limit_str),
                "--no-graph",
                "-T",
                r#"change_id.short() ++ " " ++ description.first_line() ++ "\n""#,
            ])?;

            // Parse and display commits, highlighting those with Linear trailers
            for line in output.lines() {
                if line.trim().is_empty() {
                    continue;
                }

                let parts: Vec<&str> = line.splitn(2, ' ').collect();
                let (change_id, description) = if parts.len() == 2 {
                    (parts[0], parts[1])
                } else {
                    (parts[0], "")
                };

                // Check if this commit has Linear trailers
                let full_desc =
                    run_jj_command(&["log", "-r", change_id, "--no-graph", "-T", "description"])?;

                let has_linear_trailer =
                    full_desc.contains("Linear-Issue:") || full_desc.contains("Linear-URL:");

                if has_linear_trailer {
                    // Extract the Linear issue ID
                    let issue_id = full_desc
                        .lines()
                        .find(|l| l.starts_with("Linear-Issue:"))
                        .and_then(|l| l.strip_prefix("Linear-Issue:"))
                        .map(|s| s.trim())
                        .unwrap_or("");

                    println!(
                        "{} {} {}",
                        change_id.yellow(),
                        description,
                        format!("[{}]", issue_id).cyan()
                    );
                } else {
                    println!("{} {}", change_id.dimmed(), description);
                }
            }

            Ok(())
        }
    }
}

fn run_gh_command(args: &[&str]) -> Result<String> {
    let output = Command::new("gh").args(args).output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("gh command failed: {}", stderr.trim());
    }
}

async fn create_pr(issue_id: &str, base: &str, draft: bool, web: bool) -> Result<()> {
    let (identifier, title, _branch_name, url) = get_issue_info(issue_id).await?;

    let pr_title = format!("[{}] {}", identifier, title);
    let pr_body = format!("Linear: {}", url);

    println!("{} {}", identifier.cyan(), title.dimmed());
    println!("Creating PR with title: {}", pr_title.green());

    let mut args = vec![
        "pr", "create", "--title", &pr_title, "--body", &pr_body, "--base", base,
    ];

    if draft {
        args.push("--draft");
    }

    if web {
        args.push("--web");
    }

    let result = run_gh_command(&args)?;

    if !result.is_empty() {
        println!("{} PR created: {}", "✓".green(), result);
    } else {
        println!("{} PR created successfully!", "✓".green());
    }

    Ok(())
}
