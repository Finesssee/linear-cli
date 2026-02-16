use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[cfg(unix)]
use std::io::Write;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Workspace {
    pub api_key: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    pub current: Option<String>,
    #[serde(default)]
    pub workspaces: HashMap<String, Workspace>,
    // Legacy field for backward compatibility
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

fn config_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .context("Could not find config directory")?
        .join("linear-cli");

    fs::create_dir_all(&config_dir)?;
    Ok(config_dir.join("config.toml"))
}

pub fn load_config() -> Result<Config> {
    let path = config_path()?;
    if path.exists() {
        let content = fs::read_to_string(&path)?;
        let mut config: Config = toml::from_str(&content)?;

        // Migrate legacy api_key to workspaces if needed
        if let Some(legacy_key) = config.api_key.take() {
            if !config.workspaces.contains_key("default") {
                config.workspaces.insert(
                    "default".to_string(),
                    Workspace {
                        api_key: legacy_key,
                    },
                );
                if config.current.is_none() {
                    config.current = Some("default".to_string());
                }
                // Save migrated config
                save_config(&config)?;
            }
        }

        Ok(config)
    } else {
        Ok(Config::default())
    }
}

pub fn save_config(config: &Config) -> Result<()> {
    let path = config_path()?;
    let content = toml::to_string_pretty(config)?;

    // Use secure file permissions on Unix (0600 = owner read/write only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)?;
        file.write_all(content.as_bytes())?;
    }

    #[cfg(not(unix))]
    {
        fs::write(&path, content)?;
    }

    Ok(())
}

pub fn set_api_key(key: &str) -> Result<()> {
    let mut config = load_config()?;
    let profile = std::env::var("LINEAR_CLI_PROFILE")
        .ok()
        .filter(|p| !p.is_empty());
    let workspace_name = profile
        .or_else(|| config.current.clone())
        .unwrap_or_else(|| "default".to_string());
    config.workspaces.insert(
        workspace_name.clone(),
        Workspace {
            api_key: key.to_string(),
        },
    );
    if config.current.is_none() {
        config.current = Some(workspace_name.clone());
    }
    save_config(&config)?;
    Ok(())
}

pub fn get_api_key() -> Result<String> {
    // Check for LINEAR_API_KEY environment variable first
    if let Ok(api_key) = std::env::var("LINEAR_API_KEY") {
        if !api_key.is_empty() {
            return Ok(api_key);
        }
    }

    // Try keyring if feature is enabled
    #[cfg(feature = "secure-storage")]
    {
        let config = load_config()?;
        let profile = std::env::var("LINEAR_CLI_PROFILE")
            .ok()
            .filter(|p| !p.is_empty())
            .or(config.current.clone())
            .unwrap_or_else(|| "default".to_string());

        if let Ok(Some(key)) = crate::keyring::get_key(&profile) {
            return Ok(key);
        }
    }

    // Fall back to config file
    let config = load_config()?;
    let profile = std::env::var("LINEAR_CLI_PROFILE")
        .ok()
        .filter(|p| !p.is_empty());
    let current = profile.or(config.current.clone()).context(
        "No workspace selected. Run: linear config workspace-add <name> or set LINEAR_CLI_PROFILE",
    )?;
    let workspace = config.workspaces.get(&current).context(format!(
        "Workspace '{}' not found. Run: linear config workspace-add <name>",
        current
    ))?;
    Ok(workspace.api_key.clone())
}

pub fn config_file_path() -> Result<PathBuf> {
    config_path()
}

pub fn current_profile() -> Result<String> {
    let config = load_config()?;
    let profile = std::env::var("LINEAR_CLI_PROFILE")
        .ok()
        .filter(|p| !p.is_empty());
    profile.or(config.current).context("No workspace selected")
}

pub fn set_workspace_key(name: &str, api_key: &str) -> Result<()> {
    let mut config = load_config()?;
    config.workspaces.insert(
        name.to_string(),
        Workspace {
            api_key: api_key.to_string(),
        },
    );
    if config.current.is_none() {
        config.current = Some(name.to_string());
    }
    save_config(&config)?;
    Ok(())
}

