use std::env;
use std::error::Error;
use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_page_builder::PageBuilderReviewedPublishRuntime;
use rustok_pages::dto::{
    CreatePageInput, PageBodyInput, PageBodyRevisionInput, PageTranslationInput, PublishPageInput,
    RebuildPageArtifactInput, ReplacePageArtifactBindingInput, ReviewedPagePublishRuntimeInput,
    RollbackPageInput, SavePageDocumentInput,
};
use rustok_pages::entities::{
    page, page_artifact_binding_replacement_operation, page_body, page_publish_operation_artifact,
    page_publish_rebuild_source, page_published_landing_artifact, page_rollback_operation,
    page_static_landing_artifact,
};
use rustok_pages::services::PageService;
use rustok_pages::{PagesError, PagesModule};
use sea_orm::{
    ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend,
    EntityTrait, PaginatorTrait, QueryFilter, Statement,
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
    async fn setup(label: &str) -> TestResult<Option<Self>> {
        let Some(database_url) = database_url() else {
            eprintln!(
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Pages rollback-activated repair-to-rollback continuity harness"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_pages_rollback_anchor_repair_rollback_{}_{}",
            label,
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

struct Fixture {
    tenant_id: Uuid,
    page_id: Uuid,
    oldest_publish_operation_id: Uuid,
    oldest_artifact_id: Uuid,
    repaired_publish_operation_id: Uuid,
    repaired_source: page_publish_rebuild_source::Model,
    activation_anchor_operation_id: Uuid,
    activation_anchor_version: i32,
    reviewed: PageBuilderReviewedPublishRuntime,
}

struct RepairedCurrent {
    rebuilt_artifact_id: Uuid,
    current_version: i32,
}

#[tokio::test]
async fn rollback_continues_after_rollback_activated_physical_loss_repair_on_postgres()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup("success").await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let service = page_service(&db);
    let fixture = create_fixture(&db, &service, "rollback-anchor-repair-rollback-success").await?;
    let repaired = repair_rollback_activated_publish(&db, &service, &fixture, "success").await?;

    let input = RollbackPageInput {
        expected_version: repaired.current_version,
        idempotency_key: "rollback-anchor-repair-rollback-v1".to_string(),
    };
    let rollback = service
        .rollback_to_previous(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            input.clone(),
        )
        .await?;
    assert!(!rollback.replayed);
    assert_eq!(
        rollback.target_publish_operation_id,
        fixture.oldest_publish_operation_id
    );
    assert_eq!(rollback.version, repaired.current_version + 1);

    let binding = current_binding(&db, &fixture).await?;
    assert_eq!(binding.artifact_id, fixture.oldest_artifact_id);

    let replay = service
        .rollback_to_previous(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            input,
        )
        .await?;
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, rollback.operation_id);
    assert_eq!(replay.version, rollback.version);

    database.cleanup().await
}

#[tokio::test]
async fn rollback_rejects_repaired_cursor_when_rollback_activation_anchor_hash_is_corrupted_on_postgres()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup("anchor_hash").await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let service = page_service(&db);
    let fixture = create_fixture(&db, &service, "rollback-anchor-repair-rollback-hash").await?;
    let repaired = repair_rollback_activated_publish(&db, &service, &fixture, "hash").await?;

    let anchor =
        page_rollback_operation::Entity::find_by_id(fixture.activation_anchor_operation_id)
            .one(&db)
            .await?
            .ok_or_else(|| std::io::Error::other("rollback activation anchor is missing"))?;
    let changed = db
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE page_rollback_operations SET request_hash = $1 WHERE id = $2",
            vec![
                different_sha256(&anchor.request_hash).into(),
                anchor.id.into(),
            ],
        ))
        .await?;
    assert_eq!(changed.rows_affected(), 1);

    let before_page = page::Entity::find_by_id(fixture.page_id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("page is missing before rollback rejection"))?;
    assert_eq!(before_page.version, repaired.current_version);
    let before_rollbacks = page_rollback_operation::Entity::find()
        .filter(page_rollback_operation::Column::TenantId.eq(fixture.tenant_id))
        .filter(page_rollback_operation::Column::PageId.eq(fixture.page_id))
        .count(&db)
        .await?;

    let result = service
        .rollback_to_previous(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            RollbackPageInput {
                expected_version: repaired.current_version,
                idempotency_key: "rollback-anchor-corrupt-final-rollback-v1".to_string(),
            },
        )
        .await;
    assert!(matches!(
        result,
        Err(PagesError::RollbackTargetUnavailable(_))
    ));

    let after_page = page::Entity::find_by_id(fixture.page_id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("page disappeared after rollback rejection"))?;
    assert_eq!(after_page.version, before_page.version);
    assert_eq!(
        page_rollback_operation::Entity::find()
            .filter(page_rollback_operation::Column::TenantId.eq(fixture.tenant_id))
            .filter(page_rollback_operation::Column::PageId.eq(fixture.page_id))
            .count(&db)
            .await?,
        before_rollbacks
    );
    assert_eq!(
        current_binding(&db, &fixture).await?.artifact_id,
        repaired.rebuilt_artifact_id
    );

    database.cleanup().await
}

