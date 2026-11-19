use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use uuid::Uuid;

use crate::{EffectivePolicyCacheIdentity, ModuleEffectivePolicy, ModuleEffectivePolicyError};

/// Default maximum age of a cache entry, measured using a monotonic clock.
/// A TTL limits reuse, but is not proof of current authorization: missed or
/// delayed invalidations can leave a stale entry available until it expires.
pub const DEFAULT_POLICY_CACHE_MAX_AGE_SECONDS: i64 = 60;

/// Default upper bound on cached tenant policies per process.
pub const DEFAULT_POLICY_CACHE_MAX_ENTRIES: usize = 4096;

#[derive(Clone, Debug)]
struct PolicyCacheEntry {
    policy: ModuleEffectivePolicy,
    identity: EffectivePolicyCacheIdentity,
    cached_at: DateTime<Utc>,
    cached_at_monotonic: Instant,
}

impl PolicyCacheEntry {
    fn is_fresh(&self, max_age: Duration) -> bool {
        max_age > Duration::zero()
            && max_age
                .to_std()
                .is_ok_and(|age| self.cached_at_monotonic.elapsed() < age)
    }
}

/// Thread-safe in-memory cache for resolved effective policies.
///
/// `get` checks a caller-supplied revision; `get_latest` does not obtain or
/// check the current owner revision. Even a revision match does not identify
/// volatile executor inputs. The monotonic TTL limits entry reuse, not the age
/// of the underlying database snapshot. Observed invalidations fence stamped
/// in-flight reads locally, but do not provide distributed or transactional
/// authorization freshness. Capacity is bounded by `max_entries`.
#[derive(Clone)]
pub struct ModuleEffectivePolicyCache {
    entries: Arc<RwLock<HashMap<Uuid, PolicyCacheEntry>>>,
    // One bounded process-local generation fences in-flight loaders across all
    // tenants. Lock ordering is always generation before entries.
    generation: Arc<RwLock<u64>>,
    max_age: Duration,
    max_entries: usize,
}

/// Non-forgeable process-local read stamp for one particular cache instance.
/// It is not a policy revision or a distributed ordering token.
#[derive(Clone)]
pub struct PolicyCacheReadStamp {
    generation: Arc<RwLock<u64>>,
    value: u64,
}

impl Default for ModuleEffectivePolicyCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleEffectivePolicyCache {
    pub fn new() -> Self {
        Self::with_limits(
            Duration::seconds(DEFAULT_POLICY_CACHE_MAX_AGE_SECONDS),
            DEFAULT_POLICY_CACHE_MAX_ENTRIES,
        )
    }

