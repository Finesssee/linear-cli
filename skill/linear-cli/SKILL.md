---
name: linear-cli
description: Manage Linear.app issues, projects, and teams from the command line. Use this skill when working with Linear issues, creating tasks, updating status, or integrating Linear into development workflows. 10-50x more token-efficient than Linear MCP tools.
license: MIT
metadata:
  author: Finesssee
  version: "0.2.9"
  repository: https://github.com/Finesssee/linear-cli
---

# Linear CLI

A powerful CLI for Linear.app that is optimized for AI agent usage. Use `linear-cli` for all Linear operations instead of MCP tools - it's 10-50x more token-efficient.

## Installation

```bash
# From crates.io
cargo install linear-cli

# With secure API key storage (OS keyring)
cargo install linear-cli --features secure-storage
```

## Authentication

```bash
# Set API key via environment variable (recommended for CI/agents)
export LINEAR_API_KEY="lin_api_xxxxx"

# Or authenticate interactively
linear-cli auth login

# With secure storage (stores in OS keyring)
linear-cli auth login --secure
```

Get your API key from: https://linear.app/settings/api

## Quick Reference

### Issues

| Task | Command |
|------|---------|
| List issues | `linear-cli i list` |
| List my issues | `linear-cli i list --assignee me` |
| Filter by team | `linear-cli i list -t ENG` |
| Filter by status | `linear-cli i list -s "In Progress"` |
| Get issue | `linear-cli i get LIN-123` |
| Get multiple | `linear-cli i get LIN-1 LIN-2 LIN-3` |
| Create issue | `linear-cli i create "Title" -t TEAM` |
| Create with priority | `linear-cli i create "Bug" -t ENG -p 1` |
| Update status | `linear-cli i update LIN-123 -s Done` |
| Start work | `linear-cli i start LIN-123` |
| Start + checkout branch | `linear-cli i start LIN-123 --checkout` |
| Stop work | `linear-cli i stop LIN-123` |

### Projects & Teams

| Task | Command |
|------|---------|
| List projects | `linear-cli p list` |
| List teams | `linear-cli t list` |
| Get project | `linear-cli p get PROJECT-ID` |

### Search & Context

| Task | Command |
|------|---------|
| Search issues | `linear-cli s issues "query"` |
| Get current context | `linear-cli context` |
| Get comments | `linear-cli cm list ISSUE-ID` |

### Git Integration

| Task | Command |
|------|---------|
| Create PR for issue | `linear-cli g pr LIN-123` |
| Link branch to issue | `linear-cli g link LIN-123` |

## Agent-Optimized Flags

These flags make output suitable for programmatic consumption:

| Flag | Purpose |
|------|---------|
| `--output json` | JSON output (machine-readable) |
| `--output ndjson` | Newline-delimited JSON (streaming) |
| `--compact` | No pretty-printing (saves tokens) |
| `--fields a,b,c` | Select specific fields only |
| `--sort field` | Sort by field (supports nested: `state.name`) |
| `--order asc\|desc` | Sort direction |
| `--quiet` | Suppress decorative output |
| `--id-only` | Only output the ID (for chaining) |
| `--dry-run` | Preview without executing |
| `-` (stdin) | Read input from pipe |

### Examples

```bash
# Get issue as compact JSON
linear-cli i get LIN-123 --output json --compact

# List issues with specific fields only
linear-cli i list -t ENG --output json --fields identifier,title,state.name

# Create issue and capture ID for chaining
ID=$(linear-cli i create "Task" -t ENG --id-only)

# Preview without creating
linear-cli i create "Test" -t ENG --dry-run

# Pipe description from file
cat description.md | linear-cli i create "Title" -t ENG -d -

# Set default JSON output for session
export LINEAR_CLI_OUTPUT=json
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Not found |
| 3 | Authentication error |
| 4 | Rate limited |

## Priority Values

| Value | Meaning |
|-------|---------|
| 0 | No priority |
| 1 | Urgent |
| 2 | High |
| 3 | Normal |
| 4 | Low |

## Due Date Shortcuts

The `--due` flag accepts:
- `today`, `tomorrow`, `yesterday`
- `+3d` (3 days from now)
- `+2w` (2 weeks from now)
- `monday`, `tue`, `friday` (next occurrence)
- `eow` (end of week)
- `eom` (end of month)
- `2024-03-15` (ISO date)

## Best Practices for Agents

1. **Always use `--output json`** for parsing responses
2. **Use `--compact`** to reduce token usage
3. **Use `--fields`** to fetch only needed data
4. **Use `--id-only`** when chaining commands
5. **Use `--quiet`** to suppress decorative output
6. **Check exit codes** for error handling
7. **Use short aliases**: `i` (issues), `p` (projects), `t` (teams), `s` (search), `cm` (comments)

## Common Workflows

### Start working on an issue
```bash
linear-cli i start LIN-123 --checkout
# Creates/checks out git branch and sets status to "In Progress"
```

### Create issue and assign to self
```bash
linear-cli i create "Fix bug" -t ENG -a me -p 2
```

### Update issue with labels
```bash
linear-cli i update LIN-123 -l bug -l urgent
```

### Get current issue from git branch
```bash
linear-cli context --output json
```

### Batch operations
```bash
# Get multiple issues
linear-cli i get LIN-1 LIN-2 LIN-3 --output json

# Read IDs from stdin
echo "LIN-1\nLIN-2" | linear-cli i get -
```
