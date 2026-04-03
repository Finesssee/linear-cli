use anyhow::{anyhow, Context, Result};
use dialoguer::{theme::ColorfulTheme, Select};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::output::print_json_owned;
use crate::{AgentOptions, OutputOptions};

const RELEASE_API_URL: &str = "https://api.github.com/repos/Finesssee/linear-cli/releases/latest";
const UPDATE_CHECK_INTERVAL_SECONDS: u64 = 24 * 60 * 60;

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    draft: bool,
    prerelease: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct UpdateState {
    last_checked_at: Option<u64>,
    last_seen_latest_version: Option<String>,
    skipped_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UpdateStatus {
    current_version: String,
    latest_version: Option<String>,
    release_url: Option<String>,
    update_available: bool,
    skipped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ParsedVersion {
    major: u64,
    minor: u64,
    patch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UpdateCommandPlan {
    program: String,
    args: Vec<String>,
    display: String,
}

fn release_check_timeout() -> Duration {
    Duration::from_secs(15)
}

fn release_check_connect_timeout() -> Duration {
    Duration::from_secs(10)
}

fn release_check_client() -> Result<Client> {
    Client::builder()
        .user_agent(format!("linear-cli/{}", env!("CARGO_PKG_VERSION")))
        .timeout(release_check_timeout())
        .connect_timeout(release_check_connect_timeout())
        .build()
        .context("Failed to build update-check HTTP client")
}

pub async fn handle(check: bool, output: &OutputOptions, _agent_opts: AgentOptions) -> Result<()> {
    let mut state = load_update_state().unwrap_or_default();
    let status = fetch_update_status(&state).await?;
    record_successful_check(&mut state, &status);
    save_update_state(&state)?;

    if check || !status.update_available {
        print_update_status(output, &status)?;
        return Ok(());
    }

    if !output.is_json() && !output.has_template() {
        println!(
            "Updating linear-cli from {} to {}",
            status.current_version,
            status.latest_version.as_deref().unwrap_or("latest")
        );
    }

    let used_plan = run_update_workflow(cfg!(feature = "secure-storage"))?;
    state.skipped_version = None;
    save_update_state(&state)?;

    if output.is_json() || output.has_template() {
        print_json_owned(
            json!({
                "updated": true,
                "current_version": status.current_version,
                "latest_version": status.latest_version,
                "release_url": status.release_url,
                "command": {
                    "program": used_plan.program,
                    "args": used_plan.args,
                    "display": used_plan.display,
                }
            }),
            output,
        )?;
    } else {
        println!("Updated with: {}", used_plan.display);
    }

    Ok(())
}

pub async fn maybe_prompt_for_update(auto_confirm: bool) -> Result<Option<i32>> {
    let mut state = load_update_state().unwrap_or_default();
    if !should_check_now(&state, now_unix_seconds()) {
        return Ok(None);
    }

    let status = match fetch_update_status(&state).await {
        Ok(status) => status,
        Err(_) => return Ok(None),
    };
    record_successful_check(&mut state, &status);
    let _ = save_update_state(&state);

    if !status.update_available || status.skipped {
        return Ok(None);
    }

    let latest = status
        .latest_version
        .as_deref()
        .unwrap_or("a newer release");

    if auto_confirm {
        eprintln!(
            "A newer linear-cli release is available ({} -> {}). Updating now...",
            status.current_version, latest
        );
        run_update_workflow(cfg!(feature = "secure-storage"))?;
        state.skipped_version = None;
        let _ = save_update_state(&state);
        return Ok(Some(0));
    }

    let choices = ["Update now", "Stay on current version"];
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(format!(
            "A newer linear-cli release is available ({} -> {}).",
            status.current_version, latest
        ))
        .items(&choices)
        .default(0)
        .interact()?;

    match selection {
        0 => {
            run_update_workflow(cfg!(feature = "secure-storage"))?;
            state.skipped_version = None;
            let _ = save_update_state(&state);
            Ok(Some(0))
        }
        1 => {
            state.skipped_version = status.latest_version.clone();
            let _ = save_update_state(&state);
            Ok(None)
        }
        _ => Ok(None),
    }
}

async fn fetch_update_status(state: &UpdateState) -> Result<UpdateStatus> {
    let current_version = current_version_tag();
    let release = fetch_latest_release().await?;
    let latest_version = release.tag_name;
    let update_available = is_newer_version(&latest_version, &current_version);
    let skipped = state.skipped_version.as_deref() == Some(latest_version.as_str());

    Ok(UpdateStatus {
        current_version,
        latest_version: Some(latest_version),
        release_url: Some(release.html_url),
        update_available,
        skipped,
    })
}

async fn fetch_latest_release() -> Result<GitHubRelease> {
    let client = release_check_client()?;

    let release = client
        .get(RELEASE_API_URL)
        .send()
        .await
        .context("Failed to check for the latest release")?
        .error_for_status()
        .context("Latest release check returned an error")?
        .json::<GitHubRelease>()
        .await
        .context("Failed to parse latest release response")?;

    if release.draft || release.prerelease {
        return Err(anyhow!("Latest release is not a stable published release"));
    }

    Ok(release)
}

fn run_update_workflow(secure_storage_enabled: bool) -> Result<UpdateCommandPlan> {
    let candidates = candidate_update_commands(
        cargo_binstall_available(),
        cargo_available(),
        secure_storage_enabled,
    );

    if candidates.is_empty() {
        let manual = manual_update_command(secure_storage_enabled);
        eprintln!("No automatic updater is available in this shell.");
        eprintln!("Run this instead:");
        eprintln!("  {}", manual);
        anyhow::bail!("Automatic update is unavailable");
    }

    let mut last_error: Option<anyhow::Error> = None;
    let total = candidates.len();

    for (index, plan) in candidates.iter().enumerate() {
        if total > 1 {
            eprintln!(
                "Trying updater {} of {}: {}",
                index + 1,
                total,
                plan.display
            );
        }

        match run_update_command(plan) {
            Ok(status) if status.success() => return Ok(plan.clone()),
            Ok(status) => {
                eprintln!(
                    "Updater exited with status {}: {}",
                    format_exit_status(&status),
                    plan.display
                );
                last_error = Some(anyhow!(
                    "Updater exited with status {}",
                    format_exit_status(&status)
                ));
            }
            Err(err) => {
                eprintln!("Updater failed: {}", err);
                last_error = Some(err);
            }
        }
    }

    let manual = manual_update_command(secure_storage_enabled);
    eprintln!("Automatic update did not complete.");
    eprintln!("Run this instead:");
    eprintln!("  {}", manual);

    match last_error {
        Some(err) => Err(err),
        None => anyhow::bail!("Automatic update did not complete"),
    }
}

fn run_update_command(plan: &UpdateCommandPlan) -> Result<ExitStatus> {
    Command::new(&plan.program)
        .args(&plan.args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("Failed to launch updater: {}", plan.display))
}

fn candidate_update_commands(
    cargo_binstall_available: bool,
    cargo_available: bool,
    secure_storage_enabled: bool,
) -> Vec<UpdateCommandPlan> {
    let mut commands = Vec::new();

    if secure_storage_enabled {
        if cargo_available {
            commands.push(cargo_install_plan(true));
        }
        return commands;
    }

    if cargo_binstall_available {
        commands.push(cargo_binstall_plan());
    }

    if cargo_available {
        commands.push(cargo_install_plan(false));
    }

    commands
}

fn cargo_binstall_plan() -> UpdateCommandPlan {
    UpdateCommandPlan {
        program: "cargo".to_string(),
        args: vec![
            "binstall".to_string(),
            "linear-cli".to_string(),
            "--force".to_string(),
        ],
        display: "cargo binstall linear-cli --force".to_string(),
    }
}

fn cargo_install_plan(secure_storage_enabled: bool) -> UpdateCommandPlan {
    let mut args = vec![
        "install".to_string(),
        "linear-cli".to_string(),
        "--force".to_string(),
    ];

    if secure_storage_enabled {
        args.push("--features".to_string());
        args.push("secure-storage".to_string());
    }

    let display = if secure_storage_enabled {
        "cargo install linear-cli --force --features secure-storage".to_string()
    } else {
        "cargo install linear-cli --force".to_string()
    };

    UpdateCommandPlan {
        program: "cargo".to_string(),
        args,
        display,
    }
}

fn manual_update_command(secure_storage_enabled: bool) -> String {
    cargo_install_plan(secure_storage_enabled).display
}

fn cargo_binstall_available() -> bool {
    Command::new("cargo")
        .args(["binstall", "--version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn cargo_available() -> bool {
    Command::new("cargo")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn print_update_status(output: &OutputOptions, status: &UpdateStatus) -> Result<()> {
    if output.is_json() || output.has_template() {
        print_json_owned(
            json!({
                "current_version": status.current_version,
                "latest_version": status.latest_version,
                "release_url": status.release_url,
                "update_available": status.update_available,
                "skipped": status.skipped,
            }),
            output,
        )?;
        return Ok(());
    }

    match status.latest_version.as_deref() {
        Some(latest) if status.update_available => {
            println!(
                "linear-cli {} is installed. {} is available.",
                status.current_version, latest
            );
            println!("Run `linear-cli update` to upgrade.");
        }
        _ => {
            println!("linear-cli {} is up to date.", status.current_version);
        }
    }

    Ok(())
}

fn record_successful_check(state: &mut UpdateState, status: &UpdateStatus) {
    state.last_checked_at = Some(now_unix_seconds());
    state.last_seen_latest_version = status.latest_version.clone();

    if !status.skipped {
        state.skipped_version = None;
    }
}

fn should_check_now(state: &UpdateState, now: u64) -> bool {
    match state.last_checked_at {
        Some(last_checked_at) => {
            now.saturating_sub(last_checked_at) >= UPDATE_CHECK_INTERVAL_SECONDS
        }
        None => true,
    }
}

fn load_update_state() -> Result<UpdateState> {
    let path = update_state_path()?;
    if !path.exists() {
        return Ok(UpdateState::default());
    }

    let contents = fs::read_to_string(&path)
        .with_context(|| format!("Failed to read update state at {}", path.display()))?;
    let state = serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse update state at {}", path.display()))?;
    Ok(state)
}

fn save_update_state(state: &UpdateState) -> Result<()> {
    let path = update_state_path()?;
    let dir = path
        .parent()
        .context("Update state path has no parent directory")?;
    fs::create_dir_all(dir)?;

    let content = serde_json::to_string_pretty(state)?;
    let temp_path = path.with_extension("tmp");

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&temp_path)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
    }

    #[cfg(not(unix))]
    {
        let mut file = fs::File::create(&temp_path)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
    }

    fs::rename(&temp_path, &path)?;
    Ok(())
}

fn update_state_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .context("Could not find config directory")?
        .join("linear-cli");
    Ok(config_dir.join("update.json"))
}

fn current_version_tag() -> String {
    format!("v{}", env!("CARGO_PKG_VERSION"))
}

fn parse_version(tag: &str) -> Option<ParsedVersion> {
    let normalized = tag.trim().trim_start_matches('v');
    let mut parts = normalized.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }

    Some(ParsedVersion {
        major,
        minor,
        patch,
    })
}

fn is_newer_version(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(latest), Some(current)) => latest > current,
        _ => false,
    }
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

fn format_exit_status(status: &ExitStatus) -> String {
    status
        .code()
        .map(|code| code.to_string())
        .unwrap_or_else(|| "terminated by signal".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version_strips_v_prefix() {
        assert_eq!(
            parse_version("v0.3.15"),
            Some(ParsedVersion {
                major: 0,
                minor: 3,
                patch: 15,
            })
        );
    }

    #[test]
    fn test_parse_version_rejects_malformed_tags() {
        assert_eq!(parse_version("latest"), None);
        assert_eq!(parse_version("v0.3"), None);
        assert_eq!(parse_version("v0.3.15-beta.1"), None);
    }

    #[test]
    fn test_is_newer_version_handles_plain_and_prefixed_versions() {
        assert!(is_newer_version("v0.3.15", "0.3.14"));
        assert!(!is_newer_version("v0.3.14", "0.3.14"));
        assert!(!is_newer_version("v0.3.13", "0.3.14"));
    }

    #[test]
    fn test_should_check_now_after_interval() {
        let now = 200_000;
        let state = UpdateState {
            last_checked_at: Some(now - UPDATE_CHECK_INTERVAL_SECONDS - 1),
            last_seen_latest_version: None,
            skipped_version: None,
        };

        assert!(should_check_now(&state, now));
    }

    #[test]
    fn test_should_not_check_again_within_interval() {
        let now = 200_000;
        let state = UpdateState {
            last_checked_at: Some(now - 60),
            last_seen_latest_version: None,
            skipped_version: None,
        };

        assert!(!should_check_now(&state, now));
    }

    #[test]
    fn test_candidate_update_commands_prefers_binstall_then_cargo() {
        let plans = candidate_update_commands(true, true, false);
        let displays: Vec<&str> = plans.iter().map(|plan| plan.display.as_str()).collect();

        assert_eq!(
            displays,
            vec![
                "cargo binstall linear-cli --force",
                "cargo install linear-cli --force"
            ]
        );
    }

    #[test]
    fn test_candidate_update_commands_preserves_secure_storage_feature() {
        let plans = candidate_update_commands(true, true, true);
        let displays: Vec<&str> = plans.iter().map(|plan| plan.display.as_str()).collect();

        assert_eq!(
            displays,
            vec!["cargo install linear-cli --force --features secure-storage"]
        );
    }

    #[test]
    fn test_record_successful_check_clears_skip_for_newer_release() {
        let mut state = UpdateState {
            last_checked_at: None,
            last_seen_latest_version: Some("v0.3.14".to_string()),
            skipped_version: Some("v0.3.14".to_string()),
        };
        let status = UpdateStatus {
            current_version: "v0.3.14".to_string(),
            latest_version: Some("v0.3.15".to_string()),
            release_url: None,
            update_available: true,
            skipped: false,
        };

        record_successful_check(&mut state, &status);

        assert_eq!(state.last_seen_latest_version.as_deref(), Some("v0.3.15"));
        assert_eq!(state.skipped_version, None);
    }

    #[test]
    fn test_release_check_timeout_configuration_is_explicit() {
        assert_eq!(release_check_timeout(), Duration::from_secs(15));
        assert_eq!(release_check_connect_timeout(), Duration::from_secs(10));
    }
}
