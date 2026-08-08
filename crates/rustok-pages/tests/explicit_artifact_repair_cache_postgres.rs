use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rustok_core::events::EventHandler;
use rustok_core::{CONTENT_FORMAT_GRAPESJS, MigrationSource, SecurityContext};
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_outbox::{OutboxModule, OutboxTransport, SysEvents, TransactionalEventBus};
use rustok_page_builder::PageBuilderReviewedPublishRuntime;
use rustok_pages::dto::{
    CreatePageInput, PageBodyInput, PageBodyRevisionInput, PageTranslationInput, PublishPageInput,
    RebuildPageArtifactInput, ReplacePageArtifactBindingInput, ReviewedPagePublishRuntimeInput,
};
use rustok_pages::entities::{
    page, page_publish_rebuild_source, page_published_landing_artifact,
    page_static_landing_artifact,
};
use rustok_pages::services::PageService;
use rustok_pages::{
    PAGES_CACHE_ENTITY_KIND, PageCacheError, PageCacheGenerationSnapshot,
    PageCacheInvalidationCause, PageCacheInvalidationEventHandler, PageCacheInvalidationPort,
    PageCacheInvalidationReceipt, PageCacheInvalidationRequest, PagesCacheInvalidationRuntime,
    PagesModule,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection,
    DbBackend, EntityTrait, QueryFilter, Set, Statement,
};
use sea_orm_migration::SchemaManager;
use serde_json::json;
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_PAGES_TEST_DATABASE_URL";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

struct TestDatabase {
    control: DatabaseConnection,
    database_url: String,
    schema_name: String,
}

