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
    page, page_body, page_publish_operation_artifact, page_publish_rebuild_source,
    page_published_landing_artifact, page_static_landing_artifact,
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
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Pages repair rollback continuity harness"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_pages_repair_rollback_{}_{}",
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

struct TwoPublishFixture {
    tenant_id: Uuid,
    page_id: Uuid,
    first_publish_operation_id: Uuid,
    first_artifact_id: Uuid,
    second_publish_operation_id: Uuid,
    second_publish_version: i32,
    second_source: page_publish_rebuild_source::Model,
    reviewed: PageBuilderReviewedPublishRuntime,
}

#[tokio::test]
async fn rollback_continues_after_physical_loss_rebuild_and_activation_on_postgres()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup("success").await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let service = page_service(&db);
    let fixture = create_two_publish_fixture(&db, &service, "repair-rollback-success").await?;

    remove_current_source_artifact(&db, &fixture).await?;
    let rebuild = service
        .rebuild_immutable_artifact(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            RebuildPageArtifactInput {
                source_id: fixture.second_source.id,
                expected_provenance_hash: fixture.second_source.provenance_hash.clone(),
                idempotency_key: "repair-rollback-rebuild-v1".to_string(),
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
                expected_version: fixture.second_publish_version,
                expected_current_artifact_id: fixture.second_source.artifact_id,
                idempotency_key: "repair-rollback-activate-v1".to_string(),
            },
        )
        .await?;
    assert_eq!(
        activation.replacement_artifact_id,
        rebuild.rebuilt_artifact_id
    );
    assert_eq!(
        page_publish_operation_artifact::Entity::find()
            .filter(
                page_publish_operation_artifact::Column::OperationId
                    .eq(fixture.second_publish_operation_id),
            )
            .count(&db)
            .await?,
        0
    );

    let rollback_input = RollbackPageInput {
        expected_version: activation.version,
        idempotency_key: "repair-rollback-v1".to_string(),
    };
    let rollback = service
        .rollback_to_previous(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            rollback_input.clone(),
        )
        .await?;
    assert!(!rollback.replayed);
    assert_eq!(
        rollback.target_publish_operation_id,
        fixture.first_publish_operation_id
    );
    assert_eq!(rollback.version, activation.version + 1);

    let binding = page_published_landing_artifact::Entity::find()
        .filter(page_published_landing_artifact::Column::TenantId.eq(fixture.tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(fixture.page_id))
        .filter(page_published_landing_artifact::Column::Locale.eq("en"))
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("rollback binding is missing"))?;
    assert_eq!(binding.artifact_id, fixture.first_artifact_id);
    assert!(
        page_static_landing_artifact::Entity::find_by_id(fixture.second_source.artifact_id)
            .one(&db)
            .await?
            .is_none()
    );

    let replay = service
        .rollback_to_previous(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            rollback_input,
        )
        .await?;
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, rollback.operation_id);
    assert_eq!(replay.version, rollback.version);

    database.cleanup().await
}

#[tokio::test]
async fn historical_target_still_requires_original_manifest_on_postgres() -> TestResult<()> {
    let Some(database) = TestDatabase::setup("historical_target").await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let service = page_service(&db);
    let fixture = create_two_publish_fixture(&db, &service, "repair-rollback-target").await?;

    let removed = page_publish_operation_artifact::Entity::delete_many()
        .filter(
            page_publish_operation_artifact::Column::OperationId
                .eq(fixture.first_publish_operation_id),
        )
        .exec(&db)
        .await?;
    assert_eq!(removed.rows_affected, 1);
    assert!(
        page_static_landing_artifact::Entity::find_by_id(fixture.first_artifact_id)
            .one(&db)
            .await?
            .is_some()
    );

    let before = page::Entity::find_by_id(fixture.page_id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("page is missing before rollback rejection"))?;
    let result = service
        .rollback_to_previous(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            RollbackPageInput {
                expected_version: fixture.second_publish_version,
                idempotency_key: "historical-target-missing-manifest-v1".to_string(),
            },
        )
        .await;
    assert!(matches!(
        result,
        Err(PagesError::RollbackTargetUnavailable(_))
    ));
    let after = page::Entity::find_by_id(fixture.page_id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("page disappeared after rollback rejection"))?;
    assert_eq!(after.version, before.version);
    let binding = page_published_landing_artifact::Entity::find()
        .filter(page_published_landing_artifact::Column::TenantId.eq(fixture.tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(fixture.page_id))
        .filter(page_published_landing_artifact::Column::Locale.eq("en"))
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("current binding disappeared after rejection"))?;
    assert_ne!(binding.artifact_id, fixture.first_artifact_id);

    database.cleanup().await
}

