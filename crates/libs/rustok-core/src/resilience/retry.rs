/// Retry Policy with Exponential Backoff
///
/// Automatically retries failed operations with configurable strategies.
use std::time::Duration;

/// Retry strategy
#[derive(Debug, Clone)]
pub enum RetryStrategy {
    /// Fixed delay between retries
    Fixed(Duration),

    /// Exponential backoff: delay = base * 2^attempt
    Exponential { base: Duration, max: Duration },

    /// Linear backoff: delay = base * attempt
    Linear { base: Duration, max: Duration },
}

impl RetryStrategy {
    pub fn delay(&self, attempt: u32) -> Duration {
        match self {
            RetryStrategy::Fixed(duration) => *duration,

            RetryStrategy::Exponential { base, max } => {
                let delay = base.mul_f64(2f64.powi(attempt as i32));
                delay.min(*max)
            }

            RetryStrategy::Linear { base, max } => {
                let delay = base.saturating_mul(attempt);
                delay.min(*max)
            }
        }
    }
}

/// Retry policy configuration
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts
    pub max_attempts: u32,

    /// Retry strategy
    pub strategy: RetryStrategy,

    /// Predicate to determine if error is retryable
    /// None means retry all errors
    pub retryable_predicate: Option<fn(&str) -> bool>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            strategy: RetryStrategy::Exponential {
                base: Duration::from_millis(100),
                max: Duration::from_secs(5),
            },
            retryable_predicate: None,
        }
    }
}

impl RetryPolicy {
    /// Execute operation with retry
    pub async fn execute<F, Fut, T, E>(&self, mut f: F) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        let mut attempt = 0;

        loop {
            match f().await {
                Ok(value) => return Ok(value),
                Err(err) => {
                    attempt += 1;

                    // Check if we should retry
                    let should_retry = if let Some(predicate) = self.retryable_predicate {
                        predicate(&err.to_string())
                    } else {
                        true
                    };

                    if !should_retry || attempt >= self.max_attempts {
                        return Err(err);
                    }

                    // Calculate delay and sleep
                    let delay = self.strategy.delay(attempt);

                    tracing::debug!(
                        attempt = attempt,
                        delay_ms = delay.as_millis(),
                        error = %err,
                        "Retrying operation"
                    );

                    tokio::time::sleep(delay).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn test_retry_success_on_second_attempt() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let policy = RetryPolicy::default();

        let result = policy
            .execute(|| async {
                let count = counter_clone.fetch_add(1, Ordering::SeqCst);

                if count == 0 {
                    Err("first attempt fails")
                } else {
                    Ok(42)
                }
            })
            .await;

        assert_eq!(result, Ok(42));
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_retry_exhausted() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let policy = RetryPolicy {
            max_attempts: 3,
            ..Default::default()
        };

        let result = policy
            .execute(|| async {
                counter_clone.fetch_add(1, Ordering::SeqCst);
                Err::<i32, _>("always fails")
            })
            .await;

        assert!(result.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_exponential_backoff() {
        let strategy = RetryStrategy::Exponential {
            base: Duration::from_millis(100),
            max: Duration::from_secs(10),
        };

        assert_eq!(strategy.delay(0), Duration::from_millis(100));
        assert_eq!(strategy.delay(1), Duration::from_millis(200));
        assert_eq!(strategy.delay(2), Duration::from_millis(400));
        assert_eq!(strategy.delay(3), Duration::from_millis(800));
    }

    #[test]
    fn test_linear_backoff() {
        let strategy = RetryStrategy::Linear {
            base: Duration::from_millis(100),
            max: Duration::from_secs(1),
        };

        assert_eq!(strategy.delay(1), Duration::from_millis(100));
        assert_eq!(strategy.delay(2), Duration::from_millis(200));
        assert_eq!(strategy.delay(3), Duration::from_millis(300));
    }
}
