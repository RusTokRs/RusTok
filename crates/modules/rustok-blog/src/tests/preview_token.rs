//! Preview-token lifecycle regressions: TTL bounds, draft-only minting, revocation,
//! and tenant-scoped retention.

use chrono::{Duration, Utc};
use rustok_outbox::TransactionalEventBus;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, Set,
};
use uuid::Uuid;

use super::hardening::{
    admin, create_draft_post, post_service, set_blog_module_enabled, setup_hardening_schema,
};
use crate::BlogError;
use crate::entities::blog_preview_token;
use crate::services::PreviewTokenService;
use crate::services::preview_token::MAX_PREVIEW_TOKEN_CLEANUP_BATCH;

/// The length bound the service enforces; the constant lives in the service module.
const MAX_TTL_HOURS: i64 = 720;

fn token_service(db: &DatabaseConnection, event_bus: &TransactionalEventBus) -> PreviewTokenService {
    PreviewTokenService::new(db.clone(), event_bus.clone())
}

/// Writes a token that is already expired, which the service can no longer mint.
async fn insert_expired_token(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    post_id: Uuid,
    hours_ago: i64,
) -> Uuid {
    let id = Uuid::new_v4();
    let created = Utc::now() - Duration::hours(hours_ago);
    blog_preview_token::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        post_id: Set(post_id),
        token: Set(format!("expired-{id}")),
        created_by: Set(None),
        expires_at: Set(created.into()),
        created_at: Set(created.into()),
    }
    .insert(db)
    .await
    .expect("expired token should be inserted");
    id
}

async fn token_rows(db: &DatabaseConnection, tenant_id: Uuid) -> u64 {
    blog_preview_token::Entity::find()
        .filter(blog_preview_token::Column::TenantId.eq(tenant_id))
        .count(db)
        .await
        .expect("token count should be readable")
}

#[tokio::test]
async fn preview_token_ttl_bounds_reject_zero_and_values_above_the_maximum() {
    let (db, event_bus) = setup_hardening_schema().await;
    let tenant_id = Uuid::new_v4();
    set_blog_module_enabled(&db, tenant_id, true).await;
    let post_id = create_draft_post(&db, &event_bus, tenant_id, "ttl-bounds").await;
    let service = token_service(&db, &event_bus);

    for requested in [0_i64, -1, MAX_TTL_HOURS + 1] {
        let error = service
            .create_token(tenant_id, post_id, admin(), Some(requested))
            .await
            .expect_err("an out-of-range TTL must be refused");
        assert!(
            matches!(error, BlogError::Validation(_)),
            "TTL {requested} must be reported as a validation error, got {error:?}"
        );
    }

    // The bounds themselves are accepted, so the refusal above is the range and not
    // an accidental off-by-one.
    service
        .create_token(tenant_id, post_id, admin(), Some(1))
        .await
        .expect("the minimum TTL must be accepted");
    service
        .create_token(tenant_id, post_id, admin(), Some(MAX_TTL_HOURS))
        .await
        .expect("the maximum TTL must be accepted");
}

#[tokio::test]
async fn preview_token_minting_is_draft_only() {
    let (db, event_bus) = setup_hardening_schema().await;
    let tenant_id = Uuid::new_v4();
    set_blog_module_enabled(&db, tenant_id, true).await;
    let post_id = create_draft_post(&db, &event_bus, tenant_id, "draft-only").await;
    let service = token_service(&db, &event_bus);
    let posts = post_service(&db, &event_bus);

    service
        .create_token(tenant_id, post_id, admin(), None)
        .await
        .expect("a draft may be previewed");

    posts
        .publish_post(tenant_id, post_id, admin())
        .await
        .expect("the draft should publish");
    let published = service
        .create_token(tenant_id, post_id, admin(), None)
        .await
        .expect_err("a published post must not mint a preview token");
    assert!(matches!(published, BlogError::Validation(_)));

    posts
        .archive_post(tenant_id, post_id, admin(), None)
        .await
        .expect("the post should archive");
    let archived = service
        .create_token(tenant_id, post_id, admin(), None)
        .await
        .expect_err("an archived post must not mint a preview token");
    assert!(matches!(archived, BlogError::Validation(_)));
}

