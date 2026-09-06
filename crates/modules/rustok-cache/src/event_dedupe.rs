use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::{Mutex as AsyncMutex, MutexGuard as AsyncMutexGuard};
use uuid::Uuid;

pub const DEFAULT_MAX_CACHE_EVENT_DEDUPE_ENTRIES: usize = 4_096;
pub const DEFAULT_CACHE_EVENT_DEDUPE_TTL: Duration = Duration::from_secs(60 * 60);
const CACHE_EVENT_DEDUPE_LOCK_STRIPES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheEventDedupeDecision {
    FirstSeen,
    Duplicate,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheEventDedupeStats {
    pub first_seen_total: u64,
    pub duplicate_total: u64,
    pub expired_total: u64,
    pub capacity_eviction_total: u64,
    pub entries: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheEventDedupeError {
    ZeroCapacity,
    ZeroTtl,
}

impl std::fmt::Display for CacheEventDedupeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroCapacity => write!(formatter, "cache event dedupe capacity must be positive"),
            Self::ZeroTtl => write!(formatter, "cache event dedupe TTL must be positive"),
        }
    }
}

impl std::error::Error for CacheEventDedupeError {}

#[derive(Debug)]
struct CacheEventDedupeState {
    entries: HashMap<Uuid, Instant>,
    insertion_order: VecDeque<(Instant, Uuid)>,
}

impl CacheEventDedupeState {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(capacity),
            insertion_order: VecDeque::with_capacity(capacity),
        }
    }
}

/// Bounded process-local deduplication for stable event identifiers.
///
/// This primitive is intentionally an optimization rather than a correctness gate. If an entry
/// expires or is evicted under capacity pressure, observing the same event again yields
/// `FirstSeen`; cache invalidation may rotate an extra generation, but it is never skipped because
/// of missing dedupe state.
#[derive(Debug)]
pub struct BoundedCacheEventDedupe {
    max_entries: usize,
    ttl: Duration,
    state: Mutex<CacheEventDedupeState>,
    event_locks: [AsyncMutex<()>; CACHE_EVENT_DEDUPE_LOCK_STRIPES],
    first_seen_total: AtomicU64,
    duplicate_total: AtomicU64,
    expired_total: AtomicU64,
    capacity_eviction_total: AtomicU64,
}

impl Default for BoundedCacheEventDedupe {
    fn default() -> Self {
        Self::new(
            DEFAULT_MAX_CACHE_EVENT_DEDUPE_ENTRIES,
            DEFAULT_CACHE_EVENT_DEDUPE_TTL,
        )
        .expect("default cache event dedupe configuration is valid")
    }
}

impl BoundedCacheEventDedupe {
    pub fn new(max_entries: usize, ttl: Duration) -> Result<Self, CacheEventDedupeError> {
        if max_entries == 0 {
            return Err(CacheEventDedupeError::ZeroCapacity);
        }
        if ttl.is_zero() {
            return Err(CacheEventDedupeError::ZeroTtl);
        }

        Ok(Self {
            max_entries,
            ttl,
            state: Mutex::new(CacheEventDedupeState::with_capacity(max_entries)),
            event_locks: std::array::from_fn(|_| AsyncMutex::new(())),
            first_seen_total: AtomicU64::new(0),
            duplicate_total: AtomicU64::new(0),
            expired_total: AtomicU64::new(0),
            capacity_eviction_total: AtomicU64::new(0),
        })
    }

