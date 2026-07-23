use rustok_api::{PortActor, PortContext, PortError};
use rustok_cart::{
    CartCheckoutLifecycleRequest, CartCheckoutPort, CartCheckoutSnapshotRequest, CartResponse,
    CartStatus,
};
use rustok_order::OrderStatusKind;
use rustok_payment::PaymentCollectionStatusKind;
use std::{sync::Arc, time::Duration};
use thiserror::Error;
use uuid::Uuid;

use super::{
    CheckoutFulfillmentCreatedState, CheckoutOperationCheckpoint, CheckoutOperationError,
    CheckoutOperationJournal, CheckoutOperationStage, CheckoutOperationStatus,
    DEFAULT_CHECKOUT_LEASE_SECONDS,
};

const CART_PORT_DEADLINE_SECONDS: u64 = 2;

#[derive(Clone, Debug)]
pub struct CheckoutCompletedState {
    pub operation_id: Uuid,
    pub cart: CartResponse,
    pub checkout: CheckoutFulfillmentCreatedState,
}

#[derive(Debug, Error)]
pub enum CheckoutFinalizationError {
    #[error(transparent)]
    Operation(#[from] CheckoutOperationError),
    #[error("cart boundary `{stage}` failed with `{code}` (retryable={retryable}): {message}")]
    CartBoundary {
        stage: &'static str,
        code: String,
        message: String,
        retryable: bool,
    },
    #[error("checkout finalization conflict: {0}")]
    Conflict(String),
}

pub type CheckoutFinalizationResult<T> = Result<T, CheckoutFinalizationError>;

pub struct CheckoutFinalizationExecutor {
    cart_checkout_port: Arc<dyn CartCheckoutPort>,
    operation_journal: CheckoutOperationJournal,
    lease_seconds: i64,
    port_deadline: Duration,
}

impl CheckoutFinalizationExecutor {
    pub fn new(
        db: sea_orm::DatabaseConnection,
        cart_checkout_port: Arc<dyn CartCheckoutPort>,
    ) -> Self {
        Self {
            cart_checkout_port,
            operation_journal: CheckoutOperationJournal::new(db),
            lease_seconds: DEFAULT_CHECKOUT_LEASE_SECONDS,
            port_deadline: Duration::from_secs(CART_PORT_DEADLINE_SECONDS),
        }
    }

    pub fn with_lease_seconds(mut self, lease_seconds: i64) -> Self {
        self.lease_seconds = lease_seconds;
        self
    }

    pub fn with_port_deadline(mut self, port_deadline: Duration) -> Self {
        self.port_deadline = port_deadline;
        self
    }

    pub async fn complete(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        lease_owner: impl Into<String>,
        state: CheckoutFulfillmentCreatedState,
    ) -> CheckoutFinalizationResult<CheckoutCompletedState> {
        let lease_owner = lease_owner.into();
        let cart_id = validate_state(tenant_id, &state)?;

        let operation = self
            .operation_journal
            .get(tenant_id, state.operation_id)
            .await?;
        if operation.cart_id != cart_id {
            return Err(CheckoutFinalizationError::Conflict(format!(
                "checkout operation {} is bound to cart {}, not {}",
                operation.id, operation.cart_id, cart_id
            )));
        }
        if operation.status == CheckoutOperationStatus::Completed.as_str() {
            if operation.stage != CheckoutOperationStage::Completed.as_str() {
                return Err(CheckoutFinalizationError::Conflict(format!(
                    "checkout operation {} is completed at invalid stage `{}`",
                    operation.id, operation.stage
                )));
            }
            let cart = self.read_cart(tenant_id, actor_id, cart_id, &state).await?;
            ensure_completed_cart(&cart, cart_id, &state)?;
            return Ok(CheckoutCompletedState {
                operation_id: operation.id,
                cart,
                checkout: state,
            });
        }
        if operation.status != CheckoutOperationStatus::Executing.as_str() {
            return Err(CheckoutFinalizationError::Conflict(format!(
                "checkout operation {} must be executing, not `{}`",
                operation.id, operation.status
            )));
        }

        match operation.stage.as_str() {
            stage if stage == CheckoutOperationStage::FulfillmentCreated.as_str() => {
                let current = self.read_cart(tenant_id, actor_id, cart_id, &state).await?;
                validate_cart_identity(&current, cart_id, &state)?;
                let cart = match cart_status(&current)? {
                    CartStatus::Completed => current,
                    CartStatus::CheckingOut | CartStatus::Active => self
                        .cart_checkout_port
                        .complete_cart_checkout(
                            cart_context(
                                tenant_id,
                                actor_id,
                                &state,
                                self.port_deadline,
                                "complete",
                                true,
                            ),
                            CartCheckoutLifecycleRequest { cart_id },
                        )
                        .await
                        .map_err(|error| cart_error("complete_cart_checkout", error))?,
                    CartStatus::Abandoned => {
                        return Err(CheckoutFinalizationError::Conflict(format!(
                            "cart {} is abandoned and cannot complete checkout",
                            current.id
                        )));
                    }
                };
                ensure_completed_cart(&cart, cart_id, &state)?;
                self.operation_journal
                    .checkpoint(CheckoutOperationCheckpoint {
                        tenant_id,
                        operation_id: operation.id,
                        lease_owner: lease_owner.clone(),
                        expected_stage: CheckoutOperationStage::FulfillmentCreated,
                        next_stage: CheckoutOperationStage::CartCompleted,
                        snapshot_hash: None,
                        order_id: operation.order_id,
                        payment_collection_id: operation.payment_collection_id,
                        lease_seconds: self.lease_seconds,
                    })
                    .await?;
                self.operation_journal
                    .mark_completed(tenant_id, operation.id, lease_owner.clone())
                    .await?;
                Ok(CheckoutCompletedState {
                    operation_id: operation.id,
                    cart,
                    checkout: state,
                })
            }
            stage if stage == CheckoutOperationStage::CartCompleted.as_str() => {
                let cart = self.read_cart(tenant_id, actor_id, cart_id, &state).await?;
                ensure_completed_cart(&cart, cart_id, &state)?;
                self.operation_journal
                    .mark_completed(tenant_id, operation.id, lease_owner.clone())
                    .await?;
                Ok(CheckoutCompletedState {
                    operation_id: operation.id,
                    cart,
                    checkout: state,
                })
            }
            stage if stage == CheckoutOperationStage::Completed.as_str() => {
                Err(CheckoutFinalizationError::Conflict(format!(
                    "checkout operation {} has completed stage while status is `{}`",
                    operation.id, operation.status
                )))
            }
            stage => Err(CheckoutFinalizationError::Conflict(format!(
                "checkout operation {} cannot finalize from stage `{stage}`",
                operation.id
            ))),
        }
    }

