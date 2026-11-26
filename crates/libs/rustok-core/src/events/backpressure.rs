//! Backpressure and rate limiting for event processing
//!
//! This module provides mechanisms to prevent Out-Of-Memory (OOM) errors
//! from event bursts by implementing queue depth monitoring and backpressure.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use thiserror::Error;

/// Errors that can occur during backpressure control
#[derive(Debug, Error, Clone, PartialEq)]
pub enum BackpressureError {
    /// Queue is at critical capacity, rejecting new events
    #[error("Event queue is at critical capacity ({current}/{max}), rejecting events")]
    QueueFull { current: usize, max: usize },

    /// Queue depth exceeds warning threshold
    #[error("Event queue depth ({current}/{max}) exceeds warning threshold")]
    WarningThreshold { current: usize, max: usize },
}

/// Backpressure state indicating current system load
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackpressureState {
    /// Normal operation, accepting all events
    Normal,
    /// Queue depth approaching limit, may start slowing down
    Warning,
    /// Queue at critical capacity, rejecting new events
    Critical,
}

impl BackpressureState {
    /// Returns true if the state is Critical
    pub fn is_critical(&self) -> bool {
        matches!(self, BackpressureState::Critical)
    }

    /// Returns true if the state is Warning or Critical
    pub fn is_degraded(&self) -> bool {
        matches!(
            self,
            BackpressureState::Warning | BackpressureState::Critical
        )
    }
}

/// Configuration for backpressure control
#[derive(Debug, Clone)]
pub struct BackpressureConfig {
    /// Maximum queue depth before rejecting events
    pub max_queue_depth: usize,
    /// Warning threshold (percentage of max_queue_depth)
    /// Default: 0.7 (70%)
    pub warning_threshold: f64,
    /// Critical threshold (percentage of max_queue_depth)
    /// Default: 0.9 (90%)
    pub critical_threshold: f64,
}

impl Default for BackpressureConfig {
    fn default() -> Self {
        Self {
            max_queue_depth: 10_000,
            warning_threshold: 0.7,
            critical_threshold: 0.9,
        }
    }
}

fn threshold_depth(max_queue_depth: usize, threshold: f64) -> usize {
    if max_queue_depth == 0 {
        return 0;
    }
    ((max_queue_depth as f64 * threshold).ceil() as usize).clamp(1, max_queue_depth)
}

impl BackpressureConfig {
    /// Creates a new configuration with custom values
    pub fn new(max_queue_depth: usize, warning_threshold: f64, critical_threshold: f64) -> Self {
        assert!(
            warning_threshold < critical_threshold,
            "warning_threshold must be less than critical_threshold"
        );
        assert!(
            critical_threshold <= 1.0,
            "critical_threshold must be <= 1.0"
        );
        Self {
            max_queue_depth,
            warning_threshold,
            critical_threshold,
        }
    }

    /// Calculate warning threshold in absolute terms
    pub fn warning_depth(&self) -> usize {
        threshold_depth(self.max_queue_depth, self.warning_threshold)
    }

    /// Calculate critical threshold in absolute terms
    pub fn critical_depth(&self) -> usize {
        threshold_depth(self.max_queue_depth, self.critical_threshold)
    }
}

/// Metrics for backpressure monitoring
#[derive(Debug, Clone, Copy)]
pub struct BackpressureMetrics {
    /// Current queue depth
    pub current_depth: usize,
    /// Maximum queue depth configured
    pub max_depth: usize,
    /// Current backpressure state
    pub state: BackpressureState,
    /// Total events accepted
    pub events_accepted: u64,
    /// Total events rejected due to backpressure
    pub events_rejected: u64,
    /// Number of times warning threshold was exceeded
    pub warning_count: u64,
    /// Number of times critical threshold was exceeded
    pub critical_count: u64,
}

/// Controller for managing backpressure based on queue depth
#[derive(Clone)]
pub struct BackpressureController {
    config: BackpressureConfig,
    current_depth: Arc<AtomicUsize>,
    events_accepted: Arc<AtomicU64>,
    events_rejected: Arc<AtomicU64>,
    warning_count: Arc<AtomicU64>,
    critical_count: Arc<AtomicU64>,
    ownership_state: Arc<AtomicU64>,
    next_ownership_id: Arc<AtomicU64>,
}

