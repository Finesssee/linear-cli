use anyhow::{Context, Result};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Write};
use tokio::net::TcpListener;

const LINEAR_AUTHORIZE_URL: &str = "https://linear.app/oauth/authorize";
const LINEAR_TOKEN_URL: &str = "https://api.linear.app/oauth/token";
const LINEAR_REVOKE_URL: &str = "https://api.linear.app/oauth/revoke";

/// Default client_id for linear-cli OAuth app (registered with Linear)
pub const DEFAULT_CLIENT_ID: &str = "linear-cli-default";

/// PKCE challenge pair
pub struct PkceChallenge {
    pub verifier: String,
    pub challenge: String,
}

impl PkceChallenge {
    pub fn generate() -> Self {
        // Generate 32 random bytes, base64url-encode to get verifier
        let mut rng = rand::thread_rng();
        let random_bytes: Vec<u8> = (0..32).map(|_| rng.gen::<u8>()).collect();
        let verifier = base64_url_encode(&random_bytes);

        // S256: SHA256(verifier) then base64url-encode
        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let hash = hasher.finalize();
        let challenge = base64_url_encode(&hash);

        Self { verifier, challenge }
    }
}

/// OAuth tokens received from Linear
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>, // Unix timestamp
    pub token_type: String,
    pub scope: Option<String>,
}

/// Base64 URL-safe encoding without padding (per RFC 7636)
fn base64_url_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

/// Build the authorization URL for Linear OAuth
pub fn build_authorize_url(
    client_id: &str,
    redirect_uri: &str,
    scopes: &str,
    state: &str,
    pkce: &PkceChallenge,
) -> String {
    let mut url = url::Url::parse(LINEAR_AUTHORIZE_URL).expect("valid authorize URL");
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", scopes)
        .append_pair("state", state)
        .append_pair("code_challenge", &pkce.challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("prompt", "consent");
    url.to_string()
}

/// Start a temporary HTTP server on localhost to receive the OAuth callback.
/// Returns the authorization code from the callback.
pub async fn wait_for_callback(port: u16, expected_state: &str) -> Result<String> {
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr)
        .await
        .context(format!(
            "Failed to bind to {}. Is another process using this port?",
            addr
        ))?;

    // Use a timeout so we don't hang forever
    let timeout = tokio::time::Duration::from_secs(300); // 5 minutes
    let (stream, _) = tokio::time::timeout(timeout, listener.accept())
        .await
        .context("Timed out waiting for OAuth callback (5 minutes)")?
        .context("Failed to accept connection")?;

    // Convert to std stream for sync read/write
    let std_stream = stream.into_std()?;
    std_stream.set_nonblocking(false)?;
    let mut reader = BufReader::new(&std_stream);

    // Read the HTTP request line
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    // Parse the URL from "GET /callback?code=xxx&state=yyy HTTP/1.1"
    let path = request_line
        .split_whitespace()
        .nth(1)
        .context("Invalid HTTP request")?;

    let full_url = format!("http://localhost{}", path);
    let parsed = url::Url::parse(&full_url).context("Failed to parse callback URL")?;

    // Extract query parameters
    let params: std::collections::HashMap<String, String> = parsed
        .query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    // Check for error
    if let Some(error) = params.get("error") {
        let desc = params
            .get("error_description")
            .cloned()
            .unwrap_or_default();
        // Send error response to browser
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
             <html><body><h2>Authentication Failed</h2>\
             <p>{}: {}</p>\
             <p>You can close this window.</p></body></html>",
            error, desc
        );
        let mut writer = &std_stream;
        let _ = writer.write_all(response.as_bytes());
        anyhow::bail!("OAuth error: {} - {}", error, desc);
    }

    // Validate state
    let state = params
        .get("state")
        .context("Missing state parameter in callback")?;
    if state != expected_state {
        let response = "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\n\r\n\
             <html><body><h2>State Mismatch</h2>\
             <p>Possible CSRF attack. Please try again.</p></body></html>";
        let mut writer = &std_stream;
        let _ = writer.write_all(response.as_bytes());
        anyhow::bail!("OAuth state mismatch - possible CSRF attack");
    }

    let code = params
        .get("code")
        .context("Missing authorization code in callback")?
        .clone();

    // Send success response to browser
    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
         <html><body><h2>Authentication Successful!</h2>\
         <p>You can close this window and return to the terminal.</p></body></html>";
    let mut writer = &std_stream;
    let _ = writer.write_all(response.as_bytes());

    Ok(code)
}

