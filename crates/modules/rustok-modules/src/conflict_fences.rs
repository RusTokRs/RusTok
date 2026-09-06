//! Deterministic conflict-key fences and cross-scope concurrency coordinator.
//!
//! Enforces a strict hierarchical ordering across all operation types to eliminate
//! distributed deadlocks:
//!
//! `ReleaseUnit < DataMigrationOwner < Namespace < Topology`

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeSet;
use uuid::Uuid;

/// Hierarchical classification of conflict-key scopes.
///
/// Integer values dictate the mandatory lock acquisition order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum ConflictKeyKind {
    /// Global module or static role release unit.
    ReleaseUnit = 1,
    /// Database schema or DDL migration owner lock.
    DataMigrationOwner = 2,
    /// Ingress traffic fence for a module.
    Traffic = 3,
    /// Background job and queue execution fence.
    JobQueue = 4,
    /// Tenant-scoped module namespace or settings domain.
    Namespace = 5,
    /// Physical or logical node deployment slot.
    Topology = 6,
}

/// Strongly-typed conflict key with enforced hierarchical ordering.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConflictKey {
    pub kind: ConflictKeyKind,
    pub key: String,
}

impl PartialOrd for ConflictKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ConflictKey {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.kind.cmp(&other.kind) {
            Ordering::Equal => self.key.cmp(&other.key),
            non_equal => non_equal,
        }
    }
}

impl ConflictKey {
    pub fn new(kind: ConflictKeyKind, key: impl Into<String>) -> Self {
        Self {
            kind,
            key: key.into(),
        }
    }

    /// Global release unit fence for dynamic module artifact.
    pub fn release_unit(module_slug: &str) -> Self {
        Self::new(
            ConflictKeyKind::ReleaseUnit,
            format!("release_unit:{}", module_slug),
        )
    }

    /// Global release unit fence for static distribution release.
    pub fn static_distribution_release(distribution_id: Uuid) -> Self {
        Self::new(
            ConflictKeyKind::ReleaseUnit,
            format!("release_unit:static:{}", distribution_id),
        )
    }

    /// Fleet-level operations-tool exclusion fence.
    pub fn fleet_operations_tool() -> Self {
        Self::new(
            ConflictKeyKind::ReleaseUnit,
            "fleet:operations_tool",
        )
    }

    /// Database schema migration owner fence for a module.
    pub fn data_migration_owner(module_slug: &str) -> Self {
        Self::new(
            ConflictKeyKind::DataMigrationOwner,
            format!("data_owner:{}", module_slug),
        )
    }

    /// Tenant-isolated module namespace fence.
    pub fn namespace(tenant_id: Uuid, module_slug: &str) -> Self {
        Self::new(
            ConflictKeyKind::Namespace,
            format!("namespace:{}:{}", tenant_id, module_slug),
        )
    }

    /// Physical/logical node deployment slot fence.
    pub fn topology(node_id: &str) -> Self {
        Self::new(ConflictKeyKind::Topology, format!("topology:{}", node_id))
    }

    /// Ingress traffic fence for a module.
    pub fn traffic(module_slug: &str, tenant_id: Option<Uuid>) -> Self {
        let key = match tenant_id {
            Some(tid) => format!("traffic:{}:{}", tid, module_slug),
            None => format!("traffic:{}", module_slug),
        };
        Self::new(ConflictKeyKind::Traffic, key)
    }

    /// Background job and queue execution fence for a module.
    pub fn job_queue(module_slug: &str, tenant_id: Option<Uuid>) -> Self {
        let key = match tenant_id {
            Some(tid) => format!("job_queue:{}:{}", tid, module_slug),
            None => format!("job_queue:{}", module_slug),
        };
        Self::new(ConflictKeyKind::JobQueue, key)
    }
}

/// Deterministically sorted, deduplicated set of conflict fences for an operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictFenceSet {
    keys: Vec<ConflictKey>,
}

impl ConflictFenceSet {
    /// Creates a conflict fence set, automatically sorting and deduplicating keys.
    pub fn new(keys: Vec<ConflictKey>) -> Self {
        let set: BTreeSet<ConflictKey> = keys.into_iter().collect();
        Self {
            keys: set.into_iter().collect(),
        }
    }

