use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use rustok_api::{OptionalAuthContext, PortError, RequestContext, TenantContext};
use rustok_web::{HttpError, HttpResult, port_error_to_http_error};
use uuid::Uuid;

use super::{
    super::CommerceHttpRuntime, StoreAddCartLineItemInput, StoreCartContextPatch,
    StoreCartResponse, StoreCreateCartInput, StoreUpdateCartInput, StoreUpdateCartLineItemInput,
};
use crate::dto::CartResponse;
use rustok_cart::{
    CartStorefrontCreateRequest, CartStorefrontReadRequest, CartStorefrontRemoveLineItemRequest,
    in_process_cart_storefront_port,
};
use rustok_pricing::in_process_pricing_read_port;

mod shipping_owner_reads;

fn log_storefront_cart_route_entry(
    operation: &'static str,
    tenant_id: Uuid,
    cart_id: Option<Uuid>,
    line_item_id: Option<Uuid>,
) {
    tracing::debug!(
        boundary = "commerce_storefront_cart_http",
        operation,
        tenant_id_non_nil = !tenant_id.is_nil(),
        cart_id_present = cart_id.is_some(),
        cart_id_non_nil = cart_id.is_some_and(|value| !value.is_nil()),
        line_item_id_present = line_item_id.is_some(),
        line_item_id_non_nil = line_item_id.is_some_and(|value| !value.is_nil()),
        "storefront cart route entered with bounded diagnostics"
    );
}

fn map_cart_port_error(
    error: PortError,
    operation: &'static str,
    tenant_id: Uuid,
    cart_id: Option<Uuid>,
) -> HttpError {
    let owner_error_kind = match &error.kind {
        rustok_api::PortErrorKind::Validation => "validation",
        rustok_api::PortErrorKind::NotFound => "not_found",
        rustok_api::PortErrorKind::Conflict => "conflict",
        rustok_api::PortErrorKind::Forbidden => "forbidden",
        rustok_api::PortErrorKind::Unavailable => "unavailable",
        rustok_api::PortErrorKind::Timeout => "timeout",
        rustok_api::PortErrorKind::InvariantViolation => "invariant_violation",
    };
    let owner_code_length = error.code.chars().count();
    let retryable = error.retryable;
    let public = port_error_to_http_error(error);
    let critical = matches!(
        public.status,
        StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
            | StatusCode::INTERNAL_SERVER_ERROR
    );
    if critical {
        tracing::error!(
            owner = "rustok_cart",
            operation,
            tenant_id_non_nil = !tenant_id.is_nil(),
            cart_id_present = cart_id.is_some(),
            owner_error_kind,
            owner_code_length,
            retryable,
            public_status = %public.status,
            boundary = "commerce_storefront_cart_http",
            "storefront cart port operation failed with bounded diagnostics"
        );
    } else {
        tracing::warn!(
            owner = "rustok_cart",
            operation,
            tenant_id_non_nil = !tenant_id.is_nil(),
            cart_id_present = cart_id.is_some(),
            owner_error_kind,
            owner_code_length,
            retryable,
            public_status = %public.status,
            boundary = "commerce_storefront_cart_http",
            "storefront cart port operation was rejected with bounded diagnostics"
        );
    }
    public
}

