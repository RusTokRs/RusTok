//! Owner-command regressions for the slices that shipped without owner tests.
//!
//! `publish_scheduled_posts` was the largest untested command in the module (audit
//! finding L-7 of 2026-10-15): the host scheduler calls it for one tenant, and nothing
//! exercised the due-post scan, the tenant scope, the batch bound or the
//! `BlogPostPublished` publication. The pin tests below cover the write path that
//! `pin.rs`'s predicate-only test cannot reach.
//!
//! The harness is the crate's sqlite unit-test harness (`rustok_test_utils`), so no
//! external database is required. A due post is written directly: the owner commands
//! require a future schedule on input, and only the passage of time makes one due.

use chrono::{Duration, Utc};
use sea_orm::{DatabaseConnection, EntityTrait};
use uuid::Uuid;

use super::hardening::{
    admin, create_draft_post, make_post_due, outbox_event_count, post_service, set_post_schedule,
    setup_hardening_schema,
};
use crate::BlogPostStatus;
use crate::entities::blog_post;
use crate::services::{MAX_SCHEDULED_PUBLISH_BATCH, status_to_storage};

/// Reads a post row straight from the owner table, so an assertion cannot be satisfied by
/// a cached DTO.
async fn post_row(db: &DatabaseConnection, post_id: Uuid) -> blog_post::Model {
    blog_post::Entity::find_by_id(post_id)
        .one(db)
        .await
        .expect("post read should not fail")
        .expect("post should exist")
}

async fn assert_status(
    db: &DatabaseConnection,
    post_id: Uuid,
    expected: BlogPostStatus,
    message: &str,
) {
    let row = post_row(db, post_id).await;
    assert_eq!(
        row.status.as_str(),
        status_to_storage(expected),
        "{message}"
    );
}

#[tokio::test]
async fn scheduled_publish_claims_only_due_drafts_of_the_tenant() {
    let (db, events) = setup_hardening_schema().await;
    let tenant_id = Uuid::new_v4();
    let other_tenant = Uuid::new_v4();
    let service = post_service(&db, &events);

    let due = create_draft_post(&db, &events, tenant_id, "due").await;
    make_post_due(&db, due, 1).await;
    let future = create_draft_post(&db, &events, tenant_id, "future").await;
    set_post_schedule(&db, future, Some(Utc::now() + Duration::hours(1))).await;
    let unscheduled = create_draft_post(&db, &events, tenant_id, "unscheduled").await;
    let foreign_due = create_draft_post(&db, &events, other_tenant, "foreign-due").await;
    make_post_due(&db, foreign_due, 1).await;

    let published = service
        .publish_scheduled_posts(tenant_id, MAX_SCHEDULED_PUBLISH_BATCH)
        .await
        .expect("the scheduled scan should run");

    assert_eq!(
        published,
        vec![due],
        "only the tenant's due draft is published"
    );
    assert_status(
        &db,
        due,
        BlogPostStatus::Published,
        "a due draft becomes published",
    )
    .await;
    let published_row = post_row(&db, due).await;
    assert!(
        published_row.published_at.is_some(),
        "a scheduled publish stamps the first publication time"
    );
    assert_status(
        &db,
        future,
        BlogPostStatus::Draft,
        "a future schedule is left alone",
    )
    .await;
    assert_status(
        &db,
        unscheduled,
        BlogPostStatus::Draft,
        "a draft without a schedule is left alone",
    )
    .await;
    assert_status(
        &db,
        foreign_due,
        BlogPostStatus::Draft,
        "another tenant's due draft is never reached",
    )
    .await;
    assert_eq!(
        outbox_event_count(&db, "blog.post.published").await,
        1,
        "each published post is announced once"
    );

    // The due set is drained: a repeated scheduler iteration finds nothing to publish.
    let second = service
        .publish_scheduled_posts(tenant_id, MAX_SCHEDULED_PUBLISH_BATCH)
        .await
        .expect("the second scan should run");
    assert!(second.is_empty(), "a drained queue must not republish");
    assert_eq!(
        outbox_event_count(&db, "blog.post.published").await,
        1,
        "a repeated iteration publishes no second event"
    );
}

#[tokio::test]
async fn scheduled_publish_drains_oldest_due_first_and_clamps_its_limit() {
    let (db, events) = setup_hardening_schema().await;
    let tenant_id = Uuid::new_v4();
    let service = post_service(&db, &events);

    let oldest = create_draft_post(&db, &events, tenant_id, "oldest").await;
    make_post_due(&db, oldest, 3).await;
    let middle = create_draft_post(&db, &events, tenant_id, "middle").await;
    make_post_due(&db, middle, 2).await;
    let newest = create_draft_post(&db, &events, tenant_id, "newest").await;
    make_post_due(&db, newest, 1).await;

    // A zero limit is clamped to one post, so housekeeping cannot publish a whole backlog
    // in one call.
    let first = service
        .publish_scheduled_posts(tenant_id, 0)
        .await
        .expect("the scheduled scan should run");
    assert_eq!(
        first,
        vec![oldest],
        "a zero limit publishes exactly the oldest due post"
    );

    let rest = service
        .publish_scheduled_posts(tenant_id, MAX_SCHEDULED_PUBLISH_BATCH)
        .await
        .expect("the second scan should run");
    assert_eq!(
        rest,
        vec![middle, newest],
        "the remaining due drafts drain oldest first"
    );
    assert_eq!(
        outbox_event_count(&db, "blog.post.published").await,
        3,
        "every published post is announced exactly once"
    );
    for post_id in [oldest, middle, newest] {
        assert_status(
            &db,
            post_id,
            BlogPostStatus::Published,
            "every due draft is published once the batch allows it",
        )
        .await;
    }
}

#[tokio::test]
async fn unpin_releases_pin_state_of_an_archived_post() {
    let (db, events) = setup_hardening_schema().await;
    let tenant_id = Uuid::new_v4();
    let service = post_service(&db, &events);
    let post_id = create_draft_post(&db, &events, tenant_id, "pin-release").await;

    service
        .publish_post(tenant_id, post_id, admin())
        .await
        .expect("the draft should publish");
    service
        .pin_post(tenant_id, post_id, admin())
        .await
        .expect("a published post should be pinnable");
    let pinned = post_row(&db, post_id).await;
    assert!(
        pinned.is_pinned,
        "a pin writes the flag the public ordering reads"
    );
    assert!(
        pinned.pinned_at.is_some(),
        "a pin writes the timestamp with the flag"
    );

    service
        .archive_post(tenant_id, post_id, admin(), None)
        .await
        .expect("the post should archive");
    let archived = post_row(&db, post_id).await;
    assert!(
        archived.is_pinned,
        "archiving keeps the pin state until it is released"
    );

    service
        .unpin_post(tenant_id, post_id, admin())
        .await
        .expect("an archived post must stay releasable from the pinned block");
    let released = post_row(&db, post_id).await;
    assert!(!released.is_pinned, "unpin clears the flag");
    assert!(
        released.pinned_at.is_none(),
        "unpin clears the timestamp together with the flag"
    );
}