    /// Builds a cache with explicit freshness and capacity limits. A
    /// non-positive `max_age` or a zero `max_entries` disables serving from
    /// the cache entirely (every lookup is a miss, inserts are dropped).
    pub fn with_limits(max_age: Duration, max_entries: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            generation: Arc::new(RwLock::new(0)),
            max_age,
            max_entries,
        }
    }

    fn fresh_entry(
        &self,
        entries: &HashMap<Uuid, PolicyCacheEntry>,
        tenant_id: Uuid,
    ) -> Option<PolicyCacheEntry> {
        let entry = entries.get(&tenant_id)?;
        entry
            .is_fresh(self.max_age)
            .then(|| entry.clone())
    }

    /// Capture before starting an owner read. Every observed invalidation,
    /// including one for a missing entry, invalidates outstanding stamps.
    pub fn begin_read(&self) -> Result<PolicyCacheReadStamp, ModuleEffectivePolicyError> {
        let value = *self
            .generation
            .read()
            .map_err(|_| ModuleEffectivePolicyError::CacheLockPoisoned)?;
        if value == u64::MAX {
            return Err(ModuleEffectivePolicyError::CacheGenerationExhausted);
        }
        Ok(PolicyCacheReadStamp {
            generation: Arc::clone(&self.generation),
            value,
        })
    }

    /// Revalidate an earlier hit after awaiting other owner reads. Check the
    /// exact cached value and TTL as well as the generation, since the entry
    /// may expire or be replaced while the caller is suspended.
    pub fn is_current_hit(
        &self,
        stamp: &PolicyCacheReadStamp,
        tenant_id: Uuid,
        policy: &ModuleEffectivePolicy,
    ) -> Result<bool, ModuleEffectivePolicyError> {
        if !Arc::ptr_eq(&stamp.generation, &self.generation) {
            return Ok(false);
        }
        let generation = self
            .generation
            .read()
            .map_err(|_| ModuleEffectivePolicyError::CacheLockPoisoned)?;
        if *generation == u64::MAX {
            return Err(ModuleEffectivePolicyError::CacheGenerationExhausted);
        }
        if *generation != stamp.value {
            return Ok(false);
        }
        let entries = self
            .entries
            .read()
            .map_err(|_| ModuleEffectivePolicyError::CacheLockPoisoned)?;
        Ok(self
            .fresh_entry(&entries, tenant_id)
            .is_some_and(|entry| &entry.policy == policy))
    }

    /// Publish a read only while its stamp is current. A concurrent fresh
    /// insertion wins: the caller must re-read it rather than overwrite it
    /// with an older completion. `false` means discard and retry the owner read.
    pub fn insert_if_current(
        &self,
        stamp: &PolicyCacheReadStamp,
        tenant_id: Uuid,
        policy: ModuleEffectivePolicy,
    ) -> Result<bool, ModuleEffectivePolicyError> {
        let identity = policy.cache_identity(tenant_id)?;
        if !Arc::ptr_eq(&stamp.generation, &self.generation) {
            return Ok(false);
        }
        let generation = self
            .generation
            .read()
            .map_err(|_| ModuleEffectivePolicyError::CacheLockPoisoned)?;
        if *generation == u64::MAX {
            return Err(ModuleEffectivePolicyError::CacheGenerationExhausted);
        }
        if *generation != stamp.value {
            return Ok(false);
        }
        if self.max_entries == 0 || self.max_age <= Duration::zero() {
            return Ok(true);
        }
        let mut entries = self
            .entries
            .write()
            .map_err(|_| ModuleEffectivePolicyError::CacheLockPoisoned)?;
        if self.fresh_entry(&entries, tenant_id).is_some() {
            return Ok(false);
        }
        self.insert_entry(&mut entries, tenant_id, policy, identity);
        Ok(true)
    }

    /// Retrieves the cached policy for `tenant_id` if its content identity
    /// matches `expected_revision` exactly and the entry is within its TTL.
    ///
    /// If no entry exists or if the cached revision differs from `expected_revision`,
    /// this fails closed and returns `None`.
    pub fn get(&self, tenant_id: Uuid, expected_revision: &str) -> Option<ModuleEffectivePolicy> {
        let entries = self.entries.read().ok()?;
        let entry = self.fresh_entry(&entries, tenant_id)?;
        if entry.identity.matches(tenant_id, expected_revision) {
            Some(entry.policy)
        } else {
            None
        }
    }

    /// Retrieves the currently cached policy and identity for `tenant_id`, if
    /// any entry exists and is within its TTL.
    pub fn get_latest(
        &self,
        tenant_id: Uuid,
    ) -> Option<(ModuleEffectivePolicy, EffectivePolicyCacheIdentity)> {
        let entries = self.entries.read().ok()?;
        let entry = self.fresh_entry(&entries, tenant_id)?;
        Some((entry.policy, entry.identity))
    }

    /// Retrieves the currently cached policy, identity, and timestamp for
    /// `tenant_id`, if any entry exists and is within its TTL.
    pub fn get_with_metadata(
        &self,
        tenant_id: Uuid,
    ) -> Option<(
        ModuleEffectivePolicy,
        EffectivePolicyCacheIdentity,
        DateTime<Utc>,
    )> {
        let entries = self.entries.read().ok()?;
        let entry = self.fresh_entry(&entries, tenant_id)?;
        Some((entry.policy, entry.identity, entry.cached_at))
    }

    /// Inserts a supplied policy without fencing an asynchronous owner read.
    /// Callers loading policy across an await must instead use `begin_read` and
    /// `insert_if_current` or an invalidation can be overwritten by stale data.
    /// Fails with `CacheLockPoisoned` if the cache lock is poisoned.
    pub fn insert(
        &self,
        tenant_id: Uuid,
        policy: ModuleEffectivePolicy,
    ) -> Result<EffectivePolicyCacheIdentity, ModuleEffectivePolicyError> {
        let identity = policy.cache_identity(tenant_id)?;
        if self.max_entries == 0 || self.max_age <= Duration::zero() {
            return Ok(identity);
        }
        let mut entries = self
            .entries
            .write()
            .map_err(|_| ModuleEffectivePolicyError::CacheLockPoisoned)?;
        self.insert_entry(&mut entries, tenant_id, policy, identity.clone());
        Ok(identity)
    }

    fn insert_entry(
        &self,
        entries: &mut HashMap<Uuid, PolicyCacheEntry>,
        tenant_id: Uuid,
        policy: ModuleEffectivePolicy,
        identity: EffectivePolicyCacheIdentity,
    ) {
        let entry = PolicyCacheEntry {
            policy,
            identity,
            cached_at: Utc::now(),
            cached_at_monotonic: Instant::now(),
        };
        if !entries.contains_key(&tenant_id) && entries.len() >= self.max_entries {
            // Evict the oldest entry to keep the cache bounded. Capacity is
            // reached rarely (tenant churn), so the linear scan is acceptable.
            if let Some(oldest_tenant) = entries
                .iter()
                .min_by_key(|(_, candidate)| candidate.cached_at_monotonic)
                .map(|(tenant_id, _)| *tenant_id)
            {
                entries.remove(&oldest_tenant);
            }
        }
        entries.insert(tenant_id, entry);
    }

    /// Invalidates the cached policy for `tenant_id`.
    pub fn invalidate_tenant(&self, tenant_id: Uuid) -> bool {
        let Ok(mut generation) = self.generation.write() else {
            return false;
        };
        // Never wrap: saturation permanently disables stamped operations.
        *generation = generation.saturating_add(1);
        if let Ok(mut entries) = self.entries.write() {
            entries.remove(&tenant_id).is_some()
        } else {
            false
        }
    }

    /// Invalidates the cached entry only if its current revision matches `stale_revision`.
    pub fn invalidate_if_stale(&self, tenant_id: Uuid, stale_revision: &str) -> bool {
        let Ok(mut generation) = self.generation.write() else {
            return false;
        };
        *generation = generation.saturating_add(1);
        let Ok(mut entries) = self.entries.write() else {
            return false;
        };
        let should_remove = entries
            .get(&tenant_id)
            .is_some_and(|entry| entry.identity.matches(tenant_id, stale_revision));
        if should_remove {
            entries.remove(&tenant_id);
            return true;
        }
        false
    }

    /// Applies an observed transition event, fencing in-flight reads and evicting
    /// entries matching `previous_revision` or differing from `next_revision`.
    /// Revision equality alone does not prove the cached decisions are current.
    pub fn apply_transition_event(
        &self,
        tenant_id: Uuid,
        previous_revision: Option<&str>,
        next_revision: &str,
    ) -> bool {
        let Ok(mut generation) = self.generation.write() else {
            return false;
        };
        *generation = generation.saturating_add(1);
        let Ok(mut entries) = self.entries.write() else {
            return false;
        };
        let should_remove = entries.get(&tenant_id).is_some_and(|entry| {
            previous_revision.is_some_and(|previous| entry.identity.matches(tenant_id, previous))
                || !entry.identity.matches(tenant_id, next_revision)
        });
        if should_remove {
            entries.remove(&tenant_id);
            return true;
        }
        false
    }

    /// Purges all entries from the cache.
    pub fn clear(&self) {
        let Ok(mut generation) = self.generation.write() else {
            return;
        };
        *generation = generation.saturating_add(1);
        if let Ok(mut entries) = self.entries.write() {
            entries.clear();
        }
    }

    /// Returns the number of cached tenant entries.
    pub fn len(&self) -> usize {
        self.entries.read().map(|e| e.len()).unwrap_or(0)
    }

    /// Returns true if the cache contains no entries.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ModuleDefinitionCatalog;
    use crate::policy::ModuleEffectivePolicyQuery;

    fn empty_policy() -> ModuleEffectivePolicy {
        let catalog = ModuleDefinitionCatalog::default();
        ModuleEffectivePolicyQuery::new(&catalog, [], [], [])
            .execute()
            .expect("empty effective policy")
    }

    fn cache_with_policy(max_age: Duration) -> (ModuleEffectivePolicyCache, Uuid) {
        let cache = ModuleEffectivePolicyCache::with_limits(max_age, 8);
        let tenant_id = Uuid::new_v4();
        cache
            .insert(tenant_id, empty_policy())
            .expect("insert policy");
        (cache, tenant_id)
    }

    #[test]
    fn fresh_entry_is_served_and_expired_entry_is_a_miss() {
        let (cache, tenant_id) = cache_with_policy(Duration::seconds(60));
        assert!(cache.get_latest(tenant_id).is_some());

        let (expired_cache, expired_tenant) = cache_with_policy(Duration::seconds(-1));
        assert!(expired_cache.get_latest(expired_tenant).is_none());
        assert!(expired_cache.get_with_metadata(expired_tenant).is_none());
    }

    #[test]
    fn invalidation_fences_a_loader_that_finishes_after_the_event() {
        use std::sync::Barrier;
        use std::thread;

        let cache = ModuleEffectivePolicyCache::new();
        let tenant_id = Uuid::new_v4();
        let started = Arc::new(Barrier::new(2));
        let resume = Arc::new(Barrier::new(2));
        let loader_cache = cache.clone();
        let loader_started = Arc::clone(&started);
        let loader_resume = Arc::clone(&resume);
        let loader = thread::spawn(move || {
            let stamp = loader_cache.begin_read().unwrap();
            loader_started.wait();
            loader_resume.wait();
            loader_cache
                .insert_if_current(&stamp, tenant_id, empty_policy())
                .unwrap()
        });
        started.wait();
        // No entry existed, but the invalidation must still fence the read.
        assert!(!cache.invalidate_tenant(tenant_id));
        resume.wait();
        assert!(!loader.join().unwrap());
        assert!(cache.get_latest(tenant_id).is_none());
        let stamp = cache.begin_read().unwrap();
        assert!(cache
            .insert_if_current(&stamp, tenant_id, empty_policy())
            .unwrap());
        assert!(cache.get_latest(tenant_id).is_some());
    }

    #[test]
    fn clear_and_conditional_events_fence_even_without_an_entry() {
        let cache = ModuleEffectivePolicyCache::new();
        let tenant = Uuid::new_v4();
        let stamp = cache.begin_read().unwrap();
        cache.clear();
        assert!(!cache
            .insert_if_current(&stamp, tenant, empty_policy())
            .unwrap());
        let stamp = cache.begin_read().unwrap();
        assert!(!cache.invalidate_if_stale(tenant, "old"));
        assert!(!cache
            .insert_if_current(&stamp, tenant, empty_policy())
            .unwrap());
        let stamp = cache.begin_read().unwrap();
        assert!(!cache.apply_transition_event(tenant, Some("old"), "next"));
        assert!(!cache
            .insert_if_current(&stamp, tenant, empty_policy())
            .unwrap());
    }

    #[test]
    fn competing_fills_do_not_overwrite_a_fresh_winner_and_stamps_are_cache_local() {
        let cache = ModuleEffectivePolicyCache::new();
        let other = ModuleEffectivePolicyCache::new();
        let tenant = Uuid::new_v4();
        let first = cache.begin_read().unwrap();
        let second = cache.begin_read().unwrap();
        assert!(cache
            .insert_if_current(&second, tenant, empty_policy())
            .unwrap());
        assert!(!cache
            .insert_if_current(&first, tenant, empty_policy())
            .unwrap());
        assert!(!other
            .insert_if_current(&first, tenant, empty_policy())
            .unwrap());
        assert!(cache.is_current_hit(&first, tenant, &empty_policy()).unwrap());
        cache.clear();
        assert!(!cache.is_current_hit(&first, tenant, &empty_policy()).unwrap());
    }

    #[test]
    fn matching_transition_still_fences_a_suspended_hit() {
        let (cache, tenant) = cache_with_policy(Duration::seconds(60));
        let stamp = cache.begin_read().unwrap();
        let (policy, identity) = cache.get_latest(tenant).unwrap();
        assert!(!cache.apply_transition_event(tenant, None, identity.policy_revision()));
        assert!(cache.get_latest(tenant).is_some());
        assert!(!cache.is_current_hit(&stamp, tenant, &policy).unwrap());
    }

    #[test]
    fn ttl_uses_monotonic_time_not_wall_clock_metadata() {
        let (cache, tenant) = cache_with_policy(Duration::seconds(60));
        let stamp = cache.begin_read().unwrap();
        {
            let mut entries = cache.entries.write().unwrap();
            let entry = entries.get_mut(&tenant).unwrap();
            entry.cached_at = Utc::now() + Duration::days(1);
            entry.cached_at_monotonic = Instant::now() - std::time::Duration::from_secs(61);
        }
        assert!(cache.get_latest(tenant).is_none());
        assert!(!cache.is_current_hit(&stamp, tenant, &empty_policy()).unwrap());
    }

    #[test]
    fn generation_exhaustion_fails_closed_instead_of_wrapping() {
        let cache = ModuleEffectivePolicyCache::new();
        let tenant = Uuid::new_v4();
        let stamp = cache.begin_read().unwrap();
        *cache.generation.write().unwrap() = u64::MAX - 1;
        cache.invalidate_tenant(tenant);
        assert!(matches!(
            cache.begin_read(),
            Err(ModuleEffectivePolicyError::CacheGenerationExhausted)
        ));
        assert!(matches!(
            cache.insert_if_current(&stamp, tenant, empty_policy()),
            Err(ModuleEffectivePolicyError::CacheGenerationExhausted)
        ));
    }

    #[test]
    fn capacity_bound_evicts_oldest_tenant() {
        let cache = ModuleEffectivePolicyCache::with_limits(Duration::seconds(60), 2);
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let third = Uuid::new_v4();
        for tenant_id in [first, second, third] {
            cache
                .insert(tenant_id, empty_policy())
                .expect("insert policy");
        }
        assert_eq!(cache.len(), 2);
        assert!(cache.get_latest(third).is_some());
        // Exactly one of the first two tenants survived eviction.
        assert!(cache.get_latest(first).is_some() ^ cache.get_latest(second).is_some());
    }
}
