use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
#[cfg(feature = "ssr")]
use uuid::Uuid;

use crate::core::StorefrontPricingQuery;
#[cfg(feature = "ssr")]
use crate::core::{normalize_pricing_context, parse_optional_uuid_string, sanitize_channel_slug};
#[cfg(feature = "ssr")]
use crate::core::{
    normalize_public_channel_slug, pricing_quote_matches_request, resolve_requested_locale,
};
use crate::model::StorefrontPricingData;
#[cfg(feature = "ssr")]
use crate::model::{
    PricingChannelOption, PricingPriceListOption, PricingProductDetail, PricingProductList,
    PricingProductListItem, PricingProductTranslation, PricingVariant,
};
#[cfg(feature = "ssr")]
use crate::model::{PricingEffectivePrice, PricingPrice};

#[cfg(feature = "ssr")]
const PRICING_STOREFRONT_NATIVE_OWNER: &str = "rustok_pricing.storefront";
#[cfg(feature = "ssr")]
const PRICING_STOREFRONT_NATIVE_BOUNDARY: &str = "pricing_storefront_native_transport";

#[cfg(feature = "ssr")]
fn map_runtime_dependency_error(dependency: &'static str) -> ServerFnError {
    tracing::error!(
        owner = PRICING_STOREFRONT_NATIVE_OWNER,
        owner_operation = "storefront_pricing_native",
        dependency,
        code = "pricing.storefront_runtime_unavailable",
        boundary = PRICING_STOREFRONT_NATIVE_BOUNDARY,
        "pricing storefront native runtime dependency is unavailable"
    );
    ServerFnError::new("Storefront pricing is temporarily unavailable")
}

#[cfg(feature = "ssr")]
fn map_request_context_error<E: std::fmt::Display>(_error: E) -> ServerFnError {
    tracing::error!(
        owner = PRICING_STOREFRONT_NATIVE_OWNER,
        owner_operation = "storefront_pricing_native",
        code = "pricing.storefront_request_context_unavailable",
        boundary = PRICING_STOREFRONT_NATIVE_BOUNDARY,
        request_context_rejected = true,
        "pricing storefront trusted request context is unavailable"
    );
    ServerFnError::new("Storefront pricing context is unavailable")
}

#[cfg(feature = "ssr")]
fn map_tenant_context_error<E: std::fmt::Display>(
    request_context: Option<&rustok_api::RequestContext>,
    error: E,
) -> ServerFnError {
    if let Some(request_context) = request_context {
        tracing::error!(
            error = %error,
            owner = PRICING_STOREFRONT_NATIVE_OWNER,
            owner_operation = "storefront_pricing_native",
            tenant_id = %request_context.tenant_id,
            channel_id = ?request_context.channel_id,
            channel_slug = ?request_context.channel_slug,
            locale = %request_context.locale,
            code = "pricing.storefront_tenant_context_unavailable",
            boundary = PRICING_STOREFRONT_NATIVE_BOUNDARY,
            "pricing storefront tenant context extraction failed"
        );
    } else {
        tracing::error!(
            error = %error,
            owner = PRICING_STOREFRONT_NATIVE_OWNER,
            owner_operation = "storefront_pricing_native",
            code = "pricing.storefront_tenant_context_unavailable",
            boundary = PRICING_STOREFRONT_NATIVE_BOUNDARY,
            "pricing storefront tenant context extraction failed without request context"
        );
    }
    ServerFnError::new("Pricing storefront context is unavailable")
}

