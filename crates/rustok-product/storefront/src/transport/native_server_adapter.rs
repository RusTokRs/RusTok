#![allow(clippy::too_many_arguments)]

use leptos::prelude::*;
use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::core::build_pricing_context;
#[cfg(feature = "ssr")]
use crate::core::{resolve_requested_locale, sanitize_channel_slug, sanitize_uuid_string};

#[allow(unused_imports)]
use crate::model::{
    ProductDetail, ProductEffectivePrice, ProductList, ProductListItem, ProductPricingContext,
    ProductPricingDetail, ProductPricingVariant, ProductScopedPrice, StorefrontProductsData,
};
#[cfg(feature = "ssr")]
use crate::model::{ProductPrice, ProductTranslation, ProductVariant};

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

impl From<GraphqlHttpError> for ApiError {
    fn from(value: GraphqlHttpError) -> Self {
        Self::Graphql(value.to_string())
    }
}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

const STOREFRONT_PRODUCTS_QUERY: &str = "query StorefrontCommerceProducts($locale: String, $filter: StorefrontProductsFilter) { storefrontProducts(locale: $locale, filter: $filter) { total page perPage hasNext items { id status title handle sellerId vendor productType tags createdAt publishedAt } } }";
const STOREFRONT_PRODUCT_QUERY: &str = "query StorefrontCommerceProduct($locale: String, $handle: String!) { storefrontProduct(locale: $locale, handle: $handle) { id status sellerId vendor productType tags publishedAt translations { locale title handle description } variants { id title sku inventoryQuantity inStock prices { currencyCode amount compareAtAmount onSale } } } }";
const STOREFRONT_PRICING_PRODUCT_QUERY: &str = "query StorefrontProductPricing($locale: String, $handle: String!, $currencyCode: String, $regionId: UUID, $priceListId: UUID, $channelId: UUID, $channelSlug: String, $quantity: Int) { storefrontPricingProduct(locale: $locale, handle: $handle, currencyCode: $currencyCode, regionId: $regionId, priceListId: $priceListId, channelId: $channelId, channelSlug: $channelSlug, quantity: $quantity) { variants { id title sku prices { currencyCode amount compareAtAmount discountPercent onSale } effectivePrice { currencyCode amount compareAtAmount discountPercent onSale priceListId channelId channelSlug } } } }";

#[derive(Debug, Deserialize)]
struct StorefrontProductsResponse {
    #[serde(rename = "storefrontProducts")]
    storefront_products: ProductList,
}

#[derive(Debug, Deserialize)]
struct StorefrontProductResponse {
    #[serde(rename = "storefrontProduct")]
    storefront_product: Option<ProductDetail>,
}

#[derive(Debug, Deserialize)]
struct StorefrontPricingProductResponse {
    #[serde(rename = "storefrontPricingProduct")]
    storefront_pricing_product: Option<ProductPricingDetail>,
}

#[derive(Debug, Serialize)]
struct StorefrontProductsVariables {
    locale: Option<String>,
    filter: StorefrontProductsFilter,
}

#[derive(Debug, Serialize)]
struct StorefrontProductVariables {
    locale: Option<String>,
    handle: String,
}

#[derive(Debug, Serialize)]
struct StorefrontPricingProductVariables {
    locale: Option<String>,
    handle: String,
    #[serde(rename = "currencyCode")]
    currency_code: Option<String>,
    #[serde(rename = "regionId")]
    region_id: Option<String>,
    #[serde(rename = "priceListId")]
    price_list_id: Option<String>,
    #[serde(rename = "channelId")]
    channel_id: Option<String>,
    #[serde(rename = "channelSlug")]
    channel_slug: Option<String>,
    quantity: Option<i32>,
}

#[derive(Debug, Serialize)]
struct StorefrontProductsFilter {
    vendor: Option<String>,
    #[serde(rename = "productType")]
    product_type: Option<String>,
    search: Option<String>,
    page: Option<u64>,
    #[serde(rename = "perPage")]
    per_page: Option<u64>,
}

fn configured_tenant_slug() -> Option<String> {
    [
        "RUSTOK_TENANT_SLUG",
        "NEXT_PUBLIC_TENANT_SLUG",
        "NEXT_PUBLIC_DEFAULT_TENANT_SLUG",
    ]
    .into_iter()
    .find_map(|key| {
        std::env::var(key).ok().and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
    })
}

fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }

    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}/api/graphql")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}/api/graphql")
    }
}

async fn request<V, T>(query: &str, variables: V) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, Some(variables)),
        None,
        configured_tenant_slug(),
        None,
    )
    .await
    .map_err(ApiError::from)
}

pub async fn fetch_storefront_products_server(
    selected_handle: Option<String>,
    locale: Option<String>,
    currency_code: Option<String>,
    region_id: Option<String>,
    price_list_id: Option<String>,
    channel_id: Option<String>,
    channel_slug: Option<String>,
    quantity: Option<i32>,
) -> Result<StorefrontProductsData, ApiError> {
    storefront_products_native(
        selected_handle,
        locale,
        currency_code,
        region_id,
        price_list_id,
        channel_id,
        channel_slug,
        quantity,
    )
    .await
    .map_err(ApiError::from)
}

