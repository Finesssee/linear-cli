use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

#[cfg(unix)]
use std::io::Write;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    pub token_type: String,
    #[serde(default)]
    pub scopes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Workspace {
    pub api_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth: Option<OAuthConfig>,
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
                        oauth: None,
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
    let existing_oauth = config.workspaces.get(&workspace_name).and_then(|w| w.oauth.clone());
    config.workspaces.insert(
        workspace_name.clone(),
        Workspace {
            api_key: key.to_string(),
            oauth: existing_oauth,
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

    // Check for OAuth tokens
    {
        let config = load_config()?;
        let profile = std::env::var("LINEAR_CLI_PROFILE")
            .ok()
            .filter(|p| !p.is_empty())
            .or(config.current.clone())
            .unwrap_or_else(|| "default".to_string());

        if let Ok(Some(oauth)) = get_oauth_config(&profile) {
            if !oauth.access_token.is_empty() {
                return Ok(format!("Bearer {}", oauth.access_token));
            }
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
    static PROFILE: OnceLock<String> = OnceLock::new();

    if let Some(cached) = PROFILE.get() {
        return Ok(cached.clone());
    }

    let config = load_config()?;
    let profile = std::env::var("LINEAR_CLI_PROFILE")
        .ok()
        .filter(|p| !p.is_empty());
    let resolved = profile
        .or(config.current)
        .context("No workspace selected")?;

    // Store for future calls (ignore if another thread beat us)
    let _ = PROFILE.set(resolved.clone());
    Ok(resolved)
}

pub fn set_workspace_key(name: &str, api_key: &str) -> Result<()> {
    let mut config = load_config()?;
    let existing_oauth = config.workspaces.get(name).and_then(|w| w.oauth.clone());
    config.workspaces.insert(
        name.to_string(),
        Workspace {
            api_key: api_key.to_string(),
            oauth: existing_oauth,
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
            oauth: None,
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

/// Save OAuth config for a profile
pub fn save_oauth_config(profile: &str, oauth_config: &OAuthConfig) -> Result<()> {
    let mut config = load_config()?;
    let workspace = config.workspaces.entry(profile.to_string())
        .or_insert_with(|| Workspace { api_key: String::new(), oauth: None });
    workspace.oauth = Some(oauth_config.clone());
    if config.current.is_none() {
        config.current = Some(profile.to_string());
    }
    save_config(&config)?;
    Ok(())
}

/// Get OAuth config for a profile
pub fn get_oauth_config(profile: &str) -> Result<Option<OAuthConfig>> {
    // Try keyring first if feature enabled
    #[cfg(feature = "secure-storage")]
    {
        if let Ok(Some(json_str)) = crate::keyring::get_oauth_tokens(profile) {
            if let Ok(oauth) = serde_json::from_str::<OAuthConfig>(&json_str) {
                return Ok(Some(oauth));
            }
        }
    }

    // Fall back to config file
    let config = load_config()?;
    Ok(config.workspaces.get(profile).and_then(|w| w.oauth.clone()))
}

/// Clear OAuth config for a profile
pub fn clear_oauth_config(profile: &str) -> Result<()> {
    #[cfg(feature = "secure-storage")]
    {
        let _ = crate::keyring::delete_oauth_tokens(profile);
    }

    let mut config = load_config()?;
    if let Some(workspace) = config.workspaces.get_mut(profile) {
        workspace.oauth = None;
    }
    save_config(&config)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert!(config.current.is_none());
        assert!(config.workspaces.is_empty());
        assert!(config.api_key.is_none());
    }

    #[test]
    fn test_config_serialize_deserialize() {
        let mut config = Config::default();
        config.current = Some("prod".to_string());
        config.workspaces.insert(
            "prod".to_string(),
            Workspace {
                api_key: "lin_api_prod123".to_string(),
                oauth: None,
            },
        );
        config.workspaces.insert(
            "staging".to_string(),
            Workspace {
                api_key: "lin_api_staging456".to_string(),
                oauth: None,
            },
        );

        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();

        assert_eq!(parsed.current, Some("prod".to_string()));
        assert_eq!(parsed.workspaces.len(), 2);
        assert_eq!(parsed.workspaces["prod"].api_key, "lin_api_prod123");
        assert_eq!(parsed.workspaces["staging"].api_key, "lin_api_staging456");
    }

    #[test]
    fn test_config_legacy_migration_parse() {
        // Legacy config format with top-level api_key
        let toml_str = r#"
            api_key = "lin_api_legacy_key"
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.api_key, Some("lin_api_legacy_key".to_string()));
        assert!(config.workspaces.is_empty());
        assert!(config.current.is_none());
    }

    #[test]
    fn test_config_with_workspaces_parse() {
        let toml_str = r#"
            current = "default"

            [workspaces.default]
            api_key = "lin_api_key1"

            [workspaces.staging]
            api_key = "lin_api_key2"
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.current, Some("default".to_string()));
        assert_eq!(config.workspaces.len(), 2);
        assert!(config.api_key.is_none());
    }

    #[test]
    fn test_config_api_key_not_serialized_when_none() {
        let config = Config {
            current: Some("default".to_string()),
            workspaces: HashMap::new(),
            api_key: None,
        };

        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(!toml_str.contains("api_key"));
    }

    #[test]
    fn test_config_with_oauth_parse() {
        let toml_str = r#"
            current = "oauth-profile"

            [workspaces.oauth-profile]
            api_key = ""

            [workspaces.oauth-profile.oauth]
            client_id = "abc123"
            access_token = "lin_oauth_xxx"
            refresh_token = "lin_refresh_yyy"
            expires_at = 1700000000
            token_type = "Bearer"
            scopes = ["read", "write"]
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.current, Some("oauth-profile".to_string()));
        let ws = &config.workspaces["oauth-profile"];
        assert!(ws.oauth.is_some());
        let oauth = ws.oauth.as_ref().unwrap();
        assert_eq!(oauth.client_id, "abc123");
        assert_eq!(oauth.access_token, "lin_oauth_xxx");
        assert_eq!(oauth.refresh_token, Some("lin_refresh_yyy".to_string()));
        assert_eq!(oauth.expires_at, Some(1700000000));
        assert_eq!(oauth.scopes, vec!["read", "write"]);
    }

    #[test]
    fn test_config_without_oauth_still_parses() {
        let toml_str = r#"
            current = "default"

            [workspaces.default]
            api_key = "lin_api_key1"
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        let ws = &config.workspaces["default"];
        assert!(ws.oauth.is_none());
    }

    #[test]
    fn test_oauth_config_serialize() {
        let oauth = OAuthConfig {
            client_id: "test".to_string(),
            access_token: "acc".to_string(),
            refresh_token: Some("ref".to_string()),
            expires_at: Some(1700000000),
            token_type: "Bearer".to_string(),
            scopes: vec!["read".to_string()],
        };
        let json = serde_json::to_string(&oauth).unwrap();
        let parsed: OAuthConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.client_id, "test");
    }

    #[test]
    fn test_config_with_mixed_profiles() {
        let toml_str = r#"
            current = "default"

            [workspaces.default]
            api_key = "lin_api_key1"

            [workspaces.oauth-ws]
            api_key = ""

            [workspaces.oauth-ws.oauth]
            client_id = "cid"
            access_token = "at"
            token_type = "Bearer"
            scopes = ["read"]
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.workspaces["default"].oauth.is_none());
        assert!(config.workspaces["oauth-ws"].oauth.is_some());
    }

    #[test]
    fn test_oauth_config_roundtrip_toml() {
        let mut config = Config::default();
        config.current = Some("oauth-test".to_string());
        config.workspaces.insert(
            "oauth-test".to_string(),
            Workspace {
                api_key: String::new(),
                oauth: Some(OAuthConfig {
                    client_id: "my-app".to_string(),
                    access_token: "lin_oauth_token123".to_string(),
                    refresh_token: Some("lin_refresh_abc".to_string()),
                    expires_at: Some(1700000000),
                    token_type: "Bearer".to_string(),
                    scopes: vec!["read".to_string(), "write".to_string()],
                }),
            },
        );

        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();

        let ws = &parsed.workspaces["oauth-test"];
        let oauth = ws.oauth.as_ref().unwrap();
        assert_eq!(oauth.client_id, "my-app");
        assert_eq!(oauth.access_token, "lin_oauth_token123");
        assert_eq!(oauth.refresh_token.as_deref(), Some("lin_refresh_abc"));
        assert_eq!(oauth.expires_at, Some(1700000000));
        assert_eq!(oauth.token_type, "Bearer");
        assert_eq!(oauth.scopes, vec!["read", "write"]);
    }

    #[test]
    fn test_oauth_not_serialized_when_none() {
        let mut config = Config::default();
        config.current = Some("default".to_string());
        config.workspaces.insert(
            "default".to_string(),
            Workspace {
                api_key: "lin_api_key".to_string(),
                oauth: None,
            },
        );

        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(!toml_str.contains("[workspaces.default.oauth]"), "oauth section should not appear when None");
        assert!(!toml_str.contains("client_id"));
        assert!(!toml_str.contains("access_token"));
    }

    #[test]
    fn test_oauth_config_empty_scopes() {
        let toml_str = r#"
            current = "test"

            [workspaces.test]
            api_key = ""

            [workspaces.test.oauth]
            client_id = "cid"
            access_token = "tok"
            token_type = "Bearer"
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        let oauth = config.workspaces["test"].oauth.as_ref().unwrap();
        assert!(oauth.scopes.is_empty(), "scopes should default to empty vec");
        assert!(oauth.refresh_token.is_none());
        assert!(oauth.expires_at.is_none());
    }

    #[test]
    fn test_oauth_config_json_roundtrip() {
        let oauth = OAuthConfig {
            client_id: "app-id".to_string(),
            access_token: "access".to_string(),
            refresh_token: Some("refresh".to_string()),
            expires_at: Some(1700086400),
            token_type: "Bearer".to_string(),
            scopes: vec!["read".to_string(), "write".to_string(), "issues:create".to_string()],
        };
        let json = serde_json::to_string(&oauth).unwrap();
        let parsed: OAuthConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.client_id, "app-id");
        assert_eq!(parsed.scopes.len(), 3);
        assert_eq!(parsed.scopes[2], "issues:create");
    }

    #[test]
    fn test_workspace_with_both_apikey_and_oauth() {
        let toml_str = r#"
            current = "dual"

            [workspaces.dual]
            api_key = "lin_api_key_primary"

            [workspaces.dual.oauth]
            client_id = "cid"
            access_token = "oauth_tok"
            token_type = "Bearer"
            scopes = ["read"]
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        let ws = &config.workspaces["dual"];
        assert_eq!(ws.api_key, "lin_api_key_primary");
        assert!(ws.oauth.is_some());
        assert_eq!(ws.oauth.as_ref().unwrap().access_token, "oauth_tok");
    }
}
