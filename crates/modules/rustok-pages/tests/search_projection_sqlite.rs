//! The public search document of a page is built from its published landing, read through the
//! landing binding. The page body (the authoring document) and landings that a later publish
//! superseded never reach the index, and a published page without a landing is not indexed.
//! The projection indexes a landing under the locale of its binding; the schema refuses a binding
//! to a landing of another page locale.

use std::error::Error;

use chrono::Utc;
use rustok_core::MigrationSource;
use rustok_core::search_projection::{SearchProjectionDocument, SearchProjectionSource};
use rustok_page_builder::PAGE_BUILDER_DOCUMENT_FORMAT;
use rustok_page_builder::static_landing::StaticLandingCompiler;
use rustok_pages::entities::{
    page, page_body, page_published_landing_artifact, page_static_landing_artifact,
    page_translation,
};
use rustok_pages::{PageSearchProjectionSource, PagesModule};
use sea_orm::{ActiveModelTrait, ConnectOptions, Database, DatabaseConnection, Set};
use sea_orm_migration::SchemaManager;
use serde_json::{Value, json};
use uuid::Uuid;

const DOCUMENT_ONLY_TEXT: &str = "Authoring document words";
const SUPERSEDED_TEXT: &str = "Superseded landing headline";
const CURRENT_TEXT: &str = "Current landing headline";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test]
async fn the_bound_landing_is_indexed_not_a_superseded_one_and_not_the_document() -> TestResult<()>
{
    let db = setup_db().await?;
    let tenant_id = Uuid::new_v4();
    let page_id = insert_published_page(&db, tenant_id, &["en"]).await?;
    let body_id = insert_body(&db, tenant_id, page_id, "en", DOCUMENT_ONLY_TEXT).await?;
    // Two landings of the same page locale are stored; only the second one is published.
    insert_artifact(&db, tenant_id, page_id, "en", SUPERSEDED_TEXT).await?;
    let current = insert_artifact(&db, tenant_id, page_id, "en", CURRENT_TEXT).await?;
    bind_landing(&db, tenant_id, page_id, "en", body_id, current).await?;

    let documents = load_page_documents(&db, tenant_id, page_id).await?;

    assert_eq!(documents.len(), 1);
    let document = &documents[0];
    assert_eq!(document.locale, "en");
    assert!(
        document.body.contains(CURRENT_TEXT),
        "the published landing text must be indexed, got {:?}",
        document.body
    );
    assert!(!document.body.contains(SUPERSEDED_TEXT));
    assert!(!document.body.contains(DOCUMENT_ONLY_TEXT));
    assert!(
        !document.body.contains('<'),
        "the indexed body must be text, not markup: {:?}",
        document.body
    );
    Ok(())
}

#[tokio::test]
async fn a_published_page_without_a_published_landing_is_not_indexed() -> TestResult<()> {
    let db = setup_db().await?;
    let tenant_id = Uuid::new_v4();
    let page_id = insert_published_page(&db, tenant_id, &["en"]).await?;
    insert_body(&db, tenant_id, page_id, "en", DOCUMENT_ONLY_TEXT).await?;

    assert!(
        load_page_documents(&db, tenant_id, page_id)
            .await?
            .is_empty()
    );

    // A stored landing that no binding publishes changes nothing.
    insert_artifact(&db, tenant_id, page_id, "en", SUPERSEDED_TEXT).await?;
    assert!(
        load_page_documents(&db, tenant_id, page_id)
            .await?
            .is_empty()
    );

    let listed = PageSearchProjectionSource::new(db.clone())
        .list_public_documents(tenant_id, None, 10)
        .await
        .map_err(projection_error)?;
    assert!(listed.documents.is_empty());
    assert!(listed.next_cursor.is_none());
    Ok(())
}