/// Exchange authorization code for tokens
pub async fn exchange_code(
    client_id: &str,
    redirect_uri: &str,
    code: &str,
    verifier: &str,
) -> Result<OAuthTokens> {
    let client = reqwest::Client::new();
    let response = client
        .post(LINEAR_TOKEN_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("redirect_uri", redirect_uri),
            ("code", code),
            ("code_verifier", verifier),
        ])
        .send()
        .await
        .context("Failed to exchange authorization code")?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Token exchange failed (HTTP {}): {}", status, body);
    }

    let token_response: serde_json::Value = response
        .json()
        .await
        .context("Failed to parse token response")?;

    let access_token = token_response["access_token"]
        .as_str()
        .context("Missing access_token in response")?
        .to_string();

    let refresh_token = token_response["refresh_token"]
        .as_str()
        .map(|s| s.to_string());

    let expires_in = token_response["expires_in"].as_i64();
    let expires_at = expires_in.map(|e| chrono::Utc::now().timestamp() + e);

    let token_type = token_response["token_type"]
        .as_str()
        .unwrap_or("Bearer")
        .to_string();

    let scope = token_response["scope"]
        .as_str()
        .map(|s| s.to_string());

    Ok(OAuthTokens {
        access_token,
        refresh_token,
        expires_at,
        token_type,
        scope,
    })
}

/// Refresh an access token using a refresh token
pub async fn refresh_tokens(client_id: &str, refresh_token: &str) -> Result<OAuthTokens> {
    let client = reqwest::Client::new();
    let response = client
        .post(LINEAR_TOKEN_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await
        .context("Failed to refresh token")?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Token refresh failed (HTTP {}): {}", status, body);
    }

    let token_response: serde_json::Value = response
        .json()
        .await
        .context("Failed to parse refresh response")?;

    let access_token = token_response["access_token"]
        .as_str()
        .context("Missing access_token in refresh response")?
        .to_string();

    let new_refresh = token_response["refresh_token"]
        .as_str()
        .map(|s| s.to_string())
        .or_else(|| Some(refresh_token.to_string()));

    let expires_in = token_response["expires_in"].as_i64();
    let expires_at = expires_in.map(|e| chrono::Utc::now().timestamp() + e);

    let token_type = token_response["token_type"]
        .as_str()
        .unwrap_or("Bearer")
        .to_string();

    let scope = token_response["scope"]
        .as_str()
        .map(|s| s.to_string());

    Ok(OAuthTokens {
        access_token,
        refresh_token: new_refresh,
        expires_at,
        token_type,
        scope,
    })
}

/// Revoke an OAuth token
pub async fn revoke_token(token: &str) -> Result<()> {
    let client = reqwest::Client::new();
    let response = client
        .post(LINEAR_REVOKE_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&[("token", token)])
        .send()
        .await
        .context("Failed to revoke token")?;

    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Token revocation failed: {}", body);
    }

    Ok(())
}

/// Check if OAuth tokens are expired (with 5-minute buffer)
pub fn is_expired(tokens: &OAuthTokens) -> bool {
    match tokens.expires_at {
        Some(expires_at) => {
            let buffer = 300; // 5 minutes
            chrono::Utc::now().timestamp() >= (expires_at - buffer)
        }
        None => false, // No expiry = doesn't expire (legacy tokens)
    }
}

