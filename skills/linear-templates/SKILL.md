---
name: linear-templates
description: Manage issue templates - local templates and Linear API templates. Use when creating or using templates.
allowed-tools: Bash
---

# Local Templates

```bash
# List local templates
linear-cli tpl list

# Show template
linear-cli tpl show bug

# Create local template
linear-cli tpl create bug

# Delete local template
linear-cli tpl delete bug
```

# API Templates (Linear server-side)

```bash
# List remote templates
linear-cli tpl remote-list
linear-cli tpl remote-list --output json

# Get remote template
linear-cli tpl remote-get TEMPLATE_ID

# Create remote template
linear-cli tpl remote-create "Bug Report" -t ENG

# Update remote template
linear-cli tpl remote-update TEMPLATE_ID --name "Updated"

# Delete remote template
linear-cli tpl remote-delete TEMPLATE_ID --force
```

## Flags

| Flag | Purpose |
|------|---------|
| `-t TEAM` | Team for remote templates |
| `--output json` | JSON output |
