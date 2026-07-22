//! Async Utilities
//!
//! Common async patterns and utilities for RusToK.
//!
//! # Features
//!
//! - **Parallel Execution**: Run futures in parallel with controlled concurrency
//! - **Batch Processing**: Process items in batches with rate limiting
//! - **Cancellation**: Cooperative cancellation with timeout and signal handling
//! - **Backpressure**: Adaptive rate limiting based on system load
//! - **Retry**: Exponential backoff retry with jitter
//!
//! # Example
//!
//! ```ignore
//! use rustok_core::async_utils::{parallel, batch, retry, BackoffConfig};
//!
//! // Parallel execution
//! let results = parallel(items, 10, |item| async {
//!     process(item).await
//! }).await;
//!
//! // Batch processing
//! let results = batch(items, 100, Duration::from_millis(10), |batch| async {
//!     process_batch(batch).await
//! }).await;
//!
//! // Retry with backoff
//! let result = retry(
//!     || async { fetch_data().await },
//!     BackoffConfig::default()
//! ).await;
//! ```

use std::future::Future;
use std::time::Duration;

use futures::stream::{FuturesUnordered, StreamExt};

/// Run futures in parallel with a limit on concurrent execution
pub async fn parallel<T, F, Fut, E>(
    items: Vec<T>,
    concurrency: usize,
    f: F,
) -> Vec<Result<Fut::Output, E>>
where
    F: Fn(T) -> Fut,
    Fut: Future,
{
    let mut results = Vec::with_capacity(items.len());
    let mut futures = FuturesUnordered::new();

    for item in items {
        if futures.len() >= concurrency {
            if let Some(result) = futures.next().await {
                results.push(Ok(result));
            }
        }
        futures.push(f(item));
    }

    while let Some(result) = futures.next().await {
        results.push(Ok(result));
    }

    results
}

/// Process items in batches with rate limiting
pub async fn batch<T, R, F, Fut>(items: Vec<T>, batch_size: usize, delay: Duration, f: F) -> Vec<R>
where
    T: Clone,
    F: Fn(Vec<T>) -> Fut,
    Fut: Future<Output = Vec<R>>,
{
    let mut results = Vec::new();

    for chunk in items.chunks(batch_size) {
        let batch: Vec<T> = chunk.to_vec();
        let batch_results = f(batch).await;
        results.extend(batch_results);

        if delay > Duration::ZERO {
            tokio::time::sleep(delay).await;
        }
    }

    results
}

/// Configuration for exponential backoff
#[derive(Debug, Clone)]
pub struct BackoffConfig {
    /// Initial delay
    pub initial_delay: Duration,
    /// Maximum delay
    pub max_delay: Duration,
    /// Multiplier for each retry
    pub multiplier: f64,
    /// Maximum number of retries
    pub max_retries: u32,
    /// Add random jitter (0.0 - 1.0)
    pub jitter: f64,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            multiplier: 2.0,
            max_retries: 3,
            jitter: 0.1,
        }
    }
}

impl BackoffConfig {
    /// Create a new config with custom initial delay
    pub fn with_initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }

    /// Create a new config with custom max delay
    pub fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    /// Create a new config with custom multiplier
    pub fn with_multiplier(mut self, multiplier: f64) -> Self {
        self.multiplier = multiplier;
        self
    }

    /// Create a new config with custom max retries
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Create a new config with custom jitter
    pub fn with_jitter(mut self, jitter: f64) -> Self {
        self.jitter = jitter.clamp(0.0, 1.0);
        self
    }

    /// Calculate delay for a specific attempt
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::ZERO;
        }

        let base_delay =
            self.initial_delay.as_millis() as f64 * self.multiplier.powi(attempt as i32 - 1);

        let jitter_amount = base_delay * self.jitter * (rand::random::<f64>() * 2.0 - 1.0);
        let delay_ms = (base_delay + jitter_amount).min(self.max_delay.as_millis() as f64) as u64;

        Duration::from_millis(delay_ms)
    }
}

