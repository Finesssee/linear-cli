use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use serde_json::json;
use tabled::{Table, Tabled};

use crate::api::LinearClient;
use crate::display_options;
use crate::output::{
    ensure_non_empty, filter_values, print_json, print_json_owned, sort_values, OutputOptions,
};
use crate::pagination::paginate_nodes;
use crate::text::truncate;
use crate::types::Favorite;

#[derive(Tabled)]
struct FavoriteRow {
    #[tabled(rename = "Type")]
    fav_type: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "ID")]
    id: String,
}

#[derive(Subcommand, Debug)]
pub enum FavoriteCommands {
    /// List all favorites
    #[command(alias = "ls")]
    List,
    /// Add an issue/project to favorites
    Add {
        /// Issue identifier (e.g., LIN-123) or project ID
        id: String,
    },
    /// Remove from favorites
    Remove {
        /// Issue identifier or project ID
        id: String,
    },
}

pub async fn handle(cmd: FavoriteCommands, output: &OutputOptions) -> Result<()> {
    match cmd {
        FavoriteCommands::List => list_favorites(output).await,
        FavoriteCommands::Add { id } => add_favorite(&id, output).await,
        FavoriteCommands::Remove { id } => remove_favorite(&id, output).await,
    }
}

async fn list_favorites(output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($first: Int, $after: String, $last: Int, $before: String) {
            favorites(first: $first, after: $after, last: $last, before: $before) {
                nodes {
                    id
                    type
                    sortOrder
                    issue {
                        id
                        identifier
                        title
                    }
                    project {
                        id
                        name
                    }
                    label {
                        id
                        name
                    }
                    cycle {
                        id
                        name
                        number
                    }
                    document {
                        id
                        title
                    }
                    customView {
                        id
                        name
                    }
                }
                pageInfo {
                    hasNextPage
                    endCursor
                    hasPreviousPage
                    startCursor
                }
            }
        }
    "#;

    let pagination = output.pagination.with_default_limit(100);
    let mut favorites = paginate_nodes(
        &client,
        query,
        serde_json::Map::new(),
        &["data", "favorites", "nodes"],
        &["data", "favorites", "pageInfo"],
        &pagination,
        250,
    )
    .await?;

    if output.is_json() || output.has_template() {
        print_json_owned(json!(favorites), output)?;
        return Ok(());
    }

    filter_values(&mut favorites, &output.filters);
    if let Some(sort_key) = output.json.sort.as_deref() {
        sort_values(&mut favorites, sort_key, output.json.order);
    }

    ensure_non_empty(&favorites, output)?;
    if favorites.is_empty() {
        println!("No favorites found.");
        return Ok(());
    }

    let display = display_options();
    let max_width = display.max_width(40);

    let rows: Vec<FavoriteRow> = favorites
        .iter()
        .filter_map(|v| serde_json::from_value::<Favorite>(v.clone()).ok())
        .map(|f| {
            let (fav_type, name, id) = match f.favorite_type.as_deref() {
                Some("issue") => {
                    if let Some(issue) = &f.issue {
                        let display_name = format!(
                            "{} - {}",
                            issue.identifier,
                            issue.title.as_deref().unwrap_or("")
                        );
                        (
                            "Issue".to_string(),
                            truncate(&display_name, max_width),
                            issue.id.clone(),
                        )
                    } else {
                        ("Issue".to_string(), "-".to_string(), f.id.clone())
                    }
                }
                Some("project") => {
                    if let Some(project) = &f.project {
                        (
                            "Project".to_string(),
                            truncate(&project.name, max_width),
                            project.id.clone(),
                        )
                    } else {
                        ("Project".to_string(), "-".to_string(), f.id.clone())
                    }
                }
                Some("label") => {
                    if let Some(label) = &f.label {
                        (
                            "Label".to_string(),
                            truncate(&label.name, max_width),
                            label.id.clone(),
                        )
                    } else {
                        ("Label".to_string(), "-".to_string(), f.id.clone())
                    }
                }
                Some("cycle") => {
                    if let Some(cycle) = &f.cycle {
                        let display_name = cycle.name.as_deref().unwrap_or("");
                        let display_name = if display_name.is_empty() {
                            cycle
                                .number
                                .map(|n| format!("Cycle {}", n))
                                .unwrap_or_else(|| "-".to_string())
                        } else {
                            display_name.to_string()
                        };
                        (
                            "Cycle".to_string(),
                            truncate(&display_name, max_width),
                            cycle.id.clone(),
                        )
                    } else {
                        ("Cycle".to_string(), "-".to_string(), f.id.clone())
                    }
                }
                Some("document") => {
                    if let Some(doc) = &f.document {
                        (
                            "Document".to_string(),
                            truncate(&doc.title, max_width),
                            doc.id.clone(),
                        )
                    } else {
                        ("Document".to_string(), "-".to_string(), f.id.clone())
                    }
                }
                Some("customView") => {
                    if let Some(view) = &f.custom_view {
                        (
                            "View".to_string(),
                            truncate(&view.name, max_width),
                            view.id.clone(),
                        )
                    } else {
                        ("View".to_string(), "-".to_string(), f.id.clone())
                    }
                }
                Some(t) => (t.to_string(), "-".to_string(), f.id.clone()),
                None => ("Unknown".to_string(), "-".to_string(), f.id.clone()),
            };

            FavoriteRow { fav_type, name, id }
        })
        .collect();

    println!("{} ({}):", "Favorites".bold(), rows.len());
    println!("{}", Table::new(rows));

    Ok(())
}

