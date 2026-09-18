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
    excerpt: Option<String>,
    seo_title: Option<String>,
    seo_description: Option<String>,
    article_body: Option<String>,
    now: chrono::DateTime<chrono::Utc>,
}

use rustok_api::{Action, PLATFORM_FALLBACK_LOCALE, Resource, RichTextDocument};
use rustok_content::{
    available_locales_from, normalize_locale_code, resolve_by_locale_with_fallback,
};
use rustok_core::SecurityContext;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;
use serde_json::Value;

use crate::dto::{
    CreatePostInput, PostListQuery, PostListResponse, PostResponse, PostSummary, UpdatePostInput,
};
use crate::entities::{blog_post, blog_post_channel_visibility, blog_post_translation};
use crate::error::{BlogError, BlogResult};
use crate::richtext::{canonical_article_body, normalize_article, project_stored_article};
use crate::services::category::CategoryService;
use crate::services::rbac::{
    can_read_non_public_posts, enforce_create_author, enforce_owned_scope, enforce_scope,
};
use crate::services::tag::{find_post_ids_by_tag, load_post_tags_map, sync_post_tags_in_tx};
use crate::state_machine::BlogPostStatus;

pub struct PostService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

struct ResolvedTranslationRecord<'a> {
    translation: Option<&'a blog_post_translation::Model>,
    effective_locale: String,
}

mod commands;
mod queries;
mod repository;

#[cfg(test)]
mod tests;

impl PostService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }
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
    let ascending = matches!(query.sort_order.as_deref(), Some("asc" | "ASC"));
    match query.sort_by.as_deref() {
        Some("published_at") => {
            if ascending {
                select = select.order_by_asc(blog_post::Column::PublishedAt);
            } else {
                select = select.order_by_desc(blog_post::Column::PublishedAt);
            }
        }
        Some("updated_at") => {
            if ascending {
                select = select.order_by_asc(blog_post::Column::UpdatedAt);
            } else {
                select = select.order_by_desc(blog_post::Column::UpdatedAt);
            }
        }
        _ => {
            if ascending {
                select = select.order_by_asc(blog_post::Column::CreatedAt);
            } else {
                select = select.order_by_desc(blog_post::Column::CreatedAt);
            }
        }
    }
    select
}

fn validate_title(title: &str) -> BlogResult<()> {
    if title.trim().is_empty() {
        return Err(BlogError::validation("Title cannot be empty"));
    }
    if title.len() > 512 {
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

fn validate_locale(locale: &str) -> BlogResult<()> {
    if locale.trim().is_empty() {
        return Err(BlogError::validation("Locale cannot be empty"));
    }
    Ok(())
}

fn validate_tags(tags: &[String]) -> BlogResult<()> {
    if tags.len() > 20 {
        return Err(BlogError::validation("Cannot have more than 20 tags"));
    }
    Ok(())
}

fn normalize_locale(locale: &str) -> BlogResult<String> {
    normalize_locale_code(locale).ok_or_else(|| BlogError::validation("Invalid locale"))
}

fn normalize_slug(slug: &str) -> String {
    let mut normalized = String::with_capacity(slug.len());
    let mut previous_dash = false;
    for ch in slug.chars().flat_map(|ch| ch.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch);
            previous_dash = false;
        } else if !previous_dash {
            normalized.push('-');
            previous_dash = true;
        }
    }
    normalized.trim_matches('-').to_string()
}

fn build_post_metadata(
    metadata: Option<Value>,
    tags: Option<Vec<String>>,
    category_id: Option<Uuid>,
    featured_image_url: Option<String>,
    seo_title: Option<String>,
    seo_description: Option<String>,
) -> Value {
    let mut metadata = metadata.unwrap_or_else(|| serde_json::json!({}));
    if !metadata.is_object() {
        metadata = serde_json::json!({});
    }
    if let Some(tags) = tags {
        set_metadata_array(&mut metadata, "tags", tags);
    }
    if let Some(category_id) = category_id {
        set_metadata_uuid(&mut metadata, "category_id", category_id);
    }
    if let Some(featured_image_url) = featured_image_url {
        set_metadata_string(&mut metadata, "featured_image_url", &featured_image_url);
    }
    if let Some(seo_title) = seo_title {
        set_metadata_string(&mut metadata, "seo_title", &seo_title);
    }
    if let Some(seo_description) = seo_description {
        set_metadata_string(&mut metadata, "seo_description", &seo_description);
    }
    strip_channel_visibility_metadata(&mut metadata);
    metadata
}

fn set_metadata_array(metadata: &mut Value, key: &str, values: Vec<String>) {
    ensure_metadata_object(metadata).insert(key.to_string(), serde_json::json!(values));
}

fn set_metadata_uuid(metadata: &mut Value, key: &str, value: Uuid) {
    ensure_metadata_object(metadata).insert(key.to_string(), serde_json::json!(value));
}

fn set_metadata_string(metadata: &mut Value, key: &str, value: &str) {
    ensure_metadata_object(metadata).insert(key.to_string(), serde_json::json!(value));
}

fn ensure_metadata_object(metadata: &mut Value) -> &mut serde_json::Map<String, Value> {
    if !metadata.is_object() {
        *metadata = serde_json::json!({});
    }
    metadata
        .as_object_mut()
        .expect("metadata must be an object after normalization")
}

fn merge_metadata(base: &mut Value, patch: Value) {
    match patch {
        Value::Object(patch_map) => {
            let base_map = ensure_metadata_object(base);
            for (key, value) in patch_map {
                base_map.insert(key, value);
            }
        }
        other => *base = other,
    }
}

fn strip_channel_visibility_metadata(metadata: &mut Value) {
    if let Some(object) = metadata.as_object_mut() {
        object.remove("channel_visibility");
    }
}

pub(crate) fn extract_channel_slugs(metadata: &Value) -> Vec<String> {
    metadata
        .get("channel_visibility")
        .and_then(|value| value.get("allowed_channel_slugs"))
        .and_then(|value| value.as_array())
        .map(|items| {
            normalize_channel_slugs(
                &items
                    .iter()
                    .filter_map(|item| item.as_str().map(ToOwned::to_owned))
                    .collect::<Vec<_>>(),
            )
        })
        .unwrap_or_default()
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

fn normalize_channel_slugs(channel_slugs: &[String]) -> Vec<String> {
    let mut normalized = channel_slugs
        .iter()
        .map(|item| item.trim().to_ascii_lowercase())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
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

fn extract_tags(metadata: &Value) -> Vec<String> {
    metadata
        .get("tags")
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(ToOwned::to_owned))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub(crate) fn storage_to_status(status: &str) -> BlogResult<BlogPostStatus> {
    match status {
        "draft" => Ok(BlogPostStatus::Draft),
        "published" => Ok(BlogPostStatus::Published),
        "archived" => Ok(BlogPostStatus::Archived),
        other => Err(BlogError::validation(format!(
            "Unknown blog post status: {other}"
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