impl TestDatabase {
    async fn setup() -> TestResult<Option<Self>> {
        let Some(database_url) = database_url() else {
            eprintln!(
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Pages explicit repair cache harness"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_pages_explicit_repair_cache_{}",
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let db = scoped_connection(&database_url, &schema_name).await?;
        let manager = SchemaManager::new(&db);
        for migration in OutboxModule
            .migrations()
            .into_iter()
            .chain(PagesModule.migrations())
        {
            migration.up(&manager).await?;
        }

        Ok(Some(Self {
            control,
            database_url,
            schema_name,
        }))
    }

    async fn connection(&self) -> TestResult<DatabaseConnection> {
        scoped_connection(&self.database_url, &self.schema_name).await
    }

    async fn cleanup(self) -> TestResult<()> {
        self.control
            .execute_unprepared(&format!(
                r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
                self.schema_name
            ))
            .await?;
        Ok(())
    }
}

#[derive(Default)]
struct RecordingCacheState {
    generations: PageCacheGenerationSnapshot,
    requests: Vec<PageCacheInvalidationRequest>,
    receipts: Vec<PageCacheInvalidationReceipt>,
}

struct RecordingCachePort {
    state: Mutex<RecordingCacheState>,
}

impl RecordingCachePort {
    fn new(generations: PageCacheGenerationSnapshot) -> Self {
        Self {
            state: Mutex::new(RecordingCacheState {
                generations,
                ..RecordingCacheState::default()
            }),
        }
    }

    fn generations(&self) -> PageCacheGenerationSnapshot {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .generations
    }

    fn recorded(
        &self,
    ) -> (
        Vec<PageCacheInvalidationRequest>,
        Vec<PageCacheInvalidationReceipt>,
    ) {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (state.requests.clone(), state.receipts.clone())
    }
}

#[async_trait]
impl PageCacheInvalidationPort for RecordingCachePort {
    async fn invalidate(
        &self,
        request: PageCacheInvalidationRequest,
    ) -> Result<PageCacheInvalidationReceipt, PageCacheError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.requests.push(request.clone());
        let mut receipt = PageCacheInvalidationReceipt::new(&request);
        for scope in request.scopes() {
            let next = state.generations.generation(*scope) + 1;
            state.generations.record(*scope, next);
            receipt.record(*scope, next);
        }
        state.receipts.push(receipt.clone());
        Ok(receipt)
    }
}

#[tokio::test]
async fn rebuilt_bytes_and_activation_cache_rotate_only_after_committed_events_on_postgres(
) -> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let tenant_id = Uuid::new_v4();
    enable_pages_module(&db, tenant_id).await?;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let service = PageService::new(db.clone(), event_bus);
    let reviewed = PageBuilderReviewedPublishRuntime::new(
        "explicit-artifact-repair-cache-postgres",
        json!({ "surface": "storefront", "channel": "web" }),
    )?;
    let reviewed_input = || ReviewedPagePublishRuntimeInput {
        format: reviewed.format.clone(),
        scenario_id: reviewed.scenario_id.clone(),
        context: reviewed.context.clone(),
        review_hash: reviewed.review_hash.clone(),
    };
    let project = json!({
        "pages": [{
            "id": "home",
            "flyPageMeta": {
                "title": "Explicit repair cache",
                "description": "Byte reproduction and after-commit cache regression",
                "slug": "home"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": "Explicit repair cache"
                }]
            }
        }]
    });

    let draft = service
        .create(
            tenant_id,
            SecurityContext::system(),
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Explicit repair cache".to_string(),
                    slug: Some("home".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    content: serde_json::to_string(&project)?,
                    format: Some(CONTENT_FORMAT_GRAPESJS.to_string()),
                    content_json: None,
                }),
                channel_slugs: None,
                publish: false,
            },
        )
        .await?;
    let revision = draft
        .body
        .as_ref()
        .ok_or_else(|| std::io::Error::other("draft body is missing"))?
        .updated_at
        .clone();
    service
        .publish_reviewed(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            PublishPageInput {
                expected_version: draft.version,
                expected_body_revisions: vec![PageBodyRevisionInput {
                    locale: "en".to_string(),
                    revision,
                }],
                idempotency_key: "explicit-repair-cache-publish-v1".to_string(),
                runtime: reviewed_input(),
            },
        )
        .await?;

    let source = page_publish_rebuild_source::Entity::find()
        .filter(page_publish_rebuild_source::Column::TenantId.eq(tenant_id))
        .filter(page_publish_rebuild_source::Column::PageId.eq(draft.id))
        .filter(page_publish_rebuild_source::Column::Locale.eq("en"))
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("publish rebuild source is missing"))?;
    let original_artifact = page_static_landing_artifact::Entity::find_by_id(source.artifact_id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("canonical artifact is missing"))?;
    let canonical_snapshot = original_artifact.clone();
    let binding_before = page_published_landing_artifact::Entity::find()
        .filter(page_published_landing_artifact::Column::TenantId.eq(tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(draft.id))
        .filter(page_published_landing_artifact::Column::Locale.eq("en"))
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("published binding is missing"))?;
    let page_before = page::Entity::find_by_id(draft.id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("published page is missing"))?;

    let initial_generations = PageCacheGenerationSnapshot::new(11, 13, 17);
    let cache_port = Arc::new(RecordingCachePort::new(initial_generations));
    let invalidation_port: Arc<dyn PageCacheInvalidationPort> = cache_port.clone();
    let cache_handler = PageCacheInvalidationEventHandler::new(PagesCacheInvalidationRuntime::new(
        invalidation_port,
    ));

    let events_before_rebuild = outbox_event_ids(&db).await?;
    let mut damaged: page_static_landing_artifact::ActiveModel = original_artifact.into();
    damaged.document_html = Set("<main>damaged canonical document</main>".to_string());
    damaged.body_html = Set("<main>damaged canonical body</main>".to_string());
    damaged.css = Set("body{display:none}".to_string());
    damaged.update(&db).await?;

    let rebuilt = service
        .rebuild_immutable_artifact(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            RebuildPageArtifactInput {
                source_id: source.id,
                expected_provenance_hash: source.provenance_hash.clone(),
                idempotency_key: "explicit-repair-cache-rebuild-v1".to_string(),
                runtime: reviewed_input(),
            },
        )
        .await?;
    assert!(!rebuilt.replayed);
    assert_ne!(rebuilt.rebuilt_artifact_id, canonical_snapshot.id);
    assert_eq!(rebuilt.artifact_hash, canonical_snapshot.artifact_hash);
    assert_eq!(
        rebuilt.materialization_hash,
        canonical_snapshot
            .materialization_hash
            .clone()
            .ok_or_else(|| std::io::Error::other("canonical materialization hash is missing"))?
    );
    assert_eq!(outbox_event_ids(&db).await?, events_before_rebuild);
    assert_eq!(cache_port.generations(), initial_generations);

    let rebuilt_record =
        page_static_landing_artifact::Entity::find_by_id(rebuilt.rebuilt_artifact_id)
            .one(&db)
            .await?
            .ok_or_else(|| std::io::Error::other("rebuilt artifact is missing"))?;
    let mut expected_rebuilt = canonical_snapshot.clone();
    expected_rebuilt.id = rebuilt_record.id;
    expected_rebuilt.instance_key = rebuilt_record.instance_key.clone();
    expected_rebuilt.created_at = rebuilt_record.created_at;
    assert_eq!(rebuilt_record, expected_rebuilt);
    assert_eq!(
        page_published_landing_artifact::Entity::find_by_id(binding_before.page_body_id)
            .one(&db)
            .await?
            .ok_or_else(|| std::io::Error::other("binding disappeared after rebuild"))?
            .artifact_id,
        canonical_snapshot.id
    );
    assert_eq!(
        page::Entity::find_by_id(draft.id)
            .one(&db)
            .await?
            .ok_or_else(|| std::io::Error::other("page disappeared after rebuild"))?
            .version,
        page_before.version
    );

    let events_before_activation = outbox_event_ids(&db).await?;
    let replaced = service
        .replace_rebuilt_artifact_binding(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            ReplacePageArtifactBindingInput {
                rebuild_operation_id: rebuilt.operation_id,
                expected_version: page_before.version,
                expected_current_artifact_id: canonical_snapshot.id,
                idempotency_key: "explicit-repair-cache-activate-v1".to_string(),
            },
        )
        .await?;
    assert!(!replaced.replayed);
    assert_eq!(replaced.version, page_before.version + 1);
    assert_eq!(replaced.replacement_artifact_id, rebuilt.rebuilt_artifact_id);

    // The owner command has committed here, but cache generations cannot move until durable
    // lifecycle envelopes are delivered to the cache handler.
    assert_eq!(cache_port.generations(), initial_generations);
    let (updated_envelope, published_envelope) = activation_envelopes(
        &db,
        &events_before_activation,
        tenant_id,
        draft.id,
    )
    .await?;

    cache_handler.handle(&updated_envelope).await?;
    assert_eq!(
        cache_port.generations(),
        PageCacheGenerationSnapshot::new(
            initial_generations.route + 1,
            initial_generations.page + 1,
            initial_generations.artifact,
        )
    );

    cache_handler.handle(&published_envelope).await?;
    let final_generations = cache_port.generations();
    assert_eq!(
        final_generations,
        PageCacheGenerationSnapshot::new(
            initial_generations.route + 2,
            initial_generations.page + 2,
            initial_generations.artifact + 1,
        )
    );
    assert_cache_receipts(
        cache_port.as_ref(),
        &updated_envelope,
        &published_envelope,
        initial_generations,
        final_generations,
    )?;

    database.cleanup().await
}

