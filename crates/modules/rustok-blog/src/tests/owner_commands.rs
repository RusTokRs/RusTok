//! Owner-command regressions for the slices that shipped without owner tests.
//!
//! `publish_scheduled_posts` was the largest untested command in the module (audit
//! finding L-7 of 2026-10-15): the host scheduler calls it for one tenant, and nothing
//! exercised the due-post scan, the tenant scope, the batch bound or the
//! `BlogPostPublished` publication. The pin tests below cover the write path that
//! `pin.rs`'s predicate-only test cannot reach: the published-only rejection, the
//! tenant scope, and the column boundary of the pin statement itself (pin state must
//! not be written through `status`/`published_at`/`archived_at`/`scheduled_at`, and a
//! pin is presentation state, not a new `version`).
//!
//! The harness is the crate's sqlite unit-test harness (`rustok_test_utils`), so no
//! external database is required. A due post is written directly: the owner commands
//! require a future schedule on input, and only the passage of time makes one due.

use chrono::{Duration, Utc};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, TransactionTrait,
};
use uuid::Uuid;

use super::hardening::{
    admin, create_draft_post, make_post_due, outbox_event_count, post_input, post_service,
    set_post_schedule, setup_hardening_schema,
};
use crate::entities::{blog_post, blog_post_tag, blog_tag_usage};
use crate::services::{MAX_SCHEDULED_PUBLISH_BATCH, TagService, status_to_storage};
use crate::{BlogError, BlogPostStatus, CreateTagInput, UpdateTagInput};

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

#[tokio::test]
async fn pinning_a_draft_is_rejected_without_writing_pin_state() {
    let (db, events) = setup_hardening_schema().await;
    let tenant_id = Uuid::new_v4();
    let service = post_service(&db, &events);
    let post_id = create_draft_post(&db, &events, tenant_id, "pin-draft").await;

    let error = service
        .pin_post(tenant_id, post_id, admin())
        .await
        .expect_err("a draft must not be pinnable");

    assert!(
        matches!(error, BlogError::Validation(_)),
        "the published-only rule is a validation failure, got {error:?}"
    );
    let row = post_row(&db, post_id).await;
    assert!(!row.is_pinned, "a rejected pin leaves the flag untouched");
    assert!(
        row.pinned_at.is_none(),
        "a rejected pin leaves the timestamp untouched"
    );
}

#[tokio::test]
async fn pin_state_is_written_only_for_the_owning_tenant() {
    let (db, events) = setup_hardening_schema().await;
    let tenant_id = Uuid::new_v4();
    let foreign_tenant = Uuid::new_v4();
    let service = post_service(&db, &events);
    let post_id = create_draft_post(&db, &events, tenant_id, "pin-scope").await;
    service
        .publish_post(tenant_id, post_id, admin())
        .await
        .expect("the draft should publish");

    let error = service
        .pin_post(foreign_tenant, post_id, admin())
        .await
        .expect_err("another tenant must not reach the post");

    assert!(
        matches!(error, BlogError::PostNotFound(_)),
        "a foreign tenant sees the post as absent, got {error:?}"
    );
    assert!(
        !post_row(&db, post_id).await.is_pinned,
        "a foreign tenant cannot write pin state"
    );
}

