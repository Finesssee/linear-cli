use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Default cache TTL in seconds (1 hour)
const DEFAULT_TTL_SECONDS: u64 = 3600;

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
    /// Check if the cache entry is still valid
    pub fn is_valid(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();
        now < self.timestamp + self.ttl_seconds
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
}

impl CacheType {
    /// Get the filename for this cache type
    pub fn filename(&self) -> &'static str {
        match self {
            CacheType::Teams => "teams.json",
            CacheType::Users => "users.json",
            CacheType::Statuses => "statuses.json",
            CacheType::Labels => "labels.json",
        }
    }

    /// Get display name for this cache type
    pub fn display_name(&self) -> &'static str {
        match self {
            CacheType::Teams => "Teams",
            CacheType::Users => "Users",
            CacheType::Statuses => "Statuses",
            CacheType::Labels => "Labels",
        }
    }

    /// Get all cache types
    pub fn all() -> &'static [CacheType] {
        &[
            CacheType::Teams,
            CacheType::Users,
            CacheType::Statuses,
            CacheType::Labels,
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

    /// Get the cache directory path
    fn cache_dir() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not find config directory")?
            .join("linear-cli")
            .join("cache");
        Ok(config_dir)
    }

    /// Get the path for a specific cache type
    fn cache_path(&self, cache_type: CacheType) -> PathBuf {
        self.cache_dir.join(cache_type.filename())
    }

    /// Get cached data if valid
    pub fn get(&self, cache_type: CacheType) -> Option<Value> {
        let path = self.cache_path(cache_type);
        if !path.exists() {
            return None;
        }

        let content = fs::read_to_string(&path).ok()?;
        let entry: CacheEntry = serde_json::from_str(&content).ok()?;

        if entry.is_valid() {
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

    /// Set cached data
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
        fs::write(path, content)?;
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

    /// Get cached data for a specific key within a cache type (e.g., statuses for a specific team)
    pub fn get_keyed(&self, cache_type: CacheType, key: &str) -> Option<Value> {
        let data = self.get(cache_type)?;
        data.get(key).cloned()
    }

    /// Set cached data for a specific key within a cache type
    pub fn set_keyed(&self, cache_type: CacheType, key: &str, value: Value) -> Result<()> {
        let mut data = self.get(cache_type).unwrap_or_else(|| json!({}));

        if let Some(obj) = data.as_object_mut() {
            obj.insert(key.to_string(), value);
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
}
