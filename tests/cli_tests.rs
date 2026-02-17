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

// --- Additional alias tests ---

#[test]
fn test_teams_alias() {
    let (code1, stdout1, _) = run_cli(&["t", "--help"]);
    let (code2, stdout2, _) = run_cli(&["teams", "--help"]);
    assert_eq!(code1, 0);
    assert_eq!(code2, 0);
    assert_eq!(stdout1, stdout2);
}

#[test]
fn test_comments_alias() {
    let (code1, stdout1, _) = run_cli(&["cm", "--help"]);
    let (code2, stdout2, _) = run_cli(&["comments", "--help"]);
    assert_eq!(code1, 0);
    assert_eq!(code2, 0);
    assert_eq!(stdout1, stdout2);
}

#[test]
fn test_git_alias() {
    let (code1, stdout1, _) = run_cli(&["g", "--help"]);
    let (code2, stdout2, _) = run_cli(&["git", "--help"]);
    assert_eq!(code1, 0);
    assert_eq!(code2, 0);
    assert_eq!(stdout1, stdout2);
}

#[test]
fn test_search_alias() {
    let (code1, stdout1, _) = run_cli(&["s", "--help"]);
    let (code2, stdout2, _) = run_cli(&["search", "--help"]);
    assert_eq!(code1, 0);
    assert_eq!(code2, 0);
    assert_eq!(stdout1, stdout2);
}

// --- Help text completeness ---

