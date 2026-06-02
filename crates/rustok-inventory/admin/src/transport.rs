#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::core::{
    normalized_product_selector, normalized_products_filter, InventoryProductRequest,
    InventoryProductsRequest,
};
use crate::model::{InventoryAdminBootstrap, InventoryProductDetail, InventoryProductList};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InventoryTransportError {
    Network,
    Graphql(String),
    Http(String),
    Unauthorized,
}

impl fmt::Display for InventoryTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network => formatter.write_str("Network error"),
            Self::Graphql(message) => write!(formatter, "GraphQL error: {message}"),
            Self::Http(message) => write!(formatter, "Http error: {message}"),
            Self::Unauthorized => formatter.write_str("Unauthorized"),
        }
    }
}

impl std::error::Error for InventoryTransportError {}

impl From<GraphqlHttpError> for InventoryTransportError {
    fn from(error: GraphqlHttpError) -> Self {
        match error {
            GraphqlHttpError::Network => Self::Network,
            GraphqlHttpError::Graphql(message) => Self::Graphql(message),
            GraphqlHttpError::Http(message) => Self::Http(message),
            GraphqlHttpError::Unauthorized => Self::Unauthorized,
        }
    }
}

pub trait InventoryReadTransport {
    async fn fetch_bootstrap(
        &self,
        token: Option<String>,
        tenant_slug: Option<String>,
    ) -> Result<InventoryAdminBootstrap, InventoryTransportError>;

    async fn fetch_products(
        &self,
        request: InventoryProductsRequest,
    ) -> Result<InventoryProductList, InventoryTransportError>;

    async fn fetch_product(
        &self,
        request: InventoryProductRequest,
    ) -> Result<Option<InventoryProductDetail>, InventoryTransportError>;
}

#[derive(Clone, Debug, Default)]
pub struct CommerceGraphqlInventoryReadAdapter;

const BOOTSTRAP_QUERY: &str = "query InventoryAdminBootstrap { currentTenant { id slug name } }";
const PRODUCTS_QUERY: &str = "query InventoryAdminProducts($tenantId: UUID!, $locale: String, $filter: ProductsFilter) { products(tenantId: $tenantId, locale: $locale, filter: $filter) { total page perPage hasNext items { id status title handle vendor productType shippingProfileSlug tags createdAt publishedAt } } }";
const PRODUCT_QUERY: &str = "query InventoryAdminProduct($tenantId: UUID!, $id: UUID!, $locale: String) { product(tenantId: $tenantId, id: $id, locale: $locale) { id status vendor productType shippingProfileSlug createdAt updatedAt publishedAt translations { locale title handle description } variants { id sku barcode shippingProfileSlug title option1 option2 option3 inventoryQuantity inventoryPolicy inStock prices { currencyCode amount compareAtAmount onSale } } } }";

#[derive(Debug, Deserialize)]
struct BootstrapResponse {
    #[serde(rename = "currentTenant")]
    current_tenant: crate::model::CurrentTenant,
}

#[derive(Debug, Deserialize)]
struct ProductsResponse {
    products: InventoryProductList,
}

#[derive(Debug, Deserialize)]
struct ProductResponse {
    product: Option<InventoryProductDetail>,
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

fn products_variables(
    tenant_id: String,
    locale: Option<String>,
    search: Option<String>,
    status: Option<String>,
) -> TenantScopedVariables<ProductsVariables> {
    let filter = normalized_products_filter(locale, search, status);
    TenantScopedVariables {
        tenant_id,
        extra: ProductsVariables {
            locale: filter.locale,
            filter: ProductsFilter {
                status: filter.status,
                vendor: None,
                search: filter.search,
                page: Some(filter.page),
                per_page: Some(filter.per_page),
            },
        },
    }
}

fn product_variables(
    tenant_id: String,
    id: String,
    locale: Option<String>,
) -> TenantScopedVariables<ProductVariables> {
    let selector = normalized_product_selector(id, locale);
    TenantScopedVariables {
        tenant_id,
        extra: ProductVariables {
            id: selector.id,
            locale: selector.locale,
        },
    }
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
) -> Result<T, InventoryTransportError>
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
    .map_err(InventoryTransportError::from)
}

