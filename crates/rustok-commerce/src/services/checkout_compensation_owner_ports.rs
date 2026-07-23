use std::{sync::Arc, time::Duration};

use rustok_api::{PLATFORM_FALLBACK_LOCALE, PortActor, PortContext, PortError};
use rustok_cart::{
    CartCheckoutLifecycleRequest, CartCheckoutPort, CartCheckoutSnapshotRequest, CartResponse,
    CartStatus,
};
use rustok_inventory::{
    InventoryIdentityReservationReleaseRequest, InventoryReservationIdentityPort,
};
use rustok_order::{
    in_process_checkout_order_compensation_port, CheckoutOrderCompensationPort,
    CheckoutOrderCompensationRequest, CheckoutOrderIdentityPort,
    InProcessCheckoutOrderCompensationPort, OrderStatusKind,
};
use rustok_outbox::TransactionalEventBus;
use rustok_payment::{
    in_process_checkout_payment_compensation_port, CheckoutPaymentCompensationPort,
    CheckoutPaymentCompensationRequest, InProcessCheckoutPaymentCompensationPort,
    PaymentCollectionStatusKind, PaymentProviderRegistry,
};
use sea_orm::DatabaseConnection;
use serde_json::json;
use thiserror::Error;
use uuid::Uuid;

use crate::entities::{checkout_inventory_reservation, checkout_operation};

use super::{
    CheckoutInventoryReservationError, CheckoutInventoryReservationJournal,
    CheckoutInventoryReservationStatus, CheckoutOperationError, CheckoutOperationJournal,
    CheckoutOperationStage, CheckoutOperationStatus, DEFAULT_CHECKOUT_LEASE_SECONDS,
};

const COMPENSATION_PORT_DEADLINE_SECONDS: u64 = 3;
const ORDER_MANUAL_RECONCILIATION_CODE: &str = "order.checkout_compensation_manual_reconciliation";
const PAYMENT_MANUAL_RECONCILIATION_CODE: &str =
    "payment.checkout_compensation_manual_reconciliation";

#[derive(Debug, Error)]
pub enum CheckoutCompensationError {
    #[error(transparent)]
    Operation(#[from] CheckoutOperationError),
    #[error(transparent)]
    ReservationJournal(#[from] CheckoutInventoryReservationError),
    #[error("checkout compensation requires manual reconciliation: {0}")]
    ManualReconciliation(String),
    #[error("checkout compensation conflict: {0}")]
    Conflict(String),
    #[error(
        "checkout compensation boundary `{stage}` failed with `{code}` (retryable={retryable}): {message}"
    )]
    Boundary {
        stage: &'static str,
        code: String,
        message: String,
        retryable: bool,
    },
    #[error("checkout compensation failed: {compensation}; journal update failed: {journal}")]
    CompensationAndJournal {
        compensation: Box<CheckoutCompensationError>,
        journal: CheckoutOperationError,
    },
}

pub type CheckoutCompensationResult<T> = Result<T, CheckoutCompensationError>;

pub struct CheckoutCompensationService {
    owner_db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    operation_journal: CheckoutOperationJournal,
    reservation_journal: CheckoutInventoryReservationJournal,
    reservation_port: Arc<dyn InventoryReservationIdentityPort>,
    cart_port: Arc<dyn CartCheckoutPort>,
    order_compensation_port: Arc<dyn CheckoutOrderCompensationPort>,
    payment_compensation_port: Arc<dyn CheckoutPaymentCompensationPort>,
    lease_seconds: i64,
    port_deadline: Duration,
}

impl CheckoutCompensationService {
    pub fn new(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        reservation_port: Arc<dyn InventoryReservationIdentityPort>,
        cart_port: Arc<dyn CartCheckoutPort>,
    ) -> Self {
        Self {
            owner_db: db.clone(),
            event_bus: event_bus.clone(),
            operation_journal: CheckoutOperationJournal::new(db.clone()),
            reservation_journal: CheckoutInventoryReservationJournal::new(db.clone()),
            reservation_port,
            cart_port,
            order_compensation_port: in_process_checkout_order_compensation_port(
                db.clone(),
                event_bus,
            ),
            payment_compensation_port: in_process_checkout_payment_compensation_port(db),
            lease_seconds: DEFAULT_CHECKOUT_LEASE_SECONDS,
            port_deadline: Duration::from_secs(COMPENSATION_PORT_DEADLINE_SECONDS),
        }
    }

    pub fn with_payment_provider_registry(
        mut self,
        payment_provider_registry: PaymentProviderRegistry,
    ) -> Self {
        self.payment_compensation_port = Arc::new(
            InProcessCheckoutPaymentCompensationPort::with_provider_registry(
                self.owner_db.clone(),
                payment_provider_registry,
            ),
        );
        self
    }

