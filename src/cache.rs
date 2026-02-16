use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::config;

/// Default cache TTL in seconds (1 hour)
const DEFAULT_TTL_SECONDS: u64 = 3600;

#[derive(Debug, Clone, Copy, Default)]
pub struct CacheOptions {
    pub ttl_seconds: Option<u64>,
    pub no_cache: bool,
}

impl CacheOptions {
    pub fn effective_ttl_seconds(&self) -> u64 {
        self.ttl_seconds.unwrap_or(DEFAULT_TTL_SECONDS)
    }
}

/// Cache entry with timestamp and data
#[derive(Debug, Serialize, Deserialize)]
pub struct CacheEntry {
    /// Unix timestamp when the cache was created
    pub timestamp: u64,
    /// TTL in seconds for this cache entry
    pub ttl_seconds: u64,
    /// The cached data
    pub data: Value,
}

impl CacheEntry {
    /// Check if the cache entry is still valid using its stored TTL
    pub fn is_valid(&self) -> bool {
        self.is_valid_with_ttl(self.ttl_seconds)
    }

    /// Check if the cache entry is still valid using a custom TTL override
    pub fn is_valid_with_ttl(&self, ttl_seconds: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();
        now < self.timestamp + ttl_seconds
    }

    /// Get the age of the cache entry in seconds
    pub fn age_seconds(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();
        now.saturating_sub(self.timestamp)
    }
}

/// Cache types supported by the CLI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheType {
    Teams,
    Users,
    Statuses,
    Labels,
    Projects,
}

impl CacheType {
    /// Get the filename for this cache type
    pub fn filename(&self) -> &'static str {
        match self {
            CacheType::Teams => "teams.json",
            CacheType::Users => "users.json",
            CacheType::Statuses => "statuses.json",
            CacheType::Labels => "labels.json",
            CacheType::Projects => "projects.json",
        }
    }

    /// Get display name for this cache type
    pub fn display_name(&self) -> &'static str {
        match self {
            CacheType::Teams => "Teams",
            CacheType::Users => "Users",
            CacheType::Statuses => "Statuses",
            CacheType::Labels => "Labels",
            CacheType::Projects => "Projects",
        }
    }

    /// Get all cache types
    pub fn all() -> &'static [CacheType] {
        &[
            CacheType::Teams,
            CacheType::Users,
            CacheType::Statuses,
            CacheType::Labels,
            CacheType::Projects,
        ]
    }
}

/// Cache manager for Linear CLI
pub struct Cache {
    cache_dir: PathBuf,
    ttl_seconds: u64,
}

impl Cache {
    /// Create a new cache instance with default TTL
    pub fn new() -> Result<Self> {
        Self::with_ttl(DEFAULT_TTL_SECONDS)
    }

    /// Create a new cache instance with custom TTL in seconds
    pub fn with_ttl(ttl_seconds: u64) -> Result<Self> {
        let cache_dir = Self::cache_dir()?;
        fs::create_dir_all(&cache_dir)?;
        Ok(Self {
            cache_dir,
            ttl_seconds,
        })
    }

    /// Get the cache directory path, scoped by workspace/profile
    pub(crate) fn cache_dir() -> Result<PathBuf> {
        static CACHE_DIR: OnceLock<PathBuf> = OnceLock::new();

        if let Some(cached) = CACHE_DIR.get() {
            return Ok(cached.clone());
        }

        let profile = config::current_profile().unwrap_or_else(|_| "default".to_string());
        let config_dir = dirs::config_dir()
            .context("Could not find config directory")?
            .join("linear-cli")
            .join("cache")
            .join(profile);

        // Store for future calls (ignore if another thread beat us)
        let _ = CACHE_DIR.set(config_dir.clone());
        Ok(config_dir)
    }

    /// Get the path for a specific cache type
    fn cache_path(&self, cache_type: CacheType) -> PathBuf {
        self.cache_dir.join(cache_type.filename())
    }

    /// Get cached data if valid (uses the instance's TTL for expiry checks)
    pub fn get(&self, cache_type: CacheType) -> Option<Value> {
        let path = self.cache_path(cache_type);
        if !path.exists() {
            return None;
        }

        let content = fs::read_to_string(&path).ok()?;
        let entry: CacheEntry = serde_json::from_str(&content).ok()?;

        if entry.is_valid_with_ttl(self.ttl_seconds) {
            Some(entry.data)
        } else {
            // Cache expired, remove it
            let _ = fs::remove_file(&path);
            None
        }
    }