#[tokio::test]
async fn preview_token_reads_stop_after_revocation_and_expiry() {
    let (db, event_bus) = setup_hardening_schema().await;
    let tenant_id = Uuid::new_v4();
    let foreign_tenant = Uuid::new_v4();
    set_blog_module_enabled(&db, tenant_id, true).await;
    set_blog_module_enabled(&db, foreign_tenant, true).await;
    let post_id = create_draft_post(&db, &event_bus, tenant_id, "revoked").await;
    let service = token_service(&db, &event_bus);

    let token = service
        .create_token(tenant_id, post_id, admin(), None)
        .await
        .expect("a draft may be previewed");
    let preview = service
        .get_post_by_token(&token.token, "en", None)
        .await
        .expect("token read should not fail")
        .expect("a live token must resolve");
    assert_eq!(preview.id, post_id);

    service
        .revoke_token(tenant_id, token.id, admin())
        .await
        .expect("revocation should succeed");
    assert!(
        service
            .get_post_by_token(&token.token, "en", None)
            .await
            .expect("token read should not fail")
            .is_none(),
        "a revoked token must stop resolving"
    );

    // A missing token is reported as a missing token, and another tenant cannot reach
    // this tenant's token ids.
    for owner in [tenant_id, foreign_tenant] {
        let error = service
            .revoke_token(owner, token.id, admin())
            .await
            .expect_err("revoking a missing token must fail");
        assert!(matches!(error, BlogError::PreviewTokenNotFound(id) if id == token.id));
    }

    let expired_id = insert_expired_token(&db, tenant_id, post_id, 2).await;
    assert!(
        service
            .get_post_by_token(&format!("expired-{expired_id}"), "en", None)
            .await
            .expect("token read should not fail")
            .is_none(),
        "an expired token must stop resolving"
    );
}

#[tokio::test]
async fn preview_token_read_fails_closed_when_blog_is_disabled_for_the_tenant() {
    let (db, event_bus) = setup_hardening_schema().await;
    let tenant_id = Uuid::new_v4();
    set_blog_module_enabled(&db, tenant_id, true).await;
    let post_id = create_draft_post(&db, &event_bus, tenant_id, "gate").await;
    let service = token_service(&db, &event_bus);

    let token = service
        .create_token(tenant_id, post_id, admin(), None)
        .await
        .expect("a draft may be previewed");
    assert!(
        service
            .get_post_by_token(&token.token, "en", None)
            .await
            .expect("token read should not fail")
            .is_some(),
        "the enabled tenant resolves the token"
    );

    set_blog_module_enabled(&db, tenant_id, false).await;
    assert!(
        service
            .get_post_by_token(&token.token, "en", None)
            .await
            .expect("token read should not fail")
            .is_none(),
        "a disabled tenant must not keep serving previews from minted tokens"
    );
}

#[tokio::test]
async fn preview_token_cleanup_is_tenant_scoped_and_bounded() {
    let (db, event_bus) = setup_hardening_schema().await;
    let tenant_id = Uuid::new_v4();
    let other_tenant = Uuid::new_v4();
    set_blog_module_enabled(&db, tenant_id, true).await;
    set_blog_module_enabled(&db, other_tenant, true).await;
    let post_id = create_draft_post(&db, &event_bus, tenant_id, "cleanup").await;
    let other_post_id = create_draft_post(&db, &event_bus, other_tenant, "cleanup-other").await;
    let service = token_service(&db, &event_bus);

    let live = service
        .create_token(tenant_id, post_id, admin(), Some(1))
        .await
        .expect("a draft may be previewed");
    insert_expired_token(&db, tenant_id, post_id, 3).await;
    insert_expired_token(&db, tenant_id, post_id, 4).await;
    insert_expired_token(&db, other_tenant, other_post_id, 3).await;

    // A zero limit is clamped to one row, so housekeeping stays bounded instead of
    // deleting the whole backlog in one call.
    assert_eq!(
        service
            .cleanup_expired(tenant_id, 0)
            .await
            .expect("cleanup should run"),
        1
    );
    assert_eq!(
        service
            .cleanup_expired(tenant_id, MAX_PREVIEW_TOKEN_CLEANUP_BATCH)
            .await
            .expect("cleanup should run"),
        1
    );
    assert_eq!(
        service
            .cleanup_expired(tenant_id, MAX_PREVIEW_TOKEN_CLEANUP_BATCH)
            .await
            .expect("cleanup should run"),
        0,
        "the tenant backlog is drained"
    );
    assert!(
        service
            .get_post_by_token(&live.token, "en", None)
            .await
            .expect("token read should not fail")
            .is_some(),
        "cleanup must not touch unexpired tokens"
    );
    assert_eq!(
        token_rows(&db, other_tenant).await,
        1,
        "another tenant's expired tokens are untouched"
    );
    assert_eq!(
        service
            .cleanup_expired(other_tenant, MAX_PREVIEW_TOKEN_CLEANUP_BATCH)
            .await
            .expect("cleanup should run"),
        1,
        "each tenant drains its own backlog"
    );
}
