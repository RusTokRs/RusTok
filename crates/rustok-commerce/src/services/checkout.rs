use thiserror::Error;
use tracing::instrument;
use uuid::Uuid;
use validator::Validate;

use rustok_cart::error::CartError;
use rustok_fulfillment::error::FulfillmentError;
use rustok_order::error::OrderError;
use rustok_outbox::TransactionalEventBus;
use rustok_payment::error::PaymentError;
use sea_orm::DatabaseConnection;

use crate::dto::{
    AuthorizePaymentInput, CancelPaymentInput, CompleteCheckoutInput, CompleteCheckoutResponse,
    CreateFulfillmentInput, CreateOrderInput, CreateOrderLineItemInput,
    CreatePaymentCollectionInput, ResolveStoreContextInput,
};
use crate::{CartService, FulfillmentService, OrderService, PaymentService, StoreContextService};

const MANUAL_PROVIDER_ID: &str = "manual";

#[derive(Debug, Error)]
pub enum CheckoutError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("cart {0} cannot be checked out in its current state")]
    CartNotReady(Uuid),
    #[error("cart {0} has no line items")]
    EmptyCart(Uuid),
    #[error("checkout failed at stage `{stage}`: {source}")]
    StageFailure {
        stage: &'static str,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

pub type CheckoutResult<T> = Result<T, CheckoutError>;

pub struct CheckoutService {
    cart_service: CartService,
    order_service: OrderService,
    payment_service: PaymentService,
    fulfillment_service: FulfillmentService,
    context_service: StoreContextService,
}

impl CheckoutService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            cart_service: CartService::new(db.clone()),
            order_service: OrderService::new(db.clone(), event_bus),
            payment_service: PaymentService::new(db.clone()),
            fulfillment_service: FulfillmentService::new(db.clone()),
            context_service: StoreContextService::new(db),
        }
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, actor_id = %actor_id))]
    pub async fn complete_checkout(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: CompleteCheckoutInput,
    ) -> CheckoutResult<CompleteCheckoutResponse> {
        input
            .validate()
            .map_err(|error| CheckoutError::Validation(error.to_string()))?;

        let cart = self
            .cart_service
            .get_cart(tenant_id, input.cart_id)
            .await
            .map_err(stage_error("load_cart"))?;
        if cart.status != "active" {
            return Err(CheckoutError::CartNotReady(cart.id));
        }
        if cart.line_items.is_empty() {
            return Err(CheckoutError::EmptyCart(cart.id));
        }
        let context = self
            .context_service
            .resolve_context(
                tenant_id,
                ResolveStoreContextInput {
                    region_id: input.region_id,
                    country_code: input.country_code.clone(),
                    locale: input.locale.clone(),
                    currency_code: Some(cart.currency_code.clone()),
                },
            )
            .await
            .map_err(stage_error("resolve_context"))?;

        let mut order = self
            .order_service
            .create_order(
                tenant_id,
                actor_id,
                CreateOrderInput {
                    customer_id: cart.customer_id,
                    currency_code: cart.currency_code.clone(),
                    line_items: cart
                        .line_items
                        .iter()
                        .map(|item| CreateOrderLineItemInput {
                            product_id: item.product_id,
                            variant_id: item.variant_id,
                            sku: item.sku.clone(),
                            title: item.title.clone(),
                            quantity: item.quantity,
                            unit_price: item.unit_price,
                            metadata: item.metadata.clone(),
                        })
                        .collect(),
                    metadata: input.metadata.clone(),
                },
            )
            .await
            .map_err(stage_error("create_order"))?;

        if let Err(error) = self
            .order_service
            .confirm_order(tenant_id, actor_id, order.id)
            .await
        {
            self.compensate_order(tenant_id, actor_id, order.id, "confirm_order_failed")
                .await;
            return Err(stage_error("confirm_order")(error));
        } else {
            order = self
                .order_service
                .get_order(tenant_id, order.id)
                .await
                .map_err(stage_error("reload_order"))?;
        }

        let payment_collection = match self
            .payment_service
            .create_collection(
                tenant_id,
                CreatePaymentCollectionInput {
                    cart_id: Some(cart.id),
                    order_id: Some(order.id),
                    customer_id: cart.customer_id,
                    currency_code: cart.currency_code.clone(),
                    amount: cart.total_amount,
                    metadata: input.metadata.clone(),
                },
            )
            .await
        {
            Ok(collection) => collection,
            Err(error) => {
                self.compensate_order(tenant_id, actor_id, order.id, "payment_collection_failed")
                    .await;
                return Err(stage_error("create_payment_collection")(error));
            }
        };

        let authorized_payment = match self
            .payment_service
            .authorize_collection(
                tenant_id,
                payment_collection.id,
                AuthorizePaymentInput {
                    provider_id: None,
                    provider_payment_id: None,
                    amount: Some(cart.total_amount),
                    metadata: input.metadata.clone(),
                },
            )
            .await
        {
            Ok(collection) => collection,
            Err(error) => {
                self.compensate_payment_and_order(
                    tenant_id,
                    actor_id,
                    payment_collection.id,
                    order.id,
                    "payment_authorization_failed",
                )
                .await;
                return Err(stage_error("authorize_payment")(error));
            }
        };

        let fulfillment = if input.create_fulfillment {
            match self
                .fulfillment_service
                .create_fulfillment(
                    tenant_id,
                    CreateFulfillmentInput {
                        order_id: order.id,
                        shipping_option_id: input.shipping_option_id,
                        customer_id: cart.customer_id,
                        carrier: None,
                        tracking_number: None,
                        metadata: input.metadata.clone(),
                    },
                )
                .await
            {
                Ok(fulfillment) => Some(fulfillment),
                Err(error) => {
                    self.compensate_payment_and_order(
                        tenant_id,
                        actor_id,
                        authorized_payment.id,
                        order.id,
                        "fulfillment_creation_failed",
                    )
                    .await;
                    return Err(stage_error("create_fulfillment")(error));
                }
            }
        } else {
            None
        };

        let captured_payment = match self
            .payment_service
            .capture_collection(
                tenant_id,
                authorized_payment.id,
                rustok_payment::dto::CapturePaymentInput {
                    amount: Some(cart.total_amount),
                    metadata: input.metadata.clone(),
                },
            )
            .await
        {
            Ok(collection) => collection,
            Err(error) => {
                self.compensate_payment_and_order(
                    tenant_id,
                    actor_id,
                    authorized_payment.id,
                    order.id,
                    "payment_capture_failed",
                )
                .await;
                return Err(stage_error("capture_payment")(error));
            }
        };
        let payment_reference = captured_payment
            .payments
            .last()
            .map(|payment| payment.provider_payment_id.clone())
            .unwrap_or_else(|| format!("manual_{}", order.id));
        let payment_method = captured_payment
            .provider_id
            .clone()
            .unwrap_or_else(|| MANUAL_PROVIDER_ID.to_string());

        let order = self
            .order_service
            .mark_paid(
                tenant_id,
                actor_id,
                order.id,
                payment_reference,
                payment_method,
            )
            .await
            .map_err(stage_error("mark_order_paid"))?;

        let cart = self
            .cart_service
            .complete_cart(tenant_id, cart.id)
            .await
            .map_err(stage_error("complete_cart"))?;

        Ok(CompleteCheckoutResponse {
            cart,
            order,
            payment_collection: captured_payment,
            fulfillment,
            context,
        })
    }

    async fn compensate_order(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        order_id: Uuid,
        reason: &str,
    ) {
        let _ = self
            .order_service
            .cancel_order(tenant_id, actor_id, order_id, Some(reason.to_string()))
            .await;
    }

    async fn compensate_payment_and_order(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        payment_collection_id: Uuid,
        order_id: Uuid,
        reason: &str,
    ) {
        let _ = self
            .payment_service
            .cancel_collection(
                tenant_id,
                payment_collection_id,
                CancelPaymentInput {
                    reason: Some(reason.to_string()),
                    metadata: serde_json::json!({ "compensated": true, "reason": reason }),
                },
            )
            .await;
        let _ = self
            .order_service
            .cancel_order(tenant_id, actor_id, order_id, Some(reason.to_string()))
            .await;
    }
}

fn stage_error<E>(stage: &'static str) -> impl FnOnce(E) -> CheckoutError
where
    E: std::error::Error + Send + Sync + 'static,
{
    move |source| CheckoutError::StageFailure {
        stage,
        source: Box::new(source),
    }
}

impl From<CartError> for CheckoutError {
    fn from(source: CartError) -> Self {
        stage_error("cart")(source)
    }
}

impl From<OrderError> for CheckoutError {
    fn from(source: OrderError) -> Self {
        stage_error("order")(source)
    }
}

impl From<PaymentError> for CheckoutError {
    fn from(source: PaymentError) -> Self {
        stage_error("payment")(source)
    }
}

impl From<FulfillmentError> for CheckoutError {
    fn from(source: FulfillmentError) -> Self {
        stage_error("fulfillment")(source)
    }
}
