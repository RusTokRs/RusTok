#![allow(clippy::too_many_arguments)]

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[cfg(feature = "ssr")]
use crate::core::pricing_quote_matches_request;
#[cfg(feature = "ssr")]
use crate::core::resolve_requested_locale;

#[cfg(feature = "ssr")]
use crate::model::{
    ProductCatalogSearchOption, ProductEffectivePrice, ProductImage, ProductPrice,
    ProductPricingVariant, ProductTranslation, ProductVariant,
};
use crate::model::{ProductCatalogSearchOptions, ProductDetail, ProductPricingDetail};

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
    fn from(_value: ServerFnError) -> Self {
        // Owner handlers log a correlated public envelope; browser/network/framework diagnostics
        // are not a public Product payload and must not enter SSR evidence attributes.
        Self::ServerFn("Product storefront request could not be completed".into())
    }
}

#[cfg(feature = "ssr")]
fn map_product_internal_error() -> ServerFnError {
    let public = rustok_product::ProductPublicError::internal();
    tracing::error!(owner = "rustok_product.storefront", owner_operation = "storefront_native_read",
        boundary = "product_storefront_native_server", code = public.code,
        correlation_id = %public.correlation_id, "product storefront read could not be established");
    ServerFnError::new(public.to_string())
}

/// A rejected trusted context is not an absent optional channel. Never widen a read on rejection.
#[cfg(feature = "ssr")]
pub(super) async fn require_request_context(
    operation: &'static str,
    boundary: &'static str,
) -> Result<rustok_api::RequestContext, ServerFnError> {
    leptos_axum::extract::<rustok_api::RequestContext>()
        .await
        .map_err(|_| {
            let public = rustok_product::ProductPublicError::internal();
            tracing::error!(
                owner = "rustok_product.storefront",
                owner_operation = operation,
                boundary,
                code = "product.storefront_request_context_unavailable",
                public_code = public.code,
                correlation_id = %public.correlation_id,
                request_context_rejected = true,
                "product storefront trusted request context extraction failed"
            );
            ServerFnError::new(public.to_string())
        })
}

