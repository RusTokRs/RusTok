use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

use crate::{EffectivePolicyCacheIdentity, ModuleEffectivePolicy, ModuleEffectivePolicyError};

/// Default upper bound on how long a cached policy may be served without
/// re-resolution. Event-driven invalidation is the primary freshness
/// mechanism; the TTL is the fail-safe that bounds the damage of any missed
/// invalidation (e.g. a security-state change on another node).
pub const DEFAULT_POLICY_CACHE_MAX_AGE_SECONDS: i64 = 60;

/// Default upper bound on cached tenant policies per process.
pub const DEFAULT_POLICY_CACHE_MAX_ENTRIES: usize = 4096;

#[derive(Clone, Debug)]
struct PolicyCacheEntry {
    policy: ModuleEffectivePolicy,
    identity: EffectivePolicyCacheIdentity,
    cached_at: DateTime<Utc>,
}

impl PolicyCacheEntry {
    fn is_fresh(&self, now: DateTime<Utc>, max_age: Duration) -> bool {
        now.signed_duration_since(self.cached_at) < max_age
    }
}

/// Thread-safe, revision-dependent in-memory cache for resolved effective policies.
///
/// Neither tenant identity, TTL, nor a process generation alone can authorize a
/// cache hit. Lookups with an expected revision verify that the cached policy's
/// content-addressed `EffectivePolicyCacheIdentity` matches the expected revision
/// exactly. If it does not match, the lookup fails closed and returns `None`.
///
/// Every entry also carries a hard TTL (`max_age`): an entry older than the TTL
/// is a miss on every read path, including `get_latest`. Invalidation events
/// remain the primary freshness mechanism, but no missed invalidation can make
/// this cache serve a policy for longer than the TTL. The cache is bounded to
/// `max_entries` tenants; inserting beyond capacity evicts the oldest entry.
#[derive(Clone)]
pub struct ModuleEffectivePolicyCache {
    entries: Arc<RwLock<HashMap<Uuid, PolicyCacheEntry>>>,
    max_age: Duration,
    max_entries: usize,
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
            .is_fresh(Utc::now(), self.max_age)
            .then(|| entry.clone())
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

    /// Inserts a newly resolved effective policy into the cache, bound to its
    /// deterministic `EffectivePolicyCacheIdentity`.
    ///
    /// Fails with `CacheLockPoisoned` when the cache lock is poisoned instead
    /// of silently dropping the write: callers must learn that the next read
    /// will re-resolve from the durable source.
    pub fn insert(
        &self,
        tenant_id: Uuid,
        policy: ModuleEffectivePolicy,
    ) -> Result<EffectivePolicyCacheIdentity, ModuleEffectivePolicyError> {
        let identity = policy.cache_identity(tenant_id)?;
        if self.max_entries == 0 || self.max_age <= Duration::zero() {
            return Ok(identity);
        }
        let entry = PolicyCacheEntry {
            policy,
            identity: identity.clone(),
            cached_at: Utc::now(),
        };
        let mut entries = self
            .entries
            .write()
            .map_err(|_| ModuleEffectivePolicyError::CacheLockPoisoned)?;
        if !entries.contains_key(&tenant_id) && entries.len() >= self.max_entries {
            // Evict the oldest entry to keep the cache bounded. Capacity is
            // reached rarely (tenant churn), so the linear scan is acceptable.
            if let Some(oldest_tenant) = entries
                .iter()
                .min_by_key(|(_, candidate)| candidate.cached_at)
                .map(|(tenant_id, _)| *tenant_id)
            {
                entries.remove(&oldest_tenant);
            }
        }
        entries.insert(tenant_id, entry);
        Ok(identity)
    }

    /// Invalidates the cached policy for `tenant_id`.
    pub fn invalidate_tenant(&self, tenant_id: Uuid) -> bool {
        if let Ok(mut entries) = self.entries.write() {
            entries.remove(&tenant_id).is_some()
        } else {
            false
        }
    }

    /// Invalidates the cached entry only if its current revision matches `stale_revision`.
    pub fn invalidate_if_stale(&self, tenant_id: Uuid, stale_revision: &str) -> bool {
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

    /// Applies an outbox transition event: if the cached policy was at `previous_revision`,
    /// or if it has not yet reached `next_revision`, it is evicted so that the node cannot
    /// serve obsolete decisions.
    pub fn apply_transition_event(
        &self,
        tenant_id: Uuid,
        previous_revision: Option<&str>,
        next_revision: &str,
    ) -> bool {
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
