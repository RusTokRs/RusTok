mod shared;

mod m20260308_000001_create_oauth_apps;
mod m20260308_000002_create_oauth_tokens;
mod m20260308_000003_create_oauth_codes;
mod m20260308_000004_create_oauth_consents;
mod m20260329_000001_add_oauth_app_granted_permissions;
mod m20260424_000001_rename_legacy_oauth_tables;
mod m20260713_000001_create_auth_invite_consumptions;
mod m20260714_900001_enforce_rbac_relation_tenant_integrity;
mod m20260714_900002_create_rbac_invalidation_state;
mod m20260716_000001_create_flex_field_definition_cache_generation;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260308_000001_create_oauth_apps::Migration),
        Box::new(m20260308_000002_create_oauth_tokens::Migration),
        Box::new(m20260308_000003_create_oauth_codes::Migration),
        Box::new(m20260308_000004_create_oauth_consents::Migration),
        Box::new(m20260329_000001_add_oauth_app_granted_permissions::Migration),
        Box::new(m20260424_000001_rename_legacy_oauth_tables::Migration),
        Box::new(m20260713_000001_create_auth_invite_consumptions::Migration),
        Box::new(m20260714_900001_enforce_rbac_relation_tenant_integrity::Migration),
        Box::new(m20260714_900002_create_rbac_invalidation_state::Migration),
        Box::new(m20260716_000001_create_flex_field_definition_cache_generation::Migration),
    ]
}
