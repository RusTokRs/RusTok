use chrono::{DateTime, Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;

use crate::entities::blog_preview_token;
use crate::error::{BlogError, BlogResult};
use crate::services::post::PostService;
use crate::services::rbac::enforce_owned_scope;
use crate::PostResponse;
use rustok_outbox::TransactionalEventBus;

/// Default token TTL: 7 days
const DEFAULT_TTL_HOURS: i64 = 168;
/// Maximum token TTL: 30 days
const MAX_TTL_HOURS: i64 = 720;

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

    /// Create a preview token for a draft post.
    ///
    /// The token allows unauthenticated access to the post content for preview
    /// purposes. Tokens have a configurable TTL (default 7 days, max 30 days).
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
        enforce_owned_scope(&security, Resource::BlogPosts, Action::Update, post.author_id)?;

        let ttl = ttl_hours.unwrap_or(DEFAULT_TTL_HOURS).min(MAX_TTL_HOURS);
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

    /// Look up a post by preview token. Returns the post even if it's a Draft.
    /// Returns None if the token is invalid, expired, or the post doesn't exist.
    #[instrument(skip(self))]
    pub async fn get_post_by_token(
        &self,
        token: &str,
        locale: &str,
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

        // Load the post using the system security context (bypasses normal auth)
        let post_service = PostService::new(self.db.clone(), self.event_bus.clone());
        match post_service
            .get_post(
                token_record.tenant_id,
                SecurityContext::system(),
                token_record.post_id,
                locale,
            )
            .await
        {
            Ok(post) => Ok(Some(post)),
            Err(BlogError::PostNotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Revoke a specific preview token.
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
            return Err(BlogError::PostNotFound(token_id));
        };

        // Verify the caller has permission (author or admin)
        let post_service = PostService::new(self.db.clone(), self.event_bus.clone());
        let post = post_service
            .find_post(tenant_id, token_record.post_id)
            .await?;
        enforce_owned_scope(&security, Resource::BlogPosts, Action::Update, post.author_id)?;

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
        enforce_owned_scope(&security, Resource::BlogPosts, Action::Update, post.author_id)?;

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

    /// Clean up expired tokens. Should be called periodically.
    #[instrument(skip(self))]
    pub async fn cleanup_expired(&self) -> BlogResult<u64> {
        let now = Utc::now();
        let result = blog_preview_token::Entity::delete_many()
            .filter(blog_preview_token::Column::ExpiresAt.lt(now))
            .exec(&self.db)
            .await
            .map_err(BlogError::from)?;

        Ok(result.rows_affected)
    }
}

/// Generate a cryptographically random preview token.
/// Format: 32 bytes of randomness, base64url-encoded (43 chars).
fn generate_preview_token() -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

    let bytes: [u8; 32] = rand::random();
    URL_SAFE_NO_PAD.encode(bytes)
}
