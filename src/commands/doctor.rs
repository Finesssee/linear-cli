use anyhow::Result;
use serde_json::json;

use crate::api::LinearClient;
use crate::cache::{self, Cache};
use crate::config;
use crate::output::{print_json_owned, OutputOptions};

pub async fn run(output: &OutputOptions, check_api: bool, fix: bool) -> Result<()> {
    let config_path = config::config_file_path()?;
    let config_data = config::load_config()?;
    let profile = config::current_profile().ok();
    let env_key = std::env::var("LINEAR_API_KEY")
        .ok()
        .filter(|k| !k.is_empty());
    let env_profile = std::env::var("LINEAR_CLI_PROFILE")
        .ok()
        .filter(|p| !p.is_empty());
    let cache_dir = cache::cache_dir_path()?;

    let configured = profile
        .as_ref()
        .and_then(|p| config_data.workspaces.get(p))
        .map(|w| !w.api_key.is_empty())
        .unwrap_or(false);

    let mut api_ok = None;
    let mut api_error = None;
    if check_api || fix {
        match validate_api().await {
            Ok(()) => api_ok = Some(true),
            Err(err) => {
                api_ok = Some(false);
                api_error = Some(err.to_string());
            }
        }
    }

    // --fix mode: auto-remediate common issues
    if fix {
        let mut fixed = Vec::new();

        // Fix 1: Missing config file — create default
        if !config_path.exists() {
            let default_config = config::Config::default();
            config::save_config(&default_config)?;
            fixed.push("Created default config file");
        }

        // Fix 2: No profile configured — create a "default" workspace entry
        if !configured && profile.is_none() {
            // Only prompt if not in quiet/json mode
            if !output.is_json() && !crate::output::is_quiet() {
                use std::io::{self, Write};
                println!("No API key configured.");
                print!("Enter your Linear API key: ");
                io::stdout().flush()?;

                let mut key = String::new();
                io::stdin().read_line(&mut key)?;
                let key = key.trim();

                if !key.is_empty() {
                    config::set_api_key(key)?;
                    fixed.push("Saved API key");
                }
            }
        }

        // Fix 3: Stale/corrupt cache — clear it
        if let Ok(cache) = Cache::new() {
            if let Ok(()) = cache.clear_all() {
                fixed.push("Cleared cache");
            }
        }

        // Fix 4: API key invalid — prompt for new one
        if api_ok == Some(false) && !output.is_json() && !crate::output::is_quiet() {
            use std::io::{self, Write};
            println!("API key is invalid or expired.");
            print!("Enter a new Linear API key (or press Enter to skip): ");
            io::stdout().flush()?;

            let mut key = String::new();
            io::stdin().read_line(&mut key)?;
            let key = key.trim();

            if !key.is_empty() {
                config::set_api_key(key)?;
                fixed.push("Updated API key");

                // Re-validate
                match validate_api().await {
                    Ok(()) => {
                        api_ok = Some(true);
                        api_error = None;
                        fixed.push("API key validated successfully");
                    }
                    Err(err) => {
                        api_error = Some(err.to_string());
                    }
                }
            }
        }

        if output.is_json() || output.has_template() {
            print_json_owned(
                json!({
                    "fix": true,
                    "fixed": fixed,
                    "config_path": config_path.to_string_lossy(),
                    "api_ok": api_ok,
                    "api_error": api_error,
                }),
                output,
            )?;
            return Ok(());
        }

        if fixed.is_empty() {
            println!("No issues found to fix.");
        } else {
            println!("Fixed:");
            for item in &fixed {
                println!("  + {}", item);
            }
        }

        if let Some(false) = api_ok {
            if let Some(err) = &api_error {
                println!("API still failing: {}", err);
            }
        }

        return Ok(());
    }

    if output.is_json() || output.has_template() {
        print_json_owned(
            json!({
                "config_path": config_path.to_string_lossy(),
                "profile": profile,
                "configured": configured,
                "env_api_key": env_key.is_some(),
                "env_profile": env_profile,
                "cache_dir": cache_dir.to_string_lossy(),
                "cache_ttl_seconds": output.cache.effective_ttl_seconds(),
                "api_ok": api_ok,
                "api_error": api_error,
            }),
            output,
        )?;
        return Ok(());
    }

    println!("Config path: {}", config_path.display());
    println!("Profile: {}", profile.unwrap_or_else(|| "none".to_string()));
    println!("Configured: {}", if configured { "yes" } else { "no" });
    println!(
        "Env API key override: {}",
        if env_key.is_some() { "yes" } else { "no" }
    );
    println!(
        "Env profile override: {}",
        env_profile.unwrap_or_else(|| "none".to_string())
    );
    println!("Cache dir: {}", cache_dir.display());
    println!("Cache TTL: {}s", output.cache.effective_ttl_seconds());
    if let Some(api_ok) = api_ok {
        println!("API check: {}", if api_ok { "ok" } else { "failed" });
        if let Some(err) = api_error {
            println!("API error: {}", err);
        }
    }

    Ok(())
}

async fn validate_api() -> Result<()> {
    let client = LinearClient::new()?;
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
        anyhow::bail!("Viewer query failed");
    }
    Ok(())
}