#[tokio::test]
async fn surviving_manifest_identity_mismatch_is_not_healed_by_repair_on_postgres() -> TestResult<()>
{
    let Some(database) = TestDatabase::setup("manifest_mismatch").await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let service = page_service(&db);
    let fixture =
        create_two_publish_fixture(&db, &service, "repair-rollback-manifest-mismatch").await?;

    let rebuild = service
        .rebuild_immutable_artifact(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            RebuildPageArtifactInput {
                source_id: fixture.second_source.id,
                expected_provenance_hash: fixture.second_source.provenance_hash.clone(),
                idempotency_key: "manifest-mismatch-rebuild-v1".to_string(),
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
                expected_version: fixture.second_publish_version,
                expected_current_artifact_id: fixture.second_source.artifact_id,
                idempotency_key: "manifest-mismatch-activate-v1".to_string(),
            },
        )
        .await?;

    let tampered_hash = "f".repeat(64);
    let changed = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE page_publish_operation_artifacts SET artifact_hash = $1 WHERE operation_id = $2",
            vec![
                tampered_hash.into(),
                fixture.second_publish_operation_id.into(),
            ],
        ))
        .await?;
    assert_eq!(changed.rows_affected(), 1);

    let before = page::Entity::find_by_id(fixture.page_id)
        .one(&db)
        .await?
        .ok_or_else(|| {
            std::io::Error::other("page is missing before manifest mismatch rollback")
        })?;
    let result = service
        .rollback_to_previous(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            RollbackPageInput {
                expected_version: activation.version,
                idempotency_key: "manifest-mismatch-rollback-v1".to_string(),
            },
        )
        .await;
    assert!(matches!(
        result,
        Err(PagesError::RollbackTargetUnavailable(_))
    ));
    let after = page::Entity::find_by_id(fixture.page_id)
        .one(&db)
        .await?
        .ok_or_else(|| {
            std::io::Error::other("page disappeared after manifest mismatch rejection")
        })?;
    assert_eq!(after.version, before.version);
    let binding = page_published_landing_artifact::Entity::find()
        .filter(page_published_landing_artifact::Column::TenantId.eq(fixture.tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(fixture.page_id))
        .filter(page_published_landing_artifact::Column::Locale.eq("en"))
        .one(&db)
        .await?
        .ok_or_else(|| {
            std::io::Error::other("repaired binding disappeared after manifest mismatch")
        })?;
    assert_eq!(binding.artifact_id, rebuild.rebuilt_artifact_id);

    database.cleanup().await
}

#[tokio::test]
async fn missing_current_manifest_is_not_healed_while_source_artifact_still_exists_on_postgres()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup("manifest_missing_source_live").await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let service = page_service(&db);
    let fixture = create_two_publish_fixture(
        &db,
        &service,
        "repair-rollback-manifest-missing-source-live",
    )
    .await?;

    let rebuild = service
        .rebuild_immutable_artifact(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            RebuildPageArtifactInput {
                source_id: fixture.second_source.id,
                expected_provenance_hash: fixture.second_source.provenance_hash.clone(),
                idempotency_key: "manifest-missing-source-live-rebuild-v1".to_string(),
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
                expected_version: fixture.second_publish_version,
                expected_current_artifact_id: fixture.second_source.artifact_id,
                idempotency_key: "manifest-missing-source-live-activate-v1".to_string(),
            },
        )
        .await?;

    let removed = page_publish_operation_artifact::Entity::delete_many()
        .filter(
            page_publish_operation_artifact::Column::OperationId
                .eq(fixture.second_publish_operation_id),
        )
        .exec(&db)
        .await?;
    assert_eq!(removed.rows_affected, 1);
    assert!(
        page_static_landing_artifact::Entity::find_by_id(fixture.second_source.artifact_id)
            .one(&db)
            .await?
            .is_some()
    );

    let before = page::Entity::find_by_id(fixture.page_id)
        .one(&db)
        .await?
        .ok_or_else(|| {
            std::io::Error::other("page is missing before live-source manifest rejection")
        })?;
    let result = service
        .rollback_to_previous(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            RollbackPageInput {
                expected_version: activation.version,
                idempotency_key: "manifest-missing-source-live-rollback-v1".to_string(),
            },
        )
        .await;
    assert!(matches!(
        result,
        Err(PagesError::RollbackTargetUnavailable(_))
    ));
    let after = page::Entity::find_by_id(fixture.page_id)
        .one(&db)
        .await?
        .ok_or_else(|| {
            std::io::Error::other("page disappeared after live-source manifest rejection")
        })?;
    assert_eq!(after.version, before.version);
    let binding = page_published_landing_artifact::Entity::find()
        .filter(page_published_landing_artifact::Column::TenantId.eq(fixture.tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(fixture.page_id))
        .filter(page_published_landing_artifact::Column::Locale.eq("en"))
        .one(&db)
        .await?
        .ok_or_else(|| {
            std::io::Error::other(
                "repaired binding disappeared after live-source manifest rejection",
            )
        })?;
    assert_eq!(binding.artifact_id, rebuild.rebuilt_artifact_id);

    database.cleanup().await
}

