mod m20260726_000001_enforce_tenant_locale_policy;
mod m20261102_000002_tenant_identity_keys;
mod m20261103_000003_tenant_identity_index;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260726_000001_enforce_tenant_locale_policy::Migration),
        Box::new(m20261102_000002_tenant_identity_keys::Migration),
        Box::new(m20261103_000003_tenant_identity_index::Migration),
    ]
}
