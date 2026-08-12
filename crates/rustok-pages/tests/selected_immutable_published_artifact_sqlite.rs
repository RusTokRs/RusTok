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
    ReviewedPagePublishRuntimeInput,
};
use rustok_pages::entities::{
    page_body, page_published_landing_artifact, page_static_landing_artifact,
};
use rustok_pages::services::{PageBuilderArtifactService, PageService};
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
    body_id: Uuid,
    artifact_id: Uuid,
    artifact_hash: String,
    document_html: String,
}

#[tokio::test]
async fn storefront_reads_selected_immutable_artifact_after_persisted_draft_mutation()
-> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let fixture = create_reviewed_published_page(&db, event_bus, tenant_id).await?;
    let artifacts = PageBuilderArtifactService::new(db.clone());

    let exact_before = artifacts
        .load_public_bound_artifact_with_fallback(
            tenant_id,
            fixture.page_id,
            "en",
            None,
            Some("web"),
        )
        .await?
        .ok_or_else(|| std::io::Error::other("published artifact is missing before draft edit"))?;
    let fallback_before = artifacts
        .load_public_bound_artifact_with_fallback(
            tenant_id,
            fixture.page_id,
            "fr",
            Some("en"),
            Some("web"),
        )
        .await?
        .ok_or_else(|| std::io::Error::other("published fallback artifact is missing"))?;

    assert_eq!(exact_before.artifact_hash, fixture.artifact_hash);
    assert_eq!(exact_before.document_html, fixture.document_html);
    assert_eq!(fallback_before.artifact_hash, fixture.artifact_hash);
    assert_eq!(fallback_before.locale, "en");
    assert!(
        exact_before
            .document_html
            .contains("Published immutable artifact")
    );
    assert!(!exact_before.document_html.contains("Draft-only mutation"));

    persist_new_draft_body(&db, tenant_id, &fixture).await?;

    let persisted_body = page_body::Entity::find_by_id(fixture.body_id)
        .filter(page_body::Column::TenantId.eq(tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("mutated page body disappeared"))?;
    assert!(persisted_body.content.contains("Draft-only mutation"));

    let binding = page_published_landing_artifact::Entity::find_by_id(fixture.body_id)
        .filter(page_published_landing_artifact::Column::TenantId.eq(tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(fixture.page_id))
        .filter(page_published_landing_artifact::Column::Locale.eq("en"))
        .one(&db)
        .await?
        .ok_or_else(|| std::io::Error::other("published artifact binding disappeared"))?;
    assert_eq!(binding.artifact_id, fixture.artifact_id);

    let exact_after = artifacts
        .load_public_bound_artifact_with_fallback(
            tenant_id,
            fixture.page_id,
            "en",
            None,
            Some("web"),
        )
        .await?
        .ok_or_else(|| std::io::Error::other("published artifact is missing after draft edit"))?;
    let fallback_after = artifacts
        .load_public_bound_artifact_with_fallback(
            tenant_id,
            fixture.page_id,
            "fr",
            Some("en"),
            Some("web"),
        )
        .await?
        .ok_or_else(|| std::io::Error::other("published fallback disappeared after draft edit"))?;

    assert_eq!(exact_after.artifact_hash, exact_before.artifact_hash);
    assert_eq!(exact_after.document_html, exact_before.document_html);
    assert_eq!(fallback_after.artifact_hash, fallback_before.artifact_hash);
    assert_eq!(fallback_after.document_html, fallback_before.document_html);
    assert!(
        exact_after
            .document_html
            .contains("Published immutable artifact")
    );
    assert!(!exact_after.document_html.contains("Draft-only mutation"));
    assert!(!fallback_after.document_html.contains("Draft-only mutation"));

    Ok(())
}

async fn persist_new_draft_body(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    fixture: &PublishedFixture,
) -> TestResult<()> {
    let body = page_body::Entity::find_by_id(fixture.body_id)
        .filter(page_body::Column::TenantId.eq(tenant_id))
        .filter(page_body::Column::PageId.eq(fixture.page_id))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("published page body is missing"))?;
    let mut active: page_body::ActiveModel = body.into();
    active.content = Set(serde_json::to_string(&json!({
        "pages": [{
            "id": "home",
            "flyPageMeta": {
                "title": "Draft-only mutation",
                "description": "This persisted draft must not replace the selected artifact",
                "slug": "home"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "draft-heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": "Draft-only mutation"
                }]
            }
        }]
    }))?);
    active.updated_at = Set((Utc::now() + ChronoDuration::seconds(1)).into());
    active.update(db).await?;
    Ok(())
}

