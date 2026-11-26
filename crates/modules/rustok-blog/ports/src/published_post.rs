use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rustok_api::{PortActorKind, PortContext, PortError, RichTextView};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const BLOG_PUBLISHED_POST_READ_CLAIM: &str = "blog:published_post:read";
pub const MAX_BLOG_POST_SLUG_BYTES: usize = 255;
const MAX_READ_DEADLINE: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReadBlogPublishedPost {
    pub slug: String,
    pub fallback_locale: Option<String>,
}

/// The post detail that the Blog owner has admitted for public display.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlogPublishedPost {
    pub tenant_id: Uuid,
    pub id: Uuid,
    pub author_id: Uuid,
    pub slug: String,
    pub title: String,
    pub requested_locale: String,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
    pub content: RichTextView,
    pub content_plain_text: String,
    pub excerpt: Option<String>,
    pub category_id: Option<Uuid>,
    pub category_name: Option<String>,
    pub tags: Vec<String>,
    pub featured_image_url: Option<String>,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
    pub channel_slugs: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub published_at: DateTime<Utc>,
    pub is_pinned: bool,
    pub pinned_at: Option<DateTime<Utc>>,
    pub version: i32,
}

/// Service adapters must authenticate the caller and bind tenant and channel
/// authority before building this context. Verify remote credentials at the
/// transport boundary before assigning the claim.
pub fn published_post_read_scope(
    context: &PortContext,
    request: &ReadBlogPublishedPost,
) -> Result<(Uuid, Duration), PortError> {
    context.require_read_semantics()?;
    if context.actor.kind != PortActorKind::Service
        || context.actor.id.trim().is_empty()
        || !context
            .claims
            .iter()
            .any(|claim| claim == BLOG_PUBLISHED_POST_READ_CLAIM)
    {
        return Err(PortError::forbidden(
            "blog.published_post_forbidden",
            "Blog public post access is denied",
        ));
    }
    let tenant_id = Uuid::parse_str(&context.tenant_id)
        .ok()
        .filter(|id| !id.is_nil())
        .ok_or_else(|| {
            PortError::validation(
                "blog.published_post_tenant_invalid",
                "Blog public reads require a tenant",
            )
        })?;
    let deadline = Duration::from_millis(context.deadline_ms.unwrap_or_default());
    if deadline > MAX_READ_DEADLINE || request.slug.trim().len() > MAX_BLOG_POST_SLUG_BYTES {
        return Err(PortError::validation(
            "blog.published_post_request_invalid",
            "Blog public read exceeds its request bounds",
        ));
    }
    Ok((tenant_id, deadline))
}

/// Reads an admitted published post by its current slug or a historical route.
/// Missing posts, drafts and channel-hidden posts produce the same absence.
#[async_trait]
pub trait BlogPublishedPostReadPort: Send + Sync {
    async fn read_published_post(
        &self,
        context: PortContext,
        request: ReadBlogPublishedPost,
    ) -> Result<Option<BlogPublishedPost>, PortError>;
}
