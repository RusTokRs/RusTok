/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 * You may not remove or alter this copyright notice or license header.
 */

//! Bounded clock observations for the managed JWT lease and credential admission,
//! not a distributed time service.
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MAX_DIVERGENCE: Duration = Duration::from_secs(1);
const MAX_DB_READ: Duration = Duration::from_secs(1);
const MAX_DB_SKEW_MS: i64 = 500;

/// Conservative uncertainty for second-resolution key retirement decisions.
pub(crate) const KEY_CLOCK_MARGIN_MS: i64 = 2_000;

#[derive(Clone, Copy)]
pub(crate) struct ClockSample {
    mono: Instant,
    wall: SystemTime,
}

impl ClockSample {
    pub(crate) fn now() -> Self {
        Self {
            mono: Instant::now(),
            wall: SystemTime::now(),
        }
    }

    pub(crate) fn checked_add(self, duration: Duration) -> Option<Self> {
        Some(Self {
            mono: self.mono.checked_add(duration)?,
            wall: self.wall.checked_add(duration)?,
        })
    }

    pub(crate) fn utc(self) -> Option<chrono::DateTime<chrono::Utc>> {
        let elapsed = self.wall.duration_since(UNIX_EPOCH).ok()?;
        chrono::DateTime::from_timestamp(
            i64::try_from(elapsed.as_secs()).ok()?,
            elapsed.subsec_nanos(),
        )
    }

    fn wall_ms(self) -> Option<i64> {
        i64::try_from(self.wall.duration_since(UNIX_EPOCH).ok()?.as_millis()).ok()
    }
}

/// Sticky for the lifetime of the owning runtime or database authority.
pub(crate) struct ClockGuard {
    anchor: ClockSample,
    faulted: AtomicBool,
    // Serialize sampling, not merely publication, to avoid treating an
    // out-of-order concurrent observation as a backward clock step.
    credential_high_water: Mutex<ClockSample>,
}

impl ClockGuard {
    pub(crate) fn new() -> Self {
        Self::at(ClockSample::now())
    }

    fn at(anchor: ClockSample) -> Self {
        Self {
            anchor,
            faulted: AtomicBool::new(false),
            credential_high_water: Mutex::new(anchor),
        }
    }

    pub(crate) fn faulted(&self) -> bool {
        self.faulted.load(Ordering::Acquire)
    }

    pub(crate) fn trip(&self) {
        if !self.faulted.swap(true, Ordering::AcqRel) {
            tracing::error!("Auth clock fault; restart only after clock remediation");
        }
    }

    pub(crate) fn healthy_at(&self, now: ClockSample) -> bool {
        if self.faulted() {
            return false;
        }
        let consistent = now
            .mono
            .checked_duration_since(self.anchor.mono)
            .zip(now.wall.duration_since(self.anchor.wall).ok())
            .is_some_and(|(mono, wall)| mono.abs_diff(wall) <= MAX_DIVERGENCE);
        if !consistent || now.wall_ms().is_none() {
            self.trip();
        }
        !self.faulted()
    }

    /// Credential admission must not reuse an earlier observed wall time,
    /// even within the lease's one-second divergence tolerance.
    pub(crate) fn credential_sample(
        &self,
        sample: impl FnOnce() -> ClockSample,
    ) -> Option<ClockSample> {
        let Ok(mut previous) = self.credential_high_water.lock() else {
            self.trip();
            return None;
        };
        let now = sample();
        if now.wall < previous.wall || now.mono < previous.mono {
            self.trip();
            return None;
        }
        if !self.healthy_at(now) {
            return None;
        }
        *previous = now;
        Some(now)
    }

    /// The timestamp was evaluated somewhere within the read interval.
    /// Wide or backwards intervals cannot authorize a time-sensitive write.
    pub(crate) fn validate_db(&self, before: ClockSample, after: ClockSample, db_ms: i64) -> bool {
        if !self.healthy_at(before) || !self.healthy_at(after) {
            return false;
        }
        let Some(elapsed) = after.mono.checked_duration_since(before.mono) else {
            self.trip();
            return false;
        };
        let Some(wall_elapsed) = after.wall.duration_since(before.wall).ok() else {
            self.trip();
            return false;
        };
        if elapsed.abs_diff(wall_elapsed) > MAX_DIVERGENCE {
            self.trip();
            return false;
        }
        if elapsed > MAX_DB_READ || wall_elapsed > MAX_DB_READ {
            return false;
        }
        let valid = before
            .wall_ms()
            .zip(after.wall_ms())
            .is_some_and(|(low, high)| {
                db_ms > 0
                    && db_ms >= low.saturating_sub(MAX_DB_SKEW_MS)
                    && db_ms <= high.saturating_add(MAX_DB_SKEW_MS)
            });
        if !valid {
            self.trip();
        }
        !self.faulted()
    }
}

/// Both deadlines start before DB IO. A grant carries the authority's fault
/// latch, so a later diagnostic observation can close the same request gate.
#[derive(Clone)]
pub(crate) struct LeaseDeadline {
    expires: ClockSample,
    pub(crate) authority: Arc<ClockGuard>,
    expired: Arc<AtomicBool>,
    witness_fault: Option<Arc<AtomicBool>>,
}

