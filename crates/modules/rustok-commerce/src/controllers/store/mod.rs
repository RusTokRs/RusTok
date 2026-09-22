pub mod carts;
pub mod checkout;
pub(crate) mod line_item_resolution;
pub mod orders;
#[path = "products_owner_list.rs"]
pub mod products;
#[path = "products.rs"]
pub mod products_legacy;

pub use carts::*;
pub use checkout::*;
pub use orders::*;
pub use products::*;

#[cfg(test)]
mod tests;

use axum::http::StatusCode;
use rust_decimal::Decimal;
use rustok_api::locale_tags_match;
use rustok_api::{PortActor, PortContext, PortError, RequestContext};
use rustok_cart::{CartStorefrontPort, CartStorefrontRepriceRequest};
use rustok_channel::error::ChannelError;
use rustok_customer::{CustomerUserProjectionRequest, in_process_customer_read_port};

use rustok_pricing::{
    PriceResolutionContext, PricingReadPort, ResolveProductPriceRequest,
    in_process_pricing_read_port,
};
use rustok_product::entities::{product_translation, variant_translation};
use rustok_web::{HttpError, HttpResult, port_error_to_http_error};
use sea_orm::DatabaseConnection;
use serde::de::Deserializer;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::common::PaginationParams;
use crate::{
    StoreContextError, StoreContextService,
    dto::{AddCartLineItemInput, CartResponse, ResolveStoreContextInput, StoreContextResponse},
    storefront_channel::{
        is_module_enabled_for_request_channel, normalize_public_channel_slug,
        public_channel_slug_from_request,
    },
};

pub const MODULE_SLUG: &str = "commerce";

fn map_storefront_context_error(error: StoreContextError, tenant_id: Uuid) -> HttpError {
    let (status, code, message, error_kind) = match &error {
        StoreContextError::TenantNotFound(_) => (
            StatusCode::NOT_FOUND,
            "commerce_store_context_not_found",
            "Store context was not found".to_string(),
            "tenant_not_found",
        ),
        StoreContextError::Validation(msg) => (
            StatusCode::BAD_REQUEST,
            "commerce_store_context_invalid",
            msg.clone(),
            "validation",
        ),
        err @ StoreContextError::CurrencyRegionMismatch { .. } => (
            StatusCode::BAD_REQUEST,
            "commerce_store_context_invalid",
            err.to_string(),
            "validation",
        ),
        StoreContextError::TenantBoundary { .. } => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_store_context_failed",
            "Store context could not be resolved safely".to_string(),
            "tenant_boundary",
        ),
        StoreContextError::RegionBoundary { .. } => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_store_context_failed",
            "Store context could not be resolved safely".to_string(),
            "region_boundary",
        ),
        StoreContextError::Database(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_store_context_unavailable",
            "Store context is temporarily unavailable".to_string(),
            "database",
        ),
    };
    tracing::error!(
        error = ?error,
        owner = "rustok_commerce.store_context",
        operation = "resolve_store_context",
        tenant_id = %tenant_id,
        error_kind,
        public_code = code,
        status = %status,
        boundary = "commerce_storefront_shared_http",
        "storefront context resolution failed"
    );
    HttpError::new(status, code, message)
}

fn map_storefront_customer_port_error(
    error: PortError,
    tenant_id: Uuid,
    user_id: Uuid,
) -> HttpError {
    let public = port_error_to_http_error(error.clone());
    tracing::error!(
        error = ?error,
        owner = "rustok_customer",
        operation = "resolve_current_customer",
        tenant_id = %tenant_id,
        user_id = %user_id,
        error_kind = ?error.kind,
        retryable = error.retryable,
        public_code = %public.code,
        status = %public.status,
        boundary = "commerce_storefront_shared_http",
        "storefront customer projection failed"
    );
    public
}

