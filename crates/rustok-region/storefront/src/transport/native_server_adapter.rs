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
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .ok();
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
            .map_err(ServerFnError::new)?
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
