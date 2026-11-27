use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
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

fn translation(title: &str, slug: &str) -> PageTranslationInput {
    PageTranslationInput {
        locale: "en".to_string(),
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
                translations: vec![translation(title, slug)],
                template: Some("default".to_string()),
                body: None,
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect("page draft should be created")
}

async fn patch_slug(
    service: &PageService,
    tenant_id: Uuid,
    page: &rustok_pages::PageResponse,
    title: &str,
    slug: &str,
) -> rustok_pages::PageResponse {
    service
        .patch_metadata(
            tenant_id,
            SecurityContext::system(),
            page.id,
            PatchPageMetadataInput {
                expected_version: page.version,
                translations: Some(vec![translation(title, slug)]),
                template: None,
                channel_slugs: None,
            },
        )
        .await
        .expect("page slug should update")
}

#[tokio::test]
async fn delete_turns_every_retained_public_route_into_gone_without_rewriting_redirects() {
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

    let published_rename =
        patch_slug(&service, tenant_id, &published, "About us", "about-us").await;
    let first_unpublished = service
        .unpublish_if_current(
            tenant_id,
            SecurityContext::system(),
            published_rename.id,
            Some(published_rename.version),
        )
        .await
        .expect("published route should be snapshotted before unpublish");

    let draft_rename = patch_slug(
        &service,
        tenant_id,
        &first_unpublished,
        "Company",
        "company",
    )
    .await;
    let republished = service
        .publish_non_builder_if_current(
            tenant_id,
            SecurityContext::system(),
            draft_rename.id,
            Some(draft_rename.version),
        )
        .await
        .expect("renamed draft should republish");
    let final_unpublished = service
        .unpublish_if_current(
            tenant_id,
            SecurityContext::system(),
            republished.id,
            Some(republished.version),
        )
        .await
        .expect("second public route should be snapshotted before unpublish");

    service
        .delete(tenant_id, SecurityContext::system(), final_unpublished.id)
        .await
        .expect("unpublished page should delete with tombstones");

    let routes = PageRouteService::new(db);
    for slug in ["about", "about-us", "company"] {
        let resolution = routes
            .resolve(tenant_id, "en", slug)
            .await
            .expect("every formerly public route should remain resolvable as gone");
        assert_eq!(resolution.disposition, PageRouteDisposition::Gone);
        assert_eq!(resolution.requested_page_id, Some(final_unpublished.id));
        assert!(resolution.canonical.is_none());
        assert!(resolution.alias_id.is_some());
    }

    let duplicate = service
        .create(
            tenant_id,
            SecurityContext::system(),
            CreatePageInput {
                translations: vec![translation("Replacement", "company")],
                template: Some("default".to_string()),
                body: None,
                channel_slugs: None,
                publish: false,
            },
        )
        .await
        .expect_err("a deleted public route claim must remain reserved");
    assert!(matches!(duplicate, PagesError::DuplicateSlug { .. }));
}

#[tokio::test]
async fn deleting_a_never_published_draft_does_not_reserve_its_slug() {
    let (_db, service, tenant_id) = setup().await;
    let draft = create_draft(&service, tenant_id, "Temporary", "temporary").await;
    service
        .delete(tenant_id, SecurityContext::system(), draft.id)
        .await
        .expect("draft should delete without a public tombstone");

    let replacement = create_draft(&service, tenant_id, "Replacement", "temporary").await;
    assert_eq!(
        replacement
            .translation
            .and_then(|translation| translation.slug)
            .as_deref(),
        Some("temporary")
    );
}

#[tokio::test]
async fn unpublish_receipt_replays_before_a_later_publish_and_cas_check() {
    let (_db, service, tenant_id) = setup().await;
    let security = SecurityContext::system();
    let draft = create_draft(&service, tenant_id, "Lifecycle", "lifecycle").await;
    let published = service
        .publish_non_builder_if_current(tenant_id, security.clone(), draft.id, Some(draft.version))
        .await
        .expect("page should publish");
    let unpublish_key = "unpublish-replay-1".to_string();

    let unpublished = service
        .unpublish_with_receipt(
            tenant_id,
            security.clone(),
            published.id,
            Some(published.version),
            unpublish_key.clone(),
        )
        .await
        .expect("first unpublish should commit");
    assert_eq!(
        unpublished.status,
        rustok_content::entities::node::ContentStatus::Draft
    );

    let republished = service
        .publish_non_builder_if_current(
            tenant_id,
            security.clone(),
            published.id,
            Some(unpublished.version),
        )
        .await
        .expect("a later publish should commit");
    assert_eq!(
        republished.status,
        rustok_content::entities::node::ContentStatus::Published
    );

    let replayed = service
        .unpublish_with_receipt(
            tenant_id,
            security,
            published.id,
            Some(published.version),
            unpublish_key,
        )
        .await
        .expect("retry must replay before the stale version check");
    assert_eq!(replayed.version, unpublished.version);
    assert_eq!(replayed.status, unpublished.status);
    let current = service
        .get(tenant_id, SecurityContext::system(), published.id)
        .await
        .expect("later published page remains current");
    assert_eq!(
        current.status,
        rustok_content::entities::node::ContentStatus::Published
    );
}

#[tokio::test]
async fn delete_receipt_survives_page_removal_and_replays_for_the_same_actor() {
    let (_db, service, tenant_id) = setup().await;
    let security = SecurityContext::system();
    let draft = create_draft(&service, tenant_id, "Delete receipt", "delete-receipt").await;
    let key = "delete-replay-1".to_string();

    service
        .delete_with_receipt(
            tenant_id,
            security.clone(),
            draft.id,
            Some(draft.version),
            key.clone(),
        )
        .await
        .expect("first delete should commit");
    service
        .delete_with_receipt(
            tenant_id,
            security.clone(),
            draft.id,
            Some(draft.version),
            key.clone(),
        )
        .await
        .expect("retry should replay even though the page row is gone");

    let conflict = service
        .delete_with_receipt(
            tenant_id,
            SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4())),
            draft.id,
            Some(draft.version),
            key,
        )
        .await
        .expect_err("the same receipt must not be replayed by a different actor");
    assert!(conflict.to_string().contains("idempotency key"));
}

#[tokio::test]
async fn archive_receipt_replays_after_a_later_publish() {
    let (_db, service, tenant_id) = setup().await;
    let security = SecurityContext::system();
    let draft = create_draft(&service, tenant_id, "Archive receipt", "archive-receipt").await;
    let key = "archive-replay-1".to_string();

    let archived = service
        .archive_with_receipt(
            tenant_id,
            security.clone(),
            draft.id,
            Some(draft.version),
            key.clone(),
        )
        .await
        .expect("archive should commit");
    assert_eq!(
        archived.status,
        rustok_content::entities::node::ContentStatus::Archived
    );

    let published = service
        .publish_non_builder_if_current(
            tenant_id,
            security.clone(),
            draft.id,
            Some(archived.version),
        )
        .await
        .expect("a later publish should commit");
    assert_eq!(
        published.status,
        rustok_content::entities::node::ContentStatus::Published
    );

    let replayed = service
        .archive_with_receipt(tenant_id, security, draft.id, Some(draft.version), key)
        .await
        .expect("archive retry should replay before stale CAS");
    assert_eq!(replayed.version, archived.version);
    assert_eq!(replayed.status, archived.status);
}
