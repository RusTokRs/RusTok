use chrono::Utc;
use rustok_cart::entities::cart_line_item;
use rustok_core::generate_id;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    sea_query::Expr,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::entities::{checkout_inventory_reservation, checkout_operation};

use super::checkout_operation::{CheckoutOperationStage, CheckoutOperationStatus};

const MAX_EXTERNAL_ID_LENGTH: usize = 191;
const MAX_ERROR_CODE_LENGTH: usize = 100;
const MAX_ERROR_MESSAGE_LENGTH: usize = 2000;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum CheckoutInventoryReservationStatus {
    Planned,
    Reserved,
    Released,
    Consumed,
}

impl CheckoutInventoryReservationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Reserved => "reserved",
            Self::Released => "released",
            Self::Consumed => "consumed",
        }
    }
}

#[derive(Debug, Error)]
pub enum CheckoutInventoryReservationError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("checkout inventory reservation {0} not found")]
    NotFound(Uuid),
    #[error("checkout inventory reservation conflict: {0}")]
    Conflict(String),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

pub type CheckoutInventoryReservationResult<T> = Result<T, CheckoutInventoryReservationError>;

#[derive(Clone, Debug)]
pub struct PlanCheckoutInventoryReservation {
    pub tenant_id: Uuid,
    pub checkout_operation_id: Uuid,
    pub cart_line_item_id: Uuid,
    pub variant_id: Uuid,
    pub quantity: i32,
}

#[derive(Clone)]
pub struct CheckoutInventoryReservationJournal {
    db: DatabaseConnection,
}

impl CheckoutInventoryReservationJournal {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn plan(
        &self,
        input: PlanCheckoutInventoryReservation,
    ) -> CheckoutInventoryReservationResult<checkout_inventory_reservation::Model> {
        if input.quantity <= 0 {
            return Err(CheckoutInventoryReservationError::Validation(
                "reservation quantity must be positive".to_string(),
            ));
        }

        if let Some(existing) = self
            .find_by_operation_line(
                input.tenant_id,
                input.checkout_operation_id,
                input.cart_line_item_id,
            )
            .await?
        {
            ensure_same_plan(&existing, &input)?;
            return Ok(existing);
        }

        let operation = checkout_operation::Entity::find_by_id(input.checkout_operation_id)
            .filter(checkout_operation::Column::TenantId.eq(input.tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                CheckoutInventoryReservationError::Conflict(format!(
                    "checkout operation {} was not found for tenant {}",
                    input.checkout_operation_id, input.tenant_id
                ))
            })?;

        if terminal_operation_status(operation.status.as_str()) {
            return Err(CheckoutInventoryReservationError::Conflict(format!(
                "checkout operation {} is terminal with status `{}`",
                operation.id, operation.status
            )));
        }
        if operation.stage != CheckoutOperationStage::CartLocked.as_str() {
            return Err(CheckoutInventoryReservationError::Conflict(format!(
                "checkout operation {} cannot plan inventory from stage `{}`",
                operation.id, operation.stage
            )));
        }

        let line = cart_line_item::Entity::find_by_id(input.cart_line_item_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                CheckoutInventoryReservationError::Conflict(format!(
                    "cart line item {} was not found",
                    input.cart_line_item_id
                ))
            })?;
        if line.cart_id != operation.cart_id {
            return Err(CheckoutInventoryReservationError::Conflict(format!(
                "cart line item {} does not belong to checkout cart {}",
                line.id, operation.cart_id
            )));
        }
        if line.variant_id != Some(input.variant_id) {
            return Err(CheckoutInventoryReservationError::Conflict(format!(
                "cart line item {} is not bound to variant {}",
                line.id, input.variant_id
            )));
        }
        if line.quantity != input.quantity {
            return Err(CheckoutInventoryReservationError::Conflict(format!(
                "cart line item {} quantity changed from planned quantity {} to {}",
                line.id, input.quantity, line.quantity
            )));
        }

