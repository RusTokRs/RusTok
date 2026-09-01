use std::env;
use std::error::Error;
use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxModule, OutboxTransport, SysEvents, TransactionalEventBus};
use rustok_page_builder::PageBuilderReviewedPublishRuntime;
use rustok_pages::dto::{
    CreatePageInput, PageBodyInput, PageBodyRevisionInput, PageTranslationInput, PublishPageInput,
    RebuildPageArtifactInput, ReplacePageArtifactBindingInput, ReviewedPagePublishRuntimeInput,
};
use rustok_pages::entities::{
    page, page_artifact_binding_replacement_operation, page_artifact_rebuild_operation,
    page_publish_operation_artifact, page_publish_rebuild_source, page_published_landing_artifact,
    page_static_landing_artifact,
};
use rustok_pages::services::PageService;
use rustok_pages::{PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT, PagesError, PagesModule};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection,
    DbBackend, EntityTrait, PaginatorTrait, QueryFilter, Set, Statement,
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
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Pages artifact-loss activation recovery harness"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_pages_artifact_loss_activation_{}_{}",
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

struct PublishedFixture {
    tenant_id: Uuid,
    page_id: Uuid,
    page_version: i32,
    source: page_publish_rebuild_source::Model,
    reviewed: PageBuilderReviewedPublishRuntime,
}

#[tokio::test]
async fn missing_binding_activation_recovers_after_physical_source_artifact_loss_on_postgres()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup("success").await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let service = page_service(&db);
    let fixture =
        create_published_fixture(&db, &service, "artifact-loss-activation-success").await?;
    let source_before = fixture.source.clone();

    remove_binding_manifest_and_source_artifact(
        &db,
        fixture.tenant_id,
        fixture.page_id,
        &fixture.source,
    )
    .await?;
    let rebuild =
        rebuild_from_fixture(&service, &fixture, "artifact-loss-activation-rebuild-v1").await?;
    let rebuild_record = page_artifact_rebuild_operation::Entity::find_by_id(rebuild.operation_id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("rebuild receipt is missing"))?;
    let events_before_activation = SysEvents::find().count(&db).await?;

    let input = ReplacePageArtifactBindingInput {
        rebuild_operation_id: rebuild.operation_id,
        expected_version: fixture.page_version,
        expected_current_artifact_id: fixture.source.artifact_id,
        idempotency_key: "artifact-loss-activation-v1".to_string(),
    };
    let activated = service
        .replace_rebuilt_artifact_binding(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            input.clone(),
        )
        .await?;
    assert!(!activated.replayed);
    assert_eq!(activated.previous_artifact_id, fixture.source.artifact_id);
    assert_eq!(
        activated.replacement_artifact_id,
        rebuild.rebuilt_artifact_id
    );
    assert_eq!(activated.version, fixture.page_version + 1);

    let binding = page_published_landing_artifact::Entity::find()
        .filter(page_published_landing_artifact::Column::TenantId.eq(fixture.tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(fixture.page_id))
        .filter(page_published_landing_artifact::Column::Locale.eq("en"))
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("recovered binding is missing"))?;
    assert_eq!(binding.page_body_id, fixture.source.page_body_id);
    assert_eq!(binding.artifact_id, rebuild.rebuilt_artifact_id);
    assert!(
        page_static_landing_artifact::Entity::find_by_id(fixture.source.artifact_id)
            .one(&db)
            .await?
            .is_none()
    );
    assert_eq!(
        page_publish_rebuild_source::Entity::find_by_id(fixture.source.id)
            .one(&db)
            .await?
            .ok_or_else(|| std::io::Error::other("retained provenance disappeared"))?,
        source_before
    );
    assert_eq!(
        page_artifact_rebuild_operation::Entity::find_by_id(rebuild.operation_id)
            .one(&db)
            .await?
            .ok_or_else(|| std::io::Error::other("rebuild receipt disappeared"))?,
        rebuild_record
    );
    assert_eq!(
        page_artifact_binding_replacement_operation::Entity::find()
            .filter(
                page_artifact_binding_replacement_operation::Column::TenantId
                    .eq(fixture.tenant_id),
            )
            .filter(
                page_artifact_binding_replacement_operation::Column::PageId.eq(fixture.page_id),
            )
            .count(&db)
            .await?,
        1
    );
    assert_eq!(
        SysEvents::find().count(&db).await?,
        events_before_activation + 2
    );
    let page_after = page::Entity::find_by_id(fixture.page_id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("page disappeared after activation"))?;
    assert_eq!(page_after.version, fixture.page_version + 1);
    assert_eq!(page_after.status, "published");

    let replay = service
        .replace_rebuilt_artifact_binding(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            input,
        )
        .await?;
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, activated.operation_id);
    assert_eq!(replay.version, activated.version);
    assert_eq!(
        SysEvents::find().count(&db).await?,
        events_before_activation + 2
    );
    assert_eq!(
        page_artifact_binding_replacement_operation::Entity::find()
            .filter(
                page_artifact_binding_replacement_operation::Column::TenantId
                    .eq(fixture.tenant_id),
            )
            .filter(
                page_artifact_binding_replacement_operation::Column::PageId.eq(fixture.page_id),
            )
            .count(&db)
            .await?,
        1
    );

    database.cleanup().await
}