#[tokio::test]
async fn a_locale_without_its_own_landing_is_indexed_without_landing_text() -> TestResult<()> {
    let db = setup_db().await?;
    let tenant_id = Uuid::new_v4();
    let page_id = insert_published_page(&db, tenant_id, &["en", "ru"]).await?;
    let body_id = insert_body(&db, tenant_id, page_id, "en", DOCUMENT_ONLY_TEXT).await?;
    // The `ru` locale has a document but was never published: visitors get the `en` landing.
    insert_body(&db, tenant_id, page_id, "ru", DOCUMENT_ONLY_TEXT).await?;
    let artifact_id = insert_artifact(&db, tenant_id, page_id, "en", CURRENT_TEXT).await?;
    bind_landing(&db, tenant_id, page_id, "en", body_id, artifact_id).await?;

    let mut documents = load_page_documents(&db, tenant_id, page_id).await?;
    documents.sort_by(|left, right| left.locale.cmp(&right.locale));

    assert_eq!(documents.len(), 2);
    assert_eq!(documents[0].locale, "en");
    assert!(documents[0].body.contains(CURRENT_TEXT));
    assert_eq!(documents[1].locale, "ru");
    assert_eq!(documents[1].title, "Page ru");
    assert_eq!(documents[1].body, "");
    Ok(())
}

#[tokio::test]
async fn a_binding_names_a_landing_of_its_own_page_locale() -> TestResult<()> {
    let db = setup_db().await?;
    let tenant_id = Uuid::new_v4();
    let page_id = insert_published_page(&db, tenant_id, &["en", "ru"]).await?;
    let body_id = insert_body(&db, tenant_id, page_id, "en", DOCUMENT_ONLY_TEXT).await?;
    let other_locale = insert_artifact(&db, tenant_id, page_id, "ru", SUPERSEDED_TEXT).await?;
    let own_locale = insert_artifact(&db, tenant_id, page_id, "en", CURRENT_TEXT).await?;

    // The projection indexes the landing a binding names under the locale of the binding, so
    // a binding to a landing of another page locale would index text in the wrong language.
    // The schema refuses such a binding (`m20260721_000004`: a trigger on SQLite, a composite
    // foreign key on PostgreSQL); the check in the projection is only a second line behind it.
    assert!(
        bind_landing(&db, tenant_id, page_id, "en", body_id, other_locale)
            .await
            .is_err(),
        "the schema must refuse a binding to a landing of another page locale"
    );
    assert!(
        load_page_documents(&db, tenant_id, page_id)
            .await?
            .is_empty()
    );

    // The same binding to the landing of its own page locale is stored: the refusal above
    // was about the locale of the landing, not about the fixture.
    bind_landing(&db, tenant_id, page_id, "en", body_id, own_locale).await?;

    let mut documents = load_page_documents(&db, tenant_id, page_id).await?;
    documents.sort_by(|left, right| left.locale.cmp(&right.locale));

    assert_eq!(documents.len(), 2);
    assert_eq!(documents[0].locale, "en");
    assert!(documents[0].body.contains(CURRENT_TEXT));
    assert!(!documents[0].body.contains(SUPERSEDED_TEXT));
    // The stored `ru` landing is not bound, so the `ru` document carries no landing text.
    assert_eq!(documents[1].locale, "ru");
    assert_eq!(documents[1].body, "");
    Ok(())
}

async fn load_page_documents(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    page_id: Uuid,
) -> TestResult<Vec<SearchProjectionDocument>> {
    PageSearchProjectionSource::new(db.clone())
        .load_public_entity(tenant_id, "page", page_id)
        .await
        .map_err(projection_error)
}

fn projection_error(error: impl std::fmt::Display) -> Box<dyn Error + Send + Sync> {
    std::io::Error::other(format!("page search projection failed: {error}")).into()
}

fn landing_project(heading: &str) -> Value {
    json!({
        "pages": [{
            "id": "home",
            "flyPageMeta": {
                "title": heading,
                "description": "Pages search projection fixture",
                "slug": "home"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": heading
                }]
            }
        }]
    })
}

