use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, FixedOffset, SecondsFormat, Utc};
use rustok_api::{Patch, RichTextDocument, RichTextView};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::error::{BlogError, BlogResult};
use crate::state_machine::BlogPostStatus;

/// Upper bound of an accepted list cursor token, in bytes.
///
/// A cursor is opaque and client-supplied, so it is decoded before its contents
/// can be validated: without a bound, one request could hand the process an
/// arbitrarily large base64 blob (query strings are not covered by the router's
/// `DefaultBodyLimit`). The longest token this module issues is a
/// `PublishedPostCursor`: `v1|0|<rfc3339>|<rfc3339>|<uuid>` is ~110 bytes and
/// encodes to ~150. 512 keeps every issued token valid with a wide margin while
/// refusing anything that is not one of ours.
const MAX_LIST_CURSOR_TOKEN_BYTES: usize = 512;

/// Rejects an oversized cursor before it is decoded.
fn ensure_cursor_token_is_bounded(token: &str) -> BlogResult<()> {
    if token.len() > MAX_LIST_CURSOR_TOKEN_BYTES {
        return Err(BlogError::validation("Blog list cursor is invalid"));
    }
    Ok(())
}

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
    pub scheduled_at: Option<DateTime<Utc>>,
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
    #[serde(default, skip_serializing_if = "Patch::is_keep")]
    #[schema(value_type = Option<String>)]
    pub scheduled_at: Patch<DateTime<Utc>>,
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
    /// Scheduled publication time. When set on a Draft post, the scheduler
    /// worker publishes it automatically once this timestamp is reached.
    /// `None` when no schedule is active.
    pub scheduled_at: Option<DateTime<Utc>>,
    /// Whether the post is pinned to the top of public listings.
    pub is_pinned: bool,
    /// When the post was pinned. `None` if not pinned.
    pub pinned_at: Option<DateTime<Utc>>,
    pub version: i32,
}

#[cfg(test)]
mod tests {
    use super::{CreatePostInput, PublishedPostCursor};
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    use rustok_api::RichTextDocument;
    use uuid::Uuid;

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
            scheduled_at: None,
        };
        let encoded = serde_json::to_value(input).expect("serialize");
        assert_eq!(encoded["content"]["type"], "doc");
    }

    #[test]
    fn bulk_failure_reports_the_redacted_public_error() {
        use crate::BlogError;

        let post_id = Uuid::from_u128(11);
        let mut result = super::BulkOperationResult::new();
        result.record_failure(
            post_id,
            BlogError::Invariant("blog_posts row 17 failed the storage check".to_string()),
        );

        assert_eq!(result.failed, 1);
        let failure = result.failures.first().expect("one recorded failure");
        assert_eq!(failure.post_id, post_id);
        assert!(
            !failure.message.contains("row 17"),
            "an internal invariant message must not reach the API surface"
        );
        assert!(
            !failure.code.contains("row 17"),
            "the public code carries no internal detail"
        );
        assert!(!failure.message.is_empty());
    }

    #[test]
    fn public_list_cursor_round_trips_every_sort_key() {
        let cursor = PublishedPostCursor {
            is_pinned: true,
            pinned_at: Some(
                chrono::DateTime::parse_from_rfc3339("2026-10-09T10:15:30.123456789Z")
                    .expect("pinned_at"),
            ),
            published_at: chrono::DateTime::parse_from_rfc3339("2026-10-08T08:00:00Z")
                .expect("published_at"),
            id: Uuid::from_u128(7),
        };

        let decoded =
            PublishedPostCursor::decode(&cursor.encode()).expect("cursor must round-trip");
        assert_eq!(decoded, cursor);

        let unpinned = PublishedPostCursor {
            is_pinned: false,
            pinned_at: None,
            ..cursor
        };
        assert_eq!(
            PublishedPostCursor::decode(&unpinned.encode()).expect("unpinned cursor"),
            unpinned
        );
    }

    #[test]
    fn public_list_cursor_rejects_legacy_and_malformed_tokens() {
        // Legacy `(published_at, id)` tokens cannot express the pinned ordering and
        // must be rejected so a client cannot silently resume with a lossy position.
        let legacy =
            URL_SAFE_NO_PAD.encode("2026-10-08T08:00:00Z|00000000-0000-0000-0000-000000000007");
        assert!(PublishedPostCursor::decode(&legacy).is_err());

        assert!(PublishedPostCursor::decode("not-a-token").is_err());
        assert!(PublishedPostCursor::decode("").is_err());
    }

    /// A cursor is decoded before anything else can validate it, and query
    /// strings bypass the router body limit, so an oversized token must be
    /// refused instead of being base64-decoded in full.
    #[test]
    fn list_cursors_are_bounded_before_decoding() {
        let oversized = "A".repeat(super::MAX_LIST_CURSOR_TOKEN_BYTES + 1);
        let public_error = PublishedPostCursor::decode(&oversized)
            .expect_err("an oversized public cursor is refused");
        assert!(matches!(
            public_error,
            crate::BlogError::Validation(_)
        ));
        let admin_error = super::AdminPostCursor::decode(
            &oversized,
            super::PostSortField::CreatedAt,
            super::PostSortOrder::Desc,
        )
        .expect_err("an oversized admin cursor is refused");
        assert!(matches!(
            admin_error,
            crate::BlogError::Validation(_)
        ));

        // The bound accepts a real token with a wide margin.
        let cursor = PublishedPostCursor {
            is_pinned: false,
            pinned_at: None,
            published_at: chrono::DateTime::parse_from_rfc3339("2026-10-08T08:00:00Z")
                .expect("published_at"),
            id: Uuid::from_u128(7),
        };
        assert!(PublishedPostCursor::decode(&cursor.encode()).is_ok());
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
    /// Scheduled publication time. Present on Draft posts with an active schedule.
    pub scheduled_at: Option<DateTime<Utc>>,
    pub is_pinned: bool,
    pub pinned_at: Option<DateTime<Utc>>,
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
        let token = token.trim();
        ensure_cursor_token_is_bounded(token)?;
        let bytes = URL_SAFE_NO_PAD.decode(token).map_err(|_| invalid())?;
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

/// Keyset position of a public post list. It carries every sort key of the public
/// ordering `is_pinned DESC, pinned_at DESC NULLS LAST, published_at DESC, id DESC`,
/// because a cursor that omits the leading keys can skip or repeat rows once pinned
/// posts exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublishedPostCursor {
    pub is_pinned: bool,
    pub pinned_at: Option<DateTime<FixedOffset>>,
    pub published_at: DateTime<FixedOffset>,
    pub id: Uuid,
}

