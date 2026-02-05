# Agent Skills

`linear-cli` includes Agent Skills for AI coding assistants. Skills provide contextual documentation that agents can load when performing Linear tasks.

## Installation

```bash
# Install all skills
npx skills add Finesssee/linear-cli

# Install specific skill
npx skills add Finesssee/linear-cli --skill linear-list

# Install globally (available in all projects)
npx skills add Finesssee/linear-cli -g
```

## Available Skills (27 total)

### Issues
| Skill | Description |
|-------|-------------|
| `linear-list` | List and get issues |
| `linear-create` | Create issues |
| `linear-update` | Update issues (status, priority, assignee, labels) |
| `linear-workflow` | Start/stop work, get current issue context |

### Git
| Skill | Description |
|-------|-------------|
| `linear-git` | Branches, checkout, context |
| `linear-pr` | Create GitHub PRs |

### Planning
| Skill | Description |
|-------|-------------|
| `linear-projects` | Manage projects |
| `linear-roadmaps` | View roadmaps |
| `linear-initiatives` | High-level tracking |
| `linear-cycles` | Sprint cycles |

### Organization
| Skill | Description |
|-------|-------------|
| `linear-teams` | Teams and users |
| `linear-labels` | Label management |
| `linear-relations` | Issue relationships (blocks, parent/child) |
| `linear-templates` | Issue templates |

### Operations
| Skill | Description |
|-------|-------------|
| `linear-bulk` | Bulk operations |
| `linear-export` | Export to CSV/Markdown |
| `linear-triage` | Triage inbox |
| `linear-favorites` | Quick access |

### Tracking
| Skill | Description |
|-------|-------------|
| `linear-metrics` | Velocity, burndown, progress |
| `linear-history` | Issue activity logs |
| `linear-time` | Time tracking |
| `linear-watch` | Watch for updates |

### Other
| Skill | Description |
|-------|-------------|
| `linear-search` | Search issues and projects |
| `linear-notifications` | Manage notifications |
| `linear-documents` | Documentation |
| `linear-uploads` | Download attachments |
| `linear-config` | Auth, API keys, workspaces, diagnostics |

## Supported Agents

Skills work with any agent that supports the [Agent Skills](https://agentskills.io) format:

- Claude Code
- OpenAI Codex
- Cursor
- Amp
- Roo Code
- Gemini CLI
- And many more

## Why Skills?

Skills are 10-50x more token-efficient than MCP tools:

- **MCP tools**: Each API call returns full JSON, uses many tokens
- **Skills**: Agent learns commands once, uses CLI directly

## Viewing Installed Skills

```bash
# List installed skills
npx skills list

# List globally installed
npx skills list -g
```

## Skill Contents

Each skill contains:

- **Frontmatter**: Name, description, allowed tools
- **Commands**: CLI commands with examples
- **Flags**: Agent-optimized flags (`--output json`, `--compact`, etc.)
- **Exit codes**: For error handling
- **Workflows**: Common task patterns

Example skill structure:
```yaml
---
name: linear-list
description: List and get Linear issues...
allowed-tools: Bash
---

# List/Get Issues

\`\`\`bash
linear-cli i list --output json
\`\`\`
```

## Updating Skills

```bash
# Check for updates
npx skills check

# Update all skills
npx skills update
```

## Removing Skills

```bash
# Remove specific skill
npx skills remove --skill linear-list

# Remove all linear-cli skills
npx skills remove Finesssee/linear-cli
```
