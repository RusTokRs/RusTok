//! Regressions for the Search projection source boundary.
//!
//! Two properties are pinned here because both used to be wrong in a way no
//! compile-time check can catch:
//!
//! * the public corpus is `published` **and** stamped with `published_at`, like
//!   every other Blog read path, so the source never hands the storefront a
//!   document it marks as non-discoverable;
//! * the keyset walk the indexer performs is ordered by immutable keys, so a post
//!   pinned between two pages cannot jump *before* the cursor and be missed for the
//!   whole rebuild.
//!
//! The harness is the crate's sqlite unit-test harness, so no external database is
//! required.

use std::collections::HashSet;

use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use uuid::Uuid;

use rustok_core::search_projection::SearchProjectionSource;

use super::hardening::{admin, create_draft_post, post_service, setup_hardening_schema};
use crate::BlogSearchProjectionSource;
use crate::entities::blog_post;

/// Publishes a draft and returns its id.
async fn publish(
    db: &DatabaseConnection,
    event_bus: &rustok_outbox::TransactionalEventBus,
    tenant_id: Uuid,
    slug: &str,
) -> Uuid {
    let post_id = create_draft_post(db, event_bus, tenant_id, slug).await;
    post_service(db, event_bus)
        .publish_post(tenant_id, post_id, admin())
        .await
        .expect("draft should publish");
    post_id
}

/// Pins a post through the owner command, which rewrites `is_pinned`/`pinned_at`
/// without emitting a reindex request.
async fn pin(
    db: &DatabaseConnection,
    event_bus: &rustok_outbox::TransactionalEventBus,
    tenant_id: Uuid,
    post_id: Uuid,
) {
    post_service(db, event_bus)
        .pin_post(tenant_id, post_id, admin())
        .await
        .expect("a published post should pin");
}

/// Walks the public corpus page by page, exactly as the Search indexer does.
async fn walk(source: &BlogSearchProjectionSource, tenant_id: Uuid, page_size: usize) -> Vec<Uuid> {
    let mut seen = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let page = source
            .list_public_documents(tenant_id, after.clone(), page_size)
            .await
            .expect("the projection page should be readable");
        assert!(
            page.documents.len() <= page_size,
            "a page never exceeds its bound"
        );
        seen.extend(page.documents.iter().map(|document| document.document_id));
        match page.next_cursor {
            Some(cursor) => after = Some(cursor),
            None => break,
        }
    }
    seen
}

#[tokio::test]
async fn pinning_a_post_between_pages_does_not_skip_it() {
    let (db, event_bus) = setup_hardening_schema().await;
    let tenant_id = Uuid::new_v4();
    let source = BlogSearchProjectionSource::new(db.clone(), None);

    let first = publish(&db, &event_bus, tenant_id, "first").await;
    let second = publish(&db, &event_bus, tenant_id, "second").await;
    let third = publish(&db, &event_bus, tenant_id, "third").await;

    // First page of one document: the walk now has a cursor in the middle of the
    // published corpus.
    let mut after: Option<String> = None;
    let mut visited = Vec::new();
    let mut pinned_once = false;
    loop {
        let page = source
            .list_public_documents(tenant_id, after.clone(), 1)
            .await
            .expect("the projection page should be readable");
        visited.extend(page.documents.iter().map(|document| document.document_id));

        // Pin a post the walk has not necessarily reached yet. Pin state is
        // presentation state: it must not reorder this walk.
        if !pinned_once {
            pin(&db, &event_bus, tenant_id, first).await;
            pinned_once = true;
        }

        match page.next_cursor {
            Some(cursor) => after = Some(cursor),
            None => break,
        }
    }

    let unique = visited.iter().copied().collect::<HashSet<_>>();
    assert_eq!(
        unique.len(),
        visited.len(),
        "a document must not be visited twice: {visited:?}"
    );
    for post_id in [first, second, third] {
        assert!(
            unique.contains(&post_id),
            "post {post_id} must be visited exactly once across the walk: {visited:?}"
        );
    }

    // Re-walking from the start after the pin sees the same corpus.
    let rewound = walk(&source, tenant_id, 2).await;
    assert_eq!(
        rewound.iter().copied().collect::<HashSet<_>>(),
        unique,
        "the corpus is independent of pin state"
    );
}

#[tokio::test]
async fn a_published_post_without_a_publication_timestamp_is_not_projectable() {
    let (db, event_bus) = setup_hardening_schema().await;
    let tenant_id = Uuid::new_v4();
    let source = BlogSearchProjectionSource::new(db.clone(), None);

    let healthy = publish(&db, &event_bus, tenant_id, "healthy").await;
    let damaged = publish(&db, &event_bus, tenant_id, "damaged").await;

    // Published state without its timestamp is only reachable as corruption, but it
    // is exactly the row every other public surface refuses.
    let row = blog_post::Entity::find_by_id(damaged)
        .one(&db)
        .await
        .expect("post read should not fail")
        .expect("post should exist");
    crate::tests::hardening::allow_legacy_invalid_post_fixture(&db).await;
    let mut active: blog_post::ActiveModel = row.into();
    active.published_at = Set(None);
    active.update(&db).await.expect("damage should persist");

    let documents = walk(&source, tenant_id, 10).await;
    assert!(
        documents.contains(&healthy),
        "the healthy post must stay projectable: {documents:?}"
    );
    assert!(
        !documents.contains(&damaged),
        "a published post without published_at is not part of the public corpus: {documents:?}"
    );

    let entity = source
        .load_public_entity(tenant_id, "blog_post", damaged)
        .await
        .expect("entity lookup should not fail");
    assert!(
        entity.is_empty(),
        "the single-entity lookup applies the same corpus rule"
    );
}
