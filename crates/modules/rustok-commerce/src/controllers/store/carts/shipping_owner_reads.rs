use rustok_api::{AuthContext, PortContext, PortError, PortErrorKind, RequestContext};
use rustok_cart::{CartStorefrontContextUpdateRequest, in_process_cart_storefront_port};
use rustok_fulfillment::{
    ListShippingOptionProjectionsRequest, ReadShippingOptionProjectionRequest,
    ShippingOptionReadPort,
};
use rustok_web::{HttpError, HttpResult};
use sea_orm::DatabaseConnection;
use std::collections::BTreeSet;
use uuid::Uuid;

use crate::{
    dto::{CartResponse, UpdateCartContextInput},
    storefront_channel::is_metadata_visible_for_public_channel,
    storefront_shipping::{
        enrich_cart_delivery_groups_from_options, is_shipping_option_compatible_with_profiles,
        normalize_shipping_profile_slug,
    },
};

use super::super::{StoreCartContextPatch, StoreCartResponse};

pub(super) struct SelectedShippingOptionValidation<'a> {
    pub(super) selected_shipping_option_id: Option<Uuid>,
    pub(super) shipping_selections: Option<&'a [crate::dto::CartShippingSelectionInput]>,
    pub(super) currency_code: &'a str,
    pub(super) public_channel_slug: Option<&'a str>,
    pub(super) requested_locale: Option<&'a str>,
    pub(super) tenant_default_locale: Option<&'a str>,
}

const STOREFRONT_CART_SHIPPING_OWNER: &str = "rustok_fulfillment";
const STOREFRONT_CART_SHIPPING_BOUNDARY: &str = "commerce_storefront_cart_shipping_http";

fn shipping_read_context(
    tenant_id: Uuid,
    request_context: &RequestContext,
    auth: Option<&AuthContext>,
    cart_id: Uuid,
    operation: &str,
) -> PortContext {
    super::super::storefront_cart_port_context(
        tenant_id,
        request_context,
        auth,
        cart_id,
        operation,
        false,
    )
}

fn map_shipping_port_error(
    error: PortError,
    context: &PortContext,
    operation: &'static str,
    tenant_id: Uuid,
    cart_id: Uuid,
) -> HttpError {
    let (status, code, message, error_kind) = match &error.kind {
        PortErrorKind::Validation => (
            axum::http::StatusCode::BAD_REQUEST,
            "commerce_store_shipping_invalid",
            "Shipping request is invalid",
            "validation",
        ),
        PortErrorKind::NotFound => (
            axum::http::StatusCode::NOT_FOUND,
            "commerce_store_not_found",
            "Commerce resource not found",
            "not_found",
        ),
        PortErrorKind::Conflict => (
            axum::http::StatusCode::CONFLICT,
            "commerce_store_shipping_state_conflict",
            "Shipping operation conflicts with the current state",
            "state_conflict",
        ),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "commerce_store_shipping_unavailable",
            "Shipping service is temporarily unavailable",
            "unavailable",
        ),
        PortErrorKind::Forbidden | PortErrorKind::InvariantViolation => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "commerce_store_shipping_failed",
            "Shipping operation could not be completed safely",
            "owner_failure",
        ),
    };

    tracing::error!(
        owner = STOREFRONT_CART_SHIPPING_OWNER,
        operation,
        correlation_id = %context.correlation_id,
        tenant_id_non_nil = !tenant_id.is_nil(),
        cart_id_non_nil = !cart_id.is_nil(),
        owner_error_kind = ?error.kind,
        owner_code_length = error.code.chars().count(),
        retryable = error.retryable,
        error_kind,
        public_code = code,
        status = status.as_u16(),
        boundary = STOREFRONT_CART_SHIPPING_BOUNDARY,
        "storefront cart shipping owner read failed with bounded diagnostics"
    );

    HttpError::new(status, code, message)
}

