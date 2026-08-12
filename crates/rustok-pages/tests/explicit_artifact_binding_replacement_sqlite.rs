use std::error::Error;
use std::sync::Arc;

use rustok_channel::ChannelModule;
use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_page_builder::PageBuilderReviewedPublishRuntime;
use rustok_pages::dto::{
    CreatePageInput, PageBodyInput, PageBodyRevisionInput, PageTranslationInput, PublishPageInput,
    RebuildPageArtifactInput, ReplacePageArtifactBindingInput, ReviewedPagePublishRuntimeInput,
};
use rustok_pages::entities::{
    page, page_artifact_binding_replacement_operation, page_publish_rebuild_source,
    page_published_landing_artifact, page_static_landing_artifact,
};
use rustok_pages::services::{PageBuilderArtifactService, PageService};
use rustok_pages::{PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT, PagesError, PagesModule};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection,
    DbBackend, EntityTrait, PaginatorTrait, QueryFilter, Set, Statement,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use serde_json::json;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test]
async fn explicit_binding_replacement_switches_exact_rebuild_and_replays() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let service = PageService::new(db.clone(), event_bus);
    let reviewed = PageBuilderReviewedPublishRuntime::new(
        "explicit-artifact-binding-replacement",
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
                "title": "Explicit binding replacement",
                "description": "Immutable binding replacement regression",
                "slug": "home"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": "Explicit binding replacement"
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
                    title: "Explicit binding replacement".to_string(),
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
    service
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
                idempotency_key: "binding-replacement-publish-v1".to_string(),
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
        .ok_or_else(|| std::io::Error::other("published artifact is missing"))?;
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

    let mut corrupted: page_static_landing_artifact::ActiveModel = original_artifact.clone().into();
    corrupted.document_html = Set("<main>corrupted current artifact</main>".to_string());
    corrupted.update(&db).await?;

    let rebuild = service
        .rebuild_immutable_artifact(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            RebuildPageArtifactInput {
                source_id: source.id,
                expected_provenance_hash: source.provenance_hash.clone(),
                idempotency_key: "binding-replacement-rebuild-v1".to_string(),
                runtime: reviewed_input(),
            },
        )
        .await?;
    assert_eq!(rebuild.source_artifact_id, original_artifact.id);
    assert_ne!(rebuild.rebuilt_artifact_id, original_artifact.id);

    let stale = service
        .replace_rebuilt_artifact_binding(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            ReplacePageArtifactBindingInput {
                rebuild_operation_id: rebuild.operation_id,
                expected_version: page_before.version,
                expected_current_artifact_id: Uuid::new_v4(),
                idempotency_key: "binding-replacement-stale-v1".to_string(),
            },
        )
        .await;
    assert!(matches!(
        stale,
        Err(PagesError::RollbackTargetUnavailable(message))
            if message.contains(PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT)
    ));
    assert_eq!(
        page_artifact_binding_replacement_operation::Entity::find()
            .filter(page_artifact_binding_replacement_operation::Column::TenantId.eq(tenant_id),)
            .filter(page_artifact_binding_replacement_operation::Column::PageId.eq(draft.id))
            .count(&db)
            .await?,
        0
    );
    let binding_after_stale =
        page_published_landing_artifact::Entity::find_by_id(binding_before.page_body_id)
            .one(&db)
            .await?
            .ok_or_else(|| std::io::Error::other("binding disappeared after stale request"))?;
    assert_eq!(binding_after_stale.artifact_id, original_artifact.id);

    let replace_input = ReplacePageArtifactBindingInput {
        rebuild_operation_id: rebuild.operation_id,
        expected_version: page_before.version,
        expected_current_artifact_id: original_artifact.id,
        idempotency_key: "binding-replacement-activate-v1".to_string(),
    };
    let replaced = service
        .replace_rebuilt_artifact_binding(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            replace_input.clone(),
        )
        .await?;
    assert!(!replaced.replayed);
    assert_eq!(replaced.rebuild_operation_id, rebuild.operation_id);
    assert_eq!(replaced.previous_artifact_id, original_artifact.id);
    assert_eq!(
        replaced.replacement_artifact_id,
        rebuild.rebuilt_artifact_id
    );
    assert_eq!(replaced.version, page_before.version + 1);

    let binding_after =
        page_published_landing_artifact::Entity::find_by_id(binding_before.page_body_id)
            .one(&db)
            .await?
            .ok_or_else(|| std::io::Error::other("binding disappeared after replacement"))?;
    assert_eq!(binding_after.artifact_id, rebuild.rebuilt_artifact_id);
    let page_after = page::Entity::find_by_id(draft.id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("page disappeared after replacement"))?;
    assert_eq!(page_after.version, page_before.version + 1);
    assert_eq!(page_after.status, "published");

    let corrupted_after = page_static_landing_artifact::Entity::find_by_id(original_artifact.id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("source artifact was deleted"))?;
    assert_eq!(
        corrupted_after.document_html,
        "<main>corrupted current artifact</main>"
    );

    let public = PageBuilderArtifactService::new(db.clone())
        .load_public_bound_artifact_with_fallback(tenant_id, draft.id, "en", None, Some("web"))
        .await?
        .ok_or_else(|| std::io::Error::other("replacement artifact is not public"))?;
    assert_eq!(public.artifact_hash, source.artifact_hash);
    assert!(
        public
            .document_html
            .contains("Explicit binding replacement")
    );
    assert!(!public.document_html.contains("corrupted current artifact"));

    let replay = service
        .replace_rebuilt_artifact_binding(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            replace_input,
        )
        .await?;
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, replaced.operation_id);
    assert_eq!(replay.version, replaced.version);
    assert_eq!(
        page_artifact_binding_replacement_operation::Entity::find()
            .filter(page_artifact_binding_replacement_operation::Column::TenantId.eq(tenant_id),)
            .filter(page_artifact_binding_replacement_operation::Column::PageId.eq(draft.id))
            .count(&db)
            .await?,
        1
    );
    let page_after_replay = page::Entity::find_by_id(draft.id)
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("page disappeared after replay"))?;
    assert_eq!(page_after_replay.version, replaced.version);
    Ok(())
}

async fn setup_db(tenant_id: Uuid) -> TestResult<DatabaseConnection> {
    let database_url = format!(
        "sqlite:file:pages_explicit_artifact_binding_replacement_{}?mode=memory&cache=shared",
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
