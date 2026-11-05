//! Canonical creation migrations for shared platform identity tables.
//!
//! Available without full domain composition so component fixtures can satisfy
//! real tenant/user foreign keys without recreating private table definitions.
//! The production migrator composes this same source with subsequent changes.

use sea_orm_migration::MigrationTrait;

#[path = "m20250101_000001_create_tenants.rs"]
pub(crate) mod m20250101_000001_create_tenants;
#[path = "m20250101_000002_create_users.rs"]
pub(crate) mod m20250101_000002_create_users;
#[path = "m20250101_000004_create_sessions.rs"]
pub(crate) mod m20250101_000004_create_sessions;
#[path = "m20250101_000005_create_roles_and_permissions.rs"]
pub(crate) mod m20250101_000005_create_roles_and_permissions;
#[path = "m20250101_000006_add_metadata_to_tenants_and_users.rs"]
pub(crate) mod m20250101_000006_add_metadata_to_tenants_and_users;
#[path = "m20250130_000004_create_tenant_locales.rs"]
pub(crate) mod m20250130_000004_create_tenant_locales;

/// Returns the canonical identity-table creation prefix, without module tables.
/// Callers still compose every subsequent migration required by their scenario.
pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20250101_000001_create_tenants::Migration),
        Box::new(m20250101_000002_create_users::Migration),
        Box::new(m20250130_000004_create_tenant_locales::Migration),
        Box::new(m20250101_000004_create_sessions::Migration),
        Box::new(m20250101_000005_create_roles_and_permissions::Migration),
        Box::new(m20250101_000006_add_metadata_to_tenants_and_users::Migration),
    ]
}