pub(super) async fn enrich_storefront_cart(
    shipping_option_read_port: &dyn ShippingOptionReadPort,
    tenant_id: Uuid,
    request_context: &RequestContext,
    auth: Option<&AuthContext>,
    tenant_default_locale: &str,
    cart: CartResponse,
) -> HttpResult<CartResponse> {
    let public_channel_slug =
        super::super::storefront_public_channel_slug_for_cart(&cart, request_context);
    let cart_id = cart.id;
    let context = shipping_read_context(
        tenant_id,
        request_context,
        auth,
        cart_id,
        "shipping-options-list",
    );
    let options = shipping_option_read_port
        .list_shipping_option_projections(
            context.clone(),
            ListShippingOptionProjectionsRequest {
                requested_locale: Some(request_context.locale.clone()),
                tenant_default_locale: Some(tenant_default_locale.to_string()),
            },
        )
        .await
        .map_err(|error| {
            map_shipping_port_error(
                error,
                &context,
                "list_shipping_option_projections",
                tenant_id,
                cart_id,
            )
        })?;

    let mut cart =
        enrich_cart_delivery_groups_from_options(cart, options, public_channel_slug.as_deref());

    if cart.delivery_groups.len() == 1 {
        let is_compatible = |opt_id: Uuid| {
            cart.delivery_groups[0]
                .available_shipping_options
                .iter()
                .any(|opt| opt.id == opt_id)
        };
        let selected_id = cart.delivery_groups[0]
            .selected_shipping_option_id
            .filter(|id| is_compatible(*id))
            .or_else(|| {
                cart.selected_shipping_option_id
                    .filter(|id| is_compatible(*id))
            });

        cart.delivery_groups[0].selected_shipping_option_id = selected_id;
        cart.selected_shipping_option_id = selected_id;
    }

    Ok(cart)
}

