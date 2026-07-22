use std::collections::BTreeMap;

use fly::ProjectHash;
#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{GraphqlHttpError, GraphqlRequest, execute as execute_graphql};
use rustok_page_builder::PageBuilderReviewedPublishRuntime;
use rustok_page_builder::runtime_scenario_release::RuntimeScenarioReleaseBaseline;
use rustok_page_builder_admin::{load_publish_scenario_selection, resolve_publish_scenario};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::{
    CreatePageDraft, PageDetail, PageList, PageMutationResult, PublishPageReceipt,
    RollbackPageReceipt,
};

pub type ApiError = GraphqlHttpError;

const PAGES_QUERY: &str = "query PagesAdmin($filter: ListGqlPagesFilter) { pages(filter: $filter) { total items { id status template title slug updatedAt } } }";
const PAGE_QUERY: &str = "query PageAdmin($id: UUID!, $locale: String) { page(id: $id, locale: $locale) { id version status template updatedAt availableLocales channelSlugs translation { locale title slug metaTitle metaDescription } body { locale content format contentJson updatedAt } } }";
const PAGE_BUILDER_SCENARIO_BASELINE_QUERY: &str = "query PageBuilderScenarioBaseline($pageId: UUID!) { pageBuilderScenarioBaseline(pageId: $pageId) { baseline } }";
const CREATE_PAGE_MUTATION: &str = "mutation CreatePage($input: CreateGqlPageInput!) { createPage(input: $input) { id version status updatedAt translation { locale title slug } } }";
const PATCH_PAGE_METADATA_MUTATION: &str = "mutation PatchPageMetadata($id: UUID!, $input: PatchGqlPageMetadataInput!) { patchPageMetadata(id: $id, input: $input) { id version status template updatedAt availableLocales channelSlugs translation { locale title slug metaTitle metaDescription } body { locale content format contentJson updatedAt } } }";
const SAVE_PAGE_DOCUMENT_MUTATION: &str = "mutation SavePageDocument($id: UUID!, $input: SaveGqlPageDocumentInput!) { savePageDocument(id: $id, input: $input) { id version status template updatedAt availableLocales channelSlugs translation { locale title slug } body { locale content format contentJson updatedAt } } }";
const PUBLISH_PAGE_MUTATION: &str = "mutation PublishPage($id: UUID!, $input: PublishGqlPageInput!) { publishPage(id: $id, input: $input) { operationId pageId version idempotencyKey reviewHash sanitizedSetHash artifactSetHash replayed publishedAt } }";
const ROLLBACK_PAGE_MUTATION: &str = "mutation RollbackPage($id: UUID!, $input: RollbackGqlPageInput!) { rollbackPage(id: $id, input: $input) { operationId pageId version idempotencyKey targetPublishOperationId sourceArtifactSetHash targetArtifactSetHash replayed rolledBackAt } }";
const UNPUBLISH_PAGE_MUTATION: &str = "mutation UnpublishPage($id: UUID!) { unpublishPage(id: $id) { id version status updatedAt translation { locale title slug } } }";
const DELETE_PAGE_MUTATION: &str = "mutation DeletePage($id: UUID!) { deletePage(id: $id) }";
const PUBLISH_IDEMPOTENCY_FORMAT: &str = "pages_admin_publish_v1";
const ROLLBACK_IDEMPOTENCY_FORMAT: &str = "pages_admin_rollback_v1";

#[derive(Debug, Deserialize)]
struct PagesResponse {
    pages: PageList,
}

#[derive(Debug, Deserialize)]
struct CreatePageResponse {
    #[serde(rename = "createPage")]
    create_page: PageMutationResult,
}

#[derive(Debug, Deserialize)]
struct PageResponse {
    page: Option<PageDetail>,
}

#[derive(Debug, Deserialize)]
struct PageBuilderScenarioBaselinePayload {
    baseline: Value,
}

#[derive(Debug, Deserialize)]
struct PageBuilderScenarioBaselineResponse {
    #[serde(rename = "pageBuilderScenarioBaseline")]
    page_builder_scenario_baseline: Option<PageBuilderScenarioBaselinePayload>,
}

#[derive(Debug, Deserialize)]
struct PatchPageMetadataResponse {
    #[serde(rename = "patchPageMetadata")]
    patch_page_metadata: PageDetail,
}

