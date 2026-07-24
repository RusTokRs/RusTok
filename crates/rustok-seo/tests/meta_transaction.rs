use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result as AnyResult;
use async_trait::async_trait;
use rustok_api::TenantContext;
use rustok_core::{Error, EventEnvelope, EventTransport, ReliabilityLevel};
use rustok_outbox::TransactionalEventBus;
use rustok_seo::entities::{
    self as seo_meta, meta_translation, seo_event_delivery, seo_index_cursor,
    seo_index_delivery, seo_revision,
};
use rustok_seo::{
    SeoMetaInput, SeoMetaTranslationInput, SeoApplicationServices, SeoTargetRegistry, SeoTargetSlug,
    seo_builtin_slug,
};
use rustok_seo_targets::{
    SeoLoadedTargetRecord, SeoTargetAlternateRoute, SeoTargetCapabilities, SeoTargetLoadRequest,
    SeoTargetOpenGraphRecord, SeoTargetProvider, SeoTargetRuntimeContext, SeoTemplateFieldMap,
};
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database,
    DatabaseConnection, DbBackend, EntityTrait, PaginatorTrait, QueryFilter, Statement,
};
use serde_json::json;
use uuid::Uuid;

struct FailOnNthTransport {
    calls: AtomicUsize,
    fail_on: usize,
}

impl FailOnNthTransport {
    fn new(fail_on: usize) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            fail_on,
        }
    }
}

#[async_trait]
impl EventTransport for FailOnNthTransport {
    async fn publish(&self, _envelope: EventEnvelope) -> rustok_core::Result<()> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if call == self.fail_on {
            return Err(Error::Validation(format!(
                "forced SEO transaction failure on publish {call}"
            )));
        }
        Ok(())
    }

    fn reliability_level(&self) -> ReliabilityLevel {
        ReliabilityLevel::InMemory
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct TestPageProvider;

#[async_trait]
impl SeoTargetProvider for TestPageProvider {
    fn slug(&self) -> SeoTargetSlug {
        page_slug()
    }

    fn display_name(&self) -> &'static str {
        "Test page"
    }

    fn owner_module_slug(&self) -> &'static str {
        "test-pages"
    }

    fn capabilities(&self) -> SeoTargetCapabilities {
        SeoTargetCapabilities::new(true, true, false, false)
    }

    async fn load_target(
        &self,
        _runtime: &SeoTargetRuntimeContext,
        request: SeoTargetLoadRequest<'_>,
    ) -> AnyResult<Option<SeoLoadedTargetRecord>> {
        Ok(Some(SeoLoadedTargetRecord {
            target_kind: page_slug(),
            target_id: request.target_id,
            requested_locale: Some(request.locale.to_string()),
            effective_locale: request.locale.to_string(),
            title: "Transactional page".to_string(),
            description: Some("Transactional SEO metadata".to_string()),
            canonical_route: "/transactional-page".to_string(),
            alternates: vec![SeoTargetAlternateRoute {
                locale: request.locale.to_string(),
                route: "/transactional-page".to_string(),
            }],
            open_graph: SeoTargetOpenGraphRecord::default(),
            structured_data: json!({"@type": "WebPage"}),
            fallback_source: "test".to_string(),
            template_fields: SeoTemplateFieldMap::default(),
        }))
    }
}

#[tokio::test]
async fn metadata_transaction_rolls_back_when_reindex_event_fails() {
    let db = test_db().await;
    create_tables(&db).await;

    let tenant = TenantContext {
        id: Uuid::new_v4(),
        name: "SEO transaction tenant".to_string(),
        slug: "seo-transaction".to_string(),
        domain: None,
        settings: json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    };
    let target_id = Uuid::new_v4();
    let mut registry = SeoTargetRegistry::default();
    registry
        .register(TestPageProvider)
        .expect("test page provider should register");
    let service = SeoApplicationServices::new(
        db.clone(),
        TransactionalEventBus::new(Arc::new(FailOnNthTransport::new(2))),
        Arc::new(registry),
    );

    let error = service
        .metadata().upsert_meta(
            &tenant,
            SeoMetaInput {
                target_kind: page_slug(),
                target_id,
                noindex: false,
                nofollow: false,
                canonical_url: None,
                structured_data: None,
                translations: vec![SeoMetaTranslationInput {
                    locale: "en".to_string(),
                    title: Some("Atomic title".to_string()),
                    description: Some("Atomic description".to_string()),
                    keywords: None,
                    og_title: None,
                    og_description: None,
                    og_image: None,
                }],
            },
        )
        .await
        .expect_err("reindex failure must abort the metadata transaction");
    assert!(
        error
            .to_string()
            .contains("failed to enqueue SEO entity reindex event transactionally")
    );

    assert_eq!(
        seo_meta::Entity::find()
            .count(&db)
            .await
            .expect("metadata count should load"),
        0
    );
    assert_eq!(
        meta_translation::Entity::find()
            .count(&db)
            .await
            .expect("translation count should load"),
        0
    );
    assert_eq!(
        seo_event_delivery::Entity::find()
            .count(&db)
            .await
            .expect("event delivery count should load"),
        0
    );
    assert_eq!(
        seo_index_delivery::Entity::find()
            .count(&db)
            .await
            .expect("index delivery count should load"),
        0
    );
    assert_eq!(
        seo_index_cursor::Entity::find()
            .count(&db)
            .await
            .expect("index cursor count should load"),
        0
    );
}

