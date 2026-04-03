# Security Hardening Design

**Date:** 2026-04-04

**Goal**

Ship a runtime hardening release for `linear-cli` that reduces exposure around untrusted network input, local secret display, partial file writes, and unsafe terminal rendering without changing the core CLI surface.

**Approved Scope**

This design follows the user-approved `opt 3` full sweep, but keeps the implementation bounded to the highest-leverage areas already identified in the codebase:

- explicit HTTP timeouts for OAuth token exchange, refresh, revoke, and GitHub release checks
- exact listener path matching for OAuth callbacks and webhook posts
- consistent secret masking in human-facing config and workspace displays
- atomic writes for exports and upload downloads when writing to disk
- shared sanitization for untrusted terminal text, plus targeted fixes where raw strings bypass shared helpers

**Non-Goals**

- changing auth flows, scopes, or credential storage backends
- redesigning the updater or replacing Cargo-based install paths
- introducing sandboxing or signature verification for updates
- broad output-format rewrites for machine-readable modes

**Approach**

The hardening work will be implemented in narrow slices with tests first:

1. Shared text sanitization helpers
2. Secret masking helpers and config display fixes
3. Exact path matching in local listeners
4. Explicit timeout configuration for outbound security-sensitive HTTP clients
5. Atomic write helpers for export and upload paths
6. Targeted command output call sites that still print untrusted text directly

This order starts with the lowest-risk shared behavior, then moves outward into network and file-system boundaries.

**Architecture Notes**

- `src/text.rs` becomes the shared home for terminal-safe text helpers. Existing helpers such as `truncate` and `strip_markdown` should sanitize control characters so most command output benefits automatically.
- `src/config.rs` should centralize API-key masking in a single helper instead of duplicating slightly different display rules.
- `src/oauth.rs`, `src/commands/webhooks.rs`, and `src/commands/update.rs` should share the same security posture for exact route handling and explicit request timeouts.
- `src/commands/export.rs` and `src/commands/uploads.rs` should use temp-file-and-rename writes for file destinations, while leaving stdout behavior unchanged.

**Testing Strategy**

- Add failing unit tests for each behavior change before implementation.
- Prefer pure helper tests where possible.
- Use focused command-path tests for listener route matching and output masking.
- Run full Rust verification after the slices land: formatting, tests, clippy, and release build.

**Release Outcome**

This work should ship as the next patch release after `0.3.18`, with release notes clearly separating runtime hardening improvements from the earlier documentation-only security release.