pub fn config_get(key: &str, raw: bool) -> Result<()> {
    match key.to_lowercase().as_str() {
        "api-key" | "api_key" => {
            let api_key = get_api_key()?;
            if raw || api_key.len() <= 12 {
                println!("{}", api_key);
            } else {
                let masked = format!("{}...{}", &api_key[..8], &api_key[api_key.len() - 4..]);
                println!("{}", masked);
            }
        }
        "profile" => {
            let profile = current_profile()?;
            println!("{}", profile);
        }
        _ => anyhow::bail!("Unknown config key: {}", key),
    }
    Ok(())
}

pub fn config_set(key: &str, value: &str) -> Result<()> {
    match key.to_lowercase().as_str() {
        "api-key" | "api_key" => set_api_key(value),
        "profile" => workspace_switch(value),
        _ => anyhow::bail!("Unknown config key: {}", key),
    }
}

pub fn show_config() -> Result<()> {
    let config = load_config()?;
    let path = config_path()?;

    println!("Config file: {}", path.display());
    println!();

    if let Some(current) = &config.current {
        println!("Current workspace: {}", current);
        if let Some(workspace) = config.workspaces.get(current) {
            let key = &workspace.api_key;
            if key.len() > 12 {
                let masked = format!("{}...{}", &key[..8], &key[key.len() - 4..]);
                println!("API Key: {}", masked);
            } else {
                println!("API Key: {}", key);
            }
        }
    } else {
        println!("No workspace configured. Run: linear workspace add <name>");
    }

    Ok(())
}

// Workspace management functions

pub fn workspace_add(name: &str, api_key: &str) -> Result<()> {
    let mut config = load_config()?;

    if config.workspaces.contains_key(name) {
        anyhow::bail!(
            "Workspace '{}' already exists. Use 'workspace remove' first to replace it.",
            name
        );
    }

    config.workspaces.insert(
        name.to_string(),
        Workspace {
            api_key: api_key.to_string(),
        },
    );

    // If this is the first workspace, make it current
    if config.current.is_none() {
        config.current = Some(name.to_string());
    }

    save_config(&config)?;
    println!("Workspace '{}' added successfully!", name);

    if config.current.as_ref() == Some(&name.to_string()) {
        println!("Switched to workspace '{}'", name);
    }

    Ok(())
}

pub fn workspace_list() -> Result<()> {
    let config = load_config()?;

    if config.workspaces.is_empty() {
        println!("No workspaces configured. Run: linear workspace add <name>");
        return Ok(());
    }

    println!("Configured workspaces:");
    println!();

    for (name, workspace) in &config.workspaces {
        let is_current = config.current.as_ref() == Some(name);
        let marker = if is_current { "*" } else { " " };
        let key = &workspace.api_key;
        let masked = if key.len() > 12 {
            format!("{}...{}", &key[..8], &key[key.len() - 4..])
        } else {
            key.clone()
        };
        println!("{} {} ({})", marker, name, masked);
    }

    println!();
    println!("* = current workspace");

    Ok(())
}

pub fn workspace_switch(name: &str) -> Result<()> {
    let mut config = load_config()?;

    if !config.workspaces.contains_key(name) {
        anyhow::bail!(
            "Workspace '{}' not found. Use 'workspace list' to see available workspaces.",
            name
        );
    }

    config.current = Some(name.to_string());
    save_config(&config)?;
    println!("Switched to workspace '{}'", name);

    Ok(())
}

pub fn workspace_current() -> Result<()> {
    let config = load_config()?;

    if let Some(current) = &config.current {
        println!("Current workspace: {}", current);
        if let Some(workspace) = config.workspaces.get(current) {
            let key = &workspace.api_key;
            if key.len() > 12 {
                let masked = format!("{}...{}", &key[..8], &key[key.len() - 4..]);
                println!("API Key: {}", masked);
            } else {
                println!("API Key: {}", key);
            }
        }
    } else {
        println!("No workspace selected. Run: linear workspace add <name>");
    }

    Ok(())
}

pub fn workspace_remove(name: &str) -> Result<()> {
    let mut config = load_config()?;

    if !config.workspaces.contains_key(name) {
        anyhow::bail!("Workspace '{}' not found.", name);
    }

    config.workspaces.remove(name);

    // If we removed the current workspace, clear it or switch to another
    if config.current.as_ref() == Some(&name.to_string()) {
        config.current = config.workspaces.keys().next().cloned();
        if let Some(new_current) = &config.current {
            println!("Switched to workspace '{}'", new_current);
        }
    }

    save_config(&config)?;
    println!("Workspace '{}' removed.", name);

    Ok(())
}