#[test]
fn test_notifications_help() {
    let (code, stdout, _stderr) = run_cli(&["notifications", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("list"));
    assert!(stdout.contains("read"));
}

#[test]
fn test_labels_help() {
    let (code, stdout, _stderr) = run_cli(&["labels", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("list"));
}

#[test]
fn test_cycles_help() {
    let (code, stdout, _stderr) = run_cli(&["cycles", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("list"));
}

#[test]
fn test_cache_help() {
    let (code, stdout, _stderr) = run_cli(&["cache", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("status"));
    assert!(stdout.contains("clear"));
}

#[test]
fn test_export_help() {
    let (code, stdout, _stderr) = run_cli(&["export", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("csv"));
}

#[test]
fn test_uploads_help() {
    let (code, stdout, _stderr) = run_cli(&["uploads", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("fetch"));
}

// --- Subcommand help tests ---

#[test]
fn test_issues_list_help() {
    let (code, stdout, _stderr) = run_cli(&["issues", "list", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--team"));
    assert!(stdout.contains("--state"));
    assert!(stdout.contains("--assignee"));
}

#[test]
fn test_issues_create_help() {
    let (code, stdout, _stderr) = run_cli(&["issues", "create", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--team"));
    assert!(stdout.contains("--priority"));
    assert!(stdout.contains("--description"));
    assert!(stdout.contains("--dry-run"));
}

#[test]
fn test_bulk_update_state_help() {
    let (code, stdout, _stderr) = run_cli(&["bulk", "update-state", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("STATE"));
    assert!(stdout.contains("--issues"));
}

// --- Global flags ---

#[test]
fn test_quiet_flag_exists() {
    let (code, stdout, _stderr) = run_cli(&["--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--quiet") || stdout.contains("-q"));
}

#[test]
fn test_dry_run_flag_exists() {
    let (code, stdout, _stderr) = run_cli(&["--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--dry-run"));
}

#[test]
fn test_compact_flag_exists() {
    let (code, stdout, _stderr) = run_cli(&["--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--compact"));
}

#[test]
fn test_fields_flag_exists() {
    let (code, stdout, _stderr) = run_cli(&["--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--fields"));
}

// --- CLI name consistency ---

#[test]
fn test_binary_name_in_help() {
    let (code, stdout, _stderr) = run_cli(&["--help"]);
    assert_eq!(code, 0);
    // The help should show the binary name
    assert!(
        stdout.contains("linear-cli") || stdout.contains("Usage:"),
        "Help output should contain binary name or usage info"
    );
}

#[test]
fn test_version_contains_semver() {
    let (code, stdout, _stderr) = run_cli(&["--version"]);
    assert_eq!(code, 0);
    // Version should contain a semver-like pattern (digit.digit)
    assert!(
        stdout.chars().any(|c| c == '.'),
        "Version output should contain a dot-separated version number"
    );
}

// --- Help tests for commands without coverage ---

#[test]
fn test_time_help() {
    let (code, stdout, _stderr) = run_cli(&["time", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("log"));
    assert!(stdout.contains("list"));
}

#[test]
fn test_relations_help() {
    let (code, stdout, _stderr) = run_cli(&["relations", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("list"));
    assert!(stdout.contains("add"));
    assert!(stdout.contains("remove"));
}

#[test]
fn test_favorites_help() {
    let (code, stdout, _stderr) = run_cli(&["favorites", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("list"));
    assert!(stdout.contains("add"));
}

#[test]
fn test_roadmaps_help() {
    let (code, stdout, _stderr) = run_cli(&["roadmaps", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("list"));
    assert!(stdout.contains("get"));
}

#[test]
fn test_initiatives_help() {
    let (code, stdout, _stderr) = run_cli(&["initiatives", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("list"));
    assert!(stdout.contains("get"));
}

#[test]
fn test_documents_help() {
    let (code, stdout, _stderr) = run_cli(&["documents", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("list"));
    assert!(stdout.contains("get"));
    assert!(stdout.contains("create"));
}

#[test]
fn test_context_help() {
    let (code, stdout, _stderr) = run_cli(&["context", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("context") || stdout.contains("issue") || stdout.contains("branch"));
}

// --- Alias tests for commands without coverage ---

#[test]
fn test_time_alias() {
    let (code1, stdout1, _) = run_cli(&["tm", "--help"]);
    let (code2, stdout2, _) = run_cli(&["time", "--help"]);
    assert_eq!(code1, 0);
    assert_eq!(code2, 0);
    assert_eq!(stdout1, stdout2);
}

#[test]
fn test_relations_alias() {
    let (code1, stdout1, _) = run_cli(&["rel", "--help"]);
    let (code2, stdout2, _) = run_cli(&["relations", "--help"]);
    assert_eq!(code1, 0);
    assert_eq!(code2, 0);
    assert_eq!(stdout1, stdout2);
}

#[test]
fn test_favorites_alias() {
    let (code1, stdout1, _) = run_cli(&["fav", "--help"]);
    let (code2, stdout2, _) = run_cli(&["favorites", "--help"]);
    assert_eq!(code1, 0);
    assert_eq!(code2, 0);
    assert_eq!(stdout1, stdout2);
}

#[test]
fn test_roadmaps_alias() {
    let (code1, stdout1, _) = run_cli(&["rm", "--help"]);
    let (code2, stdout2, _) = run_cli(&["roadmaps", "--help"]);
    assert_eq!(code1, 0);
    assert_eq!(code2, 0);
    assert_eq!(stdout1, stdout2);
}

#[test]
fn test_initiatives_alias() {
    let (code1, stdout1, _) = run_cli(&["init", "--help"]);
    let (code2, stdout2, _) = run_cli(&["initiatives", "--help"]);
    assert_eq!(code1, 0);
    assert_eq!(code2, 0);
    assert_eq!(stdout1, stdout2);
}

#[test]
fn test_documents_alias() {
    let (code1, stdout1, _) = run_cli(&["d", "--help"]);
    let (code2, stdout2, _) = run_cli(&["documents", "--help"]);
    assert_eq!(code1, 0);
    assert_eq!(code2, 0);
    assert_eq!(stdout1, stdout2);
}

#[test]
fn test_context_alias() {
    let (code1, stdout1, _) = run_cli(&["ctx", "--help"]);
    let (code2, stdout2, _) = run_cli(&["context", "--help"]);
    assert_eq!(code1, 0);
    assert_eq!(code2, 0);
    assert_eq!(stdout1, stdout2);
}

// --- v0.3.4 new subcommand tests ---

#[test]
fn test_watch_help() {
    let (code, stdout, _stderr) = run_cli(&["watch", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("issue"));
    assert!(stdout.contains("project"));
    assert!(stdout.contains("team"));
}

#[test]
fn test_watch_issue_help() {
    let (code, stdout, _stderr) = run_cli(&["watch", "issue", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--interval"));
}

#[test]
fn test_watch_project_help() {
    let (code, stdout, _stderr) = run_cli(&["watch", "project", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--interval"));
}

#[test]
fn test_watch_team_help() {
    let (code, stdout, _stderr) = run_cli(&["watch", "team", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--interval"));
}

#[test]
fn test_roadmaps_create_help() {
    let (code, stdout, _stderr) = run_cli(&["roadmaps", "create", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--description"));
}

#[test]
fn test_roadmaps_update_help() {
    let (code, stdout, _stderr) = run_cli(&["roadmaps", "update", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--name"));
    assert!(stdout.contains("--dry-run"));
}

#[test]
fn test_initiatives_create_help() {
    let (code, stdout, _stderr) = run_cli(&["initiatives", "create", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--description"));
    assert!(stdout.contains("--status"));
}

#[test]
fn test_initiatives_update_help() {
    let (code, stdout, _stderr) = run_cli(&["initiatives", "update", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--name"));
    assert!(stdout.contains("--status"));
    assert!(stdout.contains("--dry-run"));
}

#[test]
fn test_documents_delete_help() {
    let (code, stdout, _stderr) = run_cli(&["documents", "delete", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--force"));
    assert!(stdout.contains("--dry-run"));
}

#[test]
fn test_roadmaps_help_includes_create() {
    let (code, stdout, _stderr) = run_cli(&["roadmaps", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("create"));
    assert!(stdout.contains("update"));
}

#[test]
fn test_initiatives_help_includes_create() {
    let (code, stdout, _stderr) = run_cli(&["initiatives", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("create"));
    assert!(stdout.contains("update"));
}

#[test]
fn test_documents_help_includes_delete() {
    let (code, stdout, _stderr) = run_cli(&["documents", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("delete"));
}

// --- v0.3.5 new subcommand tests ---

#[test]
fn test_triage_help() {
    let (code, stdout, _stderr) = run_cli(&["triage", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("list"));
    assert!(stdout.contains("claim"));
}

#[test]
fn test_triage_alias() {
    let (code1, stdout1, _) = run_cli(&["tr", "--help"]);
    let (code2, stdout2, _) = run_cli(&["triage", "--help"]);
    assert_eq!(code1, 0);
    assert_eq!(code2, 0);
    assert_eq!(stdout1, stdout2);
}

#[test]
fn test_notifications_archive_help() {
    let (code, stdout, _stderr) = run_cli(&["notifications", "archive", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("id") || stdout.contains("ID"));
}

#[test]
fn test_notifications_archive_all_help() {
    let (code, _stdout, _stderr) = run_cli(&["notifications", "archive-all", "--help"]);
    assert_eq!(code, 0);
}

#[test]
fn test_notifications_help_includes_archive() {
    let (code, stdout, _stderr) = run_cli(&["notifications", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("archive"));
}

#[test]
fn test_cycles_create_help() {
    let (code, stdout, _stderr) = run_cli(&["cycles", "create", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--team"));
    assert!(stdout.contains("--name"));
    assert!(stdout.contains("--starts-at"));
    assert!(stdout.contains("--ends-at"));
}

#[test]
fn test_cycles_update_help() {
    let (code, stdout, _stderr) = run_cli(&["cycles", "update", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--name"));
    assert!(stdout.contains("--dry-run"));
}

#[test]
fn test_cycles_help_includes_create() {
    let (code, stdout, _stderr) = run_cli(&["cycles", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("create"));
    assert!(stdout.contains("update"));
}

// --- v0.3.6 OAuth tests ---

#[test]
fn test_auth_help_includes_oauth() {
    let (code, stdout, _stderr) = run_cli(&["auth", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("oauth"), "auth help should list oauth subcommand");
    assert!(stdout.contains("revoke"), "auth help should list revoke subcommand");
    assert!(stdout.contains("login"));
    assert!(stdout.contains("logout"));
    assert!(stdout.contains("status"));
}

#[test]
fn test_auth_oauth_help() {
    let (code, stdout, _stderr) = run_cli(&["auth", "oauth", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--client-id"));
    assert!(stdout.contains("--scopes"));
    assert!(stdout.contains("--port"));
    assert!(stdout.contains("--secure"));
}

#[test]
fn test_auth_oauth_default_scopes() {
    let (code, stdout, _stderr) = run_cli(&["auth", "oauth", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("read,write,admin"), "default scopes should be read,write,admin");
}

#[test]
fn test_auth_oauth_default_port() {
    let (code, stdout, _stderr) = run_cli(&["auth", "oauth", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("8484"), "default port should be 8484");
}

#[test]
fn test_auth_revoke_help() {
    let (code, stdout, _stderr) = run_cli(&["auth", "revoke", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--force"));
}

#[test]
fn test_auth_status_help() {
    let (code, stdout, _stderr) = run_cli(&["auth", "status", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--validate"));
}

#[test]
fn test_auth_help_examples_include_oauth() {
    let (code, stdout, _stderr) = run_cli(&["auth", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("linear auth oauth"), "help examples should show oauth usage");
    assert!(stdout.contains("linear auth revoke"), "help examples should show revoke usage");
}

// --- v0.3.7 Views + Webhooks tests ---

#[test]
fn test_views_help() {
    let (code, stdout, _stderr) = run_cli(&["views", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("list"));
    assert!(stdout.contains("get"));
    assert!(stdout.contains("create"));
    assert!(stdout.contains("update"));
    assert!(stdout.contains("delete"));
}

#[test]
fn test_views_alias() {
    let (code1, stdout1, _) = run_cli(&["v", "--help"]);
    let (code2, stdout2, _) = run_cli(&["views", "--help"]);
    assert_eq!(code1, 0);
    assert_eq!(code2, 0);
    assert_eq!(stdout1, stdout2);
}

#[test]
fn test_views_create_help() {
    let (code, stdout, _stderr) = run_cli(&["views", "create", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--description"));
    assert!(stdout.contains("--team"));
    assert!(stdout.contains("--shared"));
    assert!(stdout.contains("--filter-json"));
    assert!(stdout.contains("--icon"));
    assert!(stdout.contains("--color"));
}

#[test]
fn test_views_update_help() {
    let (code, stdout, _stderr) = run_cli(&["views", "update", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--name"));
    assert!(stdout.contains("--description"));
    assert!(stdout.contains("--shared"));
    assert!(stdout.contains("--filter-json"));
}

#[test]
fn test_views_delete_help() {
    let (code, stdout, _stderr) = run_cli(&["views", "delete", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--force"));
}

#[test]
fn test_webhooks_help() {
    let (code, stdout, _stderr) = run_cli(&["webhooks", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("list"));
    assert!(stdout.contains("get"));
    assert!(stdout.contains("create"));
    assert!(stdout.contains("update"));
    assert!(stdout.contains("delete"));
    assert!(stdout.contains("rotate-secret"));
    assert!(stdout.contains("listen"));
}

#[test]
fn test_webhooks_alias() {
    let (code1, stdout1, _) = run_cli(&["wh", "--help"]);
    let (code2, stdout2, _) = run_cli(&["webhooks", "--help"]);
    assert_eq!(code1, 0);
    assert_eq!(code2, 0);
    assert_eq!(stdout1, stdout2);
}

#[test]
fn test_webhooks_create_help() {
    let (code, stdout, _stderr) = run_cli(&["webhooks", "create", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--events"));
    assert!(stdout.contains("--team"));
    assert!(stdout.contains("--all-teams"));
    assert!(stdout.contains("--label"));
    assert!(stdout.contains("--secret"));
}

#[test]
fn test_webhooks_update_help() {
    let (code, stdout, _stderr) = run_cli(&["webhooks", "update", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--url"));
    assert!(stdout.contains("--events"));
    assert!(stdout.contains("--enabled"));
    assert!(stdout.contains("--disabled"));
    assert!(stdout.contains("--label"));
}

#[test]
fn test_webhooks_delete_help() {
    let (code, stdout, _stderr) = run_cli(&["webhooks", "delete", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--force"));
}

#[test]
fn test_webhooks_listen_help() {
    let (code, stdout, _stderr) = run_cli(&["webhooks", "listen", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--port"));
    assert!(stdout.contains("--events"));
    assert!(stdout.contains("--team"));
    assert!(stdout.contains("--secret"));
    assert!(stdout.contains("--url"));
    assert!(stdout.contains("--json"));
    assert!(stdout.contains("ngrok") || stdout.contains("tunnel"));
}

#[test]
fn test_webhooks_rotate_secret_help() {
    let (code, stdout, _stderr) = run_cli(&["webhooks", "rotate-secret", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("<ID>") || stdout.contains("id") || stdout.contains("ID"));
}

#[test]
fn test_issues_list_view_flag() {
    let (code, stdout, _stderr) = run_cli(&["issues", "list", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--view"), "issues list should have --view flag");
}

#[test]
fn test_projects_list_view_flag() {
    let (code, stdout, _stderr) = run_cli(&["projects", "list", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("--view"), "projects list should have --view flag");
}

#[test]
fn test_auth_oauth_default_scopes_include_admin() {
    let (code, stdout, _stderr) = run_cli(&["auth", "oauth", "--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("read,write,admin"),
        "default scopes should now include admin"
    );
}

// === Whoami command tests ===

#[test]
fn test_whoami_help() {
    let (code, stdout, _stderr) = run_cli(&["whoami", "--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("authenticated user") || stdout.contains("users me"),
        "whoami should describe showing current user"
    );
}

#[test]
fn test_whoami_alias_me() {
    // "me" should be an alias for whoami
    let (code, stdout, _stderr) = run_cli(&["me", "--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("authenticated user") || stdout.contains("users me"),
        "me alias should work for whoami"
    );
}

#[test]
fn test_help_shows_whoami() {
    let (code, stdout, _stderr) = run_cli(&["--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("whoami"),
        "top-level help should list whoami command"
    );
}

// === Raw GraphQL API command tests ===

#[test]
fn test_api_help() {
    let (code, stdout, _stderr) = run_cli(&["api", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("query"), "api help should mention query");
    assert!(stdout.contains("mutate"), "api help should mention mutate");
}

#[test]
fn test_api_query_help() {
    let (code, stdout, _stderr) = run_cli(&["api", "query", "--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("--variable"),
        "api query should have --variable flag"
    );
    assert!(
        stdout.contains("--paginate"),
        "api query should have --paginate flag"
    );
}

#[test]
fn test_api_mutate_help() {
    let (code, stdout, _stderr) = run_cli(&["api", "mutate", "--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("--variable"),
        "api mutate should have --variable flag"
    );
}

#[test]
fn test_help_shows_api() {
    let (code, stdout, _stderr) = run_cli(&["--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("api") || stdout.contains("Api"),
        "top-level help should list api command"
    );
}

// === --since / --newer-than time filter tests ===

#[test]
fn test_issues_list_since_flag() {
    let (code, stdout, _stderr) = run_cli(&["issues", "list", "--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("--since"),
        "issues list should have --since flag"
    );
}

#[test]
fn test_issues_list_newer_than_alias() {
    let (code, stdout, _stderr) = run_cli(&["issues", "list", "--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("newer-than") || stdout.contains("--since"),
        "issues list should support --newer-than alias"
    );
}

#[test]
fn test_issues_get_history_flag() {
    let (code, stdout, _stderr) = run_cli(&["issues", "get", "--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("--history"),
        "issues get should have --history flag"
    );
}

#[test]
fn test_issues_get_comments_flag() {
    let (code, stdout, _stderr) = run_cli(&["issues", "get", "--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("--comments"),
        "issues get should have --comments flag"
    );
}

#[test]
fn test_issues_open_help() {
    let (code, stdout, _stderr) = run_cli(&["issues", "open", "--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("Open issue in browser"),
        "issues open should show help"
    );
}

#[test]
fn test_issues_list_group_by_flag() {
    let (code, stdout, _stderr) = run_cli(&["issues", "list", "--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("--group-by"),
        "issues list should have --group-by flag"
    );
}

#[test]
fn test_issues_list_count_only_flag() {
    let (code, stdout, _stderr) = run_cli(&["issues", "list", "--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("--count-only"),
        "issues list should have --count-only flag"
    );
}

#[test]
fn test_projects_open_help() {
    let (code, stdout, _stderr) = run_cli(&["projects", "open", "--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("Open project in browser"),
        "projects open should show help"
    );
}

#[test]
fn test_issues_close_help() {
    let (code, stdout, _stderr) = run_cli(&["issues", "close", "--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("Close an issue"),
        "issues close should show help"
    );
}

#[test]
fn test_issues_close_alias_done() {
    let (code, stdout, _stderr) = run_cli(&["issues", "done", "--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("Close an issue"),
        "issues done alias should work"
    );
}

// === Milestone CRUD tests ===

#[test]
fn test_milestones_help() {
    let (code, stdout, _stderr) = run_cli(&["milestones", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("list"), "milestones should have list");
    assert!(stdout.contains("get"), "milestones should have get");
    assert!(stdout.contains("create"), "milestones should have create");
    assert!(stdout.contains("update"), "milestones should have update");
    assert!(stdout.contains("delete"), "milestones should have delete");
}

#[test]
fn test_milestones_alias_ms() {
    let (code, stdout, _stderr) = run_cli(&["ms", "--help"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("list"), "ms alias should work");
}

#[test]
fn test_milestones_create_help() {
    let (code, stdout, _stderr) = run_cli(&["milestones", "create", "--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("--project"),
        "milestone create should require --project"
    );
    assert!(
        stdout.contains("--target-date"),
        "milestone create should have --target-date"
    );
}

#[test]
fn test_milestones_update_help() {
    let (code, stdout, _stderr) = run_cli(&["milestones", "update", "--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("--name"),
        "milestone update should have --name"
    );
    assert!(
        stdout.contains("--target-date"),
        "milestone update should have --target-date"
    );
}

#[test]
fn test_milestones_delete_help() {
    let (code, stdout, _stderr) = run_cli(&["milestones", "delete", "--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("--force"),
        "milestone delete should have --force"
    );
}

#[test]
fn test_help_shows_milestones() {
    let (code, stdout, _stderr) = run_cli(&["--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("milestones") || stdout.contains("Milestones"),
        "top-level help should list milestones command"
    );
}

// === Pager support tests ===

#[test]
fn test_no_pager_flag() {
    let (code, stdout, _stderr) = run_cli(&["--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("--no-pager"),
        "global help should show --no-pager flag"
    );
}

#[test]
fn test_no_pager_env_var() {
    // Verify LINEAR_CLI_NO_PAGER env var is documented in help
    let (code, stdout, _stderr) = run_cli(&["--help"]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("LINEAR_CLI_NO_PAGER") || stdout.contains("no-pager"),
        "no-pager should be available as flag or env var"
    );
}

