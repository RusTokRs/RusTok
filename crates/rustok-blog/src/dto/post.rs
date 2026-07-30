use chrono::{DateTime, Utc};
use rustok_api::{RichTextDocument, RichTextView};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

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
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
    pub channel_slugs: Option<Vec<String>>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default)]
pub struct UpdatePostInput {
    pub locale: Option<String>,
    #[schema(max_length = 512)]
    pub title: Option<String>,
    pub content: Option<RichTextDocument>,
    #[schema(max_length = 1000)]
    pub excerpt: Option<String>,
    #[schema(max_length = 255)]
    pub slug: Option<String>,
    #[schema(max_items = 20)]
    pub tags: Option<Vec<String>>,
    pub category_id: Option<Uuid>,
    pub featured_image_url: Option<String>,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
    pub channel_slugs: Option<Vec<String>>,
    pub metadata: Option<Value>,
    pub version: Option<i32>,
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
    pub view_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, IntoParams, Default)]
pub struct PostListQuery {
    pub status: Option<BlogPostStatus>,
    pub category_id: Option<Uuid>,
    pub tag: Option<String>,
    pub author_id: Option<Uuid>,
    pub search: Option<String>,
    pub locale: Option<String>,
    #[schema(default = 1)]
    pub page: Option<u32>,
    #[schema(default = 20, maximum = 100)]
    pub per_page: Option<u32>,
    #[schema(default = "created_at")]
    pub sort_by: Option<String>,
    #[schema(default = "desc")]
    pub sort_order: Option<String>,
}

impl PostListQuery {
    pub fn page(&self) -> u32 {
        self.page.unwrap_or(1).max(1)
    }

    pub fn per_page(&self) -> u32 {
        self.per_page.unwrap_or(20).clamp(1, 100)
    }

    pub fn offset(&self) -> u64 {
        (self.page() - 1) as u64 * self.per_page() as u64
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PostListResponse {
    pub items: Vec<PostSummary>,
    pub total: u64,
    pub page: u32,
    pub per_page: u32,
    pub total_pages: u32,
}

impl PostListResponse {
    pub fn new(items: Vec<PostSummary>, total: u64, query: &PostListQuery) -> Self {
        let per_page = query.per_page();
        let total_pages = ((total as f64) / (per_page as f64)).ceil() as u32;

        Self {
            items,
            total,
            page: query.page(),
            per_page,
            total_pages,
        }
    }
}
