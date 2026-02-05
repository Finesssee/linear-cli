use anyhow::Result;
use clap::Subcommand;
use dialoguer::{Confirm, Password};
use serde_json::json;

use crate::api::LinearClient;
use crate::config;
use crate::output::{print_json, OutputOptions};

#[derive(Subcommand)]
pub enum AuthCommands {
    /// Store API key for the current profile
    Login {
        /// API key to store (if omitted, prompt interactively)
        #[arg(long, value_name = "KEY")]
        key: Option<String>,
        /// Validate the API key before saving
        #[arg(long)]
        validate: bool,
    },
    /// Remove API key for the current profile
    Logout {
        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },
    /// Show current auth status
    Status {
        /// Validate API access
        #[arg(long)]
        validate: bool,
    },
}

pub async fn handle(cmd: AuthCommands, output: &OutputOptions) -> Result<()> {
    match cmd {
        AuthCommands::Login { key, validate } => login(key, validate, output).await,
        AuthCommands::Logout { force } => logout(force, output).await,
        AuthCommands::Status { validate } => status(validate, output).await,
    }
}

async fn login(key: Option<String>, validate: bool, output: &OutputOptions) -> Result<()> {
    let key = match key {
        Some(key) => key,
        None => Password::new().with_prompt("Linear API key").interact()?,
    };

    if validate {
        validate_key(&key).await?;
    }

    let profile = resolve_profile_for_write()?;
    config::set_workspace_key(&profile, &key)?;

    if output.is_json() || output.has_template() {
        print_json(
            &json!({
                "profile": profile,
                "saved": true
            }),
            output,
        )?;
        return Ok(());
    }

    println!("API key saved for profile '{}'", profile);
    Ok(())
}

async fn logout(force: bool, output: &OutputOptions) -> Result<()> {
    let profile = config::current_profile()?;

    if !force {
        let confirmed = Confirm::new()
            .with_prompt(format!(
                "Remove API key and profile '{}' from config?",
                profile
            ))
            .default(false)
            .interact()?;
        if !confirmed {
            return Ok(());
        }
    }

    config::workspace_remove(&profile)?;

    if output.is_json() || output.has_template() {
        print_json(
            &json!({
                "profile": profile,
                "removed": true
            }),
            output,
        )?;
        return Ok(());
    }

    println!("Removed profile '{}'", profile);
    Ok(())
}

async fn status(validate: bool, output: &OutputOptions) -> Result<()> {
    let config_data = config::load_config()?;
    let profile = config::current_profile().ok();
    let env_key = std::env::var("LINEAR_API_KEY")
        .ok()
        .filter(|k| !k.is_empty());
    let env_profile = std::env::var("LINEAR_CLI_PROFILE")
        .ok()
        .filter(|p| !p.is_empty());

    let configured = profile
        .as_ref()
        .and_then(|p| config_data.workspaces.get(p))
        .map(|w| !w.api_key.is_empty())
        .unwrap_or(false);

    let mut validated = None;
    if validate {
        let key = env_key.clone().or_else(|| {
            profile
                .as_ref()
                .and_then(|p| config_data.workspaces.get(p))
                .map(|w| w.api_key.clone())
        });
        validated = match key {
            Some(key) => Some(validate_key(&key).await.is_ok()),
            None => Some(false),
        };
    }

    if output.is_json() || output.has_template() {
        print_json(
            &json!({
                "profile": profile,
                "configured": configured,
                "env_api_key": env_key.is_some(),
                "env_profile": env_profile,
                "validated": validated,
            }),
            output,
        )?;
        return Ok(());
    }

    println!("Profile: {}", profile.unwrap_or_else(|| "none".to_string()));
    println!("Configured: {}", if configured { "yes" } else { "no" });
    println!(
        "Env API key override: {}",
        if env_key.is_some() { "yes" } else { "no" }
    );
    if let Some(validated) = validated {
        println!("Validated: {}", if validated { "yes" } else { "no" });
    }

    Ok(())
}

async fn validate_key(key: &str) -> Result<()> {
    let client = LinearClient::with_api_key(key.to_string())?;
    let query = r#"
        query {
            viewer {
                id
            }
        }
    "#;
    let result = client.query(query, None).await?;
    let viewer = &result["data"]["viewer"];
    if viewer.is_null() {
        anyhow::bail!("API key validation failed");
    }
    Ok(())
}

fn resolve_profile_for_write() -> Result<String> {
    if let Ok(profile) = std::env::var("LINEAR_CLI_PROFILE") {
        if !profile.trim().is_empty() {
            return Ok(profile);
        }
    }
    let config_data = config::load_config()?;
    Ok(config_data.current.unwrap_or_else(|| "default".to_string()))
}
