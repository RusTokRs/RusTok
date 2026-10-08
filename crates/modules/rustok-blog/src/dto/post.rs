use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, FixedOffset, SecondsFormat, Utc};
use rustok_api::{Patch, RichTextDocument, RichTextView};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::error::{BlogError, BlogResult};
use crate::state_machine::BlogPostStatus;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreatePostInput {
    pub locale: String,
    #[schema(max_length = 512)]
    pub title: String,
    pub content: RichTextDocument,
    #[schema(max_length = 1000)]
    pub excerpt: Option<String>,
    #[schema(max_length = 255)]
    pub slug: Option<String>,
    pub publish: bool,
    #[schema(max_items = 20)]
    pub tags: Vec<String>,
    pub category_id: Option<Uuid>,
    pub featured_image_url: Option<String>,
    #[schema(max_length = 255)]
    pub seo_title: Option<String>,
    #[schema(max_length = 1000)]
    pub seo_description: Option<String>,
    pub channel_slugs: Option<Vec<String>>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdatePostInput {
    pub locale: Option<String>,
    #[schema(max_length = 512)]
    pub title: Option<String>,
    pub content: Option<RichTextDocument>,
    #[serde(default, skip_serializing_if = "Patch::is_keep")]
    #[schema(value_type = Option<String>, max_length = 1000)]
    pub excerpt: Patch<String>,
    #[schema(max_length = 255)]
    pub slug: Option<String>,
    #[schema(max_items = 20)]
    pub tags: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Patch::is_keep")]
    #[schema(value_type = Option<Uuid>)]
    pub category_id: Patch<Uuid>,
    #[serde(default, skip_serializing_if = "Patch::is_keep")]
    #[schema(value_type = Option<String>)]
    pub featured_image_url: Patch<String>,
    #[serde(default, skip_serializing_if = "Patch::is_keep")]
    #[schema(value_type = Option<String>, max_length = 255)]
    pub seo_title: Patch<String>,
    #[serde(default, skip_serializing_if = "Patch::is_keep")]
    #[schema(value_type = Option<String>, max_length = 1000)]
    pub seo_description: Patch<String>,
    pub channel_slugs: Option<Vec<String>>,
    pub metadata: Option<Value>,
    pub version: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PostResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub author_id: Uuid,
    pub title: String,
    pub slug: String,
    pub requested_locale: String,
    pub locale: String,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
    pub content: RichTextView,
    pub content_plain_text: String,
    pub excerpt: Option<String>,
    pub status: BlogPostStatus,
    pub category_id: Option<Uuid>,
    pub category_name: Option<String>,
    pub tags: Vec<String>,
    pub featured_image_url: Option<String>,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
    pub channel_slugs: Vec<String>,
    pub metadata: Value,
    pub comment_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// First publication time. Set on the first publish and kept across
    /// unpublish, archive and restore; `None` only if the post was never published.
    pub published_at: Option<DateTime<Utc>>,
    pub version: i32,
}

#[cfg(test)]
mod tests {
    use super::CreatePostInput;
    use rustok_api::RichTextDocument;

    #[test]
    fn create_post_input_serde_requires_canonical_document() {
        let input = CreatePostInput {
            locale: "en".to_string(),
            title: "Title".to_string(),
            content: RichTextDocument::single_paragraph("Body"),
            excerpt: None,
            slug: None,
            publish: false,
            tags: Vec::new(),
            category_id: None,
            featured_image_url: None,
            seo_title: None,
            seo_description: None,
            channel_slugs: None,
            metadata: None,
        };
        let encoded = serde_json::to_value(input).expect("serialize");
        assert_eq!(encoded["content"]["type"], "doc");
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PostSummary {
    pub id: Uuid,
    pub title: String,
    pub slug: String,
    pub locale: String,
    pub effective_locale: String,
    #[serde(default)]
    pub available_locales: Vec<String>,
    pub excerpt: Option<String>,
    pub status: BlogPostStatus,
    pub author_id: Uuid,
    pub author_name: Option<String>,
    pub category_id: Option<Uuid>,
    pub category_name: Option<String>,
    pub tags: Vec<String>,
    pub featured_image_url: Option<String>,
    pub channel_slugs: Vec<String>,
    pub comment_count: i64,
    /// First publication time. Set on the first publish and kept across
    /// unpublish, archive and restore; `None` only if the post was never published.
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PostSortField {
    PublishedAt,
    UpdatedAt,
    #[default]
    CreatedAt,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PostSortOrder {
    Asc,
    #[default]
    Desc,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, IntoParams, Default)]
pub struct PostListQuery {
    pub status: Option<BlogPostStatus>,
    pub category_id: Option<Uuid>,
    pub tag: Option<String>,
    pub author_id: Option<Uuid>,
    pub locale: Option<String>,
    /// Opaque cursor: `next_cursor` of the previous page. Absent for the first page.
    pub after: Option<String>,
    #[schema(default = 20, maximum = 100)]
    pub per_page: Option<u32>,
    pub sort_by: Option<PostSortField>,
    pub sort_order: Option<PostSortOrder>,
}

impl PostListQuery {
    pub fn per_page(&self) -> u32 {
        self.per_page.unwrap_or(20).clamp(1, 100)
    }

    pub fn sort(&self) -> (PostSortField, PostSortOrder) {
        (
            self.sort_by.unwrap_or_default(),
            self.sort_order.unwrap_or_default(),
        )
    }
}

/// One page of an admin post list. The list never counts rows; `next_cursor` is
/// present only when another page exists.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PostListResponse {
    pub items: Vec<PostSummary>,
    pub next_cursor: Option<String>,
    pub per_page: u32,
}

impl PostListResponse {
    pub fn new(items: Vec<PostSummary>, next_cursor: Option<String>, per_page: u32) -> Self {
        Self {
            items,
            next_cursor,
            per_page,
        }
    }
}

/// Keyset position of an admin post list: the sort value and id of the last item of
/// the previous page. Posts whose sort column is NULL (drafts have no `published_at`)
/// come after every non-null value, so their cursor carries `value: None`.
/// The sort field and order are part of the token, so a cursor cannot be replayed
/// against a different sort.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdminPostCursor {
    pub sort_by: PostSortField,
    pub sort_order: PostSortOrder,
    pub value: Option<DateTime<FixedOffset>>,
    pub id: Uuid,
}

impl AdminPostCursor {
    /// Opaque, URL-safe token. Clients must not parse it.
    pub fn encode(&self) -> String {
        let value = self.value.map_or_else(
            || "-".to_string(),
            |value| value.to_rfc3339_opts(SecondsFormat::Nanos, false),
        );
        let raw = format!(
            "{}|{}|{}|{}",
            sort_field_token(self.sort_by),
            sort_order_token(self.sort_order),
            value,
            self.id
        );
        URL_SAFE_NO_PAD.encode(raw)
    }

    /// Decodes a token and checks that it was issued for the same sort as `sort_by`/`sort_order`.
    pub fn decode(
        token: &str,
        sort_by: PostSortField,
        sort_order: PostSortOrder,
    ) -> BlogResult<Self> {
        let invalid = || BlogError::validation("Blog list cursor is invalid");
        let bytes = URL_SAFE_NO_PAD.decode(token.trim()).map_err(|_| invalid())?;
        let raw = String::from_utf8(bytes).map_err(|_| invalid())?;
        let mut parts = raw.split('|');
        let (Some(field), Some(order), Some(value), Some(id), None) = (
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
        ) else {
            return Err(invalid());
        };
        if parse_sort_field(field).ok_or_else(invalid)? != sort_by
            || parse_sort_order(order).ok_or_else(invalid)? != sort_order
        {
            return Err(BlogError::validation(
                "Blog list cursor does not match the requested sort",
            ));
        }
        let value = if value == "-" {
            None
        } else {
            Some(DateTime::parse_from_rfc3339(value).map_err(|_| invalid())?)
        };
        Ok(Self {
            sort_by,
            sort_order,
            value,
            id: Uuid::parse_str(id).map_err(|_| invalid())?,
        })
    }
}

fn sort_field_token(field: PostSortField) -> &'static str {
    match field {
        PostSortField::PublishedAt => "published_at",
        PostSortField::UpdatedAt => "updated_at",
        PostSortField::CreatedAt => "created_at",
    }
}

fn parse_sort_field(token: &str) -> Option<PostSortField> {
    match token {
        "published_at" => Some(PostSortField::PublishedAt),
        "updated_at" => Some(PostSortField::UpdatedAt),
        "created_at" => Some(PostSortField::CreatedAt),
        _ => None,
    }
}

fn sort_order_token(order: PostSortOrder) -> &'static str {
    match order {
        PostSortOrder::Asc => "asc",
        PostSortOrder::Desc => "desc",
    }
}