    pub fn with_order_identity_port(
        mut self,
        order_identity_port: Arc<dyn CheckoutOrderIdentityPort>,
    ) -> Self {
        self.order_compensation_port =
            Arc::new(InProcessCheckoutOrderCompensationPort::with_identity_port(
                self.owner_db.clone(),
                self.event_bus.clone(),
                order_identity_port,
            ));
        self
    }

    pub fn with_order_compensation_port(
        mut self,
        order_compensation_port: Arc<dyn CheckoutOrderCompensationPort>,
    ) -> Self {
        self.order_compensation_port = order_compensation_port;
        self
    }

    pub fn with_payment_compensation_port(
        mut self,
        payment_compensation_port: Arc<dyn CheckoutPaymentCompensationPort>,
    ) -> Self {
        self.payment_compensation_port = payment_compensation_port;
        self
    }

    pub fn with_lease_seconds(mut self, lease_seconds: i64) -> Self {
        self.lease_seconds = lease_seconds;
        self
    }

    pub async fn compensate(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        operation_id: Uuid,
        lease_owner: impl Into<String>,
    ) -> CheckoutCompensationResult<checkout_operation::Model> {
        let lease_owner = lease_owner.into();
        let Some(operation) = self
            .operation_journal
            .claim_compensation(
                tenant_id,
                operation_id,
                lease_owner.as_str(),
                self.lease_seconds,
            )
            .await?
        else {
            let current = self.operation_journal.get(tenant_id, operation_id).await?;
            if current.status == CheckoutOperationStatus::Compensated.as_str() {
                return Ok(current);
            }
            return Err(CheckoutCompensationError::Conflict(format!(
                "checkout operation {} cannot be claimed for compensation; status={}, lease_owner={}",
                current.id,
                current.status,
                current.lease_owner.as_deref().unwrap_or("none")
            )));
        };

        let result = self
            .compensate_claimed(tenant_id, actor_id, &operation)
            .await;
        match result {
            Ok(()) => self
                .operation_journal
                .mark_compensated(tenant_id, operation.id, lease_owner)
                .await
                .map_err(Into::into),
            Err(compensation) => {
                let code = compensation_error_code(&compensation);
                let message = compensation.to_string();
                match self
                    .operation_journal
                    .mark_compensation_retryable(
                        tenant_id,
                        operation.id,
                        lease_owner,
                        code,
                        message,
                    )
                    .await
                {
                    Ok(_) => Err(compensation),
                    Err(journal) => Err(CheckoutCompensationError::CompensationAndJournal {
                        compensation: Box::new(compensation),
                        journal,
                    }),
                }
            }
        }
    }

    async fn compensate_claimed(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        operation: &checkout_operation::Model,
    ) -> CheckoutCompensationResult<()> {
        if stage_rank(operation.stage.as_str())?
            >= stage_rank(CheckoutOperationStage::PaymentCaptured.as_str())?
        {
            return Err(CheckoutCompensationError::ManualReconciliation(format!(
                "checkout operation {} reached `{}`; captured funds must be reconciled through refund policy",
                operation.id, operation.stage
            )));
        }

        self.compensate_payment(tenant_id, actor_id, operation)
            .await?;
        self.compensate_order(tenant_id, actor_id, operation)
            .await?;

        // Order cancellation releases adopted reservation rows through the
        // checkout lifecycle trigger. Any still-reserved rows are pre-adoption
        // identities and must be released directly.
        self.release_remaining_reservations(tenant_id, operation)
            .await?;
        self.release_cart(tenant_id, operation).await?;
        Ok(())
    }

    async fn compensate_payment(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        operation: &checkout_operation::Model,
    ) -> CheckoutCompensationResult<()> {
        let snapshot = self
            .payment_compensation_port
            .compensate_checkout_payment(
                payment_context(tenant_id, actor_id, operation, self.port_deadline),
                CheckoutPaymentCompensationRequest {
                    checkout_operation_id: operation.id,
                    collection_id: operation.payment_collection_id,
                    reason: Some("checkout_compensation".to_string()),
                    metadata: json!({
                        "checkout": {
                            "operation_id": operation.id,
                            "compensation": true,
                        }
                    }),
                },
            )
            .await
            .map_err(|error| owner_boundary_error("compensate_payment", error))?;
        if let Some(snapshot) = snapshot {
            if operation.payment_collection_id != Some(snapshot.collection_id)
                || snapshot.status_kind() != PaymentCollectionStatusKind::Cancelled
            {
                return Err(CheckoutCompensationError::Conflict(format!(
                    "payment compensation result does not match checkout operation {}",
                    operation.id
                )));
            }
        } else if operation.payment_collection_id.is_some() {
            return Err(CheckoutCompensationError::ManualReconciliation(format!(
                "checkout operation {} records a payment collection but payment owner returned no compensation result",
                operation.id
            )));
        }
        Ok(())
    }