#[tokio::test]
async fn pin_writes_leave_lifecycle_columns_and_version_untouched() {
    let (db, events) = setup_hardening_schema().await;
    let tenant_id = Uuid::new_v4();
    let service = post_service(&db, &events);
    let post_id = create_draft_post(&db, &events, tenant_id, "pin-columns").await;
    service
        .publish_post(tenant_id, post_id, admin())
        .await
        .expect("the draft should publish");
    let published = post_row(&db, post_id).await;

    service
        .pin_post(tenant_id, post_id, admin())
        .await
        .expect("a published post should be pinnable");
    let pinned = post_row(&db, post_id).await;
    assert!(pinned.is_pinned, "the pin flag is written");
    assert_eq!(
        pinned.status, published.status,
        "pin state must not be written through the lifecycle column"
    );
    assert_eq!(
        pinned.version, published.version,
        "a pin is presentation state, not a new revision"
    );
    assert_eq!(
        pinned.published_at, published.published_at,
        "a pin must not rewrite the publication timestamp"
    );
    assert_eq!(
        pinned.archived_at, published.archived_at,
        "a pin must not rewrite the archive timestamp"
    );
    assert_eq!(
        pinned.scheduled_at, published.scheduled_at,
        "a pin must not rewrite the schedule"
    );

    service
        .unpin_post(tenant_id, post_id, admin())
        .await
        .expect("a published post should be releasable");
    let released = post_row(&db, post_id).await;
    assert!(!released.is_pinned, "unpin clears the flag");
    assert_eq!(
        released.status, published.status,
        "unpin must not be written through the lifecycle column either"
    );
    assert_eq!(
        released.version, published.version,
        "unpin is presentation state, not a new revision"
    );
}

/// Lifecycle commands are keyed by their **source** status, not by their target.
///
/// `unpublish` and `restore` both land on `Draft`; before the commands carried
/// their own source status, `unpublish` on an archived post resurrected it and
/// `restore` on a published post silently unpublished it. A command whose post
/// is already in the target state is an idempotent no-op (and emits no second
/// event), so a retried request after a lost response is safe.
#[tokio::test]
async fn lifecycle_commands_are_source_scoped_and_idempotent() {
    let (db, events) = setup_hardening_schema().await;
    let tenant_id = Uuid::new_v4();
    let service = post_service(&db, &events);
    let post_id = create_draft_post(&db, &events, tenant_id, "lifecycle").await;

    // Already in the target state: no-op success, no event.
    service
        .restore_post(tenant_id, post_id, admin())
        .await
        .expect("restore on a draft is an idempotent no-op");
    service
        .unpublish_post(tenant_id, post_id, admin())
        .await
        .expect("unpublish on a draft is an idempotent no-op");

    service
        .publish_post(tenant_id, post_id, admin())
        .await
        .expect("a draft should publish");
    service
        .publish_post(tenant_id, post_id, admin())
        .await
        .expect("publishing a published post is an idempotent no-op");
    assert_eq!(
        outbox_event_count(&db, "blog.post.published").await,
        1,
        "a repeated publish must not announce a second publication"
    );

    let error = service
        .restore_post(tenant_id, post_id, admin())
        .await
        .expect_err("restore must not withdraw a published post");
    assert!(matches!(error, crate::BlogError::CannotRestorePublished));
    assert_status(
        &db,
        post_id,
        BlogPostStatus::Published,
        "a refused restore leaves the post published",
    )
    .await;

    service
        .archive_post(tenant_id, post_id, admin(), None)
        .await
        .expect("a published post should archive");

    let error = service
        .unpublish_post(tenant_id, post_id, admin())
        .await
        .expect_err("unpublish must not resurrect an archived post");
    assert!(matches!(error, crate::BlogError::CannotUnpublishArchived));
    assert_status(
        &db,
        post_id,
        BlogPostStatus::Archived,
        "a refused unpublish leaves the post archived",
    )
    .await;

    service
        .restore_post(tenant_id, post_id, admin())
        .await
        .expect("an archived post restores to draft");
    assert_status(
        &db,
        post_id,
        BlogPostStatus::Draft,
        "restore returns the post to draft",
    )
    .await;
}

/// A post created as published is a publication for every event consumer.
#[tokio::test]
async fn creating_a_published_post_announces_the_publication() {
    let (db, events) = setup_hardening_schema().await;
    let tenant_id = Uuid::new_v4();
    let service = post_service(&db, &events);

    let mut input = post_input("published-at-create");
    input.publish = true;
    service
        .create_post(tenant_id, admin(), input)
        .await
        .expect("a published post should be created");

    assert_eq!(
        outbox_event_count(&db, "blog.post.created").await,
        1,
        "creation is announced"
    );
    assert_eq!(
        outbox_event_count(&db, "blog.post.published").await,
        1,
        "the same transaction announces the publication"
    );
}