fn map_storefront_channel_error(
    error: ChannelError,
    request_context: &RequestContext,
) -> HttpError {
    let (status, code, message, error_kind) = match &error {
        ChannelError::NotFound(_) | ChannelError::InactiveChannel(_) => (
            StatusCode::NOT_FOUND,
            "commerce_store_channel_not_found",
            "Store channel was not found",
            "not_found",
        ),
        ChannelError::Database(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "commerce_store_channel_unavailable",
            "Store channel is temporarily unavailable",
            "database",
        ),
        ChannelError::SlugAlreadyExists(_)
        | ChannelError::InvalidTargetType(_)
        | ChannelError::InvalidTargetValue(_)
        | ChannelError::InvalidPolicyDefinition(_)
        | ChannelError::TargetAlreadyExists(_, _)
        | ChannelError::PolicySetSlugAlreadyExists(_)
        | ChannelError::InvalidPolicyOperation(_)
        | ChannelError::Serialization(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_store_channel_failed",
            "Store channel could not be resolved safely",
            "unexpected_owner_error",
        ),
    };
    tracing::error!(
        error = ?error,
        owner = "rustok_channel",
        operation = "ensure_storefront_channel_enabled",
        channel_id = ?request_context.channel_id,
        channel_slug = ?request_context.channel_slug,
        error_kind,
        public_code = code,
        status = %status,
        boundary = "commerce_storefront_shared_http",
        "storefront channel resolution failed"
    );
    HttpError::new(status, code, message)
}

pub fn axum_router() -> axum::Router<super::CommerceHttpRuntime> {
    axum::Router::new()
        .route("/products", axum::routing::get(products::list_products))
        .route("/products/{id}", axum::routing::get(products::show_product))
        .route("/regions", axum::routing::get(products::list_regions))
        .route(
            "/shipping-options",
            axum::routing::get(products::list_shipping_options),
        )
        .route("/carts", axum::routing::post(carts::create_cart))
        .route(
            "/carts/{id}",
            axum::routing::get(carts::get_cart).post(carts::update_cart_context),
        )
        .route(
            "/carts/{id}/line-items",
            axum::routing::post(carts::add_cart_line_item),
        )
        .route(
            "/carts/{id}/line-items/{line_id}",
            axum::routing::post(carts::update_cart_line_item).delete(carts::remove_cart_line_item),
        )
        .route(
            "/carts/{id}/complete",
            axum::routing::post(checkout::complete_cart_checkout),
        )
        .route(
            "/payment-collections",
            axum::routing::post(checkout::create_payment_collection),
        )
        .route("/orders/{id}", axum::routing::get(orders::get_order))
        .route(
            "/orders/{id}/returns",
            axum::routing::get(orders::list_order_returns).post(orders::create_order_return),
        )
        .route(
            "/orders/{id}/refunds",
            axum::routing::get(orders::list_order_refunds),
        )
        .route(
            "/orders/{id}/changes",
            axum::routing::get(orders::list_order_changes),
        )
        .route("/customers/me", axum::routing::get(orders::get_me))
}

pub(crate) async fn resolve_context_for_db(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    request_context: &RequestContext,
    region_id: Option<Uuid>,
    country_code: Option<String>,
    locale: Option<String>,
    currency_code: Option<String>,
) -> HttpResult<StoreContextResponse> {
    let service = StoreContextService::new(
        db.clone(),
        std::sync::Arc::new(rustok_region::RegionService::new(db.clone())),
    );
    service
        .resolve_context(
            tenant_id,
            ResolveStoreContextInput {
                region_id,
                country_code,
                locale: locale.or_else(|| Some(request_context.locale.clone())),
                currency_code,
            },
        )
        .await
        .map_err(|error| map_storefront_context_error(error, tenant_id))
}

pub(crate) async fn resolve_context_from_cart_for_db(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    request_context: &RequestContext,
    cart: &CartResponse,
) -> HttpResult<StoreContextResponse> {
    resolve_context_for_db(
        db,
        tenant_id,
        request_context,
        cart.region_id,
        cart.country_code.clone(),
        cart.locale_code.clone(),
        Some(cart.currency_code.clone()),
    )
    .await
}

pub(crate) async fn current_customer_id_for_db(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    auth: Option<&rustok_api::AuthContext>,
) -> HttpResult<Option<Uuid>> {
    let Some(auth) = auth else {
        return Ok(None);
    };

    match in_process_customer_read_port(db.clone())
        .read_customer_projection_by_user(
            storefront_customer_port_context(tenant_id, auth.user_id),
            CustomerUserProjectionRequest {
                user_id: auth.user_id,
            },
        )
        .await
    {
        Ok(customer) => Ok(Some(customer.id)),
        Err(error) if error.code == "customer.customer_by_user_not_found" => Ok(None),
        Err(error) => Err(map_storefront_customer_port_error(
            error,
            tenant_id,
            auth.user_id,
        )),
    }
}