fn parse_sort_order(token: &str) -> Option<PostSortOrder> {
    match token {
        "asc" => Some(PostSortOrder::Asc),
        "desc" => Some(PostSortOrder::Desc),
        _ => None,
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct ArchivePostInput {
    #[schema(max_length = 1000)]
    pub reason: Option<String>,
}

/// Keyset position of a public post list: `(published_at, id)` of the last item
/// on the previous page. Ordering is `published_at DESC, id DESC`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublishedPostCursor {
    pub published_at: DateTime<FixedOffset>,
    pub id: Uuid,
}

impl PublishedPostCursor {
    /// Opaque, URL-safe token. Clients must not parse it.
    pub fn encode(&self) -> String {
        let raw = format!(
            "{}|{}",
            self.published_at.to_rfc3339_opts(SecondsFormat::Nanos, false),
            self.id
        );
        URL_SAFE_NO_PAD.encode(raw)
    }

    pub fn decode(value: &str) -> BlogResult<Self> {
        let invalid = || BlogError::validation("Blog list cursor is invalid");
        let bytes = URL_SAFE_NO_PAD
            .decode(value.trim())
            .map_err(|_| invalid())?;
        let raw = String::from_utf8(bytes).map_err(|_| invalid())?;
        let (published_at, id) = raw.split_once('|').ok_or_else(invalid)?;
        Ok(Self {
            published_at: DateTime::parse_from_rfc3339(published_at).map_err(|_| invalid())?,
            id: Uuid::parse_str(id).map_err(|_| invalid())?,
        })
    }
}

/// Public, cursor-paginated post list request. Public lists never count rows.
#[derive(Debug, Clone, Default)]
pub struct PublicPostsPageQuery {
    pub category_id: Option<Uuid>,
    pub tag: Option<String>,
    pub author_id: Option<Uuid>,
    pub locale: Option<String>,
    pub per_page: Option<u32>,
    pub after: Option<PublishedPostCursor>,
}

impl PublicPostsPageQuery {
    pub fn per_page(&self) -> u32 {
        self.per_page.unwrap_or(20).clamp(1, 100)
    }
}

#[derive(Debug, Clone)]
pub struct PublicPostPage {
    pub items: Vec<PostSummary>,
    /// Present only when another page exists.
    pub next_cursor: Option<PublishedPostCursor>,
}

/// One keyset page of a published-post scan for bulk and sitemap jobs.
/// `next_after` is the last id of this page when another page exists.
#[derive(Debug, Clone)]
pub struct PublishedPostScanPage {
    pub items: Vec<PostSummary>,
    pub next_after: Option<Uuid>,
}

