mod graphql_adapter;

use crate::api;
use crate::model::{
    CategoryDetail, CategoryDraft, CategoryListItem, ReplyListItem, TopicDetail, TopicDraft,
    TopicListItem,
};

pub type ApiError = String;

pub async fn fetch_categories(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
) -> Result<Vec<CategoryListItem>, ApiError> {
    match graphql_adapter::fetch_categories(token.clone(), tenant_slug.clone(), locale.clone())
        .await
    {
        Ok(categories) => Ok(categories),
        Err(_) => api::fetch_categories(token, tenant_slug, locale).await,
    }
}

pub async fn fetch_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: String,
) -> Result<CategoryDetail, ApiError> {
    match graphql_adapter::fetch_category(
        token.clone(),
        tenant_slug.clone(),
        id.clone(),
        locale.clone(),
    )
    .await
    {
        Ok(category) => Ok(category),
        Err(_) => api::fetch_category(token, tenant_slug, id, locale).await,
    }
}

pub async fn create_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    draft: CategoryDraft,
) -> Result<CategoryDetail, ApiError> {
    match graphql_adapter::create_category(token.clone(), tenant_slug.clone(), draft.clone()).await
    {
        Ok(category) => Ok(category),
        Err(_) => api::create_category(token, tenant_slug, draft).await,
    }
}

pub async fn update_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    draft: CategoryDraft,
) -> Result<CategoryDetail, ApiError> {
    match graphql_adapter::update_category(
        token.clone(),
        tenant_slug.clone(),
        id.clone(),
        draft.clone(),
    )
    .await
    {
        Ok(category) => Ok(category),
        Err(_) => api::update_category(token, tenant_slug, id, draft).await,
    }
}

pub async fn delete_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    match graphql_adapter::delete_category(token.clone(), tenant_slug.clone(), id.clone()).await {
        Ok(()) => Ok(()),
        Err(_) => api::delete_category(token, tenant_slug, id).await,
    }
}

pub async fn fetch_topics(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
    category_id: Option<String>,
) -> Result<Vec<TopicListItem>, ApiError> {
    match graphql_adapter::fetch_topics(
        token.clone(),
        tenant_slug.clone(),
        locale.clone(),
        category_id.clone(),
    )
    .await
    {
        Ok(topics) => Ok(topics),
        Err(_) => api::fetch_topics(token, tenant_slug, locale, category_id).await,
    }
}

pub async fn fetch_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: String,
) -> Result<TopicDetail, ApiError> {
    match graphql_adapter::fetch_topic(
        token.clone(),
        tenant_slug.clone(),
        id.clone(),
        locale.clone(),
    )
    .await
    {
        Ok(topic) => Ok(topic),
        Err(_) => api::fetch_topic(token, tenant_slug, id, locale).await,
    }
}

pub async fn create_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    draft: TopicDraft,
) -> Result<TopicDetail, ApiError> {
    match graphql_adapter::create_topic(token.clone(), tenant_slug.clone(), draft.clone()).await {
        Ok(topic) => Ok(topic),
        Err(_) => api::create_topic(token, tenant_slug, draft).await,
    }
}

pub async fn update_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    draft: TopicDraft,
) -> Result<TopicDetail, ApiError> {
    match graphql_adapter::update_topic(
        token.clone(),
        tenant_slug.clone(),
        id.clone(),
        draft.clone(),
    )
    .await
    {
        Ok(topic) => Ok(topic),
        Err(_) => api::update_topic(token, tenant_slug, id, draft).await,
    }
}

pub async fn delete_topic(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<(), ApiError> {
    match graphql_adapter::delete_topic(token.clone(), tenant_slug.clone(), id.clone()).await {
        Ok(()) => Ok(()),
        Err(_) => api::delete_topic(token, tenant_slug, id).await,
    }
}

pub async fn fetch_replies(
    token: Option<String>,
    tenant_slug: Option<String>,
    topic_id: String,
    locale: String,
) -> Result<Vec<ReplyListItem>, ApiError> {
    match graphql_adapter::fetch_replies(
        token.clone(),
        tenant_slug.clone(),
        topic_id.clone(),
        locale.clone(),
    )
    .await
    {
        Ok(replies) => Ok(replies),
        Err(_) => api::fetch_replies(token, tenant_slug, topic_id, locale).await,
    }
}