/// Create a storefront cart
#[utoipa::path(
    post,
    path = "/store/carts",
    tag = "store",
    request_body = StoreCreateCartInput,
    responses(
        (status = 201, description = "Cart created", body = StoreCartResponse),
        (status = 400, description = "Invalid request", body = rustok_web::ErrorBody)
    )
)]
pub async fn create_cart(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: OptionalAuthContext,
    request_context: RequestContext,
    Json(input): Json<StoreCreateCartInput>,
) -> HttpResult<(StatusCode, Json<StoreCartResponse>)> {
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    log_storefront_cart_route_entry("create_cart", tenant.id, None, None);

    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;

    let customer_id =
        super::current_customer_id_for_db(runtime.db(), tenant.id, auth.0.as_ref()).await?;
    let context = super::resolve_context_for_db(
        runtime.db(),
        tenant.id,
        &request_context,
        input.region_id,
        input.country_code.clone(),
        input.locale.clone(),
        input.currency_code.clone(),
    )
    .await?;
    let currency_code = context
        .currency_code
        .clone()
        .or(input.currency_code.clone())
        .ok_or_else(|| {
            HttpError::bad_request(
                "commerce_store_cart_context_invalid",
                "Currency code is required unless it can be resolved from region or country",
            )
        })?;

    let cart = in_process_cart_storefront_port(runtime.db_clone())
        .create_storefront_cart(
            super::storefront_cart_port_context(
                tenant.id,
                &request_context,
                auth.0.as_ref(),
                tenant.id,
                "create",
                true,
            ),
            CartStorefrontCreateRequest {
                input: crate::dto::CreateCartInput {
                    customer_id,
                    email: input.email,
                    region_id: context.region.as_ref().map(|region| region.id),
                    country_code: input.country_code,
                    locale_code: Some(context.locale.clone()),
                    selected_shipping_option_id: None,
                    currency_code,
                    metadata: super::sanitize_storefront_metadata(input.metadata),
                },
                channel_id: request_context.channel_id,
                channel_slug: request_context.channel_slug.clone(),
            },
        )
        .await
        .map_err(|error| map_cart_port_error(error, "create_cart", tenant.id, None))?;
    let shipping_option_read_port = runtime.shipping_option_read_port();
    let cart = shipping_owner_reads::enrich_storefront_cart(
        shipping_option_read_port.as_ref(),
        tenant.id,
        &request_context,
        auth.0.as_ref(),
        tenant.default_locale.as_str(),
        cart,
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(StoreCartResponse { cart, context }),
    ))
}

/// Get storefront cart
#[utoipa::path(
    get,
    path = "/store/carts/{id}",
    tag = "store",
    params(("id" = Uuid, Path, description = "Cart ID")),
    responses(
        (status = 200, description = "Cart details", body = CartResponse),
        (status = 401, description = "Authentication required for customer-owned carts", body = rustok_web::ErrorBody),
        (status = 403, description = "Cart access denied: guest capability or customer ownership", body = rustok_web::ErrorBody),
        (status = 404, description = "Cart not found", body = rustok_web::ErrorBody)
    )
)]
pub async fn get_cart(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: OptionalAuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
) -> HttpResult<Json<CartResponse>> {
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_cart_id",
            "Cart ID must not be nil",
        ));
    }
    log_storefront_cart_route_entry("get_cart", tenant.id, Some(id), None);

    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;

    let customer_id =
        super::current_customer_id_for_db(runtime.db(), tenant.id, auth.0.as_ref()).await?;
    let cart = in_process_cart_storefront_port(runtime.db_clone())
        .read_storefront_cart(
            super::storefront_cart_port_context(
                tenant.id,
                &request_context,
                auth.0.as_ref(),
                id,
                "read",
                false,
            ),
            CartStorefrontReadRequest { cart_id: id },
        )
        .await
        .map_err(|error| map_cart_port_error(error, "get_cart", tenant.id, Some(id)))?;
    super::ensure_store_cart_access(&cart, customer_id, auth.0.as_ref())?;
    let shipping_option_read_port = runtime.shipping_option_read_port();
    Ok(Json(
        shipping_owner_reads::enrich_storefront_cart(
            shipping_option_read_port.as_ref(),
            tenant.id,
            &request_context,
            auth.0.as_ref(),
            tenant.default_locale.as_str(),
            cart,
        )
        .await?,
    ))
}

