---
name: linear-teams
description: Manage Linear teams and users - list, create, update, delete teams. Use when managing teams or viewing user profiles.
allowed-tools: Bash
---

# Teams

```bash
# List teams
linear-cli t list
linear-cli t list --output json

# Get team details
linear-cli t get ENG
linear-cli t members ENG             # List team members

# Create team
linear-cli t create "Platform" -k PLT
linear-cli t create "Mobile" -k MOB --description "Mobile team" --private

# Update team
linear-cli t update ENG --name "Engineering" --timezone "America/New_York"

# Delete team
linear-cli t delete TEAM_ID --force
```

# Users

```bash
# List users
linear-cli u list                    # All workspace users
linear-cli u list --team ENG         # Team members only

# Current user
linear-cli u me
linear-cli me                        # Alias (whoami)
```

## Flags

| Flag | Purpose |
|------|---------|
| `-k KEY` | Team key |
| `--private` | Private team |
| `--output json` | JSON output |
| `--compact` | No formatting |
