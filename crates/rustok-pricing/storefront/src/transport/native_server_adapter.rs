use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
#[cfg(feature = "ssr")]
use uuid::Uuid;

use crate::core::StorefrontPricingQuery;
#[cfg(feature = "ssr")]
use crate::core::{normalize_public_channel_slug, resolve_requested_locale};
#[cfg(feature = "ssr")]
use crate::core::{parse_optional_uuid_string, sanitize_channel_slug, sanitize_resolution_context};
use crate::model::StorefrontPricingData;
#[cfg(feature = "ssr")]
use crate::model::{
    PricingChannelOption, PricingPriceListOption, PricingProductDetail, PricingProductList,
    PricingProductListItem, PricingProductTranslation, PricingVariant,
};
#[cfg(feature = "ssr")]
use crate::model::{PricingEffectivePrice, PricingPrice};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    Graphql(String),
    ServerFn(String),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Graphql(error) => write!(f, "{error}"),
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

pub(crate) async fn fetch_storefront_pricing_server(
    query: StorefrontPricingQuery,
) -> Result<StorefrontPricingData, ApiError> {
    storefront_pricing_native(query)
        .await
        .map_err(ApiError::from)
}

pub(crate) async fn fetch_storefront_pricing(
    query: StorefrontPricingQuery,
) -> Result<StorefrontPricingData, ApiError> {
    fetch_storefront_pricing_server(query).await
}

#[cfg(feature = "ssr")]
fn map_native_list(value: rustok_pricing::StorefrontPricingProductList) -> PricingProductList {
    PricingProductList {
        items: value.items.into_iter().map(map_native_list_item).collect(),
        total: value.total,
        page: value.page,
        per_page: value.per_page,
        has_next: value.has_next,
    }
}

#[cfg(feature = "ssr")]
fn map_native_list_item(
    value: rustok_pricing::StorefrontPricingProductListItem,
) -> PricingProductListItem {
    PricingProductListItem {
        id: value.id.to_string(),
        title: value.title,
        handle: value.handle,
        seller_id: value.seller_id,
        vendor: value.vendor,
        product_type: value.product_type,
        created_at: value.created_at.to_rfc3339(),
        published_at: value.published_at.map(|value| value.to_rfc3339()),
        variant_count: value.variant_count,
        sale_variant_count: value.sale_variant_count,
        currencies: value.currencies,
    }
}

