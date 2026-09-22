mod m20260726_000001_enforce_tenant_locale_policy;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(
        m20260726_000001_enforce_tenant_locale_policy::Migration,
    )]
}
