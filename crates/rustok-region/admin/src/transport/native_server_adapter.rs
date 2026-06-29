use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::{RegionAdminBootstrap, RegionDetail, RegionDraft, RegionList};

#[cfg(feature = "ssr")]
use crate::model::RegionListItem;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    ServerFn(String),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ServerFn(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

pub async fn fetch_bootstrap() -> Result<RegionAdminBootstrap, ApiError> {
    region_bootstrap_native().await.map_err(Into::into)
}

pub async fn fetch_regions() -> Result<RegionList, ApiError> {
    region_list_native().await.map_err(Into::into)
}

pub async fn fetch_region_detail(region_id: String) -> Result<RegionDetail, ApiError> {
    region_detail_native(region_id).await.map_err(Into::into)
}

pub async fn create_region(payload: RegionDraft) -> Result<RegionDetail, ApiError> {
    region_create_native(payload).await.map_err(Into::into)
}

pub async fn update_region(
    region_id: String,
    payload: RegionDraft,
) -> Result<RegionDetail, ApiError> {
    region_update_native(region_id, payload)
        .await
        .map_err(Into::into)
}

#[cfg(feature = "ssr")]
fn ensure_permission(
    permissions: &[rustok_core::Permission],
    required: &[rustok_core::Permission],
    message: &str,
) -> Result<(), ServerFnError> {
    if !rustok_api::has_any_effective_permission(permissions, required) {
        return Err(ServerFnError::new(format!("Permission denied: {message}")));
    }

    Ok(())
}

#[cfg(feature = "ssr")]
fn parse_uuid(value: &str, field_name: &str) -> Result<uuid::Uuid, ServerFnError> {
    uuid::Uuid::parse_str(value.trim())
        .map_err(|_| ServerFnError::new(format!("Invalid {field_name}")))
}

#[cfg(feature = "ssr")]
fn parse_tax_rate(value: &str) -> Result<rust_decimal::Decimal, ServerFnError> {
    <rust_decimal::Decimal as std::str::FromStr>::from_str(value.trim())
        .map_err(|_| ServerFnError::new("Invalid tax_rate"))
}

#[cfg(feature = "ssr")]
fn parse_optional_tax_provider_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(feature = "ssr")]
fn parse_countries(value: &str) -> Result<Vec<String>, ServerFnError> {
    let countries = value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_uppercase())
        .collect::<Vec<_>>();
    if countries.is_empty() {
        return Err(ServerFnError::new(
            "countries must contain at least one country code",
        ));
    }

    Ok(countries)
}

#[cfg(feature = "ssr")]
fn parse_metadata(value: &str) -> Result<serde_json::Value, ServerFnError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Ok(serde_json::json!({}))
    } else {
        serde_json::from_str(trimmed).map_err(|_| ServerFnError::new("Invalid metadata JSON"))
    }
}

#[cfg(feature = "ssr")]
fn parse_country_tax_policies(
    value: &str,
) -> Result<Vec<rustok_region::RegionCountryTaxPolicyInput>, ServerFnError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(trimmed)
        .map_err(|_| ServerFnError::new("Invalid country_tax_policies JSON"))
}

#[cfg(feature = "ssr")]
fn map_current_tenant(tenant: &rustok_api::TenantContext) -> crate::model::CurrentTenant {
    crate::model::CurrentTenant {
        id: tenant.id.to_string(),
        slug: tenant.slug.clone(),
        name: tenant.name.clone(),
    }
}

