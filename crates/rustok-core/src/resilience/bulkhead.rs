/// Bulkhead Pattern Implementation
///
/// Isolates resources by limiting the number of concurrent calls to a downstream
/// service. Named after watertight compartments in ships — if one compartment
/// floods, the others remain intact.
///
/// How it works:
/// - A semaphore with `max_concurrent_calls` permits controls access
/// - Callers either acquire a permit immediately, wait up to `max_wait_duration`,
///   or are rejected when the bulkhead is full
///
/// Benefits:
/// - Prevents one slow service from starving all other callers
/// - Provides back-pressure to upstream callers
/// - Enables resource capacity planning per service boundary
///
/// Example:
/// ```ignore
/// let bulkhead = Bulkhead::new(BulkheadConfig {
///     max_concurrent_calls: 10,
///     max_wait_duration: Some(Duration::from_millis(500)),
/// });
///
/// match bulkhead.call(|| downstream_service.call()).await {
///     Ok(result)                    => // Handle success
///     Err(BulkheadError::Full)      => // Bulkhead saturated, reject request
///     Err(BulkheadError::Timeout)   => // Waited too long for a permit
///     Err(BulkheadError::Upstream(e)) => // Downstream returned an error
/// }
/// ```
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Bulkhead configuration
#[derive(Debug, Clone)]
pub struct BulkheadConfig {
    /// Maximum number of concurrent calls allowed through the bulkhead
    pub max_concurrent_calls: usize,

    /// How long to wait for a permit before returning `BulkheadError::Timeout`.
    /// `None` means fail immediately (no waiting) when all permits are taken.
    pub max_wait_duration: Option<Duration>,
}

impl Default for BulkheadConfig {
    fn default() -> Self {
        Self {
            max_concurrent_calls: 25,
            max_wait_duration: Some(Duration::from_millis(500)),
        }
    }
}

/// Bulkhead error
#[derive(Debug, thiserror::Error)]
pub enum BulkheadError<E = String> {
    /// Bulkhead is saturated and `max_wait_duration` is `None`
    #[error("Bulkhead is full, request rejected immediately")]
    Full,

    /// Waited for `max_wait_duration` but no permit became available
    #[error("Timed out waiting for a bulkhead permit")]
    Timeout,

    /// The wrapped operation itself returned an error
    #[error("Upstream error: {0}")]
    Upstream(E),
}

/// Bulkhead implementation
pub struct Bulkhead {
    semaphore: Arc<Semaphore>,
    config: BulkheadConfig,

    // Metrics (atomic for lock-free reads)
    total_requests: AtomicU64,
    total_successes: AtomicU64,
    total_failures: AtomicU64,
    total_rejected: AtomicU64,
}

