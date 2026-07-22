use std::sync::Arc;
/// Circuit Breaker Pattern Implementation
///
/// Prevents cascading failures by failing fast while an upstream dependency is unavailable.
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

/// Circuit breaker configuration.
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before opening the circuit.
    pub failure_threshold: u32,
    /// Number of successful half-open probes required before closing the circuit.
    pub success_threshold: u32,
    /// Time to wait before transitioning from open to half-open.
    pub timeout: Duration,
    /// Maximum concurrently executing half-open probes. `None` leaves the concurrency unbounded.
    pub half_open_max_requests: Option<u32>,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            timeout: Duration::from_secs(60),
            half_open_max_requests: Some(3),
        }
    }
}

/// Circuit breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

impl CircuitState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Closed => "closed",
            Self::Open => "open",
            Self::HalfOpen => "half_open",
        }
    }

    /// Numeric representation for metrics: closed=0, open=1, half-open=2.
    pub fn as_u8(&self) -> u8 {
        match self {
            Self::Closed => 0,
            Self::Open => 1,
            Self::HalfOpen => 2,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CircuitBreakerError<E = String> {
    #[error("Circuit breaker is open, requests blocked")]
    Open,
    #[error("Upstream error: {0}")]
    Upstream(E),
}

#[derive(Debug, Clone, Copy)]
enum CircuitAdmission {
    Closed,
    HalfOpen { epoch: u64 },
}

struct CircuitBreakerState {
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    last_failure_time: Option<Instant>,
    half_open_in_flight: u32,
    half_open_epoch: u64,
}

impl CircuitBreakerState {
    fn new() -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            last_failure_time: None,
            half_open_in_flight: 0,
            half_open_epoch: 0,
        }
    }

    fn invalidate_half_open_epoch(&mut self) {
        self.half_open_epoch = self.half_open_epoch.wrapping_add(1);
        self.half_open_in_flight = 0;
        self.success_count = 0;
    }
}

