use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};

use crate::model::{PricingAdminBootstrap, PricingProductDetail, PricingProductList};

pub type ApiError = GraphqlHttpError;

const BOOTSTRAP_QUERY: &str = "query PricingAdminBootstrap { currentTenant { id slug name } }";
const PRODUCTS_QUERY: &str = "query PricingAdminProducts($tenantId: UUID!, $locale: String, $filter: ProductsFilter) { products(tenantId: $tenantId, locale: $locale, filter: $filter) { total page perPage hasNext items { id status title handle vendor productType shippingProfileSlug tags createdAt publishedAt } } }";
const PRODUCT_QUERY: &str = "query PricingAdminProduct($tenantId: UUID!, $id: UUID!, $locale: String) { product(tenantId: $tenantId, id: $id, locale: $locale) { id status vendor productType shippingProfileSlug createdAt updatedAt publishedAt translations { locale title handle description } variants { id sku barcode shippingProfileSlug title option1 option2 option3 prices { currencyCode amount compareAtAmount onSale } } } }";

#[derive(Debug, Deserialize)]
struct BootstrapResponse {
    #[serde(rename = "currentTenant")]
    current_tenant: crate::model::CurrentTenant,
}

#[derive(Debug, Deserialize)]
struct ProductsResponse {
    products: PricingProductList,
}

#[derive(Debug, Deserialize)]
struct ProductResponse {
    product: Option<PricingProductDetail>,
}

#[derive(Debug, Serialize)]
struct TenantScopedVariables<T> {
    #[serde(rename = "tenantId")]
    tenant_id: String,
    #[serde(flatten)]
    extra: T,
}

#[derive(Debug, Serialize)]
struct ProductsVariables {
    locale: Option<String>,
    filter: ProductsFilter,
}

#[derive(Debug, Serialize)]
struct ProductVariables {
    id: String,
    locale: Option<String>,
}

#[derive(Debug, Serialize)]
struct ProductsFilter {
    status: Option<String>,
    vendor: Option<String>,
    search: Option<String>,
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

async fn request<V, T>(
    query: &str,
    variables: Option<V>,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, variables),
        token,
        tenant_slug,
        None,
    )
    .await
}

pub async fn fetch_bootstrap(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<PricingAdminBootstrap, ApiError> {
    let response: BootstrapResponse =
        request::<serde_json::Value, BootstrapResponse>(BOOTSTRAP_QUERY, None, token, tenant_slug)
            .await?;
    Ok(PricingAdminBootstrap {
        current_tenant: response.current_tenant,
    })
}

pub async fn fetch_products(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: String,
    search: Option<String>,
    status: Option<String>,
) -> Result<PricingProductList, ApiError> {
    let response: ProductsResponse = request(
        PRODUCTS_QUERY,
        Some(TenantScopedVariables {
            tenant_id,
            extra: ProductsVariables {
                locale: Some(locale),
                filter: ProductsFilter {
                    status,
                    vendor: None,
                    search,
                    page: Some(1),
                    per_page: Some(24),
                },
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.products)
}

pub async fn fetch_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
    locale: String,
) -> Result<Option<PricingProductDetail>, ApiError> {
    let response: ProductResponse = request(
        PRODUCT_QUERY,
        Some(TenantScopedVariables {
            tenant_id,
            extra: ProductVariables {
                id,
                locale: Some(locale),
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.product)
}