pub(crate) fn storefront_customer_port_context(tenant_id: Uuid, user_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(user_id.to_string()),
        "en",
        format!("storefront-customer:{user_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2))
}

pub(crate) fn storefront_cart_port_context(
    tenant_id: Uuid,
    request_context: &RequestContext,
    auth: Option<&rustok_api::AuthContext>,
    resource_id: Uuid,
    operation: &str,
    is_write: bool,
) -> PortContext {
    let actor = auth
        .map(|value| PortActor::user(value.user_id.to_string()))
        .unwrap_or_else(|| PortActor::service("rustok-commerce.storefront"));
    let correlation_id = format!("storefront-cart:{operation}:{resource_id}");
    let context = PortContext::new(
        tenant_id.to_string(),
        actor,
        request_context.locale.as_str(),
        correlation_id.clone(),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    let context = request_context
        .channel_slug
        .as_deref()
        .map(|channel| context.clone().with_channel(channel))
        .unwrap_or(context);
    if is_write {
        context.with_idempotency_key(correlation_id)
    } else {
        context
    }
}

pub(crate) async fn ensure_storefront_channel_enabled_for_db(
    db: &DatabaseConnection,
    request_context: &RequestContext,
) -> HttpResult<()> {
    let enabled = is_module_enabled_for_request_channel(db, request_context, MODULE_SLUG)
        .await
        .map_err(|error| map_storefront_channel_error(error, request_context))?;

    if !enabled {
        return Err(HttpError::unauthorized(
            "commerce_store_denied",
            format!(
                "Module '{MODULE_SLUG}' is not enabled for channel '{}'",
                request_context.channel_slug.as_deref().unwrap_or("current"),
            ),
        ));
    }

    Ok(())
}

pub(crate) fn storefront_public_channel_slug_for_cart(
    cart: &CartResponse,
    request_context: &RequestContext,
) -> Option<String> {
    normalize_public_channel_slug(cart.channel_slug.as_deref())
        .or_else(|| public_channel_slug_from_request(request_context))
}

pub(crate) fn ensure_store_cart_access(
    cart: &CartResponse,
    customer_id: Option<Uuid>,
) -> HttpResult<()> {
    if let Some(expected_customer_id) = cart.customer_id
        && customer_id != Some(expected_customer_id)
    {
        return Err(HttpError::unauthorized(
            "commerce_store_denied",
            "Cart belongs to another customer".to_string(),
        ));
    }

    Ok(())
}

pub(crate) fn ensure_cart_allows_payment_collection(cart: &CartResponse) -> HttpResult<()> {
    if cart.status == "completed" {
        return Err(HttpError::bad_request(
            "commerce_store_invalid",
            "Cannot create payment collection for completed cart".to_string(),
        ));
    }

    Ok(())
}

pub(crate) fn checkout_actor_id(auth: Option<&rustok_api::AuthContext>) -> Uuid {
    auth.map(|auth| auth.user_id).unwrap_or_else(Uuid::nil)
}

pub(crate) async fn reprice_storefront_cart_line_items_for_db(
    db: &DatabaseConnection,
    event_bus: rustok_outbox::TransactionalEventBus,
    tenant_id: Uuid,
    request_context: &RequestContext,
    storefront_port: &dyn CartStorefrontPort,
    cart: CartResponse,
) -> HttpResult<CartResponse> {
    if cart.line_items.is_empty() {
        return Ok(cart);
    }

    let pricing_read_port = in_process_pricing_read_port(db.clone(), event_bus);
    let mut updates = Vec::new();
    for line_item in &cart.line_items {
        let Some(variant_id) = line_item.variant_id else {
            continue;
        };
        let pricing_context =
            build_store_pricing_context(&cart, request_context, line_item.quantity);
        let resolved_price: rustok_pricing::ResolvedPrice = pricing_read_port
            .resolve_product_price(
                storefront_pricing_port_context(tenant_id, request_context, cart.id, line_item.id),
                ResolveProductPriceRequest {
                    product_id: line_item.product_id,
                    variant_id,
                    region_id: pricing_context.region_id,
                    channel_id: pricing_context.channel_id,
                    channel_slug: pricing_context.channel_slug,
                    price_list_id: pricing_context.price_list_id,
                    quantity: pricing_context.quantity,
                    currency_code: pricing_context.currency_code,
                },
            )
            .await
            .map_err(rustok_web::port_error_to_http_error)?
            .into();
        updates.push(storefront_cart_pricing_update(
            line_item.id,
            line_item.quantity,
            &resolved_price,
        ));
    }

    if updates.is_empty() {
        Ok(cart)
    } else {
        storefront_port
            .reprice_storefront_line_items(
                storefront_cart_port_context(
                    tenant_id,
                    request_context,
                    None,
                    cart.id,
                    "reprice",
                    true,
                ),
                CartStorefrontRepriceRequest {
                    cart_id: cart.id,
                    updates,
                },
            )
            .await
            .map_err(rustok_web::port_error_to_http_error)
    }
}

pub(crate) fn storefront_pricing_port_context(
    tenant_id: Uuid,
    request_context: &RequestContext,
    cart_id: Uuid,
    line_item_id: Uuid,
) -> PortContext {
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::service("rustok-commerce.storefront-pricing"),
        request_context.locale.as_str(),
        format!("storefront-pricing:{cart_id}:{line_item_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    request_context
        .channel_slug
        .as_deref()
        .map(|channel| context.clone().with_channel(channel))
        .unwrap_or(context)
}

pub(crate) fn storefront_cart_pricing_update(
    line_item_id: Uuid,
    quantity: i32,
    resolved_price: &rustok_pricing::ResolvedPrice,
) -> rustok_cart::services::cart::CartLineItemPricingUpdate {
    let (base_unit_price, pricing_adjustment) =
        storefront_cart_pricing_snapshot(quantity, resolved_price);

    rustok_cart::services::cart::CartLineItemPricingUpdate {
        line_item_id,
        unit_price: base_unit_price,
        pricing_adjustment,
    }
}

pub(crate) fn storefront_cart_pricing_snapshot(
    quantity: i32,
    resolved_price: &rustok_pricing::ResolvedPrice,
) -> (
    Decimal,
    Option<rustok_cart::services::cart::CartPricingAdjustmentUpdate>,
) {
    let base_unit_price = resolved_price
        .compare_at_amount
        .filter(|compare_at| *compare_at > resolved_price.amount)
        .unwrap_or(resolved_price.amount);
    let pricing_adjustment = if base_unit_price > resolved_price.amount {
        let mut metadata = serde_json::Map::new();
        metadata.insert(
            "kind".to_string(),
            Value::from(if resolved_price.price_list_id.is_some() {
                "price_list"
            } else {
                "sale"
            }),
        );
        metadata.insert(
            "base_amount".to_string(),
            Value::from(base_unit_price.normalize().to_string()),
        );
        metadata.insert(
            "effective_amount".to_string(),
            Value::from(resolved_price.amount.normalize().to_string()),
        );
        if let Some(compare_at_amount) = resolved_price.compare_at_amount {
            metadata.insert(
                "compare_at_amount".to_string(),
                Value::from(compare_at_amount.normalize().to_string()),
            );
        }
        if let Some(discount_percent) = resolved_price.discount_percent {
            metadata.insert(
                "discount_percent".to_string(),
                Value::from(discount_percent.normalize().to_string()),
            );
        }
        if let Some(price_list_id) = resolved_price.price_list_id {
            metadata.insert(
                "price_list_id".to_string(),
                Value::from(price_list_id.to_string()),
            );
        }
        if let Some(channel_id) = resolved_price.channel_id {
            metadata.insert(
                "channel_id".to_string(),
                Value::from(channel_id.to_string()),
            );
        }
        if let Some(channel_slug) = resolved_price.channel_slug.as_deref() {
            metadata.insert("channel_slug".to_string(), Value::from(channel_slug));
        }

        Some(rustok_cart::services::cart::CartPricingAdjustmentUpdate {
            source_id: resolved_price.price_list_id.map(|value| value.to_string()),
            amount: (base_unit_price - resolved_price.amount) * Decimal::from(quantity),
            metadata: Value::Object(metadata),
        })
    } else {
        None
    };

    (base_unit_price, pricing_adjustment)
}

#[derive(Debug)]
pub(crate) struct ResolvedStoreLineItemInput {
    pub(crate) add_line_item: AddCartLineItemInput,
    pub(crate) pricing_adjustment: Option<rustok_cart::services::cart::CartPricingAdjustmentUpdate>,
}

pub(crate) fn requested_cart_context(
    cart: &CartResponse,
    request_context: &RequestContext,
    patch: StoreCartContextPatch,
) -> RequestedCartContext {
    let region_was_explicit = patch.region_id.is_some();

    RequestedCartContext {
        email: patch.email.unwrap_or_else(|| cart.email.clone()),
        region_id: patch.region_id.unwrap_or(cart.region_id),
        country_code: match patch.country_code {
            Some(country_code) => country_code,
            None if region_was_explicit => None,
            None => cart.country_code.clone(),
        },
        locale: patch
            .locale
            .unwrap_or_else(|| cart.locale_code.clone())
            .or_else(|| Some(request_context.locale.clone())),
        selected_shipping_option_id: patch
            .selected_shipping_option_id
            .unwrap_or(cart.selected_shipping_option_id),
        shipping_selections: patch
            .shipping_selections
            .unwrap_or_else(|| current_shipping_selections(cart)),
    }
}

pub(crate) fn current_shipping_selections(
    cart: &CartResponse,
) -> Vec<crate::dto::CartShippingSelectionInput> {
    cart.delivery_groups
        .iter()
        .map(|group| crate::dto::CartShippingSelectionInput {
            shipping_profile_slug: group.shipping_profile_slug.clone(),
            seller_id: group.seller_id.clone(),
            seller_scope: None,
            selected_shipping_option_id: group
                .selected_shipping_option_id
                .or(cart.selected_shipping_option_id),
        })
        .collect()
}

pub(crate) fn build_store_pricing_context(
    cart: &CartResponse,
    request_context: &RequestContext,
    quantity: i32,
) -> PriceResolutionContext {
    PriceResolutionContext {
        currency_code: cart.currency_code.to_ascii_uppercase(),
        region_id: cart.region_id,
        price_list_id: None,
        channel_id: cart.channel_id.or(request_context.channel_id),
        channel_slug: storefront_public_channel_slug_for_cart(cart, request_context),
        quantity: Some(quantity),
    }
}

pub(crate) struct StoreLineItemResolution<'a> {
    pub(crate) pricing_read_port: &'a dyn PricingReadPort,
    pub(crate) pricing_context: &'a PriceResolutionContext,
    pub(crate) locale: &'a str,
    pub(crate) default_locale: &'a str,
    pub(crate) public_channel_slug: Option<&'a str>,
    pub(crate) input: StoreAddCartLineItemInput,
}

fn store_line_item_pricing_port_context(
    tenant_id: Uuid,
    variant_id: Uuid,
    locale: &str,
    pricing_context: &PriceResolutionContext,
) -> PortContext {
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::service("rustok-commerce.storefront-pricing"),
        locale,
        format!("storefront-add-line-item:{variant_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2));
    pricing_context
        .channel_slug
        .as_deref()
        .map(|channel| context.clone().with_channel(channel))
        .unwrap_or(context)
}

pub(crate) fn pick_product_translation<'a>(
    translations: &'a [product_translation::Model],
    locale: &str,
    default_locale: &str,
) -> Option<&'a product_translation::Model> {
    translations
        .iter()
        .find(|translation| locale_tags_match(&translation.locale, locale))
        .or_else(|| {
            (!locale_tags_match(default_locale, locale)).then(|| {
                translations
                    .iter()
                    .find(|translation| locale_tags_match(&translation.locale, default_locale))
            })?
        })
        .or_else(|| translations.first())
}

pub(crate) fn pick_variant_translation<'a>(
    translations: &'a [variant_translation::Model],
    locale: &str,
    default_locale: &str,
) -> Option<&'a variant_translation::Model> {
    translations
        .iter()
        .find(|translation| locale_tags_match(&translation.locale, locale))
        .or_else(|| {
            (!locale_tags_match(default_locale, locale)).then(|| {
                translations
                    .iter()
                    .find(|translation| locale_tags_match(&translation.locale, default_locale))
            })?
        })
        .or_else(|| translations.first())
}