fn assert_cache_receipts(
    port: &RecordingCachePort,
    updated: &EventEnvelope,
    published: &EventEnvelope,
    initial: PageCacheGenerationSnapshot,
    final_generations: PageCacheGenerationSnapshot,
) -> TestResult<()> {
    let (requests, receipts) = port.recorded();
    assert_eq!(requests.len(), 2);
    assert_eq!(receipts.len(), 2);

    assert_eq!(requests[0].event_id, updated.id);
    assert_eq!(requests[0].correlation_id, updated.correlation_id);
    assert_eq!(requests[0].cause, PageCacheInvalidationCause::Updated);
    assert_eq!(receipts[0].event_id, updated.id);
    assert_eq!(receipts[0].correlation_id, updated.correlation_id);
    assert_eq!(receipts[0].route_generation, Some(initial.route + 1));
    assert_eq!(receipts[0].page_generation, Some(initial.page + 1));
    assert_eq!(receipts[0].artifact_generation, None);

    assert_eq!(requests[1].event_id, published.id);
    assert_eq!(requests[1].correlation_id, published.correlation_id);
    assert_eq!(requests[1].cause, PageCacheInvalidationCause::Published);
    assert_eq!(receipts[1].event_id, published.id);
    assert_eq!(receipts[1].correlation_id, published.correlation_id);
    assert_eq!(receipts[1].route_generation, Some(final_generations.route));
    assert_eq!(receipts[1].page_generation, Some(final_generations.page));
    assert_eq!(receipts[1].artifact_generation, Some(final_generations.artifact));
    Ok(())
}

