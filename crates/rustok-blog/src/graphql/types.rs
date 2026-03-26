use async_graphql::{Enum, InputObject, SimpleObject};
use serde_json::Value;
use uuid::Uuid;

use crate::services::extract_channel_slugs;
use crate::{BlogPostStatus, CreatePostInput as DomainCreatePostInput, PostResponse};
use rustok_content::dto::NodeListItem;

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
#[graphql(name = "BlogPostStatus", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlContentStatus {
    Draft,
    Published,
    Archived,
}

impl From<rustok_content::entities::node::ContentStatus> for GqlContentStatus {
    fn from(status: rustok_content::entities::node::ContentStatus) -> Self {
        match status {
            rustok_content::entities::node::ContentStatus::Draft => Self::Draft,
            rustok_content::entities::node::ContentStatus::Published => Self::Published,
            rustok_content::entities::node::ContentStatus::Archived => Self::Archived,
        }
    }
}

impl From<GqlContentStatus> for rustok_content::entities::node::ContentStatus {
    fn from(status: GqlContentStatus) -> Self {
        match status {
            GqlContentStatus::Draft => Self::Draft,
            GqlContentStatus::Published => Self::Published,
            GqlContentStatus::Archived => Self::Archived,
        }
    }
}

#[derive(SimpleObject)]
pub struct GqlPost {
    pub id: Uuid,
    pub requested_locale: String,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
    pub title: String,
    pub slug: Option<String>,
    pub excerpt: Option<String>,
    pub body: Option<String>,
    pub body_format: String,
    pub content_json: Option<Value>,
    pub status: GqlContentStatus,
    pub author_id: Option<Uuid>,
    pub created_at: String,
    pub updated_at: String,
    pub published_at: Option<String>,
    pub tags: Vec<String>,
    pub featured_image_url: Option<String>,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
    pub channel_slugs: Vec<String>,
}

#[derive(SimpleObject)]
pub struct GqlPostListItem {
    pub id: Uuid,
    pub title: String,
    pub effective_locale: String,
    pub slug: Option<String>,
    pub excerpt: Option<String>,
    pub status: GqlContentStatus,
    pub author_id: Option<Uuid>,
    pub created_at: String,
    pub published_at: Option<String>,
    pub channel_slugs: Vec<String>,
}

#[derive(SimpleObject)]
pub struct GqlPostList {
    pub items: Vec<GqlPostListItem>,
    pub total: u64,
}

#[derive(InputObject)]
pub struct CreatePostInput {
    pub locale: String,
    pub title: String,
    pub body: String,
    pub body_format: Option<String>,
    pub content_json: Option<Value>,
    pub excerpt: Option<String>,
    pub slug: Option<String>,
    pub publish: bool,
    pub tags: Vec<String>,
    pub category_id: Option<Uuid>,
    pub featured_image_url: Option<String>,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
    pub channel_slugs: Option<Vec<String>>,
}

#[derive(InputObject)]
pub struct UpdatePostInput {
    pub locale: Option<String>,
    pub title: Option<String>,
    pub body: Option<String>,
    pub body_format: Option<String>,
    pub content_json: Option<Value>,
    pub excerpt: Option<String>,
    pub slug: Option<String>,
    pub status: Option<GqlContentStatus>,
    pub tags: Option<Vec<String>>,
    pub category_id: Option<Uuid>,
    pub featured_image_url: Option<String>,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
    pub channel_slugs: Option<Vec<String>>,
}

#[derive(InputObject)]
pub struct PostsFilter {
    pub status: Option<GqlContentStatus>,
    pub author_id: Option<Uuid>,
    pub locale: Option<String>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

impl From<PostResponse> for GqlPost {
    fn from(post: PostResponse) -> Self {
        Self {
            id: post.id,
            requested_locale: post.requested_locale,
            effective_locale: post.effective_locale,
            available_locales: post.available_locales,
            title: post.title,
            slug: Some(post.slug),
            excerpt: post.excerpt,
            body: Some(post.body),
            body_format: post.body_format,
            content_json: post.content_json,
            status: match post.status {
                BlogPostStatus::Draft => GqlContentStatus::Draft,
                BlogPostStatus::Published => GqlContentStatus::Published,
                BlogPostStatus::Archived => GqlContentStatus::Archived,
            },
            author_id: Some(post.author_id),
            created_at: post.created_at.to_rfc3339(),
            updated_at: post.updated_at.to_rfc3339(),
            published_at: post.published_at.map(|value| value.to_rfc3339()),
            tags: post.tags,
            featured_image_url: post.featured_image_url,
            seo_title: post.seo_title,
            seo_description: post.seo_description,
            channel_slugs: post.channel_slugs,
        }
    }
}

impl From<NodeListItem> for GqlPostListItem {
    fn from(item: NodeListItem) -> Self {
        Self {
            id: item.id,
            title: item.title.unwrap_or_default(),
            effective_locale: item.effective_locale,
            slug: item.slug,
            excerpt: item.excerpt,
            status: item.status.into(),
            author_id: item.author_id,
            created_at: item.created_at,
            published_at: item.published_at,
            channel_slugs: extract_channel_slugs(&item.metadata),
        }
    }
}

impl From<CreatePostInput> for DomainCreatePostInput {
    fn from(input: CreatePostInput) -> Self {
        Self {
            locale: input.locale,
            title: input.title,
            body: input.body,
            body_format: input
                .body_format
                .unwrap_or_else(|| rustok_core::CONTENT_FORMAT_MARKDOWN.to_string()),
            content_json: input.content_json,
            excerpt: input.excerpt,
            slug: input.slug,
            publish: input.publish,
            tags: input.tags,
            category_id: input.category_id,
            featured_image_url: input.featured_image_url,
            seo_title: input.seo_title,
            seo_description: input.seo_description,
            channel_slugs: input.channel_slugs,
            metadata: None,
        }
    }
}