async fn create_reviewed_published_page(
    db: &DatabaseConnection,
    event_bus: TransactionalEventBus,
    tenant_id: Uuid,
) -> TestResult<PublishedFixture> {
    let project = json!({
        "pages": [{
            "id": "home",
            "flyPageMeta": {
                "title": "Published immutable artifact",
                "description": "Selected immutable artifact owner regression",
                "slug": "home"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "published-heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": "Published immutable artifact"
                }]
            }
        }]
    });
    let service = PageService::new(db.clone(), event_bus);
    let draft = service
        .create(
            tenant_id,
            SecurityContext::system(),
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Published immutable artifact".to_string(),
                    slug: Some("home".to_string()),
                    meta_title: Some("Published immutable artifact".to_string()),
                    meta_description: Some(
                        "Selected immutable artifact owner regression".to_string(),
                    ),
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
    let body_response = draft
        .body
        .as_ref()
        .ok_or_else(|| std::io::Error::other("reviewed draft is missing its body"))?;
    let body_revision = body_response.updated_at.clone();
    let body = page_body::Entity::find()
        .filter(page_body::Column::TenantId.eq(tenant_id))
        .filter(page_body::Column::PageId.eq(draft.id))
        .filter(page_body::Column::Locale.eq("en"))
        .one(db)
        .await?
        .ok_or_else(|| {
            std::io::Error::other("reviewed draft body is missing from owner storage")
        })?;
    let body_id = body.id;
    let reviewed = PageBuilderReviewedPublishRuntime::new(
        "selected-immutable-artifact",
        json!({ "surface": "storefront", "channel": "web" }),
    )?;
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
                idempotency_key: "selected-immutable-artifact-v1".to_string(),
                runtime: ReviewedPagePublishRuntimeInput {
                    format: reviewed.format,
                    scenario_id: reviewed.scenario_id,
                    context: reviewed.context,
                    review_hash: reviewed.review_hash,
                },
            },
        )
        .await?;
    assert_eq!(publish.page_id, draft.id);
    assert!(!publish.replayed);

    let artifact = page_static_landing_artifact::Entity::find()
        .filter(page_static_landing_artifact::Column::TenantId.eq(tenant_id))
        .filter(page_static_landing_artifact::Column::PageId.eq(draft.id))
        .filter(page_static_landing_artifact::Column::Locale.eq("en"))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("reviewed publish did not retain an artifact"))?;
    assert!(artifact.materialization_hash.is_some());
    assert!(artifact.materialization_identity.is_some());
    assert!(artifact.runtime_snapshots.is_some());

    let binding = page_published_landing_artifact::Entity::find_by_id(body_id)
        .filter(page_published_landing_artifact::Column::TenantId.eq(tenant_id))
        .filter(page_published_landing_artifact::Column::PageId.eq(draft.id))
        .filter(page_published_landing_artifact::Column::Locale.eq("en"))
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("reviewed publish did not retain a binding"))?;
    assert_eq!(binding.artifact_id, artifact.id);

    Ok(PublishedFixture {
        page_id: draft.id,
        body_id,
        artifact_id: artifact.id,
        artifact_hash: artifact.artifact_hash,
        document_html: artifact.document_html,
    })
}

async fn setup_db(tenant_id: Uuid) -> TestResult<DatabaseConnection> {
    let database_url = format!(
        "sqlite:file:pages_selected_immutable_artifact_{}?mode=memory&cache=shared",
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