#[tokio::test]
async fn revision_creation_rolls_back_when_reindex_event_fails() {
    let db = test_db().await;
    create_tables(&db).await;

    let tenant = TenantContext {
        id: Uuid::new_v4(),
        name: "SEO revision tenant".to_string(),
        slug: "seo-revision".to_string(),
        domain: None,
        settings: json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    };
    let target_id = Uuid::new_v4();
    let meta_id = Uuid::new_v4();
    seo_meta::ActiveModel {
        id: Set(meta_id),
        tenant_id: Set(tenant.id),
        target_type: Set(page_slug().into_string()),
        target_id: Set(target_id),
        no_index: Set(false),
        no_follow: Set(false),
        canonical_url: Set(None),
        structured_data: Set(None),
    }
    .insert(&db)
    .await
    .expect("explicit metadata should be seeded");
    meta_translation::ActiveModel {
        id: Set(Uuid::new_v4()),
        meta_id: Set(meta_id),
        locale: Set("en".to_string()),
        title: Set(Some("Revision title".to_string())),
        description: Set(Some("Revision description".to_string())),
        keywords: Set(None),
        og_title: Set(None),
        og_description: Set(None),
        og_image: Set(None),
    }
    .insert(&db)
    .await
    .expect("explicit metadata translation should be seeded");

    let service = SeoApplicationServices::new(
        db.clone(),
        TransactionalEventBus::new(Arc::new(FailOnNthTransport::new(2))),
        Arc::new(SeoTargetRegistry::default()),
    );
    let error = service
        .metadata().publish_revision(
            &tenant,
            page_slug(),
            target_id,
            Some("Atomic revision".to_string()),
        )
        .await
        .expect_err("reindex failure must abort revision creation");
    assert!(
        error
            .to_string()
            .contains("failed to enqueue SEO entity reindex event transactionally")
    );

    assert_eq!(
        seo_meta::Entity::find()
            .count(&db)
            .await
            .expect("metadata count should load"),
        1
    );
    assert_eq!(
        meta_translation::Entity::find()
            .count(&db)
            .await
            .expect("translation count should load"),
        1
    );
    assert_eq!(
        seo_revision::Entity::find()
            .count(&db)
            .await
            .expect("revision count should load"),
        0
    );
    assert_eq!(
        seo_event_delivery::Entity::find()
            .count(&db)
            .await
            .expect("event delivery count should load"),
        0
    );
    assert_eq!(
        seo_index_delivery::Entity::find()
            .count(&db)
            .await
            .expect("index delivery count should load"),
        0
    );
    assert_eq!(
        seo_index_cursor::Entity::find()
            .count(&db)
            .await
            .expect("index cursor count should load"),
        0
    );
}

