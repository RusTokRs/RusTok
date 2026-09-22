use std::error::Error;
use std::sync::Arc;

use rustok_api::{Action, Resource};
use rustok_channel::ChannelModule;
use rustok_core::{MigrationSource, PermissionScope, SecurityContext};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_page_builder::PageBuilderReviewedPublishRuntime;
use rustok_pages::dto::{
    CreatePageInput, PageBodyInput, PageBodyRevisionInput, PageTranslationInput, PublishPageInput,
    RebuildPageArtifactInput, ReviewedPagePublishRuntimeInput,
};
use rustok_pages::entities::{page_publish_rebuild_source, page_static_landing_artifact};
use rustok_pages::services::PageService;
use rustok_pages::{
    AuditPageArtifactsInput, PAGE_ARTIFACT_INTEGRITY_AUDIT_FORMAT, PAGE_ARTIFACT_INTEGRITY_INVALID,
    PagesError, PagesModule,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection,
    DbBackend, EntityTrait, QueryFilter, Set, Statement,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use serde_json::json;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Clone)]
struct PublishedFixture {
    page_id: Uuid,
    source: page_publish_rebuild_source::Model,
    reviewed: PageBuilderReviewedPublishRuntime,
}

#[tokio::test]
async fn audit_manage_scope_is_all_or_none_and_public_read_is_denied() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let service = page_service(&db);
    let system = SecurityContext::system();
    let public = SecurityContext::public_read();

    assert_eq!(
        system.get_scope(Resource::Pages, Action::Manage),
        PermissionScope::All
    );
    assert_eq!(
        public.get_scope(Resource::Pages, Action::Manage),
        PermissionScope::None
    );

    let result = service
        .audit_immutable_artifact_integrity(
            tenant_id,
            public,
            Uuid::new_v4(),
            AuditPageArtifactsInput {
                max_records: Some(1),
            },
        )
        .await;
    assert!(matches!(
        result,
        Err(PagesError::Forbidden(message))
            if message == "Immutable artifact audit requires tenant-wide pages:manage"
    ));
    Ok(())
}

#[tokio::test]
async fn audit_accepts_canonical_and_rebuilt_records_and_truncates_at_requested_limit()
-> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let service = page_service(&db);
    let fixture = publish_fixture(&service, &db, tenant_id).await?;

    let rebuilt = service
        .rebuild_immutable_artifact(
            tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            RebuildPageArtifactInput {
                source_id: fixture.source.id,
                expected_provenance_hash: fixture.source.provenance_hash.clone(),
                idempotency_key: "artifact-audit-rebuild-v1".to_string(),
                runtime: reviewed_input(&fixture.reviewed),
            },
        )
        .await?;
    assert!(!rebuilt.replayed);

    let complete = service
        .audit_immutable_artifact_integrity(
            tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            AuditPageArtifactsInput {
                max_records: Some(2),
            },
        )
        .await?;
    assert_eq!(complete.format, PAGE_ARTIFACT_INTEGRITY_AUDIT_FORMAT);
    assert_eq!(complete.page_id, fixture.page_id);
    assert_eq!(complete.max_records, 2);
    assert_eq!(complete.scanned_artifact_count, 2);
    assert_eq!(complete.valid_artifact_count, 2);
    assert_eq!(complete.invalid_artifact_count, 0);
    assert!(!complete.truncated);
    assert!(!complete.findings_truncated);
    assert!(complete.findings.is_empty());
    assert_eq!(complete.audit_hash.len(), 64);

    let bounded = service
        .audit_immutable_artifact_integrity(
            tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            AuditPageArtifactsInput {
                max_records: Some(1),
            },
        )
        .await?;
    assert_eq!(bounded.max_records, 1);
    assert_eq!(bounded.scanned_artifact_count, 1);
    assert_eq!(bounded.valid_artifact_count, 1);
    assert_eq!(bounded.invalid_artifact_count, 0);
    assert!(bounded.truncated);
    assert!(!bounded.findings_truncated);
    assert_eq!(bounded.audit_hash.len(), 64);
    Ok(())
}