async fn repair_rollback_activated_publish(
    db: &DatabaseConnection,
    service: &PageService,
    fixture: &Fixture,
    key: &str,
) -> TestResult<RepairedCurrent> {
    remove_source_artifact(db, &fixture.repaired_source).await?;

    let rebuild = service
        .rebuild_immutable_artifact(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            RebuildPageArtifactInput {
                source_id: fixture.repaired_source.id,
                expected_provenance_hash: fixture.repaired_source.provenance_hash.clone(),
                idempotency_key: format!("{key}-rollback-anchor-rebuild-v1"),
                runtime: reviewed_input(&fixture.reviewed),
            },
        )
        .await?;
    let activation = service
        .replace_rebuilt_artifact_binding(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            ReplacePageArtifactBindingInput {
                rebuild_operation_id: rebuild.operation_id,
                expected_version: fixture.activation_anchor_version,
                expected_current_artifact_id: fixture.repaired_source.artifact_id,
                idempotency_key: format!("{key}-rollback-anchor-activate-v1"),
            },
        )
        .await?;
    assert_eq!(activation.version, fixture.activation_anchor_version + 1);
    assert_eq!(
        page_artifact_binding_replacement_operation::Entity::find()
            .filter(
                page_artifact_binding_replacement_operation::Column::TenantId.eq(fixture.tenant_id)
            )
            .filter(page_artifact_binding_replacement_operation::Column::PageId.eq(fixture.page_id))
            .count(db)
            .await?,
        1
    );
    assert_eq!(
        page_publish_operation_artifact::Entity::find()
            .filter(
                page_publish_operation_artifact::Column::OperationId
                    .eq(fixture.repaired_publish_operation_id),
            )
            .count(db)
            .await?,
        0
    );

    Ok(RepairedCurrent {
        rebuilt_artifact_id: rebuild.rebuilt_artifact_id,
        current_version: activation.version,
    })
}

async fn create_fixture(
    db: &DatabaseConnection,
    service: &PageService,
    scenario: &str,
) -> TestResult<Fixture> {
    let tenant_id = Uuid::new_v4();
    enable_pages_module(db, tenant_id).await?;
    let reviewed = PageBuilderReviewedPublishRuntime::new(
        scenario,
        json!({ "surface": "storefront", "channel": "web" }),
    )?;

    let draft = service
        .create(
            tenant_id,
            SecurityContext::system(),
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Rollback continuity P0".to_string(),
                    slug: Some("home".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    document: project_json("Rollback continuity P0")?,
                }),
                channel_slugs: None,
                publish: false,
            },
        )
        .await?;
    let oldest_revision = draft
        .body
        .as_ref()
        .ok_or_else(|| std::io::Error::other("oldest draft body is missing"))?
        .updated_at
        .clone();
    let oldest = service
        .publish_reviewed(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            PublishPageInput {
                expected_version: draft.version,
                expected_body_revisions: vec![PageBodyRevisionInput {
                    locale: "en".to_string(),
                    revision: oldest_revision,
                }],
                idempotency_key: format!("{scenario}-publish-p0-v1"),
                runtime: reviewed_input(&reviewed),
            },
        )
        .await?;
    let oldest_source = publish_source(db, oldest.operation_id).await?;

    let unpublished_oldest = service
        .unpublish_if_current(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            Some(oldest.version),
        )
        .await?;
    replace_document(db, service, tenant_id, draft.id, "Rollback continuity P1").await?;
    let middle_body = body_for_locale(db, tenant_id, draft.id).await?;
    let middle = service
        .publish_reviewed(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            PublishPageInput {
                expected_version: unpublished_oldest.version,
                expected_body_revisions: vec![PageBodyRevisionInput {
                    locale: "en".to_string(),
                    revision: middle_body.updated_at.to_string(),
                }],
                idempotency_key: format!("{scenario}-publish-p1-v1"),
                runtime: reviewed_input(&reviewed),
            },
        )
        .await?;
    let middle_source = publish_source(db, middle.operation_id).await?;

    let unpublished_middle = service
        .unpublish_if_current(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            Some(middle.version),
        )
        .await?;
    replace_document(db, service, tenant_id, draft.id, "Rollback continuity P2").await?;
    let newest_body = body_for_locale(db, tenant_id, draft.id).await?;
    let newest = service
        .publish_reviewed(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            PublishPageInput {
                expected_version: unpublished_middle.version,
                expected_body_revisions: vec![PageBodyRevisionInput {
                    locale: "en".to_string(),
                    revision: newest_body.updated_at.to_string(),
                }],
                idempotency_key: format!("{scenario}-publish-p2-v1"),
                runtime: reviewed_input(&reviewed),
            },
        )
        .await?;

    let activation_anchor = service
        .rollback_to_previous(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            RollbackPageInput {
                expected_version: newest.version,
                idempotency_key: format!("{scenario}-rollback-p2-to-p1-v1"),
            },
        )
        .await?;
    assert_eq!(
        activation_anchor.target_publish_operation_id,
        middle.operation_id
    );
    assert_eq!(activation_anchor.version, newest.version + 1);
    assert_eq!(
        current_binding_raw(db, tenant_id, draft.id)
            .await?
            .artifact_id,
        middle_source.artifact_id
    );

    Ok(Fixture {
        tenant_id,
        page_id: draft.id,
        oldest_publish_operation_id: oldest.operation_id,
        oldest_artifact_id: oldest_source.artifact_id,
        repaired_publish_operation_id: middle.operation_id,
        repaired_source: middle_source,
        activation_anchor_operation_id: activation_anchor.operation_id,
        activation_anchor_version: activation_anchor.version,
        reviewed,
    })
}

