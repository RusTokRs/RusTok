use super::*;
use crate::services::canonical_post_route;
use crate::tests::hardening::{
    create_draft_post, insert_route, outbox_event_count, setup_hardening_schema,
};

fn adapter(
    db: &DatabaseConnection,
    events: &TransactionalEventBus,
) -> BlogRouteReconciliationWorkAdapter {
    BlogRouteReconciliationWorkAdapter::new(db.clone(), events.clone())
}

#[tokio::test]
async fn reconciliation_removes_stale_routes_and_then_stops_claiming() {
    let (db, events) = setup_hardening_schema().await;
    let tenant_id = Uuid::new_v4();
    let ghost_target = Uuid::new_v4();
    insert_route(
        &db,
        tenant_id,
        "ghost-reconcile",
        BLOG_POST_TARGET_TYPE,
        ghost_target,
    )
    .await;

    let adapter = adapter(&db, &events);
    let item = adapter
        .claim(BLOG_ROUTE_RECONCILIATION_WORKER)
        .await
        .expect("claim should not fail")
        .expect("a stale route must be claimable");
    assert_eq!(item.tenant_id, tenant_id);

    let outcome = adapter
        .execute(item)
        .await
        .expect("reconciliation should run");
    assert!(matches!(outcome, ModuleWorkOutcome::Completed));
    assert!(
        BlogPostRouteOwner::find_redirect(&db, tenant_id, &canonical_post_route("ghost-reconcile"))
            .await
            .expect("route read")
            .is_none(),
        "reconciliation must remove the stale route"
    );
    assert_eq!(
        outbox_event_count(&db, "content.url_alias.purged").await,
        1,
        "reconciliation must report the purged route"
    );

    assert!(
        adapter
            .claim(BLOG_ROUTE_RECONCILIATION_WORKER)
            .await
            .expect("claim should not fail")
            .is_none(),
        "an empty stale set is not a work item"
    );
    assert_eq!(
        outbox_event_count(&db, "content.url_alias.purged").await,
        1,
        "a second run must not publish again"
    );

    // A route whose target still exists belongs to no one: it must be kept.
    let post_id = create_draft_post(&db, &events, tenant_id, "live-reconcile").await;
    insert_route(&db, tenant_id, "live-route", BLOG_POST_TARGET_TYPE, post_id).await;
    assert!(
        adapter
            .claim(BLOG_ROUTE_RECONCILIATION_WORKER)
            .await
            .expect("claim should not fail")
            .is_none()
    );
    assert!(
        BlogPostRouteOwner::find_redirect(&db, tenant_id, &canonical_post_route("live-route"))
            .await
            .expect("route read")
            .is_some(),
        "a live target keeps its route"
    );
}

#[tokio::test]
async fn reconciliation_claims_only_its_own_worker_slug() {
    let (db, events) = setup_hardening_schema().await;
    let adapter = adapter(&db, &events);

    assert!(
        adapter
            .claim(BLOG_SCHEDULED_PUBLISH_WORKER)
            .await
            .expect("claim should not fail")
            .is_none(),
        "another Blog worker's slug is not this worker's work"
    );
    assert!(
        adapter
            .claim(BLOG_ROUTE_RECONCILIATION_WORKER)
            .await
            .expect("claim should not fail")
            .is_none(),
        "a schema without stale routes claims nothing"
    );
}
