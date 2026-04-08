use leptos::prelude::*;
use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::{ProductDetail, ProductList, StorefrontProductsData};
#[cfg(feature = "ssr")]
use crate::model::{ProductListItem, ProductPrice, ProductTranslation, ProductVariant};

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

const STOREFRONT_PRODUCTS_QUERY: &str = "query StorefrontCommerceProducts($locale: String, $filter: StorefrontProductsFilter) { storefrontProducts(locale: $locale, filter: $filter) { total page perPage hasNext items { id status title handle vendor productType tags createdAt publishedAt } } }";
const STOREFRONT_PRODUCT_QUERY: &str = "query StorefrontCommerceProduct($locale: String, $handle: String!) { storefrontProduct(locale: $locale, handle: $handle) { id status vendor productType tags publishedAt translations { locale title handle description } variants { id title sku inventoryQuantity inStock prices { currencyCode amount compareAtAmount onSale } } } }";

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

pub async fn fetch_storefront_products(
    selected_handle: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontProductsData, ApiError> {
    match fetch_storefront_products_server(selected_handle.clone(), locale.clone()).await {
        Ok(data) => Ok(data),
        Err(_) => fetch_storefront_products_graphql(selected_handle, locale).await,
    }
}

pub async fn fetch_storefront_products_server(
    selected_handle: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontProductsData, ApiError> {
    storefront_products_native(selected_handle, locale)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_storefront_products_graphql(
    selected_handle: Option<String>,
    locale: Option<String>,
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
            StorefrontProductVariables { locale, handle },
        )
        .await?;
        response.storefront_product
    } else {
        None
    };

    Ok(StorefrontProductsData {
        products: products_response.storefront_products,
        selected_product,
        selected_handle: resolved_handle,
    })
}

#[cfg(feature = "ssr")]
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

#[server(prefix = "/api/fn", endpoint = "product/storefront-data")]
async fn storefront_products_native(
    selected_handle: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontProductsData, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::loco::transactional_event_bus_from_context;
        use rustok_product::CatalogService;

        let app_ctx = expect_context::<AppContext>();
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .ok();
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let requested_locale = locale
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| request_context.as_ref().map(|ctx| ctx.locale.clone()))
            .unwrap_or_else(|| tenant.default_locale.clone());
        let public_channel_slug = request_context
            .as_ref()
            .and_then(|ctx| normalize_public_channel_slug(ctx.channel_slug.as_deref()));

        let service = CatalogService::new(
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

        Ok(StorefrontProductsData {
            products: map_product_list(products),
            selected_product,
            selected_handle: resolved_handle,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (selected_handle, locale);
        Err(ServerFnError::new(
            "product/storefront-data requires the `ssr` feature",
        ))
    }
}