async fn activation_envelopes(
    db: &DatabaseConnection,
    before: &HashSet<Uuid>,
    tenant_id: Uuid,
    page_id: Uuid,
) -> TestResult<(EventEnvelope, EventEnvelope)> {
    let new_events = SysEvents::find()
        .all(db)
        .await?
        .into_iter()
        .filter(|event| !before.contains(&event.id))
        .collect::<Vec<_>>();
    assert_eq!(new_events.len(), 2);

    let mut updated = None;
    let mut published = None;
    for stored in new_events {
        let envelope: EventEnvelope = serde_json::from_value(stored.payload)?;
        envelope.validate_registered_schema()?;
        assert_eq!(envelope.tenant_id, tenant_id);
        let lifecycle = match &envelope.event {
            DomainEvent::NodeUpdated { node_id, kind } => {
                assert_eq!(*node_id, page_id);
                assert_eq!(kind, PAGES_CACHE_ENTITY_KIND);
                PageCacheInvalidationCause::Updated
            }
            DomainEvent::NodePublished { node_id, kind } => {
                assert_eq!(*node_id, page_id);
                assert_eq!(kind, PAGES_CACHE_ENTITY_KIND);
                PageCacheInvalidationCause::Published
            }
            other => panic!("unexpected activation lifecycle event: {other:?}"),
        };
        match lifecycle {
            PageCacheInvalidationCause::Updated => updated = Some(envelope),
            PageCacheInvalidationCause::Published => published = Some(envelope),
            other => panic!("unexpected activation cache cause: {other:?}"),
        }
    }

    Ok((
        updated.ok_or_else(|| std::io::Error::other("NodeUpdated envelope is missing"))?,
        published.ok_or_else(|| std::io::Error::other("NodePublished envelope is missing"))?,
    ))
}

async fn outbox_event_ids(db: &DatabaseConnection) -> TestResult<HashSet<Uuid>> {
    Ok(SysEvents::find()
        .all(db)
        .await?
        .into_iter()
        .map(|event| event.id)
        .collect())
}

async fn enable_pages_module(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<()> {
    db.execute_unprepared(
        "CREATE TABLE tenant_modules (\
            id UUID PRIMARY KEY NOT NULL, \
            tenant_id UUID NOT NULL, \
            module_slug TEXT NOT NULL, \
            enabled BOOLEAN NOT NULL, \
            settings JSONB NOT NULL, \
            created_at TIMESTAMPTZ NOT NULL, \
            updated_at TIMESTAMPTZ NOT NULL\
        )",
    )
    .await?;
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO tenant_modules (id, tenant_id, module_slug, enabled, settings, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![
            Uuid::new_v4().into(),
            tenant_id.into(),
            "pages".into(),
            true.into(),
            json!({}).into(),
        ],
    ))
    .await?;
    Ok(())
}

fn database_url() -> Option<String> {
    env::var(DATABASE_ENV)
        .or_else(|_| env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

async fn connect(database_url: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    Ok(Database::connect(options).await?)
}

async fn scoped_connection(
    database_url: &str,
    schema_name: &str,
) -> TestResult<DatabaseConnection> {
    let db = connect(database_url).await?;
    db.execute_unprepared(&format!(r#"SET search_path TO "{schema_name}""#))
        .await?;
    Ok(db)
}
