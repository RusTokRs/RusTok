use rustok_api::{PLATFORM_FALLBACK_LOCALE, PortActor, PortContext, PortError};
use rustok_cart::PreparedCartCheckoutSnapshot;
use rustok_inventory::{
    InventoryIdentityReservationReleaseRequest, InventoryIdentityReservationRequest,
    InventoryReservationIdentityPort,
};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use thiserror::Error;
use uuid::Uuid;

use crate::entities::{checkout_inventory_reservation, checkout_operation};

use super::{
    CheckoutInventoryReservationError, CheckoutInventoryReservationJournal,
    CheckoutInventoryReservationStatus, CheckoutOperationCheckpoint, CheckoutOperationError,
    CheckoutOperationJournal, CheckoutOperationStage, CheckoutOperationStatus,
    DEFAULT_CHECKOUT_LEASE_SECONDS, PlanCheckoutInventoryReservation,
};

const DEFAULT_INVENTORY_PORT_DEADLINE_SECONDS: u64 = 2;

#[derive(Debug, Error)]
pub enum CheckoutInventoryExecutionError {
    #[error(transparent)]
    ReservationJournal(#[from] CheckoutInventoryReservationError),
    #[error(transparent)]
    OperationJournal(#[from] CheckoutOperationError),
    #[error("checkout inventory snapshot is invalid: {0}")]
    Snapshot(String),
    #[error(
        "inventory boundary failed for cart line {cart_line_item_id} and reservation {reservation_id}: {code}: {message}"
    )]
    Boundary {
        cart_line_item_id: Uuid,
        reservation_id: Uuid,
        code: String,
        message: String,
        retryable: bool,
    },
    #[error(
        "inventory boundary failed for cart line {cart_line_item_id} and reservation {reservation_id}: {code}: {message}; recording the failure also failed: {journal}"
    )]
    BoundaryAndJournal {
        cart_line_item_id: Uuid,
        reservation_id: Uuid,
        code: String,
        message: String,
        retryable: bool,
        journal: Box<CheckoutInventoryReservationError>,
    },
}

pub type CheckoutInventoryExecutionResult<T> = Result<T, CheckoutInventoryExecutionError>;

#[derive(Clone)]
pub struct CheckoutInventoryReservationExecutor {
    reservation_journal: CheckoutInventoryReservationJournal,
    operation_journal: CheckoutOperationJournal,
    reservation_port: Arc<dyn InventoryReservationIdentityPort>,
    port_deadline: Duration,
    lease_seconds: i64,
}

impl CheckoutInventoryReservationExecutor {
    pub fn new(
        db: sea_orm::DatabaseConnection,
        reservation_port: Arc<dyn InventoryReservationIdentityPort>,
    ) -> Self {
        Self {
            reservation_journal: CheckoutInventoryReservationJournal::new(db.clone()),
            operation_journal: CheckoutOperationJournal::new(db),
            reservation_port,
            port_deadline: Duration::from_secs(DEFAULT_INVENTORY_PORT_DEADLINE_SECONDS),
            lease_seconds: DEFAULT_CHECKOUT_LEASE_SECONDS,
        }
    }

    pub fn with_port_deadline(mut self, port_deadline: Duration) -> Self {
        self.port_deadline = port_deadline;
        self
    }

    pub fn with_lease_seconds(mut self, lease_seconds: i64) -> Self {
        self.lease_seconds = lease_seconds;
        self
    }

