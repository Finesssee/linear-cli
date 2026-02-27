---
name: linear-cycles
description: Manage Linear sprint cycles - list, create, update, delete, complete. Use when managing cycles.
allowed-tools: Bash
---

# Cycles

```bash
# List cycles
linear-cli c list -t ENG             # Team cycles
linear-cli c list -t ENG --output json

# Current cycle
linear-cli c current -t ENG
linear-cli c current -t ENG --output json

# Create cycle
linear-cli c create -t ENG --name "Sprint 5"
linear-cli c create -t ENG --name "Sprint 5" --starts-at 2024-01-01 --ends-at 2024-01-14

# Get cycle details
linear-cli c get CYCLE_ID

# Update cycle
linear-cli c update CYCLE_ID --name "Sprint 5b"
linear-cli c update CYCLE_ID --description "Updated goals" --dry-run

# Complete a cycle
linear-cli c complete CYCLE_ID

# Delete cycle
linear-cli c delete CYCLE_ID --force
```

## Flags

| Flag | Purpose |
|------|---------|
| `--output json` | JSON output |
| `--compact` | No formatting |
| `--dry-run` | Preview without updating |
| `--force` | Skip delete confirmation |