#[tokio::test]
async fn audit_reports_corrupted_immutable_payload_with_hashed_finding() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let service = page_service(&db);
    let fixture = publish_fixture(&service, &db, tenant_id).await?;
    let artifact = page_static_landing_artifact::Entity::find_by_id(fixture.source.artifact_id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("canonical artifact is missing"))?;
    let mut active: page_static_landing_artifact::ActiveModel = artifact.into();
    active.document_html = Set("<main>corrupted immutable payload</main>".to_string());
    active.update(&db).await?;

    let result = service
        .audit_immutable_artifact_integrity(
            tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            AuditPageArtifactsInput {
                max_records: Some(1),
            },
        )
        .await?;
    assert_eq!(result.scanned_artifact_count, 1);
    assert_eq!(result.valid_artifact_count, 0);
    assert_eq!(result.invalid_artifact_count, 1);
    assert!(!result.truncated);
    assert_eq!(result.findings.len(), 1);
    let finding = &result.findings[0];
    assert_eq!(finding.artifact_id, fixture.source.artifact_id);
    assert_eq!(finding.code, PAGE_ARTIFACT_INTEGRITY_INVALID);
    assert_eq!(finding.locale_hash.len(), 64);
    assert_eq!(finding.record_identity_hash.len(), 64);
    assert_eq!(finding.diagnostic_hash.len(), 64);
    assert_eq!(result.audit_hash.len(), 64);
    Ok(())
}

#[tokio::test]
async fn audit_reports_partial_materialization_evidence_without_exposing_payloads() -> TestResult<()>
{
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let service = page_service(&db);
    let fixture = publish_fixture(&service, &db, tenant_id).await?;
    let artifact = page_static_landing_artifact::Entity::find_by_id(fixture.source.artifact_id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("canonical artifact is missing"))?;
    assert!(artifact.materialization_hash.is_some());
    assert!(artifact.materialization_identity.is_some());
    assert!(artifact.runtime_snapshots.is_some());
    let mut active: page_static_landing_artifact::ActiveModel = artifact.into();
    active.runtime_snapshots = Set(None);
    active.update(&db).await?;

    let result = service
        .audit_immutable_artifact_integrity(
            tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            AuditPageArtifactsInput {
                max_records: Some(1),
            },
        )
        .await?;
    assert_eq!(result.scanned_artifact_count, 1);
    assert_eq!(result.valid_artifact_count, 0);
    assert_eq!(result.invalid_artifact_count, 1);
    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.findings[0].code, PAGE_ARTIFACT_INTEGRITY_INVALID);
    assert_eq!(result.findings[0].diagnostic_hash.len(), 64);
    assert_eq!(result.audit_hash.len(), 64);
    Ok(())
}

fn page_service(db: &DatabaseConnection) -> PageService {
    let transport = Arc::new(OutboxTransport::new(db.clone()));
    PageService::new(db.clone(), TransactionalEventBus::new(transport))
}

async fn publish_fixture(
    service: &PageService,
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<PublishedFixture> {
    let reviewed = PageBuilderReviewedPublishRuntime::new(
        "immutable-artifact-audit-sqlite",
        json!({ "surface": "storefront", "channel": "web" }),
    )?;
    let project = json!({
        "pages": [{
            "id": "home",
            "flyPageMeta": {
                "title": "Immutable artifact audit",
                "description": "SQLite audit regression",
                "slug": "home"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": "Immutable artifact audit"
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
                    title: "Immutable artifact audit".to_string(),
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
                idempotency_key: "artifact-audit-publish-v1".to_string(),
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
    Ok(PublishedFixture {
        page_id: draft.id,
        source,
        reviewed,
    })
}

fn reviewed_input(reviewed: &PageBuilderReviewedPublishRuntime) -> ReviewedPagePublishRuntimeInput {
    ReviewedPagePublishRuntimeInput {
        format: reviewed.format.clone(),
        scenario_id: reviewed.scenario_id.clone(),
        context: reviewed.context.clone(),
        review_hash: reviewed.review_hash.clone(),
    }
}

async fn setup_db(tenant_id: Uuid) -> TestResult<DatabaseConnection> {
    let database_url = format!(
        "sqlite:file:pages_immutable_artifact_audit_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;

    db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL)".to_string(),
    ))
    .await?;
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO tenants (id) VALUES (?)",
        [tenant_id.into()],
    ))
    .await?;
    db.execute_raw(Statement::from_string(
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
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO tenant_modules (id, tenant_id, module_slug, enabled, settings, created_at, updated_at) \
         VALUES (?, ?, 'pages', 1, '{}', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        [Uuid::new_v4().into(), tenant_id.into()],
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
