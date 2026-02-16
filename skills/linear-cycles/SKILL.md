---
name: linear-cycles
description: Manage Linear sprint cycles. Use when listing, creating, or updating cycles.
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

# Update cycle
linear-cli c update CYCLE_ID --name "Sprint 5b"
linear-cli c update CYCLE_ID --description "Updated goals" --dry-run
```

## Flags

| Flag | Purpose |
|------|---------|
| `--output json` | JSON output |
| `--compact` | No formatting |
| `--dry-run` | Preview without updating |
