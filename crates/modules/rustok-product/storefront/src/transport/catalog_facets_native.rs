#![allow(clippy::too_many_arguments)]

use leptos::prelude::*;

use crate::catalog_controls::CatalogListInput;
use crate::model::ProductCatalogFacet;
#[cfg(feature = "ssr")]
use crate::model::ProductCatalogFacetValue;

use super::native_server_adapter::ApiError;

#[cfg(feature = "ssr")]
const PRODUCT_STOREFRONT_FACETS_OPERATION: &str = "storefront_catalog_facets";

#[cfg(feature = "ssr")]
fn map_product_service_error(
    error: rustok_product::CommerceError,
    operation: &'static str,
) -> ServerFnError {
    ServerFnError::new(
        rustok_product::map_product_public_error(
            &error,
            operation,
            "product_storefront_catalog_facets_native",
        )
        .to_string(),
    )
}

/// Maps the owner-resolved facet contract into the storefront facet model.
#[cfg(feature = "ssr")]
fn map_owner_catalog_facets(
    facets: Vec<rustok_product::StorefrontCatalogFacet>,
) -> Vec<ProductCatalogFacet> {
    facets
        .into_iter()
        .map(|facet| ProductCatalogFacet {
            code: facet.code,
            label: facet.label,
            value_type: facet.value_type,
            is_localized: facet.is_localized,
            is_enumerable: facet.is_enumerable,
            is_truncated: facet.is_truncated,
            total_products: facet.total_products,
            values: facet
                .values
                .into_iter()
                .map(|value| ProductCatalogFacetValue {
                    value: value.value,
                    label: value.label,
                    count: value.count,
                })
                .collect(),
        })
        .collect()
}

#[server(prefix = "/api/fn", endpoint = "product/storefront/catalog-facets", client = rustok_auth_ui::ui::leptos::AuthorizedBrowserClient)]
async fn storefront_catalog_facets_native(
    locale: Option<String>,
    search: Option<String>,
    category_id: Option<String>,
    sort_by: Option<String>,
    sort_direction: Option<String>,
    attribute_filters: Vec<String>,
    facet_codes: Vec<String>,
) -> Result<Vec<ProductCatalogFacet>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_product::StorefrontProductListQuery;

        let scope =
            super::native_server_adapter::product_buyer_scope(PRODUCT_STOREFRONT_FACETS_OPERATION)
                .await?;
        let requested_locale = crate::core::resolve_requested_locale(
            locale,
            Some(&scope.request.locale),
            &scope.tenant.default_locale,
        );
        let public_channel_slug = scope.request.channel_slug.clone();
        let list_query = StorefrontProductListQuery::try_from_transport_with_attribute_filters(
            search,
            category_id,
            sort_by,
            sort_direction,
            attribute_filters,
        )
        .map_err(|error| map_product_service_error(error, "storefront_catalog_facets_input"))?;
        let facets = super::native_server_adapter::product_read_call(
            &scope,
            super::native_server_adapter::product_read_port(&scope).load_storefront_catalog_facets(
                super::native_server_adapter::product_read_context(
                    &scope,
                    requested_locale.clone(),
                    public_channel_slug.clone(),
                ),
                rustok_product::StorefrontCatalogFacetsRequest {
                    locale: Some(requested_locale),
                    fallback_locale: Some(scope.tenant.default_locale.clone()),
                    public_channel_slug,
                    query: list_query,
                    facet_codes,
                },
            ),
        )
        .await
        .map_err(super::native_server_adapter::map_product_port_error)?;

        Ok(map_owner_catalog_facets(facets))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (
            locale,
            search,
            category_id,
            sort_by,
            sort_direction,
            attribute_filters,
            facet_codes,
        );
        Err(ServerFnError::new(
            "product/storefront/catalog-facets requires the `ssr` feature",
        ))
    }
}

pub async fn fetch_catalog_facets(
    locale: Option<String>,
    controls: CatalogListInput,
    facet_codes: Vec<String>,
) -> Result<Vec<ProductCatalogFacet>, ApiError> {
    let facets = storefront_catalog_facets_native(
        locale,
        controls.search,
        controls.category_id,
        controls.sort_by,
        controls.sort_direction,
        controls.attribute_filters,
        facet_codes,
    )
    .await
    .map_err(ApiError::from)?;
    Ok(facets)
}
