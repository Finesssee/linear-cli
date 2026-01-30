use anyhow::Result;
use clap::{Subcommand, ValueEnum};
use serde_json::json;
use tabled::{Table, Tabled};

use crate::api::LinearClient;
use crate::output::{print_json, OutputOptions};
use crate::text::truncate;
use crate::DISPLAY_OPTIONS;

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum RelationType {
    /// Issue blocks another
    Blocks,
    /// Issue is blocked by another
    BlockedBy,
    /// Related issues
    Related,
    /// Duplicate of another issue
    Duplicate,
}

impl RelationType {
    fn to_api_string(&self) -> &'static str {
        match self {
            RelationType::Blocks => "blocks",
            RelationType::BlockedBy => "blockedBy",
            RelationType::Related => "related",
            RelationType::Duplicate => "duplicate",
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum RelationCommands {
    /// List issue relationships
    #[command(alias = "ls")]
    List {
        /// Issue identifier (e.g., LIN-123)
        id: String,
    },
    /// Add a relationship between issues
    Add {
        /// Source issue identifier
        from: String,
        /// Relationship type
        #[arg(short = 'r', long, value_enum)]
        relation: RelationType,
        /// Target issue identifier
        to: String,
    },
    /// Remove a relationship between issues
    Remove {
        /// Relation ID to remove
        id: String,
    },
    /// Set parent issue
    Parent {
        /// Child issue identifier
        child: String,
        /// Parent issue identifier
        parent: String,
    },
    /// Remove parent from issue
    Unparent {
        /// Issue identifier
        id: String,
    },
}

#[derive(Tabled)]
struct RelationRow {
    #[tabled(rename = "Type")]
    relation_type: String,
    #[tabled(rename = "Issue")]
    issue: String,
    #[tabled(rename = "Title")]
    title: String,
    #[tabled(rename = "Status")]
    status: String,
}

pub async fn handle(cmd: RelationCommands, output: &OutputOptions) -> Result<()> {
    match cmd {
        RelationCommands::List { id } => list_relations(&id, output).await,
        RelationCommands::Add { from, relation, to } => {
            add_relation(&from, relation, &to, output).await
        }
        RelationCommands::Remove { id } => remove_relation(&id, output).await,
        RelationCommands::Parent { child, parent } => set_parent(&child, &parent, output).await,
        RelationCommands::Unparent { id } => remove_parent(&id, output).await,
    }
}

async fn list_relations(id: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($id: String!) {
            issue(id: $id) {
                id
                identifier
                title
                parent {
                    id
                    identifier
                    title
                    state { name }
                }
                children {
                    nodes {
                        id
                        identifier
                        title
                        state { name }
                    }
                }
                relations {
                    nodes {
                        id
                        type
                        relatedIssue {
                            id
                            identifier
                            title
                            state { name }
                        }
                    }
                }
                inverseRelations {
                    nodes {
                        id
                        type
                        issue {
                            id
                            identifier
                            title
                            state { name }
                        }
                    }
                }
            }
        }
    "#;

    let result = client.query(query, Some(json!({ "id": id }))).await?;
    let issue = &result["data"]["issue"];

    if issue.is_null() {
        anyhow::bail!("Issue not found: {}", id);
    }

    if output.is_json() {
        print_json(
            &json!({
                "issue": {
                    "id": issue["id"],
                    "identifier": issue["identifier"],
                    "title": issue["title"],
                },
                "parent": issue["parent"],
                "children": issue["children"]["nodes"],
                "relations": issue["relations"]["nodes"],
                "inverseRelations": issue["inverseRelations"]["nodes"],
            }),
            output,
        )?;
    } else {
        let display = DISPLAY_OPTIONS.get().cloned().unwrap_or_default();
        let max_width = display.max_width(40);

        println!(
            "Relations for {} - {}\n",
            issue["identifier"].as_str().unwrap_or(id),
            issue["title"].as_str().unwrap_or("")
        );

        // Parent
        if !issue["parent"].is_null() {
            let parent = &issue["parent"];
            println!("Parent:");
            println!(
                "  {} - {} ({})",
                parent["identifier"].as_str().unwrap_or("-"),
                truncate(parent["title"].as_str().unwrap_or("-"), max_width),
                parent["state"]["name"].as_str().unwrap_or("-")
            );
            println!();
        }

        // Children
        let children = issue["children"]["nodes"].as_array();
        if let Some(children) = children {
            if !children.is_empty() {
                println!("Children ({}):", children.len());
                for child in children {
                    println!(
                        "  {} - {} ({})",
                        child["identifier"].as_str().unwrap_or("-"),
                        truncate(child["title"].as_str().unwrap_or("-"), max_width),
                        child["state"]["name"].as_str().unwrap_or("-")
                    );
                }
                println!();
            }
        }

        // Build relation rows
        let mut rows: Vec<RelationRow> = Vec::new();

        // Outgoing relations
        if let Some(relations) = issue["relations"]["nodes"].as_array() {
            for rel in relations {
                let related = &rel["relatedIssue"];
                rows.push(RelationRow {
                    relation_type: rel["type"].as_str().unwrap_or("-").to_string(),
                    issue: related["identifier"].as_str().unwrap_or("-").to_string(),
                    title: truncate(related["title"].as_str().unwrap_or("-"), max_width),
                    status: related["state"]["name"].as_str().unwrap_or("-").to_string(),
                });
            }
        }

        // Incoming relations
        if let Some(inverse) = issue["inverseRelations"]["nodes"].as_array() {
            for rel in inverse {
                let related = &rel["issue"];
                let rel_type = match rel["type"].as_str() {
                    Some("blocks") => "blocked by",
                    Some("blockedBy") => "blocks",
                    Some(t) => t,
                    None => "-",
                };
                rows.push(RelationRow {
                    relation_type: rel_type.to_string(),
                    issue: related["identifier"].as_str().unwrap_or("-").to_string(),
                    title: truncate(related["title"].as_str().unwrap_or("-"), max_width),
                    status: related["state"]["name"].as_str().unwrap_or("-").to_string(),
                });
            }
        }

        if rows.is_empty() {
            println!("No other relations");
        } else {
            println!("Relations:");
            println!("{}", Table::new(rows));
        }
    }

    Ok(())
}

async fn add_relation(from: &str, relation: RelationType, to: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let mutation = r#"
        mutation($issueId: String!, $relatedIssueId: String!, $type: String!) {
            issueRelationCreate(input: {
                issueId: $issueId
                relatedIssueId: $relatedIssueId
                type: $type
            }) {
                success
                issueRelation {
                    id
                    type
                    issue { identifier }
                    relatedIssue { identifier }
                }
            }
        }
    "#;

    let result = client
        .mutate(
            mutation,
            Some(json!({
                "issueId": from,
                "relatedIssueId": to,
                "type": relation.to_api_string()
            })),
        )
        .await?;

    if output.is_json() {
        print_json(&result["data"]["issueRelationCreate"], output)?;
    } else {
        let rel = &result["data"]["issueRelationCreate"]["issueRelation"];
        println!(
            "Created relation: {} {} {}",
            rel["issue"]["identifier"].as_str().unwrap_or(from),
            relation.to_api_string(),
            rel["relatedIssue"]["identifier"].as_str().unwrap_or(to)
        );
    }

    Ok(())
}

async fn remove_relation(id: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let mutation = r#"
        mutation($id: String!) {
            issueRelationDelete(id: $id) {
                success
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "id": id })))
        .await?;

