use rustok_graphql::{GraphqlRequest, execute as execute_graphql, graphql_url};
use serde::{Deserialize, Serialize};

use super::{ApiError, configured_tenant_slug};
use crate::model::{PageDetail, PageList, StorefrontPagesData};

const STOREFRONT_PAGES_QUERY: &str = r#"query StorefrontPages($pageSlug: String!, $filter: ListGqlPagesFilter, $locale: String) {
  selectedPage: pageBySlug(slug: $pageSlug, locale: $locale) {
    effectiveLocale
    translation { locale title slug metaTitle metaDescription }
    body { locale content format }
  }
  pages(filter: $filter) { total items { id title slug status template } }
}"#;

#[derive(Debug, Deserialize)]
struct StorefrontPagesResponse {
    #[serde(rename = "selectedPage")]
    selected_page: Option<PageDetail>,
    pages: PageList,
}

#[derive(Debug, Serialize)]
struct StorefrontPagesVariables {
    #[serde(rename = "pageSlug")]
    page_slug: String,
    filter: ListPagesFilter,
    locale: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct ListPagesFilter {
    page: u64,
    #[serde(rename = "perPage")]
    per_page: u64,
}

pub async fn fetch_storefront_pages(
    page_slug: String,
    locale: Option<String>,
) -> Result<StorefrontPagesData, ApiError> {
    let response: StorefrontPagesResponse = request(
        STOREFRONT_PAGES_QUERY,
        StorefrontPagesVariables {
            page_slug,
            filter: ListPagesFilter {
                page: 1,
                per_page: 6,
            },
            locale,
        },
    )
    .await?;

    Ok(StorefrontPagesData {
        selected_page: response.selected_page,
        pages: response.pages,
    })
}


async fn request<V, T>(query: &str, variables: V) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, Some(variables)),
        None,
        configured_tenant_slug(),
        None,
    )
    .await
    .map_err(|error| ApiError::Graphql(error.to_string()))
}