impl LeaseDeadline {
    pub(crate) fn new(
        start: ClockSample,
        duration: Duration,
        authority: Arc<ClockGuard>,
    ) -> Option<Self> {
        Some(Self {
            expires: start.checked_add(duration)?,
            authority,
            expired: Arc::new(AtomicBool::new(false)),
            witness_fault: None,
        })
    }

    pub(crate) fn with_witness_fault(mut self, fault: Arc<AtomicBool>) -> Self {
        self.witness_fault = Some(fault);
        self
    }

    fn witness_healthy(&self) -> bool {
        self.witness_fault
            .as_ref()
            .is_none_or(|fault| !fault.load(Ordering::Acquire))
    }

    pub(crate) fn remaining(&self, now: ClockSample) -> Duration {
        if !self.witness_healthy()
            || self.expired.load(Ordering::Acquire)
            || !self.authority.healthy_at(now)
        {
            return Duration::ZERO;
        }
        let mono = self.expires.mono.saturating_duration_since(now.mono);
        let wall = self
            .expires
            .wall
            .duration_since(now.wall)
            .unwrap_or(Duration::ZERO);
        let remaining = mono.min(wall);
        if remaining.is_zero() {
            self.expired.store(true, Ordering::Release);
        }
        if !self.witness_healthy()
            || self.expired.load(Ordering::Acquire)
            || self.authority.faulted()
        {
            Duration::ZERO
        } else {
            remaining
        }
    }

    pub(crate) fn later_than(&self, other: &Self) -> bool {
        self.expires.mono > other.expires.mono
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_clock_rejects_small_rollback_and_never_reopens() {
        let start = ClockSample::now();
        let guard = ClockGuard::at(start);
        let later = start.checked_add(Duration::from_millis(500)).unwrap();
        assert!(guard.credential_sample(|| later).is_some());
        let rollback = ClockSample {
            mono: start.mono + Duration::from_millis(600),
            wall: start.wall + Duration::from_millis(400),
        };
        assert!(
            guard.healthy_at(rollback),
            "lease tolerance alone admits this sample"
        );
        assert!(guard.credential_sample(|| rollback).is_none());
        assert!(guard.faulted());
        assert!(
            guard
                .credential_sample(|| start.checked_add(Duration::from_secs(2)).unwrap())
                .is_none()
        );
    }

    #[test]
    fn database_skew_is_bounded_and_faults_are_sticky() {
        let before = ClockSample::now();
        let after = before.checked_add(Duration::from_millis(100)).unwrap();
        let ms = before.wall_ms().unwrap();
        let guard = ClockGuard::at(before);
        assert!(guard.validate_db(before, after, ms - 500));
        assert!(guard.validate_db(before, after, ms + 600));
        assert!(!guard.validate_db(before, after, ms + 601));
        assert!(guard.faulted());
        assert!(!guard.validate_db(before, after, ms));
        let guard = ClockGuard::at(before);
        assert!(!guard.validate_db(before, after, ms - 501));
        assert!(guard.faulted());
    }

    #[test]
    fn slow_read_is_inconclusive_not_a_clock_jump() {
        let before = ClockSample::now();
        let guard = ClockGuard::at(before);
        let after = before.checked_add(Duration::from_secs(2)).unwrap();
        assert!(!guard.validate_db(before, after, before.wall_ms().unwrap()));
        assert!(!guard.faulted());
    }

    #[test]
    fn suspend_and_wall_rollback_fence_even_after_clock_normalizes() {
        let start = ClockSample::now();
        let suspended = ClockSample {
            mono: start.mono,
            wall: start.wall + Duration::from_secs(11),
        };
        let guard = Arc::new(ClockGuard::at(start));
        let lease = LeaseDeadline::new(start, Duration::from_secs(10), guard.clone()).unwrap();
        assert!(lease.remaining(start) > Duration::ZERO);
        assert_eq!(lease.remaining(suspended), Duration::ZERO);
        assert!(guard.faulted());
        assert_eq!(lease.remaining(start), Duration::ZERO);

        let guard = ClockGuard::at(start);
        let rollback = ClockSample {
            mono: start.mono + Duration::from_secs(3),
            wall: start.wall - Duration::from_secs(1),
        };
        assert!(!guard.healthy_at(rollback));
        assert!(!guard.healthy_at(start));
    }

    #[test]
    fn either_deadline_expires_even_inside_clock_tolerance() {
        let start = ClockSample::now();
        let guard = Arc::new(ClockGuard::at(start));
        let lease = LeaseDeadline::new(start, Duration::from_secs(10), guard.clone()).unwrap();
        let wall_first = ClockSample {
            mono: start.mono + Duration::from_millis(9_500),
            wall: start.wall + Duration::from_secs(10),
        };
        let mono_first = ClockSample {
            mono: start.mono + Duration::from_secs(10),
            wall: start.wall + Duration::from_millis(9_500),
        };
        assert_eq!(lease.remaining(wall_first), Duration::ZERO);
        let earlier_wall = ClockSample {
            mono: start.mono + Duration::from_millis(9_600),
            wall: start.wall + Duration::from_millis(9_600),
        };
        assert_eq!(lease.clone().remaining(earlier_wall), Duration::ZERO);
        let fresh_lease =
            LeaseDeadline::new(start, Duration::from_secs(10), guard.clone()).unwrap();
        assert_eq!(fresh_lease.remaining(mono_first), Duration::ZERO);
        assert!(!guard.faulted());
    }
}
