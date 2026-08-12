use std::env;
use std::error::Error;
use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_page_builder::PageBuilderReviewedPublishRuntime;
use rustok_pages::dto::{
    CreatePageInput, PageBodyInput, PageBodyRevisionInput, PageTranslationInput, PublishPageInput,
    RebuildPageArtifactInput, ReviewedPagePublishRuntimeInput,
};
use rustok_pages::entities::{page, page_publish_rebuild_source, page_static_landing_artifact};
use rustok_pages::services::PageService;
use rustok_pages::{
    AuditPageArtifactsInput, PAGE_ARTIFACT_INTEGRITY_AUDIT_FORMAT, PAGE_ARTIFACT_INTEGRITY_INVALID,
    PagesModule,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection,
    DbBackend, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set, Statement, TransactionTrait,
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
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Pages immutable artifact audit PostgreSQL harness"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let schema_name = format!("rustok_pages_artifact_audit_{}", Uuid::new_v4().simple());
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

#[derive(Clone)]
struct PublishedFixture {
    page_id: Uuid,
    source: page_publish_rebuild_source::Model,
    reviewed: PageBuilderReviewedPublishRuntime,
}

#[tokio::test]
async fn immutable_artifact_audit_locking_and_findings_hold_on_postgres() -> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let tenant_id = Uuid::new_v4();
    enable_pages_module(&db, tenant_id).await?;
    let service = page_service(&db);

    let valid = publish_fixture(&service, &db, tenant_id, "valid").await?;
    let rebuilt = service
        .rebuild_immutable_artifact(
            tenant_id,
            SecurityContext::system(),
            valid.page_id,
            RebuildPageArtifactInput {
                source_id: valid.source.id,
                expected_provenance_hash: valid.source.provenance_hash.clone(),
                idempotency_key: "artifact-audit-postgres-rebuild-valid-v1".to_string(),
                runtime: reviewed_input(&valid.reviewed),
            },
        )
        .await?;
    assert!(!rebuilt.replayed);

    let complete = service
        .audit_immutable_artifact_integrity(
            tenant_id,
            SecurityContext::system(),
            valid.page_id,
            AuditPageArtifactsInput {
                max_records: Some(2),
            },
        )
        .await?;
    assert_eq!(complete.format, PAGE_ARTIFACT_INTEGRITY_AUDIT_FORMAT);
    assert_eq!(complete.page_id, valid.page_id);
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
            valid.page_id,
            AuditPageArtifactsInput {
                max_records: Some(1),
            },
        )
        .await?;
    assert_eq!(bounded.scanned_artifact_count, 1);
    assert_eq!(bounded.valid_artifact_count, 1);
    assert_eq!(bounded.invalid_artifact_count, 0);
    assert!(bounded.truncated);
    assert!(!bounded.findings_truncated);
    assert_eq!(bounded.audit_hash.len(), 64);

    assert_shared_scan_locks_block_artifact_update(
        &database,
        tenant_id,
        valid.page_id,
        valid.source.artifact_id,
    )
    .await?;

    let corrupt = publish_fixture(&service, &db, tenant_id, "corrupt").await?;
    let artifact = page_static_landing_artifact::Entity::find_by_id(corrupt.source.artifact_id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("canonical artifact is missing"))?;
    let mut active: page_static_landing_artifact::ActiveModel = artifact.into();
    active.document_html = Set("<main>corrupted immutable payload</main>".to_string());
    active.update(&db).await?;

    let corrupt_result = service
        .audit_immutable_artifact_integrity(
            tenant_id,
            SecurityContext::system(),
            corrupt.page_id,
            AuditPageArtifactsInput {
                max_records: Some(1),
            },
        )
        .await?;
    assert_eq!(corrupt_result.scanned_artifact_count, 1);
    assert_eq!(corrupt_result.valid_artifact_count, 0);
    assert_eq!(corrupt_result.invalid_artifact_count, 1);
    assert_eq!(corrupt_result.findings.len(), 1);
    assert_eq!(
        corrupt_result.findings[0].artifact_id,
        corrupt.source.artifact_id
    );
    assert_eq!(
        corrupt_result.findings[0].code,
        PAGE_ARTIFACT_INTEGRITY_INVALID
    );
    assert_eq!(corrupt_result.findings[0].locale_hash.len(), 64);
    assert_eq!(corrupt_result.findings[0].record_identity_hash.len(), 64);
    assert_eq!(corrupt_result.findings[0].diagnostic_hash.len(), 64);
    assert_eq!(corrupt_result.audit_hash.len(), 64);

    let partial = publish_fixture(&service, &db, tenant_id, "partial").await?;
    let artifact = page_static_landing_artifact::Entity::find_by_id(partial.source.artifact_id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("materialized artifact is missing"))?;
    assert!(artifact.materialization_hash.is_some());
    assert!(artifact.materialization_identity.is_some());
    assert!(artifact.runtime_snapshots.is_some());
    let mut active: page_static_landing_artifact::ActiveModel = artifact.into();
    active.runtime_snapshots = Set(None);
    active.update(&db).await?;

    let partial_result = service
        .audit_immutable_artifact_integrity(
            tenant_id,
            SecurityContext::system(),
            partial.page_id,
            AuditPageArtifactsInput {
                max_records: Some(1),
            },
        )
        .await?;
    assert_eq!(partial_result.scanned_artifact_count, 1);
    assert_eq!(partial_result.valid_artifact_count, 0);
    assert_eq!(partial_result.invalid_artifact_count, 1);
    assert_eq!(partial_result.findings.len(), 1);
    assert_eq!(
        partial_result.findings[0].code,
        PAGE_ARTIFACT_INTEGRITY_INVALID
    );
    assert_eq!(partial_result.findings[0].locale_hash.len(), 64);
    assert_eq!(partial_result.findings[0].record_identity_hash.len(), 64);
    assert_eq!(partial_result.findings[0].diagnostic_hash.len(), 64);
    assert_eq!(partial_result.audit_hash.len(), 64);

    database.cleanup().await
}

