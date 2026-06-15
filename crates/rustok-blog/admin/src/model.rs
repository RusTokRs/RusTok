use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BlogPostList {
    pub items: Vec<BlogPostListItem>,
    pub total: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct BlogPostListItem {
    pub id: String,
    pub title: String,
    #[serde(rename = "effectiveLocale")]
    pub effective_locale: String,
    pub slug: Option<String>,
    pub excerpt: Option<String>,
    pub status: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "publishedAt")]
    pub published_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BlogPostDetail {
    pub id: String,
    #[serde(rename = "requestedLocale")]
    pub requested_locale: String,
    #[serde(rename = "effectiveLocale")]
    pub effective_locale: String,
    #[serde(rename = "availableLocales")]
    pub available_locales: Vec<String>,
    pub title: String,
    pub slug: Option<String>,
    pub excerpt: Option<String>,
    pub body: Option<String>,
    #[serde(rename = "bodyFormat")]
    pub body_format: String,
    #[serde(rename = "contentJson")]
    pub content_json: Option<Value>,
    pub status: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "publishedAt")]
    pub published_at: Option<String>,
    pub tags: Vec<String>,
    #[serde(rename = "featuredImageUrl")]
    pub featured_image_url: Option<String>,
    #[serde(rename = "seoTitle")]
    pub seo_title: Option<String>,
    #[serde(rename = "seoDescription")]
    pub seo_description: Option<String>,
}

#[derive(Clone, Debug)]
pub struct BlogPostDraft {
    pub locale: String,
    pub title: String,
    pub slug: String,
    pub excerpt: String,
    pub body: String,
    pub body_format: String,
    pub publish: bool,
    pub tags: Vec<String>,
}