/// Update storefront cart context
#[utoipa::path(
    post,
    path = "/store/carts/{id}",
    tag = "store",
    params(("id" = Uuid, Path, description = "Cart ID")),
    request_body = StoreUpdateCartInput,
    responses(
        (status = 200, description = "Updated cart context", body = StoreCartResponse),
        (status = 401, description = "Authentication required for customer-owned carts", body = rustok_web::ErrorBody),
        (status = 403, description = "Cart access denied: guest capability or customer ownership", body = rustok_web::ErrorBody),
        (status = 404, description = "Cart not found", body = rustok_web::ErrorBody)
    )
)]
pub async fn update_cart_context(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: OptionalAuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<StoreUpdateCartInput>,
) -> HttpResult<Json<StoreCartResponse>> {
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_cart_id",
            "Cart ID must not be nil",
        ));
    }
    log_storefront_cart_route_entry("update_cart_context", tenant.id, Some(id), None);

    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;

    let customer_id =
        super::current_customer_id_for_db(runtime.db(), tenant.id, auth.0.as_ref()).await?;
    let cart = in_process_cart_storefront_port(runtime.db_clone())
        .read_storefront_cart(
            super::storefront_cart_port_context(
                tenant.id,
                &request_context,
                auth.0.as_ref(),
                id,
                "read",
                false,
            ),
            CartStorefrontReadRequest { cart_id: id },
        )
        .await
        .map_err(|error| {
            map_cart_port_error(error, "update_cart_context_read", tenant.id, Some(id))
        })?;
    super::ensure_store_cart_access(&cart, customer_id, auth.0.as_ref())?;

    let shipping_option_read_port = runtime.shipping_option_read_port();
    let updated = shipping_owner_reads::apply_cart_context_patch(
        runtime.db(),
        runtime.event_bus(),
        runtime.product_catalog_read_runtime(),
        shipping_option_read_port.as_ref(),
        tenant.id,
        &request_context,
        auth.0.as_ref(),
        tenant.default_locale.as_str(),
        &cart,
        StoreCartContextPatch {
            email: input.email,
            region_id: input.region_id,
            country_code: input.country_code,
            locale: input.locale,
            selected_shipping_option_id: input.selected_shipping_option_id,
            shipping_selections: input.shipping_selections.map(|items| {
                items
                    .into_iter()
                    .map(Into::into)
                    .collect::<Vec<crate::dto::CartShippingSelectionInput>>()
            }),
        },
    )
    .await?;

    Ok(Json(updated))
}

/// Add storefront cart line item
#[utoipa::path(
    post,
    path = "/store/carts/{id}/line-items",
    tag = "store",
    params(("id" = Uuid, Path, description = "Cart ID")),
    request_body = StoreAddCartLineItemInput,
    responses(
        (status = 200, description = "Updated cart", body = CartResponse),
        (status = 401, description = "Authentication required for customer-owned carts", body = rustok_web::ErrorBody),
        (status = 403, description = "Cart access denied: guest capability or customer ownership", body = rustok_web::ErrorBody),
        (status = 404, description = "Cart not found", body = rustok_web::ErrorBody)
    )
)]
pub async fn add_cart_line_item(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: OptionalAuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(input): Json<StoreAddCartLineItemInput>,
) -> HttpResult<Json<CartResponse>> {
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_cart_id",
            "Cart ID must not be nil",
        ));
    }
    log_storefront_cart_route_entry("add_cart_line_item", tenant.id, Some(id), None);

    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;

    let customer_id =
        super::current_customer_id_for_db(runtime.db(), tenant.id, auth.0.as_ref()).await?;
    let workflow = crate::storefront_cart_runtime::StorefrontCartRuntime::new(
        runtime.product_catalog_read_port(),
        in_process_pricing_read_port(
            runtime.db_clone(),
            runtime.event_bus(),
            runtime.product_catalog_read_runtime(),
        ),
        in_process_cart_storefront_port(runtime.db_clone()),
        rustok_inventory::in_process_inventory_reservation_port(
            runtime.db_clone(),
            runtime.event_bus(),
        ),
    );
    let cart = workflow
        .add_line_item(
            super::storefront_cart_port_context(
                tenant.id,
                &request_context,
                auth.0.as_ref(),
                id,
                "add-line-item",
                true,
            ),
            customer_id,
            tenant.default_locale.as_str(),
            crate::storefront_cart_runtime::StorefrontCartAddCommand {
                cart_id: id,
                variant_id: input.variant_id,
                quantity: input.quantity,
                expected_context: input.expected_context.map(Into::into),
                expected_locale: input.expected_locale,
                metadata: input.metadata,
            },
        )
        .await
        .map_err(|error| map_cart_port_error(error, "add_cart_line_item", tenant.id, Some(id)))?;
    let shipping_option_read_port = runtime.shipping_option_read_port();
    Ok(Json(
        shipping_owner_reads::enrich_storefront_cart(
            shipping_option_read_port.as_ref(),
            tenant.id,
            &request_context,
            auth.0.as_ref(),
            tenant.default_locale.as_str(),
            cart,
        )
        .await?,
    ))
}

