use async_graphql::{InputObject, SimpleObject};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlPage {
    pub id: Uuid,
    pub version: i32,
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

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlPublishPageResult {
    pub operation_id: Uuid,
    pub page_id: Uuid,
    pub version: i32,
    pub idempotency_key: String,
    pub review_hash: String,
    pub sanitized_set_hash: String,
    pub artifact_set_hash: String,
    pub replayed: bool,
    pub published_at: String,
}

#[derive(Clone, Debug, SimpleObject)]
pub struct GqlRollbackPageResult {
    pub operation_id: Uuid,
    pub page_id: Uuid,
    pub version: i32,
    pub idempotency_key: String,
    pub target_publish_operation_id: Uuid,
    pub source_artifact_set_hash: String,
    pub target_artifact_set_hash: String,
    pub replayed: bool,
    pub rolled_back_at: String,
}

#[derive(InputObject)]
pub struct CreateGqlPageInput {
    pub translations: Vec<GqlPageTranslationInput>,
    pub template: Option<String>,
    pub body: Option<GqlPageBodyInput>,
    pub channel_slugs: Option<Vec<String>>,
    pub publish: Option<bool>,
}

#[derive(InputObject)]
pub struct PatchGqlPageMetadataInput {
    pub expected_version: i32,
    pub translations: Option<Vec<GqlPageTranslationInput>>,
    pub template: Option<String>,
    pub channel_slugs: Option<Vec<String>>,
}

#[derive(InputObject)]
pub struct SaveGqlPageDocumentInput {
    pub expected_revision: String,
    pub body: GqlPageBodyInput,
}

#[derive(InputObject)]
pub struct GqlPageBodyRevisionInput {
    pub locale: String,
    pub revision: String,
}

#[derive(InputObject)]
pub struct ReviewedGqlPagePublishRuntimeInput {
    pub format: String,
    pub scenario_id: String,
    pub context: Value,
    pub review_hash: String,
}

#[derive(InputObject)]
pub struct PublishGqlPageInput {
    pub expected_version: i32,
    pub expected_body_revisions: Vec<GqlPageBodyRevisionInput>,
    pub idempotency_key: String,
    pub runtime: ReviewedGqlPagePublishRuntimeInput,
}

#[derive(InputObject)]
pub struct RollbackGqlPageInput {
    pub expected_version: i32,
    pub idempotency_key: String,
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

impl From<crate::PageResponse> for GqlPage {
    fn from(r: crate::PageResponse) -> Self {
        Self {
            id: r.id,
            version: r.version,
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
            metadata: r.metadata.to_string(),
        }
    }
}

impl From<crate::PublishPageResult> for GqlPublishPageResult {
    fn from(result: crate::PublishPageResult) -> Self {
        Self {
            operation_id: result.operation_id,
            page_id: result.page_id,
            version: result.version,
            idempotency_key: result.idempotency_key,
            review_hash: result.review_hash,
            sanitized_set_hash: result.sanitized_set_hash,
            artifact_set_hash: result.artifact_set_hash,
            replayed: result.replayed,
            published_at: result.published_at,
        }
    }
}

impl From<crate::RollbackPageResult> for GqlRollbackPageResult {
    fn from(result: crate::RollbackPageResult) -> Self {
        Self {
            operation_id: result.operation_id,
            page_id: result.page_id,
            version: result.version,
            idempotency_key: result.idempotency_key,
            target_publish_operation_id: result.target_publish_operation_id,
            source_artifact_set_hash: result.source_artifact_set_hash,
            target_artifact_set_hash: result.target_artifact_set_hash,
            replayed: result.replayed,
            rolled_back_at: result.rolled_back_at,
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

fn content_status_str(status: &rustok_content::entities::node::ContentStatus) -> String {
    use rustok_content::entities::node::ContentStatus;
    match status {
        ContentStatus::Draft => "draft".to_string(),
        ContentStatus::Published => "published".to_string(),
        ContentStatus::Archived => "archived".to_string(),
    }
}
