use anyhow::{Context, Result};
use clap::Subcommand;
use colored::Colorize;
use serde_json::json;
use tabled::{Table, Tabled};

use crate::api::{resolve_team_id, LinearClient};
use crate::display_options;
use crate::output::{
    ensure_non_empty, filter_values, print_json, print_json_owned, sort_values, OutputOptions,
};
use crate::text::truncate;
use crate::types::Webhook;

#[derive(Subcommand)]
pub enum WebhookCommands {
    /// List all webhooks
    #[command(alias = "ls")]
    #[command(after_help = r#"EXAMPLES:
    linear webhooks list                    # List all webhooks
    linear wh list --output json            # Output as JSON"#)]
    List,
    /// Get webhook details
    #[command(after_help = r#"EXAMPLES:
    linear webhooks get WEBHOOK_ID          # View webhook details"#)]
    Get {
        /// Webhook ID
        id: String,
    },
    /// Create a new webhook
    #[command(after_help = r#"EXAMPLES:
    linear webhooks create https://example.com/hook --events Issue,Comment
    linear wh create URL --events Issue --team ENG --label "My Hook"
    linear wh create URL --events Issue --all-teams"#)]
    Create {
        /// Webhook URL to receive events
        url: String,
        /// Comma-separated resource types (e.g., Issue,Comment,Project)
        #[arg(long, value_delimiter = ',')]
        events: Vec<String>,
        /// Scope to a specific team
        #[arg(short, long)]
        team: Option<String>,
        /// Receive events for all public teams
        #[arg(long)]
        all_teams: bool,
        /// Human-readable label for the webhook
        #[arg(short, long)]
        label: Option<String>,
        /// Webhook signing secret (auto-generated if not specified)
        #[arg(long)]
        secret: Option<String>,
    },
    /// Update a webhook
    #[command(after_help = r#"EXAMPLES:
    linear webhooks update WEBHOOK_ID --url https://new-url.com
    linear wh update WEBHOOK_ID --enabled false --label "Disabled hook""#)]
    Update {
        /// Webhook ID
        id: String,
        /// New URL
        #[arg(long)]
        url: Option<String>,
        /// New comma-separated resource types
        #[arg(long, value_delimiter = ',')]
        events: Vec<String>,
        /// Enable the webhook
        #[arg(long, conflicts_with = "disabled")]
        enabled: bool,
        /// Disable the webhook
        #[arg(long, conflicts_with = "enabled")]
        disabled: bool,
        /// New label
        #[arg(short, long)]
        label: Option<String>,
    },
    /// Delete a webhook
    #[command(after_help = r#"EXAMPLES:
    linear webhooks delete WEBHOOK_ID       # Delete with confirmation
    linear wh delete WEBHOOK_ID --force     # Delete without confirmation"#)]
    Delete {
        /// Webhook ID
        id: String,
        /// Skip confirmation
        #[arg(long)]
        force: bool,
    },
    /// Rotate webhook signing secret
    #[command(after_help = r#"EXAMPLES:
    linear webhooks rotate-secret WEBHOOK_ID"#)]
    RotateSecret {
        /// Webhook ID
        id: String,
    },
    /// Listen for webhook events locally
    #[command(after_help = r#"EXAMPLES:
    linear webhooks listen --port 9000      # Start local listener
    linear wh listen --port 9000 --events Issue,Comment
    linear wh listen --url https://my-tunnel.ngrok.io --events Issue

NOTE: Linear cannot reach localhost directly.
Use a tunnel service (ngrok, cloudflare tunnel) and pass --url with your public URL."#)]
    Listen {
        /// Port for local HTTP server
        #[arg(short, long, default_value = "9000")]
        port: u16,
        /// Comma-separated resource types to subscribe to
        #[arg(long, value_delimiter = ',')]
        events: Vec<String>,
        /// Team to scope webhook to
        #[arg(short, long)]
        team: Option<String>,
        /// Webhook signing secret for HMAC verification
        #[arg(long)]
        secret: Option<String>,
        /// Public URL to register as webhook (e.g., ngrok tunnel URL)
        #[arg(long)]
        url: Option<String>,
        /// Output events as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Tabled)]
struct WebhookRow {
    #[tabled(rename = "Label")]
    label: String,
    #[tabled(rename = "URL")]
    url: String,
    #[tabled(rename = "Enabled")]
    enabled: String,
    #[tabled(rename = "Events")]
    events: String,
    #[tabled(rename = "Team")]
    team: String,
    #[tabled(rename = "ID")]
    id: String,
}

pub async fn handle(cmd: WebhookCommands, output: &OutputOptions) -> Result<()> {
    match cmd {
        WebhookCommands::List => list_webhooks(output).await,
        WebhookCommands::Get { id } => get_webhook(&id, output).await,
        WebhookCommands::Create {
            url,
            events,
            team,
            all_teams,
            label,
            secret,
        } => create_webhook(&url, events, team, all_teams, label, secret, output).await,
        WebhookCommands::Update {
            id,
            url,
            events,
            enabled,
            disabled,
            label,
        } => update_webhook(&id, url, events, enabled, disabled, label, output).await,
        WebhookCommands::Delete { id, force } => delete_webhook(&id, force, output).await,
        WebhookCommands::RotateSecret { id } => rotate_secret(&id, output).await,
        WebhookCommands::Listen {
            port,
            events,
            team,
            secret,
            url,
            json,
        } => listen(port, events, team, secret, url, json, output).await,
    }
}

async fn list_webhooks(output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query {
            webhooks {
                nodes {
                    id label url enabled secret resourceTypes allPublicTeams
                    team { id key name }
                    creator { id name }
                    createdAt updatedAt
                }
            }
        }
    "#;

    let result = client.query(query, None).await?;
    let empty = vec![];
    let nodes = result["data"]["webhooks"]["nodes"]
        .as_array()
        .unwrap_or(&empty);

    let mut webhooks: Vec<serde_json::Value> = nodes.clone();

    if output.is_json() || output.has_template() {
        print_json_owned(json!(webhooks), output)?;
        return Ok(());
    }

    filter_values(&mut webhooks, &output.filters);

    if let Some(sort_key) = output.json.sort.as_deref() {
        sort_values(&mut webhooks, sort_key, output.json.order);
    }

    ensure_non_empty(&webhooks, output)?;
    if webhooks.is_empty() {
        println!("No webhooks found.");
        return Ok(());
    }

    let width = display_options().max_width(30);
    let rows: Vec<WebhookRow> = webhooks
        .iter()
        .map(|w| {
            let events = w["resourceTypes"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_else(|| "-".to_string());

            WebhookRow {
                label: truncate(
                    w["label"].as_str().unwrap_or("-"),
                    width,
                ),
                url: truncate(w["url"].as_str().unwrap_or("-"), width),
                enabled: if w["enabled"].as_bool() == Some(true) {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
                events: truncate(&events, width),
                team: truncate(
                    w["team"]["name"].as_str().unwrap_or("All"),
                    width,
                ),
                id: w["id"].as_str().unwrap_or("").to_string(),
            }
        })
        .collect();

    let table = Table::new(rows).to_string();
    println!("{}", table);
    println!("\n{} webhooks", webhooks.len());

    Ok(())
}

async fn get_webhook(id: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let query = r#"
        query($id: String!) {
            webhook(id: $id) {
                id label url enabled secret resourceTypes allPublicTeams
                team { id key name }
                creator { id name }
                createdAt updatedAt
            }
        }
    "#;

    let result = client.query(query, Some(json!({ "id": id }))).await?;
    let webhook = &result["data"]["webhook"];

    if webhook.is_null() {
        anyhow::bail!("Webhook not found: {}", id);
    }

    if output.is_json() || output.has_template() {
        print_json(webhook, output)?;
        return Ok(());
    }

    let wh: Webhook = serde_json::from_value(webhook.clone())?;

    println!("{}", wh.label.as_deref().unwrap_or("Webhook").bold());
    println!("{}", "-".repeat(40));
    println!("URL: {}", wh.url.as_deref().unwrap_or("-"));
    println!(
        "Enabled: {}",
        if wh.enabled { "Yes" } else { "No" }
    );

    if !wh.resource_types.is_empty() {
        println!("Events: {}", wh.resource_types.join(", "));
    }

    println!(
        "All Teams: {}",
        if wh.all_public_teams { "Yes" } else { "No" }
    );

    if let Some(team) = &wh.team {
        println!("Team: {} ({})", team.name, team.key);
    }

    if let Some(creator) = &wh.creator {
        println!("Creator: {}", creator.name);
    }

    if let Some(secret) = &wh.secret {
        if !secret.is_empty() {
            println!("Secret: {}...", &secret[..secret.len().min(8)]);
        }
    }

    println!("ID: {}", wh.id);

    if let Some(created) = &wh.created_at {
        println!("Created: {}", created.chars().take(10).collect::<String>());
    }

    if let Some(updated) = &wh.updated_at {
        println!("Updated: {}", updated.chars().take(10).collect::<String>());
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn create_webhook(
    url: &str,
    events: Vec<String>,
    team: Option<String>,
    all_teams: bool,
    label: Option<String>,
    secret: Option<String>,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;

    let mut input = json!({
        "url": url,
        "resourceTypes": events,
    });

    if let Some(ref t) = team {
        let team_id = resolve_team_id(&client, t, &output.cache).await?;
        input["teamId"] = json!(team_id);
    }

    if all_teams {
        input["allPublicTeams"] = json!(true);
    }

    if let Some(l) = label {
        input["label"] = json!(l);
    }

    if let Some(s) = secret {
        input["secret"] = json!(s);
    }

    if output.dry_run {
        if output.is_json() || output.has_template() {
            print_json_owned(
                json!({
                    "dry_run": true,
                    "would_create": { "input": input }
                }),
                output,
            )?;
        } else {
            println!("{}", "[DRY RUN] Would create webhook:".yellow().bold());
            println!("  URL: {}", url);
        }
        return Ok(());
    }

    let mutation = r#"
        mutation($input: WebhookCreateInput!) {
            webhookCreate(input: $input) {
                success
                webhook { id url secret enabled resourceTypes }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "input": input })))
        .await?;

    if result["data"]["webhookCreate"]["success"].as_bool() == Some(true) {
        let webhook = &result["data"]["webhookCreate"]["webhook"];
        if output.is_json() || output.has_template() {
            print_json(webhook, output)?;
            return Ok(());
        }
        println!(
            "{} Created webhook",
            "+".green(),
        );
        println!("  ID: {}", webhook["id"].as_str().unwrap_or(""));
        println!("  URL: {}", webhook["url"].as_str().unwrap_or(""));
        println!(
            "  Enabled: {}",
            if webhook["enabled"].as_bool() == Some(true) {
                "Yes"
            } else {
                "No"
            }
        );
        if let Some(secret) = webhook["secret"].as_str() {
            if !secret.is_empty() {
                println!("  Secret: {}", secret);
            }
        }
    } else {
        anyhow::bail!("Failed to create webhook. Webhooks require admin scope — try re-authenticating with: linear-cli auth oauth");
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn update_webhook(
    id: &str,
    url: Option<String>,
    events: Vec<String>,
    enabled: bool,
    disabled: bool,
    label: Option<String>,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;

    let mut input = json!({});
    if let Some(u) = url {
        input["url"] = json!(u);
    }
    if !events.is_empty() {
        input["resourceTypes"] = json!(events);
    }
    if enabled {
        input["enabled"] = json!(true);
    }
    if disabled {
        input["enabled"] = json!(false);
    }
    if let Some(l) = label {
        input["label"] = json!(l);
    }

    if input.as_object().map(|o| o.is_empty()).unwrap_or(true) {
        println!("No updates specified.");
        return Ok(());
    }

    if output.dry_run {
        if output.is_json() || output.has_template() {
            print_json_owned(
                json!({
                    "dry_run": true,
                    "would_update": {
                        "id": id,
                        "input": input,
                    }
                }),
                output,
            )?;
        } else {
            println!("{}", "[DRY RUN] Would update webhook:".yellow().bold());
            println!("  ID: {}", id);
        }
        return Ok(());
    }

    let mutation = r#"
        mutation($id: String!, $input: WebhookUpdateInput!) {
            webhookUpdate(id: $id, input: $input) {
                success
                webhook { id url enabled }
            }
        }
    "#;

    let result = client
        .mutate(mutation, Some(json!({ "id": id, "input": input })))
        .await?;

    if result["data"]["webhookUpdate"]["success"].as_bool() == Some(true) {
        if output.is_json() || output.has_template() {
            print_json(&result["data"]["webhookUpdate"]["webhook"], output)?;
            return Ok(());
        }
        println!("{} Webhook updated", "+".green());
    } else {
        anyhow::bail!("Failed to update webhook");
    }

    Ok(())
}

async fn delete_webhook(id: &str, force: bool, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    if output.dry_run {
        if output.is_json() || output.has_template() {
            print_json_owned(
                json!({
                    "dry_run": true,
                    "would_delete": { "id": id }
                }),
                output,
            )?;
        } else {
            println!("{}", "[DRY RUN] Would delete webhook:".yellow().bold());
            println!("  ID: {}", id);
        }
        return Ok(());
    }

    if !force {
        use dialoguer::Confirm;
        let confirm = Confirm::new()
            .with_prompt(format!("Delete webhook {}?", id))
            .default(false)
            .interact()?;
        if !confirm {
            println!("Cancelled.");
            return Ok(());
        }
    }

    let mutation = r#"
        mutation($id: String!) {
            webhookDelete(id: $id) {
                success
            }
        }
    "#;

    let result = client.mutate(mutation, Some(json!({ "id": id }))).await?;

    if result["data"]["webhookDelete"]["success"].as_bool() == Some(true) {
        if output.is_json() || output.has_template() {
            print_json_owned(json!({ "deleted": true, "id": id }), output)?;
            return Ok(());
        }
        println!("{} Webhook deleted: {}", "-".red(), id);
    } else {
        anyhow::bail!("Failed to delete webhook");
    }

    Ok(())
}

async fn rotate_secret(id: &str, output: &OutputOptions) -> Result<()> {
    let client = LinearClient::new()?;

    let mutation = r#"
        mutation($id: String!) {
            webhookRotateSecret(id: $id) {
                success
                webhook { id secret }
            }
        }
    "#;

    let result = client.mutate(mutation, Some(json!({ "id": id }))).await?;

    if result["data"]["webhookRotateSecret"]["success"].as_bool() == Some(true) {
        let webhook = &result["data"]["webhookRotateSecret"]["webhook"];
        if output.is_json() || output.has_template() {
            print_json(webhook, output)?;
            return Ok(());
        }
        println!("{} Secret rotated for webhook: {}", "+".green(), id);
        if let Some(secret) = webhook["secret"].as_str() {
            println!("  New secret: {}", secret);
        }
    } else {
        anyhow::bail!("Failed to rotate webhook secret");
    }

    Ok(())
}

/// Verify HMAC-SHA256 signature from Linear webhook
fn verify_signature(secret: &str, body: &[u8], signature: &str) -> bool {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body);
    let expected = hex::encode(mac.finalize().into_bytes());
    expected == signature
}

#[allow(clippy::too_many_arguments)]
async fn listen(
    port: u16,
    events: Vec<String>,
    team: Option<String>,
    secret: Option<String>,
    url: Option<String>,
    json_output: bool,
    output: &OutputOptions,
) -> Result<()> {
    let client = LinearClient::new()?;

    // Determine the public URL for the webhook
    let webhook_url = match url {
        Some(ref u) => {
            let base = u.trim_end_matches('/');
            format!("{}/webhook", base)
        }
        None => {
            println!(
                "{}",
                "Note: Linear cannot reach localhost directly.".yellow()
            );
            println!(
                "Use a tunnel service (ngrok, cloudflare tunnel) and pass --url with your public URL."
            );
            println!(
                "Starting local server on port {} anyway...",
                port
            );
            format!("http://localhost:{}/webhook", port)
        }
    };

    // Create a temporary webhook
    let mut webhook_input = json!({
        "url": webhook_url,
        "enabled": true,
    });

    if !events.is_empty() {
        webhook_input["resourceTypes"] = json!(events);
    }

    if let Some(ref t) = team {
        let team_id = resolve_team_id(&client, t, &output.cache).await?;
        webhook_input["teamId"] = json!(team_id);
    }

    if let Some(ref s) = secret {
        webhook_input["secret"] = json!(s);
    }

    webhook_input["label"] = json!("linear-cli-listen");

    let create_mutation = r#"
        mutation($input: WebhookCreateInput!) {
            webhookCreate(input: $input) {
                success
                webhook { id url secret enabled }
            }
        }
    "#;

    let result = client
        .mutate(create_mutation, Some(json!({ "input": webhook_input })))
        .await?;

    if result["data"]["webhookCreate"]["success"].as_bool() != Some(true) {
        anyhow::bail!(
            "Failed to create temporary webhook. Webhooks require admin scope — try re-authenticating with: linear-cli auth oauth"
        );
    }

    let webhook_data = &result["data"]["webhookCreate"]["webhook"];
    let webhook_id = webhook_data["id"]
        .as_str()
        .context("Missing webhook ID")?
        .to_string();
    let webhook_secret = secret.or_else(|| {
        webhook_data["secret"]
            .as_str()
            .map(|s| s.to_string())
    });

    println!(
        "{} Temporary webhook created: {}",
        "+".green(),
        webhook_id
    );
    println!("  URL: {}", webhook_url);
    println!("  Listening on port {}...", port);
    println!("  Press Ctrl+C to stop and clean up.\n");

    // Start the HTTP server
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .context(format!("Failed to bind to port {}", port))?;

    // Set up Ctrl+C handler
    let webhook_id_clone = webhook_id.clone();
    let cleanup_client = LinearClient::new()?;
    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            accept = listener.accept() => {
                match accept {
                    Ok((stream, addr)) => {
                        let ws = webhook_secret.clone();
                        let json_out = json_output;
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(stream, addr, ws.as_deref(), json_out).await {
                                eprintln!("Error handling connection from {}: {}", addr, e);
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("Accept error: {}", e);
                    }
                }
            }
            _ = &mut shutdown => {
                println!("\n{} Shutting down...", "!".yellow());
                break;
            }
        }
    }

    // Clean up: delete the temporary webhook
    let delete_mutation = r#"
        mutation($id: String!) {
            webhookDelete(id: $id) {
                success
            }
        }
    "#;

    match cleanup_client
        .mutate(delete_mutation, Some(json!({ "id": webhook_id_clone })))
        .await
    {
        Ok(result) => {
            if result["data"]["webhookDelete"]["success"].as_bool() == Some(true) {
                println!(
                    "{} Temporary webhook cleaned up: {}",
                    "-".red(),
                    webhook_id_clone
                );
            } else {
                eprintln!("Warning: Failed to delete temporary webhook {}", webhook_id_clone);
            }
        }
        Err(e) => {
            eprintln!(
                "Warning: Failed to clean up temporary webhook {}: {}",
                webhook_id_clone, e
            );
        }
    }

    Ok(())
}

async fn handle_connection(
    mut stream: tokio::net::TcpStream,
    addr: std::net::SocketAddr,
    secret: Option<&str>,
    json_output: bool,
) -> Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    // Read request with timeout
    let mut buf = vec![0u8; 65536];
    let n = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        stream.read(&mut buf),
    )
    .await
    .context("Read timeout")?
    .context("Read error")?;

    if n == 0 {
        return Ok(());
    }

    let request = String::from_utf8_lossy(&buf[..n]);

    // Parse HTTP request to extract headers and body
    let parts: Vec<&str> = request.splitn(2, "\r\n\r\n").collect();
    if parts.len() < 2 {
        let response = "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n";
        stream.write_all(response.as_bytes()).await?;
        return Ok(());
    }

    let headers_str = parts[0];
    let body = parts[1];

    // Check it's a POST
    if !headers_str.starts_with("POST") {
        let response = "HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\n\r\n";
        stream.write_all(response.as_bytes()).await?;
        return Ok(());
    }

    // Extract signature header
    let signature = headers_str
        .lines()
        .find(|l| l.to_lowercase().starts_with("linear-signature:"))
        .and_then(|l| l.split_once(':'))
        .map(|(_, v)| v.trim().to_string());

    // Verify signature if secret is provided
    if let Some(s) = secret {
        if let Some(ref sig) = signature {
            if !verify_signature(s, body.as_bytes(), sig) {
                eprintln!(
                    "{} Invalid signature from {}",
                    "!".red(),
                    addr
                );
                let response = "HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n";
                stream.write_all(response.as_bytes()).await?;
                return Ok(());
            }
        } else {
            eprintln!(
                "{} Missing signature from {}",
                "!".yellow(),
                addr
            );
        }
    }

    // Parse body as JSON
    let body_json: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => {
            let response = "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n";
            stream.write_all(response.as_bytes()).await?;
            return Ok(());
        }
    };

    // Output the event
    if json_output {
        println!("{}", serde_json::to_string(&body_json)?);
    } else {
        let action = body_json["action"].as_str().unwrap_or("unknown");
        let event_type = body_json["type"].as_str().unwrap_or("unknown");
        let timestamp = chrono::Utc::now().format("%H:%M:%S");

        println!(
            "[{}] {} {} {}",
            timestamp.to_string().dimmed(),
            event_type.cyan(),
            action.yellow(),
            format!("from {}", addr).dimmed()
        );

        // Show key data fields
        if let Some(data) = body_json.get("data") {
            if let Some(id) = data["id"].as_str() {
                print!("  ID: {}", id);
            }
            if let Some(title) = data["title"].as_str() {
                print!("  Title: {}", title);
            }
            if let Some(identifier) = data["identifier"].as_str() {
                print!("  Key: {}", identifier);
            }
            if let Some(name) = data["name"].as_str() {
                print!("  Name: {}", name);
            }
            println!();
        }
    }

    // Send 200 OK
    let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok";
    stream.write_all(response.as_bytes()).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_signature_valid() {
        let secret = "test-secret";
        let body = b"hello world";

        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let expected = hex::encode(mac.finalize().into_bytes());

        assert!(verify_signature(secret, body, &expected));
    }

    #[test]
    fn test_verify_signature_invalid() {
        assert!(!verify_signature("secret", b"body", "wrong-signature"));
    }

    #[test]
    fn test_verify_signature_empty_body() {
        let secret = "test-secret";
        let body = b"";

        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let expected = hex::encode(mac.finalize().into_bytes());

        assert!(verify_signature(secret, body, &expected));
    }
}