/// Ownership state encoding: `(level << 32) | unique owner id`. The unique id
/// prevents an ABA relapse where a preempted guard becomes "current" again
/// after the preempting owner drops and a new owner of the same level claims.
const OWNERSHIP_FREE: u64 = 0;
const OWNERSHIP_FALLBACK: u64 = 1;
const OWNERSHIP_PRIMARY: u64 = 2;

fn encode_ownership(level: u64, owner_id: u64) -> u64 {
    (level << 32) | owner_id
}

fn ownership_level(state: u64) -> u64 {
    state >> 32
}

/// Exclusive right to release slots acquired by publishers on one bus.
///
/// A broadcast publish acquires exactly one slot, but every subscriber
/// receives a copy of the envelope. If each consumer released a slot for the
/// copy it saw, one acquire would be released N times and the depth would be
/// systematically under-counted, silently disabling backpressure. Exactly one
/// consumer therefore owns releases for a bus:
///
/// - the durable gate (server event forwarder) claims *primary* ownership and
///   releases a slot only after the event is durably persisted;
/// - an in-memory dispatcher claims *fallback* ownership when no durable gate
///   exists, and is preempted the moment a primary owner claims the bus.
///
/// Dropping the guard frees the claim so a restarted consumer can take over.
pub struct BackpressureOwnership {
    controller: BackpressureController,
    state: Arc<AtomicU64>,
    encoded: u64,
}

impl BackpressureOwnership {
    /// Whether this guard still holds the claim (a primary claim preempts a
    /// fallback guard, which must then stop releasing).
    pub fn is_current(&self) -> bool {
        self.state.load(Ordering::Acquire) == self.encoded
    }

    /// Releases one slot if this guard still owns the bus.
    pub fn release(&self) {
        if self.is_current() {
            self.controller.release();
        }
    }

    /// Releases `count` slots if this guard still owns the bus.
    pub fn release_slots(&self, count: usize) {
        if self.is_current() {
            for _ in 0..count {
                self.controller.release();
            }
        }
    }
}

impl Drop for BackpressureOwnership {
    fn drop(&mut self) {
        // Free the claim only if it is still ours; a preempting primary owner
        // must not lose its claim when the stale fallback guard drops.
        let _released_or_preempted = self.state.compare_exchange(
            self.encoded,
            OWNERSHIP_FREE,
            Ordering::AcqRel,
            Ordering::Relaxed,
        );
    }
}

impl std::fmt::Debug for BackpressureOwnership {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackpressureOwnership")
            .field("level", &ownership_level(self.encoded))
            .field("is_current", &self.is_current())
            .finish()
    }
}

impl std::fmt::Debug for BackpressureController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackpressureController")
            .field("config", &self.config)
            .field("current_depth", &self.current_depth.load(Ordering::Relaxed))
            .field(
                "events_accepted",
                &self.events_accepted.load(Ordering::Relaxed),
            )
            .field(
                "events_rejected",
                &self.events_rejected.load(Ordering::Relaxed),
            )
            .field("warning_count", &self.warning_count.load(Ordering::Relaxed))
            .field(
                "critical_count",
                &self.critical_count.load(Ordering::Relaxed),
            )
            .finish()
    }
}

impl Default for BackpressureController {
    fn default() -> Self {
        Self::new(BackpressureConfig::default())
    }
}

impl BackpressureController {
    /// Creates a new backpressure controller with the given configuration
    pub fn new(config: BackpressureConfig) -> Self {
        Self {
            config,
            current_depth: Arc::new(AtomicUsize::new(0)),
            events_accepted: Arc::new(AtomicU64::new(0)),
            events_rejected: Arc::new(AtomicU64::new(0)),
            warning_count: Arc::new(AtomicU64::new(0)),
            critical_count: Arc::new(AtomicU64::new(0)),
            ownership_state: Arc::new(AtomicU64::new(OWNERSHIP_FREE)),
            next_ownership_id: Arc::new(AtomicU64::new(1)),
        }
    }

    fn next_ownership(&self, level: u64) -> BackpressureOwnership {
        let owner_id = self.next_ownership_id.fetch_add(1, Ordering::Relaxed);
        BackpressureOwnership {
            controller: self.clone(),
            state: Arc::clone(&self.ownership_state),
            encoded: encode_ownership(level, owner_id),
        }
    }