pub(crate) fn default_metadata() -> Value {
    json!({})
}

pub(crate) fn deserialize_patch_field<'de, D, T>(
    deserializer: D,
) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Some(Option::<T>::deserialize(deserializer)?))
}

pub(crate) fn merge_metadata(current: Value, patch: Value) -> Value {
    match (current, patch) {
        (Value::Object(mut current), Value::Object(patch)) => {
            for (key, value) in patch {
                current.insert(key, value);
            }
            Value::Object(current)
        }
        (_, patch) => patch,
    }
}

pub(crate) fn normalize_store_seller_id(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_owned())
}

pub(crate) fn seller_snapshot_metadata(seller_id: Option<&str>) -> Value {
    let seller_id = normalize_store_seller_id(seller_id);

    json!({
        "seller": {
            "id": seller_id,
        }
    })
}

pub(crate) fn cart_context_metadata(cart: &CartResponse, context: &StoreContextResponse) -> Value {
    json!({
        "cart_context": {
            "channel_id": cart.channel_id,
            "channel_slug": cart.channel_slug.clone(),
            "region_id": context.region.as_ref().map(|region| region.id),
            "country_code": cart.country_code.clone(),
            "locale": context.locale.clone(),
            "currency_code": cart.currency_code.clone(),
            "selected_shipping_option_id": cart.selected_shipping_option_id,
            "shipping_selections": current_shipping_selections(cart),
            "customer_id": cart.customer_id,
            "email": cart.email.clone(),
        }
    })
}

