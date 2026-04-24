mod shared;

mod m20260308_000001_create_oauth_apps;
mod m20260308_000002_create_oauth_tokens;
mod m20260308_000003_create_oauth_codes;
mod m20260308_000004_create_oauth_consents;
mod m20260329_000001_add_oauth_app_granted_permissions;
mod m20260424_000001_rename_legacy_oauth_tables;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260308_000001_create_oauth_apps::Migration),
        Box::new(m20260308_000002_create_oauth_tokens::Migration),
        Box::new(m20260308_000003_create_oauth_codes::Migration),
        Box::new(m20260308_000004_create_oauth_consents::Migration),
        Box::new(m20260329_000001_add_oauth_app_granted_permissions::Migration),
        Box::new(m20260424_000001_rename_legacy_oauth_tables::Migration),
    ]
}