#[derive(Debug, Deserialize)]
struct SavePageDocumentResponse {
    #[serde(rename = "savePageDocument")]
    save_page_document: PageDetail,
}

#[derive(Debug, Deserialize)]
struct PublishPageResponse {
    #[serde(rename = "publishPage")]
    publish_page: PublishPageReceipt,
}

#[derive(Debug, Deserialize)]
struct RollbackPageResponse {
    #[serde(rename = "rollbackPage")]
    rollback_page: RollbackPageReceipt,
}

#[derive(Debug, Deserialize)]
struct UnpublishPageResponse {
    #[serde(rename = "unpublishPage")]
    unpublish_page: PageMutationResult,
}

#[derive(Debug, Deserialize)]
struct DeletePageResponse {
    #[serde(rename = "deletePage")]
    delete_page: bool,
}

#[derive(Debug, Serialize)]
struct PagesVariables {
    filter: ListPagesFilter,
}

#[derive(Debug, Serialize)]
struct ListPagesFilter {
    page: u64,
    #[serde(rename = "perPage")]
    per_page: u64,
}

#[derive(Debug, Serialize)]
struct CreatePageVariables {
    input: CreatePageInput,
}

#[derive(Debug, Serialize)]
struct PageWriteVariables<T> {
    id: String,
    input: T,
}

#[derive(Debug, Serialize)]
struct PageVariables {
    id: String,
    locale: Option<String>,
}

#[derive(Debug, Serialize)]
struct CreatePageInput {
    translations: Vec<PageTranslationWriteInput>,
    template: Option<String>,
    body: Option<PageBodyWriteInput>,
    #[serde(rename = "channelSlugs", skip_serializing_if = "Option::is_none")]
    channel_slugs: Option<Vec<String>>,
    publish: Option<bool>,
}

#[derive(Debug, Serialize)]
struct PatchPageMetadataInput {
    #[serde(rename = "expectedVersion")]
    expected_version: i32,
    translations: Option<Vec<PageTranslationWriteInput>>,
    template: Option<String>,
    #[serde(rename = "channelSlugs", skip_serializing_if = "Option::is_none")]
    channel_slugs: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct SavePageDocumentInput {
    #[serde(rename = "expectedRevision")]
    expected_revision: String,
    body: PageBodyWriteInput,
}

#[derive(Debug, Serialize)]
struct PublishPageInput {
    #[serde(rename = "expectedVersion")]
    expected_version: i32,
    #[serde(rename = "expectedBodyRevisions")]
    expected_body_revisions: Vec<PageBodyRevisionInput>,
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    runtime: ReviewedPagePublishRuntimeInput,
}

#[derive(Debug, Serialize)]
struct RollbackPageInput {
    #[serde(rename = "expectedVersion")]
    expected_version: i32,
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
}

#[derive(Debug, Serialize)]
struct PageBodyRevisionInput {
    locale: String,
    revision: String,
}

#[derive(Debug, Serialize)]
struct ReviewedPagePublishRuntimeInput {
    format: String,
    #[serde(rename = "scenarioId")]
    scenario_id: String,
    context: Value,
    #[serde(rename = "reviewHash")]
    review_hash: String,
}

#[derive(Debug, Serialize)]
struct PageTranslationWriteInput {
    locale: String,
    title: String,
    slug: Option<String>,
    #[serde(rename = "metaTitle")]
    meta_title: Option<String>,
    #[serde(rename = "metaDescription")]
    meta_description: Option<String>,
}

#[derive(Debug, Serialize)]
struct PageBodyWriteInput {
    locale: String,
    content: String,
    format: Option<String>,
    #[serde(rename = "contentJson")]
    content_json: Option<Value>,
}

#[derive(Debug, Serialize)]
struct PageIdVariables {
    id: String,
}

#[derive(Debug, Serialize)]
struct PageBuilderScenarioBaselineVariables {
    #[serde(rename = "pageId")]
    page_id: String,
}

fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }

    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}/api/graphql")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}/api/graphql")
    }
}

async fn request<V, T>(
    query: &str,
    variables: V,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, Some(variables)),
        token,
        tenant_slug,
        None,
    )
    .await
}

