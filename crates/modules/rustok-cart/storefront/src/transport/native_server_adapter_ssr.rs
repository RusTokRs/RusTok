use leptos::prelude::*;
use uuid::Uuid;

use crate::core::{CartCoreError, parse_cart_id, parse_line_item_id};
use crate::model::StorefrontCartData;

use super::native_server_mapping::map_native_cart;

const CART_STOREFRONT_NATIVE_BOUNDARY: &str = "cart_storefront_native_transport";
const CART_STOREFRONT_NATIVE_OWNER: &str = "rustok_cart.storefront";

fn context_extraction_error<E>(
    owner_operation: &'static str,
    code: &'static str,
    public_message: &'static str,
    _error: E,
) -> ServerFnError {
    let error_type = std::any::type_name::<E>();
    tracing::error!(
        error_type,
        owner = CART_STOREFRONT_NATIVE_OWNER,
        owner_operation,
        code,
        boundary = CART_STOREFRONT_NATIVE_BOUNDARY,
        "cart storefront request context extraction failed"
    );
    ServerFnError::new(public_message)
}

pub(super) fn tenant_context_error<E>(error: E) -> ServerFnError {
    context_extraction_error(
        "extract_tenant_context",
        "cart.storefront_tenant_context_unavailable",
        "Storefront tenant context is temporarily unavailable",
        error,
    )
}

pub(super) fn auth_context_error<E>(error: E) -> ServerFnError {
    context_extraction_error(
        "extract_optional_auth_context",
        "cart.storefront_auth_context_unavailable",
        "Storefront authentication context is temporarily unavailable",
        error,
    )
}

pub(super) fn transactional_event_bus_from_runtime(
    runtime_ctx: &rustok_api::HostRuntimeContext,
    endpoint: &'static str,
) -> Result<rustok_outbox::TransactionalEventBus, ServerFnError> {
    runtime_ctx
        .shared_get::<rustok_outbox::TransactionalEventBus>()
        .ok_or_else(|| {
            tracing::error!(
                owner = CART_STOREFRONT_NATIVE_OWNER,
                owner_operation = "resolve_transactional_event_bus",
                endpoint,
                code = "cart.storefront_event_bus_unavailable",
                boundary = CART_STOREFRONT_NATIVE_BOUNDARY,
                "cart storefront transactional event bus is missing"
            );
            ServerFnError::new("Cart runtime is temporarily unavailable")
        })
}

fn cart_input_error(
    error: CartCoreError,
    owner_operation: &'static str,
    code: &'static str,
    public_message: &'static str,
) -> ServerFnError {
    let error_type = std::any::type_name_of_val(&error);
    tracing::warn!(
        error_type,
        owner = CART_STOREFRONT_NATIVE_OWNER,
        owner_operation,
        code,
        boundary = CART_STOREFRONT_NATIVE_BOUNDARY,
        "cart storefront input was rejected"
    );
    ServerFnError::new(public_message)
}

fn customer_error(
    error: rustok_customer::CustomerError,
    tenant_id: Uuid,
    user_id: Uuid,
    owner_operation: &'static str,
    request_context: Option<&rustok_api::RequestContext>,
) -> ServerFnError {
    let error_type = std::any::type_name_of_val(&error);
    tracing::error!(
        error_type,
        owner = "rustok_customer",
        owner_operation,
        consumer = CART_STOREFRONT_NATIVE_OWNER,
        correlation_id = ?request_context.map(|context| &context.correlation_id),
        request_context_present = request_context.is_some(),
        request_tenant_id_non_nil = ?request_context.map(|context| !context.tenant_id.is_nil()),
        tenant_id_non_nil = !tenant_id.is_nil(),
        user_id_non_nil = !user_id.is_nil(),
        channel_id_present = request_context.and_then(|context| context.channel_id).is_some(),
        channel_id_non_nil = ?request_context
            .and_then(|context| context.channel_id)
            .map(|value| !value.is_nil()),
        channel_slug_present = request_context
            .and_then(|context| context.channel_slug.as_ref())
            .is_some(),
        channel_slug_length = ?request_context
            .and_then(|context| context.channel_slug.as_ref())
            .map(|value| value.chars().count()),
        locale_present = request_context
            .map(|context| !context.locale.trim().is_empty())
            .unwrap_or(false),
        locale_length = ?request_context.map(|context| context.locale.chars().count()),
        code = "cart.storefront_customer_unavailable",
        boundary = CART_STOREFRONT_NATIVE_BOUNDARY,
        "cart storefront customer lookup failed"
    );
    ServerFnError::new("Customer information is temporarily unavailable")
}