async fn add_favorite(id: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    // Try to resolve as issue first
    let issue_query = r#"
        query($identifier: String!) {
            issue(id: $identifier) {
                id
            }
        }
    "#;

    let issue_result = client
        .query(issue_query, Some(json!({ "identifier": id })))
        .await?;

    // Check if issue exists (query succeeded AND data.issue is not null)
    let is_issue = !issue_result["data"]["issue"].is_null();

    let mutation = if is_issue {
        r#"
            mutation($issueId: String!) {
                favoriteCreate(input: { issueId: $issueId }) {
                    success
                    favorite {
                        id
                    }
                }
            }
        "#
    } else {
        r#"
            mutation($projectId: String!) {
                favoriteCreate(input: { projectId: $projectId }) {
                    success
                    favorite {
                        id
                    }
                }
            }
        "#
    };

    let vars = if is_issue {
        json!({ "issueId": id })
    } else {
        json!({ "projectId": id })
    };

    let result = client.mutate(mutation, Some(vars)).await?;

    if output.is_json() {
        print_json(&result["data"]["favoriteCreate"], output)?;
    } else {
        println!("Added {} to favorites", id);
    }

    Ok(())
}

async fn remove_favorite(id: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    // First find the favorite by issue/project id
    let query = r#"
        query {
            favorites(first: 250) {
                nodes {
                    id
                    issue { identifier }
                    project { id }
                }
            }
        }
    "#;

    let result = client.query(query, None).await?;
    let favorites: Vec<Favorite> = result["data"]["favorites"]["nodes"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| serde_json::from_value::<Favorite>(v).ok())
        .collect();

    let favorite = favorites.iter().find(|f| {
        f.issue
            .as_ref()
            .map(|i| i.identifier.as_str() == id)
            .unwrap_or(false)
            || f.project
                .as_ref()
                .map(|p| p.id.as_str() == id)
                .unwrap_or(false)
    });

    if let Some(fav) = favorite {
        let fav_id = &fav.id;
        let mutation = r#"
            mutation($id: String!) {
                favoriteDelete(id: $id) {
                    success
                }
            }
        "#;

        let result = client
            .mutate(mutation, Some(json!({ "id": fav_id })))
            .await?;

        if output.is_json() {
            print_json(&result["data"]["favoriteDelete"], output)?;
        } else {
            println!("Removed {} from favorites", id);
        }
    } else {
        anyhow::bail!("Favorite not found for: {}", id);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_favorite_row_from_issue() {
        let json = r#"{
            "id": "fav1",
            "type": "issue",
            "sortOrder": 1.0,
            "issue": {
                "id": "issue1",
                "identifier": "LIN-42",
                "title": "Favorite issue"
            }
        }"#;
        let fav: Favorite = serde_json::from_str(json).unwrap();
        assert_eq!(fav.favorite_type.as_deref(), Some("issue"));
        assert_eq!(fav.issue.as_ref().unwrap().identifier, "LIN-42");
    }

    #[test]
    fn test_favorite_row_from_project() {
        let json = r#"{
            "id": "fav2",
            "type": "project",
            "project": {
                "id": "proj1",
                "name": "My Project"
            }
        }"#;
        let fav: Favorite = serde_json::from_str(json).unwrap();
        assert_eq!(fav.favorite_type.as_deref(), Some("project"));
        assert_eq!(fav.project.as_ref().unwrap().name, "My Project");
    }

    #[test]
    fn test_favorite_row_from_label() {
        let json = r#"{
            "id": "fav3",
            "type": "label",
            "label": {
                "id": "label1",
                "name": "bug"
            }
        }"#;
        let fav: Favorite = serde_json::from_str(json).unwrap();
        assert_eq!(fav.favorite_type.as_deref(), Some("label"));
        assert_eq!(fav.label.as_ref().unwrap().name, "bug");
    }

    #[test]
    fn test_favorite_row_from_cycle() {
        let json = r#"{
            "id": "fav4",
            "type": "cycle",
            "cycle": {
                "id": "cycle1",
                "name": "Sprint 5",
                "number": 5
            }
        }"#;
        let fav: Favorite = serde_json::from_str(json).unwrap();
        assert_eq!(fav.favorite_type.as_deref(), Some("cycle"));
        assert_eq!(
            fav.cycle.as_ref().unwrap().name.as_deref(),
            Some("Sprint 5")
        );
    }

    #[test]
    fn test_favorite_row_from_document() {
        let json = r#"{
            "id": "fav5",
            "type": "document",
            "document": {
                "id": "doc1",
                "title": "Design Doc"
            }
        }"#;
        let fav: Favorite = serde_json::from_str(json).unwrap();
        assert_eq!(fav.favorite_type.as_deref(), Some("document"));
        assert_eq!(fav.document.as_ref().unwrap().title, "Design Doc");
    }

    #[test]
    fn test_favorite_row_from_custom_view() {
        let json = r#"{
            "id": "fav6",
            "type": "customView",
            "customView": {
                "id": "view1",
                "name": "Bug Triage"
            }
        }"#;
        let fav: Favorite = serde_json::from_str(json).unwrap();
        assert_eq!(fav.favorite_type.as_deref(), Some("customView"));
        assert_eq!(fav.custom_view.as_ref().unwrap().name, "Bug Triage");
    }

    #[test]
    fn test_favorite_unknown_type() {
        let json = r#"{
            "id": "fav7",
            "type": "predefinedView"
        }"#;
        let fav: Favorite = serde_json::from_str(json).unwrap();
        assert_eq!(fav.favorite_type.as_deref(), Some("predefinedView"));
        assert!(fav.issue.is_none());
        assert!(fav.project.is_none());
    }
}
