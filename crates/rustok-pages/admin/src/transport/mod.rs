mod graphql_adapter;
mod scenario_baseline_cas_adapter;
mod scenario_release_adapter;

use crate::model::{
    CreatePageDraft, PageBuilderScenarioReleaseStatus, PageDetail, PageList, PageMutationResult,
};
use rustok_page_builder::runtime_scenario_release::RuntimeScenarioReleaseBaseline;

pub type TransportError = graphql_adapter::ApiError;

pub async fn fetch_pages(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<PageList, TransportError> {
    graphql_adapter::fetch_pages(token, tenant_slug).await
}

pub async fn fetch_page(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<Option<PageDetail>, TransportError> {
    graphql_adapter::fetch_page(token, tenant_slug, id).await
}

pub async fn fetch_page_builder_scenario_baseline(
    token: Option<String>,
    tenant_slug: Option<String>,
    page_id: String,
) -> Result<Option<RuntimeScenarioReleaseBaseline>, TransportError> {
    graphql_adapter::fetch_page_builder_scenario_baseline(token, tenant_slug, page_id).await
}

pub async fn fetch_page_builder_scenario_release_status(
    token: Option<String>,
    tenant_slug: Option<String>,
    page_id: String,
) -> Result<PageBuilderScenarioReleaseStatus, TransportError> {
    scenario_release_adapter::fetch(token, tenant_slug, page_id).await
}

pub async fn save_page_builder_scenario_baseline(
    token: Option<String>,
    tenant_slug: Option<String>,
    page_id: String,
    baseline: RuntimeScenarioReleaseBaseline,
    expected_baseline_hash: Option<String>,
    promotion_note: Option<String>,
) -> Result<RuntimeScenarioReleaseBaseline, TransportError> {
    scenario_baseline_cas_adapter::save(
        token,
        tenant_slug,
        page_id,
        baseline,
        expected_baseline_hash,
        promotion_note,
    )
    .await
}

pub async fn delete_page_builder_scenario_baseline(
    token: Option<String>,
    tenant_slug: Option<String>,
    page_id: String,
    expected_baseline_hash: Option<String>,
) -> Result<bool, TransportError> {
    scenario_baseline_cas_adapter::delete(token, tenant_slug, page_id, expected_baseline_hash).await
}

pub async fn create_page(
    token: Option<String>,
    tenant_slug: Option<String>,
    draft: CreatePageDraft,
) -> Result<PageMutationResult, TransportError> {
    graphql_adapter::create_page(token, tenant_slug, draft).await
}

pub async fn update_page(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    draft: CreatePageDraft,
) -> Result<PageMutationResult, TransportError> {
    graphql_adapter::update_page(token, tenant_slug, id, draft).await
}

pub async fn publish_page(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<PageMutationResult, TransportError> {
    graphql_adapter::publish_page(token, tenant_slug, id).await
}

pub async fn unpublish_page(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<PageMutationResult, TransportError> {
    graphql_adapter::unpublish_page(token, tenant_slug, id).await
}

pub async fn delete_page(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<bool, TransportError> {
    graphql_adapter::delete_page(token, tenant_slug, id).await
}