#[tokio::test]
async fn missing_binding_activation_rejects_when_source_artifact_still_exists_on_postgres()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup("source_present").await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let service = page_service(&db);
    let fixture =
        create_published_fixture(&db, &service, "artifact-loss-activation-source-present").await?;
    let rebuild = rebuild_from_fixture(&service, &fixture, "source-present-rebuild-v1").await?;

    let removed = page_published_landing_artifact::Entity::delete_many()
        .filter(page_published_landing_artifact::Column::TenantId.eq(fixture.tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(fixture.page_id))
        .filter(page_published_landing_artifact::Column::Locale.eq("en"))
        .exec(&db)
        .await?;
    assert_eq!(removed.rows_affected, 1);
    assert!(
        page_static_landing_artifact::Entity::find_by_id(fixture.source.artifact_id)
            .one(&db)
            .await?
            .is_some()
    );
    let events_before = SysEvents::find().count(&db).await?;

    let result = service
        .replace_rebuilt_artifact_binding(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            ReplacePageArtifactBindingInput {
                rebuild_operation_id: rebuild.operation_id,
                expected_version: fixture.page_version,
                expected_current_artifact_id: fixture.source.artifact_id,
                idempotency_key: "source-present-activation-v1".to_string(),
            },
        )
        .await;
    assert!(matches!(
        result,
        Err(PagesError::RollbackTargetUnavailable(message))
            if message.contains(PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT)
                && message.contains("source artifact still exists")
    ));
    assert_eq!(SysEvents::find().count(&db).await?, events_before);
    assert_eq!(
        page_artifact_binding_replacement_operation::Entity::find()
            .filter(
                page_artifact_binding_replacement_operation::Column::TenantId
                    .eq(fixture.tenant_id),
            )
            .filter(
                page_artifact_binding_replacement_operation::Column::PageId.eq(fixture.page_id),
            )
            .count(&db)
            .await?,
        0
    );
    assert_eq!(
        page_published_landing_artifact::Entity::find()
            .filter(page_published_landing_artifact::Column::TenantId.eq(fixture.tenant_id))
            .filter(page_published_landing_artifact::Column::PageId.eq(fixture.page_id))
            .count(&db)
            .await?,
        0
    );

    database.cleanup().await
}

#[tokio::test]
async fn missing_binding_activation_rejects_stale_source_publish_version_on_postgres()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup("stale_publish").await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let service = page_service(&db);
    let fixture =
        create_published_fixture(&db, &service, "artifact-loss-activation-stale-publish").await?;

    remove_binding_manifest_and_source_artifact(
        &db,
        fixture.tenant_id,
        fixture.page_id,
        &fixture.source,
    )
    .await?;
    let rebuild = rebuild_from_fixture(&service, &fixture, "stale-publish-rebuild-v1").await?;

    let current = page::Entity::find_by_id(fixture.page_id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("page is missing before stale-version fixture"))?;
    let mut advanced: page::ActiveModel = current.into();
    advanced.version = Set(fixture.page_version + 1);
    advanced.update(&db).await?;
    let events_before = SysEvents::find().count(&db).await?;

    let result = service
        .replace_rebuilt_artifact_binding(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            ReplacePageArtifactBindingInput {
                rebuild_operation_id: rebuild.operation_id,
                expected_version: fixture.page_version + 1,
                expected_current_artifact_id: fixture.source.artifact_id,
                idempotency_key: "stale-publish-activation-v1".to_string(),
            },
        )
        .await;
    assert!(matches!(
        result,
        Err(PagesError::RollbackTargetUnavailable(message))
            if message.contains(PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT)
                && message.contains("not fully explained")
    ));
    assert_eq!(SysEvents::find().count(&db).await?, events_before);
    assert_eq!(
        page_artifact_binding_replacement_operation::Entity::find()
            .filter(
                page_artifact_binding_replacement_operation::Column::TenantId
                    .eq(fixture.tenant_id),
            )
            .filter(
                page_artifact_binding_replacement_operation::Column::PageId.eq(fixture.page_id),
            )
            .count(&db)
            .await?,
        0
    );
    assert_eq!(
        page_published_landing_artifact::Entity::find()
            .filter(page_published_landing_artifact::Column::TenantId.eq(fixture.tenant_id))
            .filter(page_published_landing_artifact::Column::PageId.eq(fixture.page_id))
            .count(&db)
            .await?,
        0
    );

    database.cleanup().await
}

