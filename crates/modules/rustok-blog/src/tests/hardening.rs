//! Regression tests for the Blog hardening work: preview-token lifecycle and
//! route/target-deletion consistency.
//!
//! The harness is the crate's sqlite unit-test harness (`rustok_test_utils`), so no
//! external database is required. Row locking and multi-writer races cannot be proven
//! on sqlite — `SQLiteQueryBuilder` discards `FOR UPDATE` — so the PostgreSQL
//! `comment_projection_*_postgres_test.rs` harness stays the authority for lock
//! behavior.

use std::sync::Arc;

use rustok_core::events::{EventEnvelope, EventHandler};
use rustok_core::{DomainEvent, MigrationSource, SecurityContext, UserRole};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use rustok_test_utils::setup_test_db;
use sea_orm::{ActiveModelTrait, ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use uuid::Uuid;

use crate::dto::CreatePostInput;
use crate::integrations::target_lifecycle::BlogTargetDeletionRouteHandler;
use crate::services::{
    BLOG_POST_TARGET_TYPE, BlogPostRouteOwner, PostService, canonical_post_route,
};

/// The host owns `tenant_modules` outside the module migrations, and the preview read
/// path consults it through `rustok_api::is_tenant_module_enabled`, so the harness
/// creates the table and each test writes the row it needs. No `tenants` table is
/// required: none of the Blog, Taxonomy or outbox migrations declares a foreign key to
/// it, and the harness seeds no channel, which is the only Blog dependency that does.
const TENANT_MODULES_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS tenant_modules (
    tenant_id TEXT NOT NULL,
    module_slug TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    settings TEXT NOT NULL DEFAULT '{}',
    PRIMARY KEY (tenant_id, module_slug)
)
"#;

/// Schema plus the outbox-backed event bus the production paths use, so events land in
/// `sys_events` where a test can assert them.
pub(crate) async fn setup_hardening_schema() -> (DatabaseConnection, TransactionalEventBus) {
    let db = setup_test_db().await;
    db.execute_unprepared(TENANT_MODULES_DDL)
        .await
        .expect("tenant_modules table should exist");

    let manager = SchemaManager::new(&db);
    SysEventsMigration
        .up(&manager)
        .await
        .expect("outbox migration should apply");
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("taxonomy migration should apply");
    }
    for migration in crate::migrations::migrations() {
        migration
            .up(&manager)
            .await
            .expect("blog migration should apply");
    }

    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    (db, event_bus)
}

/// Enables or disables Blog for one tenant, mirroring the host module registry.
pub(crate) async fn set_blog_module_enabled(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    enabled: bool,
) {
    db.execute_unprepared(&format!(
        "INSERT INTO tenant_modules (tenant_id, module_slug, enabled) VALUES ('{tenant_id}', 'blog', {}) \
         ON CONFLICT (tenant_id, module_slug) DO UPDATE SET enabled = {}",
        i32::from(enabled),
        i32::from(enabled),
    ))
    .await
    .expect("tenant module row should be written");
}

pub(crate) fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

pub(crate) fn post_service(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
) -> PostService {
    PostService::new(db.clone(), event_bus.clone())
}

pub(crate) async fn create_draft_post(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    slug: &str,
) -> Uuid {
    post_service(db, event_bus)
        .create_post(tenant_id, admin(), post_input(slug))
        .await
        .expect("draft post should be created")
}

pub(crate) fn post_input(slug: &str) -> CreatePostInput {
    CreatePostInput {
        locale: "en".to_string(),
        title: format!("Post {slug}"),
        content: crate::richtext::article_document_from_plain_text("Body"),
        excerpt: None,
        slug: Some(slug.to_string()),
        publish: false,
        tags: Vec::new(),
        category_id: None,
        featured_image_url: None,
        seo_title: None,
        seo_description: None,
        channel_slugs: None,
        metadata: None,
        scheduled_at: None,
    }
}

/// Writes a Blog route row directly, which is how a stale route reaches the database
/// when its post disappears outside the Blog delete path.
pub(crate) async fn insert_route(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    slug: &str,
    target_kind: &str,
    target_id: Uuid,
) -> String {
    let source_route = canonical_post_route(slug);
    let now = chrono::Utc::now().fixed_offset();
    crate::entities::blog_post_route::ActiveModel {
        tenant_id: sea_orm::Set(tenant_id),
        source_route: sea_orm::Set(source_route.clone()),
        target_kind: sea_orm::Set(target_kind.to_string()),
        target_id: sea_orm::Set(target_id),
        created_at: sea_orm::Set(now),
        updated_at: sea_orm::Set(now),
    }
    .insert(db)
    .await
    .expect("route row should be inserted");
    source_route
}

/// Writes the schedule a post would have, which is how a draft reaches a due state once
/// the clock passes its `scheduled_at`. The owner commands refuse a schedule that is not in
/// the future, so a test that needs a due draft writes the row directly.
pub(crate) async fn set_post_schedule(
    db: &DatabaseConnection,
    post_id: Uuid,
    scheduled_at: Option<chrono::DateTime<chrono::Utc>>,
) {
    let post = crate::entities::blog_post::Entity::find_by_id(post_id)
        .one(db)
        .await
        .expect("post read should not fail")
        .expect("post should exist");
    let mut active: crate::entities::blog_post::ActiveModel = post.into();
    active.scheduled_at = sea_orm::Set(scheduled_at.map(|value| value.fixed_offset()));
    active
        .update(db)
        .await
        .expect("schedule write should not fail");
}

