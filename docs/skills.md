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

## Available Skills

| Skill | Description |
|-------|-------------|
| `linear-list` | List and get issues |
| `linear-create` | Create issues |
| `linear-update` | Update issues (status, priority, assignee, labels) |
| `linear-workflow` | Start/stop work, get current issue context |
| `linear-pr` | Create GitHub PRs linked to Linear issues |
| `linear-search` | Search issues and projects |
| `linear-uploads` | Download attachments and images |

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