async fn create_two_publish_fixture(
    db: &DatabaseConnection,
    service: &PageService,
    scenario: &str,
) -> TestResult<TwoPublishFixture> {
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
                    title: "Repair rollback continuity".to_string(),
                    slug: Some("home".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    document: project_json("Publish A")?,
                }),
                channel_slugs: None,
                publish: false,
            },
        )
        .await?;
    let first_body_revision = draft
        .body
        .as_ref()
        .ok_or_else(|| std::io::Error::other("first draft body is missing"))?
        .updated_at
        .clone();
    let first_publish = service
        .publish_reviewed(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            PublishPageInput {
                expected_version: draft.version,
                expected_body_revisions: vec![PageBodyRevisionInput {
                    locale: "en".to_string(),
                    revision: first_body_revision,
                }],
                idempotency_key: format!("{scenario}-publish-a-v1"),
                runtime: reviewed_input(&reviewed),
            },
        )
        .await?;
    let first_source = page_publish_rebuild_source::Entity::find()
        .filter(page_publish_rebuild_source::Column::OperationId.eq(first_publish.operation_id))
        .filter(page_publish_rebuild_source::Column::Locale.eq("en"))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("first publish provenance is missing"))?;

    let unpublished = service
        .unpublish_if_current(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            Some(first_publish.version),
        )
        .await?;
    let current_body = page_body::Entity::find()
        .filter(page_body::Column::TenantId.eq(tenant_id))
        .filter(page_body::Column::PageId.eq(draft.id))
        .filter(page_body::Column::Locale.eq("en"))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("body is missing before second publish"))?;
    service
        .save_document(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            SavePageDocumentInput {
                expected_revision: current_body.updated_at.to_string(),
                body: PageBodyInput {
                    locale: "en".to_string(),
                    document: project_json("Publish B")?,
                },
            },
        )
        .await?;
    let second_body = page_body::Entity::find()
        .filter(page_body::Column::TenantId.eq(tenant_id))
        .filter(page_body::Column::PageId.eq(draft.id))
        .filter(page_body::Column::Locale.eq("en"))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("body is missing after second document save"))?;
    let second_publish = service
        .publish_reviewed(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            PublishPageInput {
                expected_version: unpublished.version,
                expected_body_revisions: vec![PageBodyRevisionInput {
                    locale: "en".to_string(),
                    revision: second_body.updated_at.to_string(),
                }],
                idempotency_key: format!("{scenario}-publish-b-v1"),
                runtime: reviewed_input(&reviewed),
            },
        )
        .await?;
    let second_source = page_publish_rebuild_source::Entity::find()
        .filter(page_publish_rebuild_source::Column::OperationId.eq(second_publish.operation_id))
        .filter(page_publish_rebuild_source::Column::Locale.eq("en"))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("second publish provenance is missing"))?;

    Ok(TwoPublishFixture {
        tenant_id,
        page_id: draft.id,
        first_publish_operation_id: first_publish.operation_id,
        first_artifact_id: first_source.artifact_id,
        second_publish_operation_id: second_publish.operation_id,
        second_publish_version: second_publish.version,
        second_source,
        reviewed,
    })
}

async fn remove_current_source_artifact(
    db: &DatabaseConnection,
    fixture: &TwoPublishFixture,
) -> TestResult<()> {
    let removed_binding = page_published_landing_artifact::Entity::delete_many()
        .filter(page_published_landing_artifact::Column::TenantId.eq(fixture.tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(fixture.page_id))
        .filter(
            page_published_landing_artifact::Column::ArtifactId
                .eq(fixture.second_source.artifact_id),
        )
        .exec(db)
        .await?;
    assert_eq!(removed_binding.rows_affected, 1);
    let removed_manifest = page_publish_operation_artifact::Entity::delete_many()
        .filter(
            page_publish_operation_artifact::Column::OperationId
                .eq(fixture.second_publish_operation_id),
        )
        .filter(
            page_publish_operation_artifact::Column::ArtifactId
                .eq(fixture.second_source.artifact_id),
        )
        .exec(db)
        .await?;
    assert_eq!(removed_manifest.rows_affected, 1);
    let deleted =
        page_static_landing_artifact::Entity::delete_by_id(fixture.second_source.artifact_id)
            .exec(db)
            .await?;
    assert_eq!(deleted.rows_affected, 1);
    Ok(())
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
                "description": "Repair rollback continuity",
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
