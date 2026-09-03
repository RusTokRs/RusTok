use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

use crate::{EffectivePolicyCacheIdentity, ModuleEffectivePolicy, ModuleEffectivePolicyError};

#[derive(Clone, Debug)]
struct PolicyCacheEntry {
    policy: ModuleEffectivePolicy,
    identity: EffectivePolicyCacheIdentity,
    cached_at: DateTime<Utc>,
}

/// Thread-safe, revision-dependent in-memory cache for resolved effective policies.
///
/// Neither tenant identity, TTL, nor a process generation alone can authorize a
/// cache hit. Lookups with an expected revision verify that the cached policy's
/// content-addressed `EffectivePolicyCacheIdentity` matches the expected revision
/// exactly. If it does not match, the lookup fails closed and returns `None`.
#[derive(Clone, Default)]
pub struct ModuleEffectivePolicyCache {
    entries: Arc<RwLock<HashMap<Uuid, PolicyCacheEntry>>>,
}

impl ModuleEffectivePolicyCache {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Retrieves the cached policy for `tenant_id` if its content identity
    /// matches `expected_revision` exactly.
    ///
    /// If no entry exists or if the cached revision differs from `expected_revision`,
    /// this fails closed and returns `None`.
    pub fn get(&self, tenant_id: Uuid, expected_revision: &str) -> Option<ModuleEffectivePolicy> {
        let entries = self.entries.read().ok()?;
        let entry = entries.get(&tenant_id)?;
        if entry.identity.matches(tenant_id, expected_revision) {
            Some(entry.policy.clone())
        } else {
            None
        }
    }

    /// Retrieves the currently cached policy and identity for `tenant_id`, if any.
    pub fn get_latest(
        &self,
        tenant_id: Uuid,
    ) -> Option<(ModuleEffectivePolicy, EffectivePolicyCacheIdentity)> {
        let entries = self.entries.read().ok()?;
        let entry = entries.get(&tenant_id)?;
        Some((entry.policy.clone(), entry.identity.clone()))
    }

    /// Retrieves the currently cached policy, identity, and timestamp for `tenant_id`, if any.
    pub fn get_with_metadata(
        &self,
        tenant_id: Uuid,
    ) -> Option<(
        ModuleEffectivePolicy,
        EffectivePolicyCacheIdentity,
        DateTime<Utc>,
    )> {
        let entries = self.entries.read().ok()?;
        let entry = entries.get(&tenant_id)?;
        Some((
            entry.policy.clone(),
            entry.identity.clone(),
            entry.cached_at,
        ))
    }

    /// Inserts a newly resolved effective policy into the cache, bound to its
    /// deterministic `EffectivePolicyCacheIdentity`.
    pub fn insert(
        &self,
        tenant_id: Uuid,
        policy: ModuleEffectivePolicy,
    ) -> Result<EffectivePolicyCacheIdentity, ModuleEffectivePolicyError> {
        let identity = policy.cache_identity(tenant_id)?;
        let entry = PolicyCacheEntry {
            policy,
            identity: identity.clone(),
            cached_at: Utc::now(),
        };
        if let Ok(mut entries) = self.entries.write() {
            entries.insert(tenant_id, entry);
        }
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
        if let Ok(mut entries) = self.entries.write() {
            if let Some(entry) = entries.get(&tenant_id) {
                if entry.identity.matches(tenant_id, stale_revision) {
                    entries.remove(&tenant_id);
                    return true;
                }
            }
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
        if let Ok(mut entries) = self.entries.write() {
            if let Some(entry) = entries.get(&tenant_id) {
                if let Some(prev) = previous_revision {
                    if entry.identity.matches(tenant_id, prev) {
                        entries.remove(&tenant_id);
                        return true;
                    }
                }
                if !entry.identity.matches(tenant_id, next_revision) {
                    entries.remove(&tenant_id);
                    return true;
                }
            }
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
