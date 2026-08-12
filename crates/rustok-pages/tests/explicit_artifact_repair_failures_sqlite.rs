use std::error::Error;
use std::sync::Arc;

use rustok_channel::ChannelModule;
use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxTransport, SysEvents, SysEventsMigration, TransactionalEventBus};
use rustok_page_builder::PageBuilderReviewedPublishRuntime;
use rustok_pages::dto::{
    CreatePageInput, PageBodyInput, PageBodyRevisionInput, PageTranslationInput, PublishPageInput,
    RebuildPageArtifactInput, RebuildPageArtifactResult, ReplacePageArtifactBindingInput,
    ReviewedPagePublishRuntimeInput,
};
use rustok_pages::entities::{
    page, page_artifact_binding_replacement_operation, page_artifact_rebuild_operation,
    page_publish_rebuild_source, page_published_landing_artifact, page_static_landing_artifact,
};
use rustok_pages::services::PageService;
use rustok_pages::{
    PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT,
    PAGE_ARTIFACT_BINDING_REPLACEMENT_TARGET_INVALID, PAGE_ARTIFACT_REBUILD_SOURCE_INVALID,
    PagesError, PagesModule,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection,
    DbBackend, EntityTrait, PaginatorTrait, QueryFilter, Set, Statement,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use serde_json::json;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Debug, Clone)]
struct PublishedFixture {
    page_id: Uuid,
    source: page_publish_rebuild_source::Model,
    original_artifact_id: Uuid,
    page_version: i32,
    reviewed: PageBuilderReviewedPublishRuntime,
}

#[derive(Debug, Clone)]
struct RebuiltFixture {
    published: PublishedFixture,
    rebuilt: RebuildPageArtifactResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RepairState {
    rebuild_receipts: u64,
    activation_receipts: u64,
    artifact_count: u64,
    binding_artifact_id: Uuid,
    page_version: i32,
    page_status: String,
    event_count: u64,
}

#[tokio::test]
async fn rebuild_rejects_corrupt_provenance_atomically() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let service = page_service(&db);
    let fixture = publish_fixture(&service, &db, tenant_id).await?;

    let mut source_active: page_publish_rebuild_source::ActiveModel = fixture.source.clone().into();
    source_active.provenance_hash = Set("0".repeat(64));
    let corrupted = source_active.update(&db).await?;
    let before = repair_state(&db, tenant_id, fixture.page_id).await?;

    let result = service
        .rebuild_immutable_artifact(
            tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            RebuildPageArtifactInput {
                source_id: corrupted.id,
                expected_provenance_hash: corrupted.provenance_hash.clone(),
                idempotency_key: "repair-failure-corrupt-provenance-v1".to_string(),
                runtime: reviewed_input(&fixture.reviewed),
            },
        )
        .await;
    assert!(matches!(
        result,
        Err(PagesError::PublishOperationIntegrity(message))
            if message.contains(PAGE_ARTIFACT_REBUILD_SOURCE_INVALID)
    ));

    let after = repair_state(&db, tenant_id, fixture.page_id).await?;
    assert_eq!(after, before);
    assert_eq!(
        page_publish_rebuild_source::Entity::find_by_id(corrupted.id)
            .one(&db)
            .await?
            .ok_or_else(|| std::io::Error::other("corrupted rebuild source disappeared"))?
            .provenance_hash,
        corrupted.provenance_hash
    );
    Ok(())
}

#[tokio::test]
async fn rebuild_rejects_reviewed_runtime_mismatch_atomically() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let service = page_service(&db);
    let fixture = publish_fixture(&service, &db, tenant_id).await?;
    let mismatched = PageBuilderReviewedPublishRuntime::new(
        "explicit-artifact-repair-failure-mismatch",
        json!({ "surface": "storefront", "channel": "mobile" }),
    )?;
    let before = repair_state(&db, tenant_id, fixture.page_id).await?;

    let result = service
        .rebuild_immutable_artifact(
            tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            RebuildPageArtifactInput {
                source_id: fixture.source.id,
                expected_provenance_hash: fixture.source.provenance_hash.clone(),
                idempotency_key: "repair-failure-runtime-mismatch-v1".to_string(),
                runtime: reviewed_input(&mismatched),
            },
        )
        .await;
    assert!(matches!(
        result,
        Err(PagesError::PublishRuntimeReviewInvalid(_))
    ));

    let after = repair_state(&db, tenant_id, fixture.page_id).await?;
    assert_eq!(after, before);
    Ok(())
}

