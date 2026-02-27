---
name: linear-projects
description: Manage Linear projects - full CRUD with labels, members, archive. Use when managing projects.
allowed-tools: Bash
---

# Projects

```bash
# List projects
linear-cli p list                    # All projects
linear-cli p list --archived         # Include archived
linear-cli p list --view "Active"    # Apply saved view

# Get project
linear-cli p get PROJECT_ID
linear-cli p open PROJECT_ID        # Open in browser

# Create project (full API fields)
linear-cli p create "Q1 Roadmap" -t ENG
linear-cli p create "Feature" -t ENG --icon "🚀" --priority 1 \
  --start-date 2025-01-01 --target-date 2025-03-31 \
  --lead USER_ID --status planned --content "Project description"

# Update project
linear-cli p update PROJECT_ID --name "New Name" --status completed
linear-cli p update PROJECT_ID --lead USER_ID --priority 2

# Archive/unarchive
linear-cli p archive PROJECT_ID
linear-cli p unarchive PROJECT_ID

# Labels
linear-cli p add-labels PROJECT_ID -l label1 -l label2
linear-cli p remove-labels PROJECT_ID -l label1
linear-cli p set-labels PROJECT_ID -l label1 -l label2

# Members
linear-cli p members PROJECT_ID

# Delete
linear-cli p delete PROJECT_ID --force
```

## Flags

| Flag | Purpose |
|------|---------|
| `--icon EMOJI` | Project icon |
| `--priority N` | Priority (1=urgent, 4=low) |
| `--start-date DATE` | Start date |
| `--target-date DATE` | Target date |
| `--lead USER` | Project lead |
| `--status STATE` | Project status |
| `--id-only` | Return ID only |
| `--output json` | JSON output |