async fn assert_shared_scan_locks_block_artifact_update(
    database: &TestDatabase,
    tenant_id: Uuid,
    page_id: Uuid,
    artifact_id: Uuid,
) -> TestResult<()> {
    let locker_db = database.connection().await?;
    let updater_db = database.connection().await?;
    let locker = locker_db.begin().await?;

    let locked_page = page::Entity::find_by_id(page_id)
        .filter(page::Column::TenantId.eq(tenant_id))
        .lock_shared()
        .one(&locker)
        .await?;
    assert!(locked_page.is_some());

    let selected_ids = page_static_landing_artifact::Entity::find()
        .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
        .filter(page_static_landing_artifact::Column::PageId.eq(page_id))
        .select_only()
        .column(page_static_landing_artifact::Column::Id)
        .order_by_asc(page_static_landing_artifact::Column::CreatedAt)
        .order_by_asc(page_static_landing_artifact::Column::Id)
        .limit(2)
        .lock_shared()
        .into_tuple::<Uuid>()
        .all(&locker)
        .await?;
    assert!(selected_ids.contains(&artifact_id));

    let locked_artifact = page_static_landing_artifact::Entity::find_by_id(artifact_id)
        .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
        .filter(page_static_landing_artifact::Column::PageId.eq(page_id))
        .lock_shared()
        .one(&locker)
        .await?;
    let before_document = locked_artifact
        .ok_or_else(|| std::io::Error::other("artifact disappeared before lock assertion"))?
        .document_html;

    let updater = updater_db.begin().await?;
    updater
        .execute_unprepared("SET LOCAL lock_timeout = '100ms'")
        .await?;
    let update_error = updater
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE page_static_landing_artifacts SET document_html = $1 WHERE id = $2",
            vec![
                "<main>must stay blocked by shared audit lock</main>".into(),
                artifact_id.into(),
            ],
        ))
        .await
        .expect_err("concurrent artifact update must be blocked by shared scan locks");
    assert!(
        update_error
            .to_string()
            .to_ascii_lowercase()
            .contains("lock timeout")
    );
    updater.rollback().await?;
    locker.commit().await?;

    let stored = page_static_landing_artifact::Entity::find_by_id(artifact_id)
        .one(&locker_db)
        .await?
        .ok_or_else(|| std::io::Error::other("artifact disappeared after lock assertion"))?;
    assert_eq!(stored.document_html, before_document);
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
    label: &str,
) -> TestResult<PublishedFixture> {
    let reviewed = PageBuilderReviewedPublishRuntime::new(
        format!("immutable-artifact-audit-postgres-{label}"),
        json!({ "surface": "storefront", "channel": "web", "fixture": label }),
    )?;
    let slug = format!("audit-{label}");
    let project = json!({
        "pages": [{
            "id": slug.clone(),
            "flyPageMeta": {
                "title": format!("Immutable artifact audit {label}"),
                "description": "PostgreSQL audit regression",
                "slug": slug.clone()
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": format!("Immutable artifact audit {label}")
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
                    title: format!("Immutable artifact audit {label}"),
                    slug: Some(slug),
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
                idempotency_key: format!("artifact-audit-postgres-publish-{label}-v1"),
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
         VALUES ($1, $2, 'pages', TRUE, '{}'::jsonb, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        vec![Uuid::new_v4().into(), tenant_id.into()],
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