#[tokio::test]
async fn activation_rejects_stale_version_atomically() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let service = page_service(&db);
    let fixture = rebuild_fixture(&service, &db, tenant_id).await?;
    let before = repair_state(&db, tenant_id, fixture.published.page_id).await?;
    let stale_version = before.page_version + 1;

    let result = service
        .replace_rebuilt_artifact_binding(
            tenant_id,
            SecurityContext::system(),
            fixture.published.page_id,
            ReplacePageArtifactBindingInput {
                rebuild_operation_id: fixture.rebuilt.operation_id,
                expected_version: stale_version,
                expected_current_artifact_id: fixture.published.original_artifact_id,
                idempotency_key: "repair-failure-stale-version-v1".to_string(),
            },
        )
        .await;
    assert!(matches!(
        result,
        Err(PagesError::VersionConflict {
            expected_version,
            actual_version,
        }) if expected_version == stale_version && actual_version == before.page_version
    ));

    let after = repair_state(&db, tenant_id, fixture.published.page_id).await?;
    assert_eq!(after, before);
    Ok(())
}

#[tokio::test]
async fn activation_rejects_invalid_replacement_atomically() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let service = page_service(&db);
    let fixture = rebuild_fixture(&service, &db, tenant_id).await?;

    let replacement =
        page_static_landing_artifact::Entity::find_by_id(fixture.rebuilt.rebuilt_artifact_id)
            .one(&db)
            .await?
            .ok_or_else(|| std::io::Error::other("rebuilt replacement artifact is missing"))?;
    let mut replacement_active: page_static_landing_artifact::ActiveModel = replacement.into();
    replacement_active.artifact_hash = Set("0".repeat(64));
    replacement_active.update(&db).await?;
    let before = repair_state(&db, tenant_id, fixture.published.page_id).await?;

    let result = service
        .replace_rebuilt_artifact_binding(
            tenant_id,
            SecurityContext::system(),
            fixture.published.page_id,
            ReplacePageArtifactBindingInput {
                rebuild_operation_id: fixture.rebuilt.operation_id,
                expected_version: before.page_version,
                expected_current_artifact_id: fixture.published.original_artifact_id,
                idempotency_key: "repair-failure-invalid-replacement-v1".to_string(),
            },
        )
        .await;
    assert!(matches!(
        result,
        Err(PagesError::RollbackTargetUnavailable(message))
            if message.contains(PAGE_ARTIFACT_BINDING_REPLACEMENT_TARGET_INVALID)
    ));

    let after = repair_state(&db, tenant_id, fixture.published.page_id).await?;
    assert_eq!(after, before);
    Ok(())
}

#[tokio::test]
async fn activation_rejects_unpublished_page_atomically() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let service = page_service(&db);
    let fixture = rebuild_fixture(&service, &db, tenant_id).await?;

    service
        .unpublish_if_current(
            tenant_id,
            SecurityContext::system(),
            fixture.published.page_id,
            Some(fixture.published.page_version),
        )
        .await?;
    let before = repair_state(&db, tenant_id, fixture.published.page_id).await?;
    assert_ne!(before.page_status, "published");

    let result = service
        .replace_rebuilt_artifact_binding(
            tenant_id,
            SecurityContext::system(),
            fixture.published.page_id,
            ReplacePageArtifactBindingInput {
                rebuild_operation_id: fixture.rebuilt.operation_id,
                expected_version: before.page_version,
                expected_current_artifact_id: fixture.published.original_artifact_id,
                idempotency_key: "repair-failure-unpublished-v1".to_string(),
            },
        )
        .await;
    assert!(matches!(
        result,
        Err(PagesError::RollbackTargetUnavailable(message))
            if message.contains(PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT)
    ));

    let after = repair_state(&db, tenant_id, fixture.published.page_id).await?;
    assert_eq!(after, before);
    Ok(())
}

fn page_service(db: &DatabaseConnection) -> PageService {
    PageService::new(
        db.clone(),
        TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone()))),
    )
}