    /// Reserves all variant-backed lines from the immutable cart snapshot and
    /// advances the durable checkout operation from `cart_locked` to
    /// `inventory_reserved`. Existing reserved rows are adopted on replay.
    pub async fn reserve_and_checkpoint(
        &self,
        tenant_id: Uuid,
        actor: PortActor,
        operation_id: Uuid,
        lease_owner: impl Into<String>,
        snapshot: &PreparedCartCheckoutSnapshot,
    ) -> CheckoutInventoryExecutionResult<Vec<checkout_inventory_reservation::Model>> {
        let lease_owner = lease_owner.into();
        let operation = self
            .validate_locked_operation(tenant_id, operation_id, snapshot)
            .await?;
        let mut lines = snapshot.cart.line_items.iter().collect::<Vec<_>>();
        lines.sort_by_key(|line| line.id);
        let mut reservations = Vec::new();

        for line in lines {
            let Some(variant_id) = line.variant_id else {
                continue;
            };
            let planned = self
                .reservation_journal
                .plan(PlanCheckoutInventoryReservation {
                    tenant_id,
                    checkout_operation_id: operation_id,
                    cart_line_item_id: line.id,
                    variant_id,
                    quantity: line.quantity,
                })
                .await?;

            match planned.status.as_str() {
                status if status == CheckoutInventoryReservationStatus::Reserved.as_str() => {
                    reservations.push(planned);
                    continue;
                }
                status if status == CheckoutInventoryReservationStatus::Planned.as_str() => {}
                status => {
                    return Err(CheckoutInventoryExecutionError::Snapshot(format!(
                        "reservation {} for cart line {} is `{status}` and cannot be reused by checkout operation {}",
                        planned.reservation_id, line.id, operation_id
                    )));
                }
            }

            let provider_result = self
                .reservation_port
                .reserve_inventory_by_identity(
                    inventory_port_context(InventoryPortContextInput {
                        tenant_id,
                        actor: actor.clone(),
                        snapshot,
                        operation_id,
                        cart_line_item_id: line.id,
                        idempotency_key: planned.external_id.as_str(),
                        deadline: self.port_deadline,
                        action: "reserve",
                    }),
                    InventoryIdentityReservationRequest {
                        reservation_id: planned.reservation_id,
                        external_id: planned.external_id.clone(),
                        variant_id,
                        quantity: line.quantity,
                        line_item_id: Some(line.id),
                        metadata: json!({
                            "source": "checkout_operation",
                            "checkout_operation_id": operation_id,
                            "cart_id": snapshot.cart.id,
                            "cart_line_item_id": line.id,
                            "snapshot_hash": snapshot.snapshot_hash.as_str(),
                        }),
                    },
                )
                .await;

            let provider_result = match provider_result {
                Ok(result) => result,
                Err(boundary) => {
                    return Err(self
                        .record_boundary_failure(
                            tenant_id,
                            line.id,
                            planned.reservation_id,
                            boundary,
                        )
                        .await);
                }
            };

            if provider_result.reservation_id != planned.reservation_id
                || provider_result.external_id != planned.external_id
                || provider_result.variant_id != variant_id
                || provider_result.reserved_quantity != line.quantity
            {
                let boundary = PortError::invariant_violation(
                    "inventory.reservation_response_mismatch",
                    "inventory owner returned a reservation that does not match the persisted checkout identity",
                );
                return Err(self
                    .record_boundary_failure(tenant_id, line.id, planned.reservation_id, boundary)
                    .await);
            }

            reservations.push(
                self.reservation_journal
                    .mark_reserved(
                        tenant_id,
                        planned.reservation_id,
                        provider_result.location_id,
                    )
                    .await?,
            );
        }

        self.operation_journal
            .checkpoint(CheckoutOperationCheckpoint {
                tenant_id,
                operation_id,
                lease_owner,
                expected_stage: CheckoutOperationStage::CartLocked,
                next_stage: CheckoutOperationStage::InventoryReserved,
                snapshot_hash: None,
                order_id: operation.order_id,
                payment_collection_id: operation.payment_collection_id,
                lease_seconds: self.lease_seconds,
            })
            .await?;

        Ok(reservations)
    }

    /// Releases only reservations that the inventory owner confirmed. Planned
    /// rows have no provider side effect and remain as durable retry evidence.
    pub async fn release_reserved(
        &self,
        tenant_id: Uuid,
        actor: PortActor,
        operation_id: Uuid,
        snapshot: &PreparedCartCheckoutSnapshot,
    ) -> CheckoutInventoryExecutionResult<Vec<checkout_inventory_reservation::Model>> {
        let operation = self.operation_journal.get(tenant_id, operation_id).await?;
        validate_operation_snapshot(&operation, snapshot)?;
        let reservations = self
            .reservation_journal
            .list_by_operation(tenant_id, operation_id)
            .await?;
        let mut released = Vec::new();

        for reservation in reservations {
            match reservation.status.as_str() {
                status if status == CheckoutInventoryReservationStatus::Released.as_str() => {
                    released.push(reservation);
                }
                status if status == CheckoutInventoryReservationStatus::Planned.as_str() => {}
                status if status == CheckoutInventoryReservationStatus::Reserved.as_str() => {
                    let boundary = self
                        .reservation_port
                        .release_inventory_by_identity(
                            inventory_port_context(InventoryPortContextInput {
                                tenant_id,
                                actor: actor.clone(),
                                snapshot,
                                operation_id,
                                cart_line_item_id: reservation.cart_line_item_id,
                                idempotency_key: reservation.external_id.as_str(),
                                deadline: self.port_deadline,
                                action: "release",
                            }),
                            InventoryIdentityReservationReleaseRequest {
                                reservation_id: reservation.reservation_id,
                                external_id: reservation.external_id.clone(),
                            },
                        )
                        .await;
                    let boundary = match boundary {
                        Ok(result) => result,
                        Err(error) => {
                            return Err(self
                                .record_boundary_failure(
                                    tenant_id,
                                    reservation.cart_line_item_id,
                                    reservation.reservation_id,
                                    error,
                                )
                                .await);
                        }
                    };
                    if boundary.reservation_id != reservation.reservation_id
                        || boundary.external_id != reservation.external_id
                        || boundary.variant_id != reservation.variant_id
                    {
                        let error = PortError::invariant_violation(
                            "inventory.release_response_mismatch",
                            "inventory owner returned a release that does not match the persisted checkout identity",
                        );
                        return Err(self
                            .record_boundary_failure(
                                tenant_id,
                                reservation.cart_line_item_id,
                                reservation.reservation_id,
                                error,
                            )
                            .await);
                    }
                    released.push(
                        self.reservation_journal
                            .mark_released(tenant_id, reservation.reservation_id)
                            .await?,
                    );
                }
                status => {
                    return Err(CheckoutInventoryExecutionError::Snapshot(format!(
                        "reservation {} is `{status}` and cannot be released",
                        reservation.reservation_id
                    )));
                }
            }
        }

        Ok(released)
    }

