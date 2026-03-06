use anyhow::Result;
use std::process::Command;

pub fn run_git_command(args: &[&str]) -> Result<String> {
    let output = Command::new("git").args(args).output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Git command failed: {}", stderr.trim());
    }
}

pub fn validate_branch_name(branch: &str) -> Result<()> {
    if branch.trim().is_empty() {
        anyhow::bail!("Branch name cannot be empty");
    }
    if branch.starts_with('-') {
        anyhow::bail!("Branch name cannot start with '-'");
    }
    if branch == "@" || branch.contains("@{") {
        anyhow::bail!("Branch name contains invalid ref syntax");
    }

    let output = Command::new("git")
        .args(["check-ref-format", "--branch", branch])
        .output()?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Invalid branch name '{}': {}", branch, stderr.trim());
    }
}

pub fn git_branch_exists(branch: &str) -> bool {
    if validate_branch_name(branch).is_err() {
        return false;
    }

    let ref_name = format!("refs/heads/{}", branch);
    Command::new("git")
        .args(["show-ref", "--verify", "--quiet", &ref_name])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn generate_branch_name(identifier: &str, title: &str) -> String {
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