/// `blog_tag_usage` is derived state: a lost projection row is repaired by the
/// next relation write instead of failing it with an underflow.
#[tokio::test]
async fn tag_usage_projection_is_recomputed_from_relations() {
    let (db, events) = setup_hardening_schema().await;
    let tenant_id = Uuid::new_v4();
    let service = post_service(&db, &events);

    let mut input = post_input("tag-projection");
    input.tags = vec!["Rust".to_string(), "SeaORM".to_string()];
    let post_id = service
        .create_post(tenant_id, admin(), input)
        .await
        .expect("a tagged post should be created");

    let relations = blog_post_tag::Entity::find()
        .filter(blog_post_tag::Column::TenantId.eq(tenant_id))
        .filter(blog_post_tag::Column::PostId.eq(post_id))
        .all(&db)
        .await
        .expect("tag relations should be readable");
    let tag_ids = relations
        .iter()
        .map(|relation| relation.tag_id)
        .collect::<Vec<_>>();
    assert_eq!(tag_ids.len(), 2, "both tags are linked");

    // Simulate drift: the projection rows disappear while the relations stay.
    blog_tag_usage::Entity::delete_many()
        .filter(blog_tag_usage::Column::TenantId.eq(tenant_id))
        .exec(&db)
        .await
        .expect("the projection rows should be removable");

    let txn = db.begin().await.expect("transaction should open");
    crate::services::replace_post_tag_links_in_tx(&txn, tenant_id, post_id, &tag_ids)
        .await
        .expect("the owner command must repair the projection instead of underflowing");
    txn.commit().await.expect("transaction should commit");

    let usage = blog_tag_usage::Entity::find()
        .filter(blog_tag_usage::Column::TenantId.eq(tenant_id))
        .all(&db)
        .await
        .expect("the projection should be readable");
    assert_eq!(usage.len(), 2, "every linked tag has a projection row");
    assert!(
        usage.iter().all(|row| row.use_count == 1),
        "each tag is counted exactly once"
    );
    assert_eq!(
        blog_tag_usage::Entity::find()
            .filter(blog_tag_usage::Column::TenantId.eq(tenant_id))
            .count(&db)
            .await
            .expect("the projection should be countable"),
        2
    );
}

/// The tag projection is derived state, so the tag's own write path re-derives
/// it: a row whose `canonical_key` no longer matches the Taxonomy owner is
/// repaired by the next tag update instead of staying wrong until the next
/// post save.
///
/// Taxonomy `canonical_key` is immutable identity, so a rename must not move
/// the list ordering; the test asserts both halves — the key matches the owner
/// after the rename, and the count is unchanged.
#[tokio::test]
async fn updating_a_tag_rederives_its_usage_projection_row() {
    let (db, events) = setup_hardening_schema().await;
    let tenant_id = Uuid::new_v4();
    let tags = TagService::new(db.clone());
    let posts = post_service(&db, &events);

    let tag_id = tags
        .create_tag(
            tenant_id,
            admin(),
            CreateTagInput {
                locale: "en".to_string(),
                name: "Zebra".to_string(),
                slug: None,
            },
        )
        .await
        .expect("a tag should be created");
    let mut input = post_input("tag-update-projection");
    input.tags = vec!["Zebra".to_string()];
    posts
        .create_post(tenant_id, admin(), input)
        .await
        .expect("a tagged post should be created");

    // The owner key is read through the Taxonomy owner reader: Blog must not
    // query Taxonomy persistence directly.
    let owner_key = {
        let txn = db.begin().await.expect("transaction should open");
        let terms = rustok_taxonomy::TaxonomyOwnerReader::load_terms_by_ids_in_tx(
            &txn,
            tenant_id,
            rustok_taxonomy::TaxonomyTermKind::Tag,
            &[tag_id],
            "en",
            None,
        )
        .await
        .expect("the owner term should be readable");
        txn.commit().await.expect("transaction should commit");
        terms
            .into_iter()
            .next()
            .expect("the owner term should exist")
            .canonical_key
    };

    // Simulate drift: the projection keeps a stale key while the relations and
    // the owner term are correct.
    blog_tag_usage::Entity::update_many()
        .col_expr(
            blog_tag_usage::Column::CanonicalKey,
            sea_orm::sea_query::Expr::value("stale-key".to_string()),
        )
        .filter(blog_tag_usage::Column::TenantId.eq(tenant_id))
        .filter(blog_tag_usage::Column::TagId.eq(tag_id))
        .exec(&db)
        .await
        .expect("the projection row should be updatable");

    tags.update_tag(
        tenant_id,
        tag_id,
        admin(),
        UpdateTagInput {
            locale: "en".to_string(),
            name: Some("Aardvark".to_string()),
            slug: None,
        },
    )
    .await
    .expect("the tag should be renamed");

    let projection = blog_tag_usage::Entity::find()
        .filter(blog_tag_usage::Column::TenantId.eq(tenant_id))
        .filter(blog_tag_usage::Column::TagId.eq(tag_id))
        .one(&db)
        .await
        .expect("the projection should be readable")
        .expect("the projection row should still exist");
    assert_eq!(
        projection.canonical_key, owner_key,
        "the projection key is re-derived from the Taxonomy owner"
    );
    assert_eq!(
        projection.use_count, 1,
        "the count is re-read from the relation table, not decremented"
    );
}

