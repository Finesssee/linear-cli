use std::process::Command;

/// Helper to run CLI commands and capture output
fn run_cli(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_linear-cli"))
        .args(args)
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);

    (code, stdout, stderr)
}

#[test]
fn test_help_command() {
    let (code, stdout, _stderr) = run_cli(&["--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("A powerful CLI for Linear.app"));
    assert!(stdout.contains("Commands:"));
}

#[test]
fn test_version_command() {
    let (code, stdout, _stderr) = run_cli(&["--version"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("linear") || stdout.contains("0.1"));
}

#[test]
fn test_projects_help() {
    let (code, stdout, _stderr) = run_cli(&["projects", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("list"));
    assert!(stdout.contains("create"));
}

#[test]
fn test_issues_help() {
    let (code, stdout, _stderr) = run_cli(&["issues", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("list"));
    assert!(stdout.contains("create"));
    assert!(stdout.contains("start"));
    assert!(stdout.contains("stop"));
}

#[test]
fn test_teams_help() {
    let (code, stdout, _stderr) = run_cli(&["teams", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("list"));
}

#[test]
fn test_config_help() {
    let (code, stdout, _stderr) = run_cli(&["config", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("set-key"));
    assert!(stdout.contains("show"));
    assert!(stdout.contains("workspace-add"));
    assert!(stdout.contains("workspace-list"));
}

#[test]
fn test_bulk_help() {
    let (code, stdout, _stderr) = run_cli(&["bulk", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("update-state"));
    assert!(stdout.contains("assign"));
    assert!(stdout.contains("label"));
}

#[test]
fn test_search_help() {
    let (code, stdout, _stderr) = run_cli(&["search", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("issues"));
    assert!(stdout.contains("projects"));
}

#[test]
fn test_git_help() {
    let (code, stdout, _stderr) = run_cli(&["git", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("checkout"));
    assert!(stdout.contains("branch"));
}

#[test]
fn test_sync_help() {
    let (code, stdout, _stderr) = run_cli(&["sync", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("status"));
    assert!(stdout.contains("push"));
}

#[test]
fn test_aliases_work() {
    // Test short aliases
    let (code1, stdout1, _) = run_cli(&["p", "--help"]);
    let (code2, stdout2, _) = run_cli(&["projects", "--help"]);
    assert_eq!(code1, 0);
    assert_eq!(code2, 0);
    assert_eq!(stdout1, stdout2);

    let (code3, stdout3, _) = run_cli(&["i", "--help"]);
    let (code4, stdout4, _) = run_cli(&["issues", "--help"]);
    assert_eq!(code3, 0);
    assert_eq!(code4, 0);
    assert_eq!(stdout3, stdout4);
}

#[test]
fn test_output_format_option() {
    let (code, stdout, _stderr) = run_cli(&["--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--output"));
    assert!(stdout.contains("table"));
    assert!(stdout.contains("json"));
}

#[test]
fn test_invalid_command() {
    let (code, _stdout, stderr) = run_cli(&["invalid-command"]);
    assert_ne!(code, 0);
    assert!(stderr.contains("error") || stderr.contains("invalid"));
}
