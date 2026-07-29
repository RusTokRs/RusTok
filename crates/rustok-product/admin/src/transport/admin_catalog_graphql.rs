#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{GraphqlHttpError, GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

use crate::catalog_controls::ProductAdminListInput;
use crate::model::ProductList;

const ADMIN_PRODUCT_CATALOG_QUERY: &str = "query ProductAdminCatalog($tenantId: UUID!, $locale: String, $filter: AdminProductCatalogFilter) { adminProductCatalog(tenantId: $tenantId, locale: $locale, filter: $filter) { total page perPage hasNext items { id status title handle sellerId vendor productType shippingProfileSlug primaryCategoryId tags createdAt publishedAt } } }";

#[derive(Debug, Deserialize)]
struct AdminProductCatalogResponse {
    #[serde(rename = "adminProductCatalog")]
    admin_product_catalog: ProductList,
}

#[derive(Debug, Serialize)]
struct AdminProductCatalogVariables {
    #[serde(rename = "tenantId")]
    tenant_id: String,
    locale: Option<String>,
    filter: AdminProductCatalogFilter,
}

#[derive(Debug, Serialize)]
struct AdminProductCatalogFilter {
    search: Option<String>,
    status: Option<String>,
    #[serde(rename = "categoryId")]
    category_id: Option<String>,
    #[serde(rename = "sortBy")]
    sort_by: Option<String>,
    #[serde(rename = "sortDirection")]
    sort_direction: Option<String>,
    #[serde(rename = "attributeFilters")]
    attribute_filters: Vec<String>,
    page: Option<u64>,
    #[serde(rename = "perPage")]
    per_page: Option<u64>,
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

pub(super) async fn fetch_products(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: Option<String>,
    controls: ProductAdminListInput,
) -> Result<ProductList, GraphqlHttpError> {
    let response: AdminProductCatalogResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            ADMIN_PRODUCT_CATALOG_QUERY,
            Some(AdminProductCatalogVariables {
                tenant_id,
                locale,
                filter: AdminProductCatalogFilter {
                    search: controls.search,
                    status: controls.status,
                    category_id: controls.category_id,
                    sort_by: controls.sort_by,
                    sort_direction: controls.sort_direction,
                    attribute_filters: controls.attribute_filters,
                    page: Some(1),
                    per_page: Some(24),
                },
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await?;
    Ok(response.admin_product_catalog)
}