#[tokio::test]
async fn revision_rollback_rolls_back_when_rollback_reindex_fails() {
    let db = test_db().await;
    create_tables(&db).await;

    let tenant = TenantContext {
        id: Uuid::new_v4(),
        name: "SEO rollback tenant".to_string(),
        slug: "seo-rollback".to_string(),
        domain: None,
        settings: json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    };
    let target_id = Uuid::new_v4();
    let meta_id = Uuid::new_v4();
    seo_meta::ActiveModel {
        id: Set(meta_id),
        tenant_id: Set(tenant.id),
        target_type: Set(page_slug().into_string()),
        target_id: Set(target_id),
        no_index: Set(false),
        no_follow: Set(false),
        canonical_url: Set(None),
        structured_data: Set(None),
    }
    .insert(&db)
    .await
    .expect("current metadata should be seeded");
    meta_translation::ActiveModel {
        id: Set(Uuid::new_v4()),
        meta_id: Set(meta_id),
        locale: Set("en".to_string()),
        title: Set(Some("Current title".to_string())),
        description: Set(Some("Current description".to_string())),
        keywords: Set(None),
        og_title: Set(None),
        og_description: Set(None),
        og_image: Set(None),
    }
    .insert(&db)
    .await
    .expect("current metadata translation should be seeded");
    seo_revision::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant.id),
        target_kind: Set(page_slug().into_string()),
        target_id: Set(target_id),
        revision: Set(1),
        note: Set(Some("Rollback snapshot".to_string())),
        payload: Set(json!({
            "noindex": true,
            "nofollow": true,
            "canonical_url": null,
            "structured_data": null,
            "translations": [{
                "locale": "en",
                "title": "Revision title",
                "description": "Revision description",
                "keywords": null,
                "og_title": null,
                "og_description": null,
                "og_image": null
            }]
        })),
        created_at: Set(chrono::Utc::now().fixed_offset()),
    }
    .insert(&db)
    .await
    .expect("rollback revision should be seeded");

    let mut registry = SeoTargetRegistry::default();
    registry
        .register(TestPageProvider)
        .expect("test page provider should register");
    let service = SeoApplicationServices::new(
        db.clone(),
        TransactionalEventBus::new(Arc::new(FailOnNthTransport::new(4))),
        Arc::new(registry),
    );
    let error = service
        .metadata().rollback_revision(&tenant, page_slug(), target_id, 1)
        .await
        .expect_err("rollback reindex failure must abort the whole revision rollback");
    assert!(
        error
            .to_string()
            .contains("failed to enqueue SEO entity reindex event transactionally")
    );

    let current_meta = seo_meta::Entity::find_by_id(meta_id)
        .one(&db)
        .await
        .expect("metadata should load")
        .expect("metadata should remain");
    assert!(!current_meta.no_index);
    assert!(!current_meta.no_follow);
    let current_translation = meta_translation::Entity::find()
        .filter(meta_translation::Column::MetaId.eq(meta_id))
        .filter(meta_translation::Column::Locale.eq("en"))
        .one(&db)
        .await
        .expect("translation should load")
        .expect("translation should remain");
    assert_eq!(current_translation.title.as_deref(), Some("Current title"));
    assert_eq!(
        current_translation.description.as_deref(),
        Some("Current description")
    );
    assert_eq!(
        seo_revision::Entity::find()
            .count(&db)
            .await
            .expect("revision count should load"),
        1
    );
    assert_eq!(
        seo_event_delivery::Entity::find()
            .count(&db)
            .await
            .expect("event delivery count should load"),
        0
    );
    assert_eq!(
        seo_index_delivery::Entity::find()
            .count(&db)
            .await
            .expect("index delivery count should load"),
        0
    );
    assert_eq!(
        seo_index_cursor::Entity::find()
            .count(&db)
            .await
            .expect("index cursor count should load"),
        0
    );
}

fn page_slug() -> SeoTargetSlug {
    SeoTargetSlug::new(seo_builtin_slug::PAGE).expect("page SEO target slug must stay valid")
}

async fn test_db() -> DatabaseConnection {
    let mut options = ConnectOptions::new("sqlite::memory:");
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    Database::connect(options)
        .await
        .expect("failed to connect SEO metadata test database")
}

async fn create_tables(db: &DatabaseConnection) {
    for sql in [
        "CREATE TABLE tenant_modules (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            module_slug TEXT NOT NULL,
            enabled INTEGER NOT NULL,
            settings TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
        "CREATE TABLE meta (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            target_type TEXT NOT NULL,
            target_id TEXT NOT NULL,
            no_index INTEGER NOT NULL,
            no_follow INTEGER NOT NULL,
            canonical_url TEXT NULL,
            structured_data TEXT NULL
        )",
        "CREATE TABLE meta_translations (
            id TEXT PRIMARY KEY,
            meta_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            title TEXT NULL,
            description TEXT NULL,
            keywords TEXT NULL,
            og_title TEXT NULL,
            og_description TEXT NULL,
            og_image TEXT NULL
        )",
        "CREATE UNIQUE INDEX idx_meta_translations_locale
            ON meta_translations (meta_id, locale)",
        "CREATE TABLE seo_revisions (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            target_kind TEXT NOT NULL,
            target_id TEXT NOT NULL,
            revision INTEGER NOT NULL,
            note TEXT NULL,
            payload TEXT NOT NULL,
            created_at TEXT NOT NULL
        )",
        "CREATE UNIQUE INDEX idx_seo_revisions_target_revision
            ON seo_revisions (tenant_id, target_kind, target_id, revision)",
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
        "CREATE TABLE seo_index_deliveries (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            seo_event_type TEXT NOT NULL,
            idempotency_key TEXT NOT NULL,
            target_type TEXT NOT NULL,
            target_id TEXT NULL,
            target_scope TEXT NOT NULL,
            target_scope_key TEXT NOT NULL,
            status TEXT NOT NULL,
            attempt_count INTEGER NOT NULL,
            outbox_event_id TEXT NULL,
            next_attempt_at TEXT NULL,
            last_error TEXT NULL,
            dead_lettered_at TEXT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            dispatched_at TEXT NULL
        )",
        "CREATE TABLE seo_index_cursors (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            target_type TEXT NOT NULL,
            initial_cursor_at TEXT NOT NULL,
            high_water_mark_at TEXT NOT NULL,
            last_repair_cursor_at TEXT NULL,
            replay_mode TEXT NOT NULL,
            replay_requested_at TEXT NULL,
            replay_completed_at TEXT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
    ] {
        db.execute(Statement::from_string(DbBackend::Sqlite, sql.to_string()))
            .await
            .expect("failed to create SEO metadata transaction test table");
    }
}
