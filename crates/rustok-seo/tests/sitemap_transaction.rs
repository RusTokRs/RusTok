use std::any::Any;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::TenantContext;
use rustok_core::{Error, EventEnvelope, EventTransport, ReliabilityLevel};
use rustok_outbox::TransactionalEventBus;
use rustok_seo::entities::{seo_event_delivery, seo_sitemap_file};
use rustok_seo::{SeoApplicationServices, SeoTargetRegistry};
use rustok_tenant::entities::tenant_module;
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend,
    EntityTrait, PaginatorTrait, Statement,
};
use serde_json::json;
use uuid::Uuid;

struct FailingTransport;

#[async_trait]
impl EventTransport for FailingTransport {
    async fn publish(&self, _envelope: EventEnvelope) -> rustok_core::Result<()> {
        Err(Error::Validation(
            "forced sitemap transactional event failure".to_string(),
        ))
    }

    fn reliability_level(&self) -> ReliabilityLevel {
        ReliabilityLevel::InMemory
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[tokio::test]
async fn sitemap_generation_rolls_back_when_transactional_event_fails() {
    let db = test_db().await;
    create_tables(&db).await;

    let tenant_id = Uuid::new_v4();
    insert_seo_settings(&db, tenant_id).await;
    let service = SeoApplicationServices::new(
        db.clone(),
        TransactionalEventBus::new(Arc::new(FailingTransport)),
        Arc::new(SeoTargetRegistry::default()),
    );

    service
        .sitemaps()
        .generate_sitemaps(&tenant_context(tenant_id))
        .await
        .expect("queueing sitemap generation should succeed");

    let worker_auth =
        rustok_seo::SeoWorkerAuthorization::from_runtime_config(true, true).expect("worker auth");

    let job = service
        .sitemaps()
        .execute_next_sitemap_job(&worker_auth)
        .await
        .expect("worker execution should complete")
        .expect("job should exist");

    assert_eq!(job.status, "failed");
    assert!(
        job.last_error
            .as_deref()
            .unwrap_or_default()
            .contains("failed to enqueue sitemap event transactionally")
    );
    assert_eq!(
        seo_sitemap_file::Entity::find()
            .count(&db)
            .await
            .expect("sitemap file count should load"),
        0
    );
    assert_eq!(
        seo_event_delivery::Entity::find()
            .count(&db)
            .await
            .expect("delivery count should load"),
        0
    );
}

async fn test_db() -> DatabaseConnection {
    let mut options = ConnectOptions::new("sqlite::memory:");
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    Database::connect(options)
        .await
        .expect("failed to connect SEO test database")
}

async fn create_tables(db: &DatabaseConnection) {
    for sql in [
        "CREATE TABLE tenants (
            id TEXT PRIMARY KEY,
            slug TEXT NOT NULL,
            name TEXT NOT NULL,
            default_locale TEXT NOT NULL,
            domain TEXT NULL,
            settings TEXT NOT NULL DEFAULT '{}',
            is_active INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
        "CREATE TABLE tenant_modules (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            module_slug TEXT NOT NULL,
            enabled INTEGER NOT NULL,
            settings TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
        "CREATE TABLE seo_sitemap_jobs (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            status TEXT NOT NULL,
            file_count INTEGER NOT NULL,
            started_at TEXT NULL,
            completed_at TEXT NULL,
            last_error TEXT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
        "CREATE TABLE seo_sitemap_files (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            job_id TEXT NOT NULL,
            path TEXT NOT NULL,
            url_count INTEGER NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
        "CREATE TABLE seo_event_deliveries (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            idempotency_key TEXT NOT NULL,
            source_kind TEXT NULL,
            source_id TEXT NULL,
            status TEXT NOT NULL,
            outbox_event_id TEXT NULL,
            last_error TEXT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            dispatched_at TEXT NULL
        )",
        "CREATE UNIQUE INDEX idx_seo_event_deliveries_idempotency
            ON seo_event_deliveries (tenant_id, idempotency_key)",
    ] {
        db.execute_raw(Statement::from_string(DbBackend::Sqlite, sql.to_string()))
            .await
            .expect("failed to create SEO test table");
    }
}

async fn insert_seo_settings(db: &DatabaseConnection, tenant_id: Uuid) {
    let now = chrono::Utc::now();
    rustok_tenant::entities::tenant::ActiveModel {
        id: Set(tenant_id),
        slug: Set("seo-sitemap-test".to_string()),
        name: Set("SEO sitemap test tenant".to_string()),
        default_locale: Set("en".to_string()),
        domain: Set(Some("store.example.com".to_string())),
        settings: Set(json!({})),
        is_active: Set(true),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await
    .expect("failed to insert tenant");

    tenant_module::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        module_slug: Set("seo".to_string()),
        enabled: Set(true),
        settings: Set(json!({
            "sitemap_enabled": true,
            "sitemap_submission_endpoints": []
        })),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await
    .expect("failed to insert SEO settings");
}

fn tenant_context(tenant_id: Uuid) -> TenantContext {
    TenantContext {
        id: tenant_id,
        name: "SEO sitemap test tenant".to_string(),
        slug: "seo-sitemap-test".to_string(),
        domain: Some("store.example.com".to_string()),
        settings: json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    }
}
