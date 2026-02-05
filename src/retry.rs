use std::time::Duration;
use crate::error::CliError;
use rand::Rng;
use tokio::time::sleep;

/// Retry configuration for API calls
#[derive(Debug, Clone, Copy)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub exponential_base: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 1000,
            max_delay_ms: 30000,
            exponential_base: 2.0,
        }
    }
}

impl RetryConfig {
    pub fn new(max_retries: u32) -> Self {
        Self {
            max_retries,
            ..Default::default()
        }
    }

    pub fn no_retry() -> Self {
        Self {
            max_retries: 0,
            ..Default::default()
        }
    }

    /// Calculate delay for a given attempt (0-indexed) with jitter
    pub fn delay_for_attempt(&self, attempt: u32, retry_after: Option<u64>) -> Duration {
        // If server specified retry-after, use that
        if let Some(seconds) = retry_after {
            return Duration::from_secs(seconds);
        }

        // Exponential backoff: initial_delay * base^attempt
        let delay_ms = (self.initial_delay_ms as f64 * self.exponential_base.powi(attempt as i32))
            .min(self.max_delay_ms as f64) as u64;

        // Add ±25% jitter to avoid thundering herd
        let jitter_range = (delay_ms / 4) as i64;
        let jitter = if jitter_range > 0 {
            rand::thread_rng().gen_range(-jitter_range..=jitter_range)
        } else {
            0
        };
        let final_delay = (delay_ms as i64 + jitter).max(0) as u64;

        Duration::from_millis(final_delay)
    }
}

/// Execute a function with retry logic
pub async fn with_retry<F, Fut, T, E>(
    config: &RetryConfig,
    mut f: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display + IsRetryable,
{
    let mut last_error: Option<E> = None;

    for attempt in 0..=config.max_retries {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if attempt < config.max_retries && e.is_retryable() {
                    let retry_after = e.retry_after();
                    let delay = config.delay_for_attempt(attempt, retry_after);
                    eprintln!(
                        "Attempt {} failed: {}. Retrying in {:?}...",
                        attempt + 1,
                        e,
                        delay
                    );
                    sleep(delay).await;
                    last_error = Some(e);
                } else {
                    return Err(e);
                }
            }
        }
    }

    Err(last_error.expect("Should have an error after retries"))
}

/// Trait to determine if an error is retryable
pub trait IsRetryable {
    fn is_retryable(&self) -> bool;
    fn retry_after(&self) -> Option<u64>;
}


impl IsRetryable for CliError {
    fn is_retryable(&self) -> bool {
        let msg = self.message.to_lowercase();
        self.code == 4
            || msg.contains("rate limit")
            || msg.contains("timeout")
            || msg.contains("temporarily unavailable")
            || msg.contains("503")
            || msg.contains("502")
            || msg.contains("504")
    }

    fn retry_after(&self) -> Option<u64> {
        self.retry_after
    }
}

impl IsRetryable for anyhow::Error {
    fn is_retryable(&self) -> bool {
        if let Some(cli) = self.downcast_ref::<CliError>() {
            return cli.is_retryable();
        }
        let msg = self.to_string().to_lowercase();
        // Retry on rate limits, timeouts, and transient network errors
        msg.contains("rate limit")
            || msg.contains("429")
            || msg.contains("timeout")
            || msg.contains("connection")
            || msg.contains("temporarily unavailable")
            || msg.contains("503")
            || msg.contains("502")
            || msg.contains("504")
    }

    fn retry_after(&self) -> Option<u64> {
        self.downcast_ref::<CliError>()
            .and_then(|cli| cli.retry_after)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_delay_ms, 1000);
        assert_eq!(config.max_delay_ms, 30000);
    }

    #[test]
    fn test_retry_config_new() {
        let config = RetryConfig::new(5);
        assert_eq!(config.max_retries, 5);
    }

    #[test]
    fn test_retry_config_no_retry() {
        let config = RetryConfig::no_retry();
        assert_eq!(config.max_retries, 0);
    }

    #[test]
    fn test_delay_with_retry_after() {
        let config = RetryConfig::default();
        let delay = config.delay_for_attempt(0, Some(10));
        assert_eq!(delay, Duration::from_secs(10));
    }

    #[test]
    fn test_delay_exponential_backoff() {
        let config = RetryConfig {
            max_retries: 3,
            initial_delay_ms: 1000,
            max_delay_ms: 30000,
            exponential_base: 2.0,
        };

        // Attempt 0: ~1000ms (with jitter)
        let delay0 = config.delay_for_attempt(0, None);
        assert!(delay0.as_millis() >= 750 && delay0.as_millis() <= 1250);

        // Attempt 1: ~2000ms (with jitter)
        let delay1 = config.delay_for_attempt(1, None);
        assert!(delay1.as_millis() >= 1500 && delay1.as_millis() <= 2500);

        // Attempt 2: ~4000ms (with jitter)
        let delay2 = config.delay_for_attempt(2, None);
        assert!(delay2.as_millis() >= 3000 && delay2.as_millis() <= 5000);
    }

    #[test]
    fn test_delay_capped_at_max() {
        let config = RetryConfig {
            max_retries: 10,
            initial_delay_ms: 1000,
            max_delay_ms: 5000,
            exponential_base: 2.0,
        };

        // Attempt 10 would be 1000 * 2^10 = 1024000ms, but should be capped
        let delay = config.delay_for_attempt(10, None);
        assert!(delay.as_millis() <= 6250); // max + 25% jitter
    }

    #[test]
    fn test_cli_error_retryable() {
        let rate_limit = CliError::new(4, "Rate limit exceeded");
        assert!(rate_limit.is_retryable());

        let timeout = CliError::new(1, "Request timeout");
        assert!(timeout.is_retryable());

        let server_error = CliError::new(1, "503 Service Unavailable");
        assert!(server_error.is_retryable());

        let auth_error = CliError::new(3, "Authentication failed");
        assert!(!auth_error.is_retryable());

        let not_found = CliError::new(2, "Issue not found");
        assert!(!not_found.is_retryable());
    }

    #[test]
    fn test_cli_error_retry_after() {
        let err = CliError::new(4, "Rate limited").with_retry_after(Some(30));
        assert_eq!(err.retry_after(), Some(30));

        let err_no_retry = CliError::new(1, "Error");
        assert_eq!(err_no_retry.retry_after(), None);
    }
}

