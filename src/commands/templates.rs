use anyhow::{Context, Result};
use clap::Subcommand;
use colored::Colorize;
use dialoguer::{Input, Select};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tabled::{Table, Tabled};

use crate::display_options;
use crate::output::{ensure_non_empty, filter_values, print_json, sort_values, OutputOptions};
use crate::priority::priority_to_string;
use crate::text::truncate;
/// Issue template structure for creating issues with predefined values
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IssueTemplate {
    /// Template name (used as identifier)
    pub name: String,
    /// Optional prefix to add to issue titles
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_prefix: Option<String>,
    /// Default description for the issue
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Default priority (0=none, 1=urgent, 2=high, 3=normal, 4=low)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_priority: Option<i32>,
    /// Default labels to apply
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub default_labels: Vec<String>,
    /// Default team name or ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team: Option<String>,
}

/// Storage for all templates
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct TemplateStore {
    pub templates: HashMap<String, IssueTemplate>,
}

#[derive(Subcommand)]
pub enum TemplateCommands {
    /// List available templates
    #[command(alias = "ls")]
    List,
    /// Create a new template interactively
    Create {
        /// Template name
        name: String,
    },
    /// Show template details
    #[command(alias = "get")]
    Show {
        /// Template name
        name: String,
    },
    /// Delete a template
    #[command(alias = "rm")]
    Delete {
        /// Template name
        name: String,
        /// Skip confirmation
        #[arg(short, long)]
        force: bool,
    },
}

#[derive(Tabled)]
struct TemplateRow {
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Title Prefix")]
    title_prefix: String,
    #[tabled(rename = "Team")]
    team: String,
    #[tabled(rename = "Priority")]
    priority: String,
    #[tabled(rename = "Labels")]
    labels: String,
}

fn templates_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .context("Could not find config directory")?
        .join("linear-cli");

    fs::create_dir_all(&config_dir)?;
    Ok(config_dir.join("templates.json"))
}

pub fn load_templates() -> Result<TemplateStore> {
    let path = templates_path()?;
    if path.exists() {
        let content = fs::read_to_string(&path)?;
        let store: TemplateStore = serde_json::from_str(&content)?;
        Ok(store)
    } else {
        Ok(TemplateStore::default())
    }
}

fn save_templates(store: &TemplateStore) -> Result<()> {
    let path = templates_path()?;
    let content = serde_json::to_string_pretty(store)?;
    fs::write(path, content)?;
    Ok(())
}

pub fn get_template(name: &str) -> Result<Option<IssueTemplate>> {
    let store = load_templates()?;
    Ok(store.templates.get(name).cloned())
}

pub async fn handle(cmd: TemplateCommands, output: &OutputOptions) -> Result<()> {
    match cmd {
        TemplateCommands::List => list_templates(output),
        TemplateCommands::Create { name } => create_template(&name, output),
        TemplateCommands::Show { name } => show_template(&name, output),
        TemplateCommands::Delete { name, force } => delete_template(&name, force, output),
    }
}


fn list_templates(output: &OutputOptions) -> Result<()> {
    let store = load_templates()?;

    if store.templates.is_empty() {
        ensure_non_empty(&[], output)?;
        println!("No templates found.");
        println!("\nCreate one with: linear-cli templates create <name>");
        return Ok(());
    }

    let mut templates: Vec<serde_json::Value> =
        store.templates.values().map(|t| json!(t)).collect();

    filter_values(&mut templates, &output.filters);

    if let Some(sort_key) = output.json.sort.as_deref() {
        sort_values(&mut templates, sort_key, output.json.order);
    } else {
        templates.sort_by(|a, b| {
            a["name"]
                .as_str()
                .unwrap_or("")
                .cmp(b["name"].as_str().unwrap_or(""))
        });
    }

    if output.is_json() || output.has_template() {
        print_json(&serde_json::json!(templates), output)?;
        return Ok(());
    }

    ensure_non_empty(&templates, output)?;
    if templates.is_empty() {
        println!("No templates found.");
        return Ok(());
    }

    let width = display_options().max_width(30);
    let rows: Vec<TemplateRow> = templates
        .iter()
        .map(|t| TemplateRow {
            name: truncate(t["name"].as_str().unwrap_or(""), width),
            title_prefix: truncate(t["title_prefix"].as_str().unwrap_or("-"), width),
            team: truncate(t["team"].as_str().unwrap_or("-"), width),
            priority: priority_to_string(t["default_priority"].as_i64()),
            labels: {
                let labels = t["default_labels"].as_array().cloned().unwrap_or_default();
                if labels.is_empty() {
                    "-".to_string()
                } else {
                    let joined = labels
                        .iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    truncate(&joined, display_options().max_width(40))
                }
            },
        })
        .collect();

    let table = Table::new(rows).to_string();
    println!("{}", table);
    println!("\n{} templates", store.templates.len());

    Ok(())
}