    async fn compensate_order(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        operation: &checkout_operation::Model,
    ) -> CheckoutCompensationResult<()> {
        let snapshot = self
            .order_compensation_port
            .compensate_checkout_order(
                order_context(tenant_id, actor_id, operation, self.port_deadline),
                CheckoutOrderCompensationRequest {
                    checkout_operation_id: operation.id,
                    cart_id: operation.cart_id,
                    expected_order_id: operation.order_id,
                    reason: Some("checkout_compensation".to_string()),
                },
            )
            .await
            .map_err(|error| owner_boundary_error("compensate_order", error))?;
        if let Some(snapshot) = snapshot {
            if operation.order_id.is_some() && operation.order_id != Some(snapshot.order_id) {
                return Err(CheckoutCompensationError::Conflict(format!(
                    "order compensation result does not match checkout operation {}",
                    operation.id
                )));
            }
            if snapshot.status_kind() != OrderStatusKind::Cancelled {
                return Err(CheckoutCompensationError::Conflict(format!(
                    "order {} is not cancelled after checkout compensation",
                    snapshot.order_id
                )));
            }
        } else if operation.order_id.is_some() {
            return Err(CheckoutCompensationError::ManualReconciliation(format!(
                "checkout operation {} records an order but order owner returned no compensation result",
                operation.id
            )));
        }
        Ok(())
    }

    async fn release_remaining_reservations(
        &self,
        tenant_id: Uuid,
        operation: &checkout_operation::Model,
    ) -> CheckoutCompensationResult<()> {
        let reservations = self
            .reservation_journal
            .list_by_operation(tenant_id, operation.id)
            .await?;
        for reservation in reservations {
            match reservation.status.as_str() {
                status if status == CheckoutInventoryReservationStatus::Planned.as_str() => {}
                status if status == CheckoutInventoryReservationStatus::Released.as_str() => {}
                status if status == CheckoutInventoryReservationStatus::Reserved.as_str() => {
                    let released = self
                        .reservation_port
                        .release_inventory_by_identity(
                            inventory_context(
                                tenant_id,
                                operation,
                                &reservation,
                                self.port_deadline,
                            ),
                            InventoryIdentityReservationReleaseRequest {
                                reservation_id: reservation.reservation_id,
                                external_id: reservation.external_id.clone(),
                            },
                        )
                        .await
                        .map_err(|error| boundary_error("release_inventory", error))?;
                    if released.reservation_id != reservation.reservation_id
                        || released.external_id != reservation.external_id
                        || released.variant_id != reservation.variant_id
                    {
                        return Err(CheckoutCompensationError::Conflict(format!(
                            "inventory release response does not match checkout reservation {}",
                            reservation.reservation_id
                        )));
                    }
                    self.reservation_journal
                        .mark_released(tenant_id, reservation.reservation_id)
                        .await?;
                }
                status if status == CheckoutInventoryReservationStatus::Consumed.as_str() => {
                    return Err(CheckoutCompensationError::ManualReconciliation(format!(
                        "inventory reservation {} is already consumed",
                        reservation.reservation_id
                    )));
                }
                status => {
                    return Err(CheckoutCompensationError::Conflict(format!(
                        "inventory reservation {} has unsupported status `{status}`",
                        reservation.reservation_id
                    )));
                }
            }
        }
        Ok(())
    }

    async fn release_cart(
        &self,
        tenant_id: Uuid,
        operation: &checkout_operation::Model,
    ) -> CheckoutCompensationResult<()> {
        let current = self
            .cart_port
            .read_cart_checkout_snapshot(
                cart_context(tenant_id, operation, self.port_deadline, "read", false),
                CartCheckoutSnapshotRequest {
                    cart_id: operation.cart_id,
                    locale: None,
                },
            )
            .await
            .map_err(|error| boundary_error("read_cart", error))?;
        match cart_status(&current)? {
            CartStatus::CheckingOut => {
                let released = self
                    .cart_port
                    .release_cart_checkout(
                        cart_context(tenant_id, operation, self.port_deadline, "release", true),
                        CartCheckoutLifecycleRequest {
                            cart_id: operation.cart_id,
                        },
                    )
                    .await
                    .map_err(|error| boundary_error("release_cart", error))?;
                if cart_status(&released)? != CartStatus::Active {
                    return Err(CheckoutCompensationError::Conflict(format!(
                        "cart {} is not active after checkout release",
                        released.id
                    )));
                }
            }
            CartStatus::Active => {}
            CartStatus::Completed => {
                return Err(CheckoutCompensationError::ManualReconciliation(format!(
                    "cart {} is already completed",
                    current.id
                )));
            }
            CartStatus::Abandoned => {
                return Err(CheckoutCompensationError::Conflict(format!(
                    "cart {} is abandoned and cannot be released",
                    current.id
                )));
            }
        }
        Ok(())
    }
}

