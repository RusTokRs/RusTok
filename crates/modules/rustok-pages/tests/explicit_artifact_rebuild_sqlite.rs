use std::error::Error;
use std::sync::Arc;

use chrono::{Duration as ChronoDuration, Utc};
use rustok_channel::ChannelModule;
use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_page_builder::PageBuilderReviewedPublishRuntime;
use rustok_pages::PagesModule;
use rustok_pages::dto::{
    CreatePageInput, PageBodyInput, PageBodyRevisionInput, PageTranslationInput, PublishPageInput,
    RebuildPageArtifactInput, ReviewedPagePublishRuntimeInput,
};
use rustok_pages::entities::{
    page, page_artifact_rebuild_operation, page_body, page_publish_rebuild_source,
    page_published_landing_artifact, page_static_landing_artifact,
};
use rustok_pages::services::PageService;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection,
    DbBackend, EntityTrait, PaginatorTrait, QueryFilter, Set, Statement,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use serde_json::json;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test]
async fn explicit_rebuild_appends_exact_artifact_without_switching_public_binding() -> TestResult<()>
{
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let service = PageService::new(db.clone(), event_bus);
    let reviewed = PageBuilderReviewedPublishRuntime::new(
        "explicit-artifact-rebuild",
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
                "title": "Explicit rebuild source",
                "description": "Append-only immutable rebuild regression",
                "slug": "home"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": "Explicit rebuild source"
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
                    title: "Explicit rebuild source".to_string(),
                    slug: Some("home".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    document: project.clone(),
                }),
                channel_slugs: Some(vec!["web".to_string()]),
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
    let publish = service
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
                idempotency_key: "explicit-rebuild-publish-v1".to_string(),
                runtime: reviewed_input(),
            },
        )
        .await?;
    assert!(!publish.replayed);

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
        .ok_or_else(|| std::io::Error::other("published artifact is missing"))?;
    assert_eq!(original_artifact.instance_key, "canonical");
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
        .ok_or_else(|| std::io::Error::other("page is missing"))?;
    let artifact_count_before = page_static_landing_artifact::Entity::find()
        .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
        .filter(page_static_landing_artifact::Column::PageId.eq(draft.id))
        .count(&db)
        .await?;

    let body = page_body::Entity::find_by_id(source.page_body_id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("source body is missing"))?;
    let mut body_active: page_body::ActiveModel = body.into();
    body_active.content = Set(serde_json::to_string(&json!({
        "pages": [{
            "id": "home",
            "component": {
                "id": "changed",
                "type": "wrapper",
                "content": "Mutable draft must not become rebuild authority"
            }
        }]
    }))?);
    body_active.updated_at = Set((Utc::now() + ChronoDuration::seconds(1)).into());
    body_active.update(&db).await?;

    let mut corrupted_active: page_static_landing_artifact::ActiveModel =
        original_artifact.clone().into();
    corrupted_active.document_html = Set("<main>corrupted retained artifact</main>".to_string());
    corrupted_active.update(&db).await?;

    let rebuild_input = RebuildPageArtifactInput {
        source_id: source.id,
        expected_provenance_hash: source.provenance_hash.clone(),
        idempotency_key: "explicit-artifact-rebuild-v1".to_string(),
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
    assert_eq!(rebuilt_record.instance_key, rebuilt.artifact_instance_key);
    assert_eq!(rebuilt_record.artifact_hash, source.artifact_hash);
    assert_eq!(
        rebuilt_record.materialization_hash,
        Some(source.materialization_hash.clone())
    );
    assert_ne!(
        rebuilt_record.document_html,
        "<main>corrupted retained artifact</main>"
    );
    assert!(
        rebuilt_record
            .document_html
            .contains("Explicit rebuild source")
    );

    let binding_after =
        page_published_landing_artifact::Entity::find_by_id(binding_before.page_body_id)
            .one(&db)
            .await?
            .ok_or_else(|| std::io::Error::other("published binding disappeared"))?;
    assert_eq!(binding_after.artifact_id, binding_before.artifact_id);
    let page_after = page::Entity::find_by_id(draft.id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("page disappeared"))?;
    assert_eq!(page_after.version, page_before.version);
    assert_eq!(
        page_static_landing_artifact::Entity::find()
            .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
            .filter(page_static_landing_artifact::Column::PageId.eq(draft.id))
            .count(&db)
            .await?,
        artifact_count_before + 1
    );

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
        page_artifact_rebuild_operation::Entity::find()
            .filter(page_artifact_rebuild_operation::Column::TenantId.eq(tenant_id))
            .filter(page_artifact_rebuild_operation::Column::PageId.eq(draft.id))
            .count(&db)
            .await?,
        1
    );
    assert_eq!(
        page_static_landing_artifact::Entity::find()
            .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
            .filter(page_static_landing_artifact::Column::PageId.eq(draft.id))
            .count(&db)
            .await?,
        artifact_count_before + 1
    );
    Ok(())
}

async fn setup_db(tenant_id: Uuid) -> TestResult<DatabaseConnection> {
    let database_url = format!(
        "sqlite:file:pages_explicit_artifact_rebuild_{}?mode=memory&cache=shared",
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