/// Update storefront cart line item quantity
#[utoipa::path(
    post,
    path = "/store/carts/{id}/line-items/{line_id}",
    tag = "store",
    params(
        ("id" = Uuid, Path, description = "Cart ID"),
        ("line_id" = Uuid, Path, description = "Cart line item ID")
    ),
    request_body = StoreUpdateCartLineItemInput,
    responses(
        (status = 200, description = "Updated cart", body = CartResponse),
        (status = 401, description = "Authentication required for customer-owned carts", body = rustok_web::ErrorBody),
        (status = 403, description = "Cart access denied: guest capability or customer ownership", body = rustok_web::ErrorBody),
        (status = 404, description = "Cart or line item not found", body = rustok_web::ErrorBody),
        (status = 400, description = "Invalid quantity, snapshot or original SKU", body = rustok_web::ErrorBody),
        (status = 409, description = "Cart inputs, fulfillment snapshot or stock availability changed; inspect before retrying", body = rustok_web::ErrorBody),
        (status = 503, description = "Required owner capability is unavailable", body = rustok_web::ErrorBody),
        (status = 504, description = "Owner observation exceeded its deadline", body = rustok_web::ErrorBody)
    )
)]
pub async fn update_cart_line_item(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: OptionalAuthContext,
    request_context: RequestContext,
    Path((id, line_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<StoreUpdateCartLineItemInput>,
) -> HttpResult<Json<CartResponse>> {
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_cart_id",
            "Cart ID must not be nil",
        ));
    }
    if line_id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_line_item_id",
            "Line item ID must not be nil",
        ));
    }
    log_storefront_cart_route_entry("update_cart_line_item", tenant.id, Some(id), Some(line_id));

    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;

    let customer_id =
        super::current_customer_id_for_db(runtime.db(), tenant.id, auth.0.as_ref()).await?;
    let storefront_port = in_process_cart_storefront_port(runtime.db_clone());
    let existing = storefront_port
        .read_storefront_cart(
            super::storefront_cart_port_context(
                tenant.id,
                &request_context,
                auth.0.as_ref(),
                id,
                "read",
                false,
            ),
            CartStorefrontReadRequest { cart_id: id },
        )
        .await
        .map_err(|error| {
            map_cart_port_error(error, "update_cart_line_item_read", tenant.id, Some(id))
        })?;
    super::ensure_store_cart_access(&existing, customer_id, auth.0.as_ref())?;
    let workflow = crate::storefront_cart_runtime::StorefrontCartRuntime::new(
        runtime.product_catalog_read_port(),
        in_process_pricing_read_port(
            runtime.db_clone(),
            runtime.event_bus(),
            runtime.product_catalog_read_runtime(),
        ),
        storefront_port,
        rustok_inventory::in_process_inventory_reservation_port(
            runtime.db_clone(),
            runtime.event_bus(),
        ),
    );
    let cart = workflow
        .update_line_item_quantity(
            super::storefront_cart_port_context(
                tenant.id,
                &request_context,
                auth.0.as_ref(),
                id,
                "update-line-item",
                true,
            ),
            customer_id,
            tenant.default_locale.as_str(),
            existing,
            line_id,
            input.quantity,
        )
        .await
        .map_err(|error| {
            map_cart_port_error(error, "update_cart_line_item", tenant.id, Some(id))
        })?;
    let shipping_option_read_port = runtime.shipping_option_read_port();
    Ok(Json(
        shipping_owner_reads::enrich_storefront_cart(
            shipping_option_read_port.as_ref(),
            tenant.id,
            &request_context,
            auth.0.as_ref(),
            tenant.default_locale.as_str(),
            cart,
        )
        .await?,
    ))
}