/// Makes a draft due, i.e. scheduled `hours_ago` hours in the past.
pub(crate) async fn make_post_due(db: &DatabaseConnection, post_id: Uuid, hours_ago: i64) {
    set_post_schedule(
        db,
        post_id,
        Some(chrono::Utc::now() - chrono::Duration::hours(hours_ago)),
    )
    .await;
}

pub(crate) async fn outbox_event_count(db: &DatabaseConnection, event_type: &str) -> i64 {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS count FROM sys_events WHERE event_type = ?",
            vec![event_type.to_string().into()],
        ))
        .await
        .expect("outbox count query should run")
        .expect("outbox count query should return one row");
    row.try_get::<i64>("", "count")
        .expect("outbox count query should return a count")
}

/// The route of a deleted target must not resolve, and the row must stay behind for
/// reconciliation instead of being reported as a live redirect.
#[tokio::test]
async fn dangling_blog_route_does_not_resolve_to_a_redirect() {
    let (db, event_bus) = setup_hardening_schema().await;
    let tenant_id = Uuid::new_v4();
    let deleted_target = Uuid::new_v4();
    let source_route = insert_route(
        &db,
        tenant_id,
        "ghost-post",
        BLOG_POST_TARGET_TYPE,
        deleted_target,
    )
    .await;

    let resolved = post_service(&db, &event_bus)
        .get_post_by_slug(tenant_id, admin(), "en", "ghost-post")
        .await
        .expect("slug lookup should not fail on a dangling route");
    assert!(
        resolved.is_none(),
        "a route whose target post is gone must not resolve"
    );

    let kept = BlogPostRouteOwner::find_redirect(&db, tenant_id, &source_route)
        .await
        .expect("route read should not fail");
    assert!(
        kept.is_some(),
        "the dangling row stays for the reconciliation worker to report"
    );
    assert_eq!(
        outbox_event_count(&db, "content.url_alias.purged").await,
        0,
        "a dangling route is not reported as purged by the read path"
    );
}

/// `TargetDeleted` for a Blog post removes its routes and reports them through
/// `UrlAliasPurged`; a repeated delivery is a no-op.
#[tokio::test]
async fn target_deleted_listener_purges_blog_routes_once() {
    let (db, event_bus) = setup_hardening_schema().await;
    let tenant_id = Uuid::new_v4();
    let post_id = create_draft_post(&db, &event_bus, tenant_id, "live-post").await;
    let retired_route = insert_route(
        &db,
        tenant_id,
        "retired-slug",
        BLOG_POST_TARGET_TYPE,
        post_id,
    )
    .await;

    let handler = BlogTargetDeletionRouteHandler::new(db.clone());
    let envelope = EventEnvelope::new(
        tenant_id,
        None,
        DomainEvent::TargetDeleted {
            target_type: BLOG_POST_TARGET_TYPE.to_string(),
            target_id: post_id,
        },
    );

    handler
        .handle(&envelope)
        .await
        .expect("target deletion delivery should succeed");
    assert!(
        BlogPostRouteOwner::find_redirect(&db, tenant_id, &retired_route)
            .await
            .expect("route read")
            .is_none(),
        "the routes of a deleted target must be removed"
    );
    assert_eq!(
        outbox_event_count(&db, "content.url_alias.purged").await,
        1,
        "the removal must be reported through UrlAliasPurged"
    );

    handler
        .handle(&envelope)
        .await
        .expect("repeated target deletion delivery should succeed");
    assert_eq!(
        outbox_event_count(&db, "content.url_alias.purged").await,
        1,
        "a repeated delivery must not publish a second purge event"
    );

    assert!(
        !handler.handles(&DomainEvent::TargetDeleted {
            target_type: "forum_topic".to_string(),
            target_id: post_id,
        }),
        "the listener claims Blog post targets only"
    );
}

/// A route of another owner stays untouched when Blog reports the same id.
#[tokio::test]
async fn target_deleted_listener_keeps_routes_of_other_target_kinds() {
    let (db, event_bus) = setup_hardening_schema().await;
    let tenant_id = Uuid::new_v4();
    let shared_target = Uuid::new_v4();
    let foreign_route =
        insert_route(&db, tenant_id, "forum-slug", "forum_topic", shared_target).await;

    BlogTargetDeletionRouteHandler::new(db.clone())
        .handle(&EventEnvelope::new(
            tenant_id,
            None,
            DomainEvent::TargetDeleted {
                target_type: BLOG_POST_TARGET_TYPE.to_string(),
                target_id: shared_target,
            },
        ))
        .await
        .expect("target deletion delivery should succeed");

    assert!(
        BlogPostRouteOwner::find_redirect(&db, tenant_id, &foreign_route)
            .await
            .expect("route read")
            .is_some(),
        "Blog must not delete routes owned by another target kind"
    );
    assert_eq!(
        outbox_event_count(&db, "content.url_alias.purged").await,
        0,
        "nothing was removed, so nothing is reported"
    );
}
