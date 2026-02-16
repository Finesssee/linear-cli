# Codex Improvement Audit - Implementation Summary

14 improvements identified via OpenAI Codex analysis. All 14 implemented.

## Completed (14/14)

### Bug Fixes
1. **Time tracking error swallowing** (`src/commands/time.rs`) - `Err(_)` branches now propagate via `anyhow::bail!` instead of printing success-like messages.
2. **Identifier resolution in teams/projects** (`src/api.rs`, `src/commands/teams.rs`, `src/commands/projects.rs`) - Added `resolve_team_id` and `resolve_project_id` calls so `get` commands accept names/keys, not just UUIDs.
3. **State name resolution** (`src/api.rs`, `src/commands/issues.rs`) - Added `resolve_state_id()` so `--state "In Progress"` works in issue create/update (previously required UUIDs).
4. **Pagination truncation** (`src/commands/notifications.rs`) - Replaced hardcoded `first: 100` with `paginate_nodes(..., all: true)` for notification count and mark-all-as-read.
5. **Cache projects type missing** (`src/commands/cache.rs`) - Added `"projects" => CacheType::Projects` to cache clear.
6. **Credential storage profile switching** (`src/config.rs`) - `set_api_key()` now sets `config.current` to the actual workspace name instead of hardcoded `"default"`.

### Reliability
7. **Bounded concurrency** (`src/commands/bulk.rs`, `src/commands/notifications.rs`) - Replaced `join_all` with `buffer_unordered(10)` in all 4 bulk operations + notification mark-all.
8. **Typed ErrorKind enum** (`src/error.rs`, `src/retry.rs`, `src/output.rs`, `src/main.rs`) - Replaced raw `u8` error codes with `ErrorKind` enum (`General`, `NotFound`, `Auth`, `RateLimited`) and convenience constructors.
9. **Stderr diagnostics** (`src/output.rs`, `src/keyring.rs`, `src/retry.rs`) - Added `OnceLock<bool>` quiet mode; `eprintln!` calls now check `is_quiet()` before writing.

### Consistency
10. **CLI naming** (`src/main.rs`) - Changed clap `#[command(name = "linear")]` to `#[command(name = "linear-cli")]` to match binary name.
11. **Upload streaming** (`src/commands/uploads.rs`, `src/api.rs`) - Added `fetch_to_writer()` for streaming file downloads instead of buffering entire file in memory.

### Quality
12. **Dead code warning suppression** (`src/main.rs`) - Added `#[allow(dead_code)]` on `mod types` since structs are for gradual adoption.
13. **Test coverage** - Added 51 new tests (94->126 unit, 13->32 integration), covering config serialization, pagination options, output filtering/sorting/templates/field selection, CLI aliases, subcommand help text, and global flags.
14. **Typed API response models** - Adopted typed structs from `types.rs` in 6 command handlers (teams, users, projects, labels, comments, documents), replacing raw `serde_json::Value` indexing with `serde_json::from_value::<T>()` deserialization. Added `Serialize` to all types, added missing fields (`Document.url`, `Team.issue_count/created_at/updated_at`, `Viewer.url/created_at`, `Project.status/start_date/labels`).

## Test Results

- **126 unit tests** passing
- **32 integration tests** passing
- **0 warnings** in build output
