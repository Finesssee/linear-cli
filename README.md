# linear-cli

[![Crates.io](https://img.shields.io/crates/v/linear-cli.svg)](https://crates.io/crates/linear-cli)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)

A fast, powerful command-line interface for [Linear](https://linear.app) built with Rust.

## Features

- **Full API Coverage** - Issues, projects, labels, teams, users, cycles, comments, documents, milestones, roadmaps, initiatives, webhooks, custom views
- **OAuth 2.0** - Browser-based PKCE auth with auto-refresh, plus API key support
- **Issue Workflow** - Start, stop, close, archive, assign, move, transfer, comment, link
- **Git Integration** - Checkout branches for issues, create PRs linked to issues
- **jj (Jujutsu) Support** - First-class support for Jujutsu VCS alongside Git
- **Interactive Mode** - TUI for browsing and managing issues with assignee, status, label selection
- **Multiple Workspaces** - Switch between Linear workspaces seamlessly
- **Profiles & Auth** - Named profiles with `auth login/logout/status/oauth/revoke`
- **Secure Storage** - Optional OS keyring support (Keychain, Credential Manager, Secret Service)
- **Bulk Operations** - Perform actions on multiple issues at once
- **JSON/NDJSON Output** - Machine-readable output for scripting and agents
- **Smart Sorting** - Numeric and date-aware sorting (10 > 9, not "10" < "9")
- **Pagination & Filters** - `--limit`, `--page-size`, `--all`, `--filter`, `--since`, `--mine`, `--label`
- **Grouping** - `--group-by` for kanban-style output by state, priority, assignee, or project
- **Webhooks** - Full CRUD + real-time listener with HMAC-SHA256 signature verification
- **Raw GraphQL** - `api query` and `api mutate` for direct API access
- **Activity History** - `--history` and `--comments` flags on issue details
- **Markdown Stripping** - Clean terminal display of issue descriptions
- **Auto-Paging** - Output auto-pages through `less` on Unix terminals
- **Reliable** - HTTP timeouts, jittered retries, atomic cache writes
- **Diagnostics** - `doctor` command for config and connectivity checks
- **Fast** - Native Rust binary, no runtime dependencies

## Installation

```bash
# From crates.io
cargo install linear-cli

# With secure storage (OS keyring support)
cargo install linear-cli --features secure-storage

# From source
git clone https://github.com/Finesssee/linear-cli.git
cd linear-cli && cargo build --release
```

Pre-built binaries available at [GitHub Releases](https://github.com/Finesssee/linear-cli/releases).

## Agent Skills

**linear-cli includes Agent Skills** for AI coding assistants (Claude Code, Cursor, Codex, etc.).

```bash
# Install all skills for your AI agent
npx skills add Finesssee/linear-cli

# Or install specific skills
npx skills add Finesssee/linear-cli --skill linear-list
npx skills add Finesssee/linear-cli --skill linear-workflow
```

**27 skills covering all CLI features:**

| Category | Skills |
|----------|--------|
| **Issues** | `linear-list`, `linear-create`, `linear-update`, `linear-workflow` |
| **Git** | `linear-git`, `linear-pr` |
| **Planning** | `linear-projects`, `linear-roadmaps`, `linear-initiatives`, `linear-cycles` |
| **Organization** | `linear-teams`, `linear-labels`, `linear-relations`, `linear-templates` |
| **Operations** | `linear-bulk`, `linear-export`, `linear-triage`, `linear-favorites` |
| **Tracking** | `linear-metrics`, `linear-history`, `linear-time`, `linear-watch` |
| **Other** | `linear-search`, `linear-notifications`, `linear-documents`, `linear-uploads`, `linear-config` |

Skills are 10-50x more token-efficient than MCP tools.

## Quick Start

```bash
# 1. Configure your API key (get one at https://linear.app/settings/api)
linear-cli config set-key lin_api_xxxxxxxxxxxxx

# 2. List your issues
linear-cli i list

# 3. Start working on an issue (assigns, sets In Progress, creates branch)
linear-cli i start LIN-123 --checkout

# 4. Create a PR when done
linear-cli g pr LIN-123
```

## Commands

| Command | Alias | Description |
|---------|-------|-------------|
| `issues` | `i` | Manage issues (list, create, update, start, stop, close, assign, move, transfer, comment, link, archive) |
| `projects` | `p` | Manage projects (list, get, create, update, delete, members, add-labels) |
| `git` | `g` | Git branch operations and PR creation |
| `search` | `s` | Search issues and projects |
| `comments` | `cm` | Manage issue comments |
| `uploads` | `up` | Fetch uploads/attachments |
| `bulk` | `b` | Bulk operations on issues |
| `labels` | `l` | Manage labels (list, create, update, delete) |
| `teams` | `t` | List, view, and list team members |
| `users` | `u` | List users, view profile, get user details |
| `cycles` | `c` | Manage sprint cycles (list, get, current, create, update) |
| `milestones` | `ms` | Manage project milestones (list, get, create, update, delete) |
| `relations` | `rel` | Manage issue relations (blocks, duplicates, etc.) |
| `export` | `ex` | Export issues to JSON/CSV/Markdown |
| `favorites` | `fav` | Manage favorites |
| `history` | `hist` | View issue history and audit logs |
| `initiatives` | `init` | Manage initiatives (list, get, create, update) |
| `roadmaps` | `rm` | Manage roadmaps (list, get, create, update) |
| `metrics` | `met` | View workspace metrics |
| `notifications` | `n` | Manage notifications (list, read, archive, count) |
| `documents` | `doc` | Manage documents (list, get, create, update, delete) |
| `views` | `v` | Manage custom views (list, get, create, update, delete, apply) |
| `webhooks` | `wh` | Manage webhooks (list, get, create, update, delete, rotate-secret, listen) |
| `watch` | `w` | Watch issues/projects/teams for changes (polling) |
| `triage` | `tr` | Triage responsibility management |
| `sync` | `sy` | Sync local folders with Linear |
| `interactive` | `ui` | Interactive TUI mode |
| `api` | - | Raw GraphQL queries and mutations |
| `whoami` | - | Show current authenticated user |
| `auth` | - | Authentication (login, logout, oauth, revoke, status) |
| `config` | - | CLI configuration |
| `doctor` | - | Diagnose config and connectivity |
| `cache` | `ca` | Cache inspection and clearing |
| `time` | `tm` | Time tracking |
| `templates` | `tpl` | Manage issue templates |
| `context` | `ctx` | Detect current Linear issue from git branch |
| `common` | `tasks` | Common tasks and examples |
| `agent` | - | Agent-focused capabilities and examples |

Run `linear-cli <command> --help` for detailed usage.

## Common Examples

```bash
# Issues
linear-cli i list -t Engineering           # List team's issues
linear-cli i list --mine                   # List my issues
linear-cli i list --since 7d               # Issues from last 7 days
linear-cli i list --group-by state         # Group by status (kanban style)
linear-cli i list --count-only             # Just show count
linear-cli i create "Bug" -t ENG -p 1      # Create urgent issue
linear-cli i update LIN-123 -s Done        # Update status
linear-cli i update LIN-123 -l bug -l urgent  # Add labels
linear-cli i update LIN-123 --due tomorrow    # Set due date
linear-cli i update LIN-123 -e 3              # Set estimate (3 points)
linear-cli i get LIN-123 --history         # Show activity timeline
linear-cli i get LIN-123 --comments        # Show inline comments
linear-cli i assign LIN-123 "Alice"        # Assign to user
linear-cli i assign LIN-123               # Unassign
linear-cli i move LIN-123 "Q2 Project"     # Move to project
linear-cli i transfer LIN-123 ENG          # Transfer to team
linear-cli i comment LIN-123 -b "LGTM"    # Add comment
linear-cli i close LIN-123                 # Mark as done
linear-cli i open LIN-123                  # Open in browser
linear-cli i link LIN-123                  # Print URL

# Git workflow
linear-cli g checkout LIN-123              # Create branch for issue
linear-cli g pr LIN-123 --draft            # Create draft PR

# OAuth authentication
linear-cli auth oauth                      # Browser-based OAuth login
linear-cli auth status                     # Show auth type and token info
linear-cli auth revoke                     # Revoke OAuth tokens

# Search
linear-cli s issues "auth bug"             # Search issues

# Teams & Users
linear-cli t members ENG                   # List team members
linear-cli u get "alice@example.com"       # Look up a user
linear-cli whoami                          # Show current user

# Projects
linear-cli p members "Q1 Roadmap"          # List project members
linear-cli p open "Q1 Roadmap"             # Open in browser

# Cycles
linear-cli c get CYCLE_ID                  # Cycle details with issues
linear-cli c current -t ENG               # Show current cycle

# Milestones
linear-cli ms list -p "Q1 Roadmap"         # List project milestones
linear-cli ms create "Beta" -p PROJECT_ID  # Create milestone

# Labels
linear-cli l update LABEL_ID -n "Renamed"  # Rename a label
linear-cli l update LABEL_ID -c "#FF5733"  # Change label color

# Webhooks
linear-cli wh create https://hook.example.com  # Create webhook
linear-cli wh listen --port 8080               # Listen for events

# Raw GraphQL
linear-cli api query '{ viewer { name } }'     # Raw query
linear-cli api mutate 'mutation { ... }'       # Raw mutation

# Export
linear-cli export csv -t ENG -f issues.csv    # Export to CSV (RFC 4180)
linear-cli export markdown -t ENG             # Export to Markdown

# JSON output (great for AI agents)
linear-cli i get LIN-123 --output json --compact
linear-cli i list --output json --fields identifier,title,state.name
linear-cli cm list ISSUE_ID --output ndjson

# Pagination + filters
linear-cli i list --limit 25 --sort identifier
linear-cli i list --all --page-size 100 --filter state.name=In\ Progress

# Template output
linear-cli i list --format "{{identifier}} {{title}}"

# Profiles
linear-cli --profile work auth login
linear-cli --profile work i list

# Disable color for logs/CI
linear-cli i list --no-color
```

See [docs/examples.md](docs/examples.md) for comprehensive examples.

## Configuration

```bash
# Set API key (stored in config file)
linear-cli config set-key YOUR_API_KEY

# Or use auth login
linear-cli auth login

# OAuth 2.0 (browser-based, auto-refreshing tokens)
linear-cli auth oauth

# Store in OS keyring (requires --features secure-storage)
linear-cli auth login --secure

# Migrate existing keys to keyring
linear-cli auth migrate

# Check auth status (shows auth type, token expiry)
linear-cli auth status

# Revoke OAuth tokens
linear-cli auth revoke

# Or use environment variable
export LINEAR_API_KEY=lin_api_xxx

# Override profile per invocation
export LINEAR_CLI_PROFILE=work
```

Auth priority: `LINEAR_API_KEY` env var > OS keyring > OAuth tokens > config file API key.

Config stored at `~/.config/linear-cli/config.toml` (Linux/macOS) or `%APPDATA%\linear-cli\config.toml` (Windows).

Cache is scoped per profile at `~/.config/linear-cli/cache/{profile}/`.

## Documentation

- [Agent Skills](docs/skills.md) - 27 skills for AI agents
- [AI Agent Integration](docs/ai-agents.md) - Setup for Claude Code, Cursor, OpenAI Codex
- [Usage Examples](docs/examples.md) - Detailed command examples
- [Workflows](docs/workflows.md) - Common workflow patterns
- [JSON Samples](docs/json/README.md) - Example JSON output shapes
- [JSON Schema](docs/json/schema.json) - Schema version reference
- [Shell Completions](docs/shell-completions.md) - Tab completion setup

## Comparison with Other CLIs

| Feature | @linear/cli | linear-go | linear-cli |
|---------|---------------|-------------|--------------|
| Last updated | 2021 | 2023 | 2026 |
| Agent Skills | No | No | **27 skills** |
| OAuth 2.0 (PKCE) | No | No | Yes |
| Issue workflow actions | No | No | assign, move, transfer, close, archive, comment |
| Activity history | No | No | --history, --comments |
| Webhooks + listener | No | No | CRUD + HMAC-SHA256 listener |
| Custom views | No | No | Full CRUD + apply |
| Milestones | No | No | Full CRUD |
| Raw GraphQL API | No | No | Yes |
| Auto-paging output | No | No | Yes |
| Git PR creation | No | No | Yes |
| jj (Jujutsu) support | No | No | Yes |
| Interactive TUI | No | No | Yes |
| Bulk operations | No | No | Yes |
| Multiple workspaces | No | No | Yes |
| JSON output | No | Yes | Yes |
| 40+ commands | No | No | Yes |

## Contributing

Contributions welcome! Please open an issue or submit a pull request.

## License

[MIT](LICENSE)
