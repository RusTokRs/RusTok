use super::*;
#[path = "../../../tests/support/schema.rs"]
mod schema;
use crate::dto::PublishedPostCursor;
use std::sync::Arc;

use rustok_core::{SecurityContext, UserRole};
use rustok_outbox::OutboxTransport;
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
    schema::apply_blog_schema(db, crate::migrations::migrations())
        .await
        .expect("complete Blog prerequisite migrations should apply");
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
include!("tests/admin_list_filters.rs");
