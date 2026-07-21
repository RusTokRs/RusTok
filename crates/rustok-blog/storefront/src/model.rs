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

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct BlogCommentList {
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
    pub body: Option<String>,
    #[serde(rename = "bodyFormat")]
    pub body_format: String,
    pub status: String,
    #[serde(rename = "publishedAt")]
    pub published_at: Option<String>,
    pub tags: Vec<String>,
    #[serde(rename = "featuredImageUrl")]
    pub featured_image_url: Option<String>,
    #[serde(default, rename = "publicComments")]
    pub public_comments: BlogCommentList,
}