pub(super) async fn resolve_storefront_customer_id(
    db: sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    auth: Option<rustok_api::AuthContext>,
    request_context: Option<&rustok_api::RequestContext>,
    owner_operation: &'static str,
) -> Result<Option<Uuid>, ServerFnError> {
    let Some(auth) = auth else {
        return Ok(None);
    };

    match rustok_customer::CustomerService::new(db)
        .get_customer_by_user(tenant_id, auth.user_id)
        .await
    {
        Ok(customer) => Ok(Some(customer.id)),
        Err(rustok_customer::CustomerError::CustomerByUserNotFound(_)) => Ok(None),
        Err(error) => Err(customer_error(
            error,
            tenant_id,
            auth.user_id,
            owner_operation,
            request_context,
        )),
    }
}

// The guarded owner already verifies guest capability before sanitizing the response.
fn ensure_storefront_cart_access(
    cart: &rustok_cart::CartResponse,
    customer_id: Option<Uuid>,
) -> Result<(), ServerFnError> {
    if cart.customer_id.is_some() && cart.customer_id != customer_id {
        return Err(ServerFnError::new(
            "Cart does not belong to the current storefront customer",
        ));
    }
    Ok(())
}

pub(super) async fn storefront_cart_native(
    selected_cart_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontCartData, ServerFnError> {
    let scope = super::buyer_native::buyer_scope("read_cart").await?;
    let Some((normalized_cart_id, cart_id)) = parse_cart_id(selected_cart_id).map_err(|error| {
        cart_input_error(
            error,
            "parse_storefront_cart_id",
            "cart.storefront_cart_id_invalid",
            "Invalid cart selection",
        )
    })?
    else {
        let _ = locale;
        return Ok(StorefrontCartData {
            selected_cart_id: None,
            cart: None,
        });
    };

    // A restore/inspection is an owner read, exactly as GraphQL storefrontCart. It does not
    // reprice or mutate a cart as a side effect, and it never borrows the page's channel.
    let cart = match rustok_cart::in_process_cart_storefront_port(scope.host.db_clone())
        .read_storefront_cart(
            super::buyer_native::port_context(&scope, "read"),
            rustok_cart::CartStorefrontReadRequest { cart_id },
        )
        .await
    {
        Ok(cart) => cart,
        Err(error) if error.code == "cart.cart_not_found" => {
            return Ok(StorefrontCartData {
                selected_cart_id: Some(normalized_cart_id),
                cart: None,
            });
        }
        Err(error) => return Err(super::buyer_native::port_error(error)),
    };
    // The guarded owner already checked the guest capability before sanitizing its metadata.
    // Rechecking a guest hash on this public projection would reject every valid guest cart.
    if cart.customer_id.is_some() && cart.customer_id != scope.customer_id {
        return Err(ServerFnError::new(
            "Cart does not belong to the current storefront customer",
        ));
    }

    let _ = locale;
    Ok(StorefrontCartData {
        selected_cart_id: Some(normalized_cart_id),
        cart: Some(map_native_cart(cart)),
    })
}

pub(super) async fn storefront_cart_decrement_line_item(
    cart_id: String,
    line_item_id: String,
) -> Result<(), ServerFnError> {
    let scope = super::buyer_native::buyer_scope("decrement_line_item").await?;
    let (_, cart_id) = parse_cart_id(Some(cart_id))
        .map_err(|error| {
            cart_input_error(
                error,
                "parse_decrement_cart_id",
                "cart.storefront_cart_id_invalid",
                "Invalid cart selection",
            )
        })?
        .ok_or_else(|| ServerFnError::new("Invalid cart selection"))?;
    let (_, line_id) = parse_line_item_id(line_item_id).map_err(|error| {
        cart_input_error(
            error,
            "parse_decrement_line_item_id",
            "cart.storefront_line_item_id_invalid",
            "Invalid cart line item selection",
        )
    })?;
    let port = rustok_cart::in_process_cart_storefront_port(scope.host.db_clone());
    let cart = port
        .read_storefront_cart(
            super::buyer_native::port_context(&scope, "read"),
            rustok_cart::CartStorefrontReadRequest { cart_id },
        )
        .await
        .map_err(super::buyer_native::port_error)?;
    ensure_storefront_cart_access(&cart, scope.customer_id)?;
    let quantity = cart
        .line_items
        .iter()
        .find(|line| line.id == line_id)
        .ok_or_else(|| ServerFnError::new("Cart line item was not found"))?
        .quantity;
    if quantity < 1 {
        return Err(ServerFnError::new(
            "Cart quantity is invalid; inspect the cart before changing it",
        ));
    }
    if quantity == 1 {
        let pricing_context = rustok_cart::CartPricingContextGuard::capture_removal(&cart, line_id)
            .map_err(super::buyer_native::port_error)?;
        port.remove_storefront_line_item(
            super::buyer_native::port_context(&scope, "decrement-remove"),
            rustok_cart::CartStorefrontRemoveLineItemRequest {
                cart_id,
                line_item_id: line_id,
                pricing_context,
            },
        )
        .await
        .map_err(super::buyer_native::port_error)?;
    } else {
        let events = transactional_event_bus_from_runtime(&scope.host, "cart/decrement-line-item")?;
        let product = scope
            .host
            .shared_get::<rustok_product::ProductCatalogReadRuntime>()
            .unwrap_or_else(|| {
                rustok_product::ProductCatalogReadRuntime::in_process(
                    scope.host.db_clone(),
                    events.clone(),
                )
            });
        let workflow = rustok_commerce::storefront_cart_runtime::StorefrontCartRuntime::new(
            product.read_port(),
            rustok_pricing::in_process_pricing_read_port(
                scope.host.db_clone(),
                events.clone(),
                product,
            ),
            port,
            rustok_inventory::in_process_inventory_reservation_port(scope.host.db_clone(), events),
        );
        workflow
            .update_line_item_quantity(
                super::buyer_native::port_context(&scope, "decrement"),
                scope.customer_id,
                &scope.tenant.default_locale,
                cart,
                line_id,
                quantity - 1,
            )
            .await
            .map_err(super::buyer_native::port_error)?;
    }
    Ok(())
}

pub(super) async fn storefront_cart_remove_line_item(
    cart_id: String,
    line_item_id: String,
) -> Result<(), ServerFnError> {
    let scope = super::buyer_native::buyer_scope("remove_line_item").await?;
    let (_, cart_id) = parse_cart_id(Some(cart_id))
        .map_err(|error| {
            cart_input_error(
                error,
                "parse_remove_cart_id",
                "cart.storefront_cart_id_invalid",
                "Invalid cart selection",
            )
        })?
        .ok_or_else(|| ServerFnError::new("Invalid cart selection"))?;
    let (_, line_item_id) = parse_line_item_id(line_item_id).map_err(|error| {
        cart_input_error(
            error,
            "parse_remove_line_item_id",
            "cart.storefront_line_item_id_invalid",
            "Invalid cart line item selection",
        )
    })?;
    let port = rustok_cart::in_process_cart_storefront_port(scope.host.db_clone());
    let cart = port
        .read_storefront_cart(
            super::buyer_native::port_context(&scope, "read"),
            rustok_cart::CartStorefrontReadRequest { cart_id },
        )
        .await
        .map_err(super::buyer_native::port_error)?;
    ensure_storefront_cart_access(&cart, scope.customer_id)?;
    let pricing_context =
        rustok_cart::CartPricingContextGuard::capture_removal(&cart, line_item_id)
            .map_err(super::buyer_native::port_error)?;
    port.remove_storefront_line_item(
        super::buyer_native::port_context(&scope, "remove"),
        rustok_cart::CartStorefrontRemoveLineItemRequest {
            cart_id,
            line_item_id,
            pricing_context,
        },
    )
    .await
    .map_err(super::buyer_native::port_error)?;
    Ok(())
}
