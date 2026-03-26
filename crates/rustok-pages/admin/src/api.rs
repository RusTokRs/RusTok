use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};

use crate::model::{CreatePageDraft, PageDetail, PageList, PageMutationResult};

pub type ApiError = GraphqlHttpError;

const PAGES_QUERY: &str = "query PagesAdmin($filter: ListGqlPagesFilter) { pages(filter: $filter) { total items { id status template title slug updatedAt } } }";
const PAGE_QUERY: &str =
    "query PageAdmin($id: UUID!) { page(id: $id) { id status template channelSlugs translation { locale title slug } body { locale content format } } }";
const CREATE_PAGE_MUTATION: &str = "mutation CreatePage($input: CreateGqlPageInput!) { createPage(input: $input) { id status updatedAt translation { locale title slug } } }";
const UPDATE_PAGE_MUTATION: &str = "mutation UpdatePage($id: UUID!, $input: UpdateGqlPageInput!) { updatePage(id: $id, input: $input) { id status updatedAt translation { locale title slug } } }";
const PUBLISH_PAGE_MUTATION: &str =
    "mutation PublishPage($id: UUID!) { publishPage(id: $id) { id status updatedAt translation { locale title slug } } }";
const UNPUBLISH_PAGE_MUTATION: &str =
    "mutation UnpublishPage($id: UUID!) { unpublishPage(id: $id) { id status updatedAt translation { locale title slug } } }";
const DELETE_PAGE_MUTATION: &str = "mutation DeletePage($id: UUID!) { deletePage(id: $id) }";

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
struct UpdatePageResponse {
    #[serde(rename = "updatePage")]
    update_page: PageMutationResult,
}

#[derive(Debug, Deserialize)]
struct PublishPageResponse {
    #[serde(rename = "publishPage")]
    publish_page: PageMutationResult,
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
struct UpdatePageVariables {
    id: String,
    input: UpdatePageInput,
}

#[derive(Debug, Serialize)]
struct CreatePageInput {
    translations: Vec<CreatePageTranslationInput>,
    template: Option<String>,
    body: Option<CreatePageBodyInput>,
    #[serde(rename = "channelSlugs", skip_serializing_if = "Option::is_none")]
    channel_slugs: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    blocks: Option<Vec<()>>,
    publish: Option<bool>,
}

#[derive(Debug, Serialize)]
struct UpdatePageInput {
    translations: Option<Vec<CreatePageTranslationInput>>,
    template: Option<String>,
    body: Option<CreatePageBodyInput>,
    #[serde(rename = "channelSlugs", skip_serializing_if = "Option::is_none")]
    channel_slugs: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct CreatePageTranslationInput {
    locale: String,
    title: String,
    slug: Option<String>,
    #[serde(rename = "metaTitle")]
    meta_title: Option<String>,
    #[serde(rename = "metaDescription")]
    meta_description: Option<String>,
}

#[derive(Debug, Serialize)]
struct CreatePageBodyInput {
    locale: String,
    content: String,
    format: Option<String>,
}

#[derive(Debug, Serialize)]
struct PageIdVariables {
    id: String,
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
    let response: PageResponse =
        request(PAGE_QUERY, PageIdVariables { id }, token, tenant_slug).await?;
    Ok(response.page)
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
                translations: vec![CreatePageTranslationInput {
                    locale: draft.locale.clone(),
                    title: draft.title,
                    slug: Some(draft.slug),
                    meta_title: None,
                    meta_description: None,
                }],
                template: draft.template,
                body: Some(CreatePageBodyInput {
                    locale: draft.locale,
                    content: draft.body,
                    format: Some("markdown".to_string()),
                }),
                channel_slugs: Some(draft.channel_slugs),
                blocks: None,
                publish: Some(draft.publish),
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.create_page)
}

pub async fn update_page(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
    draft: CreatePageDraft,
) -> Result<PageMutationResult, ApiError> {
    let response: UpdatePageResponse = request(
        UPDATE_PAGE_MUTATION,
        UpdatePageVariables {
            id,
            input: UpdatePageInput {
                translations: Some(vec![CreatePageTranslationInput {
                    locale: draft.locale.clone(),
                    title: draft.title,
                    slug: Some(draft.slug),
                    meta_title: None,
                    meta_description: None,
                }]),
                template: draft.template,
                body: Some(CreatePageBodyInput {
                    locale: draft.locale,
                    content: draft.body,
                    format: Some("markdown".to_string()),
                }),
                channel_slugs: Some(draft.channel_slugs),
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.update_page)
}

pub async fn publish_page(
    token: Option<String>,
    tenant_slug: Option<String>,
    id: String,
) -> Result<PageMutationResult, ApiError> {
    let response: PublishPageResponse = request(
        PUBLISH_PAGE_MUTATION,
        PageIdVariables { id },
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.publish_page)
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
