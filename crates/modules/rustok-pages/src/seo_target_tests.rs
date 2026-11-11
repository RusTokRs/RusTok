//! The SEO record of a page, read through the registered provider after a reviewed publish.
//!
//! Content-derived values (the fallback description and the primary image) come from the
//! published landing of the page. The authoring document is never a source: an edit that has
//! not been published does not change the record, and a page without a published landing has
//! no content-derived values.

use std::error::Error;

use chrono::{Duration as ChronoDuration, Utc};
use rustok_channel::ChannelModule;
use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_page_builder::PageBuilderReviewedPublishRuntime;
use rustok_seo_targets::{
    SeoLoadedTargetRecord, SeoSitemapCandidateRecord, SeoTargetLoadRequest, SeoTargetLoadScope,
    SeoTargetProvider, SeoTargetRuntimeContext, SeoTargetSitemapRequest,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection,
    DbBackend, EntityTrait, QueryFilter, Set, Statement,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

use crate::PagesModule;
use crate::dto::{
    CreatePageInput, PageBodyInput, PageBodyRevisionInput, PageTranslationInput, PublishPageInput,
    ReviewedPagePublishRuntimeInput,
};
use crate::entities::{page_body, page_published_landing_artifact};
use crate::seo_targets::PagesSeoTargetProvider;
use crate::services::PageService;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const PAGE_TITLE: &str = "Shop";
const CHANNEL: &str = "web";
const PUBLISHED_HEADING: &str = "Welcome to the shop";
const PUBLISHED_IMAGE: &str = "https://cdn.example.com/hero.webp";
const PUBLISHED_IMAGE_ALT: &str = "Product hero";
const UNPUBLISHED_HEADING: &str = "Unpublished headline";
const UNPUBLISHED_IMAGE: &str = "https://cdn.example.com/unpublished.webp";

struct Fixture {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    tenant_id: Uuid,
    page_id: Uuid,
    body_id: Uuid,
}

#[tokio::test]
async fn description_and_image_come_from_the_published_landing() -> TestResult<()> {
    let fixture = reviewed_published_page(None).await?;

    let record = load(&fixture, SeoTargetLoadScope::PublicRoute, Some(CHANNEL))
        .await?
        .ok_or("a published page has a public SEO record")?;

    assert_eq!(record.title, PAGE_TITLE);
    // The page has no meta description, so the description is the visible landing text.
    let description = record
        .description
        .as_deref()
        .ok_or("the landing text is the fallback description")?;
    assert!(
        description.contains(PUBLISHED_HEADING),
        "description must come from the landing text, got {description:?}"
    );
    assert!(
        !description.contains('<'),
        "description must be text, not markup: {description:?}"
    );
    // The primary image is the first image of the landing markup, with its own alt text.
    assert_eq!(record.open_graph.images.len(), 1);
    assert_eq!(record.open_graph.images[0].url, PUBLISHED_IMAGE);
    assert_eq!(
        record.open_graph.images[0].alt.as_deref(),
        Some(PUBLISHED_IMAGE_ALT)
    );
    Ok(())
}

#[tokio::test]
async fn an_unpublished_document_edit_does_not_change_the_record() -> TestResult<()> {
    let fixture = reviewed_published_page(None).await?;
    let before = load(&fixture, SeoTargetLoadScope::PublicRoute, Some(CHANNEL))
        .await?
        .ok_or("a published page has a public SEO record")?;

    // The stored document changes; nothing is published.
    let body = page_body::Entity::find_by_id(fixture.body_id)
        .one(&fixture.db)
        .await?
        .ok_or("published page body is missing")?;
    let mut edited: page_body::ActiveModel = body.into();
    edited.content = Set(serde_json::to_string(&landing_project(
        UNPUBLISHED_HEADING,
        UNPUBLISHED_IMAGE,
    ))?);
    edited.updated_at = Set((Utc::now() + ChronoDuration::seconds(1)).into());
    edited.update(&fixture.db).await?;

    // The SEO editor loads the authoring record without a channel.
    for (scope, channel) in [
        (SeoTargetLoadScope::PublicRoute, Some(CHANNEL)),
        (SeoTargetLoadScope::Authoring, None),
    ] {
        let after = load(&fixture, scope, channel)
            .await?
            .ok_or("the page still has an SEO record")?;
        assert_eq!(after.description, before.description, "scope {scope:?}");
        assert_eq!(
            after.open_graph.images, before.open_graph.images,
            "scope {scope:?}"
        );
        assert!(
            !after
                .description
                .as_deref()
                .is_some_and(|description| description.contains(UNPUBLISHED_HEADING)),
            "scope {scope:?}: unpublished text must not reach the SEO record"
        );
    }
    Ok(())
}

#[tokio::test]
async fn an_authoring_view_reads_the_landing_of_a_channel_restricted_page() -> TestResult<()> {
    let fixture = reviewed_published_page(None).await?;
    let public = load(&fixture, SeoTargetLoadScope::PublicRoute, Some(CHANNEL))
        .await?
        .ok_or("a published page has a public SEO record")?;

    // No channel (the SEO editor) and a channel that does not admit the page: the author
    // still sees the description and the image the visitors of the page get.
    for channel in [None, Some("mobile")] {
        let authoring = load(&fixture, SeoTargetLoadScope::Authoring, channel)
            .await?
            .ok_or("an author reads the SEO record of the page on any channel")?;
        assert_eq!(
            authoring.description, public.description,
            "channel {channel:?}"
        );
        assert_eq!(
            authoring.open_graph.images, public.open_graph.images,
            "channel {channel:?}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn meta_description_wins_over_the_landing_text() -> TestResult<()> {
    let fixture = reviewed_published_page(Some("Hand-written description")).await?;

    let record = load(&fixture, SeoTargetLoadScope::PublicRoute, Some(CHANNEL))
        .await?
        .ok_or("a published page has a public SEO record")?;

    assert_eq!(
        record.description.as_deref(),
        Some("Hand-written description")
    );
    // The image has no hand-written counterpart on a page: it stays the landing image.
    assert_eq!(record.open_graph.images.len(), 1);
    assert_eq!(record.open_graph.images[0].url, PUBLISHED_IMAGE);
    Ok(())
}

#[tokio::test]
async fn a_visitor_of_another_channel_gets_no_record() -> TestResult<()> {
    let fixture = reviewed_published_page(None).await?;

    for channel in [Some("mobile"), None] {
        assert!(
            load(&fixture, SeoTargetLoadScope::PublicRoute, channel)
                .await?
                .is_none(),
            "channel {channel:?} must not resolve a page restricted to `{CHANNEL}`"
        );
    }
    Ok(())
}

#[tokio::test]
async fn without_a_published_landing_nothing_is_derived_from_the_document() -> TestResult<()> {
    let fixture = reviewed_published_page(None).await?;
    // The document is still stored in full; only the landing binding is gone.
    page_published_landing_artifact::Entity::delete_by_id(fixture.body_id)
        .exec(&fixture.db)
        .await?;

    let record = load(&fixture, SeoTargetLoadScope::PublicRoute, Some(CHANNEL))
        .await?
        .ok_or("the page record still resolves")?;

    // No meta description and no landing: the description falls back to the page title.
    assert_eq!(record.description.as_deref(), Some(PAGE_TITLE));
    assert!(record.open_graph.images.is_empty());
    Ok(())
}

#[tokio::test]
async fn the_sitemap_lists_a_page_with_its_landing_image_and_omits_a_page_without_a_landing()
-> TestResult<()> {
    // The sitemap covers pages without a channel restriction.
    let fixture = reviewed_published_page_on(None, None).await?;

    let candidates = sitemap(&fixture).await?;
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].target_id, fixture.page_id);
    assert_eq!(candidates[0].images.len(), 1);
    assert_eq!(candidates[0].images[0].url, PUBLISHED_IMAGE);

    // Without a published landing the page has no storefront route to list.
    page_published_landing_artifact::Entity::delete_by_id(fixture.body_id)
        .exec(&fixture.db)
        .await?;
    assert!(sitemap(&fixture).await?.is_empty());
    Ok(())
}

async fn sitemap(fixture: &Fixture) -> TestResult<Vec<SeoSitemapCandidateRecord>> {
    let runtime = SeoTargetRuntimeContext {
        db: fixture.db.clone(),
        event_bus: fixture.event_bus.clone(),
        static_settings_reader: None,
    };
    let candidates = PagesSeoTargetProvider
        .sitemap_candidates(
            &runtime,
            SeoTargetSitemapRequest {
                tenant_id: fixture.tenant_id,
                default_locale: "en",
            },
        )
        .await
        .map_err(|error| std::io::Error::other(format!("SEO sitemap scan failed: {error:#}")))?;
    Ok(candidates)
}

async fn load(
    fixture: &Fixture,
    scope: SeoTargetLoadScope,
    channel_slug: Option<&str>,
) -> TestResult<Option<SeoLoadedTargetRecord>> {
    let runtime = SeoTargetRuntimeContext {
        db: fixture.db.clone(),
        event_bus: fixture.event_bus.clone(),
        static_settings_reader: None,
    };
    let record = PagesSeoTargetProvider
        .load_target(
            &runtime,
            SeoTargetLoadRequest {
                tenant_id: fixture.tenant_id,
                default_locale: "en",
                locale: "en",
                target_id: fixture.page_id,
                scope,
                channel_slug,
            },
        )
        .await
        .map_err(|error| std::io::Error::other(format!("SEO target load failed: {error:#}")))?;
    Ok(record)
}

fn landing_project(heading: &str, image: &str) -> Value {
    json!({
        "pages": [{
            "id": "home",
            "flyPageMeta": {
                "title": PAGE_TITLE,
                "description": "Document description that is not an SEO source",
                "slug": "shop"
            },
            "component": {
                "id": "root",
                "type": "wrapper",
                "components": [{
                    "id": "heading",
                    "type": "heading",
                    "tagName": "h1",
                    "content": heading
                }, {
                    "id": "hero",
                    "type": "image",
                    "tagName": "img",
                    "attributes": { "src": image, "alt": PUBLISHED_IMAGE_ALT }
                }]
            }
        }]
    })
}

/// A page restricted to [`CHANNEL`], published through the reviewed publish command.
async fn reviewed_published_page(meta_description: Option<&str>) -> TestResult<Fixture> {
    reviewed_published_page_on(meta_description, Some(vec![CHANNEL.to_string()])).await
}

/// A page published through the reviewed publish command, with the given channel list.
async fn reviewed_published_page_on(
    meta_description: Option<&str>,
    channel_slugs: Option<Vec<String>>,
) -> TestResult<Fixture> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let service = PageService::new(db.clone(), event_bus.clone());

    let draft = service
        .create(
            tenant_id,
            SecurityContext::system(),
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: PAGE_TITLE.to_string(),
                    slug: Some("shop".to_string()),
                    meta_title: None,
                    meta_description: meta_description.map(str::to_string),
                }],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    document: landing_project(PUBLISHED_HEADING, PUBLISHED_IMAGE),
                }),
                channel_slugs,
                publish: false,
            },
        )
        .await?;
    let body_revision = draft
        .body
        .as_ref()
        .ok_or("the author of a page reads its document")?
        .updated_at
        .clone();
    let body_id = page_body::Entity::find()
        .filter(page_body::Column::TenantId.eq(tenant_id))
        .filter(page_body::Column::PageId.eq(draft.id))
        .filter(page_body::Column::Locale.eq("en"))
        .one(&db)
        .await?
        .ok_or("draft body is missing from owner storage")?
        .id;

    let reviewed = PageBuilderReviewedPublishRuntime::new(
        "pages-seo-target",
        json!({ "surface": "storefront", "channel": CHANNEL }),
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
                idempotency_key: "pages-seo-target-v1".to_string(),
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
    assert!(
        page_published_landing_artifact::Entity::find_by_id(body_id)
            .one(&db)
            .await?
            .is_some(),
        "reviewed publish must bind a landing"
    );

    Ok(Fixture {
        db,
        event_bus,
        tenant_id,
        page_id: draft.id,
        body_id,
    })
}

async fn setup_db(tenant_id: Uuid) -> TestResult<DatabaseConnection> {
    let database_url = format!(
        "sqlite:file:pages_seo_target_{}?mode=memory&cache=shared",
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
