use std::env;
use std::error::Error;
use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxModule, OutboxTransport, SysEvents, TransactionalEventBus};
use rustok_page_builder::PageBuilderReviewedPublishRuntime;
use rustok_pages::PagesModule;
use rustok_pages::dto::{
    CreatePageInput, PageBodyInput, PageBodyRevisionInput, PageTranslationInput, PublishPageInput,
    RebuildPageArtifactInput, ReviewedPagePublishRuntimeInput,
};
use rustok_pages::entities::{
    page, page_artifact_rebuild_operation, page_publish_operation_artifact,
    page_publish_rebuild_source, page_published_landing_artifact, page_static_landing_artifact,
};
use rustok_pages::services::PageService;
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
    async fn setup() -> TestResult<Option<Self>> {
        let Some(database_url) = database_url() else {
            eprintln!(
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Pages artifact-loss rebuild harness"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_pages_artifact_loss_rebuild_{}",
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

#[tokio::test]
async fn explicit_rebuild_reproduces_missing_source_artifact_from_retained_provenance_on_postgres()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let tenant_id = Uuid::new_v4();
    enable_pages_module(&db, tenant_id).await?;
    let service = PageService::new(
        db.clone(),
        TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone()))),
    );
    let reviewed = PageBuilderReviewedPublishRuntime::new(
        "artifact-loss-rebuild-postgres",
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
                "title": "Artifact loss rebuild",
                "description": "Rebuild from retained provenance after source artifact loss",
                "slug": "home"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": "Artifact loss rebuild"
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
                    title: "Artifact loss rebuild".to_string(),
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
                idempotency_key: "artifact-loss-rebuild-publish-v1".to_string(),
                runtime: reviewed_input(),
            },
        )
        .await?;
    assert!(!published.replayed);

    let source = page_publish_rebuild_source::Entity::find()
        .filter(page_publish_rebuild_source::Column::OperationId.eq(published.operation_id))
        .filter(page_publish_rebuild_source::Column::Locale.eq("en"))
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("publish rebuild source is missing"))?;
    let retained_source = source.clone();
    let canonical_artifact = page_static_landing_artifact::Entity::find_by_id(source.artifact_id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("canonical artifact is missing before loss"))?;
    let page_before_loss = page::Entity::find_by_id(draft.id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("published page is missing before loss"))?;
    let events_before_loss = SysEvents::find().count(&db).await?;

    let removed_bindings = page_published_landing_artifact::Entity::delete_many()
        .filter(page_published_landing_artifact::Column::TenantId.eq(tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(draft.id))
        .filter(page_published_landing_artifact::Column::ArtifactId.eq(source.artifact_id))
        .exec(&db)
        .await?;
    assert_eq!(removed_bindings.rows_affected, 1);
    let removed_manifest = page_publish_operation_artifact::Entity::delete_many()
        .filter(page_publish_operation_artifact::Column::OperationId.eq(source.operation_id))
        .filter(page_publish_operation_artifact::Column::ArtifactId.eq(source.artifact_id))
        .exec(&db)
        .await?;
    assert_eq!(removed_manifest.rows_affected, 1);
    let deleted_artifact = page_static_landing_artifact::Entity::delete_by_id(source.artifact_id)
        .exec(&db)
        .await?;
    assert_eq!(deleted_artifact.rows_affected, 1);
    assert!(
        page_static_landing_artifact::Entity::find_by_id(source.artifact_id)
            .one(&db)
            .await?
            .is_none()
    );
    assert_eq!(
        page_publish_rebuild_source::Entity::find_by_id(source.id)
            .one(&db)
            .await?
            .ok_or_else(|| std::io::Error::other("provenance disappeared after artifact loss"))?,
        retained_source
    );
    assert_eq!(
        page_published_landing_artifact::Entity::find()
            .filter(page_published_landing_artifact::Column::TenantId.eq(tenant_id))
            .filter(page_published_landing_artifact::Column::PageId.eq(draft.id))
            .count(&db)
            .await?,
        0
    );
    assert_eq!(SysEvents::find().count(&db).await?, events_before_loss);
    let page_after_loss = page::Entity::find_by_id(draft.id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("page disappeared after artifact loss"))?;
    assert_eq!(page_after_loss.version, page_before_loss.version);
    assert_eq!(page_after_loss.status, page_before_loss.status);

    let artifact_count_before_rebuild = page_static_landing_artifact::Entity::find()
        .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
        .filter(page_static_landing_artifact::Column::PageId.eq(draft.id))
        .count(&db)
        .await?;
    let receipt_count_before_rebuild = page_artifact_rebuild_operation::Entity::find()
        .filter(page_artifact_rebuild_operation::Column::TenantId.eq(tenant_id))
        .filter(page_artifact_rebuild_operation::Column::PageId.eq(draft.id))
        .count(&db)
        .await?;
    let events_before_rebuild = SysEvents::find().count(&db).await?;
    let rebuild_input = RebuildPageArtifactInput {
        source_id: source.id,
        expected_provenance_hash: source.provenance_hash.clone(),
        idempotency_key: "artifact-loss-rebuild-v1".to_string(),
        runtime: reviewed_input(),
    };
    let rebuilt = service
        .rebuild_immutable_artifact(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            rebuild_input.clone(),
        )
        .await?;
    assert!(!rebuilt.replayed);
    assert_eq!(rebuilt.source_id, source.id);
    assert_eq!(rebuilt.source_publish_operation_id, source.operation_id);
    assert_eq!(rebuilt.source_artifact_id, source.artifact_id);
    assert_ne!(rebuilt.rebuilt_artifact_id, source.artifact_id);
    assert_eq!(rebuilt.artifact_hash, source.artifact_hash);
    assert_eq!(rebuilt.materialization_hash, source.materialization_hash);
    assert_eq!(
        rebuilt.artifact_instance_key,
        format!("rebuild:{}", rebuilt.operation_id)
    );

    let rebuilt_record =
        page_static_landing_artifact::Entity::find_by_id(rebuilt.rebuilt_artifact_id)
            .one(&db)
            .await?
            .ok_or_else(|| std::io::Error::other("rebuilt artifact is missing"))?;
    let mut expected_rebuilt = canonical_artifact.clone();
    expected_rebuilt.id = rebuilt_record.id;
    expected_rebuilt.instance_key = rebuilt_record.instance_key.clone();
    expected_rebuilt.created_at = rebuilt_record.created_at;
    assert_eq!(rebuilt_record, expected_rebuilt);

    let receipt = page_artifact_rebuild_operation::Entity::find_by_id(rebuilt.operation_id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("rebuild receipt is missing"))?;
    assert_eq!(receipt.source_id, source.id);
    assert_eq!(receipt.source_artifact_id, source.artifact_id);
    assert_eq!(receipt.rebuilt_artifact_id, rebuilt.rebuilt_artifact_id);
    assert_eq!(
        page_static_landing_artifact::Entity::find()
            .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
            .filter(page_static_landing_artifact::Column::PageId.eq(draft.id))
            .count(&db)
            .await?,
        artifact_count_before_rebuild + 1
    );
    assert_eq!(
        page_artifact_rebuild_operation::Entity::find()
            .filter(page_artifact_rebuild_operation::Column::TenantId.eq(tenant_id))
            .filter(page_artifact_rebuild_operation::Column::PageId.eq(draft.id))
            .count(&db)
            .await?,
        receipt_count_before_rebuild + 1
    );
    assert_eq!(SysEvents::find().count(&db).await?, events_before_rebuild);
    assert_eq!(
        page_publish_rebuild_source::Entity::find_by_id(source.id)
            .one(&db)
            .await?
            .ok_or_else(|| std::io::Error::other("provenance changed during rebuild"))?,
        retained_source
    );
    assert_eq!(
        page_published_landing_artifact::Entity::find()
            .filter(page_published_landing_artifact::Column::TenantId.eq(tenant_id))
            .filter(page_published_landing_artifact::Column::PageId.eq(draft.id))
            .count(&db)
            .await?,
        0
    );
    let page_after_rebuild = page::Entity::find_by_id(draft.id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("page disappeared after rebuild"))?;
    assert_eq!(page_after_rebuild.version, page_before_loss.version);
    assert_eq!(page_after_rebuild.status, page_before_loss.status);

    let replay = service
        .rebuild_immutable_artifact(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            rebuild_input,
        )
        .await?;
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, rebuilt.operation_id);
    assert_eq!(replay.rebuilt_artifact_id, rebuilt.rebuilt_artifact_id);
    assert_eq!(
        page_static_landing_artifact::Entity::find()
            .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
            .filter(page_static_landing_artifact::Column::PageId.eq(draft.id))
            .count(&db)
            .await?,
        artifact_count_before_rebuild + 1
    );
    assert_eq!(
        page_artifact_rebuild_operation::Entity::find()
            .filter(page_artifact_rebuild_operation::Column::TenantId.eq(tenant_id))
            .filter(page_artifact_rebuild_operation::Column::PageId.eq(draft.id))
            .count(&db)
            .await?,
        receipt_count_before_rebuild + 1
    );
    assert_eq!(SysEvents::find().count(&db).await?, events_before_rebuild);

    database.cleanup().await
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