impl Bulkhead {
    /// Create a new bulkhead
    pub fn new(config: BulkheadConfig) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_calls));
        Self {
            semaphore,
            config,
            total_requests: AtomicU64::new(0),
            total_successes: AtomicU64::new(0),
            total_failures: AtomicU64::new(0),
            total_rejected: AtomicU64::new(0),
        }
    }

    /// Execute a fallible async operation with bulkhead protection
    pub async fn call<F, Fut, T, E>(&self, f: F) -> Result<T, BulkheadError<E>>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        self.total_requests.fetch_add(1, Ordering::Relaxed);

        let _permit = self.acquire_permit().await.map_err(|e| match e {
            BulkheadError::Full => BulkheadError::Full,
            BulkheadError::Timeout => BulkheadError::Timeout,
            BulkheadError::Upstream(_) => unreachable!("acquire_permit never returns Upstream"),
        })?;

        let concurrent = self.current_concurrent();
        tracing::debug!(
            concurrent,
            max = self.config.max_concurrent_calls,
            "Bulkhead: permit acquired"
        );

        match f().await {
            Ok(value) => {
                self.total_successes.fetch_add(1, Ordering::Relaxed);
                Ok(value)
            }
            Err(err) => {
                self.total_failures.fetch_add(1, Ordering::Relaxed);
                tracing::warn!(error = %err, "Bulkhead: upstream error");
                Err(BulkheadError::Upstream(err))
            }
        }
    }

    /// Acquire a semaphore permit, respecting `max_wait_duration`
    async fn acquire_permit(&self) -> Result<OwnedSemaphorePermit, BulkheadError> {
        match self.config.max_wait_duration {
            None => {
                // Fail fast — no waiting
                match self.semaphore.clone().try_acquire_owned() {
                    Ok(permit) => Ok(permit),
                    Err(_) => {
                        self.total_rejected.fetch_add(1, Ordering::Relaxed);
                        tracing::warn!(
                            max = self.config.max_concurrent_calls,
                            "Bulkhead: full, request rejected"
                        );
                        Err(BulkheadError::Full)
                    }
                }
            }
            Some(wait) => {
                match tokio::time::timeout(wait, self.semaphore.clone().acquire_owned()).await {
                    Ok(Ok(permit)) => Ok(permit),
                    Ok(Err(_)) => {
                        // Semaphore closed (only happens if dropped)
                        self.total_rejected.fetch_add(1, Ordering::Relaxed);
                        Err(BulkheadError::Full)
                    }
                    Err(_elapsed) => {
                        self.total_rejected.fetch_add(1, Ordering::Relaxed);
                        tracing::warn!(
                            wait_ms = wait.as_millis(),
                            max = self.config.max_concurrent_calls,
                            "Bulkhead: timed out waiting for permit"
                        );
                        Err(BulkheadError::Timeout)
                    }
                }
            }
        }
    }

    /// Number of currently in-flight calls
    pub fn current_concurrent(&self) -> usize {
        self.config.max_concurrent_calls - self.semaphore.available_permits()
    }

    /// Snapshot of bulkhead statistics
    pub fn stats(&self) -> BulkheadStats {
        BulkheadStats {
            max_concurrent_calls: self.config.max_concurrent_calls,
            current_concurrent: self.current_concurrent(),
            total_requests: self.total_requests.load(Ordering::Relaxed),
            total_successes: self.total_successes.load(Ordering::Relaxed),
            total_failures: self.total_failures.load(Ordering::Relaxed),
            total_rejected: self.total_rejected.load(Ordering::Relaxed),
        }
    }

    /// Export metrics in Prometheus exposition format
    ///
    /// # Example
    /// ```ignore
    /// let metrics = bulkhead.export_prometheus_metrics("payments_service");
    /// println!("{}", metrics);
    /// ```
    pub fn export_prometheus_metrics(&self, name: &str) -> String {
        let stats = self.stats();
        format!(
            r#"# HELP bulkhead_concurrent_calls_current Current number of concurrent calls in flight
# TYPE bulkhead_concurrent_calls_current gauge
bulkhead_concurrent_calls_current{{name="{name}"}} {current}
# HELP bulkhead_concurrent_calls_max Maximum number of concurrent calls allowed
# TYPE bulkhead_concurrent_calls_max gauge
bulkhead_concurrent_calls_max{{name="{name}"}} {max}
# HELP bulkhead_requests_total Total number of call attempts
# TYPE bulkhead_requests_total counter
bulkhead_requests_total{{name="{name}"}} {requests}
# HELP bulkhead_successes_total Total number of successful calls
# TYPE bulkhead_successes_total counter
bulkhead_successes_total{{name="{name}"}} {successes}
# HELP bulkhead_failures_total Total number of upstream failures
# TYPE bulkhead_failures_total counter
bulkhead_failures_total{{name="{name}"}} {failures}
# HELP bulkhead_rejected_total Total number of rejected calls (bulkhead full or timed out)
# TYPE bulkhead_rejected_total counter
bulkhead_rejected_total{{name="{name}"}} {rejected}
# HELP bulkhead_utilization Current utilisation ratio (0.0 – 1.0)
# TYPE bulkhead_utilization gauge
bulkhead_utilization{{name="{name}"}} {utilization:.4}
"#,
            name = name,
            current = stats.current_concurrent,
            max = stats.max_concurrent_calls,
            requests = stats.total_requests,
            successes = stats.total_successes,
            failures = stats.total_failures,
            rejected = stats.total_rejected,
            utilization = stats.utilization(),
        )
    }
}

/// Bulkhead statistics snapshot
#[derive(Debug, Clone, Copy)]
pub struct BulkheadStats {
    pub max_concurrent_calls: usize,
    pub current_concurrent: usize,
    pub total_requests: u64,
    pub total_successes: u64,
    pub total_failures: u64,
    pub total_rejected: u64,
}

impl BulkheadStats {
    /// Ratio of currently used permits to maximum (0.0 – 1.0)
    pub fn utilization(&self) -> f64 {
        if self.max_concurrent_calls == 0 {
            return 1.0;
        }
        self.current_concurrent as f64 / self.max_concurrent_calls as f64
    }

