use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rustok_api::{
    OptionalAuthContext, PortActor, PortContext, PortError, PortErrorKind, RequestContext,
    TenantContext,
};
use rustok_product::{LegacyStorefrontHttpProductsRequest, ProductStorefrontHttpReadPort};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use super::{
    super::common::{PaginatedResponse, PaginationMeta},
    StoreContextQuery, StoreListProductsParams,
};
use crate::controllers::{CommerceHttpRuntime, products::ProductListItem};
use crate::dto::{ProductResponse, RegionResponse, ShippingOptionResponse};
use crate::storefront_channel::public_channel_slug_from_request;

const STOREFRONT_PRODUCT_OWNER: &str = "rustok_product";
const STOREFRONT_PRODUCT_BOUNDARY: &str = "commerce_storefront_product_http";
const STOREFRONT_PRODUCT_LIST_OPERATION: &str = "list_legacy_storefront_http_products";

impl CommerceHttpRuntime {
    fn product_storefront_http_read_port(
        &self,
    ) -> Option<std::sync::Arc<dyn ProductStorefrontHttpReadPort>> {
        self.product_catalog_read_runtime
            .storefront_http_read_port()
    }
}

fn storefront_product_list_port_context(
    tenant_id: Uuid,
    locale: &str,
    public_channel_slug: Option<&str>,
    page: u64,
) -> PortContext {
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::service("rustok-commerce.storefront-product"),
        locale,
        format!("commerce-storefront-product:list:{page}"),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    match public_channel_slug {
        Some(channel) => context.with_channel(channel),
        None => context,
    }
}

fn storefront_product_list_unavailable() -> HttpError {
    tracing::error!(
        owner = STOREFRONT_PRODUCT_OWNER,
        owner_operation = STOREFRONT_PRODUCT_LIST_OPERATION,
        error_kind = "capability_unavailable",
        public_code = "commerce_store_product_unavailable",
        status = %StatusCode::SERVICE_UNAVAILABLE,
        boundary = STOREFRONT_PRODUCT_BOUNDARY,
        "storefront Product HTTP list capability is unavailable"
    );
    HttpError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "commerce_store_product_unavailable",
        "Product service is temporarily unavailable",
    )
}

fn map_storefront_product_list_port_error(
    error: PortError,
    context: &PortContext,
    tenant_id: Uuid,
) -> HttpError {
    let (status, code, message, error_kind) = match &error.kind {
        PortErrorKind::Validation => (
            StatusCode::BAD_REQUEST,
            "commerce_store_product_invalid",
            "Product request is invalid",
            "validation",
        ),
        PortErrorKind::NotFound => (
            StatusCode::NOT_FOUND,
            "commerce_store_not_found",
            "Commerce resource not found",
            "not_found",
        ),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_store_product_unavailable",
            "Product service is temporarily unavailable",
            "unavailable",
        ),
        PortErrorKind::Conflict | PortErrorKind::Forbidden | PortErrorKind::InvariantViolation => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_store_product_failed",
            "Product operation could not be completed safely",
            "owner_failure",
        ),
    };
    tracing::error!(
        owner = STOREFRONT_PRODUCT_OWNER,
        owner_operation = STOREFRONT_PRODUCT_LIST_OPERATION,
        correlation_id = %context.correlation_id,
        tenant_id_non_nil = !tenant_id.is_nil(),
        owner_error_kind = ?error.kind,
        owner_code_length = error.code.chars().count(),
        retryable = error.retryable,
        error_kind,
        public_code = code,
        status = %status,
        boundary = STOREFRONT_PRODUCT_BOUNDARY,
        "storefront Product owner list failed with bounded diagnostics"
    );
    HttpError::new(status, code, message)
}

/// List published storefront products through the Product-owned legacy HTTP projection.
#[utoipa::path(
    get,
    path = "/store/products",
    tag = "store",
    params(StoreListProductsParams),
    responses(
        (status = 200, description = "Published storefront products", body = PaginatedResponse<ProductListItem>),
        (status = 400, description = "Invalid request")
    )
)]
pub async fn list_products(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    request_context: RequestContext,
    Query(params): Query<StoreListProductsParams>,
) -> HttpResult<Json<PaginatedResponse<ProductListItem>>> {
    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;

    let pagination = params.pagination.unwrap_or_default();
    let page = pagination.page.max(1);
    let per_page = pagination.limit();
    let locale = params
        .locale
        .as_deref()
        .unwrap_or(request_context.locale.as_str());
    let public_channel_slug = public_channel_slug_from_request(&request_context);
    let port_context = storefront_product_list_port_context(
        tenant.id,
        locale,
        public_channel_slug.as_deref(),
        page,
    );
    let port = runtime
        .product_storefront_http_read_port()
        .ok_or_else(storefront_product_list_unavailable)?;
    let list = port
        .list_legacy_storefront_http_products(
            port_context.clone(),
            LegacyStorefrontHttpProductsRequest {
                locale: Some(locale.to_string()),
                fallback_locale: Some(tenant.default_locale.clone()),
                public_channel_slug,
                vendor: params.vendor,
                product_type: params.product_type,
                search: params.search,
                page,
                per_page,
            },
        )
        .await
        .map_err(|error| map_storefront_product_list_port_error(error, &port_context, tenant.id))?;

    let items = list
        .items
        .into_iter()
        .map(|item| ProductListItem {
            id: item.id,
            status: item.status.to_string(),
            title: item.title,
            handle: item.handle,
            seller_id: item.seller_id,
            vendor: item.vendor,
            product_type: item.product_type,
            shipping_profile_slug: Some(item.shipping_profile_slug),
            tags: item.tags,
            created_at: item.created_at.to_rfc3339(),
            published_at: item.published_at.map(|value| value.to_rfc3339()),
        })
        .collect::<Vec<_>>();

    Ok(Json(PaginatedResponse {
        data: items,
        meta: PaginationMeta::new(list.page, list.per_page, list.total),
    }))
}

#[utoipa::path(
    get,
    path = "/store/products/{id}",
    tag = "store",
    params(("id" = Uuid, Path, description = "Product ID")),
    responses(
        (status = 200, description = "Product details", body = ProductResponse),
        (status = 404, description = "Product not found")
    )
)]
pub async fn show_product(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<ProductResponse>> {
    super::products_legacy::show_product(State(runtime), tenant, request_context, Path(id)).await
}

#[utoipa::path(
    get,
    path = "/store/regions",
    tag = "store",
    responses(
        (status = 200, description = "Store regions", body = Vec<RegionResponse>)
    )
)]
pub async fn list_regions(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    request_context: RequestContext,
) -> HttpResult<Json<Vec<RegionResponse>>> {
    super::products_legacy::list_regions(State(runtime), tenant, request_context).await
}

#[utoipa::path(
    get,
    path = "/store/shipping-options",
    tag = "store",
    params(StoreContextQuery),
    responses(
        (status = 200, description = "Shipping options", body = Vec<ShippingOptionResponse>)
    )
)]
pub async fn list_shipping_options(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: OptionalAuthContext,
    request_context: RequestContext,
    Query(query): Query<StoreContextQuery>,
) -> HttpResult<Json<Vec<ShippingOptionResponse>>> {
    super::products_legacy::list_shipping_options(
        State(runtime),
        tenant,
        auth,
        request_context,
        Query(query),
    )
    .await
}
