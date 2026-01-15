# linear-cli

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)

A fast, powerful command-line interface for [Linear](https://linear.app) built with Rust.

## Features

- **Full API Coverage** - Projects, issues, labels, teams, users, cycles, comments, documents
- **Git Integration** - Checkout branches for issues, create PRs linked to issues
- **jj (Jujutsu) Support** - First-class support for Jujutsu VCS alongside Git
- **Local Sync** - Sync local project folders with Linear
- **Search** - Find issues and projects instantly
- **Interactive Mode** - TUI for browsing and managing issues interactively
- **Multiple Workspaces** - Switch between Linear workspaces seamlessly
- **Bulk Operations** - Perform actions on multiple issues at once
- **JSON Output** - Machine-readable output for scripting and automation
- **Shell Completions** - Tab completions for Bash, Zsh, Fish, and PowerShell
- **Fast** - Native Rust binary, no runtime dependencies

## Installation

### From crates.io (Recommended)

```bash
cargo install linear-cli
```

### From Source

```bash
git clone https://github.com/Finesssee/linear-cli.git
cd linear-cli
cargo build --release
```

### Pre-built Binaries

Download from [GitHub Releases](https://github.com/Finesssee/linear-cli/releases).

### Add to PATH

```bash
# Linux/macOS
sudo cp target/release/linear-cli /usr/local/bin/

# Or add to your shell profile
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc

# Windows (PowerShell as Admin)
Copy-Item target\release\linear-cli.exe C:\Windows\System32\

# Or add to User PATH (no admin required)
$env:Path += ";$HOME\.cargo\bin"
```

## Quick Start

```bash
# 1. Configure your API key (get one at https://linear.app/settings/api)
linear-cli config set-key lin_api_xxxxxxxxxxxxx

# 2. List your projects
linear-cli projects list

# 3. Create an issue
linear-cli issues create "Fix login bug" --team Engineering --priority 2
```

## Commands

| Command | Alias | Description |
|---------|-------|-------------|
| `projects` | `p` | Manage projects |
| `issues` | `i` | Manage issues |
| `labels` | `l` | Manage labels |
| `teams` | `t` | List and view teams |
| `users` | `u` | List and view users |
| `cycles` | `c` | Manage sprint cycles |
| `comments` | `cm` | Manage issue comments |
| `documents` | `d` | Manage documents |
| `search` | `s` | Search issues and projects |
| `sync` | `sy` | Sync local folders with Linear |
| `statuses` | `st` | View issue statuses |
| `git` | `g` | Git branch operations and PR creation |
| `jj` | `j` | Jujutsu VCS operations |
| `uploads` | `up` | Fetch uploads/attachments from Linear |
| `workspace` | `ws` | Manage multiple workspaces |
| `interactive` | `ui` | Interactive TUI mode |
| `bulk` | `b` | Bulk operations on issues |
| `completions` | - | Generate shell completions |
| `config` | - | CLI configuration |

## Usage Examples

### Projects

```bash
linear-cli p list                              # List all projects
linear-cli p list --archived                   # Include archived
linear-cli p get PROJECT_ID                    # View project details
linear-cli p create "Q1 Roadmap" -t Engineering
linear-cli p update PROJECT_ID --name "New Name"
linear-cli p delete PROJECT_ID --force
linear-cli p add-labels PROJECT_ID LABEL_ID
```

### Issues

```bash
linear-cli i list                              # List issues
linear-cli i list -t Engineering -s "In Progress"
linear-cli i list --output json                # Output as JSON
linear-cli i get LIN-123                       # View issue details
linear-cli i get LIN-123 --output json         # JSON output
linear-cli i create "Bug fix" -t Eng -p 1      # Priority: 1=urgent, 4=low
linear-cli i update LIN-123 -s Done
linear-cli i delete LIN-123 --force
linear-cli i start LIN-123                     # Start working: assigns to you, sets In Progress, creates branch
linear-cli i stop LIN-123                      # Stop working: unassigns, resets status
```

### Labels

```bash
linear-cli l list                              # List project labels
linear-cli l list --type issue                 # List issue labels
linear-cli l create "Feature" --color "#10B981"
linear-cli l create "Bug" --type issue --color "#EF4444"
linear-cli l delete LABEL_ID --force
```

### Git Integration

```bash
linear-cli g checkout LIN-123                  # Create/checkout branch for issue
linear-cli g branch LIN-123                    # Show branch name for issue
linear-cli g create LIN-123                    # Create branch without checkout
linear-cli g checkout LIN-123 -b custom-branch # Use custom branch name
linear-cli g pr LIN-123                        # Create PR linked to issue
linear-cli g pr LIN-123 --draft                # Create draft PR
linear-cli g pr LIN-123 --base main            # Specify base branch
```

### jj (Jujutsu) Integration

```bash
linear-cli j checkout LIN-123                  # Create bookmark for issue
linear-cli j bookmark LIN-123                  # Show bookmark name for issue
linear-cli j create LIN-123                    # Create bookmark without checkout
linear-cli j pr LIN-123                        # Create PR using jj git push
```

### Sync Local Folders

```bash
linear-cli sy status                           # Compare local folders with Linear
linear-cli sy push -t Engineering              # Create Linear projects for local folders
linear-cli sy push -t Engineering --dry-run    # Preview without creating
```

### Search

```bash
linear-cli s issues "authentication bug"
linear-cli s projects "backend" --limit 10
```

### Uploads

Download attachments and images from Linear issues/comments:

```bash
# Download to file
linear-cli up fetch "https://uploads.linear.app/..." -o image.png

# Output to stdout (for piping to other tools)
linear-cli up fetch "https://uploads.linear.app/..." | base64

# Useful for AI agents that need to view images
linear-cli uploads fetch URL -o /tmp/screenshot.png
```

### Other Commands

```bash
# Teams
linear-cli t list
linear-cli t get TEAM_ID

# Users
linear-cli u list
linear-cli u get me

# Cycles
linear-cli c list -t Engineering
linear-cli c current -t Engineering

# Comments
linear-cli cm list ISSUE_ID
linear-cli cm create ISSUE_ID -b "This is a comment"

# Documents
linear-cli d list
linear-cli d get DOC_ID
linear-cli d create "Doc Title" -p PROJECT_ID

# Statuses
linear-cli st list -t Engineering
linear-cli st get "In Progress" -t Engineering

# Config
linear-cli config set-key YOUR_API_KEY
linear-cli config show
```

### Interactive Mode

```bash
linear-cli ui                                  # Launch interactive TUI
linear-cli ui issues                           # Browse issues interactively
linear-cli ui projects                         # Browse projects interactively
linear-cli interactive --team Engineering      # Filter by team
```

### Multiple Workspaces

```bash
linear-cli ws list                             # List configured workspaces
linear-cli ws add personal                     # Add a new workspace
linear-cli ws switch personal                  # Switch active workspace
linear-cli ws current                          # Show current workspace
linear-cli ws remove personal                  # Remove a workspace
```

### Bulk Operations

```bash
linear-cli b update -s Done LIN-1 LIN-2 LIN-3  # Update multiple issues
linear-cli b assign --user me LIN-1 LIN-2      # Assign multiple issues
linear-cli b label --add bug LIN-1 LIN-2       # Add label to multiple issues
linear-cli b move --project "Q1" LIN-1 LIN-2   # Move issues to project
linear-cli b delete --force LIN-1 LIN-2 LIN-3  # Delete multiple issues
```

### Shell Completions

Enable tab completions for your shell:

**Bash:**
```bash
# Create completions directory if needed
mkdir -p ~/.bash_completion.d

# Generate and install completions
linear-cli completions bash > ~/.bash_completion.d/linear-cli

# Add to ~/.bashrc if not already present
echo 'source ~/.bash_completion.d/linear-cli' >> ~/.bashrc
source ~/.bashrc
```

**Zsh:**
```bash
# Create completions directory if needed  
mkdir -p ~/.zsh/completions

# Generate completions
linear-cli completions zsh > ~/.zsh/completions/_linear-cli

# Add to ~/.zshrc if not already present
echo 'fpath=(~/.zsh/completions $fpath)' >> ~/.zshrc
echo 'autoload -Uz compinit && compinit' >> ~/.zshrc
source ~/.zshrc
```

**Fish:**
```bash
# Generate and install completions
linear-cli completions fish > ~/.config/fish/completions/linear-cli.fish
```

**PowerShell:**
```powershell
# Generate completions
linear-cli completions powershell > $HOME\linear-cli.ps1

# Add to your PowerShell profile
Add-Content $PROFILE '. $HOME\linear-cli.ps1'
```

### JSON Output

```bash
# Use --output json with any list or get command
linear-cli i list --output json
linear-cli p list --output json | jq '.[] | .name'
linear-cli i get LIN-123 --output json
linear-cli t list --output json
linear-cli cm list ISSUE_ID --output json    # Comments as JSON (great for LLMs)
```

## Example Workflows

### Daily Workflow

```bash
# Start your day - check your assigned issues
linear-cli i list --assignee me

# Pick an issue and start working on it
linear-cli i start LIN-123 --checkout

# ... do your work ...

# Update the issue status when done
linear-cli i update LIN-123 -s Done
```

### Creating and Managing Issues

```bash
# Create a new bug report
linear-cli i create "Login button not working" -t ENG -p 2 -s "Backlog"

# Add a label to the issue
linear-cli bulk label "Bug" -i LIN-456

# Assign it to yourself and start working
linear-cli i start LIN-456 --checkout

# Add a comment with your findings
linear-cli cm create LIN-456 -b "Root cause: Missing null check in auth handler"

# Mark as done when fixed
linear-cli i update LIN-456 -s Done
```

### Git Integration Workflow

```bash
# Start working on an issue (assigns to you, sets "In Progress")
linear-cli i start LIN-123 --checkout

# ... make your changes ...

# Create a PR linked to the issue
linear-cli g pr LIN-123

# Or create a draft PR
linear-cli g pr LIN-123 --draft --web
```

### Project Setup Workflow

```bash
# Compare local code folders with Linear projects
linear-cli sy status

# Create Linear projects for folders that don't exist
linear-cli sy push -t ENG --dry-run    # Preview first
linear-cli sy push -t ENG              # Create projects

# Add labels to organize projects
linear-cli p add-labels PROJECT_ID LABEL1 LABEL2
```

## Configuration

Configuration is stored at:
- **Linux/macOS:** `~/.config/linear-cli/config.toml`
- **Windows:** `%APPDATA%\linear-cli\config.toml`

### Environment Variable Override

You can override the configured API key using the `LINEAR_API_KEY` environment variable. This is useful for:

- **CI/CD pipelines** - Set API key via environment without modifying config files
- **Multi-workspace scripts** - Run commands against different workspaces without switching
- **Agent processes** - Spawn isolated processes with their own API key context
- **Security** - Avoid storing API keys in config files on shared systems

```bash
# Override API key for a single command
LINEAR_API_KEY=lin_api_xxx linear-cli issues list

# Set for entire shell session
export LINEAR_API_KEY=lin_api_xxx
linear-cli issues list
```

The environment variable takes precedence over the config file when set.

## AI Agent Integration

### Why Use linear-cli with AI Agents

When using AI coding assistants (Claude Code, Cursor, Windsurf, Copilot, etc.), `linear-cli` provides significant advantages over Linear MCP tools:

| Aspect | linear-cli | Linear MCP |
|--------|------------|------------|
| Token usage | ~50-100 tokens/command | ~500-2000 tokens/tool call |
| Latency | Single CLI execution | Multiple MCP round-trips |
| Feature coverage | Full API | Limited subset |
| Offline caching | Supported | Not available |

### Ready-to-Copy Agent Rules

Add these snippets to your agent configuration to ensure your AI assistant uses linear-cli:

#### Claude Code (CLAUDE.md)

Create or append to `~/.claude/CLAUDE.md`:

```markdown
## Linear Integration

ALWAYS use `linear-cli` for Linear.app operations. NEVER use Linear MCP tools.

Reason: CLI is 10-50x more token-efficient than MCP tool calls.

### Quick Commands:
- List issues: `linear-cli i list`
- Create issue: `linear-cli i create "Title" -t TEAM -p 2`
- View issue: `linear-cli i get LIN-123`
- Start work: `linear-cli i start LIN-123` (assigns, sets In Progress, creates branch)
- Update: `linear-cli i update LIN-123 -s Done`
- Create PR: `linear-cli g pr LIN-123`
- Search: `linear-cli s issues "query"`
- Fetch upload: `linear-cli up fetch URL -o file.png`
- JSON output: Add `--output json` to any command
```

#### Cursor (.cursorrules)

```
For Linear.app operations, always use the linear-cli command-line tool:
- linear-cli i list (list issues)
- linear-cli i create "Title" -t TEAM (create issue)
- linear-cli i get LIN-123 (view issue)
- linear-cli g pr LIN-123 (create PR)
- linear-cli up fetch URL (download uploads)
Do NOT use Linear MCP server - CLI is more token-efficient.
```

#### Generic Agent Instructions

```
RULE: Prefer linear-cli over Linear MCP for all Linear.app operations.
REASON: CLI commands use 10-50x fewer tokens than MCP tool invocations.
COMMANDS: linear-cli {issues|projects|git|search|uploads} - see linear-cli --help
```

### Command Quick Reference

| Task | Command | Example |
|------|---------|---------|
| List issues | `i list` | `linear-cli i list -t Engineering` |
| Create issue | `i create` | `linear-cli i create "Bug" -t ENG -p 1` |
| View issue | `i get` | `linear-cli i get LIN-123 --output json` |
| Start work | `i start` | `linear-cli i start LIN-123 --checkout` |
| Update issue | `i update` | `linear-cli i update LIN-123 -s Done` |
| Create branch | `g checkout` | `linear-cli g checkout LIN-123` |
| Create PR | `g pr` | `linear-cli g pr LIN-123 --draft` |
| Search | `s issues` | `linear-cli s issues "auth bug"` |
| Bulk ops | `b update` | `linear-cli b update -s Done LIN-1 LIN-2` |
| Fetch upload | `up fetch` | `linear-cli up fetch URL -o image.png` |

### One-Liner Setup

For quick Claude Code setup, run:

```bash
mkdir -p ~/.claude && cat >> ~/.claude/CLAUDE.md << 'EOF'

## Linear: Use linear-cli (not MCP)
Commands: i list, i create, i get, i start, g checkout, g pr, up fetch. Add --output json for parsing.
EOF
```

## Comparison with Other CLIs

| Feature | `@linear/cli` | `linear-go` | `linear-cli` |
|---------|---------------|-------------|--------------|
| Last updated | 2021 | 2023 | 2025 |
| Create issues | ✓ | ✓ | ✓ |
| Git checkout | ✓ | ✓ | ✓ |
| Git PR creation | ✗ | ✗ | ✓ |
| jj (Jujutsu) support | ✗ | ✗ | ✓ |
| List/manage projects | ✗ | ✓ | ✓ |
| CRUD for all resources | ✗ | ✗ | ✓ |
| Search | ✗ | ✓ | ✓ |
| Local folder sync | ✗ | ✗ | ✓ |
| Issue statuses | ✗ | ✗ | ✓ |
| Documents | ✗ | ✗ | ✓ |
| Issue start/stop workflow | ✗ | ✗ | ✓ |
| Interactive TUI mode | ✗ | ✗ | ✓ |
| Multiple workspaces | ✗ | ✗ | ✓ |
| Bulk operations | ✗ | ✗ | ✓ |
| JSON output | ✗ | ✓ | ✓ |
| Shell completions | ✗ | ✓ | ✓ |

## Contributing

Contributions are welcome! Please open an issue or submit a pull request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

[MIT](LICENSE)