/// Generate a random state string for CSRF protection
pub fn generate_state() -> String {
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..16).map(|_| rng.gen::<u8>()).collect();
    base64_url_encode(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pkce_challenge_generation() {
        let pkce = PkceChallenge::generate();
        // Verifier should be non-empty base64url string
        assert!(!pkce.verifier.is_empty());
        assert!(!pkce.challenge.is_empty());
        // Verifier and challenge should be different
        assert_ne!(pkce.verifier, pkce.challenge);
    }

    #[test]
    fn test_pkce_challenge_deterministic_s256() {
        // Verify that SHA256(verifier) == challenge
        let pkce = PkceChallenge::generate();
        let mut hasher = Sha256::new();
        hasher.update(pkce.verifier.as_bytes());
        let hash = hasher.finalize();
        let expected_challenge = base64_url_encode(&hash);
        assert_eq!(pkce.challenge, expected_challenge);
    }

    #[test]
    fn test_pkce_uniqueness() {
        let pkce1 = PkceChallenge::generate();
        let pkce2 = PkceChallenge::generate();
        assert_ne!(pkce1.verifier, pkce2.verifier);
    }

    #[test]
    fn test_build_authorize_url() {
        let pkce = PkceChallenge::generate();
        let url = build_authorize_url(
            "test-client",
            "http://localhost:8484/callback",
            "read,write",
            "test-state",
            &pkce,
        );
        assert!(url.contains("client_id=test-client"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("scope=read"));
        assert!(url.contains("state=test-state"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains(&format!("code_challenge={}", pkce.challenge)));
    }

    #[test]
    fn test_build_authorize_url_base() {
        let pkce = PkceChallenge::generate();
        let url = build_authorize_url("c", "http://localhost:8484/callback", "read", "s", &pkce);
        assert!(url.starts_with("https://linear.app/oauth/authorize?"));
    }

    #[test]
    fn test_is_expired_future() {
        let tokens = OAuthTokens {
            access_token: "test".to_string(),
            refresh_token: None,
            expires_at: Some(chrono::Utc::now().timestamp() + 3600), // 1 hour from now
            token_type: "Bearer".to_string(),
            scope: None,
        };
        assert!(!is_expired(&tokens));
    }

    #[test]
    fn test_is_expired_past() {
        let tokens = OAuthTokens {
            access_token: "test".to_string(),
            refresh_token: None,
            expires_at: Some(chrono::Utc::now().timestamp() - 100), // 100s ago
            token_type: "Bearer".to_string(),
            scope: None,
        };
        assert!(is_expired(&tokens));
    }

    #[test]
    fn test_is_expired_within_buffer() {
        let tokens = OAuthTokens {
            access_token: "test".to_string(),
            refresh_token: None,
            expires_at: Some(chrono::Utc::now().timestamp() + 200), // 200s from now (within 5min buffer)
            token_type: "Bearer".to_string(),
            scope: None,
        };
        assert!(is_expired(&tokens)); // Should be "expired" due to buffer
    }

    #[test]
    fn test_is_expired_no_expiry() {
        let tokens = OAuthTokens {
            access_token: "test".to_string(),
            refresh_token: None,
            expires_at: None,
            token_type: "Bearer".to_string(),
            scope: None,
        };
        assert!(!is_expired(&tokens)); // No expiry = never expires
    }

    #[test]
    fn test_generate_state() {
        let state1 = generate_state();
        let state2 = generate_state();
        assert!(!state1.is_empty());
        assert_ne!(state1, state2);
    }

    #[test]
    fn test_base64_url_encode() {
        let data = b"hello world";
        let encoded = base64_url_encode(data);
        assert!(!encoded.contains('+'));
        assert!(!encoded.contains('/'));
        assert!(!encoded.contains('='));
    }

    #[test]
    fn test_oauth_tokens_serialize() {
        let tokens = OAuthTokens {
            access_token: "acc".to_string(),
            refresh_token: Some("ref".to_string()),
            expires_at: Some(1700000000),
            token_type: "Bearer".to_string(),
            scope: Some("read,write".to_string()),
        };
        let json = serde_json::to_string(&tokens).unwrap();
        let parsed: OAuthTokens = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.access_token, "acc");
        assert_eq!(parsed.refresh_token, Some("ref".to_string()));
        assert_eq!(parsed.expires_at, Some(1700000000));
    }

    #[test]
    fn test_pkce_verifier_length() {
        // RFC 7636 requires verifier to be 43-128 characters
        let pkce = PkceChallenge::generate();
        assert!(pkce.verifier.len() >= 43, "verifier should be at least 43 chars, got {}", pkce.verifier.len());
        assert!(pkce.verifier.len() <= 128, "verifier should be at most 128 chars, got {}", pkce.verifier.len());
    }

    #[test]
    fn test_pkce_verifier_charset() {
        // RFC 7636: verifier uses unreserved characters [A-Z] / [a-z] / [0-9] / "-" / "." / "_" / "~"
        let pkce = PkceChallenge::generate();
        for c in pkce.verifier.chars() {
            assert!(
                c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_' || c == '~',
                "verifier contains invalid character: '{}' (0x{:02x})",
                c, c as u32
            );
        }
    }

    #[test]
    fn test_oauth_tokens_minimal_deserialize() {
        // Minimal token response (no refresh, no expiry, no scope)
        let json = r#"{"access_token":"tok","token_type":"Bearer"}"#;
        let tokens: OAuthTokens = serde_json::from_str(json).unwrap();
        assert_eq!(tokens.access_token, "tok");
        assert_eq!(tokens.token_type, "Bearer");
        assert!(tokens.refresh_token.is_none());
        assert!(tokens.expires_at.is_none());
        assert!(tokens.scope.is_none());
    }

    #[test]
    fn test_oauth_tokens_full_deserialize() {
        let json = r#"{
            "access_token": "lin_oauth_abc",
            "refresh_token": "lin_refresh_xyz",
            "expires_at": 1700086400,
            "token_type": "Bearer",
            "scope": "read,write,issues:create"
        }"#;
        let tokens: OAuthTokens = serde_json::from_str(json).unwrap();
        assert_eq!(tokens.access_token, "lin_oauth_abc");
        assert_eq!(tokens.refresh_token.as_deref(), Some("lin_refresh_xyz"));
        assert_eq!(tokens.expires_at, Some(1700086400));
        assert_eq!(tokens.scope.as_deref(), Some("read,write,issues:create"));
    }

    #[test]
    fn test_build_authorize_url_encodes_special_chars() {
        let pkce = PkceChallenge::generate();
        let url = build_authorize_url(
            "client with spaces",
            "http://localhost:8484/callback",
            "read,write",
            "state+special/chars",
            &pkce,
        );
        // URL should encode spaces and special chars
        assert!(url.contains("client+with+spaces") || url.contains("client%20with%20spaces"));
        assert!(!url.contains(' '), "URL should not contain raw spaces");
    }

    #[test]
    fn test_build_authorize_url_includes_prompt() {
        let pkce = PkceChallenge::generate();
        let url = build_authorize_url("c", "http://localhost:8484/callback", "read", "s", &pkce);
        assert!(url.contains("prompt=consent"), "URL should include prompt=consent");
    }

    #[test]
    fn test_is_expired_exactly_at_buffer_boundary() {
        // Exactly 300 seconds (5 min buffer) from now — should be considered expired
        let tokens = OAuthTokens {
            access_token: "test".to_string(),
            refresh_token: None,
            expires_at: Some(chrono::Utc::now().timestamp() + 300),
            token_type: "Bearer".to_string(),
            scope: None,
        };
        assert!(is_expired(&tokens), "token expiring exactly at buffer boundary should be expired");
    }

    #[test]
    fn test_is_expired_just_past_buffer() {
        // 301 seconds from now — just past the buffer, should NOT be expired
        let tokens = OAuthTokens {
            access_token: "test".to_string(),
            refresh_token: None,
            expires_at: Some(chrono::Utc::now().timestamp() + 301),
            token_type: "Bearer".to_string(),
            scope: None,
        };
        assert!(!is_expired(&tokens), "token expiring 301s from now should not be expired");
    }

    #[test]
    fn test_generate_state_length() {
        let state = generate_state();
        // 16 random bytes base64-encoded = ~22 chars
        assert!(state.len() >= 20, "state should be at least 20 chars, got {}", state.len());
    }

    #[test]
    fn test_base64_url_encode_empty() {
        let encoded = base64_url_encode(&[]);
        assert!(encoded.is_empty(), "encoding empty data should produce empty string");
    }

    #[test]
    fn test_base64_url_encode_known_value() {
        // Known test vector: base64url("test") = "dGVzdA"
        let encoded = base64_url_encode(b"test");
        assert_eq!(encoded, "dGVzdA");
    }
}