    /// Claims fallback release ownership: succeeds only while no other owner
    /// (fallback or primary) holds the bus. Intended for in-memory consumers
    /// that are the sole drain of a bus without a durable gate.
    pub fn claim_ownership(&self) -> Option<BackpressureOwnership> {
        let ownership = self.next_ownership(OWNERSHIP_FALLBACK);
        self.ownership_state
            .compare_exchange(
                OWNERSHIP_FREE,
                ownership.encoded,
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .ok()
            .map(|_| ownership)
    }

    /// Claims primary release ownership, preempting a fallback owner. The
    /// durable gate of a bus uses this so release ownership does not depend on
    /// consumer startup order. Fails only when another primary owner exists.
    pub fn claim_primary_ownership(&self) -> Option<BackpressureOwnership> {
        let ownership = self.next_ownership(OWNERSHIP_PRIMARY);
        loop {
            let current = self.ownership_state.load(Ordering::Acquire);
            if ownership_level(current) == OWNERSHIP_PRIMARY {
                return None;
            }
            if self
                .ownership_state
                .compare_exchange(
                    current,
                    ownership.encoded,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                return Some(ownership);
            }
        }
    }

    /// Attempts to acquire permission to enqueue an event
    ///
    /// Returns Ok(()) if the event can be enqueued, or an error if rejected.
    pub fn try_acquire(&self) -> Result<(), BackpressureError> {
        let mut current = self.current_depth.load(Ordering::Relaxed);

        loop {
            let state = self.calculate_state(current);
            if state == BackpressureState::Critical {
                self.events_rejected.fetch_add(1, Ordering::Relaxed);
                self.critical_count.fetch_add(1, Ordering::Relaxed);
                tracing::warn!(
                    current_depth = current,
                    max_depth = self.config.max_queue_depth,
                    "Event queue at critical capacity, rejecting event"
                );
                return Err(BackpressureError::QueueFull {
                    current,
                    max: self.config.max_queue_depth,
                });
            }

            let next = current.saturating_add(1);
            match self.current_depth.compare_exchange_weak(
                current,
                next,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    self.events_accepted.fetch_add(1, Ordering::Relaxed);
                    if state == BackpressureState::Warning {
                        self.warning_count.fetch_add(1, Ordering::Relaxed);
                        tracing::debug!(
                            current_depth = next,
                            max_depth = self.config.max_queue_depth,
                            "Event queue approaching capacity (warning threshold)"
                        );
                    }
                    return Ok(());
                }
                Err(actual) => current = actual,
            }
        }
    }

    /// Releases one slot in the queue (called when event is processed)
    pub fn release(&self) {
        let mut current = self.current_depth.load(Ordering::Relaxed);

        loop {
            if current == 0 {
                tracing::warn!("BackpressureController: release() called with depth already at 0");
                return;
            }

            match self.current_depth.compare_exchange_weak(
                current,
                current - 1,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return,
                Err(actual) => current = actual,
            }
        }
    }

    /// Returns the current backpressure state
    pub fn state(&self) -> BackpressureState {
        let current = self.current_depth.load(Ordering::Relaxed);
        self.calculate_state(current)
    }

    /// Returns current metrics
    pub fn metrics(&self) -> BackpressureMetrics {
        let current_depth = self.current_depth.load(Ordering::Relaxed);
        BackpressureMetrics {
            current_depth,
            max_depth: self.config.max_queue_depth,
            state: self.calculate_state(current_depth),
            events_accepted: self.events_accepted.load(Ordering::Relaxed),
            events_rejected: self.events_rejected.load(Ordering::Relaxed),
            warning_count: self.warning_count.load(Ordering::Relaxed),
            critical_count: self.critical_count.load(Ordering::Relaxed),
        }
    }

    /// Returns current queue depth
    pub fn current_depth(&self) -> usize {
        self.current_depth.load(Ordering::Relaxed)
    }

    /// Returns maximum queue depth
    pub fn max_depth(&self) -> usize {
        self.config.max_queue_depth
    }

    /// Calculate backpressure state based on current queue depth
    fn calculate_state(&self, current_depth: usize) -> BackpressureState {
        let critical_depth = self.config.critical_depth();
        let warning_depth = self.config.warning_depth();

        if current_depth >= critical_depth {
            BackpressureState::Critical
        } else if current_depth >= warning_depth {
            BackpressureState::Warning
        } else {
            BackpressureState::Normal
        }
    }

