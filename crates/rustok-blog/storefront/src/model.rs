use rustok_api::RichTextView;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontBlogData {
    pub selected_post: Option<BlogPostDetail>,
    pub posts: BlogPostList,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BlogPostList {
    pub items: Vec<BlogPostListItem>,
    pub total: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BlogPostListItem {
    pub id: String,
    pub title: String,
    #[serde(rename = "effectiveLocale")]
    pub effective_locale: String,
    pub slug: Option<String>,
    pub excerpt: Option<String>,
    pub status: String,
    #[serde(rename = "publishedAt")]
    pub published_at: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BlogCommentsAvailability {
    #[default]
    Available,
    Unavailable,
    Timeout,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct BlogCommentList {
    #[serde(default)]
    pub availability: BlogCommentsAvailability,
    #[serde(default, rename = "cachedSnapshot")]
    pub cached_snapshot: bool,
    pub items: Vec<BlogCommentListItem>,
    pub total: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BlogCommentListItem {
    pub id: String,
    #[serde(rename = "effectiveLocale")]
    pub effective_locale: String,
    #[serde(rename = "authorId")]
    pub author_id: Option<String>,
    #[serde(rename = "contentPreview")]
    pub content_preview: String,
    #[serde(rename = "parentCommentId")]
    pub parent_comment_id: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BlogPostDetail {
    pub id: String,
    #[serde(rename = "effectiveLocale")]
    pub effective_locale: String,
    pub title: String,
    pub slug: Option<String>,
    pub excerpt: Option<String>,
    #[serde(default)]
    pub content: Option<RichTextView>,
    #[serde(default, rename = "contentPlainText")]
    pub content_plain_text: Option<String>,
    pub status: String,
    #[serde(rename = "publishedAt")]
    pub published_at: Option<String>,
    pub tags: Vec<String>,
    #[serde(rename = "featuredImageUrl")]
    pub featured_image_url: Option<String>,
    #[serde(default, rename = "publicComments")]
    pub public_comments: BlogCommentList,
}