impl PublishedPostCursor {
    /// Opaque, URL-safe token. Clients must not parse it.
    pub fn encode(&self) -> String {
        let pinned_at = self
            .pinned_at
            .map(|value| value.to_rfc3339_opts(SecondsFormat::Nanos, false))
            .unwrap_or_else(|| "-".to_string());
        let raw = format!(
            "v1|{}|{}|{}|{}",
            u8::from(self.is_pinned),
            pinned_at,
            self.published_at
                .to_rfc3339_opts(SecondsFormat::Nanos, false),
            self.id
        );
        URL_SAFE_NO_PAD.encode(raw)
    }

    pub fn decode(value: &str) -> BlogResult<Self> {
        let invalid = || BlogError::validation("Blog list cursor is invalid");
        let value = value.trim();
        ensure_cursor_token_is_bounded(value)?;
        let bytes = URL_SAFE_NO_PAD.decode(value).map_err(|_| invalid())?;
        let raw = String::from_utf8(bytes).map_err(|_| invalid())?;
        let parts: Vec<&str> = raw.split('|').collect();
        let [version, is_pinned, pinned_at, published_at, id] = parts.as_slice() else {
            return Err(invalid());
        };
        if *version != "v1" {
            return Err(invalid());
        }
        let is_pinned = match *is_pinned {
            "0" => false,
            "1" => true,
            _ => return Err(invalid()),
        };
        let pinned_at = if *pinned_at == "-" {
            None
        } else {
            Some(DateTime::parse_from_rfc3339(pinned_at).map_err(|_| invalid())?)
        };
        Ok(Self {
            is_pinned,
            pinned_at,
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

/// Minimal projection of one published post for SEO scans.
///
/// The SEO port needs identity, the route key, the localized title, the
/// featured image and the set of translated locales. Reusing [`PostSummary`]
/// for the scan forced every sitemap page to load tag links, category names,
/// channel allowlists and full translation rows (including the article body)
/// that the sitemap mapping discards.
#[derive(Debug, Clone)]
pub struct PublishedSeoPost {
    pub id: Uuid,
    pub slug: String,
    pub title: String,
    pub featured_image_url: Option<String>,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
}

/// One keyset page of a published-post SEO scan for bulk and sitemap jobs.
/// `next_after` is the last id of this page when another page exists.
#[derive(Debug, Clone)]
pub struct PublishedPostScanPage {
    pub items: Vec<PublishedSeoPost>,
    pub next_after: Option<Uuid>,
}

/// Result of a bulk operation on blog posts.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BulkOperationResult {
    /// Number of posts that were successfully processed.
    pub succeeded: u32,
    /// Number of posts that failed (wrong status, permission denied, etc.).
    pub failed: u32,
    /// IDs of posts that were successfully processed.
    pub succeeded_ids: Vec<Uuid>,
    /// IDs of posts that failed, with reasons.
    pub failures: Vec<BulkOperationFailure>,
}

/// A single failure in a bulk operation.
///
/// The failure carries the redacted public error of the Blog boundary, never the
/// internal error display text: `BlogError::Invariant` and `BlogError::Database`
/// messages contain storage identifiers and must not reach API consumers.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BulkOperationFailure {
    pub post_id: Uuid,
    /// Stable public code, e.g. `POST_NOT_FOUND` or `BLOG_INVALID_TRANSITION`.
    pub code: String,
    /// Redacted public message for `code`.
    pub message: String,
}

impl Default for BulkOperationResult {
    fn default() -> Self {
        Self::new()
    }
}

impl BulkOperationResult {
    pub fn new() -> Self {
        Self {
            succeeded: 0,
            failed: 0,
            succeeded_ids: Vec::new(),
            failures: Vec::new(),
        }
    }

    pub fn record_success(&mut self, post_id: Uuid) {
        self.succeeded += 1;
        self.succeeded_ids.push(post_id);
    }

    /// Records one failure through the module's single redacted public-error
    /// boundary, so a bulk response cannot leak internal Blog diagnostics.
    pub fn record_failure(&mut self, post_id: Uuid, error: crate::BlogError) {
        let public = crate::error::BlogPublicError::from(error);
        self.failed += 1;
        self.failures.push(BulkOperationFailure {
            post_id,
            code: public.code,
            message: public.message,
        });
    }
}
