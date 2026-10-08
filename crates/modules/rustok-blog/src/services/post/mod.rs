use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, DatabaseTransaction, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, Select, Set, TransactionTrait,
    sea_query::{Query, SelectStatement},
};
use std::collections::HashMap;
use tracing::instrument;
use uuid::Uuid;

struct PostTranslationUpsertInput {
    title: Option<String>,
    excerpt: rustok_api::Patch<String>,
    seo_title: rustok_api::Patch<String>,
    seo_description: rustok_api::Patch<String>,
    article_body: Option<String>,
    now: chrono::DateTime<chrono::Utc>,
}

use rustok_api::{Action, PLATFORM_FALLBACK_LOCALE, Patch, Resource};
use rustok_channel::ChannelService;
use rustok_content::{
    available_locales_from, normalize_locale_code, resolve_by_locale_with_fallback,
};
use rustok_core::SecurityContext;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;
use serde_json::Value;

use crate::dto::{
    CreatePostInput, PostListQuery, PostListResponse, PostResponse, PostSortField, PostSortOrder,
    PostSummary, PublicPostPage, PublicPostsPageQuery, PublishedPostCursor, UpdatePostInput,
};
use crate::entities::{
    blog_post, blog_post_channel_visibility, blog_post_tag, blog_post_translation,
};
use crate::error::{BlogError, BlogResult};
use crate::richtext::{canonical_article_body, normalize_article, project_stored_article};
use crate::services::category::CategoryService;
use crate::services::rbac::{
    can_read_non_public_posts, enforce_create_author, enforce_owned_scope, enforce_scope,
};
use crate::services::tag::{
    load_post_tags_map, remove_post_tag_usage_in_tx, resolve_tag_id_for_posts, sync_post_tags_in_tx,
};
use crate::state_machine::BlogPostStatus;

pub struct PostService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

#[derive(Debug, Clone)]
pub(crate) struct PostSubjectSnapshot {
    pub(crate) status: BlogPostStatus,
    pub(crate) channel_slugs: Vec<String>,
    pub(crate) version: i32,
}

struct ResolvedTranslationRecord<'a> {
    translation: Option<&'a blog_post_translation::Model>,
    effective_locale: String,
}

mod commands;
mod queries;
mod repository;
mod routes;

pub use routes::{BLOG_ROUTE_PREFIX, BlogPostRedirect, BlogPostRouteOwner};

pub(crate) use repository::load_post_subject_snapshot;

#[cfg(test)]
mod tests;

impl PostService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }
}

fn apply_tag_filter(
    select: Select<blog_post::Entity>,
    tenant_id: Uuid,
    tag_id: Uuid,
) -> Select<blog_post::Entity> {
    let tagged_post_ids = Query::select()
        .column(blog_post_tag::Column::PostId)
        .from(blog_post_tag::Entity)
        .and_where(blog_post_tag::Column::TenantId.eq(tenant_id))
        .and_where(blog_post_tag::Column::TagId.eq(tag_id))
        .to_owned();

    select.filter(blog_post::Column::Id.in_subquery(tagged_post_ids))
}

fn resolve_translation_record<'a>(
    translations: &'a [blog_post_translation::Model],
    requested: &str,
    fallback_locale: Option<&str>,
) -> ResolvedTranslationRecord<'a> {
    let resolved =
        resolve_by_locale_with_fallback(translations, requested, fallback_locale, |item| {
            item.locale.as_str()
        });
    ResolvedTranslationRecord {
        translation: resolved.item,
        effective_locale: resolved.effective_locale,
    }
}

fn apply_post_sort(
    mut select: sea_orm::Select<blog_post::Entity>,
    query: &PostListQuery,
) -> sea_orm::Select<blog_post::Entity> {
    let ascending = matches!(query.sort_order.unwrap_or_default(), PostSortOrder::Asc);
    let field = query.sort_by.unwrap_or_default();

    macro_rules! order {
        ($column:expr) => {{
            if ascending {
                select = select.order_by_asc($column);
                select = select.order_by_asc(blog_post::Column::Id);
            } else {
                select = select.order_by_desc($column);
                select = select.order_by_desc(blog_post::Column::Id);
            }
        }};
    }

    match field {
        PostSortField::PublishedAt => order!(blog_post::Column::PublishedAt),
        PostSortField::UpdatedAt => order!(blog_post::Column::UpdatedAt),
        PostSortField::CreatedAt => order!(blog_post::Column::CreatedAt),
    }

    select
}

fn validate_title(title: &str) -> BlogResult<()> {
    if title.trim().is_empty() {
        return Err(BlogError::validation("Title cannot be empty"));
    }
    if title.chars().count() > 512 {
        return Err(BlogError::validation("Title cannot exceed 512 characters"));
    }
    Ok(())
}

fn validate_optional_title(title: Option<&str>) -> BlogResult<()> {
    if let Some(title) = title {
        validate_title(title)?;
    }
    Ok(())
}