    /// Resets metrics (useful for testing)
    #[cfg(test)]
    pub fn reset_metrics(&self) {
        self.events_accepted.store(0, Ordering::Relaxed);
        self.events_rejected.store(0, Ordering::Relaxed);
        self.warning_count.store(0, Ordering::Relaxed);
        self.critical_count.store(0, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ownership_is_exclusive_and_released_on_drop() {
        let controller = BackpressureController::new(BackpressureConfig::new(10, 0.7, 0.9));
        let owner = controller
            .claim_ownership()
            .expect("first fallback claim succeeds");
        assert!(controller.claim_ownership().is_none());
        drop(owner);
        assert!(
            controller.claim_ownership().is_some(),
            "dropping the guard must free the claim for a restarted consumer"
        );
    }

    #[test]
    fn primary_ownership_preempts_fallback_releases() {
        let controller = Arc::new(BackpressureController::new(BackpressureConfig::new(
            10, 0.7, 0.9,
        )));
        let fallback = controller.claim_ownership().expect("fallback claim");
        controller.try_acquire().unwrap();
        controller.try_acquire().unwrap();

        let primary = controller
            .claim_primary_ownership()
            .expect("primary claim preempts the fallback owner");
        assert!(!fallback.is_current());
        assert!(controller.claim_primary_ownership().is_none());

        fallback.release();
        assert_eq!(
            controller.current_depth(),
            2,
            "a preempted fallback owner must not release foreign slots"
        );

        primary.release_slots(2);
        assert_eq!(controller.current_depth(), 0);

        drop(primary);
        drop(fallback);
        assert!(
            controller.claim_ownership().is_some(),
            "dropping the stale fallback guard must not steal a freed claim"
        );
    }

    #[test]
    fn preempted_fallback_guard_stays_stale_after_reclaim() {
        let controller = Arc::new(BackpressureController::new(BackpressureConfig::new(
            10, 0.7, 0.9,
        )));
        let stale = controller.claim_ownership().expect("first fallback");
        let primary = controller
            .claim_primary_ownership()
            .expect("primary preempts");
        drop(primary);

        let successor = controller
            .claim_ownership()
            .expect("a new fallback owner may claim after the primary drops");
        assert!(!stale.is_current());
        assert!(successor.is_current());

        controller.try_acquire().unwrap();
        stale.release();
        assert_eq!(
            controller.current_depth(),
            1,
            "a stale guard must not release the successor's slots (ABA)"
        );
        successor.release();
        assert_eq!(controller.current_depth(), 0);
    }

    #[test]
    fn test_backpressure_config_defaults() {
        let config = BackpressureConfig::default();
        assert_eq!(config.max_queue_depth, 10_000);
        assert_eq!(config.warning_threshold, 0.7);
        assert_eq!(config.critical_threshold, 0.9);
        assert_eq!(config.warning_depth(), 7_000);
        assert_eq!(config.critical_depth(), 9_000);
    }

    #[test]
    fn test_backpressure_config_custom() {
        let config = BackpressureConfig::new(1000, 0.6, 0.8);
        assert_eq!(config.max_queue_depth, 1000);
        assert_eq!(config.warning_depth(), 600);
        assert_eq!(config.critical_depth(), 800);
    }

    #[test]
    #[should_panic(expected = "warning_threshold must be less than critical_threshold")]
    fn test_backpressure_config_invalid_thresholds() {
        BackpressureConfig::new(1000, 0.9, 0.7);
    }

    #[test]
    fn test_backpressure_normal_state() {
        let controller = BackpressureController::new(BackpressureConfig::new(100, 0.7, 0.9));

        // Should be in Normal state initially
        assert_eq!(controller.state(), BackpressureState::Normal);
        assert_eq!(controller.current_depth(), 0);
    }

    #[test]
    fn test_backpressure_acquire_and_release() {
        let controller = BackpressureController::new(BackpressureConfig::new(100, 0.7, 0.9));

        // Acquire should succeed in normal state
        assert!(controller.try_acquire().is_ok());
        assert_eq!(controller.current_depth(), 1);

        // Release should decrease depth
        controller.release();
        assert_eq!(controller.current_depth(), 0);
    }

    #[test]
    fn test_backpressure_warning_state() {
        let controller = BackpressureController::new(BackpressureConfig::new(100, 0.7, 0.9));

        // Fill to warning threshold (70 events)
        for _ in 0..70 {
            assert!(controller.try_acquire().is_ok());
        }

        assert_eq!(controller.state(), BackpressureState::Warning);
        assert_eq!(controller.current_depth(), 70);

        // Should still accept events in warning state
        assert!(controller.try_acquire().is_ok());
    }

    #[test]
    fn test_backpressure_critical_state() {
        let controller = BackpressureController::new(BackpressureConfig::new(100, 0.7, 0.9));

        // Fill to critical threshold (90 events)
        for _ in 0..90 {
            assert!(controller.try_acquire().is_ok());
        }

        assert_eq!(controller.state(), BackpressureState::Critical);

        // Should reject events in critical state
        let result = controller.try_acquire();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            BackpressureError::QueueFull { .. }
        ));
    }