/// The SEO scan projects only what the SEO port consumes and does not hand a
/// row without a publication timestamp to the sitemap.
///
/// The scan is the input of `sitemap_candidates` and `list_bulk_summaries_page`,
/// so a skip here is a skip in every sitemap and bulk export of the tenant.
#[tokio::test]
async fn seo_scan_skips_a_published_row_without_a_publication_timestamp() {
    let (db, events) = setup_hardening_schema().await;
    let tenant_id = Uuid::new_v4();
    let posts = post_service(&db, &events);

    let mut healthy = post_input("seo-healthy");
    healthy.publish = true;
    let healthy_id = posts
        .create_post(tenant_id, admin(), healthy)
        .await
        .expect("a published post should be created");

    let mut damaged = post_input("seo-damaged");
    damaged.publish = true;
    let damaged_id = posts
        .create_post(tenant_id, admin(), damaged)
        .await
        .expect("a published post should be created");

    // Simulate pre-existing corruption the write path no longer permits: the
    // row is published but carries no publication timestamp.
    blog_post::Entity::update_many()
        .col_expr(
            blog_post::Column::PublishedAt,
            sea_orm::sea_query::Expr::value(Option::<chrono::DateTime<chrono::FixedOffset>>::None),
        )
        .filter(blog_post::Column::Id.eq(damaged_id))
        .exec(&db)
        .await
        .expect("the damaged row should be updatable");

    // Walk the scan one row per page. The damaged row must be skipped *and* the
    // cursor must move past it, otherwise a tenant is stuck on its first broken
    // row forever.
    let mut projected = Vec::new();
    let mut after = None;
    for _ in 0..4 {
        let page = posts
            .scan_published_posts(tenant_id, "en", Some("en"), after, 1)
            .await
            .expect("the scan should not fail on one damaged row");
        projected.extend(page.items.iter().map(|item| item.id));
        match page.next_after {
            Some(next) => after = Some(next),
            None => break,
        }
    }

    assert_eq!(
        projected,
        vec![healthy_id],
        "the row without a publication timestamp is not published to the sitemap, and the \
         healthy row is still reached"
    );
    let page = posts
        .scan_published_posts(tenant_id, "en", Some("en"), None, 50)
        .await
        .expect("the scan should not fail on one damaged row");
    let item = page.items.first().expect("the healthy post is projected");
    assert_eq!(item.slug, "seo-healthy");
    assert_eq!(item.effective_locale, "en");
    assert_eq!(
        item.available_locales,
        vec!["en".to_string()],
        "the projection carries the translated locale set the alternates need"
    );
}
