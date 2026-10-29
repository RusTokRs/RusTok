use super::*;
use crate::services::status_to_storage;
use crate::tests::hardening::{
    create_draft_post, make_post_due, set_post_schedule, setup_hardening_schema,
};

fn adapter(
    db: &DatabaseConnection,
    events: &TransactionalEventBus,
) -> BlogScheduledPublishWorkAdapter {
    BlogScheduledPublishWorkAdapter::new(db.clone(), events.clone())
}

/// The stored status of a post row, read from the owner table.
async fn post_status(db: &DatabaseConnection, post_id: Uuid) -> String {
    blog_post::Entity::find_by_id(post_id)
        .one(db)
        .await
        .expect("post read should not fail")
        .expect("post should exist")
        .status
}

/// The claim is a read-only scan of due drafts, so the worker must hand the host the
/// tenant whose schedule is oldest and never a tenant without due work.
#[tokio::test]
async fn claim_selects_the_tenant_of_the_earliest_due_draft_and_drains_it() {
    let (db, events) = setup_hardening_schema().await;
    let earlier_tenant = Uuid::new_v4();
    let later_tenant = Uuid::new_v4();

    let later = create_draft_post(&db, &events, later_tenant, "due-later").await;
    make_post_due(&db, later, 1).await;
    let earlier = create_draft_post(&db, &events, earlier_tenant, "due-earlier").await;
    make_post_due(&db, earlier, 3).await;

    let adapter = adapter(&db, &events);
    let first = adapter
        .claim(BLOG_SCHEDULED_PUBLISH_WORKER)
        .await
        .expect("claim should not fail")
        .expect("a due draft must be claimable");
    assert_eq!(
        first.tenant_id, earlier_tenant,
        "the tenant with the earliest due schedule is claimed first"
    );
    assert!(
        matches!(
            adapter
                .execute(first)
                .await
                .expect("the scheduled publish should run"),
            ModuleWorkOutcome::Completed
        ),
        "the worker reports a completed item"
    );
    assert_eq!(
        post_status(&db, earlier).await,
        status_to_storage(BlogPostStatus::Published),
        "the claimed tenant's due draft is published"
    );
    assert_eq!(
        post_status(&db, later).await,
        status_to_storage(BlogPostStatus::Draft),
        "another tenant's due draft is untouched"
    );

    let second = adapter
        .claim(BLOG_SCHEDULED_PUBLISH_WORKER)
        .await
        .expect("claim should not fail")
        .expect("the remaining due tenant must be claimable");
    assert_eq!(
        second.tenant_id, later_tenant,
        "the drained tenant is not claimed again"
    );
    adapter
        .execute(second)
        .await
        .expect("the second scheduled publish should run");
    assert_eq!(
        post_status(&db, later).await,
        status_to_storage(BlogPostStatus::Published),
        "the second tenant's due draft is published by its own claim"
    );

    assert!(
        adapter
            .claim(BLOG_SCHEDULED_PUBLISH_WORKER)
            .await
            .expect("claim should not fail")
            .is_none(),
        "a drained queue claims nothing"
    );
}

/// Tenant work remains claimable even when an older tenant never makes
/// progress. Claims are hints, so the first item is deliberately not
/// executed before asking for the second one.
#[tokio::test]
async fn scheduled_claim_rotates_tenants_when_the_oldest_stays_due() {
    let (db, events) = setup_hardening_schema().await;
    let oldest_tenant = Uuid::new_v4();
    let other_tenant = Uuid::new_v4();
    let oldest = create_draft_post(&db, &events, oldest_tenant, "oldest-stuck").await;
    let other = create_draft_post(&db, &events, other_tenant, "other-still-due").await;
    make_post_due(&db, oldest, 3).await;
    make_post_due(&db, other, 1).await;

    let adapter = adapter(&db, &events);
    let first = adapter
        .claim(BLOG_SCHEDULED_PUBLISH_WORKER)
        .await
        .expect("claim must succeed")
        .expect("oldest tenant has work");
    assert_eq!(first.tenant_id, oldest_tenant);

    let second = adapter
        .claim(BLOG_SCHEDULED_PUBLISH_WORKER)
        .await
        .expect("second claim must succeed")
        .expect("second tenant must not be starved");
    assert_eq!(second.tenant_id, other_tenant);

    let third = adapter
        .claim(BLOG_SCHEDULED_PUBLISH_WORKER)
        .await
        .expect("third claim must succeed")
        .expect("cycle must wrap back to remaining due work");
    assert_eq!(third.tenant_id, oldest_tenant);
}

/// The oldest due draft can be permanently invalid. The command must walk
/// beyond the first fixed-size page rather than retrying that row forever.
#[tokio::test]
async fn failed_due_draft_does_not_starve_a_later_healthy_draft() {
    let (db, events) = setup_hardening_schema().await;
    let tenant_id = Uuid::new_v4();
    let invalid = create_draft_post(&db, &events, tenant_id, "poison-due").await;
    let healthy = create_draft_post(&db, &events, tenant_id, "healthy-due").await;
    make_post_due(&db, invalid, 3).await;
    make_post_due(&db, healthy, 1).await;

    // A legacy/corrupt exhausted revision cannot be published: the owner
    // transition uses checked arithmetic, and must not wrap the version.
    blog_post::Entity::update_many()
        .col_expr(blog_post::Column::Version, Expr::value(i32::MAX))
        .filter(blog_post::Column::Id.eq(invalid))
        .exec(&db)
        .await
        .expect("invalid post revision fixture should be stored");

    let published = PostService::new(db.clone(), events.clone())
        .publish_scheduled_posts(tenant_id, 1)
        .await
        .expect("a failed row must not abort the batch");
    assert_eq!(published, vec![healthy]);
    assert_eq!(
        post_status(&db, healthy).await,
        status_to_storage(BlogPostStatus::Published)
    );
    assert_eq!(
        post_status(&db, invalid).await,
        status_to_storage(BlogPostStatus::Draft),
        "the poisoned post stays due for explicit repair/retry"
    );
}

/// A schedule in the future is not due work, and another Blog worker's slug is not this
/// adapter's source.
#[tokio::test]
async fn claim_ignores_future_schedules_and_foreign_worker_slugs() {
    let (db, events) = setup_hardening_schema().await;
    let tenant_id = Uuid::new_v4();
    let post_id = create_draft_post(&db, &events, tenant_id, "future-only").await;
    set_post_schedule(
        &db,
        post_id,
        Some(chrono::Utc::now() + chrono::Duration::hours(2)),
    )
    .await;

    let adapter = adapter(&db, &events);
    assert!(
        adapter
            .claim(BLOG_SCHEDULED_PUBLISH_WORKER)
            .await
            .expect("claim should not fail")
            .is_none(),
        "a schedule in the future is not due work"
    );
    assert!(
        adapter
            .claim(BLOG_ROUTE_RECONCILIATION_WORKER)
            .await
            .expect("claim should not fail")
            .is_none(),
        "another Blog worker's slug is not this worker's work"
    );

    make_post_due(&db, post_id, 1).await;
    let item = adapter
        .claim(BLOG_SCHEDULED_PUBLISH_WORKER)
        .await
        .expect("claim should not fail")
        .expect("a due draft becomes claimable once its schedule passes");
    assert_eq!(item.tenant_id, tenant_id);
}
