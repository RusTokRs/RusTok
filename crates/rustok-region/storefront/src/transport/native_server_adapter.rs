use leptos::prelude::*;
#[cfg(feature = "ssr")]
use rustok_ui_core::normalize_optional_ui_text;

#[cfg(feature = "ssr")]
use crate::core::resolve_storefront_regions;
#[cfg(feature = "ssr")]
use crate::model::StorefrontRegion;
use crate::model::StorefrontRegionsData;

use super::ApiError;

#[cfg(feature = "ssr")]
const REGION_STOREFRONT_NATIVE_OWNER: &str = "rustok_region.storefront";
#[cfg(feature = "ssr")]
const REGION_STOREFRONT_NATIVE_BOUNDARY: &str = "region_storefront_native_transport";

#[cfg(feature = "ssr")]
fn record_optional_request_context_error<E: std::fmt::Display>(error: E) {
    tracing::warn!(
        error = %error,
        owner = REGION_STOREFRONT_NATIVE_OWNER,
        owner_operation = "storefront_regions",
        code = "region.storefront_request_context_unavailable",
        boundary = REGION_STOREFRONT_NATIVE_BOUNDARY,
        "optional region storefront request context extraction failed"
    );
}

#[cfg(feature = "ssr")]
fn map_tenant_context_error<E: std::fmt::Display>(
    request_context: Option<&rustok_api::RequestContext>,
    error: E,
) -> ServerFnError {
    if let Some(request_context) = request_context {
        tracing::error!(
            error = %error,
            owner = REGION_STOREFRONT_NATIVE_OWNER,
            owner_operation = "storefront_regions",
            correlation_id = %request_context.correlation_id,
            tenant_id = %request_context.tenant_id,
            channel_id = ?request_context.channel_id,
            channel_slug = ?request_context.channel_slug,
            locale = %request_context.locale,
            code = "region.storefront_tenant_context_unavailable",
            boundary = REGION_STOREFRONT_NATIVE_BOUNDARY,
            "region storefront tenant context extraction failed"
        );
    } else {
        tracing::error!(
            error = %error,
            owner = REGION_STOREFRONT_NATIVE_OWNER,
            owner_operation = "storefront_regions",
            code = "region.storefront_tenant_context_unavailable",
            boundary = REGION_STOREFRONT_NATIVE_BOUNDARY,
            "region storefront tenant context extraction failed without request context"
        );
    }
    ServerFnError::new("Region storefront context is unavailable")
}

#[cfg(feature = "ssr")]
fn map_region_runtime_error<E: std::fmt::Display>(
    tenant: &rustok_api::TenantContext,
    request_context: Option<&rustok_api::RequestContext>,
    error: E,
) -> ServerFnError {
    if let Some(request_context) = request_context {
        tracing::error!(
            error = %error,
            owner = REGION_STOREFRONT_NATIVE_OWNER,
            owner_operation = "list_regions",
            correlation_id = %request_context.correlation_id,
            tenant_id = %tenant.id,
            channel_id = ?request_context.channel_id,
            channel_slug = ?request_context.channel_slug,
            locale = %request_context.locale,
            code = "region.storefront_owner_runtime_failed",
            boundary = REGION_STOREFRONT_NATIVE_BOUNDARY,
            "region storefront owner operation failed"
        );
    } else {
        tracing::error!(
            error = %error,
            owner = REGION_STOREFRONT_NATIVE_OWNER,
            owner_operation = "list_regions",
            tenant_id = %tenant.id,
            code = "region.storefront_owner_runtime_failed",
            boundary = REGION_STOREFRONT_NATIVE_BOUNDARY,
            "region storefront owner operation failed without request context"
        );
    }
    ServerFnError::new("Storefront regions are temporarily unavailable")
}

#[cfg(feature = "ssr")]
fn map_region(value: rustok_region::RegionResponse) -> StorefrontRegion {
    StorefrontRegion {
        id: value.id.to_string(),
        name: value.name,
        currency_code: value.currency_code,
        tax_provider_id: value.tax_provider_id,
        tax_rate: value.tax_rate.normalize().to_string(),
        tax_included: value.tax_included,
        country_tax_policies: value
            .country_tax_policies
            .into_iter()
            .map(|policy| crate::model::StorefrontRegionCountryTaxPolicy {
                country_code: policy.country_code,
                tax_rate: policy.tax_rate.normalize().to_string(),
                tax_included: policy.tax_included,
            })
            .collect(),
        countries: value.countries,
    }
}

#[cfg(feature = "ssr")]
fn resolve_requested_locale(
    requested: Option<String>,
    request_context_locale: Option<&str>,
    tenant_default_locale: &str,
) -> String {
    normalize_optional_ui_text(requested)
        .or_else(|| {
            request_context_locale
                .and_then(|value| normalize_optional_ui_text(Some(value.to_string())))
        })
        .or_else(|| normalize_optional_ui_text(Some(tenant_default_locale.to_string())))
        .unwrap_or_default()
}

pub async fn fetch_regions(
    selected_region_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontRegionsData, ApiError> {
    fetch_storefront_regions_server(selected_region_id, locale)
        .await
        .map_err(ApiError::from)
}

#[server(prefix = "/api/fn", endpoint = "region/storefront-data")]
async fn fetch_storefront_regions_server(
    selected_region_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontRegionsData, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::HostRuntimeContext;
        use rustok_region::RegionService;

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let request_context = match leptos_axum::extract::<rustok_api::RequestContext>().await {
            Ok(request_context) => Some(request_context),
            Err(error) => {
                record_optional_request_context_error(error);
                None
            }
        };
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(|error| map_tenant_context_error(request_context.as_ref(), error))?;
        let requested_locale = resolve_requested_locale(
            locale,
            request_context
                .as_ref()
                .map(|context| context.locale.as_str()),
            tenant.default_locale.as_str(),
        );
        let regions = RegionService::new(runtime_ctx.db_clone())
            .list_regions(
                tenant.id,
                Some(requested_locale.as_str()),
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(|error| map_region_runtime_error(&tenant, request_context.as_ref(), error))?
            .into_iter()
            .map(map_region)
            .collect();

        Ok(resolve_storefront_regions(regions, selected_region_id))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (selected_region_id, locale);
        Err(ServerFnError::new(
            "region/storefront-data requires the `ssr` feature",
        ))
    }
}
