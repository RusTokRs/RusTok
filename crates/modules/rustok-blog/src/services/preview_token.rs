use chrono::{DateTime, Duration, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;

use crate::PostResponse;
use crate::entities::blog_preview_token;
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

pub struct PreviewTokenService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

#[derive(Debug, Clone)]
pub struct PreviewToken {
    pub id: Uuid,
    pub token: String,
    pub post_id: Uuid,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
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
    /// The TTL is clamped to `MIN_TTL_HOURS..=MAX_TTL_HOURS`; an out-of-range request
    /// is rejected instead of being silently stored as already-expired.
    #[instrument(skip(self, security))]
    pub async fn create_token(
        &self,
        tenant_id: Uuid,
        post_id: Uuid,
        security: SecurityContext,
        ttl_hours: Option<i64>,
    ) -> BlogResult<PreviewToken> {
        // Only the post author or users with update permission can create tokens
        let post_service = PostService::new(self.db.clone(), self.event_bus.clone());
        let post = post_service.find_post(tenant_id, post_id).await?;
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

        let ttl = match ttl_hours {
            Some(requested) if !(MIN_TTL_HOURS..=MAX_TTL_HOURS).contains(&requested) => {
                return Err(BlogError::validation(format!(
                    "Preview token TTL must be between {MIN_TTL_HOURS} and {MAX_TTL_HOURS} hours"
                )));
            }
            Some(requested) => requested,
            None => DEFAULT_TTL_HOURS,
        };
        let now = Utc::now();
        let expires_at = now + Duration::hours(ttl);

        // Generate a cryptographically random token
        let token = generate_preview_token();

        let id = Uuid::new_v4();
        let model = blog_preview_token::ActiveModel {
            id: Set(id),
            tenant_id: Set(tenant_id),
            post_id: Set(post_id),
            token: Set(token.clone()),
            created_by: Set(security.user_id),
            expires_at: Set(expires_at.into()),
            created_at: Set(now.into()),
        };

        model.insert(&self.db).await.map_err(BlogError::from)?;

        Ok(PreviewToken {
            id,
            token,
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
        let token_record = blog_preview_token::Entity::find()
            .filter(blog_preview_token::Column::Token.eq(token))
            .one(&self.db)
            .await
            .map_err(BlogError::from)?;

        let Some(token_record) = token_record else {
            return Ok(None);
        };

        // Check if token is expired
        let expires_at: DateTime<Utc> = token_record.expires_at.into();
        if expires_at < Utc::now() {
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
            .all(&self.db)
            .await
            .map_err(BlogError::from)?;

        Ok(tokens
            .into_iter()
            .map(|t| PreviewToken {
                id: t.id,
                token: t.token,
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
            .filter(blog_preview_token::Column::ExpiresAt.lt(now))
            .limit(u64::from(limit.clamp(1, MAX_PREVIEW_TOKEN_CLEANUP_BATCH)))
            .all(&self.db)
            .await
            .map_err(BlogError::from)?;
        if expired.is_empty() {
            return Ok(0);
        }
        let ids = expired.into_iter().map(|token| token.id).collect::<Vec<_>>();
        let result = blog_preview_token::Entity::delete_many()
            .filter(blog_preview_token::Column::Id.is_in(ids))
            .exec(&self.db)
            .await
            .map_err(BlogError::from)?;

        Ok(result.rows_affected)
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