#[cfg(feature = "ssr")]
fn map_owner_runtime_error<E: std::fmt::Display>(
    operation: &'static str,
    tenant_id: Uuid,
    request_context: Option<&rustok_api::RequestContext>,
    error: E,
) -> ServerFnError {
    if let Some(request_context) = request_context {
        tracing::error!(
            error = %error,
            owner = PRICING_STOREFRONT_NATIVE_OWNER,
            owner_operation = operation,
            tenant_id = %tenant_id,
            channel_id = ?request_context.channel_id,
            channel_slug = ?request_context.channel_slug,
            locale = %request_context.locale,
            code = "pricing.storefront_owner_runtime_failed",
            boundary = PRICING_STOREFRONT_NATIVE_BOUNDARY,
            "pricing storefront owner operation failed"
        );
    } else {
        tracing::error!(
            error = %error,
            owner = PRICING_STOREFRONT_NATIVE_OWNER,
            owner_operation = operation,
            tenant_id = %tenant_id,
            code = "pricing.storefront_owner_runtime_failed",
            boundary = PRICING_STOREFRONT_NATIVE_BOUNDARY,
            "pricing storefront owner operation failed without request context"
        );
    }
    ServerFnError::new("Storefront pricing is temporarily unavailable")
}

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
        resolution_context: None,
        resolved_at: None,
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
        source: crate::model::PricingPriceSource {
            price_id: value.source.price_id().to_string(),
            kind: value.source.kind().to_string(),
            rule_kind: value
                .source
                .rule()
                .map(|rule| rule.kind.as_str().to_string()),
            adjustment_percent: value
                .source
                .rule()
                .map(|rule| rule.adjustment_percent.normalize().to_string()),
        },
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
        use rustok_pricing::{
            PriceResolutionContext, ResolveProductPricesRequest,
            StorefrontProductPricingListRequest, StorefrontProductPricingProjectionRequest,
            in_process_pricing_read_port,
        };

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let event_bus = runtime_ctx
            .shared_get::<TransactionalEventBus>()
            .ok_or_else(|| map_runtime_dependency_error("TransactionalEventBus"))?;
        let db = runtime_ctx.db_clone();
        let request_context = Some(
            leptos_axum::extract::<rustok_api::RequestContext>()
                .await
                .map_err(map_request_context_error)?,
        );
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(|error| map_tenant_context_error(request_context.as_ref(), error))?;
        let requested_locale = resolve_requested_locale(
            query.locale,
            request_context.as_ref().map(|ctx| ctx.locale.as_str()),
            tenant.default_locale.as_str(),
        );
        let explicit_scope = query.channel_id.is_some() || query.channel_slug.is_some();
        let explicit_channel_id = parse_optional_uuid_string(query.channel_id, "channel_id")
            .map_err(|err| ServerFnError::new(err.to_string()))?;
        let selected_channel_id = if explicit_scope {
            explicit_channel_id
                .as_deref()
                .map(Uuid::parse_str)
                .transpose()
                .map_err(|_| map_runtime_dependency_error("Channel identity"))?
        } else {
            request_context.as_ref().and_then(|ctx| ctx.channel_id)
        };
        let selected_channel_slug = if explicit_scope {
            sanitize_channel_slug(query.channel_slug)
        } else {
            request_context
                .as_ref()
                .and_then(|ctx| normalize_public_channel_slug(ctx.channel_slug.as_deref()))
        };
        let mut resolution_context = normalize_pricing_context(
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
                .and_then(|value| Uuid::parse_str(value).ok())
                .filter(|uuid| !uuid.is_nil());
            let price_list_id = context
                .price_list_id
                .as_deref()
                .and_then(|value| Uuid::parse_str(value).ok())
                .filter(|uuid| !uuid.is_nil());
            let channel_id = context
                .channel_id
                .as_deref()
                .and_then(|value| Uuid::parse_str(value).ok())
                .filter(|uuid| !uuid.is_nil());
            PriceResolutionContext {
                currency_code: context.currency_code.clone(),
                region_id,
                price_list_id,
                channel_id,
                channel_slug: context.channel_slug.clone(),
                quantity: Some(context.quantity),
            }
        });

        let pricing_read_port = in_process_pricing_read_port(
            db.clone(),
            event_bus.clone(),
            runtime_ctx
                .shared_get::<rustok_product::ProductCatalogReadRuntime>()
                .unwrap_or_else(|| {
                    rustok_product::ProductCatalogReadRuntime::in_process(
                        db.clone(),
                        event_bus.clone(),
                    )
                }),
        );
        let channel_service = ChannelService::new(db);
        let (available_channels, _) = channel_service
            .list_channels(tenant.id, 1, 250)
            .await
            .map_err(|error| {
                map_owner_runtime_error("list_channels", tenant.id, request_context.as_ref(), error)
            })?;
        let trusted = request_context
            .as_ref()
            .ok_or_else(|| map_runtime_dependency_error("RequestContext"))?;
        let mut active_context = rustok_api::PortContext::new(
            tenant.id.to_string(),
            trusted
                .user_id
                .map(|id| rustok_api::PortActor::user(id.to_string()))
                .unwrap_or_else(|| rustok_api::PortActor::service("rustok-pricing.storefront")),
            requested_locale.clone(),
            trusted.correlation_id.clone(),
        )
        .with_deadline(std::time::Duration::from_secs(2));
        active_context.channel = selected_channel_slug.clone();
        let active_price_lists = pricing_read_port
            .list_active_price_list_projections(
                active_context,
                rustok_pricing::ActivePriceListProjectionRequest {
                    channel_id: selected_channel_id,
                    channel_slug: selected_channel_slug.clone(),
                    fallback_locale: Some(tenant.default_locale.clone()),
                },
            )
            .await
            .map_err(|error| {
                ServerFnError::new(rustok_pricing::map_pricing_port_error(&error).to_string())
            })?
            .into_iter()
            .map(|item| PricingPriceListOption {
                id: item.price_list_id.to_string(),
                name: item.title,
                list_type: item.list_type,
                channel_id: item.channel_id.map(|id| id.to_string()),
                channel_slug: item.channel_slug,
                rule_kind: item.rule_kind,
                adjustment_percent: item
                    .adjustment_percent
                    .map(|value| value.normalize().to_string()),
            })
            .collect();
        let trusted = request_context
            .as_ref()
            .ok_or_else(|| map_runtime_dependency_error("RequestContext"))?;
        let mut list_context = rustok_api::PortContext::new(
            tenant.id.to_string(),
            trusted
                .user_id
                .map(|id| rustok_api::PortActor::user(id.to_string()))
                .unwrap_or_else(|| rustok_api::PortActor::service("rustok-pricing.storefront")),
            requested_locale.clone(),
            trusted.correlation_id.clone(),
        )
        .with_deadline(std::time::Duration::from_secs(2));
        list_context.channel = selected_channel_slug.clone();
        let products = pricing_read_port
            .list_storefront_product_pricing(
                list_context,
                StorefrontProductPricingListRequest {
                    fallback_locale: Some(tenant.default_locale.clone()),
                    channel_id: selected_channel_id,
                    channel_slug: selected_channel_slug.clone(),
                    page: 1,
                    per_page: 8,
                },
            )
            .await
            .map_err(|error| {
                ServerFnError::new(rustok_pricing::map_pricing_port_error(&error).to_string())
            })?;
        let resolved_handle = query
            .selected_handle
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| products.items.first().map(|item| item.handle.clone()));
        let selected_product = if let Some(handle) = resolved_handle.clone() {
            let mut detail_context = rustok_api::PortContext::new(
                tenant.id.to_string(),
                trusted
                    .user_id
                    .map(|id| rustok_api::PortActor::user(id.to_string()))
                    .unwrap_or_else(|| rustok_api::PortActor::service("rustok-pricing.storefront")),
                requested_locale.clone(),
                trusted.correlation_id.clone(),
            )
            .with_deadline(std::time::Duration::from_secs(2));
            detail_context.channel = selected_channel_slug.clone();
            let mut detail = pricing_read_port
                .read_storefront_product_pricing_projection(
                    detail_context,
                    StorefrontProductPricingProjectionRequest {
                        handle,
                        fallback_locale: Some(tenant.default_locale.clone()),
                        public_channel_id: selected_channel_id,
                        public_channel_slug: selected_channel_slug.clone(),
                    },
                )
                .await
                .map_err(|error| {
                    ServerFnError::new(rustok_pricing::map_pricing_port_error(&error).to_string())
                })?
                .map(map_native_detail);

            if let (Some(detail_ref), Some(context)) =
                (detail.as_mut(), native_resolution_context.as_ref())
            {
                let trusted = request_context
                    .as_ref()
                    .ok_or_else(|| map_runtime_dependency_error("RequestContext"))?;
                let mut port_context = rustok_api::PortContext::new(
                    tenant.id.to_string(),
                    trusted
                        .user_id
                        .map(|id| rustok_api::PortActor::user(id.to_string()))
                        .unwrap_or_else(|| {
                            rustok_api::PortActor::service("rustok-pricing.storefront")
                        }),
                    requested_locale.clone(),
                    trusted.correlation_id.clone(),
                )
                .with_deadline(std::time::Duration::from_secs(2));
                port_context.channel = context.channel_slug.clone();
                let snapshot =
                    pricing_read_port
                        .resolve_product_prices(
                            port_context,
                            ResolveProductPricesRequest {
                                product_id: Some(Uuid::parse_str(&detail_ref.id).map_err(
                                    |_| map_runtime_dependency_error("Product identity"),
                                )?),
                                variant_ids: detail_ref
                                    .variants
                                    .iter()
                                    .map(|variant| Uuid::parse_str(&variant.id))
                                    .collect::<Result<Vec<_>, _>>()
                                    .map_err(|_| map_runtime_dependency_error("SKU identity"))?,
                                pricing: context.clone(),
                            },
                        )
                        .await
                        .map_err(|error| {
                            ServerFnError::new(
                                rustok_pricing::map_pricing_port_error(&error).to_string(),
                            )
                        })?;
                let mut quotes = snapshot
                    .items
                    .into_iter()
                    .map(|item| (item.variant_id.to_string(), item.price))
                    .collect::<std::collections::BTreeMap<_, _>>();
                for variant in &mut detail_ref.variants {
                    variant.effective_price = quotes
                        .remove(&variant.id)
                        .ok_or_else(|| map_runtime_dependency_error("SKU quote"))?
                        .map(map_native_effective_price);
                }
                detail_ref.resolution_context = Some(map_resolution_context(snapshot.context));
                detail_ref.resolved_at = Some(snapshot.resolved_at.to_rfc3339());
            }

            detail
        } else {
            None
        };

        if selected_product.as_ref().is_some_and(|product| {
            !pricing_quote_matches_request(product, resolution_context.as_ref())
        }) {
            return Err(map_runtime_dependency_error("Pricing quote observation"));
        }
        let resolution_context = selected_product
            .as_ref()
            .and_then(|product| product.resolution_context.clone());
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

#[cfg(feature = "ssr")]
fn map_resolution_context(
    value: rustok_pricing::PriceResolutionContext,
) -> crate::model::PricingResolutionContext {
    crate::model::PricingResolutionContext {
        currency_code: value.currency_code,
        region_id: value.region_id.map(|id| id.to_string()),
        price_list_id: value.price_list_id.map(|id| id.to_string()),
        channel_id: value.channel_id.map(|id| id.to_string()),
        channel_slug: value.channel_slug,
        quantity: value.quantity.unwrap_or(1),
    }
}
