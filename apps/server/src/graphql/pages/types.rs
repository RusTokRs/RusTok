use async_graphql::{InputObject, SimpleObject};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlPage {
    pub id: Uuid,
    pub status: String,
    pub template: String,
    pub created_at: String,
    pub updated_at: String,
    pub published_at: Option<String>,
    pub translations: Vec<GqlPageTranslation>,
    pub body: Option<GqlPageBody>,
    pub metadata: String,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlPageTranslation {
    pub locale: String,
    pub title: Option<String>,
    pub slug: Option<String>,
    pub meta_title: Option<String>,
    pub meta_description: Option<String>,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlPageBody {
    pub locale: String,
    pub content: String,
    pub format: String,
    pub content_json: Option<Value>,
    pub updated_at: String,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlPageListItem {
    pub id: Uuid,
    pub status: String,
    pub template: String,
    pub title: Option<String>,
    pub slug: Option<String>,
    pub updated_at: String,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlPageList {
    pub items: Vec<GqlPageListItem>,
    pub total: u64,
}

#[derive(InputObject)]
pub struct CreateGqlPageInput {
    pub translations: Vec<GqlPageTranslationInput>,
    pub template: Option<String>,
    pub body: Option<GqlPageBodyInput>,
    pub publish: Option<bool>,
}

#[derive(InputObject)]
pub struct UpdateGqlPageInput {
    pub translations: Option<Vec<GqlPageTranslationInput>>,
    pub template: Option<String>,
    pub body: Option<GqlPageBodyInput>,
}

#[derive(InputObject)]
pub struct GqlPageTranslationInput {
    pub locale: String,
    pub title: String,
    pub slug: Option<String>,
    pub meta_title: Option<String>,
    pub meta_description: Option<String>,
}

#[derive(InputObject)]
pub struct GqlPageBodyInput {
    pub locale: String,
    pub content: String,
    pub format: Option<String>,
    pub content_json: Option<Value>,
}

#[derive(InputObject)]
pub struct ListGqlPagesFilter {
    pub locale: Option<String>,
    pub template: Option<String>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

impl From<rustok_pages::PageResponse> for GqlPage {
    fn from(r: rustok_pages::PageResponse) -> Self {
        Self {
            id: r.id,
            status: content_status_str(&r.status),
            template: r.template,
            created_at: r.created_at,
            updated_at: r.updated_at,
            published_at: r.published_at,
            translations: r.translations.into_iter().map(Into::into).collect(),
            body: r.body.map(Into::into),
            metadata: r.metadata.to_string(),
        }
    }
}

impl From<rustok_pages::PageTranslationResponse> for GqlPageTranslation {
    fn from(r: rustok_pages::PageTranslationResponse) -> Self {
        Self {
            locale: r.locale,
            title: r.title,
            slug: r.slug,
            meta_title: r.meta_title,
            meta_description: r.meta_description,
        }
    }
}

impl From<rustok_pages::PageBodyResponse> for GqlPageBody {
    fn from(r: rustok_pages::PageBodyResponse) -> Self {
        Self {
            locale: r.locale,
            content: r.content,
            format: r.format,
            content_json: r.content_json,
            updated_at: r.updated_at,
        }
    }
}

impl From<rustok_pages::PageListItem> for GqlPageListItem {
    fn from(r: rustok_pages::PageListItem) -> Self {
        Self {
            id: r.id,
            status: content_status_str(&r.status),
            template: r.template,
            title: r.title,
            slug: r.slug,
            updated_at: r.updated_at,
        }
    }
}

fn content_status_str(status: &rustok_content::entities::node::ContentStatus) -> String {
    use rustok_content::entities::node::ContentStatus;
    match status {
        ContentStatus::Draft => "draft".to_string(),
        ContentStatus::Published => "published".to_string(),
        ContentStatus::Archived => "archived".to_string(),
    }
}
