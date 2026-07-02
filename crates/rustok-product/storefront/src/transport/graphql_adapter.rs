use super::native_server_adapter::{self, ApiError};
use crate::core::ProductStorefrontFetchRequest;
use crate::model::{ProductCatalogSearchOptions, StorefrontProductsData};

const STOREFRONT_CATALOG_SEARCH_OPTIONS_QUERY: &str = "query StorefrontCatalogSearchOptions($locale: String!) { storefrontCatalogSearchOptions(locale: $locale) { categoryOptions { value label } attributeOptions { value label } } }";

#[derive(serde::Serialize)]
struct CatalogSearchOptionsVariables {
    locale: String,
}

#[derive(serde::Deserialize)]
struct CatalogSearchOptionsResponse {
    #[serde(rename = "storefrontCatalogSearchOptions")]
    options: ProductCatalogSearchOptions,
}

pub async fn fetch_products(
    request: ProductStorefrontFetchRequest,
) -> Result<StorefrontProductsData, ApiError> {
    native_server_adapter::fetch_storefront_products_graphql(
        request.selected_handle,
        request.locale,
        request.currency_code,
        request.region_id,
        request.price_list_id,
        request.channel_id,
        request.channel_slug,
        request.quantity,
    )
    .await
}

pub async fn fetch_catalog_search_options(
    locale: String,
) -> Result<ProductCatalogSearchOptions, ApiError> {
    let response: CatalogSearchOptionsResponse = native_server_adapter::request(
        STOREFRONT_CATALOG_SEARCH_OPTIONS_QUERY,
        CatalogSearchOptionsVariables { locale },
    )
    .await?;
    Ok(response.options)
}