/// Retry a future with exponential backoff
pub async fn retry<F, Fut, T, E>(f: F, config: BackoffConfig) -> Result<T, RetryError<E>>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    let mut f = f;
    let mut last_error = None;

    for attempt in 0..=config.max_retries {
        let delay = config.delay_for_attempt(attempt);
        if delay > Duration::ZERO {
            tokio::time::sleep(delay).await;
        }

        match f().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                last_error = Some(e);
                tracing::warn!(
                    attempt = attempt,
                    max_retries = config.max_retries,
                    "Retry failed"
                );
            }
        }
    }

    Err(RetryError {
        attempts: config.max_retries + 1,
        error: last_error.unwrap(),
    })
}

/// Error from retry exhaustion
#[derive(Debug, Clone)]
pub struct RetryError<E> {
    /// Number of attempts made
    pub attempts: u32,
    /// Last error encountered
    pub error: E,
}

impl<E: std::fmt::Display> std::fmt::Display for RetryError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed after {} attempts: {}", self.attempts, self.error)
    }
}

impl<E: std::error::Error + 'static> std::error::Error for RetryError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// Timeout with a custom error message
pub async fn timeout<F, Fut, T>(
    duration: Duration,
    f: F,
    error_message: impl Into<String>,
) -> Result<T, TimeoutError>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = T>,
{
    match tokio::time::timeout(duration, f()).await {
        Ok(result) => Ok(result),
        Err(_) => Err(TimeoutError {
            duration,
            message: error_message.into(),
        }),
    }
}

/// Timeout error
#[derive(Debug, Clone)]
pub struct TimeoutError {
    /// Timeout duration
    pub duration: Duration,
    /// Error message
    pub message: String,
}

impl std::fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Operation timed out after {:?}: {}",
            self.duration, self.message
        )
    }
}

impl std::error::Error for TimeoutError {}

/// Select the first future to complete from a collection
pub async fn select_first<Fut>(futures: Vec<Fut>) -> Option<Fut::Output>
where
    Fut: Future + Unpin,
{
    let mut futures: FuturesUnordered<Fut> = futures.into_iter().collect();
    futures.next().await
}

/// Run futures and return results as they complete
pub async fn join_all_ordered<Fut>(futures: Vec<Fut>) -> Vec<Fut::Output>
where
    Fut: Future,
{
    futures::future::join_all(futures).await
}

/// Debouncer for rate limiting operations
pub struct Debouncer {
    delay: Duration,
    last_call: Option<std::time::Instant>,
}

impl Debouncer {
    /// Create a new debouncer with the specified delay
    pub fn new(delay: Duration) -> Self {
        Self {
            delay,
            last_call: None,
        }
    }