    #[test]
    fn test_backpressure_metrics() {
        let controller = BackpressureController::new(BackpressureConfig::new(100, 0.7, 0.9));

        // Accept some events
        for _ in 0..50 {
            assert!(controller.try_acquire().is_ok());
        }

        let metrics = controller.metrics();
        assert_eq!(metrics.current_depth, 50);
        assert_eq!(metrics.max_depth, 100);
        assert_eq!(metrics.state, BackpressureState::Normal);
        assert_eq!(metrics.events_accepted, 50);
        assert_eq!(metrics.events_rejected, 0);
    }

    #[test]
    fn test_backpressure_rejection_metrics() {
        let controller = BackpressureController::new(BackpressureConfig::new(100, 0.7, 0.9));

        // Fill to critical
        for _ in 0..90 {
            assert!(controller.try_acquire().is_ok());
        }

        // Try to add more (should reject)
        for _ in 0..10 {
            assert!(controller.try_acquire().is_err());
        }

        let metrics = controller.metrics();
        assert_eq!(metrics.events_accepted, 90);
        assert_eq!(metrics.events_rejected, 10);
        assert!(metrics.critical_count >= 10);
    }

    #[test]
    fn test_backpressure_state_transitions() {
        let controller = BackpressureController::new(BackpressureConfig::new(100, 0.7, 0.9));

        // Normal state
        assert_eq!(controller.state(), BackpressureState::Normal);
        assert!(!controller.state().is_degraded());
        assert!(!controller.state().is_critical());

        // Transition to Warning
        for _ in 0..70 {
            assert!(controller.try_acquire().is_ok());
        }
        assert_eq!(controller.state(), BackpressureState::Warning);
        assert!(controller.state().is_degraded());
        assert!(!controller.state().is_critical());

        // Transition to Critical
        for _ in 0..20 {
            assert!(controller.try_acquire().is_ok());
        }
        assert_eq!(controller.state(), BackpressureState::Critical);
        assert!(controller.state().is_degraded());
        assert!(controller.state().is_critical());

        // Transition back to Warning
        for _ in 0..5 {
            controller.release();
        }
        assert_eq!(controller.state(), BackpressureState::Warning);

        // Transition back to Normal
        for _ in 0..20 {
            controller.release();
        }
        assert_eq!(controller.state(), BackpressureState::Normal);
    }

    #[test]
    fn concurrent_acquire_never_exceeds_critical_capacity() {
        use std::sync::Arc;
        use std::thread;

        let controller = Arc::new(BackpressureController::new(BackpressureConfig::new(
            100, 0.7, 0.9,
        )));
        let mut handles = Vec::new();

        for _ in 0..10 {
            let controller = Arc::clone(&controller);
            handles.push(thread::spawn(move || {
                for _ in 0..10 {
                    drop(controller.try_acquire());
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let metrics = controller.metrics();
        assert_eq!(metrics.current_depth, 90);
        assert_eq!(metrics.events_accepted, 90);
        assert_eq!(metrics.events_rejected, 10);
    }

    #[test]
    fn test_backpressure_concurrent_operations() {
        use std::sync::Arc;
        use std::thread;

        let controller = Arc::new(BackpressureController::new(BackpressureConfig::new(
            1000, 0.7, 0.9,
        )));

        let mut handles = vec![];

        // Spawn multiple threads trying to acquire
        for _ in 0..10 {
            let ctrl = Arc::clone(&controller);
            handles.push(thread::spawn(move || {
                for _ in 0..50 {
                    assert!(ctrl.try_acquire().is_ok());
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let metrics = controller.metrics();
        // All 500 events should be accepted (below critical threshold)
        assert_eq!(metrics.events_accepted, 500);
        assert_eq!(metrics.current_depth, 500);
    }
}