fn create_template(name: &str, output: &OutputOptions) -> Result<()> {
    let mut store = load_templates()?;

    if store.templates.contains_key(name) {
        anyhow::bail!("Template already exists. Delete it first or choose a different name.");
    }

    println!("{} Creating template: {}", "+".green(), name.cyan());
    println!("Press Enter to skip optional fields.\n");

    let title_prefix: String = Input::new()
        .with_prompt("Title prefix (e.g., [Bug], [Feature])")
        .allow_empty(true)
        .interact_text()?;

    let title_prefix = if title_prefix.is_empty() {
        None
    } else {
        Some(title_prefix)
    };

    let description: String = Input::new()
        .with_prompt("Default description")
        .allow_empty(true)
        .interact_text()?;

    let description = if description.is_empty() {
        None
    } else {
        Some(description)
    };

    let priority_options = vec!["None", "Urgent (1)", "High (2)", "Normal (3)", "Low (4)"];
    let priority_selection = Select::new()
        .with_prompt("Default priority")
        .items(&priority_options)
        .default(0)
        .interact()?;

    let default_priority = match priority_selection {
        0 => None,
        1 => Some(1),
        2 => Some(2),
        3 => Some(3),
        4 => Some(4),
        _ => None,
    };

    let team: String = Input::new()
        .with_prompt("Default team (name or key)")
        .allow_empty(true)
        .interact_text()?;

    let team = if team.is_empty() { None } else { Some(team) };

    let labels_input: String = Input::new()
        .with_prompt("Default labels (comma-separated)")
        .allow_empty(true)
        .interact_text()?;

    let default_labels: Vec<String> = if labels_input.is_empty() {
        vec![]
    } else {
        labels_input
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };

    let template = IssueTemplate {
        name: name.to_string(),
        title_prefix,
        description,
        default_priority,
        default_labels,
        team,
    };

    store.templates.insert(name.to_string(), template);
    save_templates(&store)?;

    if output.is_json() || output.has_template() {
        print_json(&json!(store.templates.get(name)), output)?;
        return Ok(());
    }

    println!("\n{} Template created successfully!", "+".green());

    Ok(())
}

fn show_template(name: &str, output: &OutputOptions) -> Result<()> {
    let store = load_templates()?;

    let template = store
        .templates
        .get(name)
        .ok_or_else(|| anyhow::anyhow!("Template not found"))?;

    if output.is_json() || output.has_template() {
        print_json(&json!(template), output)?;
        return Ok(());
    }

    println!("{} {}", "Template:".bold(), template.name.cyan().bold());
    println!("{}", "-".repeat(40));

    println!(
        "Title Prefix: {}",
        template.title_prefix.as_ref().unwrap_or(&"-".to_string())
    );

    if let Some(desc) = &template.description {
        println!("Description:  {}", desc);
    } else {
        println!("Description:  -");
    }

    println!(
        "Priority:     {}",
        priority_to_string(template.default_priority.map(|p| p as i64))
    );

    println!(
        "Team:         {}",
        template.team.as_ref().unwrap_or(&"-".to_string())
    );

    if template.default_labels.is_empty() {
        println!("Labels:       -");
    } else {
        println!("Labels:       {}", template.default_labels.join(", "));
    }

    Ok(())
}

fn delete_template(name: &str, force: bool, output: &OutputOptions) -> Result<()> {
    let mut store = load_templates()?;

    if !store.templates.contains_key(name) {
        anyhow::bail!("Template not found");
    }

    if !force {
        println!("Are you sure you want to delete this template?");
        println!("Use --force to skip this prompt.");
        return Ok(());
    }

    store.templates.remove(name);
    save_templates(&store)?;

    if output.is_json() || output.has_template() {
        print_json(&json!({ "deleted": name }), output)?;
        return Ok(());
    }

    println!("{} Template deleted", "+".green());

    Ok(())
}
