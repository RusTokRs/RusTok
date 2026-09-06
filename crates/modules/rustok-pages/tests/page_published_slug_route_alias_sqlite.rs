use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_pages::{
    CreatePageInput, PageRouteDisposition, PageRouteService, PageService, PageTranslationInput,
    PagesError, PagesModule, PatchPageMetadataInput,
};
use rustok_test_utils::db::setup_test_db;
use sea_orm::DatabaseConnection;
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

async fn setup() -> (DatabaseConnection, PageService, Uuid) {
    let db = setup_test_db().await;
    let manager = SchemaManager::new(&db);
    SysEventsMigration
        .up(&manager)
        .await
        .expect("outbox migrations should apply");
    for migration in PagesModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("Pages migrations should apply");
    }
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let service = PageService::new(db.clone(), event_bus);
    (db, service, Uuid::new_v4())
}

fn translation(locale: &str, title: &str, slug: &str) -> PageTranslationInput {
    PageTranslationInput {
        locale: locale.to_string(),
        title: title.to_string(),
        slug: Some(slug.to_string()),
        meta_title: None,
        meta_description: None,
    }
}

async fn create_draft(
    service: &PageService,
    tenant_id: Uuid,
    title: &str,
    slug: &str,
) -> rustok_pages::PageResponse {
    service
        .create(
            tenant_id,
            SecurityContext::system(),
            CreatePageInput {
                translations: vec![translation("en", title, slug)],
                template: Some("default".to_string()),
                body: None,
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect("page draft should be created")
}

#[tokio::test]
async fn published_slug_renames_create_immutable_redirects_and_reserve_old_claims() {
    let (db, service, tenant_id) = setup().await;
    let draft = create_draft(&service, tenant_id, "About", "about").await;
    let published = service
        .publish_non_builder_if_current(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            Some(draft.version),
        )
        .await
        .expect("page should publish");

    let renamed = service
        .patch_metadata(
            tenant_id,
            SecurityContext::system(),
            published.id,
            PatchPageMetadataInput {
                expected_version: published.version,
                translations: Some(vec![translation("en", "About us", "about-us")]),
                template: None,
                channel_slugs: None,
            },
        )
        .await
        .expect("published slug should rename with an alias");
    let latest = service
        .patch_metadata(
            tenant_id,
            SecurityContext::system(),
            renamed.id,
            PatchPageMetadataInput {
                expected_version: renamed.version,
                translations: Some(vec![translation("en", "Company", "company")]),
                template: None,
                channel_slugs: None,
            },
        )
        .await
        .expect("second published rename should append another alias");

    let routes = PageRouteService::new(db.clone());
    let canonical = routes
        .resolve(tenant_id, "en", "company")
        .await
        .expect("current slug should resolve canonically");
    assert_eq!(canonical.disposition, PageRouteDisposition::Canonical);
    assert_eq!(canonical.requested_page_id, Some(latest.id));
    assert_eq!(
        canonical
            .canonical
            .as_ref()
            .map(|route| route.path.as_str()),
        Some("/en/modules/pages?slug=company")
    );

    for old_slug in ["about", "about-us"] {
        let redirect = routes
            .resolve(tenant_id, "en", old_slug)
            .await
            .expect("published historical slug should resolve as a redirect");
        assert_eq!(redirect.disposition, PageRouteDisposition::Redirect);
        assert_eq!(redirect.requested_page_id, Some(latest.id));
        assert!(redirect.alias_id.is_some());
        assert_eq!(
            redirect.canonical.as_ref().map(|route| route.path.as_str()),
            Some("/en/modules/pages?slug=company")
        );
    }

    let duplicate = service
        .create(
            tenant_id,
            SecurityContext::system(),
            CreatePageInput {
                translations: vec![translation("en", "Replacement", "about")],
                template: Some("default".to_string()),
                body: None,
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect_err("an immutable historical route claim must not be reused");
    assert!(matches!(duplicate, PagesError::DuplicateSlug { .. }));
}

#[tokio::test]
async fn draft_only_slug_renames_do_not_create_public_route_history() {
    let (_db, service, tenant_id) = setup().await;
    let draft = create_draft(&service, tenant_id, "Draft old", "draft-old").await;
    service
        .patch_metadata(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            PatchPageMetadataInput {
                expected_version: draft.version,
                translations: Some(vec![translation("en", "Draft new", "draft-new")]),
                template: None,
                channel_slugs: None,
            },
        )
        .await
        .expect("draft slug should rename without public history");

    let replacement = create_draft(&service, tenant_id, "Replacement", "draft-old").await;
    assert_eq!(
        replacement
            .translation
            .and_then(|translation| translation.slug)
            .as_deref(),
        Some("draft-old")
    );
}
