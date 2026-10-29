use super::*;
use std::sync::Arc;

use rustok_core::MigrationSource;
use rustok_core::{SecurityContext, UserRole};
use rustok_outbox::{OutboxTransport, SysEventsMigration};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Statement};
use sea_orm_migration::{MigrationTrait, SchemaManager};

async fn setup_test_db() -> DatabaseConnection {
    let db_url = format!(
        "sqlite:file:blog_service_post_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut opts = ConnectOptions::new(db_url);
    opts.max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);

    Database::connect(opts)
        .await
        .expect("failed to connect blog test sqlite database")
}

async fn ensure_blog_schema(db: &DatabaseConnection) {
    db.execute_raw(Statement::from_string(
        db.get_database_backend(),
        r#"
        CREATE TABLE IF NOT EXISTS tenants (
            id TEXT PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            slug TEXT NOT NULL UNIQUE,
            domain TEXT NULL UNIQUE,
            settings TEXT NOT NULL DEFAULT '{}',
            default_locale TEXT NOT NULL DEFAULT 'en',
            is_active BOOLEAN NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    ))
    .await
    .expect("tenants table should exist for channel foreign keys");

    let manager = SchemaManager::new(db);
    SysEventsMigration
        .up(&manager)
        .await
        .expect("outbox migration should apply");
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("taxonomy migration should apply");
    }
    for migration in rustok_channel::migrations::migrations() {
        if db.get_database_backend() == sea_orm::DatabaseBackend::Sqlite
            && (migration.name() == "m20260730_000010_add_channel_index_revision"
                || migration.name() == "m20260731_000011_add_channel_index_tombstones"
                || migration.name() == "m20260807_000012_add_channel_index_identity_generation")
        {
            continue;
        }
        migration
            .up(&manager)
            .await
            .expect("channel migration should apply");
    }
    for migration in crate::migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("blog migration should apply");
    }
}

async fn seed_tenant(db: &DatabaseConnection, tenant_id: Uuid) {
    db.execute_raw(Statement::from_sql_and_values(
        db.get_database_backend(),
        "INSERT OR IGNORE INTO tenants (id, name, slug, settings, default_locale, is_active, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        [
            tenant_id.into(),
            format!("Tenant {tenant_id}").into(),
            format!("tenant-{tenant_id}").into(),
            "{}".to_string().into(),
            "en".to_string().into(),
            true.into(),
        ],
    ))
    .await
    .expect("tenant should be seeded");
}

async fn seed_channel(db: &DatabaseConnection, tenant_id: Uuid, slug: &str) {
    let service = rustok_channel::ChannelService::new(db.clone());
    service
        .create_channel(rustok_channel::CreateChannelInput {
            tenant_id,
            slug: slug.to_string(),
            name: format!("Channel {slug}"),
            settings: None,
        })
        .await
        .expect("channel should be created");
}


// Include focused test groups here to preserve the established test path namespace.
include!("tests/unit.rs");
include!("tests/lifecycle.rs");
include!("tests/public_listing.rs");
include!("tests/scheduling.rs");
include!("tests/search.rs");