    if output.is_json() {
        print_json(&result["data"]["issueRelationDelete"], output)?;
    } else {
        println!("Relation removed");
    }

    Ok(())
}

async fn set_parent(child: &str, parent: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let mutation = r#"
        mutation($id: String!, $parentId: String!) {
            issueUpdate(id: $id, input: { parentId: $parentId }) {
                success
                issue {
                    id
                    identifier
                    parent { identifier title }
                }
            }
        }
    "#;

    let result = client
        .mutate(
            mutation,
            Some(json!({ "id": child, "parentId": parent })),
        )
        .await?;

    if output.is_json() {
        print_json(&result["data"]["issueUpdate"], output)?;
    } else {
        let issue = &result["data"]["issueUpdate"]["issue"];
        println!(
            "Set parent of {} to {} ({})",
            issue["identifier"].as_str().unwrap_or(child),
            issue["parent"]["identifier"].as_str().unwrap_or(parent),
            issue["parent"]["title"].as_str().unwrap_or("")
        );
    }

    Ok(())
}

async fn remove_parent(id: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let mutation = r#"
        mutation($id: String!) {
            issueUpdate(id: $id, input: { parentId: null }) {
                success
                issue {
                    id
                    identifier
                }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "id": id })))
        .await?;

    if output.is_json() {
        print_json(&result["data"]["issueUpdate"], output)?;
    } else {
        let issue = &result["data"]["issueUpdate"]["issue"];
        println!(
            "Removed parent from {}",
            issue["identifier"].as_str().unwrap_or(id)
        );
    }

    Ok(())
}