pub async fn fetch_products(
    request: crate::core::ProductStorefrontFetchRequest,
) -> Result<StorefrontProductsData, ApiError> {
    fetch_storefront_products_server(
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

pub async fn fetch_storefront_products_graphql(
    selected_handle: Option<String>,
    locale: Option<String>,
    currency_code: Option<String>,
    region_id: Option<String>,
    price_list_id: Option<String>,
    channel_id: Option<String>,
    channel_slug: Option<String>,
    quantity: Option<i32>,
) -> Result<StorefrontProductsData, ApiError> {
    let products_response: StorefrontProductsResponse = request(
        STOREFRONT_PRODUCTS_QUERY,
        StorefrontProductsVariables {
            locale: locale.clone(),
            filter: StorefrontProductsFilter {
                vendor: None,
                product_type: None,
                search: None,
                page: Some(1),
                per_page: Some(12),
            },
        },
    )
    .await?;

    let resolved_handle = selected_handle.or_else(|| {
        products_response
            .storefront_products
            .items
            .first()
            .map(|item| item.handle.clone())
            .filter(|handle| !handle.is_empty())
    });

    let selected_product = if let Some(handle) = resolved_handle.clone() {
        let response: StorefrontProductResponse = request(
            STOREFRONT_PRODUCT_QUERY,
            StorefrontProductVariables {
                locale: locale.clone(),
                handle,
            },
        )
        .await?;
        response.storefront_product
    } else {
        None
    };

    let resolution_context = build_pricing_context(
        selected_product.as_ref(),
        currency_code,
        region_id,
        price_list_id,
        channel_id,
        channel_slug,
        quantity,
    );
    let selected_pricing = if let Some(handle) = resolved_handle.clone() {
        let response: StorefrontPricingProductResponse = request(
            STOREFRONT_PRICING_PRODUCT_QUERY,
            StorefrontPricingProductVariables {
                locale: locale.clone(),
                handle,
                currency_code: resolution_context
                    .as_ref()
                    .map(|context| context.currency_code.clone()),
                region_id: resolution_context
                    .as_ref()
                    .and_then(|context| context.region_id.clone()),
                price_list_id: resolution_context
                    .as_ref()
                    .and_then(|context| context.price_list_id.clone()),
                channel_id: resolution_context
                    .as_ref()
                    .and_then(|context| context.channel_id.clone()),
                channel_slug: resolution_context
                    .as_ref()
                    .and_then(|context| context.channel_slug.clone()),
                quantity: resolution_context.as_ref().map(|context| context.quantity),
            },
        )
        .await?;
        response.storefront_pricing_product
    } else {
        None
    };

    Ok(StorefrontProductsData {
        products: products_response.storefront_products,
        selected_product,
        selected_pricing,
        selected_handle: resolved_handle,
        resolution_context,
    })
}

#[allow(dead_code)]
fn normalize_public_channel_slug(channel_slug: Option<&str>) -> Option<String> {
    channel_slug
        .map(str::trim)
        .filter(|slug| !slug.is_empty())
        .map(|slug| slug.to_ascii_lowercase())
}

#[cfg(feature = "ssr")]
fn map_product_list(value: rustok_product::StorefrontProductList) -> ProductList {
    ProductList {
        items: value.items.into_iter().map(map_product_list_item).collect(),
        total: value.total,
        page: value.page,
        per_page: value.per_page,
        has_next: value.has_next,
    }
}

#[cfg(feature = "ssr")]
fn map_product_list_item(value: rustok_product::StorefrontProductListItem) -> ProductListItem {
    ProductListItem {
        id: value.id.to_string(),
        status: value.status.to_string(),
        title: value.title,
        handle: value.handle,
        seller_id: value.seller_id,
        vendor: value.vendor,
        product_type: value.product_type,
        tags: value.tags,
        created_at: value.created_at.to_rfc3339(),
        published_at: value.published_at.map(|value| value.to_rfc3339()),
    }
}

#[cfg(feature = "ssr")]
fn map_product_detail(value: rustok_commerce_foundation::dto::ProductResponse) -> ProductDetail {
    ProductDetail {
        id: value.id.to_string(),
        status: value.status.to_string(),
        seller_id: value.seller_id,
        vendor: value.vendor,
        product_type: value.product_type,
        tags: value.tags,
        published_at: value.published_at.map(|item| item.to_rfc3339()),
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
        variants: value
            .variants
            .into_iter()
            .map(|item| ProductVariant {
                id: item.id.to_string(),
                title: item.title,
                sku: item.sku,
                inventory_quantity: item.inventory_quantity,
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
    }
}

#[cfg(feature = "ssr")]
fn map_product_pricing_detail(
    value: rustok_pricing::StorefrontPricingProductDetail,
) -> ProductPricingDetail {
    ProductPricingDetail {
        variants: value
            .variants
            .into_iter()
            .map(|variant| ProductPricingVariant {
                id: variant.id.to_string(),
                title: variant.title,
                sku: variant.sku,
                prices: variant
                    .prices
                    .into_iter()
                    .map(|price| ProductScopedPrice {
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
fn map_effective_price(value: rustok_pricing::ResolvedPrice) -> ProductEffectivePrice {
    ProductEffectivePrice {
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

#[server(prefix = "/api/fn", endpoint = "product/storefront-data")]
async fn storefront_products_native(
    selected_handle: Option<String>,
    locale: Option<String>,
    currency_code: Option<String>,
    region_id: Option<String>,
    price_list_id: Option<String>,
    channel_id: Option<String>,
    channel_slug: Option<String>,
    quantity: Option<i32>,
) -> Result<StorefrontProductsData, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_outbox::loco::transactional_event_bus_from_context;
        use rustok_pricing::{PriceResolutionContext, PricingService};
        use rustok_product::CatalogService;
        use uuid::Uuid;

        let app_ctx = expect_context::<AppContext>();
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .ok();
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let requested_locale = resolve_requested_locale(
            locale,
            request_context.as_ref().map(|ctx| ctx.locale.as_str()),
            tenant.default_locale.as_str(),
        );
        let public_channel_slug = request_context
            .as_ref()
            .and_then(|ctx| normalize_public_channel_slug(ctx.channel_slug.as_deref()));
        let selected_channel_id = sanitize_uuid_string(channel_id)
            .as_deref()
            .and_then(|value| Uuid::parse_str(value).ok())
            .or_else(|| request_context.as_ref().and_then(|ctx| ctx.channel_id));
        let selected_channel_slug =
            sanitize_channel_slug(channel_slug).or_else(|| public_channel_slug.clone());

        let service = CatalogService::new(
            app_ctx.db.clone(),
            transactional_event_bus_from_context(&app_ctx),
        );
        let pricing_service = PricingService::new(
            app_ctx.db.clone(),
            transactional_event_bus_from_context(&app_ctx),
        );
        let products = service
            .list_published_products_with_locale_fallback(
                tenant.id,
                requested_locale.as_str(),
                Some(tenant.default_locale.as_str()),
                public_channel_slug.as_deref(),
                1,
                12,
            )
            .await
            .map_err(ServerFnError::new)?;
        let resolved_handle = selected_handle
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| {
                products
                    .items
                    .first()
                    .map(|item| item.handle.clone())
                    .filter(|value| !value.is_empty())
            });
        let selected_product = if let Some(handle) = resolved_handle.clone() {
            service
                .get_published_product_by_handle_with_locale_fallback(
                    tenant.id,
                    handle.as_str(),
                    requested_locale.as_str(),
                    Some(tenant.default_locale.as_str()),
                    public_channel_slug.as_deref(),
                )
                .await
                .map_err(ServerFnError::new)?
                .map(map_product_detail)
        } else {
            None
        };
        let resolution_context = build_pricing_context(
            selected_product.as_ref(),
            currency_code,
            region_id,
            price_list_id,
            selected_channel_id.map(|item| item.to_string()),
            selected_channel_slug.clone(),
            quantity,
        );
        let native_resolution_context =
            resolution_context
                .as_ref()
                .map(|context| PriceResolutionContext {
                    currency_code: context.currency_code.clone(),
                    region_id: context
                        .region_id
                        .as_deref()
                        .and_then(|value| Uuid::parse_str(value).ok()),
                    price_list_id: context
                        .price_list_id
                        .as_deref()
                        .and_then(|value| Uuid::parse_str(value).ok()),
                    channel_id: context
                        .channel_id
                        .as_deref()
                        .and_then(|value| Uuid::parse_str(value).ok()),
                    channel_slug: context.channel_slug.clone(),
                    quantity: Some(context.quantity),
                });
        let selected_pricing = if let Some(handle) = resolved_handle.clone() {
            let mut detail = pricing_service
                .get_published_product_pricing_by_handle_with_locale_fallback(
                    tenant.id,
                    handle.as_str(),
                    requested_locale.as_str(),
                    Some(tenant.default_locale.as_str()),
                    selected_channel_slug.as_deref(),
                )
                .await
                .map_err(ServerFnError::new)?
                .map(map_product_pricing_detail);

            if let (Some(detail_ref), Some(context)) =
                (detail.as_mut(), native_resolution_context.as_ref())
            {
                for variant in &mut detail_ref.variants {
                    let variant_id = Uuid::parse_str(&variant.id).map_err(ServerFnError::new)?;
                    let effective_price = pricing_service
                        .resolve_variant_price(tenant.id, variant_id, context.clone())
                        .await
                        .map_err(ServerFnError::new)?;
                    variant.effective_price = effective_price.map(map_effective_price);
                }
            }

            detail
        } else {
            None
        };

        Ok(StorefrontProductsData {
            products: map_product_list(products),
            selected_product,
            selected_pricing,
            selected_handle: resolved_handle,
            resolution_context,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (
            selected_handle,
            locale,
            currency_code,
            region_id,
            price_list_id,
            channel_id,
            channel_slug,
            quantity,
        );
        Err(ServerFnError::new(
            "product/storefront-data requires the `ssr` feature",
        ))
    }
}