async fn replace_document(
    db: &DatabaseConnection,
    service: &PageService,
    tenant_id: Uuid,
    page_id: Uuid,
    title: &str,
) -> TestResult<()> {
    let current = body_for_locale(db, tenant_id, page_id).await?;
    service
        .save_document(
            tenant_id,
            SecurityContext::system(),
            page_id,
            SavePageDocumentInput {
                expected_revision: current.updated_at.to_string(),
                body: PageBodyInput {
                    locale: "en".to_string(),
                    document: project_json(title)?,
                },
            },
        )
        .await?;
    Ok(())
}

async fn publish_source(
    db: &DatabaseConnection,
    operation_id: Uuid,
) -> TestResult<page_publish_rebuild_source::Model> {
    Ok(page_publish_rebuild_source::Entity::find()
        .filter(page_publish_rebuild_source::Column::OperationId.eq(operation_id))
        .filter(page_publish_rebuild_source::Column::Locale.eq("en"))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("publish rebuild source is missing"))?)
}

async fn body_for_locale(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    page_id: Uuid,
) -> TestResult<page_body::Model> {
    Ok(page_body::Entity::find()
        .filter(page_body::Column::TenantId.eq(tenant_id))
        .filter(page_body::Column::PageId.eq(page_id))
        .filter(page_body::Column::Locale.eq("en"))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("English page body is missing"))?)
}

async fn remove_source_artifact(
    db: &DatabaseConnection,
    source: &page_publish_rebuild_source::Model,
) -> TestResult<()> {
    let removed_binding = page_published_landing_artifact::Entity::delete_many()
        .filter(page_published_landing_artifact::Column::TenantId.eq(source.tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(source.page_id))
        .filter(page_published_landing_artifact::Column::Locale.eq(&source.locale))
        .filter(page_published_landing_artifact::Column::ArtifactId.eq(source.artifact_id))
        .exec(db)
        .await?;
    assert_eq!(removed_binding.rows_affected, 1);
    let removed_manifest = page_publish_operation_artifact::Entity::delete_many()
        .filter(page_publish_operation_artifact::Column::OperationId.eq(source.operation_id))
        .filter(page_publish_operation_artifact::Column::Locale.eq(&source.locale))
        .filter(page_publish_operation_artifact::Column::ArtifactId.eq(source.artifact_id))
        .exec(db)
        .await?;
    assert_eq!(removed_manifest.rows_affected, 1);
    let deleted = page_static_landing_artifact::Entity::delete_by_id(source.artifact_id)
        .exec(db)
        .await?;
    assert_eq!(deleted.rows_affected, 1);
    Ok(())
}

async fn current_binding(
    db: &DatabaseConnection,
    fixture: &Fixture,
) -> TestResult<page_published_landing_artifact::Model> {
    current_binding_raw(db, fixture.tenant_id, fixture.page_id).await
}

async fn current_binding_raw(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    page_id: Uuid,
) -> TestResult<page_published_landing_artifact::Model> {
    Ok(page_published_landing_artifact::Entity::find()
        .filter(page_published_landing_artifact::Column::TenantId.eq(tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(page_id))
        .filter(page_published_landing_artifact::Column::Locale.eq("en"))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("current English binding is missing"))?)
}

fn reviewed_input(reviewed: &PageBuilderReviewedPublishRuntime) -> ReviewedPagePublishRuntimeInput {
    ReviewedPagePublishRuntimeInput {
        format: reviewed.format.clone(),
        scenario_id: reviewed.scenario_id.clone(),
        context: reviewed.context.clone(),
        review_hash: reviewed.review_hash.clone(),
    }
}

fn project_json(title: &str) -> TestResult<serde_json::Value> {
    Ok(json!({
        "pages": [{
            "id": "home",
            "flyPageMeta": {
                "title": title,
                "description": "Rollback-activated repair-to-rollback continuity",
                "slug": "home"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": title
                }]
            }
        }]
    }))
}

fn different_sha256(value: &str) -> String {
    if value.starts_with('f') {
        "e".repeat(64)
    } else {
        "f".repeat(64)
    }
}

fn page_service(db: &DatabaseConnection) -> PageService {
    PageService::new(
        db.clone(),
        TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone()))),
    )
}

async fn enable_pages_module(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<()> {
    db.execute_unprepared(
        "CREATE TABLE IF NOT EXISTS tenant_modules (\
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
    db.execute_raw(Statement::from_sql_and_values(
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
