use async_graphql::{InputObject, SimpleObject};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlPage {
    pub id: Uuid,
    pub status: String,
    pub requested_locale: Option<String>,
    pub effective_locale: Option<String>,
    pub available_locales: Vec<String>,
    pub template: String,
    pub created_at: String,
    pub updated_at: String,
    pub published_at: Option<String>,
    pub translation: Option<GqlPageTranslation>,
    pub translations: Vec<GqlPageTranslation>,
    pub body: Option<GqlPageBody>,
    pub channel_slugs: Vec<String>,
    pub blocks: Vec<GqlBlock>,
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
    pub channel_slugs: Vec<String>,
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
    pub blocks: Option<Vec<CreateGqlBlockInput>>,
    pub channel_slugs: Option<Vec<String>>,
    pub publish: Option<bool>,
}

#[derive(InputObject)]
pub struct UpdateGqlPageInput {
    pub translations: Option<Vec<GqlPageTranslationInput>>,
    pub template: Option<String>,
    pub body: Option<GqlPageBodyInput>,
    pub channel_slugs: Option<Vec<String>>,
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

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlBlock {
    pub id: Uuid,
    pub block_type: String,
    pub position: i32,
    pub data: Value,
    pub translations: Option<Vec<GqlBlockTranslation>>,
}

#[derive(Clone, Debug, InputObject)]
pub struct CreateGqlBlockInput {
    pub block_type: String,
    pub position: i32,
    pub data: Value,
    pub translations: Option<Vec<GqlBlockTranslationInput>>,
}

#[derive(Clone, Debug, InputObject)]
pub struct UpdateGqlBlockInput {
    pub position: Option<i32>,
    pub data: Option<Value>,
    pub translations: Option<Vec<GqlBlockTranslationInput>>,
}

#[derive(Clone, Debug, InputObject)]
pub struct GqlBlockTranslationInput {
    pub locale: String,
    pub data: Value,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlBlockTranslation {
    pub locale: String,
    pub data: Value,
}

#[derive(Clone, Debug, InputObject)]
pub struct ReorderBlocksInput {
    pub block_ids: Vec<Uuid>,
}

#[derive(InputObject)]
pub struct ListGqlPagesFilter {
    pub locale: Option<String>,
    pub template: Option<String>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

impl From<crate::PageResponse> for GqlPage {
    fn from(r: crate::PageResponse) -> Self {
        Self {
            id: r.id,
            status: content_status_str(&r.status),
            requested_locale: r.requested_locale,
            effective_locale: r.effective_locale,
            available_locales: r.available_locales,
            template: r.template,
            created_at: r.created_at,
            updated_at: r.updated_at,
            published_at: r.published_at,
            translation: r.translation.map(Into::into),
            translations: r.translations.into_iter().map(Into::into).collect(),
            body: r.body.map(Into::into),
            channel_slugs: r.channel_slugs,
            blocks: r.blocks.into_iter().map(Into::into).collect(),
            metadata: r.metadata.to_string(),
        }
    }
}

impl From<crate::PageTranslationResponse> for GqlPageTranslation {
    fn from(r: crate::PageTranslationResponse) -> Self {
        Self {
            locale: r.locale,
            title: r.title,
            slug: r.slug,
            meta_title: r.meta_title,
            meta_description: r.meta_description,
        }
    }
}

impl From<crate::PageBodyResponse> for GqlPageBody {
    fn from(r: crate::PageBodyResponse) -> Self {
        Self {
            locale: r.locale,
            content: r.content,
            format: r.format,
            content_json: r.content_json,
            updated_at: r.updated_at,
        }
    }
}

impl From<crate::PageListItem> for GqlPageListItem {
    fn from(r: crate::PageListItem) -> Self {
        Self {
            id: r.id,
            status: content_status_str(&r.status),
            template: r.template,
            title: r.title,
            slug: r.slug,
            channel_slugs: r.channel_slugs,
            updated_at: r.updated_at,
        }
    }
}

impl From<crate::BlockResponse> for GqlBlock {
    fn from(r: crate::BlockResponse) -> Self {
        Self {
            id: r.id,
            block_type: block_type_str(&r.block_type),
            position: r.position,
            data: r.data,
            translations: r.translations.map(|items| {
                items
                    .into_iter()
                    .map(|translation| GqlBlockTranslation {
                        locale: translation.locale,
                        data: translation.data,
                    })
                    .collect()
            }),
        }
    }
}

fn block_type_str(block_type: &crate::BlockType) -> String {
    use crate::BlockType;
    match block_type {
        BlockType::Hero => "hero".to_string(),
        BlockType::Text => "text".to_string(),
        BlockType::Image => "image".to_string(),
        BlockType::Gallery => "gallery".to_string(),
        BlockType::Cta => "cta".to_string(),
        BlockType::Features => "features".to_string(),
        BlockType::Testimonials => "testimonials".to_string(),
        BlockType::Pricing => "pricing".to_string(),
        BlockType::Faq => "faq".to_string(),
        BlockType::Contact => "contact".to_string(),
        BlockType::ProductGrid => "product_grid".to_string(),
        BlockType::Newsletter => "newsletter".to_string(),
        BlockType::Video => "video".to_string(),
        BlockType::Html => "html".to_string(),
        BlockType::Spacer => "spacer".to_string(),
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