#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
pub struct StoreListProductsParams {
    #[serde(flatten)]
    pub pagination: Option<PaginationParams>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub search: Option<String>,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema, Default)]
pub struct StoreOrderReturnsParams {
    #[serde(flatten)]
    pub pagination: PaginationParams,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema, Default)]
pub struct StoreOrderRefundsParams {
    #[serde(flatten)]
    pub pagination: PaginationParams,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema, Default)]
pub struct StoreOrderChangesParams {
    #[serde(flatten)]
    pub pagination: PaginationParams,
    pub status: Option<String>,
    pub change_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema, Default)]
pub struct StoreContextQuery {
    pub cart_id: Option<Uuid>,
    pub region_id: Option<Uuid>,
    pub country_code: Option<String>,
    pub locale: Option<String>,
    pub currency_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct StoreCreateCartInput {
    pub email: Option<String>,
    pub currency_code: Option<String>,
    pub region_id: Option<Uuid>,
    pub country_code: Option<String>,
    pub locale: Option<String>,
    #[serde(default = "default_metadata")]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct StoreCartResponse {
    pub cart: CartResponse,
    pub context: StoreContextResponse,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct StoreUpdateCartInput {
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub email: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub region_id: Option<Option<Uuid>>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub country_code: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub locale: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_patch_field")]
    pub selected_shipping_option_id: Option<Option<Uuid>>,
    #[serde(default)]
    pub shipping_selections: Option<Vec<StoreCartShippingSelectionInput>>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct StoreCreatePaymentCollectionInput {
    pub cart_id: Uuid,
    #[serde(default = "default_metadata")]
    pub metadata: Value,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct StoreCompleteCartInput {
    pub shipping_option_id: Option<Uuid>,
    pub shipping_selections: Option<Vec<StoreCartShippingSelectionInput>>,
    pub region_id: Option<Uuid>,
    pub country_code: Option<String>,
    pub locale: Option<String>,
    #[serde(default = "default_true")]
    pub create_fulfillment: bool,
    #[serde(default = "default_metadata")]
    pub metadata: Value,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct StoreAddCartLineItemInput {
    pub variant_id: Uuid,
    pub quantity: i32,
    #[serde(default = "default_metadata")]
    pub metadata: Value,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct StoreUpdateCartLineItemInput {
    pub quantity: i32,
}

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone)]
pub(crate) struct StoreCartContextPatch {
    pub(crate) email: Option<Option<String>>,
    pub(crate) region_id: Option<Option<Uuid>>,
    pub(crate) country_code: Option<Option<String>>,
    pub(crate) locale: Option<Option<String>>,
    pub(crate) selected_shipping_option_id: Option<Option<Uuid>>,
    pub(crate) shipping_selections: Option<Vec<crate::dto::CartShippingSelectionInput>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RequestedCartContext {
    pub(crate) email: Option<String>,
    pub(crate) region_id: Option<Uuid>,
    pub(crate) country_code: Option<String>,
    pub(crate) locale: Option<String>,
    pub(crate) selected_shipping_option_id: Option<Uuid>,
    pub(crate) shipping_selections: Vec<crate::dto::CartShippingSelectionInput>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct StoreCartShippingSelectionInput {
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
    pub selected_shipping_option_id: Option<Uuid>,
}

impl From<StoreCartShippingSelectionInput> for crate::dto::CartShippingSelectionInput {
    fn from(value: StoreCartShippingSelectionInput) -> Self {
        Self {
            shipping_profile_slug: value.shipping_profile_slug,
            seller_id: value.seller_id,
            seller_scope: None,
            selected_shipping_option_id: value.selected_shipping_option_id,
        }
    }
}