async fn validate_selected_shipping_option(
    shipping_option_read_port: &dyn ShippingOptionReadPort,
    tenant_id: Uuid,
    request_context: &RequestContext,
    auth: Option<&AuthContext>,
    cart: &CartResponse,
    validation: SelectedShippingOptionValidation<'_>,
) -> HttpResult<()> {
    let selections = if let Some(shipping_selections) = validation.shipping_selections {
        shipping_selections.to_vec()
    } else if let Some(selected_shipping_option_id) = validation.selected_shipping_option_id {
        if cart.delivery_groups.len() > 1 {
            return Err(HttpError::bad_request(
                "commerce_store_invalid",
                "selected_shipping_option_id can only be used for carts with a single delivery group"
                    .to_string(),
            ));
        }
        cart.delivery_groups
            .first()
            .map(|group| {
                vec![crate::dto::CartShippingSelectionInput {
                    shipping_profile_slug: group.shipping_profile_slug.clone(),
                    seller_id: group.seller_id.clone(),
                    seller_scope: None,
                    selected_shipping_option_id: Some(selected_shipping_option_id),
                }]
            })
            .unwrap_or_default()
    } else {
        super::super::current_shipping_selections(cart)
    };

    for selection in selections {
        let Some(selected_shipping_option_id) = selection.selected_shipping_option_id else {
            continue;
        };
        let required_shipping_profiles = BTreeSet::from([normalize_shipping_profile_slug(
            selection.shipping_profile_slug.as_str(),
        )
        .unwrap_or_else(|| "default".to_string())]);
        let context = shipping_read_context(
            tenant_id,
            request_context,
            auth,
            cart.id,
            "shipping-option-read",
        );
        let option = shipping_option_read_port
            .read_shipping_option_projection(
                context.clone(),
                ReadShippingOptionProjectionRequest {
                    shipping_option_id: selected_shipping_option_id,
                    requested_locale: validation.requested_locale.map(str::to_string),
                    tenant_default_locale: validation.tenant_default_locale.map(str::to_string),
                },
            )
            .await
            .map_err(|error| {
                map_shipping_port_error(
                    error,
                    &context,
                    "read_shipping_option_projection",
                    tenant_id,
                    cart.id,
                )
            })?;
        if !option
            .currency_code
            .eq_ignore_ascii_case(validation.currency_code)
        {
            return Err(HttpError::bad_request(
                "commerce_store_invalid",
                format!(
                    "Shipping option {} uses currency {}, expected {}",
                    option.id, option.currency_code, validation.currency_code
                ),
            ));
        }
        if !is_metadata_visible_for_public_channel(&option.metadata, validation.public_channel_slug)
        {
            return Err(HttpError::bad_request(
                "commerce_store_invalid",
                format!(
                    "Shipping option {} is not available for the current channel",
                    option.id
                ),
            ));
        }
        if !is_shipping_option_compatible_with_profiles(&option, &required_shipping_profiles) {
            return Err(HttpError::bad_request(
                "commerce_store_invalid",
                format!(
                    "Shipping option {} is not compatible with shipping profile {}",
                    option.id, selection.shipping_profile_slug
                ),
            ));
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn apply_cart_context_patch(
    db: &DatabaseConnection,
    event_bus: rustok_outbox::TransactionalEventBus,
    shipping_option_read_port: &dyn ShippingOptionReadPort,
    tenant_id: Uuid,
    request_context: &RequestContext,
    auth: Option<&AuthContext>,
    tenant_default_locale: &str,
    cart: &CartResponse,
    patch: StoreCartContextPatch,
) -> HttpResult<StoreCartResponse> {
    let requested = super::super::requested_cart_context(cart, request_context, patch);

    let context = super::super::resolve_context_for_db(
        db,
        tenant_id,
        request_context,
        requested.region_id,
        requested.country_code.clone(),
        requested.locale,
        Some(cart.currency_code.clone()),
    )
    .await?;

    let public_channel_slug =
        super::super::storefront_public_channel_slug_for_cart(cart, request_context);
    validate_selected_shipping_option(
        shipping_option_read_port,
        tenant_id,
        request_context,
        auth,
        cart,
        SelectedShippingOptionValidation {
            selected_shipping_option_id: requested.selected_shipping_option_id,
            shipping_selections: Some(requested.shipping_selections.as_slice()),
            currency_code: &cart.currency_code,
            public_channel_slug: public_channel_slug.as_deref(),
            requested_locale: Some(request_context.locale.as_str()),
            tenant_default_locale: Some(tenant_default_locale),
        },
    )
    .await?;

    let storefront_port = in_process_cart_storefront_port(db.clone());
    let updated_cart = storefront_port
        .update_storefront_context(
            super::super::storefront_cart_port_context(
                tenant_id,
                request_context,
                None,
                cart.id,
                "update-context",
                true,
            ),
            CartStorefrontContextUpdateRequest {
                cart_id: cart.id,
                input: UpdateCartContextInput {
                    email: requested.email,
                    region_id: context.region.as_ref().map(|region| region.id),
                    country_code: context
                        .region
                        .as_ref()
                        .and_then(|region| region.countries.first().cloned())
                        .or(requested.country_code),
                    locale_code: Some(context.locale.clone()),
                    selected_shipping_option_id: requested.selected_shipping_option_id,
                    shipping_selections: Some(requested.shipping_selections.clone()),
                },
            },
        )
        .await
        .map_err(rustok_web::port_error_to_http_error)?;
    let updated_cart = super::super::reprice_storefront_cart_line_items_for_db(
        db,
        event_bus,
        tenant_id,
        request_context,
        storefront_port.as_ref(),
        updated_cart,
    )
    .await?;
    let updated_cart = enrich_storefront_cart(
        shipping_option_read_port,
        tenant_id,
        request_context,
        auth,
        tenant_default_locale,
        updated_cart,
    )
    .await?;

    Ok(StoreCartResponse {
        cart: updated_cart,
        context,
    })
}