    /// Serialize the probe/work/commit sequence for a stable event identifier.
    ///
    /// The bounded striped lock does not reserve or commit the identifier. Callers must re-check
    /// `is_duplicate` after acquiring the guard and call `observe` only after the protected work
    /// succeeds. A failed attempt simply drops the guard, allowing a retry to perform the work.
    pub async fn serialize_event(&self, event_id: Uuid) -> AsyncMutexGuard<'_, ()> {
        self.event_locks[event_lock_index(event_id)].lock().await
    }

    /// Check whether an event was already committed to this dedupe window.
    ///
    /// A false result does not reserve the identifier. Call `observe` only after the protected
    /// operation succeeds. Callers that can process the same event concurrently should hold the
    /// guard returned by `serialize_event` across probe, work, and commit.
    pub fn is_duplicate(&self, event_id: Uuid) -> bool {
        self.is_duplicate_at(event_id, Instant::now())
    }

    /// Commit an event identifier after the protected operation succeeds.
    pub fn observe(&self, event_id: Uuid) -> CacheEventDedupeDecision {
        self.observe_at(event_id, Instant::now())
    }

    pub fn stats(&self) -> CacheEventDedupeStats {
        let entries = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .entries
            .len();
        CacheEventDedupeStats {
            first_seen_total: self.first_seen_total.load(Ordering::Relaxed),
            duplicate_total: self.duplicate_total.load(Ordering::Relaxed),
            expired_total: self.expired_total.load(Ordering::Relaxed),
            capacity_eviction_total: self.capacity_eviction_total.load(Ordering::Relaxed),
            entries,
        }
    }

    pub fn max_entries(&self) -> usize {
        self.max_entries
    }

    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    fn is_duplicate_at(&self, event_id: Uuid, now: Instant) -> bool {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.prune_expired(&mut state, now);
        let duplicate = state.entries.contains_key(&event_id);
        if duplicate {
            self.duplicate_total.fetch_add(1, Ordering::Relaxed);
        }
        duplicate
    }

    fn observe_at(&self, event_id: Uuid, now: Instant) -> CacheEventDedupeDecision {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.prune_expired(&mut state, now);

        if state.entries.contains_key(&event_id) {
            self.duplicate_total.fetch_add(1, Ordering::Relaxed);
            return CacheEventDedupeDecision::Duplicate;
        }

        while state.entries.len() >= self.max_entries {
            let Some((inserted_at, oldest_id)) = state.insertion_order.pop_front() else {
                break;
            };
            if state.entries.get(&oldest_id).copied() == Some(inserted_at) {
                state.entries.remove(&oldest_id);
                self.capacity_eviction_total.fetch_add(1, Ordering::Relaxed);
            }
        }

        state.entries.insert(event_id, now);
        state.insertion_order.push_back((now, event_id));
        self.first_seen_total.fetch_add(1, Ordering::Relaxed);
        CacheEventDedupeDecision::FirstSeen
    }

    fn prune_expired(&self, state: &mut CacheEventDedupeState, now: Instant) {
        while let Some((inserted_at, event_id)) = state.insertion_order.front().copied() {
            if now.saturating_duration_since(inserted_at) < self.ttl {
                break;
            }
            state.insertion_order.pop_front();
            if state.entries.get(&event_id).copied() == Some(inserted_at) {
                state.entries.remove(&event_id);
                self.expired_total.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

fn event_lock_index(event_id: Uuid) -> usize {
    (event_id.as_u128() % CACHE_EVENT_DEDUPE_LOCK_STRIPES as u128) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn duplicate_is_suppressed_within_ttl() {
        let dedupe = BoundedCacheEventDedupe::new(4, Duration::from_secs(30)).unwrap();
        let event_id = Uuid::new_v4();
        let now = Instant::now();

        assert_eq!(
            dedupe.observe_at(event_id, now),
            CacheEventDedupeDecision::FirstSeen
        );
        assert!(dedupe.is_duplicate_at(event_id, now + Duration::from_secs(1)));
        assert_eq!(dedupe.stats().duplicate_total, 1);
    }

    #[test]
    fn probe_does_not_precommit_failed_work() {
        let dedupe = BoundedCacheEventDedupe::new(4, Duration::from_secs(30)).unwrap();
        let event_id = Uuid::new_v4();
        let now = Instant::now();

        assert!(!dedupe.is_duplicate_at(event_id, now));
        assert!(!dedupe.is_duplicate_at(event_id, now + Duration::from_secs(1)));
        assert_eq!(dedupe.stats().entries, 0);
        assert_eq!(
            dedupe.observe_at(event_id, now + Duration::from_secs(2)),
            CacheEventDedupeDecision::FirstSeen
        );
    }

    #[tokio::test]
    async fn same_event_serialization_closes_the_concurrent_probe_race() {
        let dedupe = Arc::new(BoundedCacheEventDedupe::new(4, Duration::from_secs(30)).unwrap());
        let event_id = Uuid::new_v4();
        let first_guard = dedupe.serialize_event(event_id).await;

        let waiter = {
            let dedupe = Arc::clone(&dedupe);
            tokio::spawn(async move {
                let _guard = dedupe.serialize_event(event_id).await;
                dedupe.is_duplicate(event_id)
            })
        };
        tokio::task::yield_now().await;
        assert!(!waiter.is_finished());

        assert_eq!(
            dedupe.observe(event_id),
            CacheEventDedupeDecision::FirstSeen
        );
        drop(first_guard);
        assert!(waiter.await.unwrap());
    }

    #[test]
    fn expired_event_is_accepted_again() {
        let dedupe = BoundedCacheEventDedupe::new(4, Duration::from_secs(2)).unwrap();
        let event_id = Uuid::new_v4();
        let now = Instant::now();

        assert_eq!(
            dedupe.observe_at(event_id, now),
            CacheEventDedupeDecision::FirstSeen
        );
        assert!(!dedupe.is_duplicate_at(event_id, now + Duration::from_secs(2)));
        assert_eq!(
            dedupe.observe_at(event_id, now + Duration::from_secs(2)),
            CacheEventDedupeDecision::FirstSeen
        );
        let stats = dedupe.stats();
        assert_eq!(stats.expired_total, 1);
        assert_eq!(stats.entries, 1);
    }

    #[test]
    fn capacity_evicts_oldest_without_skipping_new_events() {
        let dedupe = BoundedCacheEventDedupe::new(2, Duration::from_secs(60)).unwrap();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let third = Uuid::new_v4();
        let now = Instant::now();

        assert_eq!(
            dedupe.observe_at(first, now),
            CacheEventDedupeDecision::FirstSeen
        );
        assert_eq!(
            dedupe.observe_at(second, now + Duration::from_millis(1)),
            CacheEventDedupeDecision::FirstSeen
        );
        assert_eq!(
            dedupe.observe_at(third, now + Duration::from_millis(2)),
            CacheEventDedupeDecision::FirstSeen
        );
        assert_eq!(dedupe.stats().capacity_eviction_total, 1);
        assert_eq!(dedupe.stats().entries, 2);
        assert_eq!(
            dedupe.observe_at(first, now + Duration::from_millis(3)),
            CacheEventDedupeDecision::FirstSeen
        );
    }

    #[test]
    fn rejects_unbounded_configurations() {
        assert!(matches!(
            BoundedCacheEventDedupe::new(0, Duration::from_secs(1)),
            Err(CacheEventDedupeError::ZeroCapacity)
        ));
        assert!(matches!(
            BoundedCacheEventDedupe::new(1, Duration::ZERO),
            Err(CacheEventDedupeError::ZeroTtl)
        ));
    }
}
