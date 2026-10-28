use rustok_api::{RichTextDocument, RichTextView};
use serde::{Deserialize, Serialize};

/// Sort column of the admin post list, spelled as the Blog GraphQL enum.
///
/// The admin list is keyset-paged, so the sort is part of the transport request and
/// the cursor is bound to it: changing either value restarts the list at page one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BlogPostSortField {
    PublishedAt,
    UpdatedAt,
    #[default]
    CreatedAt,
}

impl BlogPostSortField {
    /// The GraphQL enum value. Kept next to the serde attribute so the wire spelling
    /// has one owner; `blog_post_admin_sort_tokens_match_the_wire_spelling` asserts the
    /// two agree.
    pub fn as_graphql(self) -> &'static str {
        match self {
            Self::PublishedAt => "PUBLISHED_AT",
            Self::UpdatedAt => "UPDATED_AT",
            Self::CreatedAt => "CREATED_AT",
        }
    }

    pub fn from_graphql(value: &str) -> Option<Self> {
        match value {
            "PUBLISHED_AT" => Some(Self::PublishedAt),
            "UPDATED_AT" => Some(Self::UpdatedAt),
            "CREATED_AT" => Some(Self::CreatedAt),
            _ => None,
        }
    }
}

/// Sort direction of the admin post list, spelled as the Blog GraphQL enum.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BlogPostSortOrder {
    Asc,
    #[default]
    Desc,
}

impl BlogPostSortOrder {
    /// The GraphQL enum value; see [`BlogPostSortField::as_graphql`].
    pub fn as_graphql(self) -> &'static str {
        match self {
            Self::Asc => "ASC",
            Self::Desc => "DESC",
        }
    }

    pub fn from_graphql(value: &str) -> Option<Self> {
        match value {
            "ASC" => Some(Self::Asc),
            "DESC" => Some(Self::Desc),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BlogPostList {
    pub items: Vec<BlogPostListItem>,
    /// Present only when another page exists.
    #[serde(rename = "nextCursor", default)]
    pub next_cursor: Option<String>,
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
    #[serde(rename = "scheduledAt")]
    pub scheduled_at: Option<String>,
    #[serde(rename = "isPinned", default)]
    pub is_pinned: bool,
    #[serde(rename = "pinnedAt")]
    pub pinned_at: Option<String>,
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
    #[serde(rename = "scheduledAt")]
    pub scheduled_at: Option<String>,
    #[serde(rename = "isPinned", default)]
    pub is_pinned: bool,
    #[serde(rename = "pinnedAt")]
    pub pinned_at: Option<String>,
    pub tags: Vec<String>,
    #[serde(rename = "categoryId")]
    pub category_id: Option<String>,
    #[serde(rename = "channelSlugs", default)]
    pub channel_slugs: Vec<String>,
    #[serde(rename = "featuredImageUrl")]
    pub featured_image_url: Option<String>,
    #[serde(rename = "seoTitle")]
    pub seo_title: Option<String>,
    #[serde(rename = "seoDescription")]
    pub seo_description: Option<String>,
    pub version: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct BlogCategoryOption {
    pub id: String,
    pub name: String,
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

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BlogPostDraft {
    pub locale: String,
    pub version: Option<i32>,
    pub title: String,
    pub slug: String,
    pub excerpt: String,
    pub content: RichTextDocument,
    pub publish: bool,
    pub tags: Vec<String>,
    /// None is the uncategorized state; an explicit UUID selects a Blog-owned category.
    pub category_id: Option<String>,
    /// An empty list means publication to all channels (owner contract).
    pub channel_slugs: Vec<String>,
    pub featured_image_url: Option<String>,
    pub seo_title: Option<String>,
    pub seo_description: Option<String>,
    /// RFC 3339 timestamp; absent means unscheduled.
    pub scheduled_at: Option<String>,
    /// The form may change schedule only while the post is a draft.
    pub schedule_editable: bool,
}