#[cfg(feature = "ssr")]
fn map_region_list_item(value: rustok_region::RegionResponse) -> crate::model::RegionListItem {
    let countries_preview = value.countries.join(", ");
    let country_count = value.countries.len();
    RegionListItem {
        id: value.id.to_string(),
        name: value.name,
        currency_code: value.currency_code,
        tax_provider_id: value.tax_provider_id,
        country_count,
        tax_rate: value.tax_rate.normalize().to_string(),
        tax_included: value.tax_included,
        countries_preview,
        updated_at: value.updated_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn map_region_record(value: rustok_region::RegionResponse) -> crate::model::RegionRecord {
    crate::model::RegionRecord {
        id: value.id.to_string(),
        tenant_id: value.tenant_id.to_string(),
        name: value.name,
        currency_code: value.currency_code,
        tax_provider_id: value.tax_provider_id,
        tax_rate: value.tax_rate.normalize().to_string(),
        tax_included: value.tax_included,
        country_tax_policies_pretty: serde_json::to_string_pretty(&value.country_tax_policies)
            .unwrap_or_else(|_| "[]".to_string()),
        countries: value.countries,
        metadata_pretty: serde_json::to_string_pretty(&value.metadata)
            .unwrap_or_else(|_| "{}".to_string()),
        created_at: value.created_at.to_rfc3339(),
        updated_at: value.updated_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
async fn load_region_detail(
    region_service: &rustok_region::RegionService,
    tenant: &rustok_api::TenantContext,
    region_id: uuid::Uuid,
    requested_locale: Option<&str>,
) -> Result<RegionDetail, ServerFnError> {
    let region = region_service
        .get_region(
            tenant.id,
            region_id,
            requested_locale,
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(ServerFnError::new)?;

    Ok(RegionDetail {
        region: map_region_record(region),
    })
}

#[server(prefix = "/api/fn", endpoint = "region/bootstrap")]
async fn region_bootstrap_native() -> Result<RegionAdminBootstrap, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{AuthContext, TenantContext};
        use rustok_core::Permission;

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        ensure_permission(
            &auth.permissions,
            &[Permission::REGIONS_LIST, Permission::REGIONS_READ],
            "regions:list or regions:read required",
        )?;

        Ok(RegionAdminBootstrap {
            current_tenant: map_current_tenant(&tenant),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "region/bootstrap requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "region/list")]
async fn region_list_native() -> Result<RegionList, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, RequestContext, TenantContext};
        use rustok_core::Permission;
        use rustok_region::RegionService;

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request_context = leptos_axum::extract::<RequestContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_permission(
            &auth.permissions,
            &[Permission::REGIONS_LIST],
            "regions:list required",
        )?;

        let service = RegionService::new(app_ctx.db.clone());
        let items = service
            .list_regions(
                tenant.id,
                Some(request_context.locale.as_str()),
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(ServerFnError::new)?
            .into_iter()
            .map(map_region_list_item)
            .collect();

        Ok(RegionList { items })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new("region/list requires the `ssr` feature"))
    }
}

#[server(prefix = "/api/fn", endpoint = "region/detail")]
async fn region_detail_native(region_id: String) -> Result<RegionDetail, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, RequestContext, TenantContext};
        use rustok_core::Permission;
        use rustok_region::RegionService;

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request_context = leptos_axum::extract::<RequestContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_permission(
            &auth.permissions,
            &[Permission::REGIONS_READ],
            "regions:read required",
        )?;

        let region_id = parse_uuid(&region_id, "region_id")?;
        let service = RegionService::new(app_ctx.db.clone());

        load_region_detail(
            &service,
            &tenant,
            region_id,
            Some(request_context.locale.as_str()),
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = region_id;
        Err(ServerFnError::new(
            "region/detail requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "region/create")]
async fn region_create_native(payload: RegionDraft) -> Result<RegionDetail, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_core::Permission;
        use rustok_region::{CreateRegionInput, RegionService, RegionTranslationInput};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_permission(
            &auth.permissions,
            &[Permission::REGIONS_CREATE],
            "regions:create required",
        )?;

        let service = RegionService::new(app_ctx.db.clone());
        let requested_locale = payload.locale.trim().to_string();
        let created = service
            .create_region(
                tenant.id,
                CreateRegionInput {
                    translations: vec![RegionTranslationInput {
                        locale: payload.locale.trim().to_string(),
                        name: payload.name.trim().to_string(),
                    }],
                    currency_code: payload.currency_code.trim().to_string(),
                    tax_provider_id: parse_optional_tax_provider_id(&payload.tax_provider_id),
                    tax_rate: parse_tax_rate(&payload.tax_rate)?,
                    tax_included: payload.tax_included,
                    country_tax_policies: Some(parse_country_tax_policies(
                        &payload.country_tax_policies,
                    )?),
                    countries: parse_countries(&payload.countries)?,
                    metadata: parse_metadata(&payload.metadata)?,
                },
            )
            .await
            .map_err(ServerFnError::new)?;

        load_region_detail(
            &service,
            &tenant,
            created.id,
            Some(requested_locale.as_str()),
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = payload;
        Err(ServerFnError::new(
            "region/create requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "region/update")]
async fn region_update_native(
    region_id: String,
    payload: RegionDraft,
) -> Result<RegionDetail, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_core::Permission;
        use rustok_region::{RegionService, RegionTranslationInput, UpdateRegionInput};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_permission(
            &auth.permissions,
            &[Permission::REGIONS_UPDATE],
            "regions:update required",
        )?;

        let region_id = parse_uuid(&region_id, "region_id")?;
        let service = RegionService::new(app_ctx.db.clone());
        let requested_locale = payload.locale.trim().to_string();
        service
            .update_region(
                tenant.id,
                region_id,
                UpdateRegionInput {
                    translations: Some(vec![RegionTranslationInput {
                        locale: payload.locale.trim().to_string(),
                        name: payload.name.trim().to_string(),
                    }]),
                    currency_code: Some(payload.currency_code.trim().to_string()),
                    tax_provider_id: Some(parse_optional_tax_provider_id(&payload.tax_provider_id)),
                    tax_rate: Some(parse_tax_rate(&payload.tax_rate)?),
                    tax_included: Some(payload.tax_included),
                    country_tax_policies: Some(parse_country_tax_policies(
                        &payload.country_tax_policies,
                    )?),
                    countries: Some(parse_countries(&payload.countries)?),
                    metadata: Some(parse_metadata(&payload.metadata)?),
                },
            )
            .await
            .map_err(ServerFnError::new)?;

        load_region_detail(
            &service,
            &tenant,
            region_id,
            Some(requested_locale.as_str()),
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (region_id, payload);
        Err(ServerFnError::new(
            "region/update requires the `ssr` feature",
        ))
    }
}
