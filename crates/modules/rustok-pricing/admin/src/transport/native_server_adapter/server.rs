#![allow(unused_imports, clippy::empty_line_after_outer_attr)]
use leptos::prelude::*;
use uuid::Uuid;

use crate::core::{parse_optional_uuid_string, sanitize_resolution_context};
#[cfg(feature = "ssr")]
use rustok_api::HostRuntimeContext;

#[cfg(feature = "ssr")]
use super::context::*;
#[cfg(feature = "ssr")]
use super::mappers::*;
#[cfg(feature = "ssr")]
use super::operations::*;
use crate::model::{
    PricingAdjustmentPreview, PricingAdminBootstrap, PricingDiscountDraft, PricingPriceDraft,
    PricingPriceListOption, PricingPriceListRuleDraft, PricingPriceListScopeDraft,
    PricingProductDetail, PricingProductList,
};

#[server(prefix = "/api/fn", endpoint = "pricing/admin/bootstrap")]
pub(crate) async fn pricing_admin_bootstrap_native() -> Result<PricingAdminBootstrap, ServerFnError>
{
    #[cfg(feature = "ssr")]
    {
        use super::context::*;
        use super::mappers::*;
        use super::operations::*;
        use leptos::prelude::expect_context;
        use rustok_api::HostRuntimeContext;
        use rustok_api::Permission;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_channel::ChannelService;
        use uuid::Uuid;

        let app_ctx = expect_context::<HostRuntimeContext>();

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .map_err(|_| ServerFnError::new("Pricing request context is unavailable"))?;

        ensure_permission(
            &auth.permissions,
            &[Permission::PRODUCTS_LIST, Permission::PRODUCTS_READ],
            "products:list or products:read required",
        )?;

        let channel_service = ChannelService::new(app_ctx.db_clone());
        let (available_channels, _) = channel_service
            .list_channels(tenant.id, 1, 250)
            .await
            .map_err(ServerFnError::new)?;
        let active_price_lists = list_active_price_lists_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            &request_context,
            None,
            None,
        )
        .await?;

        Ok(PricingAdminBootstrap {
            current_tenant: map_current_tenant(&tenant),
            available_channels: available_channels
                .into_iter()
                .map(map_channel_option)
                .collect(),
            active_price_lists,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "pricing/admin/bootstrap requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "pricing/admin/active-price-lists")]
pub(crate) async fn pricing_admin_active_price_lists_native(
    channel_id: Option<String>,
    channel_slug: Option<String>,
) -> Result<Vec<PricingPriceListOption>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use super::context::*;
        use super::mappers::*;
        use super::operations::*;
        use leptos::prelude::expect_context;
        use rustok_api::HostRuntimeContext;
        use rustok_api::{AuthContext, TenantContext};
        use uuid::Uuid;

        let app_ctx = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .map_err(|_| ServerFnError::new("Pricing request context is unavailable"))?;

        list_active_price_lists_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            &request_context,
            channel_id,
            channel_slug,
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (channel_id, channel_slug);
        Err(ServerFnError::new(
            "pricing/admin/active-price-lists requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "pricing/admin/products")]
pub(crate) async fn pricing_admin_products_native(
    locale: String,
    search: Option<String>,
    status: Option<String>,
) -> Result<PricingProductList, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use super::context::*;
        use super::mappers::*;
        use super::operations::*;
        use leptos::prelude::expect_context;
        use rustok_api::HostRuntimeContext;
        use rustok_api::Permission;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_pricing::PricingService;
        use uuid::Uuid;

        let app_ctx = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .map_err(|_| ServerFnError::new("Pricing request context is unavailable"))?;

        ensure_permission(
            &auth.permissions,
            &[Permission::PRODUCTS_LIST],
            "products:list required",
        )?;

        let requested_locale =
            resolve_requested_locale(Some(locale), None, tenant.default_locale.as_str());
        let service = PricingService::new(
            app_ctx.db_clone(),
            transactional_event_bus_from_runtime(&app_ctx)?,
        )
        .with_product_catalog_read_runtime(product_read_runtime(&app_ctx)?);
        let status = status
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| match value {
                "ACTIVE" => rustok_commerce_foundation::entities::product::ProductStatus::Active,
                "ARCHIVED" => {
                    rustok_commerce_foundation::entities::product::ProductStatus::Archived
                }
                _ => rustok_commerce_foundation::entities::product::ProductStatus::Draft,
            });
        let data = service
            .list_admin_product_pricing_with_locale_fallback(
                pricing_admin_read_context(&tenant, &auth, &request_context, &requested_locale)?,
                Some(tenant.default_locale.as_str()),
                search.as_deref(),
                status,
                1,
                24,
            )
            .await
            .map_err(ServerFnError::new)?;

        Ok(map_native_list(data))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (locale, search, status);
        Err(ServerFnError::new(
            "pricing/admin/products requires the `ssr` feature",
        ))
    }
}

#[allow(clippy::too_many_arguments)]
#[server(prefix = "/api/fn", endpoint = "pricing/admin/product")]
pub(crate) async fn pricing_admin_product_native(
    product_id: String,
    locale: String,
    currency_code: Option<String>,
    region_id: Option<String>,
    price_list_id: Option<String>,
    channel_id: Option<String>,
    channel_slug: Option<String>,
    quantity: Option<i32>,
) -> Result<Option<PricingProductDetail>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use super::context::*;
        use super::mappers::*;
        use super::operations::*;
        use leptos::prelude::expect_context;
        use rustok_api::HostRuntimeContext;
        use rustok_api::Permission;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_pricing::{
            PriceResolutionContext, PricingService, ResolveProductPricesRequest,
            in_process_pricing_read_port,
        };
        use uuid::Uuid;

        let app_ctx = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request_context = Some(
            leptos_axum::extract::<rustok_api::RequestContext>()
                .await
                .map_err(|_| ServerFnError::new("Pricing request context is unavailable"))?,
        );

        ensure_permission(
            &auth.permissions,
            &[Permission::PRODUCTS_READ],
            "products:read required",
        )?;

        let requested_locale =
            resolve_requested_locale(Some(locale), None, tenant.default_locale.as_str());
        let explicit_channel_id = parse_optional_uuid_string(channel_id, "channel_id")
            .map_err(|error| ServerFnError::new(error.to_string()))?;
        let mut resolution_context = sanitize_resolution_context(
            currency_code.clone(),
            region_id.clone(),
            price_list_id.clone(),
            quantity,
        )
        .map_err(|error| ServerFnError::new(error.to_string()))?;
        let explicit_scope = explicit_channel_id.is_some() || channel_slug.is_some();
        if let Some(context) = resolution_context.as_mut() {
            context.channel_id = if explicit_scope {
                explicit_channel_id
            } else {
                request_context
                    .as_ref()
                    .and_then(|ctx| ctx.channel_id.map(|id| id.to_string()))
            };
            context.channel_slug = if explicit_scope {
                channel_slug.map(|slug| slug.trim().to_ascii_lowercase())
            } else {
                request_context
                    .as_ref()
                    .and_then(|ctx| ctx.channel_slug.clone())
            };
        }
        let native_resolution_context = resolution_context.as_ref().map(|context| {
            let region_id = context
                .region_id
                .as_deref()
                .and_then(|value| Uuid::parse_str(value).ok());
            let price_list_id = context
                .price_list_id
                .as_deref()
                .and_then(|value| Uuid::parse_str(value).ok());
            let channel_id = context
                .channel_id
                .as_deref()
                .and_then(|value| Uuid::parse_str(value).ok());
            PriceResolutionContext {
                currency_code: context.currency_code.clone(),
                region_id,
                price_list_id,
                channel_id,
                channel_slug: context.channel_slug.clone(),
                quantity: Some(context.quantity),
            }
        });
        let product_runtime = product_read_runtime(&app_ctx)?;
        let service = PricingService::new(
            app_ctx.db_clone(),
            transactional_event_bus_from_runtime(&app_ctx)?,
        )
        .with_product_catalog_read_runtime(product_runtime.clone());
        let pricing_read_port = in_process_pricing_read_port(
            app_ctx.db_clone(),
            transactional_event_bus_from_runtime(&app_ctx)?,
            product_runtime,
        );
        let product_id = parse_product_id(&product_id)?;
        let mut detail = match service
            .get_admin_product_pricing_with_locale_fallback(
                pricing_admin_read_context(
                    &tenant,
                    &auth,
                    request_context.as_ref().ok_or_else(|| {
                        ServerFnError::new("Pricing request context is unavailable")
                    })?,
                    &requested_locale,
                )?,
                product_id,
                Some(tenant.default_locale.as_str()),
                native_resolution_context
                    .as_ref()
                    .and_then(|context| context.price_list_id),
            )
            .await
        {
            Ok(detail) => Some(map_native_detail(detail)),
            Err(err)
                if err.kind == rustok_api::PortErrorKind::NotFound
                    && err.code == "pricing.product_not_found" =>
            {
                None
            }
            Err(err) => return Err(ServerFnError::new(err)),
        };

        if let (Some(detail_ref), Some(context)) =
            (detail.as_mut(), native_resolution_context.as_ref())
        {
            let trusted = request_context
                .as_ref()
                .ok_or_else(|| ServerFnError::new("Pricing request context is unavailable"))?;
            let mut port_context = rustok_api::PortContext::new(
                tenant.id.to_string(),
                rustok_api::PortActor::user(auth.user_id.to_string()),
                requested_locale.clone(),
                trusted.correlation_id.clone(),
            )
            .with_deadline(std::time::Duration::from_secs(2));
            port_context.claims = auth.scopes.clone();
            port_context
                .claims
                .extend(auth.permissions.iter().map(ToString::to_string));
            port_context.channel = context.channel_slug.clone();
            let snapshot = pricing_read_port
                .resolve_product_prices(
                    port_context,
                    ResolveProductPricesRequest {
                        product_id: Some(product_id),
                        variant_ids: detail_ref
                            .variants
                            .iter()
                            .map(|variant| Uuid::parse_str(&variant.id))
                            .collect::<Result<Vec<_>, _>>()
                            .map_err(|_| ServerFnError::new("SKU identity is invalid"))?,
                        pricing: context.clone(),
                    },
                )
                .await
                .map_err(|error| {
                    ServerFnError::new(rustok_pricing::map_pricing_port_error(&error).to_string())
                })?;
            let mut quotes = snapshot
                .items
                .into_iter()
                .map(|item| (item.variant_id.to_string(), item.price))
                .collect::<std::collections::BTreeMap<_, _>>();
            for variant in &mut detail_ref.variants {
                variant.effective_price = quotes
                    .remove(&variant.id)
                    .ok_or_else(|| ServerFnError::new("SKU quote is incomplete"))?
                    .map(map_native_effective_price);
            }
            detail_ref.resolution_context = Some(map_resolution_context(snapshot.context));
            detail_ref.resolved_at = Some(snapshot.resolved_at.to_rfc3339());
        }

        Ok(detail)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (
            product_id,
            locale,
            currency_code,
            region_id,
            price_list_id,
            channel_id,
            channel_slug,
            quantity,
        );
        Err(ServerFnError::new(
            "pricing/admin/product requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "pricing/admin/update-variant-price")]
pub(crate) async fn pricing_admin_update_variant_price_native(
    variant_id: String,
    payload: PricingPriceDraft,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use super::context::*;
        use super::mappers::*;
        use super::operations::*;
        use leptos::prelude::expect_context;
        use rustok_api::HostRuntimeContext;
        use rustok_api::{AuthContext, TenantContext};
        use uuid::Uuid;

        let app_ctx = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        update_variant_price_native_with_context(&app_ctx, &auth, &tenant, variant_id, payload)
            .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (variant_id, payload);
        Err(ServerFnError::new(
            "pricing/admin/update-variant-price requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "pricing/admin/preview-variant-discount"
)]
pub(crate) async fn pricing_admin_preview_variant_discount_native(
    variant_id: String,
    payload: PricingDiscountDraft,
) -> Result<PricingAdjustmentPreview, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use super::context::*;
        use super::mappers::*;
        use super::operations::*;
        use leptos::prelude::expect_context;
        use rustok_api::HostRuntimeContext;
        use rustok_api::{AuthContext, TenantContext};
        use uuid::Uuid;

        let app_ctx = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .map_err(|_| ServerFnError::new("Pricing request context is unavailable"))?;
        preview_variant_discount_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            &request_context,
            variant_id,
            payload,
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (variant_id, payload);
        Err(ServerFnError::new(
            "pricing/admin/preview-variant-discount requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "pricing/admin/apply-variant-discount")]
pub(crate) async fn pricing_admin_apply_variant_discount_native(
    variant_id: String,
    payload: PricingDiscountDraft,
) -> Result<PricingAdjustmentPreview, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use super::context::*;
        use super::mappers::*;
        use super::operations::*;
        use leptos::prelude::expect_context;
        use rustok_api::HostRuntimeContext;
        use rustok_api::{AuthContext, TenantContext};
        use uuid::Uuid;

        let app_ctx = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        apply_variant_discount_native_with_context(&app_ctx, &auth, &tenant, variant_id, payload)
            .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (variant_id, payload);
        Err(ServerFnError::new(
            "pricing/admin/apply-variant-discount requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "pricing/admin/update-price-list-rule")]
pub(crate) async fn pricing_admin_update_price_list_rule_native(
    price_list_id: String,
    payload: PricingPriceListRuleDraft,
) -> Result<PricingPriceListOption, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use super::context::*;
        use super::mappers::*;
        use super::operations::*;
        use leptos::prelude::expect_context;
        use rustok_api::HostRuntimeContext;
        use rustok_api::{AuthContext, TenantContext};
        use uuid::Uuid;

        let app_ctx = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        update_price_list_rule_native_with_context(&app_ctx, &auth, &tenant, price_list_id, payload)
            .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (price_list_id, payload);
        Err(ServerFnError::new(
            "pricing/admin/update-price-list-rule requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "pricing/admin/update-price-list-scope")]
pub(crate) async fn pricing_admin_update_price_list_scope_native(
    price_list_id: String,
    payload: PricingPriceListScopeDraft,
) -> Result<PricingPriceListOption, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use super::context::*;
        use super::mappers::*;
        use super::operations::*;
        use leptos::prelude::expect_context;
        use rustok_api::HostRuntimeContext;
        use rustok_api::{AuthContext, TenantContext};
        use uuid::Uuid;

        let app_ctx = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        update_price_list_scope_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            price_list_id,
            payload,
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (price_list_id, payload);
        Err(ServerFnError::new(
            "pricing/admin/update-price-list-scope requires the `ssr` feature",
        ))
    }
}