impl InventoryReadTransport for CommerceGraphqlInventoryReadAdapter {
    async fn fetch_bootstrap(
        &self,
        token: Option<String>,
        tenant_slug: Option<String>,
    ) -> Result<InventoryAdminBootstrap, InventoryTransportError> {
        let response: BootstrapResponse = request::<serde_json::Value, BootstrapResponse>(
            BOOTSTRAP_QUERY,
            None,
            token,
            tenant_slug,
        )
        .await?;
        Ok(InventoryAdminBootstrap {
            current_tenant: response.current_tenant,
        })
    }

    async fn fetch_products(
        &self,
        request_data: InventoryProductsRequest,
    ) -> Result<InventoryProductList, InventoryTransportError> {
        let response: ProductsResponse = request(
            PRODUCTS_QUERY,
            Some(products_variables(
                request_data.tenant_id,
                request_data.locale,
                request_data.search,
                request_data.status,
            )),
            request_data.token,
            request_data.tenant_slug,
        )
        .await?;
        Ok(response.products)
    }

    async fn fetch_product(
        &self,
        request_data: InventoryProductRequest,
    ) -> Result<Option<InventoryProductDetail>, InventoryTransportError> {
        let response: ProductResponse = request(
            PRODUCT_QUERY,
            Some(product_variables(
                request_data.tenant_id,
                request_data.id,
                request_data.locale,
            )),
            request_data.token,
            request_data.tenant_slug,
        )
        .await?;
        Ok(response.product)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        product_variables, products_variables, GraphqlHttpError, InventoryTransportError,
        PRODUCTS_QUERY, PRODUCT_QUERY,
    };

    #[test]
    fn inventory_transport_error_maps_graphql_runtime_errors_without_leaking_type() {
        assert_eq!(
            InventoryTransportError::from(GraphqlHttpError::Network),
            InventoryTransportError::Network
        );
        assert_eq!(
            InventoryTransportError::from(GraphqlHttpError::Unauthorized),
            InventoryTransportError::Unauthorized
        );
        assert_eq!(
            InventoryTransportError::from(GraphqlHttpError::Graphql("boom".to_string())),
            InventoryTransportError::Graphql("boom".to_string())
        );
        assert_eq!(
            InventoryTransportError::from(GraphqlHttpError::Http("500".to_string())),
            InventoryTransportError::Http("500".to_string())
        );
    }

    #[test]
    fn products_variables_apply_core_normalization_and_paging_defaults() {
        let json = serde_json::to_value(products_variables(
            "tenant-1".to_string(),
            Some("  de-DE  ".to_string()),
            Some("  winter jacket  ".to_string()),
            Some(" active ".to_string()),
        ))
        .expect("products variables should serialize");

        assert_eq!(json["tenantId"], "tenant-1");
        assert_eq!(json["locale"], "de-DE");
        assert_eq!(json["filter"]["status"], "ACTIVE");
        assert_eq!(json["filter"]["search"], "winter jacket");
        assert_eq!(json["filter"]["page"], 1);
        assert_eq!(json["filter"]["perPage"], 24);
        assert!(json["filter"]["vendor"].is_null());
    }

    #[test]
    fn products_variables_drop_blank_optional_filters() {
        let json = serde_json::to_value(products_variables(
            "tenant-1".to_string(),
            Some("   ".to_string()),
            Some("   ".to_string()),
            Some("   ".to_string()),
        ))
        .expect("products variables should serialize");

        assert_eq!(json["tenantId"], "tenant-1");
        assert!(json["locale"].is_null());
        assert!(json["filter"]["status"].is_null());
        assert!(json["filter"]["search"].is_null());
    }

    #[test]
    fn product_variables_trim_locale_without_rewriting_id() {
        let json = serde_json::to_value(product_variables(
            "tenant-1".to_string(),
            " product-1 ".to_string(),
            Some("  en-US  ".to_string()),
        ))
        .expect("product variables should serialize");

        assert_eq!(json["tenantId"], "tenant-1");
        assert_eq!(json["id"], " product-1 ");
        assert_eq!(json["locale"], "en-US");
    }

    #[test]
    fn transitional_graphql_adapter_keeps_inventory_read_model_fields() {
        for expected in [
            "inventoryQuantity",
            "inventoryPolicy",
            "inStock",
            "variants",
            "translations",
            "shippingProfileSlug",
        ] {
            assert!(
                PRODUCT_QUERY.contains(expected),
                "product query must keep inventory read model field {expected}"
            );
        }

        for expected in ["total", "page", "perPage", "hasNext", "items"] {
            assert!(
                PRODUCTS_QUERY.contains(expected),
                "products query must keep inventory feed field {expected}"
            );
        }
    }
}
