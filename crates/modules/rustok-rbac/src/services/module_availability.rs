//! Module-aware permission availability.
//!
//! Modules own slices of the permission vocabulary; roles are tenant records
//! that mix grants across modules. When a module is disabled for a tenant,
//! its grants do not disappear from role records — they become *dormant*:
//! excluded from effective authorization, preserved for re-enablement, and
//! surfaced to operators so the state is visible instead of surprising.
//!
//! Permissions with no known module owner are platform-core permissions and
//! are always active.

use std::collections::{BTreeMap, BTreeSet};

use rustok_api::Permission;

/// Availability of a single permission for a tenant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModulePermissionStatus {
    /// No owning module is registered: platform-core permission, always on.
    Core,
    /// Owned by a module that is enabled for the tenant.
    Enabled(String),
    /// Owned by a module that is disabled (or absent) for the tenant.
    Disabled(String),
}

impl ModulePermissionStatus {
    pub fn is_active(&self) -> bool {
        !matches!(self, ModulePermissionStatus::Disabled(_))
    }

    /// Stable wire label used by the GraphQL control plane.
    pub fn as_str(&self) -> &'static str {
        match self {
            ModulePermissionStatus::Core => "core",
            ModulePermissionStatus::Enabled(_) => "enabled",
            ModulePermissionStatus::Disabled(_) => "disabled",
        }
    }

    pub fn module_slug(&self) -> Option<&str> {
        match self {
            ModulePermissionStatus::Core => None,
            ModulePermissionStatus::Enabled(slug) | ModulePermissionStatus::Disabled(slug) => {
                Some(slug)
            }
        }
    }
}

/// Snapshot of permission ownership and tenant module enablement.
///
/// `ownership` maps a canonical permission string (`"resource:action"`) to
/// the slug of the module that declares it. When several modules declare the
/// same permission, the first registration wins: a permission must not flip
/// between owners depending on map iteration order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PermissionAvailability {
    ownership: BTreeMap<String, String>,
    enabled_modules: BTreeSet<String>,
}

impl PermissionAvailability {
    pub fn new(ownership: BTreeMap<String, String>, enabled_modules: BTreeSet<String>) -> Self {
        Self {
            ownership,
            enabled_modules,
        }
    }

    pub fn module_slug_for(&self, permission: &str) -> Option<&str> {
        self.ownership.get(permission).map(String::as_str)
    }

    pub fn status(&self, permission: &str) -> ModulePermissionStatus {
        match self.ownership.get(permission) {
            None => ModulePermissionStatus::Core,
            Some(slug) if self.enabled_modules.contains(slug) => {
                ModulePermissionStatus::Enabled(slug.clone())
            }
            Some(slug) => ModulePermissionStatus::Disabled(slug.clone()),
        }
    }

    pub fn is_active(&self, permission: &Permission) -> bool {
        self.status(&permission.to_string()).is_active()
    }

    /// Split grants into (active, dormant) preserving input order.
    pub fn partition_active(&self, grants: &[Permission]) -> (Vec<Permission>, Vec<Permission>) {
        let mut active = Vec::new();
        let mut dormant = Vec::new();
        for grant in grants {
            if self.is_active(grant) {
                active.push(*grant);
            } else {
                dormant.push(*grant);
            }
        }
        (active, dormant)
    }

    /// Keep only grants that are effective right now.
    pub fn active_permissions(&self, grants: &[Permission]) -> Vec<Permission> {
        self.partition_active(grants).0
    }

    /// The subset of `grants` owned by disabled modules.
    pub fn dormant_permissions(&self, grants: &[Permission]) -> Vec<Permission> {
        self.partition_active(grants).1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn availability() -> PermissionAvailability {
        PermissionAvailability::new(
            BTreeMap::from([
                ("posts:read".to_string(), "blog".to_string()),
                ("posts:manage".to_string(), "blog".to_string()),
                ("ai:providers:manage".to_string(), "ai".to_string()),
            ]),
            BTreeSet::from(["blog".to_string()]),
        )
    }

    #[test]
    fn unknown_permissions_are_core_and_active() {
        let snapshot = availability();
        assert_eq!(
            snapshot.status("users:manage"),
            ModulePermissionStatus::Core
        );
        assert!(snapshot.is_active(&Permission::USERS_MANAGE));
    }

    #[test]
    fn disabled_module_permissions_are_dormant() {
        let snapshot = availability();
        assert_eq!(
            snapshot.status("ai:providers:manage"),
            ModulePermissionStatus::Disabled("ai".to_string())
        );
        assert!(!snapshot.is_active(&Permission::AI_PROVIDERS_MANAGE));
        assert_eq!(
            snapshot.status("posts:read"),
            ModulePermissionStatus::Enabled("blog".to_string())
        );
    }

    #[test]
    fn partition_keeps_manage_wildcard_with_its_module() {
        let snapshot = availability();
        let (active, dormant) = snapshot.partition_active(&[
            Permission::POSTS_READ,
            Permission::AI_PROVIDERS_MANAGE,
            Permission::USERS_READ,
        ]);
        assert_eq!(active, vec![Permission::POSTS_READ, Permission::USERS_READ]);
        assert_eq!(dormant, vec![Permission::AI_PROVIDERS_MANAGE]);
    }
}
