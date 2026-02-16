# v0.3.2 Performance Improvements - Implementation Summary

13 performance optimizations across cache infrastructure, API efficiency, and data processing.

## Completed (13/13)

### Cache Infrastructure
1. **Per-key cache timestamps** (`src/cache.rs`) - Cache entries store individual `{"data": value, "timestamp": unix_seconds}` wrappers for independent per-key expiration instead of whole-file TTL.
2. **TTL on read path** (`src/cache.rs`, `src/commands/teams.rs`, `src/commands/users.rs`, `src/commands/statuses.rs`) - `Cache::with_ttl()` used on reads so expired entries are rejected, not just on writes.
3. **Memoized config/cache paths** (`src/config.rs`, `src/cache.rs`) - `OnceLock<String>` for config profile and `OnceLock<PathBuf>` for cache directory avoid repeated filesystem lookups.

### API & Network
4. **Streaming downloads** (`src/api.rs`, `src/commands/uploads.rs`) - `fetch_to_writer()` uses `response.bytes_stream()` for chunked writes instead of buffering entire response in memory. Removed unused `fetch_bytes` method.
5. **Pagination variable optimization** (`src/pagination.rs`) - Keeps `base_variables` immutable, constructs fresh `page_vars` per iteration instead of clone-and-mutate pattern.

### Command-Level Optimizations
6. **Bulk state cache** (`src/commands/bulk.rs`) - `Arc<Mutex<HashMap<String, String>>>` caches `team_id:state_name → state_id` resolution. Resolves once per unique combo instead of per-issue.
7. **Parallel comment fetching** (`src/commands/comments.rs`) - `buffer_unordered(10)` for concurrent multi-issue comment retrieval instead of sequential for loop.
8. **Trimmed notification queries** (`src/commands/notifications.rs`) - Removed unused `comment { body }`, `actor { name }`, and `ProjectNotification` fragment from list query.
9. **`print_json_owned`** (`src/output.rs`, 15+ command files) - Takes `Value` ownership to eliminate large payload cloning. `print_json` delegates to `print_json_owned` internally.
10. **O(n) sync comparison** (`src/commands/sync.rs`) - `HashMap<String, &Value>` keyed by lowercase name replaces nested `remote.iter().find()` O(n²) iteration.
11. **Round-trip reduction** (`src/commands/cycles.rs`, `src/commands/statuses.rs`) - Team name lookups use team cache instead of separate API calls.
12. **Projects & labels caching** (`src/commands/projects.rs`, `src/commands/labels.rs`) - List results cached with `CacheType::Projects`/`CacheType::Labels`. Mutation operations (`create`/`update`/`delete`) clear cache via `Cache::new()?.clear_type()`.

### Cleanup
13. **Unused code removal** (`src/api.rs`, `src/commands/templates.rs`) - Removed dead `fetch_bytes` method and unused `print_json` import.

## Files Changed
30 files, 547 insertions, 356 deletions.

## Test Results
- **126 unit tests** passing
- **32 integration tests** passing
- **0 warnings** in build output
