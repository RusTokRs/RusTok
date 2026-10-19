use leptos::prelude::*;

use crate::catalog_controls::CatalogListInput;
use crate::core::FetchRequest;
#[cfg(feature = "ssr")]
use crate::model::{ProductImage, ProductListItem, ProductListPrice};
use crate::model::{ProductList, StorefrontProductsData};

use super::native_server_adapter::{self, ApiError};

#[cfg(feature = "ssr")]
const PRODUCT_STOREFRONT_CATALOG_OWNER: &str = "rustok_product.storefront";
#[cfg(feature = "ssr")]
const PRODUCT_STOREFRONT_CATALOG_OPERATION: &str = "storefront_catalog_list";
#[cfg(feature = "ssr")]
const PRODUCT_STOREFRONT_CATALOG_BOUNDARY: &str = "product_storefront_catalog_list_native";

#[cfg(feature = "ssr")]
fn map_runtime_dependency_error(dependency: &'static str) -> ServerFnError {
    tracing::error!(
        owner = PRODUCT_STOREFRONT_CATALOG_OWNER,
        owner_operation = PRODUCT_STOREFRONT_CATALOG_OPERATION,
        dependency,
        code = "product.storefront_catalog_runtime_unavailable",
        boundary = PRODUCT_STOREFRONT_CATALOG_BOUNDARY,
        "product storefront catalog runtime dependency is unavailable"
    );
    ServerFnError::new("Product catalog is temporarily unavailable")
}

#[cfg(feature = "ssr")]
fn map_tenant_context_error(request_context: &rustok_api::RequestContext) -> ServerFnError {
    tracing::error!(
        owner = PRODUCT_STOREFRONT_CATALOG_OWNER,
        owner_operation = PRODUCT_STOREFRONT_CATALOG_OPERATION,
        request_context_present = true,
        correlation_id_length = request_context.correlation_id.chars().count(),
        channel_id_present = request_context.channel_id.is_some(),
        channel_slug_length = request_context
            .channel_slug
            .as_deref()
            .map_or(0, |value| value.chars().count()),
        locale_length = request_context.locale.chars().count(),
        code = "product.storefront_catalog_tenant_context_unavailable",
        boundary = PRODUCT_STOREFRONT_CATALOG_BOUNDARY,
        "product storefront tenant context extraction failed"
    );
    ServerFnError::new("Product catalog context is unavailable")
}

pub async fn fetch_products(
    mut request: FetchRequest,
    controls: CatalogListInput,
) -> Result<StorefrontProductsData, ApiError> {
    let products = storefront_catalog_list_native(
        request.locale.clone(),
        controls.search,
        controls.category_id,
        controls.sort_by,
        controls.sort_direction,
        controls.attribute_filters,
        controls.currency_code,
    )
    .await
    .map_err(ApiError::from)?;
    let resolved_handle = request
        .selected_handle
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
    request.selected_handle = resolved_handle.clone();

    let mut data = native_server_adapter::fetch_products(request).await?;
    data.products = products;
    data.selected_handle = resolved_handle.clone();
    if resolved_handle.is_none() {
        data.selected_product = None;
        data.selected_pricing = None;
        data.resolution_context = None;
    }
    Ok(data)
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
            "product_storefront_catalog_list_native",
        )
        .to_string(),
    )
}

#[cfg(feature = "ssr")]
fn map_product_list(value: rustok_product::StorefrontProductList) -> ProductList {
    ProductList {
        items: value
            .items
            .into_iter()
            .map(|item| ProductListItem {
                id: item.id.to_string(),
                status: item.status.to_string(),
                title: item.title,
                handle: item.handle,
                seller_id: item.seller_id,
                vendor: item.vendor,
                product_type: item.product_type,
                tags: item.tags,
                primary_image: item.primary_image.map(map_owner_product_image),
                price_from: item.price_from.map(map_owner_product_list_price),
                created_at: item.created_at.to_rfc3339(),
                published_at: item.published_at.map(|value| value.to_rfc3339()),
            })
            .collect(),
        total: value.total,
        page: value.page,
        per_page: value.per_page,
        has_next: value.has_next,
    }
}

#[cfg(feature = "ssr")]
fn normalize_public_channel_slug(channel_slug: Option<&str>) -> Option<String> {
    channel_slug
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

#[server(prefix = "/api/fn", endpoint = "product/storefront/catalog-list")]
async fn storefront_catalog_list_native(
    locale: Option<String>,
    search: Option<String>,
    category_id: Option<String>,
    sort_by: Option<String>,
    sort_direction: Option<String>,
    attribute_filters: Vec<String>,
    currency_code: Option<String>,
) -> Result<ProductList, ServerFnError> {
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
            PRODUCT_STOREFRONT_CATALOG_OPERATION,
            PRODUCT_STOREFRONT_CATALOG_BOUNDARY,
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
        let public_channel_slug =
            normalize_public_channel_slug(request_context.channel_slug.as_deref());
        let list_query = StorefrontProductListQuery::try_from_transport_with_attribute_filters(
            search,
            category_id,
            sort_by,
            sort_direction,
            attribute_filters,
        )
        .map_err(|error| map_product_service_error(error, "storefront_catalog_list_input"))?
        .with_pagination(1, 12)
        .with_currency_code(currency_code)
        .map_err(|error| map_product_service_error(error, "storefront_catalog_list_input"))?;
        let products = CatalogService::new(runtime_ctx.db_clone(), event_bus)
            .list_published_products_with_query(
                tenant.id,
                requested_locale.as_str(),
                Some(tenant.default_locale.as_str()),
                public_channel_slug.as_deref(),
                list_query,
            )
            .await
            .map_err(|error| map_product_service_error(error, "storefront_catalog_list"))?;

        Ok(map_product_list(products))
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
            currency_code,
        );
        Err(ServerFnError::new(
            "product/storefront/catalog-list requires the `ssr` feature",
        ))
    }
}

/// Maps the owner-resolved catalog-card image into the storefront model.
#[cfg(feature = "ssr")]
pub(crate) fn map_owner_product_image(
    value: rustok_product::StorefrontProductListImage,
) -> ProductImage {
    ProductImage {
        media_id: value.media_id.to_string(),
        url: value.url,
        alt_text: value.alt_text,
        position: value.position,
    }
}

/// Maps the owner-resolved catalog-card price snapshot into the storefront model.
#[cfg(feature = "ssr")]
pub(crate) fn map_owner_product_list_price(
    value: rustok_product::StorefrontProductListPrice,
) -> ProductListPrice {
    ProductListPrice {
        currency_code: value.currency_code,
        amount: value.amount.normalize().to_string(),
        compare_at_amount: value
            .compare_at_amount
            .map(|compare_at| compare_at.normalize().to_string()),
        on_sale: value.on_sale,
    }
}