async fn insert_published_page(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    locales: &[&str],
) -> TestResult<Uuid> {
    let page_id = Uuid::new_v4();
    let now: sea_orm::prelude::DateTimeWithTimeZone = Utc::now().into();
    page::ActiveModel {
        id: Set(page_id),
        tenant_id: Set(tenant_id),
        author_id: Set(None),
        status: Set("published".to_string()),
        template: Set("default".to_string()),
        metadata: Set(json!({})),
        created_at: Set(now),
        updated_at: Set(now),
        published_at: Set(Some(now)),
        archived_at: Set(None),
        version: Set(1),
    }
    .insert(db)
    .await?;
    for locale in locales {
        page_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            page_id: Set(page_id),
            tenant_id: Set(tenant_id),
            locale: Set((*locale).to_string()),
            title: Set(format!("Page {locale}")),
            slug: Set(format!("page-{locale}")),
            meta_title: Set(None),
            meta_description: Set(None),
            revision: Set(1),
        }
        .insert(db)
        .await?;
    }
    Ok(page_id)
}

async fn insert_body(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    page_id: Uuid,
    locale: &str,
    heading: &str,
) -> TestResult<Uuid> {
    let body_id = Uuid::new_v4();
    page_body::ActiveModel {
        id: Set(body_id),
        tenant_id: Set(tenant_id),
        page_id: Set(page_id),
        locale: Set(locale.to_string()),
        content: Set(serde_json::to_string(&landing_project(heading))?),
        format: Set(PAGE_BUILDER_DOCUMENT_FORMAT.to_string()),
        updated_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;
    Ok(body_id)
}

/// Stores a compiled landing of the page locale. It is not published until it is bound.
async fn insert_artifact(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    page_id: Uuid,
    locale: &str,
    heading: &str,
) -> TestResult<Uuid> {
    let artifact = StaticLandingCompiler::default().compile_publish(&landing_project(heading))?;
    artifact.verify_integrity()?;
    let rendered = artifact
        .pages
        .first()
        .cloned()
        .ok_or_else(|| std::io::Error::other("compiled landing artifact has no page"))?;
    let artifact_id = Uuid::new_v4();
    page_static_landing_artifact::ActiveModel {
        id: Set(artifact_id),
        tenant_id: Set(tenant_id),
        page_id: Set(page_id),
        locale: Set(locale.to_string()),
        source_hash: Set(artifact.identity.source_hash.clone()),
        build_hash: Set(artifact.identity.build_hash.clone()),
        artifact_hash: Set(artifact.artifact_hash.clone()),
        materialization_hash: Set(None),
        materialization_identity: Set(None),
        runtime_snapshots: Set(None),
        renderer_id: Set(artifact.identity.renderer.id.clone()),
        renderer_release: Set(artifact.identity.renderer.release.clone()),
        identity: Set(serde_json::to_value(&artifact.identity)?),
        registry: Set(serde_json::to_value(&artifact.registry)?),
        page_index: Set(i32::try_from(rendered.page_index)?),
        fly_page_id: Set(rendered.page_id.clone()),
        slug: Set(rendered.slug.clone()),
        head: Set(serde_json::to_value(&rendered.head)?),
        document_html: Set(rendered.document_html.clone()),
        body_html: Set(rendered.body_html.clone()),
        css: Set(rendered.css.clone()),
        content_hash: Set(rendered.content_hash.clone()),
        landing_sections: Set(serde_json::to_value(&rendered.landing_sections)?),
        instance_key: Set("canonical".to_string()),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;
    Ok(artifact_id)
}

async fn bind_landing(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    page_id: Uuid,
    locale: &str,
    body_id: Uuid,
    artifact_id: Uuid,
) -> TestResult<()> {
    page_published_landing_artifact::ActiveModel {
        page_body_id: Set(body_id),
        tenant_id: Set(tenant_id),
        page_id: Set(page_id),
        locale: Set(locale.to_string()),
        artifact_id: Set(artifact_id),
        published_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;
    Ok(())
}

async fn setup_db() -> TestResult<DatabaseConnection> {
    let database_url = format!(
        "sqlite:file:pages_search_projection_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;
    let manager = SchemaManager::new(&db);
    for migration in PagesModule.migrations() {
        migration.up(&manager).await?;
    }
    Ok(db)
}