    /// Get cache entry with metadata
    pub fn get_entry(&self, cache_type: CacheType) -> Option<CacheEntry> {
        let path = self.cache_path(cache_type);
        if !path.exists() {
            return None;
        }

        let content = fs::read_to_string(&path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Set cached data using atomic file writes
    pub fn set(&self, cache_type: CacheType, data: Value) -> Result<()> {
        let path = self.cache_path(cache_type);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();

        let entry = CacheEntry {
            timestamp,
            ttl_seconds: self.ttl_seconds,
            data,
        };

        let content = serde_json::to_string_pretty(&entry)?;

        // Atomic write: write to temp file, sync, then rename
        // Use secure permissions on Unix (0600)
        let temp_path = path.with_extension("tmp");

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&temp_path)?;
            file.write_all(content.as_bytes())?;
            file.sync_all()?;
        }

        #[cfg(not(unix))]
        {
            let mut file = fs::File::create(&temp_path)?;
            file.write_all(content.as_bytes())?;
            file.sync_all()?;
        }

        // Atomic rename
        // On Windows, fs::rename fails if the destination exists, so remove it first
        #[cfg(windows)]
        {
            let _ = fs::remove_file(&path);
        }
        fs::rename(&temp_path, &path)?;
        Ok(())
    }

    /// Clear cache for a specific type
    pub fn clear_type(&self, cache_type: CacheType) -> Result<()> {
        let path = self.cache_path(cache_type);
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    /// Clear all cached data
    pub fn clear_all(&self) -> Result<()> {
        for cache_type in CacheType::all() {
            self.clear_type(*cache_type)?;
        }
        Ok(())
    }

    /// Get cached data for a specific key within a cache type (e.g., statuses for a specific team).
    /// Uses per-key timestamps to check validity, so updating one key doesn't refresh others.
    pub fn get_keyed(&self, cache_type: CacheType, key: &str) -> Option<Value> {
        let path = self.cache_path(cache_type);
        if !path.exists() {
            return None;
        }

        let content = fs::read_to_string(&path).ok()?;
        let entry: CacheEntry = serde_json::from_str(&content).ok()?;
        let wrapper = entry.data.get(key)?;

        // New format: {"data": ..., "timestamp": unix_seconds}
        if let (Some(data), Some(ts)) = (wrapper.get("data"), wrapper.get("timestamp")) {
            let timestamp = ts.as_u64()?;
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_secs();
            if now < timestamp + self.ttl_seconds {
                return Some(data.clone());
            }
            return None;
        }

        // Old format (no per-key timestamp): treat as expired for backwards compatibility
        None
    }

    /// Set cached data for a specific key within a cache type.
    /// Stores a per-key timestamp so each key expires independently.
    pub fn set_keyed(&self, cache_type: CacheType, key: &str, value: Value) -> Result<()> {
        // Read existing file data without TTL checks (we manage per-key TTLs)
        let path = self.cache_path(cache_type);
        let mut data = if path.exists() {
            fs::read_to_string(&path)
                .ok()
                .and_then(|content| serde_json::from_str::<CacheEntry>(&content).ok())
                .map(|entry| entry.data)
                .unwrap_or_else(|| json!({}))
        } else {
            json!({})
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();

        if let Some(obj) = data.as_object_mut() {
            obj.insert(
                key.to_string(),
                json!({
                    "data": value,
                    "timestamp": now,
                }),
            );
        }

        self.set(cache_type, data)
    }

    /// Get cache status for all types
    pub fn status(&self) -> Vec<CacheStatus> {
        CacheType::all()
            .iter()
            .map(|cache_type| {
                let path = self.cache_path(*cache_type);
                let (valid, age_seconds, size_bytes, item_count) = if path.exists() {
                    if let Some(entry) = self.get_entry(*cache_type) {
                        let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                        let count = entry
                            .data
                            .as_array()
                            .map(|a| a.len())
                            .or_else(|| {
                                // Handle nested nodes structure
                                entry
                                    .data
                                    .get("nodes")
                                    .and_then(|n| n.as_array())
                                    .map(|a| a.len())
                            })
                            .unwrap_or(1);
                        (
                            entry.is_valid(),
                            Some(entry.age_seconds()),
                            Some(size),
                            Some(count),
                        )
                    } else {
                        (false, None, None, None)
                    }
                } else {
                    (false, None, None, None)
                };

                CacheStatus {
                    cache_type: *cache_type,
                    valid,
                    age_seconds,
                    size_bytes,
                    item_count,
                }
            })
            .collect()
    }
}

pub fn cache_dir_path() -> Result<PathBuf> {
    Cache::cache_dir()
}

/// Status information for a cache type
#[derive(Debug)]
pub struct CacheStatus {
    pub cache_type: CacheType,
    pub valid: bool,
    pub age_seconds: Option<u64>,
    pub size_bytes: Option<u64>,
    pub item_count: Option<usize>,
}

impl CacheStatus {
    /// Format age as human-readable string
    pub fn age_display(&self) -> String {
        match self.age_seconds {
            Some(secs) if secs < 60 => format!("{}s", secs),
            Some(secs) if secs < 3600 => format!("{}m", secs / 60),
            Some(secs) => format!("{}h {}m", secs / 3600, (secs % 3600) / 60),
            None => "-".to_string(),
        }
    }

    /// Format size as human-readable string
    pub fn size_display(&self) -> String {
        match self.size_bytes {
            Some(bytes) if bytes < 1024 => format!("{} B", bytes),
            Some(bytes) if bytes < 1024 * 1024 => format!("{:.1} KB", bytes as f64 / 1024.0),
            Some(bytes) => format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0)),
            None => "-".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_entry_validity() {
        let entry = CacheEntry {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            ttl_seconds: 3600,
            data: serde_json::json!({"test": "data"}),
        };
        assert!(entry.is_valid());
    }

    #[test]
    fn test_cache_entry_expired() {
        let entry = CacheEntry {
            timestamp: 0, // Very old timestamp
            ttl_seconds: 3600,
            data: serde_json::json!({"test": "data"}),
        };
        assert!(!entry.is_valid());
    }

    #[test]
    fn test_cache_entry_age() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let entry = CacheEntry {
            timestamp: now - 60, // 60 seconds ago
            ttl_seconds: 3600,
            data: serde_json::json!({}),
        };
        let age = entry.age_seconds();
        assert!(age >= 60 && age <= 62); // Allow small drift
    }

    #[test]
    fn test_cache_type_filename() {
        assert_eq!(CacheType::Teams.filename(), "teams.json");
        assert_eq!(CacheType::Users.filename(), "users.json");
        assert_eq!(CacheType::Statuses.filename(), "statuses.json");
        assert_eq!(CacheType::Labels.filename(), "labels.json");
        assert_eq!(CacheType::Projects.filename(), "projects.json");
    }

    #[test]
    fn test_cache_type_display_name() {
        assert_eq!(CacheType::Teams.display_name(), "Teams");
        assert_eq!(CacheType::Users.display_name(), "Users");
    }

    #[test]
    fn test_cache_type_all() {
        let all = CacheType::all();
        assert_eq!(all.len(), 5);
    }

    #[test]
    fn test_cache_options_effective_ttl() {
        let opts_default = CacheOptions::default();
        assert_eq!(opts_default.effective_ttl_seconds(), DEFAULT_TTL_SECONDS);

        let opts_custom = CacheOptions {
            ttl_seconds: Some(7200),
            no_cache: false,
        };
        assert_eq!(opts_custom.effective_ttl_seconds(), 7200);
    }

    #[test]
    fn test_cache_status_age_display() {
        let status_seconds = CacheStatus {
            cache_type: CacheType::Teams,
            valid: true,
            age_seconds: Some(45),
            size_bytes: None,
            item_count: None,
        };
        assert_eq!(status_seconds.age_display(), "45s");

        let status_minutes = CacheStatus {
            cache_type: CacheType::Teams,
            valid: true,
            age_seconds: Some(120),
            size_bytes: None,
            item_count: None,
        };
        assert_eq!(status_minutes.age_display(), "2m");

        let status_hours = CacheStatus {
            cache_type: CacheType::Teams,
            valid: true,
            age_seconds: Some(3660),
            size_bytes: None,
            item_count: None,
        };
        assert_eq!(status_hours.age_display(), "1h 1m");
    }

    #[test]
    fn test_cache_status_size_display() {
        let status_bytes = CacheStatus {
            cache_type: CacheType::Teams,
            valid: true,
            age_seconds: None,
            size_bytes: Some(512),
            item_count: None,
        };
        assert_eq!(status_bytes.size_display(), "512 B");

        let status_kb = CacheStatus {
            cache_type: CacheType::Teams,
            valid: true,
            age_seconds: None,
            size_bytes: Some(2048),
            item_count: None,
        };
        assert_eq!(status_kb.size_display(), "2.0 KB");

        let status_mb = CacheStatus {
            cache_type: CacheType::Teams,
            valid: true,
            age_seconds: None,
            size_bytes: Some(1048576),
            item_count: None,
        };
        assert_eq!(status_mb.size_display(), "1.0 MB");
    }
}