    /// Rejection rate over all requests (0.0 – 1.0)
    pub fn rejection_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 0.0;
        }
        self.total_rejected as f64 / self.total_requests as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    #[tokio::test]
    async fn test_basic_call_succeeds() {
        let bulkhead = Bulkhead::new(BulkheadConfig::default());
        let result = bulkhead.call(|| async { Ok::<_, String>(42) }).await;
        assert_eq!(result.unwrap(), 42);

        let stats = bulkhead.stats();
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.total_successes, 1);
        assert_eq!(stats.total_failures, 0);
        assert_eq!(stats.total_rejected, 0);
    }

    #[tokio::test]
    async fn test_upstream_error_tracked() {
        let bulkhead = Bulkhead::new(BulkheadConfig::default());
        let result = bulkhead
            .call(|| async { Err::<i32, _>("oops") })
            .await;

        assert!(matches!(result, Err(BulkheadError::Upstream(_))));
        let stats = bulkhead.stats();
        assert_eq!(stats.total_failures, 1);
        assert_eq!(stats.total_rejected, 0);
    }

    #[tokio::test]
    async fn test_full_rejection_when_no_wait() {
        let bulkhead = Bulkhead::new(BulkheadConfig {
            max_concurrent_calls: 1,
            max_wait_duration: None,
        });

        // Hold the single permit by launching a long-running task
        let b = Arc::new(bulkhead);
        let b2 = Arc::clone(&b);

        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let handle = tokio::spawn(async move {
            b2.call(|| async move {
                rx.await.ok();
                Ok::<_, String>(())
            })
            .await
        });

        // Give the spawned task time to acquire the permit
        tokio::time::sleep(Duration::from_millis(20)).await;

        // This call should be rejected immediately
        let result = b.call(|| async { Ok::<_, String>(()) }).await;
        assert!(matches!(result, Err(BulkheadError::Full)));

        let stats = b.stats();
        assert_eq!(stats.total_rejected, 1);

        // Release the held permit
        tx.send(()).ok();
        handle.await.ok();
    }

    #[tokio::test]
    async fn test_timeout_when_wait_exceeded() {
        let bulkhead = Arc::new(Bulkhead::new(BulkheadConfig {
            max_concurrent_calls: 1,
            max_wait_duration: Some(Duration::from_millis(50)),
        }));

        let b2 = Arc::clone(&bulkhead);
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();

        let handle = tokio::spawn(async move {
            b2.call(|| async move {
                rx.await.ok();
                Ok::<_, String>(())
            })
            .await
        });

        tokio::time::sleep(Duration::from_millis(20)).await;

        // This will wait 50 ms then time out
        let result = bulkhead
            .call(|| async { Ok::<_, String>(()) })
            .await;
        assert!(matches!(result, Err(BulkheadError::Timeout)));

        tx.send(()).ok();
        handle.await.ok();
    }

    #[tokio::test]
    async fn test_concurrent_calls_tracked() {
        let counter = Arc::new(AtomicUsize::new(0));
        let bulkhead = Arc::new(Bulkhead::new(BulkheadConfig {
            max_concurrent_calls: 5,
            max_wait_duration: Some(Duration::from_millis(200)),
        }));

        let handles: Vec<_> = (0..5)
            .map(|_| {
                let b = Arc::clone(&bulkhead);
                let c = Arc::clone(&counter);
                tokio::spawn(async move {
                    b.call(|| async move {
                        c.fetch_add(1, AtomicOrdering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(30)).await;
                        Ok::<_, String>(())
                    })
                    .await
                })
            })
            .collect();

        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(bulkhead.current_concurrent(), 5);

        for h in handles {
            h.await.ok();
        }

        assert_eq!(counter.load(AtomicOrdering::SeqCst), 5);
        assert_eq!(bulkhead.current_concurrent(), 0);
    }

    #[tokio::test]
    async fn test_utilization() {
        let bulkhead = Bulkhead::new(BulkheadConfig {
            max_concurrent_calls: 10,
            max_wait_duration: None,
        });
        let stats = bulkhead.stats();
        assert_eq!(stats.utilization(), 0.0);
        assert_eq!(stats.rejection_rate(), 0.0);
    }

    #[test]
    fn test_prometheus_metrics_format() {
        let bulkhead = Bulkhead::new(BulkheadConfig::default());
        let metrics = bulkhead.export_prometheus_metrics("order_service");

        assert!(metrics.contains("bulkhead_concurrent_calls_current{name=\"order_service\"}"));
        assert!(metrics.contains("bulkhead_requests_total{name=\"order_service\"}"));
        assert!(metrics.contains("bulkhead_rejected_total{name=\"order_service\"}"));
        assert!(metrics.contains("# HELP bulkhead_utilization"));
    }
}
