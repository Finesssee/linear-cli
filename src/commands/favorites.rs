use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

use crate::api::LinearClient;
use crate::output::{print_json, OutputOptions};
use crate::types::Favorite;

#[derive(Subcommand, Debug)]
pub enum FavoriteCommands {
    /// List all favorites
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
        query {
            favorites(first: 250) {
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
                }
            }
        }
    "#;

    let result = client.query(query, None).await?;
    let favorites = &result["data"]["favorites"]["nodes"];

    print_json(favorites, output)?;
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