async fn publish_fixture(
    service: &PageService,
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<PublishedFixture> {
    let reviewed = PageBuilderReviewedPublishRuntime::new(
        "explicit-artifact-repair-failures",
        json!({ "surface": "storefront", "channel": "web" }),
    )?;
    let project = json!({
        "pages": [{
            "id": "home",
            "flyPageMeta": {
                "title": "Explicit repair failures",
                "description": "Negative repair regression",
                "slug": "home"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": "Explicit repair failures"
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
                    title: "Explicit repair failures".to_string(),
                    slug: Some("home".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    document: project.clone(),
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
                idempotency_key: "repair-failure-publish-v1".to_string(),
                runtime: reviewed_input(&reviewed),
            },
        )
        .await?;

    let source = page_publish_rebuild_source::Entity::find()
        .filter(page_publish_rebuild_source::Column::TenantId.eq(tenant_id))
        .filter(page_publish_rebuild_source::Column::PageId.eq(draft.id))
        .filter(page_publish_rebuild_source::Column::Locale.eq("en"))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("publish rebuild source is missing"))?;
    let binding = page_published_landing_artifact::Entity::find()
        .filter(page_published_landing_artifact::Column::TenantId.eq(tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(draft.id))
        .filter(page_published_landing_artifact::Column::Locale.eq("en"))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("published binding is missing"))?;
    let stored_page = page::Entity::find_by_id(draft.id)
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("published page is missing"))?;
    Ok(PublishedFixture {
        page_id: draft.id,
        source,
        original_artifact_id: binding.artifact_id,
        page_version: stored_page.version,
        reviewed,
    })
}

async fn rebuild_fixture(
    service: &PageService,
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<RebuiltFixture> {
    let published = publish_fixture(service, db, tenant_id).await?;
    let before = repair_state(db, tenant_id, published.page_id).await?;
    let rebuilt = service
        .rebuild_immutable_artifact(
            tenant_id,
            SecurityContext::system(),
            published.page_id,
            RebuildPageArtifactInput {
                source_id: published.source.id,
                expected_provenance_hash: published.source.provenance_hash.clone(),
                idempotency_key: "repair-failure-rebuild-v1".to_string(),
                runtime: reviewed_input(&published.reviewed),
            },
        )
        .await?;
    let after = repair_state(db, tenant_id, published.page_id).await?;
    assert_eq!(after.rebuild_receipts, before.rebuild_receipts + 1);
    assert_eq!(after.activation_receipts, before.activation_receipts);
    assert_eq!(after.artifact_count, before.artifact_count + 1);
    assert_eq!(after.binding_artifact_id, before.binding_artifact_id);
    assert_eq!(after.page_version, before.page_version);
    assert_eq!(after.page_status, before.page_status);
    assert_eq!(after.event_count, before.event_count);
    Ok(RebuiltFixture { published, rebuilt })
}

fn reviewed_input(reviewed: &PageBuilderReviewedPublishRuntime) -> ReviewedPagePublishRuntimeInput {
    ReviewedPagePublishRuntimeInput {
        format: reviewed.format.clone(),
        scenario_id: reviewed.scenario_id.clone(),
        context: reviewed.context.clone(),
        review_hash: reviewed.review_hash.clone(),
    }
}

async fn repair_state(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    page_id: Uuid,
) -> TestResult<RepairState> {
    let binding = page_published_landing_artifact::Entity::find()
        .filter(page_published_landing_artifact::Column::TenantId.eq(tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(page_id))
        .filter(page_published_landing_artifact::Column::Locale.eq("en"))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("published binding is missing"))?;
    let stored_page = page::Entity::find_by_id(page_id)
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("page is missing"))?;
    Ok(RepairState {
        rebuild_receipts: page_artifact_rebuild_operation::Entity::find()
            .filter(page_artifact_rebuild_operation::Column::TenantId.eq(tenant_id))
            .filter(page_artifact_rebuild_operation::Column::PageId.eq(page_id))
            .count(db)
            .await?,
        activation_receipts: page_artifact_binding_replacement_operation::Entity::find()
            .filter(page_artifact_binding_replacement_operation::Column::TenantId.eq(tenant_id))
            .filter(page_artifact_binding_replacement_operation::Column::PageId.eq(page_id))
            .count(db)
            .await?,
        artifact_count: page_static_landing_artifact::Entity::find()
            .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
            .filter(page_static_landing_artifact::Column::PageId.eq(page_id))
            .count(db)
            .await?,
        binding_artifact_id: binding.artifact_id,
        page_version: stored_page.version,
        page_status: stored_page.status,
        event_count: SysEvents::find().count(db).await?,
    })
}

async fn setup_db(tenant_id: Uuid) -> TestResult<DatabaseConnection> {
    let database_url = format!(
        "sqlite:file:pages_explicit_artifact_repair_failures_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;

    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL)".to_string(),
    ))
    .await?;
    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO tenants (id) VALUES (?)",
        [tenant_id.into()],
    ))
    .await?;
    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE tenant_modules (\
            id TEXT PRIMARY KEY NOT NULL, \
            tenant_id TEXT NOT NULL, \
            module_slug TEXT NOT NULL, \
            enabled INTEGER NOT NULL, \
            settings TEXT NOT NULL, \
            created_at TEXT NOT NULL, \
            updated_at TEXT NOT NULL\
        )"
        .to_string(),
    ))
    .await?;

    let manager = SchemaManager::new(&db);
    SysEventsMigration.up(&manager).await?;
    for migration in ChannelModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in PagesModule.migrations() {
        migration.up(&manager).await?;
    }
    Ok(db)
}
