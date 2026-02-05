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

