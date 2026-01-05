use anyhow::Result;
use clap::Subcommand;
use dialoguer::Password;

use crate::config;

#[derive(Subcommand)]
pub enum WorkspaceCommands {
    /// Add a new workspace with API key
    Add {
        /// Name for the workspace
        name: String,
        /// API key (will prompt if not provided)
        #[arg(short, long)]
        key: Option<String>,
    },
    /// List all configured workspaces
    #[command(alias = "ls")]
    List,
    /// Switch to a different workspace
    Switch {
        /// Name of the workspace to switch to
        name: String,
    },
    /// Show current workspace
    Current,
    /// Remove a workspace
    #[command(alias = "rm")]
    Remove {
        /// Name of the workspace to remove
        name: String,
    },
}

pub async fn handle(cmd: WorkspaceCommands) -> Result<()> {
    match cmd {
        WorkspaceCommands::Add { name, key } => add_workspace(&name, key).await,
        WorkspaceCommands::List => list_workspaces(),
        WorkspaceCommands::Switch { name } => switch_workspace(&name),
        WorkspaceCommands::Current => current_workspace(),
        WorkspaceCommands::Remove { name } => remove_workspace(&name),
    }
}

async fn add_workspace(name: &str, key: Option<String>) -> Result<()> {
    let api_key = match key {
        Some(k) => k,
        None => Password::new()
            .with_prompt(format!("Enter API key for workspace '{}'", name))
            .interact()?,
    };

    if api_key.is_empty() {
        anyhow::bail!("API key cannot be empty");
    }

    config::workspace_add(name, &api_key)
}

fn list_workspaces() -> Result<()> {
    config::workspace_list()
}

fn switch_workspace(name: &str) -> Result<()> {
    config::workspace_switch(name)
}

fn current_workspace() -> Result<()> {
    config::workspace_current()
}

fn remove_workspace(name: &str) -> Result<()> {
    config::workspace_remove(name)
}
