use rustok_core::{MigrationSource, RusToKModule};
use rustok_tenant::TenantModule;
use sea_orm::{ConnectionTrait, Database};
use sea_orm_migration::SchemaManager;

#[test]
fn module_metadata() {
    let module = TenantModule;
    assert_eq!(module.slug(), "tenant");
    assert_eq!(module.name(), "Tenant");
    assert_eq!(
        module.description(),
        "Multi-tenancy helpers and tenant metadata."
    );
    assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn module_exposes_locale_policy_migration_contract() {
    let module = TenantModule;
    assert_eq!(module.migrations().len(), 1);

    let dependencies = module.migration_dependencies();
    assert_eq!(dependencies.len(), 1);
    assert_eq!(
        dependencies[0].migration,
        "m20260726_000001_enforce_tenant_locale_policy"
    );
    assert_eq!(
        dependencies[0].after,
        vec!["m20260405_000001_expand_locale_storage_columns"]
    );
}

#[tokio::test]
async fn locale_policy_migration_enforces_sqlite_row_invariants() {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("SQLite test database should connect");
    db.execute_unprepared(
        r#"
CREATE TABLE tenants (
    id TEXT PRIMARY KEY NOT NULL
);
CREATE TABLE tenant_locales (
    tenant_id TEXT NOT NULL,
    locale TEXT NOT NULL,
    name TEXT NOT NULL,
    native_name TEXT NOT NULL,
    is_default BOOLEAN NOT NULL DEFAULT 0,
    is_enabled BOOLEAN NOT NULL DEFAULT 1,
    fallback_locale TEXT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tenant_id, locale),
    FOREIGN KEY (tenant_id) REFERENCES tenants (id)
);
INSERT INTO tenants (id) VALUES ('00000000-0000-0000-0000-000000000001');
"#,
    )
    .await
    .expect("foundation tenant locale schema should be created");

    let migration = TenantModule
        .migrations()
        .into_iter()
        .next()
        .expect("tenant locale policy migration should exist");
    migration
        .up(&SchemaManager::new(&db))
        .await
        .expect("tenant locale policy migration should apply to SQLite");

    let invalid_default = db
        .execute_unprepared(
            r#"
INSERT INTO tenant_locales (
    tenant_id, locale, name, native_name, is_default, is_enabled
) VALUES (
    '00000000-0000-0000-0000-000000000001', 'en', 'English', 'English', 1, 0
);
"#,
        )
        .await;
    assert!(invalid_default.is_err());

    let self_fallback = db
        .execute_unprepared(
            r#"
INSERT INTO tenant_locales (
    tenant_id, locale, name, native_name, is_default, is_enabled, fallback_locale
) VALUES (
    '00000000-0000-0000-0000-000000000001', 'de', 'German', 'Deutsch', 0, 1, 'de'
);
"#,
        )
        .await;
    assert!(self_fallback.is_err());
}