fn validate_tags(tags: &[String]) -> BlogResult<()> {
    if tags.len() > 20 {
        return Err(BlogError::validation("Cannot have more than 20 tags"));
    }
    Ok(())
}

fn validate_post_field_length(
    value: Option<&str>,
    max_chars: usize,
    field: &str,
) -> BlogResult<()> {
    if let Some(value) = value
        && value.chars().count() > max_chars
    {
        return Err(BlogError::validation(format!(
            "{field} cannot exceed {max_chars} characters"
        )));
    }
    Ok(())
}

/// Canonical-route `object_type` used by Blog posts in the shared
/// `canonical_url` / `url_alias` registry owned by `rustok-content`.
pub(crate) const BLOG_POST_TARGET_KIND: &str = "blog_post";

/// Locale under which every Blog post route is stored. Blog slugs are global
/// canonical identifiers (see `DECISIONS/2026-03-28-multilingual-content-contract.md`),
/// so the route does not vary by translation locale. `resolve_route` falls back
/// to the platform locale for every requested locale, so the route resolves
/// for any language.
pub const CANONICAL_POST_ROUTE_LOCALE: &str = PLATFORM_FALLBACK_LOCALE;

/// Canonical public route of a Blog post. Shared by Blog, content orchestration
/// and SEO target projection, so there is exactly one definition of the route.
/// The slug is already a route key (`[a-z0-9-]`), so it needs no escaping.
pub fn canonical_post_route(slug: &str) -> String {
    format!("/modules/blog?slug={slug}")
}

fn normalize_locale(locale: &str) -> BlogResult<String> {
    normalize_locale_code(locale).ok_or_else(|| BlogError::validation("Invalid locale"))
}

/// Normalizes a blog post slug through the shared Taxonomy route-key contract.
///
/// Blog post slugs are global canonical identifiers (see
/// `DECISIONS/2026-03-28-multilingual-content-contract.md`), so the same
/// normalizer is used for every locale. Non-ASCII titles are transliterated
/// instead of being dropped. Returns an empty string when nothing usable remains.
fn normalize_slug(slug: &str) -> String {
    rustok_taxonomy::normalize_term_route_key(slug).unwrap_or_default()
}

const MAX_FEATURED_IMAGE_URL_CHARS: usize = 2048;

/// Validates a featured image reference.
///
/// Accepts an absolute `http`/`https` URL or a root-relative path (`/...`,
/// not protocol-relative `//...`). Any other scheme (`javascript:`, `data:`,
/// `file:`, ...) is rejected so that the value is safe to render in `<img src>`
/// and in feed/Open Graph metadata.
fn validate_featured_image_url(url: &str) -> BlogResult<()> {
    if url.trim() != url || url.is_empty() {
        return Err(BlogError::validation(
            "Featured image URL must be a non-empty URL without surrounding whitespace",
        ));
    }
    if url.chars().count() > MAX_FEATURED_IMAGE_URL_CHARS {
        return Err(BlogError::validation(format!(
            "Featured image URL cannot exceed {MAX_FEATURED_IMAGE_URL_CHARS} characters"
        )));
    }
    if url.chars().any(char::is_control) {
        return Err(BlogError::validation(
            "Featured image URL cannot contain control characters",
        ));
    }
    if url.starts_with('/') {
        if url.starts_with("//") {
            return Err(BlogError::validation(
                "Featured image URL cannot be protocol-relative",
            ));
        }
        return Ok(());
    }
    let parsed = url::Url::parse(url).map_err(|_| {
        BlogError::validation("Featured image URL must be an absolute or root-relative URL")
    })?;
    match parsed.scheme() {
        "http" | "https" => Ok(()),
        _ => Err(BlogError::validation(
            "Featured image URL must use http or https",
        )),
    }
}

fn validate_optional_featured_image_url(url: Option<&str>) -> BlogResult<()> {
    if let Some(url) = url {
        validate_featured_image_url(url)?;
    }
    Ok(())
}

const RESERVED_POST_METADATA_KEYS: &[&str] = &[
    "tags",
    "category_id",
    "featured_image_url",
    "seo_title",
    "seo_description",
    "channel_visibility",
    "channel_slugs",
];

const MAX_POST_METADATA_BYTES: usize = 64 * 1024;
const MAX_POST_CHANNEL_SLUGS: usize = 32;
const MAX_POST_CHANNEL_SLUG_BYTES: usize = 100;
const MAX_POST_SLUG_BYTES: usize = 255;
const MAX_POST_EXCERPT_CHARS: usize = 1000;
const MAX_POST_SEO_TITLE_CHARS: usize = 255;
const MAX_POST_SEO_DESCRIPTION_CHARS: usize = 1000;
const MAX_POST_ARCHIVE_REASON_CHARS: usize = 1000;