fn page_service(db: &DatabaseConnection) -> PageService {
    PageService::new(
        db.clone(),
        TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone()))),
    )
}

async fn create_published_fixture(
    db: &DatabaseConnection,
    service: &PageService,
    scenario: &str,
) -> TestResult<PublishedFixture> {
    let tenant_id = Uuid::new_v4();
    enable_pages_module(db, tenant_id).await?;
    let reviewed = PageBuilderReviewedPublishRuntime::new(
        scenario,
        json!({ "surface": "storefront", "channel": "web" }),
    )?;
    let project = json!({
        "pages": [{
            "id": "home",
            "flyPageMeta": {
                "title": "Artifact loss activation recovery",
                "description": "Explicit recovery after immutable source loss",
                "slug": "home"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": "Artifact loss activation recovery"
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
                    title: "Artifact loss activation recovery".to_string(),
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
    let body_revision = draft
        .body
        .as_ref()
        .ok_or_else(|| std::io::Error::other("draft body is missing"))?
        .updated_at
        .clone();
    let published = service
        .publish_reviewed(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            PublishPageInput {
                expected_version: draft.version,
                expected_body_revisions: vec![PageBodyRevisionInput {
                    locale: "en".to_string(),
                    revision: body_revision,
                }],
                idempotency_key: format!("{scenario}-publish-v1"),
                runtime: reviewed_input(&reviewed),
            },
        )
        .await?;
    assert!(!published.replayed);
    let source = page_publish_rebuild_source::Entity::find()
        .filter(page_publish_rebuild_source::Column::OperationId.eq(published.operation_id))
        .filter(page_publish_rebuild_source::Column::Locale.eq("en"))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("publish rebuild source is missing"))?;
    let current = page::Entity::find_by_id(draft.id)
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("published page is missing"))?;
    Ok(PublishedFixture {
        tenant_id,
        page_id: draft.id,
        page_version: current.version,
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

async fn rebuild_from_fixture(
    service: &PageService,
    fixture: &PublishedFixture,
    idempotency_key: &str,
) -> TestResult<rustok_pages::dto::RebuildPageArtifactResult> {
    Ok(service
        .rebuild_immutable_artifact(
            fixture.tenant_id,
            SecurityContext::system(),
            fixture.page_id,
            RebuildPageArtifactInput {
                source_id: fixture.source.id,
                expected_provenance_hash: fixture.source.provenance_hash.clone(),
                idempotency_key: idempotency_key.to_string(),
                runtime: reviewed_input(&fixture.reviewed),
            },
        )
        .await?)
}

async fn remove_binding_manifest_and_source_artifact(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    page_id: Uuid,
    source: &page_publish_rebuild_source::Model,
) -> TestResult<()> {
    let removed_bindings = page_published_landing_artifact::Entity::delete_many()
        .filter(page_published_landing_artifact::Column::TenantId.eq(tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(page_id))
        .filter(page_published_landing_artifact::Column::ArtifactId.eq(source.artifact_id))
        .exec(db)
        .await?;
    assert_eq!(removed_bindings.rows_affected, 1);
    let removed_manifest = page_publish_operation_artifact::Entity::delete_many()
        .filter(page_publish_operation_artifact::Column::OperationId.eq(source.operation_id))
        .filter(page_publish_operation_artifact::Column::ArtifactId.eq(source.artifact_id))
        .exec(db)
        .await?;
    assert_eq!(removed_manifest.rows_affected, 1);
    let deleted_artifact = page_static_landing_artifact::Entity::delete_by_id(source.artifact_id)
        .exec(db)
        .await?;
    assert_eq!(deleted_artifact.rows_affected, 1);
    assert!(
        page_static_landing_artifact::Entity::find_by_id(source.artifact_id)
            .one(db)
            .await?
            .is_none()
    );
    assert_eq!(
        page_published_landing_artifact::Entity::find()
            .filter(page_published_landing_artifact::Column::TenantId.eq(tenant_id))
            .filter(page_published_landing_artifact::Column::PageId.eq(page_id))
            .count(db)
            .await?,
        0
    );
    Ok(())
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