pub async fn fetch_product_detail(
    handle: String,
    locale: Option<String>,
) -> Result<Option<ProductDetail>, ApiError> {
    storefront_product_detail_native(handle, locale)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_buyer_quote(
    request: crate::core::buyer::BuyerQuoteRequest,
) -> Result<ProductPricingDetail, ApiError> {
    storefront_buyer_quote_native(request)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_catalog_search_options(
    locale: String,
) -> Result<ProductCatalogSearchOptions, ApiError> {
    storefront_catalog_search_options_native(locale)
        .await
        .map_err(ApiError::from)
}

#[cfg(feature = "ssr")]
fn map_product_attributes(
    attributes: Vec<rustok_product::dto::StorefrontProductAttributeResponse>,
) -> Vec<crate::model::ProductAttribute> {
    attributes
        .into_iter()
        .map(|attribute| crate::model::ProductAttribute {
            code: attribute.code,
            label: attribute.label,
            value_type: attribute.value_type,
            is_localized: attribute.is_localized,
            values: attribute
                .values
                .into_iter()
                .map(|value| crate::model::ProductAttributeValue { text: value.text })
                .collect(),
        })
        .collect()
}

#[cfg(feature = "ssr")]
pub(super) fn map_product_detail(value: rustok_product::dto::ProductResponse) -> ProductDetail {
    let attributes = map_product_attributes(value.storefront_attributes);
    ProductDetail {
        id: value.id.to_string(),
        status: value.status.to_string(),
        seller_id: value.seller_id,
        vendor: value.vendor,
        product_type: value.product_type,
        tags: value.tags,
        published_at: value.published_at.map(|item| item.to_rfc3339()),
        images: value
            .images
            .into_iter()
            .map(|image| ProductImage {
                media_id: image.media_id.to_string(),
                url: image.url,
                alt_text: image.alt_text,
                position: image.position,
            })
            .collect(),
        translations: value
            .translations
            .into_iter()
            .map(|item| ProductTranslation {
                locale: item.locale,
                title: item.title,
                handle: item.handle,
                description: item.description,
            })
            .collect(),
        variant_axes: value
            .variant_axes
            .into_iter()
            .map(|axis| crate::model::ProductVariantAxis {
                id: axis.id.to_string(),
                attribute_id: axis.attribute_id.to_string(),
                name: axis.name,
                position: axis.position,
                allowed_values: axis
                    .allowed_values
                    .into_iter()
                    .map(|option| crate::model::ProductVariantAxisOption {
                        option_id: option.option_id.to_string(),
                        value: option.value,
                        position: option.position,
                    })
                    .collect(),
            })
            .collect(),
        variants: value
            .variants
            .into_iter()
            .map(|item| ProductVariant {
                id: item.id.to_string(),
                title: item.title,
                sku: item.sku,
                inventory_quantity: item.inventory_quantity,
                inventory_policy: item.inventory_policy.to_string(),
                combination_identity: item.combination_identity,
                axis_values: item
                    .axis_values
                    .into_iter()
                    .map(|value| crate::model::ProductVariantAxisValue {
                        attribute_id: value.attribute_id.to_string(),
                        option_id: value.option_id.to_string(),
                    })
                    .collect(),
                in_stock: item.in_stock,
                prices: item
                    .prices
                    .into_iter()
                    .map(|price| ProductPrice {
                        currency_code: price.currency_code,
                        amount: price.amount.normalize().to_string(),
                        compare_at_amount: price
                            .compare_at_amount
                            .map(|value| value.normalize().to_string()),
                        on_sale: price.on_sale,
                    })
                    .collect(),
            })
            .collect(),
        attributes,
    }
}

#[cfg(feature = "ssr")]
fn map_effective_price(value: rustok_pricing::ResolvedPrice) -> ProductEffectivePrice {
    ProductEffectivePrice {
        source: crate::model::ProductPriceSource {
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
        price_list_id: value.price_list_id.map(|item| item.to_string()),
        channel_id: value.channel_id.map(|item| item.to_string()),
        channel_slug: value.channel_slug,
    }
}

#[cfg(feature = "ssr")]
fn first_non_empty(values: impl IntoIterator<Item = String>) -> String {
    values
        .into_iter()
        .find(|value| !value.trim().is_empty())
        .unwrap_or_default()
}

#[server(
    prefix = "/api/fn",
    endpoint = "product/storefront/catalog-search-options",
    client = leptos_auth::AuthorizedBrowserClient
)]
async fn storefront_catalog_search_options_native(
    locale: String,
) -> Result<ProductCatalogSearchOptions, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let scope = product_buyer_scope("storefront_catalog_search_options").await?;
        let locale = rustok_api::normalize_locale_tag(&locale)
            .ok_or_else(|| ServerFnError::new("Invalid storefront locale"))?;
        let context = product_read_context(&scope, locale, scope.request.channel_slug.clone());
        let service = product_read_runtime(&scope)
            .schema_read_port()
            .ok_or_else(map_product_internal_error)?;
        let category_options = product_read_call(&scope, service.list_categories(context.clone()))
            .await
            .map_err(map_product_port_error)?
            .into_iter()
            .map(|category| ProductCatalogSearchOption {
                value: category.id.to_string(),
                label: first_non_empty([category.path, category.name, category.code]),
            })
            .collect();
        let attribute_options = product_read_call(&scope, service.list_attributes(context))
            .await
            .map_err(map_product_port_error)?
            .into_iter()
            .filter(|attribute| attribute.is_filterable || attribute.is_sortable)
            .map(|attribute| {
                let label = first_non_empty([attribute.label, attribute.code.clone()]);
                ProductCatalogSearchOption {
                    value: attribute.code.clone(),
                    label: format!("{label} ({})", attribute.code),
                }
            })
            .collect();

        Ok(ProductCatalogSearchOptions {
            category_options,
            attribute_options,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = locale;
        Err(ServerFnError::new(
            "product/storefront/catalog-search-options requires the `ssr` feature",
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

#[cfg(feature = "ssr")]
pub(super) struct ProductBuyerScope {
    pub(super) host: rustok_api::HostRuntimeContext,
    pub(super) tenant: rustok_api::TenantContext,
    pub(super) request: rustok_api::RequestContext,
    pub(super) events: rustok_outbox::TransactionalEventBus,
    auth: rustok_api::OptionalAuthContext,
    deadline: tokio::time::Instant,
}

#[cfg(feature = "ssr")]
pub(super) async fn product_buyer_scope(
    operation: &'static str,
) -> Result<ProductBuyerScope, ServerFnError> {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);
    tokio::time::timeout_at(deadline, async {
        let host = use_context::<rustok_api::HostRuntimeContext>()
            .ok_or_else(map_product_internal_error)?;
        let request = require_request_context(operation, "product_storefront_buyer_native").await?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(|_| map_product_internal_error())?;
        rustok_commerce::admit_storefront_buyer_request(host.db(), tenant.id, &request)
            .await
            .map_err(|_| map_product_internal_error())?;
        let events = host
            .shared_get::<rustok_outbox::TransactionalEventBus>()
            .ok_or_else(map_product_internal_error)?;
        let auth = leptos_axum::extract::<rustok_api::OptionalAuthContext>()
            .await
            .map_err(|_| map_product_internal_error())?;
        Ok(ProductBuyerScope {
            host,
            tenant,
            request,
            events,
            auth,
            deadline,
        })
    })
    .await
    .map_err(|_| map_product_internal_error())?
}

#[cfg(feature = "ssr")]
pub(super) fn product_read_context(
    scope: &ProductBuyerScope,
    locale: String,
    channel: Option<String>,
) -> rustok_api::PortContext {
    let mut context = rustok_api::PortContext::new(
        scope.tenant.id.to_string(),
        scope
            .auth
            .0
            .as_ref()
            .map(|auth| auth.user_id)
            .map(|id| rustok_api::PortActor::user(id.to_string()))
            .unwrap_or_else(|| rustok_api::PortActor::service("rustok-product.storefront")),
        locale,
        scope.request.correlation_id.clone(),
    )
    .with_deadline(
        scope
            .deadline
            .saturating_duration_since(tokio::time::Instant::now()),
    );
    context.channel = channel;
    context
}

/// Admission, schema, projection and quote reads share one request budget, including remote
/// selected providers. Dropping a timed-out future does not certify database cancellation.
#[cfg(feature = "ssr")]
pub(super) async fn product_read_call<T, F>(
    scope: &ProductBuyerScope,
    call: F,
) -> Result<T, rustok_api::PortError>
where
    F: std::future::Future<Output = Result<T, rustok_api::PortError>>,
{
    tokio::time::timeout_at(scope.deadline, call)
        .await
        .map_err(|_| {
            rustok_api::PortError::timeout(
                "product.storefront_read_timeout",
                "Product storefront read exceeded its request budget",
            )
        })?
}

#[cfg(feature = "ssr")]
pub(super) fn product_read_port(
    scope: &ProductBuyerScope,
) -> std::sync::Arc<dyn rustok_product::ProductCatalogReadPort> {
    product_read_runtime(scope).read_port()
}

#[cfg(feature = "ssr")]
pub(super) fn product_read_runtime(
    scope: &ProductBuyerScope,
) -> rustok_product::ProductCatalogReadRuntime {
    scope
        .host
        .shared_get::<rustok_product::ProductCatalogReadRuntime>()
        .unwrap_or_else(|| {
            rustok_product::ProductCatalogReadRuntime::in_process(
                scope.host.db_clone(),
                scope.events.clone(),
            )
        })
}

#[cfg(feature = "ssr")]
pub(super) fn map_product_port_error(error: rustok_api::PortError) -> ServerFnError {
    tracing::warn!(boundary = "product_storefront_buyer_native", kind = ?error.kind,
        code_length = error.code.len(), retryable = error.retryable, "public product projection could not be established");
    map_product_internal_error()
}

#[server(prefix = "/api/fn", endpoint = "product/storefront-data", client = leptos_auth::AuthorizedBrowserClient)]
async fn storefront_product_detail_native(
    handle: String,
    locale: Option<String>,
) -> Result<Option<ProductDetail>, ServerFnError> {
    if handle.trim().is_empty() || handle.len() > 255 {
        return Err(ServerFnError::new("Invalid product handle"));
    }
    #[cfg(feature = "ssr")]
    {
        let scope = product_buyer_scope("storefront_product_detail").await?;
        let locale = resolve_requested_locale(
            locale,
            Some(&scope.request.locale),
            &scope.tenant.default_locale,
        );
        product_read_call(
            &scope,
            product_read_port(&scope).read_storefront_product_projection(
                product_read_context(&scope, locale.clone(), scope.request.channel_slug.clone()),
                rustok_product::StorefrontProductProjectionRequest {
                    subject: rustok_product::StorefrontProductProjectionSubject::Handle { handle },
                    locale: Some(locale),
                    fallback_locale: Some(scope.tenant.default_locale.clone()),
                    public_channel_slug: scope.request.channel_slug.clone(),
                },
            ),
        )
        .await
        .map_err(map_product_port_error)
        .map(|product| product.map(map_product_detail))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (handle, locale);
        Err(ServerFnError::new(
            "Native product details require the SSR server",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "product/storefront-buyer-quote", input = leptos::server_fn::codec::Json, client = leptos_auth::AuthorizedBrowserClient)]
async fn storefront_buyer_quote_native(
    request: crate::core::buyer::BuyerQuoteRequest,
) -> Result<ProductPricingDetail, ServerFnError> {
    crate::core::buyer::validate_quote_request(&request)
        .map_err(|_| ServerFnError::new("Invalid buyer quote request"))?;
    #[cfg(feature = "ssr")]
    {
        use rustok_pricing::{PriceResolutionContext, ResolveProductPricesRequest};
        use std::collections::{BTreeMap, BTreeSet};
        let scope = product_buyer_scope("storefront_buyer_quote").await?;
        let locale = resolve_requested_locale(
            request.locale.clone(),
            Some(&scope.request.locale),
            &scope.tenant.default_locale,
        );
        let parse = |id: &str| uuid::Uuid::parse_str(id).map_err(|_| map_product_internal_error());
        let product_id = parse(&request.product_id)?;
        let pricing = &request.pricing;
        // Explicit selectors are one complete request, never half of a channel borrowed from the host.
        let selected_channel = if pricing.channel_id.is_some() || pricing.channel_slug.is_some() {
            pricing.channel_slug.clone()
        } else {
            scope.request.channel_slug.clone()
        };
        let snapshot = product_read_call(
            &scope,
            rustok_pricing::in_process_pricing_read_port(
                scope.host.db_clone(),
                scope.events.clone(),
                product_read_runtime(&scope),
            )
            .resolve_product_prices(
                product_read_context(&scope, locale.clone(), selected_channel),
                ResolveProductPricesRequest {
                    product_id: Some(product_id),
                    variant_ids: request
                        .variant_ids
                        .iter()
                        .map(|id| parse(id))
                        .collect::<Result<Vec<_>, _>>()?,
                    pricing: PriceResolutionContext {
                        currency_code: pricing.currency_code.clone(),
                        region_id: pricing.region_id.as_deref().map(parse).transpose()?,
                        price_list_id: pricing.price_list_id.as_deref().map(parse).transpose()?,
                        channel_id: pricing.channel_id.as_deref().map(parse).transpose()?,
                        channel_slug: pricing.channel_slug.clone(),
                        quantity: Some(pricing.quantity),
                    },
                },
            ),
        )
        .await
        .map_err(|error| {
            ServerFnError::new(rustok_pricing::map_pricing_port_error(&error).to_string())
        })?;
        // No money crosses the public boundary before Product confirms publication in the owner-resolved scope.
        let channel = snapshot.context.channel_slug.clone();
        let product = product_read_call(
            &scope,
            product_read_port(&scope).read_storefront_product_projection(
                product_read_context(&scope, locale.clone(), channel.clone()),
                rustok_product::StorefrontProductProjectionRequest {
                    subject: rustok_product::StorefrontProductProjectionSubject::Handle {
                        handle: request.handle.clone(),
                    },
                    locale: Some(locale),
                    fallback_locale: Some(scope.tenant.default_locale.clone()),
                    public_channel_slug: channel,
                },
            ),
        )
        .await
        .map_err(map_product_port_error)?
        .ok_or_else(map_product_internal_error)?;
        let expected = request.variant_ids.iter().cloned().collect::<BTreeSet<_>>();
        if product.id != product_id
            || product.variants.len() != expected.len()
            || product
                .variants
                .iter()
                .any(|variant| !expected.contains(&variant.id.to_string()))
            || snapshot.items.len() != expected.len()
            || snapshot
                .items
                .iter()
                .any(|item| item.product_id != product_id)
        {
            return Err(map_product_internal_error());
        }
        let mut prices = BTreeMap::new();
        for item in snapshot.items {
            if !expected.contains(&item.variant_id.to_string())
                || prices.insert(item.variant_id, item.price).is_some()
            {
                return Err(map_product_internal_error());
            }
        }
        let mut variants = Vec::with_capacity(product.variants.len());
        for variant in product.variants {
            let effective_price = prices
                .remove(&variant.id)
                .ok_or_else(map_product_internal_error)?
                .map(map_effective_price);
            variants.push(ProductPricingVariant {
                id: variant.id.to_string(),
                title: variant.title,
                sku: variant.sku,
                effective_price,
            });
        }
        let quote = ProductPricingDetail {
            id: product_id.to_string(),
            variants,
            resolution_context: Some(map_resolution_context(snapshot.context)),
            resolved_at: Some(snapshot.resolved_at.to_rfc3339()),
        };
        if !pricing_quote_matches_request(&quote, Some(&request.pricing)) {
            return Err(map_product_internal_error());
        }
        Ok(quote)
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "Native buyer quotes require the SSR server",
        ))
    }
}
