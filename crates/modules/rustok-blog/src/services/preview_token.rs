use chrono::{DateTime, Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;

use crate::PostResponse;
use crate::entities::{blog_post, blog_preview_token};
use crate::error::{BlogError, BlogResult};
use crate::services::post::{PostService, storage_to_status};
use crate::services::rbac::enforce_owned_scope;
use crate::state_machine::BlogPostStatus;
use rustok_outbox::TransactionalEventBus;

/// Default token TTL: 7 days
const DEFAULT_TTL_HOURS: i64 = 168;
/// Maximum token TTL: 30 days
const MAX_TTL_HOURS: i64 = 720;
/// Minimum token TTL: one hour. A shorter window is refused instead of being
/// persisted as an already-expired token.
const MIN_TTL_HOURS: i64 = 1;
/// Upper bound for one cleanup iteration, so retention housekeeping stays bounded
/// on a large backlog.
pub(crate) const MAX_PREVIEW_TOKEN_CLEANUP_BATCH: u32 = 500;

/// Upper bound of concurrently usable tokens per post.
///
/// A preview token is a bearer credential minted by an author; minting is not
/// bounded by time, so without a cap a long-lived draft could accumulate an
/// unbounded number of live credentials (and an unbounded `list_tokens`
/// response). The oldest *active* token is revoked when the cap is reached, so
/// the cap never blocks the mint operation itself.
pub(crate) const MAX_ACTIVE_PREVIEW_TOKENS_PER_POST: usize = 20;

/// Upper bound of a token listing, mirroring the active-token cap with room
/// for expired rows that await the cleanup worker.
pub(crate) const MAX_PREVIEW_TOKEN_LIST_LIMIT: u64 = 100;

/// Clamps a caller-supplied cleanup limit into `1..=MAX_PREVIEW_TOKEN_CLEANUP_BATCH`.
///
/// `0` means "one row", never "the whole backlog": the bound is applied to the query
/// itself, so an out-of-range request cannot delete more than one batch.
fn bounded_cleanup_batch(limit: u32) -> u32 {
    limit.clamp(1, MAX_PREVIEW_TOKEN_CLEANUP_BATCH)
}

pub struct PreviewTokenService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

#[derive(Debug, Clone)]
pub struct PreviewToken {
    pub id: Uuid,
    /// The plaintext bearer token. Present exactly once, on the value returned
    /// by `create_token`; token listings never carry it (`None`), because the
    /// database stores only its SHA-256 digest and the secret cannot be
    /// recovered or re-displayed after minting.
    pub token: Option<String>,
    pub post_id: Uuid,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// SHA-256 digest (hex) under which a preview token is stored and looked up.
/// The plaintext token exists only in the create response and in the caller's
/// hands; a database leak therefore does not leak usable preview credentials.
fn preview_token_digest(token: &str) -> String {
    use std::fmt::Write as _;

    let digest = rustok_api::sha256_digest(&[token.as_bytes()]);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}

impl PreviewTokenService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    /// Create a preview token for a **draft** post.
    ///
    /// The token is the whole authorization of the preview read, so its minting is
    /// restricted to unpublished drafts: a published post is already public, and an
    /// archived post must not be reachable through a token minted before archiving.
    /// The TTL must lie in `MIN_TTL_HOURS..=MAX_TTL_HOURS`; an out-of-range request
    /// is rejected instead of being silently stored as already-expired.
    #[instrument(skip(self, security))]
    pub async fn create_token(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        security: SecurityContext,
        ttl_hours: Option<i64>,
    ) -> BlogResult<PreviewToken> {
        if tenant_id.is_nil() {
            return Err(BlogError::validation("Tenant ID must not be nil"));
        }
        if post_id.is_nil() {
            return Err(BlogError::validation("Post ID must not be nil"));
        }
        let ttl = match ttl_hours {
            Some(requested) if !(MIN_TTL_HOURS..=MAX_TTL_HOURS).contains(&requested) => {
                return Err(BlogError::validation(format!(
                    "Preview token TTL must be between {MIN_TTL_HOURS} and {MAX_TTL_HOURS} hours"
                )));
            }
            Some(requested) => requested,
            None => DEFAULT_TTL_HOURS,
        };

        // The draft check and the token insert share one transaction on a
        // row-locked post read. A concurrent lifecycle transition either
        // commits first (this create then observes the non-draft status and
        // refuses) or waits for this transaction and revokes the freshly
        // minted token as part of leaving the draft state. The earlier
        // check-then-insert shape allowed a token minted mid-publish to
        // survive and silently reactivate when the post returned to draft.
        let txn = self.db.begin().await.map_err(BlogError::from)?;
        let post = blog_post::Entity::find_by_id(post_id)
            .filter(blog_post::Column::TenantId.eq(tenant_id))
            .lock_exclusive()
            .one(&txn)
            .await
            .map_err(BlogError::from)?
            .ok_or_else(|| BlogError::post_not_found(post_id))?;
        // Only the post author or users with update permission can create tokens
        enforce_owned_scope(
            &security,
            Resource::BlogPosts,
            Action::Update,
            post.author_id,
        )?;
        if storage_to_status(&post.status)? != BlogPostStatus::Draft {
            return Err(BlogError::validation(
                "Preview tokens can only be created for draft posts",
            ));
        }

        let now = Utc::now();
        let expires_at = now + Duration::hours(ttl);

        // The post row is locked, so this count and the eviction below cannot
        // race another mint of the same post. Expired rows do not consume the
        // cap: they are already dead credentials waiting for the cleanup worker.
        let active = blog_preview_token::Entity::find()
            .filter(blog_preview_token::Column::TenantId.eq(tenant_id))
            .filter(blog_preview_token::Column::PostId.eq(post_id))
            .filter(blog_preview_token::Column::ExpiresAt.gt(now))
            .order_by_asc(blog_preview_token::Column::CreatedAt)
            .all(&txn)
            .await
            .map_err(BlogError::from)?;
        if active.len() >= MAX_ACTIVE_PREVIEW_TOKENS_PER_POST {
            let evict = active.len() + 1 - MAX_ACTIVE_PREVIEW_TOKENS_PER_POST;
            let evicted_ids = active
                .iter()
                .take(evict)
                .map(|token| token.id)
                .collect::<Vec<_>>();
            blog_preview_token::Entity::delete_many()
                .filter(blog_preview_token::Column::TenantId.eq(tenant_id))
                .filter(blog_preview_token::Column::Id.is_in(evicted_ids))
                .exec(&txn)
                .await
                .map_err(BlogError::from)?;
        }

        // Generate a cryptographically random token. Only its SHA-256 digest
        // is persisted; the plaintext is returned exactly once, here.
        let token = generate_preview_token();

        let id = Uuid::new_v4();
        blog_preview_token::ActiveModel {
            id: Set(id),
            tenant_id: Set(tenant_id),
            post_id: Set(post_id),
            token: Set(preview_token_digest(&token)),
            created_by: Set(security.user_id),
            expires_at: Set(expires_at.into()),
            created_at: Set(now.into()),
        }
        .insert(&txn)
        .await
        .map_err(BlogError::from)?;
        txn.commit().await.map_err(BlogError::from)?;

        Ok(PreviewToken {
            id,
            token: Some(token),
            post_id,
            expires_at,
            created_at: now,
        })
    }

    /// Look up a post by preview token.
    ///
    /// Returns the post for a valid, unexpired token of a tenant that still has the
    /// Blog module enabled. The module gate is checked on the token's own tenant,
    /// because an unauthenticated preview request carries no tenant context: without
    /// it a token minted before a tenant disabled Blog would keep serving content.
    /// Returns `None` for an invalid, expired, or module-disabled token, and for a
    /// post that no longer exists.
    #[instrument(skip(self))]
    pub async fn get_post_by_token(
        &self,
        token: &str,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> BlogResult<Option<PostResponse>> {
        self.get_post_by_token_scoped(None, token, locale, fallback_locale)
            .await
    }

    /// GraphQL preview read bound to the *request's* tenant. The token remains
    /// a bearer credential, but it cannot cause the GraphQL tenant-scoped
    /// endpoint to serve a draft belonging to a different tenant.
    pub async fn get_post_by_token_for_tenant(
        &self,
        tenant_id: Uuid,
        token: &str,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> BlogResult<Option<PostResponse>> {
        if tenant_id.is_nil() {
            return Err(BlogError::validation("Tenant ID must not be nil"));
        }
        self.get_post_by_token_scoped(Some(tenant_id), token, locale, fallback_locale)
            .await
    }

    async fn get_post_by_token_scoped(
        &self,
        expected_tenant_id: Option<Uuid>,
        token: &str,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> BlogResult<Option<PostResponse>> {
        let token = token.trim();
        if token.is_empty() || token.len() > 128 {
            return Ok(None);
        }
        let mut lookup = blog_preview_token::Entity::find()
            .filter(blog_preview_token::Column::Token.eq(preview_token_digest(token)));
        if let Some(tenant_id) = expected_tenant_id {
            lookup = lookup.filter(blog_preview_token::Column::TenantId.eq(tenant_id));
        }
        let token_record = lookup.one(&self.db).await.map_err(BlogError::from)?;

        let Some(token_record) = token_record else {
            return Ok(None);
        };

        // Check if token is expired
        let expires_at: DateTime<Utc> = token_record.expires_at.into();
        if expires_at <= Utc::now() {
            return Ok(None);
        }

        match rustok_api::is_tenant_module_enabled(&self.db, token_record.tenant_id, "blog").await {
            Ok(true) => {}
            Ok(false) => {
                tracing::warn!(
                    tenant_id = %token_record.tenant_id,
                    post_id = %token_record.post_id,
                    "Preview token read refused because the tenant no longer has the Blog module enabled"
                );
                return Ok(None);
            }
            Err(error) => return Err(BlogError::from(error)),
        }

        // The token is the authorization of this read, so it is resolved through the
        // draft-only preview accessor rather than through system authority.
        let post_service = PostService::new(self.db.clone(), self.event_bus.clone());
        post_service
            .get_draft_preview(
                token_record.tenant_id,
                token_record.post_id,
                locale,
                fallback_locale,
            )
            .await
    }

    /// Revoke a specific preview token.
    ///
    /// Revoking a token that does not exist for this tenant is reported as a missing
    /// preview token, never as a missing post.
    #[instrument(skip(self, security))]
    pub async fn revoke_token(
        &self,
        tenant_id: Uuid,
        token_id: Uuid,
        security: SecurityContext,
    ) -> BlogResult<()> {
        if tenant_id.is_nil() {
            return Err(BlogError::validation("Tenant ID must not be nil"));
        }
        if token_id.is_nil() {
            return Err(BlogError::validation("Token ID must not be nil"));
        }
        let token_record = blog_preview_token::Entity::find_by_id(token_id)
            .filter(blog_preview_token::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(BlogError::from)?;

        let Some(token_record) = token_record else {
            return Err(BlogError::preview_token_not_found(token_id));
        };

        // Verify the caller has permission (author or admin)
        let post_service = PostService::new(self.db.clone(), self.event_bus.clone());
        let post = post_service
            .find_post(tenant_id, token_record.post_id)
            .await?;
        enforce_owned_scope(
            &security,
            Resource::BlogPosts,
            Action::Update,
            post.author_id,
        )?;

        blog_preview_token::Entity::delete_by_id(token_id)
            .exec(&self.db)
            .await
            .map_err(BlogError::from)?;

        Ok(())
    }

    /// List all preview tokens for a post.
    #[instrument(skip(self, security))]
    pub async fn list_tokens(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        security: SecurityContext,
    ) -> BlogResult<Vec<PreviewToken>> {
        // Verify the caller has permission
        let post_service = PostService::new(self.db.clone(), self.event_bus.clone());
        let post = post_service.find_post(tenant_id, post_id).await?;
        enforce_owned_scope(
            &security,
            Resource::BlogPosts,
            Action::Update,
            post.author_id,
        )?;

        let tokens = blog_preview_token::Entity::find()
            .filter(blog_preview_token::Column::TenantId.eq(tenant_id))
            .filter(blog_preview_token::Column::PostId.eq(post_id))
            .order_by_desc(blog_preview_token::Column::CreatedAt)
            .limit(MAX_PREVIEW_TOKEN_LIST_LIMIT)
            .all(&self.db)
            .await
            .map_err(BlogError::from)?;

        Ok(tokens
            .into_iter()
            .map(|t| PreviewToken {
                id: t.id,
                // Listings identify tokens by id and expiry only. The stored
                // value is a digest and the plaintext is unrecoverable, so a
                // listing must never carry a usable credential.
                token: None,
                post_id: t.post_id,
                expires_at: t.expires_at.into(),
                created_at: t.created_at.into(),
            })
            .collect())
    }

    /// Delete expired tokens of one tenant. Called by the Blog-owned
    /// `blog_preview_token_cleanup` module-work registration, which is tenant-scoped
    /// like every other platform work item; the table also cascades on post deletion,
    /// so this is bounded retention housekeeping rather than the only cleanup path.
    #[instrument(skip(self))]
    pub async fn cleanup_expired(&self, tenant_id: Uuid, limit: u32) -> BlogResult<u64> {
        let now = Utc::now();
        let expired = blog_preview_token::Entity::find()
            .filter(blog_preview_token::Column::TenantId.eq(tenant_id))
            .filter(blog_preview_token::Column::ExpiresAt.lte(now))
            .limit(u64::from(bounded_cleanup_batch(limit)))
            .all(&self.db)
            .await
            .map_err(BlogError::from)?;
        if expired.is_empty() {
            return Ok(0);
        }
        let ids = expired
            .into_iter()
            .map(|token| token.id)
            .collect::<Vec<_>>();
        let result = blog_preview_token::Entity::delete_many()
            .filter(blog_preview_token::Column::Id.is_in(ids))
            .exec(&self.db)
            .await
            .map_err(BlogError::from)?;

        Ok(result.rows_affected)
    }

    /// Count expired preview tokens for one tenant.
    #[instrument(skip(self))]
    pub async fn count_expired(&self, tenant_id: Uuid) -> BlogResult<u64> {
        use sea_orm::PaginatorTrait;
        let now = Utc::now();
        blog_preview_token::Entity::find()
            .filter(blog_preview_token::Column::TenantId.eq(tenant_id))
            .filter(blog_preview_token::Column::ExpiresAt.lte(now))
            .count(&self.db)
            .await
            .map_err(BlogError::from)
    }

    /// List all distinct tenants with expired preview tokens.
    #[instrument(skip(self))]
    pub async fn list_tenants_with_expired_tokens(&self) -> BlogResult<Vec<Uuid>> {
        let now = Utc::now();
        let tokens = blog_preview_token::Entity::find()
            .filter(blog_preview_token::Column::ExpiresAt.lte(now))
            .all(&self.db)
            .await
            .map_err(BlogError::from)?;
        let mut tenants: Vec<Uuid> = tokens.into_iter().map(|t| t.tenant_id).collect();
        tenants.sort_unstable();
        tenants.dedup();
        Ok(tenants)
    }
}

/// Deletes every preview token of a post inside the caller's transaction. A deleted
/// post must not leave a token that still resolves to a preview read.
pub(crate) async fn delete_post_preview_tokens_in_tx(
    txn: &sea_orm::DatabaseTransaction,
    tenant_id: Uuid,
    post_id: Uuid,
) -> BlogResult<u64> {
    let result = blog_preview_token::Entity::delete_many()
        .filter(blog_preview_token::Column::TenantId.eq(tenant_id))
        .filter(blog_preview_token::Column::PostId.eq(post_id))
        .exec(txn)
        .await
        .map_err(BlogError::from)?;
    Ok(result.rows_affected)
}

/// Generate a cryptographically random preview token.
/// Format: 32 bytes of randomness, base64url-encoded (43 chars).
fn generate_preview_token() -> String {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

    let bytes: [u8; 32] = rand::random();
    URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both directions of the cleanup bound are pinned here, because only a pure unit
    /// test can observe the upper clamp without writing `MAX + 1` expired rows.
    #[test]
    fn cleanup_batch_is_clamped_into_the_bounded_range() {
        assert_eq!(bounded_cleanup_batch(0), 1, "zero means one row");
        assert_eq!(bounded_cleanup_batch(1), 1);
        assert_eq!(
            bounded_cleanup_batch(MAX_PREVIEW_TOKEN_CLEANUP_BATCH),
            MAX_PREVIEW_TOKEN_CLEANUP_BATCH
        );
        assert_eq!(
            bounded_cleanup_batch(MAX_PREVIEW_TOKEN_CLEANUP_BATCH + 1),
            MAX_PREVIEW_TOKEN_CLEANUP_BATCH,
            "a larger request is clamped, not honoured"
        );
        assert_eq!(
            bounded_cleanup_batch(u32::MAX),
            MAX_PREVIEW_TOKEN_CLEANUP_BATCH
        );
    }
}