        let reservation_id = generate_id();
        let external_id =
            checkout_reservation_external_id(input.checkout_operation_id, input.cart_line_item_id)?;
        let now = Utc::now();
        let insert = checkout_inventory_reservation::ActiveModel {
            reservation_id: Set(reservation_id),
            tenant_id: Set(input.tenant_id),
            checkout_operation_id: Set(input.checkout_operation_id),
            cart_line_item_id: Set(input.cart_line_item_id),
            order_line_item_id: Set(None),
            external_id: Set(external_id),
            variant_id: Set(input.variant_id),
            quantity: Set(input.quantity),
            location_id: Set(None),
            status: Set(CheckoutInventoryReservationStatus::Planned
                .as_str()
                .to_string()),
            last_error_code: Set(None),
            last_error_message: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            released_at: Set(None),
            consumed_at: Set(None),
        }
        .insert(&self.db)
        .await;

        match insert {
            Ok(model) => Ok(model),
            Err(insert_error) => {
                if let Some(existing) = self
                    .find_by_operation_line(
                        input.tenant_id,
                        input.checkout_operation_id,
                        input.cart_line_item_id,
                    )
                    .await?
                {
                    ensure_same_plan(&existing, &input)?;
                    return Ok(existing);
                }
                Err(insert_error.into())
            }
        }
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        reservation_id: Uuid,
    ) -> CheckoutInventoryReservationResult<checkout_inventory_reservation::Model> {
        checkout_inventory_reservation::Entity::find_by_id(reservation_id)
            .filter(checkout_inventory_reservation::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(CheckoutInventoryReservationError::NotFound(reservation_id))
    }

    pub async fn find_by_operation_line(
        &self,
        tenant_id: Uuid,
        checkout_operation_id: Uuid,
        cart_line_item_id: Uuid,
    ) -> CheckoutInventoryReservationResult<Option<checkout_inventory_reservation::Model>> {
        checkout_inventory_reservation::Entity::find()
            .filter(checkout_inventory_reservation::Column::TenantId.eq(tenant_id))
            .filter(
                checkout_inventory_reservation::Column::CheckoutOperationId
                    .eq(checkout_operation_id),
            )
            .filter(checkout_inventory_reservation::Column::CartLineItemId.eq(cart_line_item_id))
            .one(&self.db)
            .await
            .map_err(Into::into)
    }

    pub async fn list_by_operation(
        &self,
        tenant_id: Uuid,
        checkout_operation_id: Uuid,
    ) -> CheckoutInventoryReservationResult<Vec<checkout_inventory_reservation::Model>> {
        checkout_inventory_reservation::Entity::find()
            .filter(checkout_inventory_reservation::Column::TenantId.eq(tenant_id))
            .filter(
                checkout_inventory_reservation::Column::CheckoutOperationId
                    .eq(checkout_operation_id),
            )
            .order_by_asc(checkout_inventory_reservation::Column::CartLineItemId)
            .all(&self.db)
            .await
            .map_err(Into::into)
    }

    pub async fn mark_reserved(
        &self,
        tenant_id: Uuid,
        reservation_id: Uuid,
        location_id: Uuid,
    ) -> CheckoutInventoryReservationResult<checkout_inventory_reservation::Model> {
        let current = self.get(tenant_id, reservation_id).await?;
        if current.status == CheckoutInventoryReservationStatus::Reserved.as_str() {
            if current.location_id == Some(location_id) {
                return Ok(current);
            }
            return Err(CheckoutInventoryReservationError::Conflict(format!(
                "reservation {} is already bound to another location",
                reservation_id
            )));
        }
        if current.status != CheckoutInventoryReservationStatus::Planned.as_str() {
            return Err(invalid_transition(
                reservation_id,
                current.status.as_str(),
                CheckoutInventoryReservationStatus::Reserved,
            ));
        }

        let update = checkout_inventory_reservation::Entity::update_many()
            .col_expr(
                checkout_inventory_reservation::Column::Status,
                Expr::value(CheckoutInventoryReservationStatus::Reserved.as_str()),
            )
            .col_expr(
                checkout_inventory_reservation::Column::LocationId,
                Expr::value(Some(location_id)),
            )
            .col_expr(
                checkout_inventory_reservation::Column::LastErrorCode,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                checkout_inventory_reservation::Column::LastErrorMessage,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                checkout_inventory_reservation::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(checkout_inventory_reservation::Column::TenantId.eq(tenant_id))
            .filter(checkout_inventory_reservation::Column::ReservationId.eq(reservation_id))
            .filter(
                checkout_inventory_reservation::Column::Status
                    .eq(CheckoutInventoryReservationStatus::Planned.as_str()),
            )
            .exec(&self.db)
            .await?;

        if update.rows_affected != 1 {
            return self
                .resolve_transition_race(
                    tenant_id,
                    reservation_id,
                    CheckoutInventoryReservationStatus::Reserved,
                    Some(location_id),
                )
                .await;
        }
        self.get(tenant_id, reservation_id).await
    }

    pub async fn mark_released(
        &self,
        tenant_id: Uuid,
        reservation_id: Uuid,
    ) -> CheckoutInventoryReservationResult<checkout_inventory_reservation::Model> {
        self.mark_terminal_disposition(
            tenant_id,
            reservation_id,
            CheckoutInventoryReservationStatus::Released,
        )
        .await
    }

    pub async fn mark_consumed(
        &self,
        tenant_id: Uuid,
        reservation_id: Uuid,
    ) -> CheckoutInventoryReservationResult<checkout_inventory_reservation::Model> {
        self.mark_terminal_disposition(
            tenant_id,
            reservation_id,
            CheckoutInventoryReservationStatus::Consumed,
        )
        .await
    }

    pub async fn record_error(
        &self,
        tenant_id: Uuid,
        reservation_id: Uuid,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> CheckoutInventoryReservationResult<checkout_inventory_reservation::Model> {
        let code = normalize_bounded(code.into(), "error code", MAX_ERROR_CODE_LENGTH)?;
        let message = normalize_bounded(message.into(), "error message", MAX_ERROR_MESSAGE_LENGTH)?;
        let update = checkout_inventory_reservation::Entity::update_many()
            .col_expr(
                checkout_inventory_reservation::Column::LastErrorCode,
                Expr::value(Some(code)),
            )
            .col_expr(
                checkout_inventory_reservation::Column::LastErrorMessage,
                Expr::value(Some(message)),
            )
            .col_expr(
                checkout_inventory_reservation::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(checkout_inventory_reservation::Column::TenantId.eq(tenant_id))
            .filter(checkout_inventory_reservation::Column::ReservationId.eq(reservation_id))
            .exec(&self.db)
            .await?;
        if update.rows_affected != 1 {
            return Err(CheckoutInventoryReservationError::NotFound(reservation_id));
        }
        self.get(tenant_id, reservation_id).await
    }

    async fn mark_terminal_disposition(
        &self,
        tenant_id: Uuid,
        reservation_id: Uuid,
        target: CheckoutInventoryReservationStatus,
    ) -> CheckoutInventoryReservationResult<checkout_inventory_reservation::Model> {
        debug_assert!(matches!(
            target,
            CheckoutInventoryReservationStatus::Released
                | CheckoutInventoryReservationStatus::Consumed
        ));
        let current = self.get(tenant_id, reservation_id).await?;
        if current.status == target.as_str() {
            return Ok(current);
        }
        if current.status != CheckoutInventoryReservationStatus::Reserved.as_str() {
            return Err(invalid_transition(
                reservation_id,
                current.status.as_str(),
                target,
            ));
        }

        let mut update = checkout_inventory_reservation::Entity::update_many()
            .col_expr(
                checkout_inventory_reservation::Column::Status,
                Expr::value(target.as_str()),
            )
            .col_expr(
                checkout_inventory_reservation::Column::LastErrorCode,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                checkout_inventory_reservation::Column::LastErrorMessage,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                checkout_inventory_reservation::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(checkout_inventory_reservation::Column::TenantId.eq(tenant_id))
            .filter(checkout_inventory_reservation::Column::ReservationId.eq(reservation_id))
            .filter(
                checkout_inventory_reservation::Column::Status
                    .eq(CheckoutInventoryReservationStatus::Reserved.as_str()),
            );
        update = match target {
            CheckoutInventoryReservationStatus::Released => update.col_expr(
                checkout_inventory_reservation::Column::ReleasedAt,
                Expr::current_timestamp().into(),
            ),
            CheckoutInventoryReservationStatus::Consumed => update.col_expr(
                checkout_inventory_reservation::Column::ConsumedAt,
                Expr::current_timestamp().into(),
            ),
            _ => unreachable!("terminal reservation disposition must be released or consumed"),
        };
        let result = update.exec(&self.db).await?;
        if result.rows_affected != 1 {
            return self
                .resolve_transition_race(tenant_id, reservation_id, target, current.location_id)
                .await;
        }
        self.get(tenant_id, reservation_id).await
    }

    async fn resolve_transition_race(
        &self,
        tenant_id: Uuid,
        reservation_id: Uuid,
        target: CheckoutInventoryReservationStatus,
        expected_location_id: Option<Uuid>,
    ) -> CheckoutInventoryReservationResult<checkout_inventory_reservation::Model> {
        let current = self.get(tenant_id, reservation_id).await?;
        if current.status == target.as_str()
            && (expected_location_id.is_none() || current.location_id == expected_location_id)
        {
            return Ok(current);
        }
        Err(invalid_transition(
            reservation_id,
            current.status.as_str(),
            target,
        ))
    }
}

fn checkout_reservation_external_id(
    checkout_operation_id: Uuid,
    cart_line_item_id: Uuid,
) -> CheckoutInventoryReservationResult<String> {
    let external_id = format!("checkout:{checkout_operation_id}:line:{cart_line_item_id}");
    if external_id.len() > MAX_EXTERNAL_ID_LENGTH {
        return Err(CheckoutInventoryReservationError::Validation(
            "checkout inventory reservation external id is too long".to_string(),
        ));
    }
    Ok(external_id)
}

fn ensure_same_plan(
    existing: &checkout_inventory_reservation::Model,
    input: &PlanCheckoutInventoryReservation,
) -> CheckoutInventoryReservationResult<()> {
    if existing.tenant_id != input.tenant_id
        || existing.checkout_operation_id != input.checkout_operation_id
        || existing.cart_line_item_id != input.cart_line_item_id
        || existing.variant_id != input.variant_id
        || existing.quantity != input.quantity
    {
        return Err(CheckoutInventoryReservationError::Conflict(format!(
            "reservation {} is already bound to another checkout inventory request",
            existing.reservation_id
        )));
    }
    Ok(())
}

fn invalid_transition(
    reservation_id: Uuid,
    from: &str,
    to: CheckoutInventoryReservationStatus,
) -> CheckoutInventoryReservationError {
    CheckoutInventoryReservationError::Conflict(format!(
        "reservation {reservation_id} cannot transition from `{from}` to `{}`",
        to.as_str()
    ))
}

fn terminal_operation_status(status: &str) -> bool {
    matches!(
        status,
        value if value == CheckoutOperationStatus::Completed.as_str()
            || value == CheckoutOperationStatus::Compensated.as_str()
            || value == CheckoutOperationStatus::Failed.as_str()
    )
}

fn normalize_bounded(
    value: String,
    label: &str,
    max_length: usize,
) -> CheckoutInventoryReservationResult<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(CheckoutInventoryReservationError::Validation(format!(
            "{label} must not be empty"
        )));
    }
    if value.len() > max_length {
        return Err(CheckoutInventoryReservationError::Validation(format!(
            "{label} exceeds {max_length} bytes"
        )));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_id_is_stable_for_operation_and_line() {
        let operation_id = Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap();
        let line_id = Uuid::parse_str("22222222-2222-4222-8222-222222222222").unwrap();

        assert_eq!(
            checkout_reservation_external_id(operation_id, line_id).unwrap(),
            "checkout:11111111-1111-4111-8111-111111111111:line:22222222-2222-4222-8222-222222222222"
        );
    }

    #[test]
    fn terminal_checkout_statuses_are_not_plannable() {
        assert!(terminal_operation_status(
            CheckoutOperationStatus::Completed.as_str()
        ));
        assert!(terminal_operation_status(
            CheckoutOperationStatus::Compensated.as_str()
        ));
        assert!(terminal_operation_status(
            CheckoutOperationStatus::Failed.as_str()
        ));
        assert!(!terminal_operation_status(
            CheckoutOperationStatus::Executing.as_str()
        ));
    }

    #[test]
    fn bounded_error_values_reject_empty_and_oversized_input() {
        assert!(normalize_bounded("  ".to_string(), "code", 10).is_err());
        assert!(normalize_bounded("01234567890".to_string(), "code", 10).is_err());
        assert_eq!(
            normalize_bounded("  safe  ".to_string(), "code", 10).unwrap(),
            "safe"
        );
    }
}