    pub fn keys(&self) -> &[ConflictKey] {
        &self.keys
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Checks if this fence set has any contention/overlap with another set.
    pub fn has_conflict_with(&self, other: &ConflictFenceSet) -> bool {
        self.keys.iter().any(|k| other.keys.contains(k))
    }

    /// Derives the canonical conflict fence set for a dynamic module update or rollout.
    pub fn derive_module_update_fences(
        module_slug: &str,
        tenant_id: Option<Uuid>,
        affected_nodes: &[String],
    ) -> Self {
        let mut keys = Vec::new();
        keys.push(ConflictKey::release_unit(module_slug));
        keys.push(ConflictKey::data_migration_owner(module_slug));

        if let Some(tid) = tenant_id {
            keys.push(ConflictKey::namespace(tid, module_slug));
        }

        for node in affected_nodes {
            keys.push(ConflictKey::topology(node));
        }

        Self::new(keys)
    }

    /// Derives the canonical conflict fence set for a module rollback.
    pub fn derive_module_rollback_fences(
        module_slug: &str,
        tenant_id: Option<Uuid>,
        affected_nodes: &[String],
    ) -> Self {
        Self::derive_module_update_fences(module_slug, tenant_id, affected_nodes)
    }

    /// Derives the canonical conflict fence set for a static distribution rollout.
    pub fn derive_static_distribution_rollout_fences(
        distribution_id: Uuid,
        affected_nodes: &[String],
    ) -> Self {
        let mut keys = Vec::new();
        keys.push(ConflictKey::static_distribution_release(distribution_id));

        for node in affected_nodes {
            keys.push(ConflictKey::topology(node));
        }

        Self::new(keys)
    }

    /// Derives the canonical conflict fence set for tenant-level module disable or purge.
    pub fn derive_tenant_purge_fences(module_slug: &str, tenant_id: Uuid) -> Self {
        let keys = vec![ConflictKey::namespace(tenant_id, module_slug)];
        Self::new(keys)
    }

    /// Derives point-of-no-return conflict fences (traffic, job, and write fences) before irreversible effects.
    pub fn derive_point_of_no_return_fences(
        module_slug: &str,
        tenant_id: Option<Uuid>,
        affected_nodes: &[String],
    ) -> Self {
        let mut keys = Vec::new();
        keys.push(ConflictKey::release_unit(module_slug));
        keys.push(ConflictKey::data_migration_owner(module_slug));
        keys.push(ConflictKey::traffic(module_slug, tenant_id));
        keys.push(ConflictKey::job_queue(module_slug, tenant_id));

        if let Some(tid) = tenant_id {
            keys.push(ConflictKey::namespace(tid, module_slug));
        }

        for node in affected_nodes {
            keys.push(ConflictKey::topology(node));
        }

        Self::new(keys)
    }

    /// Derives the canonical fleet exclusion fence set for operations-tool maintenance.
    pub fn derive_operations_tool_maintenance_fences() -> Self {
        Self::new(vec![ConflictKey::fleet_operations_tool()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strict_hierarchical_ordering() {
        let tenant_id = Uuid::new_v4();
        let k1 = ConflictKey::topology("node-1");
        let k2 = ConflictKey::namespace(tenant_id, "customer");
        let k3 = ConflictKey::data_migration_owner("customer");
        let k4 = ConflictKey::release_unit("customer");

        // Hierarchy rule: ReleaseUnit < DataMigrationOwner < Namespace < Topology
        let mut keys = vec![k1.clone(), k2.clone(), k3.clone(), k4.clone()];
        keys.sort();

        assert_eq!(keys[0], k4);
        assert_eq!(keys[1], k3);
        assert_eq!(keys[2], k2);
        assert_eq!(keys[3], k1);
    }

    #[test]
    fn test_conflict_detection_between_operations() {
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();

        let nodes = vec!["node-1".to_string(), "node-2".to_string()];

        // 1. Global update operation for 'customer'
        let global_update = ConflictFenceSet::derive_module_update_fences("customer", None, &nodes);

        // 2. Tenant A purge for 'customer'
        let tenant_a_purge = ConflictFenceSet::derive_tenant_purge_fences("customer", tenant_a);

        // 3. Tenant B purge for 'billing'
        let tenant_b_billing = ConflictFenceSet::derive_tenant_purge_fences("billing", tenant_b);

        // 4. Static rollout on node-1
        let static_rollout = ConflictFenceSet::derive_static_distribution_rollout_fences(
            Uuid::new_v4(),
            &["node-1".to_string()],
        );

        // Global update conflicts with static rollout because they share 'node-1'
        assert!(global_update.has_conflict_with(&static_rollout));

        // Tenant A purge does NOT conflict with Tenant B billing purge
        assert!(!tenant_a_purge.has_conflict_with(&tenant_b_billing));
    }
}