    async fn read_cart(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        cart_id: Uuid,
        state: &CheckoutFulfillmentCreatedState,
    ) -> CheckoutFinalizationResult<CartResponse> {
        self.cart_checkout_port
            .read_cart_checkout_snapshot(
                cart_context(
                    tenant_id,
                    actor_id,
                    state,
                    self.port_deadline,
                    "read",
                    false,
                ),
                CartCheckoutSnapshotRequest {
                    cart_id,
                    locale: Some(state.plan.payload.context.locale.clone()),
                },
            )
            .await
            .map_err(|error| cart_error("read_cart_checkout_snapshot", error))
    }
}

fn validate_state(
    tenant_id: Uuid,
    state: &CheckoutFulfillmentCreatedState,
) -> CheckoutFinalizationResult<Uuid> {
    let cart_id = state.payment_collection.cart_id.ok_or_else(|| {
        CheckoutFinalizationError::Conflict(
            "captured checkout collection has no cart identity".to_string(),
        )
    })?;
    if state.order.tenant_id != tenant_id
        || state.plan.tenant_id != tenant_id
        || state.payment_collection.tenant_id != tenant_id
        || state.plan.checkout_operation_id != state.operation_id
        || state.payment_collection.order_id != Some(state.order.id)
        || !state.payment_collection.status_kind().is_captured()
        || !matches!(
            state.order.status_kind(),
            OrderStatusKind::Paid | OrderStatusKind::Shipped | OrderStatusKind::Delivered
        )
        || cart_id == Uuid::nil()
    {
        return Err(CheckoutFinalizationError::Conflict(
            "fulfillment-created state does not describe one completed checkout identity"
                .to_string(),
        ));
    }
    Ok(cart_id)
}

fn validate_cart_identity(
    cart: &CartResponse,
    cart_id: Uuid,
    state: &CheckoutFulfillmentCreatedState,
) -> CheckoutFinalizationResult<()> {
    if cart.id != cart_id
        || cart.tenant_id != state.order.tenant_id
        || !cart
            .currency_code
            .eq_ignore_ascii_case(state.order.currency_code.as_str())
        || cart.total_amount != state.order.total_amount
    {
        return Err(CheckoutFinalizationError::Conflict(format!(
            "cart {} no longer matches checkout order {}",
            cart.id, state.order.id
        )));
    }
    Ok(())
}

fn ensure_completed_cart(
    cart: &CartResponse,
    cart_id: Uuid,
    state: &CheckoutFulfillmentCreatedState,
) -> CheckoutFinalizationResult<()> {
    validate_cart_identity(cart, cart_id, state)?;
    if cart_status(cart)? != CartStatus::Completed || cart.completed_at.is_none() {
        return Err(CheckoutFinalizationError::Conflict(format!(
            "cart {} is not durably completed",
            cart.id
        )));
    }
    Ok(())
}

fn cart_status(cart: &CartResponse) -> CheckoutFinalizationResult<CartStatus> {
    cart.lifecycle_status().map_err(|_| {
        CheckoutFinalizationError::Conflict(format!(
            "cart {} has an unsupported lifecycle state",
            cart.id
        ))
    })
}

fn cart_context(
    tenant_id: Uuid,
    actor_id: Uuid,
    state: &CheckoutFulfillmentCreatedState,
    deadline: Duration,
    action: &str,
    write: bool,
) -> PortContext {
    let mut context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        state.plan.payload.context.locale.clone(),
        format!("checkout:{}:cart:{action}", state.operation_id),
    )
    .with_causation_id(state.operation_id.to_string())
    .with_deadline(deadline);
    if write {
        context =
            context.with_idempotency_key(format!("checkout:{}:cart:{action}", state.operation_id));
    }
    if let Some(channel) = state.plan.payload.channel_slug.as_deref() {
        context = context.with_channel(channel.to_string());
    }
    context
}

fn cart_error(stage: &'static str, error: PortError) -> CheckoutFinalizationError {
    CheckoutFinalizationError::CartBoundary {
        stage,
        code: error.code,
        message: error.message,
        retryable: error.retryable,
    }
}
