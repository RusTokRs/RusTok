use crate::api;
use crate::model::{CreatePageDraft, PageDetail, PageList, PageMutationResult};

pub type TransportError = api::ApiError;

pub async fn fetch_pages(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<PageList, TransportError> {
    api::fetch_pages(token, tenant_slug).await
}

pub async fn fetch_page(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<Option<PageDetail>, TransportError> {
    api::fetch_page(token, tenant_slug, id).await
}

pub async fn create_page(
    token: Option<String>,
    tenant_slug: Option<String>,
    draft: CreatePageDraft,
) -> Result<PageMutationResult, TransportError> {
    api::create_page(token, tenant_slug, draft).await
}

pub async fn update_page(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    draft: CreatePageDraft,
) -> Result<PageMutationResult, TransportError> {
    api::update_page(token, tenant_slug, id, draft).await
}

pub async fn publish_page(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<PageMutationResult, TransportError> {
    api::publish_page(token, tenant_slug, id).await
}

pub async fn unpublish_page(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<PageMutationResult, TransportError> {
    api::unpublish_page(token, tenant_slug, id).await
}

pub async fn delete_page(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<bool, TransportError> {
    api::delete_page(token, tenant_slug, id).await
}