fn normalize_custom_metadata(metadata: Option<Value>) -> BlogResult<Value> {
    let metadata = metadata.unwrap_or_else(|| serde_json::json!({}));
    let Value::Object(map) = metadata else {
        return Err(BlogError::validation("Post metadata must be a JSON object"));
    };

    if let Some(key) = RESERVED_POST_METADATA_KEYS
        .iter()
        .find(|key| map.contains_key(**key))
    {
        return Err(BlogError::validation(format!(
            "Post metadata key '{key}' is reserved by a canonical typed field"
        )));
    }

    let normalized = Value::Object(map);
    let encoded = serde_json::to_vec(&normalized)
        .map_err(|_| BlogError::validation("Post metadata could not be serialized"))?;
    if encoded.len() > MAX_POST_METADATA_BYTES {
        return Err(BlogError::validation(format!(
            "Post metadata cannot exceed {MAX_POST_METADATA_BYTES} bytes"
        )));
    }

    Ok(normalized)
}

fn scrub_reserved_metadata(mut metadata: Value) -> Value {
    let Some(map) = metadata.as_object_mut() else {
        return serde_json::json!({});
    };
    for key in RESERVED_POST_METADATA_KEYS {
        map.remove(*key);
    }
    metadata
}

fn metadata_changed(previous: &Value, next: &Value) -> bool {
    previous != next
}

pub(crate) fn is_post_visible_for_channel(
    channel_slugs: &[String],
    channel_slug: Option<&str>,
) -> bool {
    if channel_slugs.is_empty() {
        return true;
    }

    let Some(channel_slug) = channel_slug else {
        return false;
    };

    let normalized = channel_slug.trim().to_ascii_lowercase();
    !normalized.is_empty() && channel_slugs.iter().any(|item| item == &normalized)
}

fn normalize_channel_slugs(channel_slugs: &[String]) -> BlogResult<Vec<String>> {
    let mut normalized = channel_slugs
        .iter()
        .map(|item| item.trim().to_ascii_lowercase())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();

    if normalized.len() > MAX_POST_CHANNEL_SLUGS {
        return Err(BlogError::validation(format!(
            "A post cannot target more than {MAX_POST_CHANNEL_SLUGS} channels"
        )));
    }
    if normalized
        .iter()
        .any(|item| item.len() > MAX_POST_CHANNEL_SLUG_BYTES)
    {
        return Err(BlogError::validation(format!(
            "Channel slugs cannot exceed {MAX_POST_CHANNEL_SLUG_BYTES} bytes"
        )));
    }

    Ok(normalized)
}

fn apply_public_post_channel_filter(
    select: Select<blog_post::Entity>,
    tenant_id: Uuid,
    channel_slug: Option<&str>,
) -> Select<blog_post::Entity> {
    let unrestricted =
        blog_post::Column::Id.not_in_subquery(all_blog_post_channel_visibility_subquery(tenant_id));
    let condition = match normalize_public_channel_slug(channel_slug) {
        Some(channel_slug) => {
            Condition::any()
                .add(unrestricted)
                .add(blog_post::Column::Id.in_subquery(
                    matching_blog_post_channel_visibility_subquery(tenant_id, &channel_slug),
                ))
        }
        None => Condition::all().add(unrestricted),
    };

    select.filter(condition)
}

fn all_blog_post_channel_visibility_subquery(tenant_id: Uuid) -> SelectStatement {
    Query::select()
        .column(blog_post_channel_visibility::Column::PostId)
        .from(blog_post_channel_visibility::Entity)
        .and_where(blog_post_channel_visibility::Column::TenantId.eq(tenant_id))
        .to_owned()
}

fn matching_blog_post_channel_visibility_subquery(
    tenant_id: Uuid,
    channel_slug: &str,
) -> SelectStatement {
    Query::select()
        .column(blog_post_channel_visibility::Column::PostId)
        .from(blog_post_channel_visibility::Entity)
        .and_where(blog_post_channel_visibility::Column::TenantId.eq(tenant_id))
        .and_where(blog_post_channel_visibility::Column::ChannelSlug.eq(channel_slug))
        .to_owned()
}

fn normalize_public_channel_slug(channel_slug: Option<&str>) -> Option<String> {
    channel_slug
        .map(str::trim)
        .filter(|slug| !slug.is_empty())
        .map(|slug| slug.to_ascii_lowercase())
}

pub(crate) fn storage_to_status(status: &str) -> BlogResult<BlogPostStatus> {
    match status {
        "draft" => Ok(BlogPostStatus::Draft),
        "published" => Ok(BlogPostStatus::Published),
        "archived" => Ok(BlogPostStatus::Archived),
        other => Err(BlogError::invariant(format!(
            "Unknown persisted Blog post status: {other}"
        ))),
    }
}

fn status_to_storage(status: BlogPostStatus) -> &'static str {
    match status {
        BlogPostStatus::Draft => "draft",
        BlogPostStatus::Published => "published",
        BlogPostStatus::Archived => "archived",
    }
}
