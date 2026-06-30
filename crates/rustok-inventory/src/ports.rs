use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Transport-neutral owner boundary for checkout/storefront inventory availability and reservations.
#[async_trait]
pub trait InventoryReservationPort: Send + Sync {
    async fn check_availability(
        &self,
        context: PortContext,
        request: InventoryAvailabilityRequest,
    ) -> Result<InventoryAvailabilitySnapshot, PortError>;

    async fn reserve_inventory(
        &self,
        context: PortContext,
        request: InventoryReservationRequest,
    ) -> Result<InventoryReservationSnapshot, PortError>;

    async fn release_inventory_reservation(
        &self,
        context: PortContext,
        request: InventoryReservationReleaseRequest,
    ) -> Result<InventoryReservationReleaseSnapshot, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InventoryAvailabilityRequest {
    pub variant_id: Uuid,
    pub requested_quantity: i32,
    pub channel_slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InventoryReservationRequest {
    pub variant_id: Uuid,
    pub quantity: i32,
    pub cart_id: Option<Uuid>,
    pub line_item_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InventoryReservationReleaseRequest {
    pub variant_id: Uuid,
    pub quantity: i32,
    pub order_id: Option<Uuid>,
    pub line_item_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InventoryAvailabilitySnapshot {
    pub variant_id: Uuid,
    pub requested_quantity: i32,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InventoryReservationSnapshot {
    pub variant_id: Uuid,
    pub reserved_quantity: i32,
    pub available_quantity: i32,
    pub in_stock: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InventoryReservationReleaseSnapshot {
    pub variant_id: Uuid,
    pub released_quantity: i32,
    pub available_quantity: i32,
    pub in_stock: bool,
}

#[async_trait]
impl InventoryReservationPort for crate::InventoryService {
    async fn check_availability(
        &self,
        context: PortContext,
        request: InventoryAvailabilityRequest,
    ) -> Result<InventoryAvailabilitySnapshot, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context)?;
        let result = self
            .check_variant_availability_for_channel(
                tenant_id,
                request.variant_id,
                request.requested_quantity,
                request.channel_slug.as_deref(),
            )
            .await
            .map_err(inventory_error_to_port_error)?;

        Ok(InventoryAvailabilitySnapshot {
            variant_id: request.variant_id,
            requested_quantity: request.requested_quantity,
            available: result.available,
        })
    }

    async fn reserve_inventory(
        &self,
        context: PortContext,
        request: InventoryReservationRequest,
    ) -> Result<InventoryReservationSnapshot, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_port_tenant_id(&context)?;
        let result = self
            .reserve(tenant_id, request.variant_id, request.quantity)
            .await
            .map_err(inventory_error_to_port_error)?;

        Ok(InventoryReservationSnapshot {
            variant_id: request.variant_id,
            reserved_quantity: result.reserved_quantity,
            available_quantity: result.available_quantity,
            in_stock: result.in_stock,
        })
    }

    async fn release_inventory_reservation(
        &self,
        context: PortContext,
        request: InventoryReservationReleaseRequest,
    ) -> Result<InventoryReservationReleaseSnapshot, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_port_tenant_id(&context)?;
        let result = self
            .release_reservation_quantity(tenant_id, request.variant_id, request.quantity)
            .await
            .map_err(inventory_error_to_port_error)?;

        Ok(InventoryReservationReleaseSnapshot {
            variant_id: request.variant_id,
            released_quantity: result.released_quantity,
            available_quantity: result.available_quantity,
            in_stock: result.in_stock,
        })
    }
}

fn parse_port_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "inventory.tenant_id_invalid",
            "PortContext.tenant_id must be a UUID for inventory ports",
        )
    })
}

fn inventory_error_to_port_error(
    error: rustok_commerce_foundation::error::CommerceError,
) -> PortError {
    use rustok_commerce_foundation::error::CommerceError;

    match error {
        CommerceError::Database(error) => PortError::unavailable(
            "inventory.database_unavailable",
            format!("inventory storage unavailable: {error}"),
        ),
        CommerceError::VariantNotFound(id) => PortError::new(
            rustok_api::PortErrorKind::NotFound,
            "inventory.variant_not_found",
            format!("variant {id} not found"),
            false,
        ),
        CommerceError::InsufficientInventory {
            requested,
            available,
        } => PortError::new(
            rustok_api::PortErrorKind::Conflict,
            "inventory.insufficient_inventory",
            format!("insufficient inventory: requested {requested}, available {available}"),
            false,
        ),
        CommerceError::InvalidPrice(message) | CommerceError::Validation(message) => {
            PortError::validation("inventory.validation", message)
        }
        other => PortError::new(
            rustok_api::PortErrorKind::InvariantViolation,
            "inventory.invariant_violation",
            format!("inventory operation failed: {other}"),
            false,
        ),
    }
}