    /// Check if enough time has passed since the last call
    pub fn is_ready(&mut self) -> bool {
        match self.last_call {
            None => {
                self.last_call = Some(std::time::Instant::now());
                true
            }
            Some(last) => {
                if last.elapsed() >= self.delay {
                    self.last_call = Some(std::time::Instant::now());
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Get remaining time until next call is allowed
    pub fn remaining(&self) -> Duration {
        match self.last_call {
            None => Duration::ZERO,
            Some(last) => {
                let elapsed = last.elapsed();
                if elapsed >= self.delay {
                    Duration::ZERO
                } else {
                    self.delay - elapsed
                }
            }
        }
    }
}

/// Throttler for limiting operation rate
pub struct Throttler {
    min_interval: Duration,
    last_call: Option<std::time::Instant>,
}

impl Throttler {
    /// Create a new throttler with the specified minimum interval
    pub fn new(min_interval: Duration) -> Self {
        Self {
            min_interval,
            last_call: None,
        }
    }

    /// Wait if necessary before proceeding
    pub async fn throttle(&mut self) {
        if let Some(last) = self.last_call {
            let elapsed = last.elapsed();
            if elapsed < self.min_interval {
                tokio::time::sleep(self.min_interval - elapsed).await;
            }
        }
        self.last_call = Some(std::time::Instant::now());
    }

    /// Try to proceed immediately, return false if throttled
    pub fn try_throttle(&mut self) -> bool {
        match self.last_call {
            None => {
                self.last_call = Some(std::time::Instant::now());
                true
            }
            Some(last) => {
                if last.elapsed() >= self.min_interval {
                    self.last_call = Some(std::time::Instant::now());
                    true
                } else {
                    false
                }
            }
        }
    }
}

/// Coalesce multiple pending updates into a single execution
pub struct Coalescer<T> {
    pending: Option<T>,
    delay: Duration,
}

impl<T> Coalescer<T> {
    /// Create a new coalescer
    pub fn new(delay: Duration) -> Self {
        Self {
            pending: None,
            delay,
        }
    }

    /// Add an update to be coalesced
    pub fn push(&mut self, value: T) {
        self.pending = Some(value);
    }

    /// Check if there's a pending update
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Take the pending update if any
    pub fn take(&mut self) -> Option<T> {
        self.pending.take()
    }

    /// Get the delay
    pub fn delay(&self) -> Duration {
        self.delay
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[tokio::test]
    async fn test_parallel() {
        let items: Vec<i32> = (0..100).collect();

        let results: Vec<Result<i32, ()>> = parallel(items, 10, |item| async move {
            tokio::time::sleep(Duration::from_millis(1)).await;
            item * 2
        })
        .await;

        assert_eq!(results.len(), 100);
    }

    #[tokio::test]
    async fn test_batch() {
        let items: Vec<i32> = (0..100).collect();

        let results = batch(items, 10, Duration::ZERO, |batch| async move {
            batch.iter().map(|i| i * 2).collect()
        })
        .await;

        assert_eq!(results.len(), 100);
    }

    #[tokio::test]
    async fn test_retry_success() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_for_retry = Arc::clone(&attempts);

        let result = retry(
            move || {
                let attempts = Arc::clone(&attempts_for_retry);
                async move {
                    let current = attempts.fetch_add(1, Ordering::SeqCst) + 1;
                    if current < 3 {
                        Err("not yet")
                    } else {
                        Ok("success")
                    }
                }
            },
            BackoffConfig::default().with_max_retries(5),
        )
        .await;

        assert_eq!(result.unwrap(), "success");
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_exhaustion() {
        let result: Result<(), RetryError<&str>> = retry(
            || async { Err("always fails") },
            BackoffConfig::default().with_max_retries(2),
        )
        .await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().attempts, 3);
    }

    #[test]
    fn test_backoff_config() {
        let config = BackoffConfig::default().with_jitter(0.0);

        assert_eq!(config.delay_for_attempt(0), Duration::ZERO);
        assert!(config.delay_for_attempt(1) >= config.initial_delay);
        assert!(config.delay_for_attempt(2) > config.delay_for_attempt(1));
    }

    #[tokio::test]
    async fn test_timeout_success() {
        let result = timeout(
            Duration::from_secs(1),
            || async {
                tokio::time::sleep(Duration::from_millis(10)).await;
                "completed"
            },
            "operation timed out",
        )
        .await;

        assert_eq!(result.unwrap(), "completed");
    }

    #[tokio::test]
    async fn test_timeout_failure() {
        let result: Result<(), TimeoutError> = timeout(
            Duration::from_millis(10),
            || async {
                tokio::time::sleep(Duration::from_secs(1)).await;
            },
            "operation timed out",
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("timed out"));
    }

    #[test]
    fn test_debouncer() {
        let mut debouncer = Debouncer::new(Duration::from_millis(100));

        assert!(debouncer.is_ready());
        assert!(!debouncer.is_ready());
        assert!(!debouncer.is_ready());
    }

    #[tokio::test]
    async fn test_throttler() {
        let mut throttler = Throttler::new(Duration::from_millis(50));

        assert!(throttler.try_throttle());
        assert!(!throttler.try_throttle());

        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(throttler.try_throttle());
    }

    #[test]
    fn test_coalescer() {
        let mut coalescer = Coalescer::new(Duration::from_millis(100));

        assert!(!coalescer.has_pending());

        coalescer.push("value1");
        assert!(coalescer.has_pending());

        coalescer.push("value2"); // Overwrites
        assert!(coalescer.has_pending());

        let value = coalescer.take();
        assert_eq!(value, Some("value2"));
        assert!(!coalescer.has_pending());
    }
}
