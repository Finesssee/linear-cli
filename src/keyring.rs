//! Secure API key storage using OS keyring.
//!
//! This module provides cross-platform credential storage:
//! - macOS: Keychain
//! - Windows: Credential Manager
//! - Linux: Secret Service (requires D-Bus and a keyring daemon)

use anyhow::{Context, Result};

const SERVICE_NAME: &str = "linear-cli";

/// Get an API key from the keyring for a profile.
/// Returns Ok(None) if no key is stored, Ok(Some(key)) if found.
pub fn get_key(profile: &str) -> Result<Option<String>> {
    let entry =
        keyring::Entry::new(SERVICE_NAME, profile).context("Failed to create keyring entry")?;

    match entry.get_password() {
        Ok(password) => Ok(Some(password)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(keyring::Error::NoStorageAccess(_)) => {
            if !crate::output::is_quiet() {
                eprintln!("Warning: Keyring not available, falling back to config file");
            }
            Ok(None)
        }
        Err(e) => {
            if !crate::output::is_quiet() {
                eprintln!(
                    "Warning: Keyring error ({}), falling back to config file",
                    e
                );
            }
            Ok(None)
        }
    }
}

/// Store an API key in the keyring for a profile.
pub fn set_key(profile: &str, api_key: &str) -> Result<()> {
    let entry =
        keyring::Entry::new(SERVICE_NAME, profile).context("Failed to create keyring entry")?;

    entry
        .set_password(api_key)
        .context("Failed to store API key in keyring")?;

    Ok(())
}

/// Delete an API key from the keyring for a profile.
/// Returns Ok(()) even if no key was stored.
pub fn delete_key(profile: &str) -> Result<()> {
    let entry =
        keyring::Entry::new(SERVICE_NAME, profile).context("Failed to create keyring entry")?;

    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()), // Already gone, that's fine
        Err(e) => Err(e).context("Failed to delete API key from keyring"),
    }
}

/// Check if keyring is available on this system.
pub fn is_available() -> bool {
    // Try to create an entry and check if we can access it
    match keyring::Entry::new(SERVICE_NAME, "__test__") {
        Ok(entry) => {
            // Try a non-destructive operation
            match entry.get_password() {
                Err(keyring::Error::NoStorageAccess(_)) => false,
                _ => true, // NoEntry or Ok means storage is accessible
            }
        }
        Err(_) => false,
    }
}

const OAUTH_SERVICE_NAME: &str = "linear-cli-oauth";

/// Get OAuth tokens JSON from keyring for a profile
pub fn get_oauth_tokens(profile: &str) -> Result<Option<String>> {
    let entry = keyring::Entry::new(OAUTH_SERVICE_NAME, profile)
        .context("Failed to create keyring entry")?;

    match entry.get_password() {
        Ok(json) => Ok(Some(json)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(keyring::Error::NoStorageAccess(_)) => Ok(None),
        Err(e) => {
            if !crate::output::is_quiet() {
                eprintln!(
                    "Warning: Keyring OAuth error ({}), falling back to config",
                    e
                );
            }
            Ok(None)
        }
    }
}

/// Store OAuth tokens JSON in keyring for a profile
pub fn set_oauth_tokens(profile: &str, json: &str) -> Result<()> {
    let entry = keyring::Entry::new(OAUTH_SERVICE_NAME, profile)
        .context("Failed to create keyring entry")?;
    entry
        .set_password(json)
        .context("Failed to store OAuth tokens in keyring")?;
    Ok(())
}

/// Delete OAuth tokens from keyring for a profile
pub fn delete_oauth_tokens(profile: &str) -> Result<()> {
    let entry = keyring::Entry::new(OAUTH_SERVICE_NAME, profile)
        .context("Failed to create keyring entry")?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e).context("Failed to delete OAuth tokens from keyring"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PROFILE: &str = "linear-cli-test-profile";
    const TEST_KEY: &str = "lin_api_test_key_12345";

    #[test]
    fn test_is_available() {
        // Just check it doesn't panic - availability depends on system
        let available = is_available();
        println!("Keyring available: {}", available);
    }

    #[test]
    fn test_set_get_delete_key() {
        if !is_available() {
            eprintln!("Skipping keyring test - keyring not available");
            return;
        }

        // Clean up any leftover from previous test runs
        let _ = delete_key(TEST_PROFILE);

        // Set a key - check for errors
        if let Err(e) = set_key(TEST_PROFILE, TEST_KEY) {
            eprintln!("Skipping test - set_key failed: {}", e);
            return;
        }

        // Get the key back
        match get_key(TEST_PROFILE) {
            Ok(Some(key)) => {
                assert_eq!(key, TEST_KEY, "Key should match");
            }
            Ok(None) => {
                // Some systems (like CI) may have keyring available but not persistent
                eprintln!("Warning: Key not found after set - keyring may not be persistent in this environment");
            }
            Err(e) => {
                eprintln!("Warning: get_key failed: {}", e);
            }
        }

        // Clean up
        let _ = delete_key(TEST_PROFILE);
    }

    #[test]
    fn test_delete_nonexistent_key() {
        if !is_available() {
            eprintln!("Skipping keyring test - keyring not available");
            return;
        }

        // Deleting a key that doesn't exist should not error
        let result = delete_key("nonexistent-profile-xyz");
        assert!(result.is_ok(), "Deleting nonexistent key should succeed");
    }

    #[test]
    fn test_overwrite_key() {
        if !is_available() {
            eprintln!("Skipping keyring test - keyring not available");
            return;
        }

        let profile = "linear-cli-test-overwrite";
        let _ = delete_key(profile); // Clean up

        // Set initial key - check for errors
        if let Err(e) = set_key(profile, "key1") {
            eprintln!("Skipping test - set_key failed: {}", e);
            return;
        }

        // Verify or skip if not persistent
        match get_key(profile) {
            Ok(Some(key)) if key == "key1" => {
                // Overwrite with new key
                set_key(profile, "key2").expect("Failed to set key2");
                if let Ok(Some(key2)) = get_key(profile) {
                    assert_eq!(key2, "key2", "Overwritten key should match");
                }
            }
            _ => {
                eprintln!("Warning: Keyring not persistent in this environment");
            }
        }

        // Clean up
        let _ = delete_key(profile);
    }
}
