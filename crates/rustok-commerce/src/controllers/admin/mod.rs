pub mod changes;
pub mod fulfillments;
pub mod orders;
pub mod payments;
pub mod products;
pub mod returns;
pub mod shipping;

pub use changes::*;
pub use fulfillments::*;
pub use orders::*;
pub use payments::*;
pub use products::*;
pub use returns::*;
pub use shipping::*;

#[cfg(test)]
mod tests;

use rust_decimal::Decimal;
use rustok_payment::PaymentError;
use rustok_web::{HttpError, HttpResult};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    FulfillmentOrchestrationError, PostOrderOrchestrationError, ShippingProfileService,
    dto::{FulfillmentResponse, OrderResponse, PaymentCollectionResponse},
    storefront_shipping::normalize_shipping_profile_slug,
};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AdminOrderDetailResponse {
    pub order: OrderResponse,
    pub payment_collection: Option<PaymentCollectionResponse>,
    pub fulfillment: Option<FulfillmentResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CompleteOrderReturnRefundInput {
    pub payment_collection_id: Option<Uuid>,
    pub amount: Decimal,
    pub reason: Option<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default)]
    pub complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CompleteOrderReturnExchangeInput {
    pub description: Option<String>,
    pub preview: serde_json::Value,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CompleteOrderReturnClaimInput {
    pub description: Option<String>,
    pub preview: serde_json::Value,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AdminCompleteOrderReturnInput {
    pub resolution_type: Option<String>,
    pub refund_id: Option<Uuid>,
    pub order_change_id: Option<Uuid>,
    pub refund: Option<CompleteOrderReturnRefundInput>,
    pub exchange: Option<CompleteOrderReturnExchangeInput>,
    pub claim: Option<CompleteOrderReturnClaimInput>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct ListOrdersParams {
    #[serde(flatten)]
    pub pagination: Option<super::common::PaginationParams>,
    pub status: Option<String>,
    pub customer_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct ListPaymentCollectionsParams {
    #[serde(flatten)]
    pub pagination: Option<super::common::PaginationParams>,
    pub status: Option<String>,
    pub order_id: Option<Uuid>,
    pub cart_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct ListFulfillmentsParams {
    #[serde(flatten)]
    pub pagination: Option<super::common::PaginationParams>,
    pub status: Option<String>,
    pub order_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct ListRefundsParams {
    #[serde(flatten)]
    pub pagination: Option<super::common::PaginationParams>,
    pub payment_collection_id: Option<Uuid>,
    pub order_id: Option<Uuid>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct ListOrderReturnsParams {
    #[serde(flatten)]
    pub pagination: Option<super::common::PaginationParams>,
    pub order_id: Option<Uuid>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct ListOrderChangesParams {
    #[serde(flatten)]
    pub pagination: Option<super::common::PaginationParams>,
    pub order_id: Option<Uuid>,
    pub status: Option<String>,
    pub change_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct ListShippingOptionsParams {
    #[serde(flatten)]
    pub pagination: Option<super::common::PaginationParams>,
    pub currency_code: Option<String>,
    pub provider_id: Option<String>,
    pub search: Option<String>,
    pub active: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct ListShippingProfilesParams {
    #[serde(flatten)]
    pub pagination: Option<super::common::PaginationParams>,
    pub search: Option<String>,
    pub active: Option<bool>,
}

pub fn axum_router() -> axum::Router<super::CommerceHttpRuntime> {
    axum::Router::new()
        .route(
            "/products",
            axum::routing::get(products::list_products).post(products::create_product),
        )
        .route(
            "/products/{id}",
            axum::routing::get(products::show_product)
                .post(products::update_product)
                .delete(products::delete_product),
        )
        .route(
            "/products/{id}/publish",
            axum::routing::post(products::publish_product),
        )
        .route(
            "/products/{id}/unpublish",
            axum::routing::post(products::unpublish_product),
        )
        .route("/orders", axum::routing::get(orders::list_orders))
        .route("/orders/{id}", axum::routing::get(orders::show_order))
        .route(
            "/orders/{id}/mark-paid",
            axum::routing::post(orders::mark_order_paid),
        )
        .route("/orders/{id}/ship", axum::routing::post(orders::ship_order))
        .route(
            "/orders/{id}/deliver",
            axum::routing::post(orders::deliver_order),
        )
        .route(
            "/orders/{id}/cancel",
            axum::routing::post(orders::cancel_order),
        )
        .route(
            "/orders/{id}/returns",
            axum::routing::post(returns::create_order_return),
        )
        .route(
            "/orders/{id}/returns/decision",
            axum::routing::post(returns::create_order_return_decision),
        )
        .route(
            "/orders/{id}/changes",
            axum::routing::post(changes::create_order_change),
        )
        .route(
            "/order-changes",
            axum::routing::get(changes::list_order_changes),
        )
        .route(
            "/order-changes/{id}",
            axum::routing::get(changes::show_order_change),
        )
        .route(
            "/order-changes/{id}/apply",
            axum::routing::post(changes::apply_order_change),
        )
        .route(
            "/order-changes/{id}/cancel",
            axum::routing::post(changes::cancel_order_change),
        )
        .route("/returns", axum::routing::get(returns::list_order_returns))
        .route(
            "/returns/{id}",
            axum::routing::get(returns::show_order_return),
        )
        .route(
            "/returns/{id}/complete",
            axum::routing::post(returns::complete_order_return),
        )
        .route(
            "/returns/{id}/cancel",
            axum::routing::post(returns::cancel_order_return),
        )
        .route(
            "/payment-collections",
            axum::routing::get(payments::list_payment_collections),
        )
        .route(
            "/payment-collections/{id}",
            axum::routing::get(payments::show_payment_collection),
        )
        .route(
            "/payment-collections/{id}/authorize",
            axum::routing::post(payments::authorize_payment_collection),
        )
        .route(
            "/payment-collections/{id}/capture",
            axum::routing::post(payments::capture_payment_collection),
        )
        .route(
            "/payment-collections/{id}/cancel",
            axum::routing::post(payments::cancel_payment_collection),
        )
        .route(
            "/payment-collections/{id}/refunds",
            axum::routing::post(payments::create_refund),
        )
        .route("/refunds", axum::routing::get(payments::list_refunds))
        .route("/refunds/{id}", axum::routing::get(payments::show_refund))
        .route(
            "/refunds/{id}/complete",
            axum::routing::post(payments::complete_refund),
        )
        .route(
            "/refunds/{id}/cancel",
            axum::routing::post(payments::cancel_refund),
        )
        .route(
            "/shipping-profiles",
            axum::routing::get(shipping::list_shipping_profiles)
                .post(shipping::create_shipping_profile),
        )
        .route(
            "/shipping-profiles/{id}",
            axum::routing::get(shipping::show_shipping_profile)
                .post(shipping::update_shipping_profile),
        )
        .route(
            "/shipping-profiles/{id}/deactivate",
            axum::routing::post(shipping::deactivate_shipping_profile),
        )
        .route(
            "/shipping-profiles/{id}/reactivate",
            axum::routing::post(shipping::reactivate_shipping_profile),
        )
        .route(
            "/shipping-options",
            axum::routing::get(shipping::list_shipping_options)
                .post(shipping::create_shipping_option),
        )
        .route(
            "/shipping-options/{id}",
            axum::routing::get(shipping::show_shipping_option)
                .post(shipping::update_shipping_option),
        )
        .route(
            "/shipping-options/{id}/deactivate",
            axum::routing::post(shipping::deactivate_shipping_option),
        )
        .route(
            "/shipping-options/{id}/reactivate",
            axum::routing::post(shipping::reactivate_shipping_option),
        )
        .route(
            "/fulfillments",
            axum::routing::get(fulfillments::list_fulfillments)
                .post(fulfillments::create_fulfillment),
        )
        .route(
            "/fulfillments/{id}",
            axum::routing::get(fulfillments::show_fulfillment),
        )
        .route(
            "/fulfillments/{id}/ship",
            axum::routing::post(fulfillments::ship_fulfillment),
        )
        .route(
            "/fulfillments/{id}/deliver",
            axum::routing::post(fulfillments::deliver_fulfillment),
        )
        .route(
            "/fulfillments/{id}/reopen",
            axum::routing::post(fulfillments::reopen_fulfillment),
        )
        .route(
            "/fulfillments/{id}/reship",
            axum::routing::post(fulfillments::reship_fulfillment),
        )
        .route(
            "/fulfillments/{id}/cancel",
            axum::routing::post(fulfillments::cancel_fulfillment),
        )
}

pub(crate) fn map_payment_orchestration_error(
    error: crate::PaymentOrchestrationError,
) -> HttpError {
    match error {
        crate::PaymentOrchestrationError::Payment(error)
        | crate::PaymentOrchestrationError::Provider(error) => map_payment_error(error),
        crate::PaymentOrchestrationError::ProviderAfterRefundReservation { source, .. } => {
            map_reserved_refund_provider_error(source)
        }
    }
}

pub(crate) fn map_payment_error(error: PaymentError) -> HttpError {
    match error {
        PaymentError::PaymentCollectionNotFound(_)
        | PaymentError::PaymentNotFound(_)
        | PaymentError::RefundNotFound(_) => {
            HttpError::not_found("commerce_admin_not_found", "Commerce resource not found")
        }
        PaymentError::Validation(_) => HttpError::bad_request(
            "commerce_admin_payment_invalid",
            "Payment request is invalid",
        ),
        PaymentError::InvalidTransition { .. } | PaymentError::ProviderRejected { .. } => {
            HttpError::new(
                axum::http::StatusCode::CONFLICT,
                "commerce_admin_payment_state_conflict",
                "Payment operation conflicts with the current state",
            )
        }
        PaymentError::ProviderUnavailable { .. } => HttpError::new(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_payment_provider_unavailable",
            "Payment provider is temporarily unavailable",
        ),
        PaymentError::ProviderInvalidResponse { .. } => HttpError::new(
            axum::http::StatusCode::BAD_GATEWAY,
            "commerce_admin_payment_provider_invalid_response",
            "Payment provider returned an invalid response; reconciliation may be required",
        ),
        PaymentError::ProviderOutcomeUnknown { .. } => HttpError::new(
            axum::http::StatusCode::CONFLICT,
            "commerce_admin_payment_reconciliation_required",
            "Payment provider outcome is unknown and requires reconciliation",
        ),
        PaymentError::ProviderConfiguration { .. } => HttpError::new(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_payment_provider_not_configured",
            "Payment provider is not configured for this tenant",
        ),
        PaymentError::Database(_) => HttpError::new(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_payment_storage_unavailable",
            "Payment storage is temporarily unavailable",
        ),
    }
}

fn map_reserved_refund_provider_error(error: PaymentError) -> HttpError {
    match error {
        PaymentError::ProviderOutcomeUnknown { .. }
        | PaymentError::ProviderInvalidResponse { .. } => HttpError::new(
            axum::http::StatusCode::CONFLICT,
            "commerce_admin_refund_reconciliation_required",
            "Refund remains reserved while the provider outcome is reconciled",
        ),
        PaymentError::ProviderUnavailable { .. } => HttpError::new(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "commerce_admin_refund_provider_unavailable",
            "Refund remains reserved and the provider operation may be retried safely",
        ),
        other => map_payment_error(other),
    }
}

pub(crate) fn map_order_error(error: rustok_order::error::OrderError) -> HttpError {
    match error {
        rustok_order::error::OrderError::OrderNotFound(_)
        | rustok_order::error::OrderError::OrderReturnNotFound(_)
        | rustok_order::error::OrderError::OrderChangeNotFound(_) => {
            HttpError::not_found("commerce_admin_not_found", "Commerce resource not found")
        }
        other => HttpError::bad_request("commerce_admin_invalid", other.to_string()),
    }
}

pub(crate) fn map_fulfillment_error(
    error: rustok_fulfillment::error::FulfillmentError,
) -> HttpError {
    match error {
        rustok_fulfillment::error::FulfillmentError::FulfillmentNotFound(_) => {
            HttpError::not_found("commerce_admin_not_found", "Commerce resource not found")
        }
        other => HttpError::bad_request("commerce_admin_invalid", other.to_string()),
    }
}

pub(crate) fn map_fulfillment_orchestration_error(
    error: FulfillmentOrchestrationError,
) -> HttpError {
    match error {
        FulfillmentOrchestrationError::OrderNotFound(_) => {
            HttpError::not_found("commerce_admin_not_found", "Commerce resource not found")
        }
        other => HttpError::bad_request("commerce_admin_invalid", other.to_string()),
    }
}

pub(crate) fn map_post_order_orchestration_error(error: PostOrderOrchestrationError) -> HttpError {
    match error {
        PostOrderOrchestrationError::Order(
            rustok_order::error::OrderError::OrderNotFound(_)
            | rustok_order::error::OrderError::OrderReturnNotFound(_)
            | rustok_order::error::OrderError::OrderChangeNotFound(_),
        )
        | PostOrderOrchestrationError::Payment(
            PaymentError::PaymentCollectionNotFound(_)
            | PaymentError::PaymentNotFound(_)
            | PaymentError::RefundNotFound(_),
        ) => HttpError::not_found("commerce_admin_not_found", "Commerce resource not found"),
        PostOrderOrchestrationError::Order(other) => {
            HttpError::bad_request("commerce_admin_invalid", other.to_string())
        }
        PostOrderOrchestrationError::Payment(error) => map_payment_error(error),
        PostOrderOrchestrationError::PaymentOrchestration(error) => {
            map_payment_orchestration_error(error)
        }
        PostOrderOrchestrationError::Validation(_) => {
            HttpError::bad_request("commerce_admin_invalid", "Post-order request is invalid")
        }
    }
}

pub(crate) fn decision_requires_payments_update(action: &str, has_refund_payload: bool) -> bool {
    if has_refund_payload {
        return true;
    }

    action.trim().to_ascii_lowercase().replace('-', "_") == "refund"
}

pub(crate) fn map_shipping_profile_error(error: crate::CommerceError) -> HttpError {
    match error {
        crate::CommerceError::ShippingProfileNotFound(_) => {
            HttpError::not_found("commerce_admin_not_found", "Commerce resource not found")
        }
        other => HttpError::bad_request("commerce_admin_invalid", other.to_string()),
    }
}

pub(crate) async fn validate_product_shipping_profile_input(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    shipping_profile_slug: Option<&str>,
) -> HttpResult<()> {
    let Some(slug) = shipping_profile_slug.and_then(normalize_shipping_profile_slug) else {
        return Ok(());
    };

    ShippingProfileService::new(db.clone())
        .ensure_shipping_profile_slug_exists(tenant_id, &slug)
        .await
        .map_err(map_shipping_profile_error)?;

    Ok(())
}

pub(crate) async fn validate_shipping_option_profile_inputs(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    allowed_shipping_profile_slugs: Option<&Vec<String>>,
) -> HttpResult<()> {
    let Some(slugs) = allowed_shipping_profile_slugs else {
        return Ok(());
    };

    ShippingProfileService::new(db.clone())
        .ensure_shipping_profile_slugs_exist(tenant_id, slugs.iter())
        .await
        .map_err(map_shipping_profile_error)?;

    Ok(())
}