/// Remove storefront cart line item
#[utoipa::path(
    delete,
    path = "/store/carts/{id}/line-items/{line_id}",
    tag = "store",
    params(
        ("id" = Uuid, Path, description = "Cart ID"),
        ("line_id" = Uuid, Path, description = "Cart line item ID")
    ),
    responses(
        (status = 200, description = "Updated cart", body = CartResponse),
        (status = 401, description = "Authentication required for customer-owned carts", body = rustok_web::ErrorBody),
        (status = 403, description = "Cart access denied: guest capability or customer ownership", body = rustok_web::ErrorBody),
        (status = 404, description = "Cart or line item not found", body = rustok_web::ErrorBody)
    )
)]
pub async fn remove_cart_line_item(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: OptionalAuthContext,
    request_context: RequestContext,
    Path((id, line_id)): Path<(Uuid, Uuid)>,
) -> HttpResult<Json<CartResponse>> {
    if tenant.id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_cart_id",
            "Cart ID must not be nil",
        ));
    }
    if line_id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_line_item_id",
            "Line item ID must not be nil",
        ));
    }
    log_storefront_cart_route_entry("remove_cart_line_item", tenant.id, Some(id), Some(line_id));

    super::ensure_storefront_channel_enabled_for_db(runtime.db(), &request_context).await?;

    let customer_id =
        super::current_customer_id_for_db(runtime.db(), tenant.id, auth.0.as_ref()).await?;
    let storefront_port = in_process_cart_storefront_port(runtime.db_clone());
    let existing = storefront_port
        .read_storefront_cart(
            super::storefront_cart_port_context(
                tenant.id,
                &request_context,
                auth.0.as_ref(),
                id,
                "read",
                false,
            ),
            CartStorefrontReadRequest { cart_id: id },
        )
        .await
        .map_err(|error| {
            map_cart_port_error(error, "remove_cart_line_item_read", tenant.id, Some(id))
        })?;
    super::ensure_store_cart_access(&existing, customer_id, auth.0.as_ref())?;

    let cart = storefront_port
        .remove_storefront_line_item(
            super::storefront_cart_port_context(
                tenant.id,
                &request_context,
                auth.0.as_ref(),
                id,
                "remove-line-item",
                true,
            ),
            CartStorefrontRemoveLineItemRequest {
                cart_id: id,
                line_item_id: line_id,
                pricing_context: rustok_cart::CartPricingContextGuard::capture_removal(
                    &existing, line_id,
                )
                .map_err(port_error_to_http_error)?,
            },
        )
        .await
        .map_err(|error| {
            map_cart_port_error(error, "remove_cart_line_item", tenant.id, Some(id))
        })?;
    let shipping_option_read_port = runtime.shipping_option_read_port();
    Ok(Json(
        shipping_owner_reads::enrich_storefront_cart(
            shipping_option_read_port.as_ref(),
            tenant.id,
            &request_context,
            auth.0.as_ref(),
            tenant.default_locale.as_str(),
            cart,
        )
        .await?,
    ))
}
