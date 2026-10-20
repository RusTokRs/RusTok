#![allow(clippy::too_many_arguments)]

use leptos::prelude::*;

use crate::catalog_controls::CatalogListInput;
use crate::model::ProductCatalogFacet;
#[cfg(feature = "ssr")]
use crate::model::ProductCatalogFacetValue;

use super::native_server_adapter::ApiError;

#[cfg(feature = "ssr")]
const PRODUCT_STOREFRONT_FACETS_OWNER: &str = "rustok_product.storefront";
#[cfg(feature = "ssr")]
const PRODUCT_STOREFRONT_FACETS_OPERATION: &str = "storefront_catalog_facets";
#[cfg(feature = "ssr")]
const PRODUCT_STOREFRONT_FACETS_BOUNDARY: &str = "product_storefront_catalog_facets_native";

#[cfg(feature = "ssr")]
fn map_runtime_dependency_error(dependency: &'static str) -> ServerFnError {
    tracing::error!(
        owner = PRODUCT_STOREFRONT_FACETS_OWNER,
        owner_operation = PRODUCT_STOREFRONT_FACETS_OPERATION,
        dependency,
        code = "product.storefront_catalog_facets_runtime_unavailable",
        boundary = PRODUCT_STOREFRONT_FACETS_BOUNDARY,
        "product storefront catalog facets runtime dependency is unavailable"
    );
    ServerFnError::new("Product catalog filters are temporarily unavailable")
}

#[cfg(feature = "ssr")]
fn map_tenant_context_error(request_context: &rustok_api::RequestContext) -> ServerFnError {
    tracing::error!(
        owner = PRODUCT_STOREFRONT_FACETS_OWNER,
        owner_operation = PRODUCT_STOREFRONT_FACETS_OPERATION,
        request_context_present = true,
        correlation_id_length = request_context.correlation_id.chars().count(),
        channel_id_present = request_context.channel_id.is_some(),
        channel_slug_length = request_context
            .channel_slug
            .as_deref()
            .map_or(0, |value| value.chars().count()),
        locale_length = request_context.locale.chars().count(),
        code = "product.storefront_catalog_facets_tenant_context_unavailable",
        boundary = PRODUCT_STOREFRONT_FACETS_BOUNDARY,
        "product storefront tenant context extraction failed"
    );
    ServerFnError::new("Product catalog filters context is unavailable")
}

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

#[server(prefix = "/api/fn", endpoint = "product/storefront/catalog-facets")]
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
        use leptos::prelude::use_context;
        use rustok_api::HostRuntimeContext;
        use rustok_outbox::TransactionalEventBus;
        use rustok_product::{CatalogService, StorefrontProductListQuery};

        let runtime_ctx = use_context::<HostRuntimeContext>()
            .ok_or_else(|| map_runtime_dependency_error("HostRuntimeContext"))?;
        let event_bus = runtime_ctx
            .shared_get::<TransactionalEventBus>()
            .ok_or_else(|| map_runtime_dependency_error("TransactionalEventBus"))?;
        let request_context = super::native_server_adapter::require_request_context(
            PRODUCT_STOREFRONT_FACETS_OPERATION,
            PRODUCT_STOREFRONT_FACETS_BOUNDARY,
        )
        .await?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(|_| map_tenant_context_error(&request_context))?;
        let requested_locale = crate::core::resolve_requested_locale(
            locale,
            Some(request_context.locale.as_str()),
            tenant.default_locale.as_str(),
        );
        let public_channel_slug = request_context
            .channel_slug
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_ascii_lowercase());
        let list_query = StorefrontProductListQuery::try_from_transport_with_attribute_filters(
            search,
            category_id,
            sort_by,
            sort_direction,
            attribute_filters,
        )
        .map_err(|error| map_product_service_error(error, "storefront_catalog_facets_input"))?;
        let facets = CatalogService::new(runtime_ctx.db_clone(), event_bus)
            .storefront_catalog_facets(
                tenant.id,
                requested_locale.as_str(),
                Some(tenant.default_locale.as_str()),
                public_channel_slug.as_deref(),
                &list_query,
                facet_codes.as_slice(),
            )
            .await
            .map_err(|error| map_product_service_error(error, "storefront_catalog_facets"))?;

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
