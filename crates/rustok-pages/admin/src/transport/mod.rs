mod graphql_adapter;
mod scenario_baseline_cas_adapter;
mod scenario_release_adapter;

use crate::model::{
    CreatePageDraft, PageBuilderScenarioReleaseStatus, PageDetail, PageList, PageMutationResult,
    PagePublicationResult,
};
use rustok_page_builder::runtime_scenario_release::RuntimeScenarioReleaseBaseline;
use serde_json::Value;

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

#[allow(clippy::too_many_arguments)]
pub async fn patch_page_metadata(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    expected_version: i32,
    locale: String,
    title: String,
    slug: String,
    meta_title: Option<String>,
    meta_description: Option<String>,
    template: Option<String>,
    channel_slugs: Vec<String>,
) -> Result<PageDetail, TransportError> {
    graphql_adapter::patch_page_metadata(
        token,
        tenant_slug,
        id,
        expected_version,
        locale,
        title,
        slug,
        meta_title,
        meta_description,
        template,
        channel_slugs,
    )
    .await
}

pub async fn save_page_document(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    expected_revision: String,
    locale: String,
    project_data: Value,
) -> Result<PageDetail, TransportError> {
    graphql_adapter::save_page_document(
        token,
        tenant_slug,
        id,
        expected_revision,
        locale,
        project_data,
    )
    .await
}

pub async fn publish_page(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<PagePublicationResult, TransportError> {
    let expected_page_id = id.clone();
    let result = graphql_adapter::publish_page(token, tenant_slug, id)
        .await
        .map(PagePublicationResult::Published)?;
    validate_publication_result(&expected_page_id, result)
}

pub async fn rollback_page(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<PagePublicationResult, TransportError> {
    let expected_page_id = id.clone();
    let result = graphql_adapter::rollback_page(token, tenant_slug, id)
        .await
        .map(PagePublicationResult::RolledBack)?;
    validate_publication_result(&expected_page_id, result)
}

pub async fn unpublish_page(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<PagePublicationResult, TransportError> {
    let expected_page_id = id.clone();
    let result = graphql_adapter::unpublish_page(token, tenant_slug, id)
        .await
        .map(PagePublicationResult::Unpublished)?;
    validate_publication_result(&expected_page_id, result)
}

fn validate_publication_result(
    expected_page_id: &str,
    result: PagePublicationResult,
) -> Result<PagePublicationResult, TransportError> {
    if result.page_id() != expected_page_id {
        return Err(TransportError::Graphql(format!(
            "Pages publication returned page `{}` for `{expected_page_id}`",
            result.page_id()
        )));
    }
    if result.version() <= 0 {
        return Err(TransportError::Graphql(format!(
            "Pages publication returned invalid version {} for `{expected_page_id}`",
            result.version()
        )));
    }
    Ok(result)
}

pub async fn delete_page(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<bool, TransportError> {
    graphql_adapter::delete_page(token, tenant_slug, id).await
}