    pub fn reservation_journal(&self) -> &CheckoutInventoryReservationJournal {
        &self.reservation_journal
    }

    async fn validate_locked_operation(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
        snapshot: &PreparedCartCheckoutSnapshot,
    ) -> CheckoutInventoryExecutionResult<checkout_operation::Model> {
        let operation = self.operation_journal.get(tenant_id, operation_id).await?;
        validate_operation_snapshot(&operation, snapshot)?;
        if operation.status != CheckoutOperationStatus::Executing.as_str() {
            return Err(CheckoutInventoryExecutionError::Snapshot(format!(
                "checkout operation {} must be executing, not `{}`",
                operation.id, operation.status
            )));
        }
        if operation.stage != CheckoutOperationStage::CartLocked.as_str() {
            return Err(CheckoutInventoryExecutionError::Snapshot(format!(
                "checkout operation {} must be at cart_locked, not `{}`",
                operation.id, operation.stage
            )));
        }
        Ok(operation)
    }

    async fn record_boundary_failure(
        &self,
        tenant_id: Uuid,
        cart_line_item_id: Uuid,
        reservation_id: Uuid,
        boundary: PortError,
    ) -> CheckoutInventoryExecutionError {
        match self
            .reservation_journal
            .record_error(
                tenant_id,
                reservation_id,
                boundary.code.clone(),
                boundary.message.clone(),
            )
            .await
        {
            Ok(_) => CheckoutInventoryExecutionError::Boundary {
                cart_line_item_id,
                reservation_id,
                code: boundary.code,
                message: boundary.message,
                retryable: boundary.retryable,
            },
            Err(journal) => CheckoutInventoryExecutionError::BoundaryAndJournal {
                cart_line_item_id,
                reservation_id,
                code: boundary.code,
                message: boundary.message,
                retryable: boundary.retryable,
                journal: Box::new(journal),
            },
        }
    }
}

fn validate_operation_snapshot(
    operation: &checkout_operation::Model,
    snapshot: &PreparedCartCheckoutSnapshot,
) -> CheckoutInventoryExecutionResult<()> {
    if operation.cart_id != snapshot.cart.id {
        return Err(CheckoutInventoryExecutionError::Snapshot(format!(
            "checkout operation {} is bound to cart {}, not {}",
            operation.id, operation.cart_id, snapshot.cart.id
        )));
    }
    if operation.snapshot_hash.as_deref() != Some(snapshot.snapshot_hash.as_str()) {
        return Err(CheckoutInventoryExecutionError::Snapshot(format!(
            "checkout operation {} snapshot hash does not match the prepared cart",
            operation.id
        )));
    }
    Ok(())
}

struct InventoryPortContextInput<'a> {
    tenant_id: Uuid,
    actor: PortActor,
    snapshot: &'a PreparedCartCheckoutSnapshot,
    operation_id: Uuid,
    cart_line_item_id: Uuid,
    idempotency_key: &'a str,
    deadline: Duration,
    action: &'a str,
}

fn inventory_port_context(input: InventoryPortContextInput<'_>) -> PortContext {
    let locale = input
        .snapshot
        .cart
        .locale_code
        .as_deref()
        .unwrap_or(PLATFORM_FALLBACK_LOCALE);
    let mut context = PortContext::new(
        input.tenant_id.to_string(),
        input.actor,
        locale,
        format!(
            "checkout:{}:inventory:{}:{}",
            input.operation_id, input.action, input.cart_line_item_id
        ),
    )
    .with_causation_id(input.operation_id.to_string())
    .with_idempotency_key(input.idempotency_key.to_string())
    .with_deadline(input.deadline);
    if let Some(channel) = input.snapshot.cart.channel_slug.as_deref() {
        context = context.with_channel(channel.to_string());
    }
    context
}
