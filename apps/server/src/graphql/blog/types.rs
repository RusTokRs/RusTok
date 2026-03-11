use async_graphql::{InputObject, SimpleObject};
use serde_json::Value;
use uuid::Uuid;

use crate::graphql::content::GqlContentStatus;
use rustok_blog::CreatePostInput as DomainCreatePostInput;
use rustok_content::dto::{NodeListItem, NodeResponse};

#[derive(SimpleObject)]
pub struct GqlPost {
    pub id: Uuid,
    pub title: String,
    pub slug: Option<String>,
    pub excerpt: Option<String>,
    pub body: Option<String>,
    pub status: GqlContentStatus,
    pub author_id: Option<Uuid>,
    pub created_at: String,
    pub published_at: Option<String>,
    pub tags: Vec<String>,
    pub featured_image_url: Option<String>,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
}

#[derive(SimpleObject)]
pub struct GqlPostListItem {
    pub id: Uuid,
    pub title: String,
    pub slug: Option<String>,
    pub excerpt: Option<String>,
    pub status: GqlContentStatus,
    pub author_id: Option<Uuid>,
    pub created_at: String,
    pub published_at: Option<String>,
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
}

#[derive(InputObject)]
pub struct PostsFilter {
    pub status: Option<GqlContentStatus>,
    pub author_id: Option<Uuid>,
    pub locale: Option<String>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

impl From<NodeResponse> for GqlPost {
    fn from(node: NodeResponse) -> Self {
        let translation = node.translations.first();
        let body = node.bodies.first();

        let tags = if let serde_json::Value::Object(map) = &node.metadata {
            map.get("tags")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default()
        } else {
            vec![]
        };

        let featured_image_url = node
            .metadata
            .get("featured_image_url")
            .and_then(|v| v.as_str())
            .map(String::from);

        let seo_title = node
            .metadata
            .get("seo_title")
            .and_then(|v| v.as_str())
            .map(String::from);

        let seo_description = node
            .metadata
            .get("seo_description")
            .and_then(|v| v.as_str())
            .map(String::from);

        Self {
            id: node.id,
            title: translation
                .and_then(|t| t.title.clone())
                .unwrap_or_default(),
            slug: translation.and_then(|t| t.slug.clone()),
            excerpt: translation.and_then(|t| t.excerpt.clone()),
            body: body.and_then(|b| b.body.clone()),
            status: node.status.into(),
            author_id: node.author_id,
            created_at: node.created_at,
            published_at: node.published_at,
            tags,
            featured_image_url,
            seo_title,
            seo_description,
        }
    }
}

impl From<NodeListItem> for GqlPostListItem {
    fn from(item: NodeListItem) -> Self {
        Self {
            id: item.id,
            title: item.title.unwrap_or_default(),
            slug: item.slug,
            excerpt: item.excerpt,
            status: item.status.into(),
            author_id: item.author_id,
            created_at: item.created_at,
            published_at: item.published_at,
        }
    }
}

impl From<CreatePostInput> for DomainCreatePostInput {
    fn from(input: CreatePostInput) -> Self {
        Self {
            locale: input.locale,
            title: input.title,
            body: input.body,
            body_format: input.body_format,
            content_json: input.content_json,
            excerpt: input.excerpt,
            slug: input.slug,
            publish: input.publish,
            tags: input.tags,
            category_id: input.category_id,
            featured_image_url: input.featured_image_url,
            seo_title: input.seo_title,
            seo_description: input.seo_description,
            metadata: None,
        }
    }
}