/// Concurrent circuit breaker with epoch-aware half-open probes.
///
/// Every admitted half-open request receives the current epoch. The breaker closes only after the
/// configured number of successes and after all probes from that epoch have completed. A failure
/// from the same epoch therefore always reopens the circuit, while a late result from an obsolete
/// epoch cannot corrupt a newer recovery attempt or a manual state transition.
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: Arc<RwLock<CircuitBreakerState>>,
    total_requests: AtomicU64,
    total_successes: AtomicU64,
    total_failures: AtomicU64,
    total_rejected: AtomicU64,
    state_transitions: AtomicU64,
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(CircuitBreakerState::new())),
            total_requests: AtomicU64::new(0),
            total_successes: AtomicU64::new(0),
            total_failures: AtomicU64::new(0),
            total_rejected: AtomicU64::new(0),
            state_transitions: AtomicU64::new(0),
        }
    }

    pub async fn call<F, Fut, T, E>(&self, operation: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        self.total_requests.fetch_add(1, Ordering::Relaxed);

        let Some(admission) = self.admit().await else {
            self.total_rejected.fetch_add(1, Ordering::Relaxed);
            let state = self.get_state().await;
            tracing::warn!(state = state.as_str(), "Circuit breaker rejected request");
            return Err(CircuitBreakerError::Open);
        };

        let started_at = Instant::now();
        match operation().await {
            Ok(value) => {
                self.record_success(admission).await;
                self.total_successes.fetch_add(1, Ordering::Relaxed);
                let state = self.get_state().await;
                tracing::debug!(
                    duration_ms = started_at.elapsed().as_millis(),
                    state = state.as_str(),
                    "Circuit breaker: success"
                );
                Ok(value)
            }
            Err(error) => {
                self.record_failure(admission).await;
                self.total_failures.fetch_add(1, Ordering::Relaxed);
                let state = self.get_state().await;
                tracing::warn!(
                    duration_ms = started_at.elapsed().as_millis(),
                    state = state.as_str(),
                    error = %error,
                    "Circuit breaker: failure"
                );
                Err(CircuitBreakerError::Upstream(error))
            }
        }
    }

    async fn admit(&self) -> Option<CircuitAdmission> {
        let mut state = self.state.write().await;
        match state.state {
            CircuitState::Closed => Some(CircuitAdmission::Closed),
            CircuitState::Open => {
                let timeout_elapsed = state
                    .last_failure_time
                    .is_some_and(|failed_at| failed_at.elapsed() >= self.config.timeout);
                if !timeout_elapsed {
                    return None;
                }

                state.state = CircuitState::HalfOpen;
                state.failure_count = 0;
                state.success_count = 0;
                state.half_open_in_flight = 1;
                state.half_open_epoch = state.half_open_epoch.wrapping_add(1);
                let epoch = state.half_open_epoch;
                self.state_transitions.fetch_add(1, Ordering::Relaxed);
                tracing::info!(epoch, "Circuit breaker: Open -> HalfOpen");
                Some(CircuitAdmission::HalfOpen { epoch })
            }
            CircuitState::HalfOpen => {
                if self
                    .config
                    .half_open_max_requests
                    .is_some_and(|maximum| state.half_open_in_flight >= maximum.max(1))
                {
                    return None;
                }
                let next = state.half_open_in_flight.checked_add(1)?;
                state.half_open_in_flight = next;
                Some(CircuitAdmission::HalfOpen {
                    epoch: state.half_open_epoch,
                })
            }
        }
    }

    async fn record_success(&self, admission: CircuitAdmission) {
        let mut state = self.state.write().await;
        match admission {
            CircuitAdmission::Closed => {
                if state.state == CircuitState::Closed {
                    state.failure_count = 0;
                }
            }
            CircuitAdmission::HalfOpen { epoch } => {
                if state.state != CircuitState::HalfOpen || state.half_open_epoch != epoch {
                    return;
                }

                state.half_open_in_flight = state.half_open_in_flight.saturating_sub(1);
                state.success_count = state.success_count.saturating_add(1);
                let required_successes = self.config.success_threshold.max(1);
                if state.success_count >= required_successes && state.half_open_in_flight == 0 {
                    state.state = CircuitState::Closed;
                    state.failure_count = 0;
                    state.last_failure_time = None;
                    state.invalidate_half_open_epoch();
                    self.state_transitions.fetch_add(1, Ordering::Relaxed);
                    tracing::info!(epoch, "Circuit breaker: HalfOpen -> Closed");
                }
            }
        }
    }

    async fn record_failure(&self, admission: CircuitAdmission) {
        let mut state = self.state.write().await;
        match admission {
            CircuitAdmission::Closed => {
                if state.state != CircuitState::Closed {
                    return;
                }

                state.failure_count = state.failure_count.saturating_add(1);
                if state.failure_count >= self.config.failure_threshold.max(1) {
                    let failure_count = state.failure_count;
                    Self::set_open(&mut state);
                    state.failure_count = failure_count;
                    self.state_transitions.fetch_add(1, Ordering::Relaxed);
                    tracing::error!(failure_count, "Circuit breaker: Closed -> Open");
                }
            }
            CircuitAdmission::HalfOpen { epoch } => {
                if state.state != CircuitState::HalfOpen || state.half_open_epoch != epoch {
                    return;
                }

                Self::set_open(&mut state);
                self.state_transitions.fetch_add(1, Ordering::Relaxed);
                tracing::warn!(epoch, "Circuit breaker: HalfOpen -> Open");
            }
        }
    }

    fn set_open(state: &mut CircuitBreakerState) {
        state.state = CircuitState::Open;
        state.last_failure_time = Some(Instant::now());
        state.failure_count = 0;
        state.invalidate_half_open_epoch();
    }

    pub async fn get_state(&self) -> CircuitState {
        self.state.read().await.state
    }

    pub async fn stats(&self) -> CircuitBreakerStats {
        let state = self.state.read().await;
        CircuitBreakerStats {
            state: state.state,
            total_requests: self.total_requests.load(Ordering::Relaxed),
            total_successes: self.total_successes.load(Ordering::Relaxed),
            total_failures: self.total_failures.load(Ordering::Relaxed),
            total_rejected: self.total_rejected.load(Ordering::Relaxed),
            state_transitions: self.state_transitions.load(Ordering::Relaxed),
            failure_count: state.failure_count,
            success_count: state.success_count,
        }
    }

    pub async fn export_prometheus_metrics(&self, name: &str) -> String {
        let stats = self.stats().await;
        format!(
            r#"# HELP circuit_breaker_state Current state of the circuit breaker (0=closed, 1=open, 2=half_open)
# TYPE circuit_breaker_state gauge
circuit_breaker_state{{name="{name}"}} {state}
# HELP circuit_breaker_requests_total Total number of requests
# TYPE circuit_breaker_requests_total counter
circuit_breaker_requests_total{{name="{name}"}} {requests}
# HELP circuit_breaker_successes_total Total number of successful requests
# TYPE circuit_breaker_successes_total counter
circuit_breaker_successes_total{{name="{name}"}} {successes}
# HELP circuit_breaker_failures_total Total number of failed requests
# TYPE circuit_breaker_failures_total counter
circuit_breaker_failures_total{{name="{name}"}} {failures}
# HELP circuit_breaker_rejected_total Total number of rejected requests (circuit open)
# TYPE circuit_breaker_rejected_total counter
circuit_breaker_rejected_total{{name="{name}"}} {rejected}
# HELP circuit_breaker_state_transitions_total Total number of state transitions
# TYPE circuit_breaker_state_transitions_total counter
circuit_breaker_state_transitions_total{{name="{name}"}} {transitions}
# HELP circuit_breaker_success_rate Current success rate (0.0 - 1.0)
# TYPE circuit_breaker_success_rate gauge
circuit_breaker_success_rate{{name="{name}"}} {success_rate}
# HELP circuit_breaker_rejection_rate Current rejection rate (0.0 - 1.0)
# TYPE circuit_breaker_rejection_rate gauge
circuit_breaker_rejection_rate{{name="{name}"}} {rejection_rate}
"#,
            name = name,
            state = stats.state.as_u8(),
            requests = stats.total_requests,
            successes = stats.total_successes,
            failures = stats.total_failures,
            rejected = stats.total_rejected,
            transitions = stats.state_transitions,
            success_rate = stats.success_rate(),
            rejection_rate = stats.rejection_rate(),
        )
    }

    pub async fn open(&self) {
        let mut state = self.state.write().await;
        if state.state != CircuitState::Open {
            Self::set_open(&mut state);
            self.state_transitions.fetch_add(1, Ordering::Relaxed);
            tracing::warn!("Circuit breaker: Manually opened");
        }
    }

    pub async fn close(&self) {
        let mut state = self.state.write().await;
        if state.state != CircuitState::Closed {
            state.state = CircuitState::Closed;
            state.failure_count = 0;
            state.last_failure_time = None;
            state.invalidate_half_open_epoch();
            self.state_transitions.fetch_add(1, Ordering::Relaxed);
            tracing::info!("Circuit breaker: Manually closed");
        }
    }

    pub async fn reset(&self) {
        let mut state = self.state.write().await;
        state.state = CircuitState::Closed;
        state.failure_count = 0;
        state.last_failure_time = None;
        state.invalidate_half_open_epoch();

        self.total_requests.store(0, Ordering::Relaxed);
        self.total_successes.store(0, Ordering::Relaxed);
        self.total_failures.store(0, Ordering::Relaxed);
        self.total_rejected.store(0, Ordering::Relaxed);
        self.state_transitions.store(0, Ordering::Relaxed);
        tracing::info!("Circuit breaker: Reset");
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CircuitBreakerStats {
    pub state: CircuitState,
    pub total_requests: u64,
    pub total_successes: u64,
    pub total_failures: u64,
    pub total_rejected: u64,
    pub state_transitions: u64,
    pub failure_count: u32,
    pub success_count: u32,
}

impl CircuitBreakerStats {
    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            1.0
        } else {
            self.total_successes as f64 / self.total_requests as f64
        }
    }

    pub fn rejection_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.total_rejected as f64 / self.total_requests as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::Notify;

    async fn open_breaker(breaker: &CircuitBreaker) {
        let result = breaker.call(|| async { Err::<(), _>("failure") }).await;
        assert!(matches!(result, Err(CircuitBreakerError::Upstream(_))));
        assert_eq!(breaker.get_state().await, CircuitState::Open);
    }

    #[tokio::test]
    async fn closed_state_records_success_and_failures() {
        let breaker = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 3,
            ..CircuitBreakerConfig::default()
        });

        assert_eq!(
            breaker
                .call(|| async { Ok::<_, String>(42) })
                .await
                .unwrap(),
            42
        );
        for _ in 0..2 {
            let _ = breaker.call(|| async { Err::<(), _>("failure") }).await;
            assert_eq!(breaker.get_state().await, CircuitState::Closed);
        }
        let _ = breaker.call(|| async { Err::<(), _>("failure") }).await;
        assert_eq!(breaker.get_state().await, CircuitState::Open);
        assert_eq!(breaker.stats().await.failure_count, 3);
    }

    #[tokio::test]
    async fn open_state_rejects_before_timeout() {
        let breaker = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 1,
            timeout: Duration::from_secs(60),
            ..CircuitBreakerConfig::default()
        });
        open_breaker(&breaker).await;

        let result = breaker.call(|| async { Ok::<_, String>(42) }).await;
        assert!(matches!(result, Err(CircuitBreakerError::Open)));
        assert_eq!(breaker.stats().await.total_rejected, 1);
    }

    #[tokio::test]
    async fn sequential_half_open_successes_close_the_circuit() {
        let breaker = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 1,
            success_threshold: 2,
            timeout: Duration::ZERO,
            half_open_max_requests: Some(1),
        });
        open_breaker(&breaker).await;

        breaker.call(|| async { Ok::<_, String>(1) }).await.unwrap();
        assert_eq!(breaker.get_state().await, CircuitState::HalfOpen);
        breaker.call(|| async { Ok::<_, String>(2) }).await.unwrap();
        assert_eq!(breaker.get_state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn first_half_open_probe_counts_toward_the_concurrency_limit() {
        let breaker = Arc::new(CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 1,
            success_threshold: 2,
            timeout: Duration::ZERO,
            half_open_max_requests: Some(1),
        }));
        open_breaker(&breaker).await;

        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let probe_breaker = Arc::clone(&breaker);
        let probe_started = Arc::clone(&started);
        let probe_release = Arc::clone(&release);
        let probe = tokio::spawn(async move {
            probe_breaker
                .call(|| async move {
                    probe_started.notify_one();
                    probe_release.notified().await;
                    Ok::<_, String>(())
                })
                .await
        });
        started.notified().await;

        let rejected = breaker.call(|| async { Ok::<_, String>(()) }).await;
        assert!(matches!(rejected, Err(CircuitBreakerError::Open)));
        release.notify_one();
        probe.await.unwrap().unwrap();
        assert_eq!(breaker.get_state().await, CircuitState::HalfOpen);
    }

    #[tokio::test]
    async fn late_half_open_failure_reopens_after_an_early_success() {
        let breaker = Arc::new(CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 1,
            success_threshold: 1,
            timeout: Duration::ZERO,
            half_open_max_requests: Some(2),
        }));
        open_breaker(&breaker).await;

        let first_started = Arc::new(Notify::new());
        let first_release = Arc::new(Notify::new());
        let first_breaker = Arc::clone(&breaker);
        let first_started_task = Arc::clone(&first_started);
        let first_release_task = Arc::clone(&first_release);
        let first = tokio::spawn(async move {
            first_breaker
                .call(|| async move {
                    first_started_task.notify_one();
                    first_release_task.notified().await;
                    Ok::<_, String>(())
                })
                .await
        });
        first_started.notified().await;

        let second_started = Arc::new(Notify::new());
        let second_release = Arc::new(Notify::new());
        let second_breaker = Arc::clone(&breaker);
        let second_started_task = Arc::clone(&second_started);
        let second_release_task = Arc::clone(&second_release);
        let second = tokio::spawn(async move {
            second_breaker
                .call(|| async move {
                    second_started_task.notify_one();
                    second_release_task.notified().await;
                    Err::<(), _>("late failure")
                })
                .await
        });
        second_started.notified().await;

        first_release.notify_one();
        first.await.unwrap().unwrap();
        assert_eq!(breaker.get_state().await, CircuitState::HalfOpen);

        second_release.notify_one();
        assert!(matches!(
            second.await.unwrap(),
            Err(CircuitBreakerError::Upstream(_))
        ));
        assert_eq!(breaker.get_state().await, CircuitState::Open);
    }

    #[tokio::test]
    async fn obsolete_half_open_result_cannot_override_manual_close() {
        let breaker = Arc::new(CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 1,
            success_threshold: 1,
            timeout: Duration::ZERO,
            half_open_max_requests: Some(1),
        }));
        open_breaker(&breaker).await;

        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let task_breaker = Arc::clone(&breaker);
        let task_started = Arc::clone(&started);
        let task_release = Arc::clone(&release);
        let task = tokio::spawn(async move {
            task_breaker
                .call(|| async move {
                    task_started.notify_one();
                    task_release.notified().await;
                    Err::<(), _>("obsolete failure")
                })
                .await
        });
        started.notified().await;
        breaker.close().await;
        release.notify_one();
        assert!(task.await.unwrap().is_err());
        assert_eq!(breaker.get_state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn statistics_reset_and_prometheus_export_remain_stable() {
        let breaker = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 10,
            ..CircuitBreakerConfig::default()
        });
        let _ = breaker.call(|| async { Ok::<_, String>(1) }).await;
        let _ = breaker.call(|| async { Ok::<_, String>(2) }).await;
        let _ = breaker.call(|| async { Err::<(), _>("failure") }).await;

        let stats = breaker.stats().await;
        assert_eq!(stats.total_requests, 3);
        assert_eq!(stats.total_successes, 2);
        assert_eq!(stats.total_failures, 1);
        assert_eq!(stats.success_rate(), 2.0 / 3.0);

        let metrics = breaker.export_prometheus_metrics("test_circuit").await;
        assert!(metrics.contains("circuit_breaker_state{name=\"test_circuit\"}"));
        assert!(metrics.contains("circuit_breaker_requests_total{name=\"test_circuit\"} 3"));

        breaker.reset().await;
        let reset = breaker.stats().await;
        assert_eq!(reset.total_requests, 0);
        assert_eq!(reset.state, CircuitState::Closed);
    }

    #[test]
    fn circuit_state_numeric_values_are_stable() {
        assert_eq!(CircuitState::Closed.as_u8(), 0);
        assert_eq!(CircuitState::Open.as_u8(), 1);
        assert_eq!(CircuitState::HalfOpen.as_u8(), 2);
    }
}
