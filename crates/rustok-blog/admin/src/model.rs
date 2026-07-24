use rustok_api::{RichTextDocument, RichTextView};
use serde::{Deserialize, Serialize};

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
    pub content: Option<RichTextView>,
    #[serde(rename = "contentPlainText")]
    pub content_plain_text: Option<String>,
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

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct BlogModerationCommentList {
    pub items: Vec<BlogModerationComment>,
    pub total: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct BlogModerationComment {
    pub id: String,
    #[serde(rename = "effectiveLocale")]
    pub effective_locale: String,
    #[serde(rename = "authorId")]
    pub author_id: Option<String>,
    #[serde(rename = "contentPreview")]
    pub content_preview: String,
    pub status: String,
    #[serde(rename = "parentCommentId")]
    pub parent_comment_id: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlogModerationStatus {
    Approved,
    Spam,
    Trash,
}

impl BlogModerationStatus {
    pub const fn graphql_value(self) -> &'static str {
        match self {
            Self::Approved => "APPROVED",
            Self::Spam => "SPAM",
            Self::Trash => "TRASH",
        }
    }
}

#[derive(Clone, Debug)]
pub struct BlogPostDraft {
    pub locale: String,
    pub title: String,
    pub slug: String,
    pub excerpt: String,
    pub content: RichTextDocument,
    pub publish: bool,
    pub tags: Vec<String>,
}
