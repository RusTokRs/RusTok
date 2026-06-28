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

use loco_rs::{controller::Routes, Error, Result};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    dto::{FulfillmentResponse, OrderResponse, PaymentCollectionResponse},
    storefront_shipping::normalize_shipping_profile_slug,
    FulfillmentOrchestrationError, PaymentService, PostOrderOrchestrationError,
    ShippingProfileService,
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

pub fn routes() -> Routes {
    Routes::new()
        .add(
            "/products",
            axum::routing::get(products::list_products).post(products::create_product),
        )
        .add(
            "/products/{id}",
            axum::routing::get(products::show_product)
                .post(products::update_product)
                .delete(products::delete_product),
        )
        .add(
            "/products/{id}/publish",
            axum::routing::post(products::publish_product),
        )
        .add(
            "/products/{id}/unpublish",
            axum::routing::post(products::unpublish_product),
        )
        .add("/orders", axum::routing::get(orders::list_orders))
        .add("/orders/{id}", axum::routing::get(orders::show_order))
        .add(
            "/orders/{id}/mark-paid",
            axum::routing::post(orders::mark_order_paid),
        )
        .add("/orders/{id}/ship", axum::routing::post(orders::ship_order))
        .add(
            "/orders/{id}/deliver",
            axum::routing::post(orders::deliver_order),
        )
        .add(
            "/orders/{id}/cancel",
            axum::routing::post(orders::cancel_order),
        )
        .add(
            "/orders/{id}/returns",
            axum::routing::post(returns::create_order_return),
        )
        .add(
            "/orders/{id}/returns/decision",
            axum::routing::post(returns::create_order_return_decision),
        )
        .add(
            "/orders/{id}/changes",
            axum::routing::post(changes::create_order_change),
        )
        .add(
            "/order-changes",
            axum::routing::get(changes::list_order_changes),
        )
        .add(
            "/order-changes/{id}",
            axum::routing::get(changes::show_order_change),
        )
        .add(
            "/order-changes/{id}/apply",
            axum::routing::post(changes::apply_order_change),
        )
        .add(
            "/order-changes/{id}/cancel",
            axum::routing::post(changes::cancel_order_change),
        )
        .add("/returns", axum::routing::get(returns::list_order_returns))
        .add(
            "/returns/{id}",
            axum::routing::get(returns::show_order_return),
        )
        .add(
            "/returns/{id}/complete",
            axum::routing::post(returns::complete_order_return),
        )
        .add(
            "/returns/{id}/cancel",
            axum::routing::post(returns::cancel_order_return),
        )
        .add(
            "/payment-collections",
            axum::routing::get(payments::list_payment_collections),
        )
        .add(
            "/payment-collections/{id}",
            axum::routing::get(payments::show_payment_collection),
        )
        .add(
            "/payment-collections/{id}/authorize",
            axum::routing::post(payments::authorize_payment_collection),
        )
        .add(
            "/payment-collections/{id}/capture",
            axum::routing::post(payments::capture_payment_collection),
        )
        .add(
            "/payment-collections/{id}/cancel",
            axum::routing::post(payments::cancel_payment_collection),
        )
        .add(
            "/payment-collections/{id}/refunds",
            axum::routing::post(payments::create_refund),
        )
        .add("/refunds", axum::routing::get(payments::list_refunds))
        .add("/refunds/{id}", axum::routing::get(payments::show_refund))
        .add(
            "/refunds/{id}/complete",
            axum::routing::post(payments::complete_refund),
        )
        .add(
            "/refunds/{id}/cancel",
            axum::routing::post(payments::cancel_refund),
        )
        .add(
            "/shipping-profiles",
            axum::routing::get(shipping::list_shipping_profiles)
                .post(shipping::create_shipping_profile),
        )
        .add(
            "/shipping-profiles/{id}",
            axum::routing::get(shipping::show_shipping_profile)
                .post(shipping::update_shipping_profile),
        )
        .add(
            "/shipping-profiles/{id}/deactivate",
            axum::routing::post(shipping::deactivate_shipping_profile),
        )
        .add(
            "/shipping-profiles/{id}/reactivate",
            axum::routing::post(shipping::reactivate_shipping_profile),
        )
        .add(
            "/shipping-options",
            axum::routing::get(shipping::list_shipping_options)
                .post(shipping::create_shipping_option),
        )
        .add(
            "/shipping-options/{id}",
            axum::routing::get(shipping::show_shipping_option)
                .post(shipping::update_shipping_option),
        )
        .add(
            "/shipping-options/{id}/deactivate",
            axum::routing::post(shipping::deactivate_shipping_option),
        )
        .add(
            "/shipping-options/{id}/reactivate",
            axum::routing::post(shipping::reactivate_shipping_option),
        )
        .add(
            "/fulfillments",
            axum::routing::get(fulfillments::list_fulfillments)
                .post(fulfillments::create_fulfillment),
        )
        .add(
            "/fulfillments/{id}",
            axum::routing::get(fulfillments::show_fulfillment),
        )
        .add(
            "/fulfillments/{id}/ship",
            axum::routing::post(fulfillments::ship_fulfillment),
        )
        .add(
            "/fulfillments/{id}/deliver",
            axum::routing::post(fulfillments::deliver_fulfillment),
        )
        .add(
            "/fulfillments/{id}/reopen",
            axum::routing::post(fulfillments::reopen_fulfillment),
        )
        .add(
            "/fulfillments/{id}/reship",
            axum::routing::post(fulfillments::reship_fulfillment),
        )
        .add(
            "/fulfillments/{id}/cancel",
            axum::routing::post(fulfillments::cancel_fulfillment),
        )
}