pub async fn fetch_pages(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<PageList, ApiError> {
    let response: PagesResponse = request(
        PAGES_QUERY,
        PagesVariables {
            filter: ListPagesFilter {
                page: 1,
                per_page: 20,
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.pages)
}

pub async fn fetch_page(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<Option<PageDetail>, ApiError> {
    fetch_page_at_locale(token, tenant_slug, id, None).await
}

async fn fetch_page_at_locale(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    locale: Option<String>,
) -> Result<Option<PageDetail>, ApiError> {
    let response: PageResponse = request(
        PAGE_QUERY,
        PageVariables { id, locale },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.page)
}

pub async fn fetch_page_builder_scenario_baseline(
    token: Option<String>,
    tenant_slug: Option<String>,
    page_id: String,
) -> Result<Option<RuntimeScenarioReleaseBaseline>, ApiError> {
    let response: PageBuilderScenarioBaselineResponse = request(
        PAGE_BUILDER_SCENARIO_BASELINE_QUERY,
        PageBuilderScenarioBaselineVariables { page_id },
        token,
        tenant_slug,
    )
    .await?;
    response
        .page_builder_scenario_baseline
        .map(|payload| serde_json::from_value(payload.baseline))
        .transpose()
        .map_err(|error| {
            GraphqlHttpError::Graphql(format!(
                "Invalid Page Builder scenario baseline response: {error}"
            ))
        })
}

pub async fn create_page(
    token: Option<String>,
    tenant_slug: Option<String>,
    draft: CreatePageDraft,
) -> Result<PageMutationResult, ApiError> {
    let response: CreatePageResponse = request(
        CREATE_PAGE_MUTATION,
        CreatePageVariables {
            input: CreatePageInput {
                translations: vec![PageTranslationWriteInput {
                    locale: draft.locale.clone(),
                    title: draft.title,
                    slug: Some(draft.slug),
                    meta_title: None,
                    meta_description: None,
                }],
                template: draft.template,
                body: Some(PageBodyWriteInput {
                    locale: draft.locale,
                    content: draft.body_content,
                    format: Some(draft.body_format),
                    content_json: Some(draft.body_content_json),
                }),
                channel_slugs: Some(draft.channel_slugs),
                publish: Some(false),
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.create_page)
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
) -> Result<PageDetail, ApiError> {
    let response: PatchPageMetadataResponse = request(
        PATCH_PAGE_METADATA_MUTATION,
        PageWriteVariables {
            id,
            input: PatchPageMetadataInput {
                expected_version,
                translations: Some(vec![PageTranslationWriteInput {
                    locale,
                    title,
                    slug: Some(slug),
                    meta_title,
                    meta_description,
                }]),
                template,
                channel_slugs: Some(channel_slugs),
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.patch_page_metadata)
}

pub async fn save_page_document(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    expected_revision: String,
    locale: String,
    project_data: Value,
) -> Result<PageDetail, ApiError> {
    let response: SavePageDocumentResponse = request(
        SAVE_PAGE_DOCUMENT_MUTATION,
        PageWriteVariables {
            id,
            input: SavePageDocumentInput {
                expected_revision,
                body: PageBodyWriteInput {
                    locale,
                    content: String::new(),
                    format: Some("grapesjs".to_string()),
                    content_json: Some(project_data),
                },
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.save_page_document)
}

pub async fn publish_page(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<PublishPageReceipt, ApiError> {
    let page = fetch_page(token.clone(), tenant_slug.clone(), id.clone())
        .await?
        .ok_or_else(|| GraphqlHttpError::Graphql("Page was not found".to_string()))?;
    let revisions =
        fetch_page_body_revisions(token.clone(), tenant_slug.clone(), &page).await?;
    let baseline = fetch_page_builder_scenario_baseline(
        token.clone(),
        tenant_slug.clone(),
        id.clone(),
    )
    .await?
    .ok_or_else(|| {
        GraphqlHttpError::Graphql(
            "Publish requires a promoted Page Builder runtime scenario baseline".to_string(),
        )
    })?;
    let selected_scenario_id = load_publish_scenario_selection(&id, &baseline.baseline_hash)
        .map_err(|error| GraphqlHttpError::Graphql(error.to_string()))?;
    let scenario = resolve_publish_scenario(&baseline, selected_scenario_id.as_deref())
        .map_err(|error| GraphqlHttpError::Graphql(error.to_string()))?;
    let reviewed =
        PageBuilderReviewedPublishRuntime::new(scenario.id.clone(), scenario.context.clone())
            .map_err(|error| {
                GraphqlHttpError::Graphql(format!(
                    "Unable to prepare reviewed Page Builder runtime: {error}"
                ))
            })?;
    let expected_body_revisions = revisions
        .iter()
        .map(|(locale, revision)| PageBodyRevisionInput {
            locale: locale.clone(),
            revision: revision.clone(),
        })
        .collect::<Vec<_>>();
    let idempotency_key = publish_idempotency_key(&page, &revisions, &reviewed)?;

    let response: PublishPageResponse = request(
        PUBLISH_PAGE_MUTATION,
        PageWriteVariables {
            id,
            input: PublishPageInput {
                expected_version: page.version,
                expected_body_revisions,
                idempotency_key,
                runtime: ReviewedPagePublishRuntimeInput {
                    format: reviewed.format,
                    scenario_id: reviewed.scenario_id,
                    context: reviewed.context,
                    review_hash: reviewed.review_hash,
                },
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.publish_page)
}

pub async fn rollback_page(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<RollbackPageReceipt, ApiError> {
    let page = fetch_page(token.clone(), tenant_slug.clone(), id.clone())
        .await?
        .ok_or_else(|| GraphqlHttpError::Graphql("Page was not found".to_string()))?;
    if page.status != "published" {
        return Err(GraphqlHttpError::Graphql(
            "Only a currently published page can be rolled back".to_string(),
        ));
    }
    let idempotency_key = rollback_idempotency_key(&page)?;
    let response: RollbackPageResponse = request(
        ROLLBACK_PAGE_MUTATION,
        PageWriteVariables {
            id,
            input: RollbackPageInput {
                expected_version: page.version,
                idempotency_key,
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.rollback_page)
}

async fn fetch_page_body_revisions(
    token: Option<String>,
    tenant_slug: Option<String>,
    page: &PageDetail,
) -> Result<BTreeMap<String, String>, ApiError> {
    let mut revisions = BTreeMap::new();
    if let Some(body) = page.body.as_ref() {
        revisions.insert(body.locale.clone(), body.updated_at.clone());
    }
    for locale in &page.available_locales {
        let localized = fetch_page_at_locale(
            token.clone(),
            tenant_slug.clone(),
            page.id.clone(),
            Some(locale.clone()),
        )
        .await?;
        if let Some(body) = localized.and_then(|page| page.body) {
            revisions.insert(body.locale, body.updated_at);
        }
    }
    if revisions.is_empty() {
        return Err(GraphqlHttpError::Graphql(
            "Publish requires at least one localized Page Builder body".to_string(),
        ));
    }
    Ok(revisions)
}

fn publish_idempotency_key(
    page: &PageDetail,
    revisions: &BTreeMap<String, String>,
    reviewed: &PageBuilderReviewedPublishRuntime,
) -> Result<String, ApiError> {
    let bytes = serde_json::to_vec(&(
        PUBLISH_IDEMPOTENCY_FORMAT,
        page.id.as_str(),
        page.version,
        revisions,
        reviewed.review_hash.as_str(),
    ))
    .map_err(|error| {
        GraphqlHttpError::Graphql(format!(
            "Unable to encode Page Builder publish identity: {error}"
        ))
    })?;
    Ok(format!(
        "pages-admin-v1:{}:{}:{}",
        page.id,
        page.version,
        ProjectHash::from_bytes(&bytes).hex()
    ))
}

fn rollback_idempotency_key(page: &PageDetail) -> Result<String, ApiError> {
    let bytes = serde_json::to_vec(&(
        ROLLBACK_IDEMPOTENCY_FORMAT,
        page.id.as_str(),
        page.version,
    ))
    .map_err(|error| {
        GraphqlHttpError::Graphql(format!("Unable to encode page rollback identity: {error}"))
    })?;
    Ok(format!(
        "pages-rollback-v1:{}:{}:{}",
        page.id,
        page.version,
        ProjectHash::from_bytes(&bytes).hex()
    ))
}

pub async fn unpublish_page(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<PageMutationResult, ApiError> {
    let response: UnpublishPageResponse = request(
        UNPUBLISH_PAGE_MUTATION,
        PageIdVariables { id },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.unpublish_page)
}

pub async fn delete_page(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<bool, ApiError> {
    let response: DeletePageResponse = request(
        DELETE_PAGE_MUTATION,
        PageIdVariables { id },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.delete_page)
}
