use rustok_api::{PLATFORM_FALLBACK_LOCALE, PortActor, PortContext, PortError};
use rustok_cart::{
    CartCheckoutLifecycleRequest, CartCheckoutPort, CartCheckoutSnapshotRequest, CartStatus,
};
use rustok_inventory::{
    InventoryIdentityReservationReleaseRequest, InventoryReservationIdentityPort,
};
use rustok_order::{
    AdoptLegacyCheckoutOrderIdentityRequest, CheckoutOrderIdentityPort,
    CheckoutOrderIdentitySnapshot, OrderError, OrderService,
    ReadCheckoutOrderIdentityByOperationRequest, in_process_checkout_order_identity_port,
};
use rustok_outbox::TransactionalEventBus;
use rustok_payment::dto::CancelPaymentInput;
use rustok_payment::error::PaymentError;
use rustok_payment::providers::PaymentProviderRegistry;
use rustok_payment::{
    PROVIDER_OPERATION_EXECUTING, PROVIDER_OPERATION_RECONCILIATION_REQUIRED,
    PROVIDER_OPERATION_SUCCEEDED, PaymentProviderOperationJournal, PaymentService,
};
use sea_orm::DatabaseConnection;
use serde_json::json;
use std::{sync::Arc, time::Duration};
use thiserror::Error;
use uuid::Uuid;

use crate::entities::{checkout_inventory_reservation, checkout_operation};

use super::{
    CheckoutInventoryReservationError, CheckoutInventoryReservationJournal,
    CheckoutInventoryReservationStatus, CheckoutOperationError, CheckoutOperationJournal,
    CheckoutOperationStage, CheckoutOperationStatus, DEFAULT_CHECKOUT_LEASE_SECONDS,
    PaymentOrchestrationError, PaymentOrchestrationService,
};

const COMPENSATION_PORT_DEADLINE_SECONDS: u64 = 3;

