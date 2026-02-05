# linear-cli

[![Crates.io](https://img.shields.io/crates/v/linear-cli.svg)](https://crates.io/crates/linear-cli)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)

A fast, powerful command-line interface for [Linear](https://linear.app) built with Rust.

## Features

- **Full API Coverage** - Projects, issues, labels, teams, users, cycles, comments, documents
- **Git Integration** - Checkout branches for issues, create PRs linked to issues
- **jj (Jujutsu) Support** - First-class support for Jujutsu VCS alongside Git
- **Interactive Mode** - TUI for browsing and managing issues
- **Multiple Workspaces** - Switch between Linear workspaces seamlessly
- **Profiles & Auth** - Named profiles with `auth login/logout/status`
- **Secure Storage** - Optional OS keyring support (Keychain, Credential Manager, Secret Service)
- **Bulk Operations** - Perform actions on multiple issues at once
- **JSON/NDJSON Output** - Machine-readable output for scripting and agents
- **Smart Sorting** - Numeric and date-aware sorting (10 > 9, not "10" < "9")
- **Pagination & Filters** - `--limit`, `--page-size`, `--all`, `--filter`
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

**Available skills:**
| Skill | Description |
|-------|-------------|
| `linear-list` | List and get issues |
| `linear-create` | Create issues |
| `linear-update` | Update issues (status, priority, assignee, labels) |
| `linear-workflow` | Start/stop work, get current issue context |
| `linear-pr` | Create GitHub PRs linked to Linear issues |
| `linear-search` | Search issues and projects |
| `linear-uploads` | Download attachments and images |

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
| `issues` | `i` | Manage issues |
| `projects` | `p` | Manage projects |
| `git` | `g` | Git branch operations and PR creation |
| `search` | `s` | Search issues and projects |
| `comments` | `cm` | Manage issue comments |
| `uploads` | `up` | Fetch uploads/attachments |
| `bulk` | `b` | Bulk operations on issues |
| `labels` | `l` | Manage labels |
| `teams` | `t` | List and view teams |
| `cycles` | `c` | Manage sprint cycles |
| `relations` | `rel` | Manage issue relations (blocks, duplicates, etc.) |
| `export` | `ex` | Export issues to JSON/CSV |
| `favorites` | `fav` | Manage favorites |
| `history` | `hist` | View issue history and audit logs |
| `initiatives` | `init` | Manage initiatives |
| `metrics` | `met` | View workspace metrics |
| `roadmaps` | `rm` | Manage roadmaps |
| `triage` | `tr` | Triage responsibility management |
| `watch` | `w` | Watch issues for changes |
| `sync` | `sy` | Sync local folders with Linear |
| `interactive` | `ui` | Interactive TUI mode |
| `config` | - | CLI configuration |
| `common` | `tasks` | Common tasks and examples |
| `agent` | - | Agent-focused capabilities and examples |
| `auth` | - | API key management and status |
| `doctor` | - | Diagnose config and connectivity |
| `cache` | `ca` | Cache inspection and clearing |

Run `linear-cli <command> --help` for detailed usage.

## Common Examples

```bash
# Issues
linear-cli i list -t Engineering           # List team's issues
linear-cli i create "Bug" -t ENG -p 1      # Create urgent issue
linear-cli i update LIN-123 -s Done        # Update status
linear-cli i update LIN-123 -l bug -l urgent  # Add labels
linear-cli i update LIN-123 --due tomorrow    # Set due date
linear-cli i update LIN-123 -e 3              # Set estimate (3 points)

# Git workflow
linear-cli g checkout LIN-123              # Create branch for issue
linear-cli g pr LIN-123 --draft            # Create draft PR

# Search
linear-cli s issues "auth bug"             # Search issues

# Export
linear-cli export csv -t ENG -f issues.csv    # Export to CSV (RFC 4180)
linear-cli export markdown -t ENG             # Export to Markdown

# Relations
linear-cli rel add LIN-123 blocks LIN-456       # LIN-123 blocks LIN-456
linear-cli rel list LIN-123                     # List issue relations

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

# Store in OS keyring (requires --features secure-storage)
linear-cli auth login --secure

# Migrate existing keys to keyring
linear-cli auth migrate

# Check auth status
linear-cli auth status

# Or use environment variable
export LINEAR_API_KEY=lin_api_xxx

# Override profile per invocation
export LINEAR_CLI_PROFILE=work
```

API key priority: `LINEAR_API_KEY` env var > OS keyring > config file.

Config stored at `~/.config/linear-cli/config.toml` (Linux/macOS) or `%APPDATA%\linear-cli\config.toml` (Windows).

Cache is scoped per profile at `~/.config/linear-cli/cache/{profile}/`.

## Documentation

- [Usage Examples](docs/examples.md) - Detailed command examples
- [Workflows](docs/workflows.md) - Common workflow patterns
- [AI Agent Integration](docs/ai-agents.md) - Setup for Claude Code, Cursor, OpenAI Codex
- [JSON Samples](docs/json/README.md) - Example JSON output shapes
- [JSON Schema](docs/json/schema.json) - Schema version reference
- [Shell Completions](docs/shell-completions.md) - Tab completion setup

## Comparison with Other CLIs

| Feature | @linear/cli | linear-go | linear-cli |
|---------|---------------|-------------|--------------|
| Last updated | 2021 | 2023 | 2026 |
| Agent Skills | No | No | Yes |
| Git PR creation | No | No | Yes |
| jj (Jujutsu) support | No | No | Yes |
| Interactive TUI | No | No | Yes |
| Bulk operations | No | No | Yes |
| Multiple workspaces | No | No | Yes |
| JSON output | No | Yes | Yes |

## Contributing

Contributions welcome! Please open an issue or submit a pull request.

## License

[MIT](LICENSE)