pub(crate) fn map_payment_orchestration_error(error: crate::PaymentOrchestrationError) -> Error {
    match error {
        crate::PaymentOrchestrationError::Payment(error) => map_payment_error(error),
        crate::PaymentOrchestrationError::Provider(error) => Error::BadRequest(error.to_string()),
    }
}

pub(crate) fn map_payment_error(error: rustok_payment::error::PaymentError) -> Error {
    match error {
        rustok_payment::error::PaymentError::PaymentCollectionNotFound(_)
        | rustok_payment::error::PaymentError::RefundNotFound(_) => Error::NotFound,
        other => Error::BadRequest(other.to_string()),
    }
}

pub(crate) fn map_order_error(error: rustok_order::error::OrderError) -> Error {
    match error {
        rustok_order::error::OrderError::OrderNotFound(_)
        | rustok_order::error::OrderError::OrderReturnNotFound(_)
        | rustok_order::error::OrderError::OrderChangeNotFound(_) => Error::NotFound,
        other => Error::BadRequest(other.to_string()),
    }
}

pub(crate) fn map_fulfillment_error(error: rustok_fulfillment::error::FulfillmentError) -> Error {
    match error {
        rustok_fulfillment::error::FulfillmentError::FulfillmentNotFound(_) => Error::NotFound,
        other => Error::BadRequest(other.to_string()),
    }
}

pub(crate) fn map_fulfillment_orchestration_error(error: FulfillmentOrchestrationError) -> Error {
    match error {
        FulfillmentOrchestrationError::OrderNotFound(_) => Error::NotFound,
        other => Error::BadRequest(other.to_string()),
    }
}

pub(crate) fn map_post_order_orchestration_error(error: PostOrderOrchestrationError) -> Error {
    match error {
        PostOrderOrchestrationError::Order(
            rustok_order::error::OrderError::OrderNotFound(_)
            | rustok_order::error::OrderError::OrderReturnNotFound(_)
            | rustok_order::error::OrderError::OrderChangeNotFound(_),
        )
        | PostOrderOrchestrationError::Payment(
            rustok_payment::error::PaymentError::PaymentCollectionNotFound(_)
            | rustok_payment::error::PaymentError::RefundNotFound(_),
        ) => Error::NotFound,
        PostOrderOrchestrationError::Order(other) => Error::BadRequest(other.to_string()),
        PostOrderOrchestrationError::Payment(other) => Error::BadRequest(other.to_string()),
        PostOrderOrchestrationError::Validation(message) => Error::BadRequest(message),
    }
}

pub(crate) fn decision_requires_payments_update(action: &str, has_refund_payload: bool) -> bool {
    if has_refund_payload {
        return true;
    }

    action.trim().to_ascii_lowercase().replace('-', "_") == "refund"
}

pub(crate) fn map_shipping_profile_error(error: crate::CommerceError) -> Error {
    match error {
        crate::CommerceError::ShippingProfileNotFound(_) => Error::NotFound,
        other => Error::BadRequest(other.to_string()),
    }
}

pub(crate) async fn validate_product_shipping_profile_input(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    shipping_profile_slug: Option<&str>,
) -> Result<()> {
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
) -> Result<()> {
    let Some(slugs) = allowed_shipping_profile_slugs else {
        return Ok(());
    };

    ShippingProfileService::new(db.clone())
        .ensure_shipping_profile_slugs_exist(tenant_id, slugs.iter())
        .await
        .map_err(map_shipping_profile_error)?;

    Ok(())
}

pub(crate) async fn resolve_return_refund_collection_id(
    payment_service: &PaymentService,
    tenant_id: Uuid,
    order_id: Uuid,
    explicit_collection_id: Option<Uuid>,
) -> Result<Uuid> {
    if let Some(collection_id) = explicit_collection_id {
        let collection = payment_service
            .get_collection(tenant_id, collection_id)
            .await
            .map_err(map_payment_error)?;
        if collection.order_id != Some(order_id) {
            return Err(Error::BadRequest(format!(
                "payment collection {collection_id} is not attached to order {order_id}"
            )));
        }
        return Ok(collection_id);
    }

    payment_service
        .find_latest_collection_by_order(tenant_id, order_id)
        .await
        .map_err(map_payment_error)?
        .map(|collection| collection.id)
        .ok_or_else(|| {
            Error::BadRequest(format!(
                "order {order_id} has no payment collection for return refund"
            ))
        })
}
