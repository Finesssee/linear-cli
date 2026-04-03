# Security Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Harden `linear-cli` against terminal injection, inconsistent secret display, loose listener routing, missing HTTP timeouts, and partial file writes.

**Architecture:** Keep the changes small and leverage existing shared helpers instead of redesigning the CLI. Implement in red-green-refactor slices so each hardening behavior lands with a dedicated regression test and a small commit.

**Tech Stack:** Rust, tokio, reqwest, anyhow, tempfile-style local temp files implemented with std fs primitives, existing cargo test/clippy/build workflow

---

### Task 1: Spec and Plan Scaffolding

**Files:**
- Create: `docs/superpowers/specs/2026-04-04-security-hardening-design.md`
- Create: `docs/superpowers/plans/2026-04-04-security-hardening.md`

- [ ] **Step 1: Save the approved design**

Write the concise design doc capturing scope, non-goals, and the ordered hardening slices.

- [ ] **Step 2: Save the implementation plan**

Write this plan with explicit file ownership and red-green steps.

- [ ] **Step 3: Commit**

Run:

```bash
git add docs/superpowers/specs/2026-04-04-security-hardening-design.md docs/superpowers/plans/2026-04-04-security-hardening.md
git commit -m "docs: add security hardening spec"
git push
```

### Task 2: Terminal Sanitization and Secret Masking

**Files:**
- Modify: `src/text.rs`
- Modify: `src/config.rs`

- [ ] **Step 1: Write failing tests for terminal sanitization**

Add tests proving ANSI escape sequences and unsafe control characters are removed or neutralized by shared text helpers, while preserving printable text and newlines where intended.

- [ ] **Step 2: Run targeted tests to verify red**

Run:

```bash
cargo test text::
```

Expected: new sanitization tests fail because current helpers preserve unsafe control bytes.

- [ ] **Step 3: Write failing tests for config masking**

Add tests proving human-facing config and workspace views never print short API keys in clear text unless the explicit raw path is used.

- [ ] **Step 4: Run targeted tests to verify red**

Run:

```bash
cargo test config::
```

Expected: new masking tests fail because short keys are still displayed verbatim.

- [ ] **Step 5: Implement minimal sanitization and masking**

Add a shared terminal-safe text helper in `src/text.rs` and make `truncate` / `strip_markdown` use it. Add a single API-key masking helper in `src/config.rs` and route `show_config`, `workspace_list`, and `workspace_current` through it.

- [ ] **Step 6: Run targeted tests to verify green**

Run:

```bash
cargo test sanitize_terminal_text
cargo test mask_api_key
```

- [ ] **Step 7: Commit**

```bash
git add src/text.rs src/config.rs
git commit -m "fix: harden terminal output and config masking"
git push
```

### Task 3: Exact Listener Routing and Explicit HTTP Timeouts

**Files:**
- Modify: `src/oauth.rs`
- Modify: `src/commands/webhooks.rs`
- Modify: `src/commands/update.rs`

- [ ] **Step 1: Write failing tests for exact path matching**

Add tests proving `/callback` and `/callback?...` are accepted, but `/callback/extra` is rejected; likewise `/webhook` and `/webhook?...` are accepted, but `/webhook/extra` is rejected.

- [ ] **Step 2: Run targeted tests to verify red**

Run:

```bash
cargo test callback
cargo test webhook
```

- [ ] **Step 3: Write failing tests for explicit timeout configuration**

Add tests around extracted client-builder helpers so OAuth and GitHub release-check clients use explicit request/connect timeouts.

- [ ] **Step 4: Run targeted tests to verify red**

Run:

```bash
cargo test timeout
```

- [ ] **Step 5: Implement minimal routing and timeout helpers**

Extract helper functions for exact route acceptance and security-sensitive HTTP client construction. Use the helpers from OAuth token exchange, refresh, revoke, and release-check paths.

- [ ] **Step 6: Run targeted tests to verify green**

Run:

```bash
cargo test callback
cargo test webhook
cargo test timeout
```

- [ ] **Step 7: Commit**

```bash
git add src/oauth.rs src/commands/webhooks.rs src/commands/update.rs
git commit -m "fix: harden listener routing and http timeouts"
git push
```

### Task 4: Atomic Writes for Exports and Upload Fetches

**Files:**
- Modify: `src/commands/export.rs`
- Modify: `src/commands/uploads.rs`

- [ ] **Step 1: Write failing tests for atomic file destinations**

Add tests that verify file-destination helpers write through a sibling temp path and only replace the final destination on success.

- [ ] **Step 2: Run targeted tests to verify red**

Run:

```bash
cargo test atomic
```

- [ ] **Step 3: Implement minimal atomic write helpers**

Create focused helpers that:
- create temp files in the destination directory
- apply `0600` permissions on Unix
- flush and sync data before rename
- rename into place only after a successful write

Apply them to export file outputs and upload fetches, leaving stdout flows unchanged.

- [ ] **Step 4: Run targeted tests to verify green**

Run:

```bash
cargo test atomic
```

- [ ] **Step 5: Commit**

```bash
git add src/commands/export.rs src/commands/uploads.rs
git commit -m "fix: harden export and upload file writes"
git push
```

### Task 5: Final Verification and Release

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Update any touched release notes or docs only if the code change requires it

- [ ] **Step 1: Run full verification**

Run:

```bash
cargo fmt --check
cargo test
cargo clippy -- -D warnings
cargo build --release
```

- [ ] **Step 2: Bump patch version**

Update the crate version from `0.3.18` to the next patch version and refresh `Cargo.lock`.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version for security hardening release"
git push
```

- [ ] **Step 4: Publish and release**

Follow the existing local manual-release flow used for `v0.3.18`, including crates.io publish, local asset builds, GitHub release upload, and live smoke checks.