#[derive(Debug, Error)]
pub enum CheckoutCompensationError {
    #[error(transparent)]
    Operation(#[from] CheckoutOperationError),
    #[error(transparent)]
    ReservationJournal(#[from] CheckoutInventoryReservationError),
    #[error(transparent)]
    Payment(#[from] PaymentError),
    #[error(transparent)]
    PaymentOrchestration(#[from] PaymentOrchestrationError),
    #[error(transparent)]
    Order(#[from] OrderError),
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
    operation_journal: CheckoutOperationJournal,
    reservation_journal: CheckoutInventoryReservationJournal,
    reservation_port: Arc<dyn InventoryReservationIdentityPort>,
    cart_port: Arc<dyn CartCheckoutPort>,
    order_identity_port: Arc<dyn CheckoutOrderIdentityPort>,
    payment_service: PaymentService,
    payment_orchestration: PaymentOrchestrationService,
    payment_operation_journal: PaymentProviderOperationJournal,
    order_service: OrderService,
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
            operation_journal: CheckoutOperationJournal::new(db.clone()),
            reservation_journal: CheckoutInventoryReservationJournal::new(db.clone()),
            reservation_port,
            cart_port,
            order_identity_port: in_process_checkout_order_identity_port(db.clone()),
            payment_service: PaymentService::new(db.clone()),
            payment_orchestration: PaymentOrchestrationService::new(db.clone()),
            payment_operation_journal: PaymentProviderOperationJournal::new(db.clone()),
            order_service: OrderService::new(db, event_bus),
            lease_seconds: DEFAULT_CHECKOUT_LEASE_SECONDS,
            port_deadline: Duration::from_secs(COMPENSATION_PORT_DEADLINE_SECONDS),
        }
    }

    pub fn with_payment_provider_registry(
        mut self,
        payment_provider_registry: PaymentProviderRegistry,
    ) -> Self {
        self.payment_orchestration = self
            .payment_orchestration
            .with_provider_registry(payment_provider_registry);
        self
    }

    pub fn with_order_identity_port(
        mut self,
        order_identity_port: Arc<dyn CheckoutOrderIdentityPort>,
    ) -> Self {
        self.order_identity_port = order_identity_port;
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

        self.compensate_payment(tenant_id, operation).await?;

        if let Some(order_id) = self
            .resolve_compensation_order_id(tenant_id, operation)
            .await?
        {
            self.compensate_order(tenant_id, actor_id, operation, order_id)
                .await?;
        }

        // Order cancellation releases adopted reservation rows through the
        // checkout lifecycle trigger. Any still-reserved rows are pre-adoption
        // identities and must be released directly.
        self.release_remaining_reservations(tenant_id, operation)
            .await?;
        self.release_cart(tenant_id, operation).await?;
        Ok(())
    }

    async fn resolve_compensation_order_id(
        &self,
        tenant_id: Uuid,
        operation: &checkout_operation::Model,
    ) -> CheckoutCompensationResult<Option<Uuid>> {
        let mut identity = self
            .order_identity_port
            .read_by_operation(
                order_identity_context(
                    tenant_id,
                    operation,
                    self.port_deadline,
                    "read",
                    false,
                ),
                ReadCheckoutOrderIdentityByOperationRequest {
                    checkout_operation_id: operation.id,
                },
            )
            .await
            .map_err(|error| boundary_error("read_order_identity", error))?;
        if identity.is_none() {
            identity = self
                .order_identity_port
                .adopt_legacy(
                    order_identity_context(
                        tenant_id,
                        operation,
                        self.port_deadline,
                        "adopt",
                        true,
                    ),
                    AdoptLegacyCheckoutOrderIdentityRequest {
                        checkout_operation_id: operation.id,
                        cart_id: operation.cart_id,
                    },
                )
                .await
                .map_err(|error| boundary_error("adopt_order_identity", error))?;
        }

        match identity {
            Some(identity) => {
                validate_compensation_identity(tenant_id, operation, &identity)?;
                Ok(Some(identity.order_id))
            }
            None if operation.order_id.is_none() => Ok(None),
            None => Err(CheckoutCompensationError::ManualReconciliation(format!(
                "checkout operation {} records order {} but has no order-owner identity",
                operation.id,
                operation.order_id.expect("checked as present")
            ))),
        }
    }

    async fn compensate_payment(
        &self,
        tenant_id: Uuid,
        operation: &checkout_operation::Model,
    ) -> CheckoutCompensationResult<()> {
        let Some(collection_id) = operation.payment_collection_id else {
            return Ok(());
        };
        let collection = self
            .payment_service
            .get_collection(tenant_id, collection_id)
            .await?;
        let provider_operations = self
            .payment_operation_journal
            .list_by_collection(tenant_id, collection_id)
            .await?;
        if let Some(unsafe_operation) = provider_operations.iter().find(|provider_operation| {
            matches!(
                provider_operation.status.as_str(),
                PROVIDER_OPERATION_EXECUTING
                    | PROVIDER_OPERATION_SUCCEEDED
                    | PROVIDER_OPERATION_RECONCILIATION_REQUIRED
            )
        }) {
            return Err(CheckoutCompensationError::ManualReconciliation(format!(
                "payment provider operation {} is `{}` for collection {}",
                unsafe_operation.id, unsafe_operation.status, collection_id
            )));
        }

        match collection.status.as_str() {
            "pending" | "authorized" => {
                self.payment_orchestration
                    .cancel_collection(
                        tenant_id,
                        collection_id,
                        CancelPaymentInput {
                            reason: Some("checkout_compensation".to_string()),
                            metadata: json!({
                                "checkout": {
                                    "operation_id": operation.id,
                                    "compensation": true,
                                }
                            }),
                        },
                    )
                    .await?;
            }
            "cancelled" => {}
            "captured" => {
                return Err(CheckoutCompensationError::ManualReconciliation(format!(
                    "payment collection {collection_id} is captured"
                )));
            }
            status => {
                return Err(CheckoutCompensationError::Conflict(format!(
                    "payment collection {collection_id} cannot compensate from `{status}`"
                )));
            }
        }
        Ok(())
    }

    async fn compensate_order(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        operation: &checkout_operation::Model,
        order_id: Uuid,
    ) -> CheckoutCompensationResult<()> {
        let order = self
            .order_service
            .get_order_with_locale_fallback(tenant_id, order_id, PLATFORM_FALLBACK_LOCALE, None)
            .await?;
        if operation.order_id.is_some() && operation.order_id != Some(order.id) {
            return Err(CheckoutCompensationError::Conflict(format!(
                "order {} does not match checkout operation {} checkpoint",
                order.id, operation.id
            )));
        }

        match order.status.as_str() {
            "pending" | "confirmed" => {
                self.order_service
                    .cancel_order(
                        tenant_id,
                        actor_id,
                        order_id,
                        Some("checkout_compensation".to_string()),
                    )
                    .await?;
            }
            "cancelled" => {}
            "paid" | "shipped" | "delivered" => {
                return Err(CheckoutCompensationError::ManualReconciliation(format!(
                    "order {order_id} is `{}` and cannot be automatically cancelled",
                    order.status
                )));
            }
            status => {
                return Err(CheckoutCompensationError::Conflict(format!(
                    "order {order_id} cannot compensate from `{status}`"
                )));
            }
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
        match current.status.as_str() {
            status if status == CartStatus::CheckingOut.as_str() => {
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
                if released.status != CartStatus::Active.as_str() {
                    return Err(CheckoutCompensationError::Conflict(format!(
                        "cart {} is `{}` after checkout release",
                        released.id, released.status
                    )));
                }
            }
            status if status == CartStatus::Active.as_str() => {}
            status if status == CartStatus::Completed.as_str() => {
                return Err(CheckoutCompensationError::ManualReconciliation(format!(
                    "cart {} is already completed",
                    current.id
                )));
            }
            status => {
                return Err(CheckoutCompensationError::Conflict(format!(
                    "cart {} cannot be released from `{status}`",
                    current.id
                )));
            }
        }
        Ok(())
    }
}

fn validate_compensation_identity(
    tenant_id: Uuid,
    operation: &checkout_operation::Model,
    identity: &CheckoutOrderIdentitySnapshot,
) -> CheckoutCompensationResult<()> {
    if identity.tenant_id != tenant_id
        || identity.checkout_operation_id != operation.id
        || identity.source_cart_id.is_some()
            && identity.source_cart_id != Some(operation.cart_id)
        || operation.order_id.is_some() && operation.order_id != Some(identity.order_id)
    {
        return Err(CheckoutCompensationError::Conflict(format!(
            "typed order identity does not match checkout operation {}",
            operation.id
        )));
    }
    Ok(())
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

fn order_identity_context(
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
        format!("checkout:{}:compensation:order-identity:{action}", operation.id),
    )
    .with_causation_id(operation.id.to_string())
    .with_deadline(deadline);
    if write {
        context.with_idempotency_key(format!(
            "checkout:{}:compensation:order-identity:{action}",
            operation.id
        ))
    } else {
        context
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
        CheckoutCompensationError::Payment(_)
        | CheckoutCompensationError::PaymentOrchestration(_) => {
            "checkout.compensation_payment_failed"
        }
        CheckoutCompensationError::Order(_) => "checkout.compensation_order_failed",
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
