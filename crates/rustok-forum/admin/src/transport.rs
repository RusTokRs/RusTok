use crate::api::{self, ApiError};
use crate::model::{
    CategoryDetail, CategoryDraft, CategoryListItem, ReplyListItem, TopicDetail, TopicDraft,
    TopicListItem,
};

pub async fn fetch_categories(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
) -> Result<Vec<CategoryListItem>, ApiError> {
    api::fetch_categories(token, tenant_slug, locale).await
}

pub async fn fetch_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: String,
) -> Result<CategoryDetail, ApiError> {
    api::fetch_category(token, tenant_slug, id, locale).await
}

pub async fn create_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    draft: CategoryDraft,
) -> Result<CategoryDetail, ApiError> {
    api::create_category(token, tenant_slug, draft).await
}

pub async fn update_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    draft: CategoryDraft,
) -> Result<CategoryDetail, ApiError> {
    api::update_category(token, tenant_slug, id, draft).await
}

pub async fn delete_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    api::delete_category(token, tenant_slug, id).await
}

pub async fn fetch_topics(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
    category_id: Option<String>,
) -> Result<Vec<TopicListItem>, ApiError> {
    api::fetch_topics(token, tenant_slug, locale, category_id).await
}

pub async fn fetch_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: String,
) -> Result<TopicDetail, ApiError> {
    api::fetch_topic(token, tenant_slug, id, locale).await
}

pub async fn create_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    draft: TopicDraft,
) -> Result<TopicDetail, ApiError> {
    api::create_topic(token, tenant_slug, draft).await
}

pub async fn update_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    draft: TopicDraft,
) -> Result<TopicDetail, ApiError> {
    api::update_topic(token, tenant_slug, id, draft).await
}

pub async fn delete_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    api::delete_topic(token, tenant_slug, id).await
}

pub async fn fetch_replies(
    token: Option<String>,
    tenant_slug: Option<String>,
    topic_id: String,
    locale: String,
) -> Result<Vec<ReplyListItem>, ApiError> {
    api::fetch_replies(token, tenant_slug, topic_id, locale).await
}
