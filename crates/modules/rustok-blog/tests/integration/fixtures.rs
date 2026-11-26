#[path = "../support/schema.rs"]
mod schema;

use rustok_api::RichTextDocument;
use rustok_blog::BlogModule;
use rustok_comments::CommentsModule;
use rustok_core::MigrationSource;
use rustok_events::EventEnvelope;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use tokio::sync::broadcast;
use uuid::Uuid;

pub(super) type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub(super) fn richtext(text: &str) -> RichTextDocument {
    serde_json::from_value(serde_json::json!({
        "type": "doc",
        "content": [{
            "type": "paragraph",
            "content": [{"type": "text", "text": text}]
        }]
    }))
    .expect("test richtext")
}

pub(super) async fn setup_blog_test_db() -> DatabaseConnection {
    let db_url = format!(
        "sqlite:file:blog_integration_{}?mode=memory&cache=shared",
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

pub(super) async fn ensure_blog_schema(db: &DatabaseConnection) {
    schema::apply_blog_schema(db, BlogModule.migrations())
        .await
        .expect("complete Blog prerequisite migrations should apply");
    let manager = SchemaManager::new(db);
    for migration in CommentsModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("comments migration should apply");
    }
}

pub(super) async fn seed_tenant(db: &DatabaseConnection, tenant_id: Uuid) {
    db.execute_raw(sea_orm::Statement::from_sql_and_values(
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

pub(super) async fn seed_channel(db: &DatabaseConnection, tenant_id: Uuid, slug: &str) {
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

pub(super) fn drain_event_types(receiver: &mut broadcast::Receiver<EventEnvelope>) -> Vec<String> {
    let mut types = Vec::new();
    loop {
        match receiver.try_recv() {
            Ok(envelope) => types.push(envelope.event.event_type().to_string()),
            Err(broadcast::error::TryRecvError::Empty) => break,
            Err(broadcast::error::TryRecvError::Closed) => break,
            Err(broadcast::error::TryRecvError::Lagged(_)) => continue,
        }
    }
    types
}
