use leptos::prelude::*;

use crate::catalog_controls::CatalogListInput;
use crate::core::FetchRequest;
use crate::model::{ProductList, ProductListItem, StorefrontProductsData};

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
fn record_optional_request_context_error<E: std::fmt::Debug>(error: E) {
    tracing::warn!(
        error = ?error,
        owner = PRODUCT_STOREFRONT_CATALOG_OWNER,
        owner_operation = PRODUCT_STOREFRONT_CATALOG_OPERATION,
        code = "product.storefront_catalog_request_context_unavailable",
        boundary = PRODUCT_STOREFRONT_CATALOG_BOUNDARY,
        "optional product storefront catalog request context extraction failed"
    );
}

#[cfg(feature = "ssr")]
fn map_tenant_context_error<E: std::fmt::Debug>(
    request_context: Option<&rustok_api::RequestContext>,
    error: E,
) -> ServerFnError {
    if let Some(request_context) = request_context {
        tracing::error!(
            error = ?error,
            owner = PRODUCT_STOREFRONT_CATALOG_OWNER,
            owner_operation = PRODUCT_STOREFRONT_CATALOG_OPERATION,
            channel_id = ?request_context.channel_id,
            channel_slug = ?request_context.channel_slug,
            locale = %request_context.locale,
            code = "product.storefront_catalog_tenant_context_unavailable",
            boundary = PRODUCT_STOREFRONT_CATALOG_BOUNDARY,
            "product storefront catalog tenant context extraction failed"
        );
    } else {
        tracing::error!(
            error = ?error,
            owner = PRODUCT_STOREFRONT_CATALOG_OWNER,
            owner_operation = PRODUCT_STOREFRONT_CATALOG_OPERATION,
            code = "product.storefront_catalog_tenant_context_unavailable",
            boundary = PRODUCT_STOREFRONT_CATALOG_BOUNDARY,
            "product storefront catalog tenant context extraction failed without request context"
        );
    }
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
) -> Result<ProductList, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::HostRuntimeContext;
        use rustok_outbox::TransactionalEventBus;
        use rustok_product::{CatalogService, StorefrontProductListQuery};

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let event_bus = runtime_ctx
            .shared_get::<TransactionalEventBus>()
            .ok_or_else(|| map_runtime_dependency_error("TransactionalEventBus"))?;
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
        let requested_locale = crate::core::resolve_requested_locale(
            locale,
            request_context.as_ref().map(|context| context.locale.as_str()),
            tenant.default_locale.as_str(),
        );
        let public_channel_slug = request_context.as_ref()
            .and_then(|context| normalize_public_channel_slug(context.channel_slug.as_deref()));
        let list_query = StorefrontProductListQuery::try_from_transport_with_attribute_filters(
            search,
            category_id,
            sort_by,
            sort_direction,
            attribute_filters,
        )
        .map_err(|error| map_product_service_error(error, "storefront_catalog_list_input"))?
        .with_pagination(1, 12);
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
        );
        Err(ServerFnError::new(
            "product/storefront/catalog-list requires the `ssr` feature",
        ))
    }
}
