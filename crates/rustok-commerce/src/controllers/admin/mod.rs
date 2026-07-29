pub mod changes;
pub mod fulfillments;
pub mod orders;
pub mod payments;
pub mod post_order_reads;
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
use rustok_order::error::OrderError;
use rustok_payment::PaymentError;
use rustok_web::HttpError;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    PostOrderOrchestrationError,
    dto::{FulfillmentResponse, OrderResponse, PaymentCollectionResponse},
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
            axum::routing::get(post_order_reads::list_order_changes),
        )
        .route(
            "/order-changes/{id}",
            axum::routing::get(post_order_reads::show_order_change),
        )
        .route(
            "/order-changes/{id}/apply",
            axum::routing::post(changes::apply_order_change),
        )
        .route(
            "/order-changes/{id}/cancel",
            axum::routing::post(changes::cancel_order_change),
        )
        .route(
            "/returns",
            axum::routing::get(post_order_reads::list_order_returns),
        )
        .route(
            "/returns/{id}",
            axum::routing::get(post_order_reads::show_order_return),
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
            "/inventory",
            axum::routing::get(products::list_inventory_items),
        )
        .route(
            "/inventory/{id}",
            axum::routing::get(products::show_inventory_item),
        )
        .route(
            "/inventory/{id}/adjust",
            axum::routing::post(products::adjust_inventory),
        )
        .route(
            "/fulfillments",
            axum::routing::get(fulfillments::list_fulfillments),
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
            "/fulfillments/{id}/cancel",
            axum::routing::post(fulfillments::cancel_fulfillment),
        )
        .route(
            "/shipping-options",
            axum::routing::get(shipping::list_shipping_options)
                .post(shipping::create_shipping_option),
        )
        .route(
            "/shipping-options/{id}",
            axum::routing::get(shipping::show_shipping_option)
                .post(shipping::update_shipping_option)
                .delete(shipping::delete_shipping_option),
        )
        .route(
            "/shipping-profiles",
            axum::routing::get(shipping::list_shipping_profiles)
                .post(shipping::create_shipping_profile),
        )
        .route(
            "/shipping-profiles/{id}",
            axum::routing::get(shipping::show_shipping_profile)
                .post(shipping::update_shipping_profile)
                .delete(shipping::delete_shipping_profile),
        )
}

#[allow(dead_code)]
fn map_order_error(error: OrderError) -> HttpError {
    match error {
        OrderError::Validation(message) => HttpError::bad_request("commerce_validation", message),
        OrderError::OrderNotFound(_)
        | OrderError::OrderReturnNotFound(_)
        | OrderError::OrderChangeNotFound(_) => {
            HttpError::not_found("commerce_admin_not_found", "Commerce resource not found")
        }
        OrderError::InvalidTransition { .. } => HttpError::bad_request(
            "commerce_order_invalid_transition",
            "Order state transition is invalid",
        ),
        OrderError::Database(error) => HttpError::internal(
            "commerce_admin_order_storage_error",
            format!("Order storage operation failed: {error}"),
        ),
        OrderError::Core(error) => HttpError::internal(
            "commerce_admin_order_core_error",
            format!("Order operation failed: {error}"),
        ),
    }
}

#[allow(dead_code)]
fn map_payment_error(error: PaymentError) -> HttpError {
    match error {
        PaymentError::Validation(message) => {
            HttpError::bad_request("commerce_validation", message)
        }
        PaymentError::PaymentCollectionNotFound(_)
        | PaymentError::PaymentNotFound(_)
        | PaymentError::RefundNotFound(_) => {
            HttpError::not_found("commerce_admin_not_found", "Commerce resource not found")
        }
        PaymentError::InvalidTransition { from, to } => HttpError::bad_request(
            "commerce_payment_invalid_transition",
            format!("Payment transition {from} -> {to} is invalid"),
        ),
        PaymentError::ProviderRejected { message, .. }
        | PaymentError::ProviderUnavailable { message, .. }
        | PaymentError::ProviderInvalidResponse { message, .. }
        | PaymentError::ProviderOutcomeUnknown { message, .. }
        | PaymentError::ProviderConfiguration { message, .. } => {
            HttpError::bad_request("commerce_payment_provider_error", message)
        }
        PaymentError::Database(error) => HttpError::internal(
            "commerce_admin_payment_storage_error",
            format!("Payment storage operation failed: {error}"),
        ),
    }
}

#[allow(dead_code)]
fn map_post_order_error(error: PostOrderOrchestrationError) -> HttpError {
    match error {
        PostOrderOrchestrationError::Validation(message) => {
            HttpError::bad_request("commerce_validation", message)
        }
        PostOrderOrchestrationError::Order(error) => map_order_error(error),
        PostOrderOrchestrationError::Payment(error) => map_payment_error(error),
        PostOrderOrchestrationError::Fulfillment(error) => match error {
            crate::error::FulfillmentError::FulfillmentNotFound(_) => {
                HttpError::not_found("commerce_admin_not_found", "Commerce resource not found")
            }
            crate::error::FulfillmentError::InvalidTransition { from, to } => {
                HttpError::bad_request(
                    "commerce_fulfillment_invalid_transition",
                    format!("Fulfillment transition {from} -> {to} is invalid"),
                )
            }
            crate::error::FulfillmentError::Validation(message) => {
                HttpError::bad_request("commerce_validation", message)
            }
            crate::error::FulfillmentError::Database(error) => HttpError::internal(
                "commerce_admin_fulfillment_storage_error",
                format!("Fulfillment storage operation failed: {error}"),
            ),
        },
        PostOrderOrchestrationError::Infrastructure(message) => {
            HttpError::internal("commerce_post_order_infrastructure", message)
        }
        PostOrderOrchestrationError::OperationConflict(message) => {
            HttpError::conflict("commerce_post_order_operation_conflict", message)
        }
        PostOrderOrchestrationError::OperationNotFound(_) => HttpError::not_found(
            "commerce_post_order_operation_not_found",
            "Return completion operation not found",
        ),
        PostOrderOrchestrationError::ReconciliationRequired(message) => {
            HttpError::conflict("commerce_post_order_reconciliation_required", message)
        }
    }
}