#[cfg(feature = "ssr")]
fn map_native_detail(
    value: rustok_pricing::StorefrontPricingProductDetail,
) -> PricingProductDetail {
    PricingProductDetail {
        id: value.id.to_string(),
        status: value.status.to_string(),
        seller_id: value.seller_id,
        vendor: value.vendor,
        product_type: value.product_type,
        published_at: value.published_at.map(|value| value.to_rfc3339()),
        translations: value
            .translations
            .into_iter()
            .map(|translation| PricingProductTranslation {
                locale: translation.locale,
                title: translation.title,
                handle: translation.handle,
                description: translation.description,
            })
            .collect(),
        variants: value
            .variants
            .into_iter()
            .map(|variant| PricingVariant {
                id: variant.id.to_string(),
                title: variant.title,
                sku: variant.sku,
                prices: variant
                    .prices
                    .into_iter()
                    .map(|price| PricingPrice {
                        currency_code: price.currency_code,
                        amount: price.amount.normalize().to_string(),
                        compare_at_amount: price
                            .compare_at_amount
                            .map(|value| value.normalize().to_string()),
                        discount_percent: price
                            .discount_percent
                            .map(|value| value.normalize().to_string()),
                        on_sale: price.on_sale,
                    })
                    .collect(),
                effective_price: None,
            })
            .collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_native_effective_price(value: rustok_pricing::ResolvedPrice) -> PricingEffectivePrice {
    PricingEffectivePrice {
        currency_code: value.currency_code,
        amount: value.amount.normalize().to_string(),
        compare_at_amount: value
            .compare_at_amount
            .map(|item| item.normalize().to_string()),
        discount_percent: value
            .discount_percent
            .map(|item| item.normalize().to_string()),
        on_sale: value.on_sale,
        region_id: value.region_id.map(|item| item.to_string()),
        price_list_id: value.price_list_id.map(|item| item.to_string()),
        channel_id: value.channel_id.map(|item| item.to_string()),
        channel_slug: value.channel_slug,
        min_quantity: value.min_quantity,
        max_quantity: value.max_quantity,
    }
}

#[cfg(feature = "ssr")]
fn map_native_price_list_option(
    value: rustok_pricing::ActivePriceListOption,
) -> PricingPriceListOption {
    PricingPriceListOption {
        id: value.id.to_string(),
        name: value.name,
        list_type: value.list_type,
        channel_id: value.channel_id.map(|item| item.to_string()),
        channel_slug: value.channel_slug,
        rule_kind: value.rule_kind,
        adjustment_percent: value
            .adjustment_percent
            .map(|item| item.normalize().to_string()),
    }
}

#[cfg(feature = "ssr")]
fn map_channel_option(value: rustok_channel::ChannelResponse) -> PricingChannelOption {
    PricingChannelOption {
        id: value.id.to_string(),
        slug: value.slug,
        name: value.name,
        is_active: value.is_active,
        is_default: value.is_default,
        status: value.status,
    }
}

#[server(prefix = "/api/fn", endpoint = "pricing/storefront-data")]
async fn storefront_pricing_native(
    query: StorefrontPricingQuery,
) -> Result<StorefrontPricingData, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::HostRuntimeContext;
        use rustok_channel::ChannelService;
        use rustok_outbox::TransactionalEventBus;
        use rustok_pricing::{PriceResolutionContext, PricingService};

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let event_bus = runtime_ctx
            .shared_get::<TransactionalEventBus>()
            .ok_or_else(|| {
                ServerFnError::new(
                    "pricing/storefront-data requires TransactionalEventBus in host runtime context",
                )
            })?;
        let db = runtime_ctx.db_clone();
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .ok();
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let requested_locale = resolve_requested_locale(
            query.locale,
            request_context.as_ref().map(|ctx| ctx.locale.as_str()),
            tenant.default_locale.as_str(),
        );
        let explicit_channel_id = parse_optional_uuid_string(query.channel_id, "channel_id")
            .map_err(|err| ServerFnError::new(err.to_string()))?;
        let selected_channel_id = explicit_channel_id
            .as_deref()
            .and_then(|value| Uuid::parse_str(value).ok())
            .or_else(|| request_context.as_ref().and_then(|ctx| ctx.channel_id));
        let selected_channel_slug = sanitize_channel_slug(query.channel_slug).or_else(|| {
            request_context
                .as_ref()
                .and_then(|ctx| normalize_public_channel_slug(ctx.channel_slug.as_deref()))
        });
        let mut resolution_context = sanitize_resolution_context(
            query.currency_code.clone(),
            query.region_id.clone(),
            query.price_list_id.clone(),
            selected_channel_id.map(|value| value.to_string()),
            selected_channel_slug.clone(),
            query.quantity,
        )
        .map_err(|err| ServerFnError::new(err.to_string()))?;
        if let Some(context) = resolution_context.as_mut() {
            context.channel_id = selected_channel_id.map(|item| item.to_string());
            context.channel_slug = selected_channel_slug.clone();
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

        let service = PricingService::new(db.clone(), event_bus);
        let channel_service = ChannelService::new(db);
        let (available_channels, _) = channel_service
            .list_channels(tenant.id, 1, 250)
            .await
            .map_err(ServerFnError::new)?;
        let active_price_lists = service
            .list_active_price_lists_for_channel(
                tenant.id,
                selected_channel_id,
                selected_channel_slug.as_deref(),
                Some(requested_locale.as_str()),
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(ServerFnError::new)?
            .into_iter()
            .map(map_native_price_list_option)
            .collect();
        let products = service
            .list_published_product_pricing_with_locale_fallback(
                tenant.id,
                requested_locale.as_str(),
                Some(tenant.default_locale.as_str()),
                selected_channel_slug.as_deref(),
                1,
                8,
            )
            .await
            .map_err(ServerFnError::new)?;
        let resolved_handle = query
            .selected_handle
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| products.items.first().map(|item| item.handle.clone()));
        let selected_product = if let Some(handle) = resolved_handle.clone() {
            let mut detail = service
                .get_published_product_pricing_by_handle_with_locale_fallback(
                    tenant.id,
                    handle.as_str(),
                    requested_locale.as_str(),
                    Some(tenant.default_locale.as_str()),
                    selected_channel_slug.as_deref(),
                )
                .await
                .map_err(ServerFnError::new)?
                .map(map_native_detail);

            if let (Some(detail_ref), Some(context)) =
                (detail.as_mut(), native_resolution_context.as_ref())
            {
                for variant in &mut detail_ref.variants {
                    let variant_id = Uuid::parse_str(&variant.id).map_err(ServerFnError::new)?;
                    let effective_price = service
                        .resolve_variant_price(tenant.id, variant_id, context.clone())
                        .await
                        .map_err(ServerFnError::new)?;
                    variant.effective_price = effective_price.map(map_native_effective_price);
                }
            }

            detail
        } else {
            None
        };

        Ok(StorefrontPricingData {
            products: map_native_list(products),
            selected_product,
            selected_handle: resolved_handle,
            resolution_context,
            available_channels: available_channels
                .into_iter()
                .map(map_channel_option)
                .collect(),
            active_price_lists,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = query;
        Err(ServerFnError::new(
            "pricing/storefront-data requires the `ssr` feature",
        ))
    }
}
