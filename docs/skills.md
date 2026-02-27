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

## Available Skills (38 total)

### Issues
| Skill | Description |
|-------|-------------|
| `linear-list` | List and get issues |
| `linear-create` | Create issues |
| `linear-update` | Update issues (status, priority, assignee, labels) |
| `linear-workflow` | Start/stop work, get current issue context |
| `linear-comments` | List, create, update, delete comments |
| `linear-done` | Mark current branch's issue as Done |

### Git
| Skill | Description |
|-------|-------------|
| `linear-git` | Branches, checkout, context |
| `linear-pr` | Create GitHub PRs |

### Planning
| Skill | Description |
|-------|-------------|
| `linear-projects` | Full project CRUD (create, update, archive, labels, members) |
| `linear-project-updates` | Project status updates with health tracking |
| `linear-milestones` | Project milestones CRUD |
| `linear-roadmaps` | View roadmaps |
| `linear-initiatives` | High-level tracking |
| `linear-cycles` | Sprint cycles (list, create, update, delete, complete) |
| `linear-sprint` | Sprint planning (status, progress, burndown, velocity, carry-over) |

### Organization
| Skill | Description |
|-------|-------------|
| `linear-teams` | Teams CRUD and user management |
| `linear-labels` | Label management |
| `linear-statuses` | Workflow state management |
| `linear-relations` | Issue relationships (blocks, parent/child) |
| `linear-templates` | Local + API templates |
| `linear-views` | Custom views CRUD |

### Operations
| Skill | Description |
|-------|-------------|
| `linear-bulk` | Bulk operations |
| `linear-import` | Import from CSV/JSON |
| `linear-export` | Export to CSV/Markdown/JSON |
| `linear-triage` | Triage inbox |
| `linear-favorites` | Quick access favorites |
| `linear-attachments` | Attachment and URL management |

### Tracking
| Skill | Description |
|-------|-------------|
| `linear-metrics` | Velocity, burndown, progress |
| `linear-history` | Issue activity logs |
| `linear-time` | Time tracking |
| `linear-watch` | Watch for updates |
| `linear-webhooks` | Webhooks CRUD + local event listener |

### Advanced
| Skill | Description |
|-------|-------------|
| `linear-api` | Raw GraphQL queries and mutations |
| `linear-search` | Search issues and projects |
| `linear-notifications` | Manage notifications |
| `linear-documents` | Documentation |
| `linear-uploads` | Download attachments |
| `linear-config` | Auth (API key + OAuth), workspaces, setup, diagnostics |

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