fn cart_status(cart: &CartResponse) -> CheckoutCompensationResult<CartStatus> {
    cart.lifecycle_status().map_err(|_| {
        CheckoutCompensationError::ManualReconciliation(format!(
            "cart {} has an unknown lifecycle state",
            cart.id
        ))
    })
}

fn inventory_context(
    tenant_id: Uuid,
    operation: &checkout_operation::Model,
    reservation: &checkout_inventory_reservation::Model,
    deadline: Duration,
) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service("rustok-commerce.checkout-compensation"),
        PLATFORM_FALLBACK_LOCALE,
        format!(
            "checkout:{}:compensation:inventory:{}",
            operation.id, reservation.cart_line_item_id
        ),
    )
    .with_causation_id(operation.id.to_string())
    .with_idempotency_key(reservation.external_id.clone())
    .with_deadline(deadline)
}

fn cart_context(
    tenant_id: Uuid,
    operation: &checkout_operation::Model,
    deadline: Duration,
    action: &str,
    write: bool,
) -> PortContext {
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::service("rustok-commerce.checkout-compensation"),
        PLATFORM_FALLBACK_LOCALE,
        format!("checkout:{}:compensation:cart:{action}", operation.id),
    )
    .with_causation_id(operation.id.to_string())
    .with_deadline(deadline);
    if write {
        context.with_idempotency_key(format!(
            "checkout:{}:compensation:cart:{action}",
            operation.id
        ))
    } else {
        context
    }
}

fn order_context(
    tenant_id: Uuid,
    actor_id: Uuid,
    operation: &checkout_operation::Model,
    deadline: Duration,
) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        PLATFORM_FALLBACK_LOCALE,
        format!("checkout:{}:compensation:order", operation.id),
    )
    .with_causation_id(operation.id.to_string())
    .with_idempotency_key(format!("checkout:{}:compensation:order", operation.id))
    .with_deadline(deadline)
}

fn payment_context(
    tenant_id: Uuid,
    actor_id: Uuid,
    operation: &checkout_operation::Model,
    deadline: Duration,
) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        PLATFORM_FALLBACK_LOCALE,
        format!("checkout:{}:compensation:payment", operation.id),
    )
    .with_causation_id(operation.id.to_string())
    .with_idempotency_key(format!("checkout:{}:compensation:payment", operation.id))
    .with_deadline(deadline)
}

fn owner_boundary_error(stage: &'static str, error: PortError) -> CheckoutCompensationError {
    if matches!(
        error.code.as_str(),
        ORDER_MANUAL_RECONCILIATION_CODE | PAYMENT_MANUAL_RECONCILIATION_CODE
    ) {
        CheckoutCompensationError::ManualReconciliation(error.message)
    } else {
        boundary_error(stage, error)
    }
}

fn boundary_error(stage: &'static str, error: PortError) -> CheckoutCompensationError {
    CheckoutCompensationError::Boundary {
        stage,
        code: error.code,
        message: error.message,
        retryable: error.retryable,
    }
}

fn compensation_error_code(error: &CheckoutCompensationError) -> &'static str {
    match error {
        CheckoutCompensationError::ManualReconciliation(_) => {
            "checkout.compensation_manual_reconciliation"
        }
        CheckoutCompensationError::Boundary { .. } => "checkout.compensation_boundary_failed",
        CheckoutCompensationError::ReservationJournal(_) => {
            "checkout.compensation_inventory_failed"
        }
        CheckoutCompensationError::Operation(_)
        | CheckoutCompensationError::Conflict(_)
        | CheckoutCompensationError::CompensationAndJournal { .. } => {
            "checkout.compensation_failed"
        }
    }
}

fn stage_rank(stage: &str) -> CheckoutCompensationResult<u8> {
    match stage {
        "created" => Ok(0),
        "cart_locked" => Ok(1),
        "inventory_reserved" => Ok(2),
        "order_created" => Ok(3),
        "payment_ready" => Ok(4),
        "payment_authorized" => Ok(5),
        "payment_captured" => Ok(6),
        "fulfillment_created" => Ok(7),
        "cart_completed" => Ok(8),
        "completed" => Ok(9),
        other => Err(CheckoutCompensationError::Conflict(format!(
            "unsupported checkout stage `{other}`"
        ))),
    }
}
