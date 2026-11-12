use crate::reservation_events::ReservationEventPlan;
use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use rustok_commerce_foundation::entities::{
    inventory_item, inventory_level, reservation_item, stock_location,
};
use rustok_commerce_foundation::error::CommerceError;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait,
    sea_query::{Expr, ExprTrait},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use uuid::Uuid;

const MAX_RESERVATION_EXTERNAL_ID_LENGTH: usize = 191;
const MAX_RESERVATION_CANDIDATE_LEVELS: usize = 4096;
const INVENTORY_PORT_BOUNDARY: &str = "inventory_reservation_port";

/// Transport-neutral owner boundary for checkout/storefront inventory availability and legacy
/// quantity-based reservations.
///
/// New reservation consumers must use [`InventoryReservationIdentityPort`]. The quantity-based
/// mutation methods remain temporarily for the proved anonymous compatibility pool only. They
/// refuse Cart/Order/line selectors and cannot mutate keyed allocations. These operations are
/// non-idempotent: a transport key is not a durable receipt or permission for automatic replay.
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

/// Owner boundary for durable, replay-safe inventory reservations.
///
/// `reservation_id` and `external_id` together identify one logical reservation. Repeating the
/// same reserve request returns the existing reservation. Reusing either identity for another
/// variant, quantity, line item or location fails closed. Release only affects the exact row.
/// Initial allocation honors the effective `PortContext.channel` visibility of stock locations.
/// A matching replay returns its original allocation, not a new eligibility proof or a second
/// allocation. In particular, release/replay never relocates stock after visibility changes.
#[async_trait]
pub trait InventoryReservationIdentityPort: Send + Sync {
    async fn reserve_inventory_by_identity(
        &self,
        context: PortContext,
        request: InventoryIdentityReservationRequest,
    ) -> Result<InventoryIdentityReservationSnapshot, PortError>;

    async fn release_inventory_by_identity(
        &self,
        context: PortContext,
        request: InventoryIdentityReservationReleaseRequest,
    ) -> Result<InventoryIdentityReservationReleaseSnapshot, PortError>;

    /// Atomically rebinds existing checkout allocations to their immutable Order lines.
    /// This does not allocate, release, consume or relocate stock. Exact replay is a no-op.
    async fn adopt_inventory_by_identity(
        &self,
        context: PortContext,
        request: InventoryIdentityReservationAdoptionRequest,
    ) -> Result<InventoryIdentityReservationAdoptionSnapshot, PortError>;
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
    /// Nonempty identity selectors refuse; use InventoryReservationIdentityPort instead.
    pub cart_id: Option<Uuid>,
    /// Nonempty identity selectors refuse; this command only targets the anonymous pool.
    pub line_item_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InventoryReservationReleaseRequest {
    pub variant_id: Uuid,
    pub quantity: i32,
    /// Nonempty identity selectors refuse; use InventoryReservationIdentityPort instead.
    pub order_id: Option<Uuid>,
    /// Nonempty identity selectors refuse; no keyed allocation is released by quantity.
    pub line_item_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InventoryIdentityReservationRequest {
    pub reservation_id: Uuid,
    pub external_id: String,
    pub variant_id: Uuid,
    pub quantity: i32,
    pub line_item_id: Option<Uuid>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InventoryIdentityReservationReleaseRequest {
    pub reservation_id: Uuid,
    pub external_id: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InventoryIdentityReservationSnapshot {
    pub reservation_id: Uuid,
    pub external_id: String,
    pub variant_id: Uuid,
    pub location_id: Uuid,
    pub reserved_quantity: i32,
    pub available_quantity: i32,
    pub in_stock: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InventoryIdentityReservationReleaseSnapshot {
    pub reservation_id: Uuid,
    pub external_id: String,
    pub variant_id: Uuid,
    pub location_id: Uuid,
    pub released_quantity: i32,
    pub available_quantity: i32,
    pub in_stock: bool,
}

/// The owner-local adoption batch is bounded independently of any transport envelope.
pub const MAX_RESERVATION_ADOPTION_ITEMS: usize = 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InventoryIdentityReservationAdoptionItem {
    pub reservation_id: Uuid,
    pub external_id: String,
    pub variant_id: Uuid,
    pub quantity: i32,
    /// The saved allocation location, not a location selector or a relocation request.
    pub location_id: Uuid,
    pub cart_line_item_id: Uuid,
    pub order_line_item_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InventoryIdentityReservationAdoptionRequest {
    pub checkout_operation_id: Uuid,
    pub cart_id: Uuid,
    pub order_id: Uuid,
    pub items: Vec<InventoryIdentityReservationAdoptionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InventoryIdentityReservationAdoptionSnapshot {
    pub checkout_operation_id: Uuid,
    pub cart_id: Uuid,
    pub order_id: Uuid,
    /// Complete validated bindings, in request order, returned only after owner commit.
    pub items: Vec<InventoryIdentityReservationAdoptionItem>,
}

#[derive(Clone)]
pub struct PersistentInventoryReservationIdentityPort {
    db: DatabaseConnection,
}

impl PersistentInventoryReservationIdentityPort {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl InventoryReservationPort for crate::InventoryService {
    async fn check_availability(
        &self,
        context: PortContext,
        request: InventoryAvailabilityRequest,
    ) -> Result<InventoryAvailabilitySnapshot, PortError> {
        let owner_operation = "check_availability";
        tracing::debug!(
            target: "rustok_inventory::ports",
            operation = owner_operation,
            variant_id_non_nil = !request.variant_id.is_nil(),
            quantity_negative = request.requested_quantity < 0,
            channel_present = request.channel_slug.is_some(),
            "check_availability port request received"
        );
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        validate_non_nil_uuid(request.variant_id, "variant_id")?;
        if request.requested_quantity < 0 {
            return Err(PortError::validation(
                "inventory.requested_quantity_negative",
                "requested quantity must be non-negative",
            ));
        }
        let result = self
            .check_variant_availability_for_channel(
                tenant_id,
                request.variant_id,
                request.requested_quantity,
                inventory_request_channel(&context, request.channel_slug.as_deref())?.as_deref(),
            )
            .await
            .map_err(|error| inventory_error_to_port_error(&context, owner_operation, error))?;

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
        let owner_operation = "reserve_inventory";
        tracing::debug!(
            target: "rustok_inventory::ports",
            operation = owner_operation,
            variant_id_non_nil = !request.variant_id.is_nil(),
            quantity_negative = request.quantity < 0,
            cart_id_present = request.cart_id.is_some(),
            line_item_id_present = request.line_item_id.is_some(),
            "reserve_inventory port request received"
        );
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        parse_port_tenant_id(&context, owner_operation)?;
        validate_non_nil_uuid(request.variant_id, "variant_id")?;
        if let Some(cart_id) = request.cart_id
            && cart_id.is_nil()
        {
            return Err(PortError::validation(
                "inventory.cart_id_nil",
                "cart_id cannot be nil",
            ));
        }
        if let Some(line_item_id) = request.line_item_id
            && line_item_id.is_nil()
        {
            return Err(PortError::validation(
                "inventory.line_item_id_nil",
                "line_item_id cannot be nil",
            ));
        }
        if request.quantity < 0 {
            return Err(PortError::validation(
                "inventory.reservation_quantity_invalid",
                "reservation quantity must be non-negative",
            ));
        }
        require_anonymous_quantity_scope(
            request.cart_id.is_some(),
            request.line_item_id.is_some(),
        )?;
        let result = self
            .reserve(&context, request.variant_id, request.quantity)
            .await
            .map_err(|error| inventory_error_to_port_error(&context, owner_operation, error))?;

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
        let owner_operation = "release_inventory_reservation";
        tracing::debug!(
            target: "rustok_inventory::ports",
            operation = owner_operation,
            variant_id_non_nil = !request.variant_id.is_nil(),
            quantity_negative = request.quantity < 0,
            order_id_present = request.order_id.is_some(),
            line_item_id_present = request.line_item_id.is_some(),
            "release_inventory_reservation port request received"
        );
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        parse_port_tenant_id(&context, owner_operation)?;
        validate_non_nil_uuid(request.variant_id, "variant_id")?;
        if let Some(order_id) = request.order_id
            && order_id.is_nil()
        {
            return Err(PortError::validation(
                "inventory.order_id_nil",
                "order_id cannot be nil",
            ));
        }
        if let Some(line_item_id) = request.line_item_id
            && line_item_id.is_nil()
        {
            return Err(PortError::validation(
                "inventory.line_item_id_nil",
                "line_item_id cannot be nil",
            ));
        }
        if request.quantity < 0 {
            return Err(PortError::validation(
                "inventory.reservation_release_quantity_invalid",
                "reservation release quantity must be non-negative",
            ));
        }
        require_anonymous_quantity_scope(
            request.order_id.is_some(),
            request.line_item_id.is_some(),
        )?;
        let result = self
            .release_reservation_quantity(&context, request.variant_id, request.quantity)
            .await
            .map_err(|error| inventory_error_to_port_error(&context, owner_operation, error))?;

        Ok(InventoryReservationReleaseSnapshot {
            variant_id: request.variant_id,
            released_quantity: result.released_quantity,
            available_quantity: result.available_quantity,
            in_stock: result.in_stock,
        })
    }
}

fn require_anonymous_quantity_scope(
    parent_present: bool,
    line_present: bool,
) -> Result<(), PortError> {
    if parent_present || line_present {
        return Err(PortError::validation(
            "inventory.reservation_identity_required",
            "quantity-only reservations cannot carry Cart/Order/line selectors; use InventoryReservationIdentityPort",
        ));
    }
    Ok(())
}

#[async_trait]
impl InventoryReservationIdentityPort for PersistentInventoryReservationIdentityPort {
    async fn adopt_inventory_by_identity(
        &self,
        context: PortContext,
        request: InventoryIdentityReservationAdoptionRequest,
    ) -> Result<InventoryIdentityReservationAdoptionSnapshot, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        crate::reservation_adoption::adopt_inventory_by_identity(&self.db, context, request).await
    }

    async fn reserve_inventory_by_identity(
        &self,
        context: PortContext,
        mut request: InventoryIdentityReservationRequest,
    ) -> Result<InventoryIdentityReservationSnapshot, PortError> {
        let owner_operation = "reserve_inventory_by_identity";
        tracing::debug!(
            target: "rustok_inventory::ports",
            operation = owner_operation,
            reservation_id_non_nil = !request.reservation_id.is_nil(),
            external_id_length = request.external_id.chars().count(),
            variant_id_non_nil = !request.variant_id.is_nil(),
            quantity_negative = request.quantity < 0,
            line_item_id_present = request.line_item_id.is_some(),
            "reserve_inventory_by_identity port request received"
        );
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        validate_non_nil_uuid(request.reservation_id, "reservation_id")?;
        validate_non_nil_uuid(request.variant_id, "variant_id")?;
        if let Some(line_item_id) = request.line_item_id
            && line_item_id.is_nil()
        {
            return Err(PortError::validation(
                "inventory.line_item_id_nil",
                "line_item_id cannot be nil",
            ));
        }
        request.external_id = normalize_external_id(request.external_id)?;
        if request.quantity <= 0 {
            return Err(PortError::validation(
                "inventory.reservation_quantity_invalid",
                "reservation quantity must be positive",
            ));
        }

        let txn =
            self.db.begin().await.map_err(|error| {
                storage_unavailable_with_context(&context, owner_operation, error)
            })?;
        let variant = load_tenant_variant(
            &context,
            owner_operation,
            &txn,
            tenant_id,
            request.variant_id,
        )
        .await?;
        let inventory_item =
            load_inventory_item_for_update(&context, owner_operation, &txn, request.variant_id)
                .await?
                .ok_or_else(|| {
                    PortError::conflict(
                        "inventory.state_not_found",
                        "variant has no configured inventory state",
                    )
                })?;

        if let Some(existing) = reservation_item::Entity::find_by_id(request.reservation_id)
            .one(&txn)
            .await
            .map_err(|error| storage_unavailable_with_context(&context, owner_operation, error))?
        {
            let snapshot = existing_reservation_snapshot(
                &context,
                owner_operation,
                &txn,
                &variant,
                &inventory_item,
                existing,
                &request,
            )
            .await?;
            txn.commit().await.map_err(|error| {
                storage_unavailable_with_context(&context, owner_operation, error)
            })?;
            return Ok(snapshot);
        }

        if let Some(existing) = find_reservation_by_external_id(
            &context,
            owner_operation,
            &txn,
            inventory_item.id,
            request.external_id.as_str(),
        )
        .await?
        {
            let snapshot = existing_reservation_snapshot(
                &context,
                owner_operation,
                &txn,
                &variant,
                &inventory_item,
                existing,
                &request,
            )
            .await?;
            txn.commit().await.map_err(|error| {
                storage_unavailable_with_context(&context, owner_operation, error)
            })?;
            return Ok(snapshot);
        }

        let event_plan = ReservationEventPlan::prepare(&txn, &context, owner_operation, &variant)
            .await
            .map_err(|error| inventory_error_to_port_error(&context, owner_operation, error))?;

        // Receipts above win over current presentation/visibility changes. Only a NEW
        // allocation selects locations; it must never consume another channel's hidden stock.
        let channel_slug = inventory_request_channel(&context, None)?;
        let levels = inventory_level::Entity::find()
            .filter(inventory_level::Column::InventoryItemId.eq(inventory_item.id))
            .limit((MAX_RESERVATION_CANDIDATE_LEVELS + 1) as u64)
            .all(&txn)
            .await
            .map_err(|error| storage_unavailable_with_context(&context, owner_operation, error))?;
        if levels.len() > MAX_RESERVATION_CANDIDATE_LEVELS {
            return Err(PortError::conflict(
                "inventory.reservation_level_limit",
                "SKU inventory exceeds 4096 candidate levels; an inventory operator must narrow its allocation layout",
            ));
        }
        let active_location_ids = stock_location::Entity::find()
            .filter(stock_location::Column::TenantId.eq(tenant_id))
            .filter(stock_location::Column::DeletedAt.is_null())
            .filter(stock_location::Column::Id.is_in(levels.iter().map(|level| level.location_id)))
            .all(&txn)
            .await
            .map_err(|error| storage_unavailable_with_context(&context, owner_operation, error))?
            .into_iter()
            .filter(|location| {
                crate::is_metadata_visible_for_public_channel(
                    &location.metadata,
                    channel_slug.as_deref(),
                )
            })
            .map(|location| location.id)
            .collect::<HashSet<_>>();
        let mut levels = levels
            .into_iter()
            .filter(|level| active_location_ids.contains(&level.location_id))
            .collect::<Vec<_>>();
        levels.sort_by(|left, right| {
            level_available(right)
                .cmp(&level_available(left))
                .then_with(|| left.id.cmp(&right.id))
        });

        let allows_backorder = variant.allows_backorder();
        let mut selected = None;
        for level in levels {
            let new_reserved = level
                .reserved_quantity
                .checked_add(request.quantity)
                .filter(|_| level.reserved_quantity >= 0 && level.incoming_quantity >= 0)
                .ok_or_else(|| {
                    PortError::invariant_violation(
                        "inventory.reservation_quantity_unrepresentable",
                        "reservation counters are inconsistent or exceed the supported range",
                    )
                })?;
            let mut update = inventory_level::Entity::update_many()
                .col_expr(
                    inventory_level::Column::ReservedQuantity,
                    Expr::value(new_reserved),
                )
                .col_expr(
                    inventory_level::Column::UpdatedAt,
                    Expr::current_timestamp(),
                )
                .filter(inventory_level::Column::Id.eq(level.id))
                .filter(inventory_level::Column::InventoryItemId.eq(inventory_item.id))
                .filter(inventory_level::Column::LocationId.eq(level.location_id))
                .filter(inventory_level::Column::ReservedQuantity.eq(level.reserved_quantity));
            if !allows_backorder {
                update = update.filter(
                    Expr::col(inventory_level::Column::StockedQuantity)
                        .sub(Expr::col(inventory_level::Column::ReservedQuantity))
                        .gte(request.quantity),
                );
            }
            if update
                .exec(&txn)
                .await
                .map_err(|error| {
                    storage_unavailable_with_context(&context, owner_operation, error)
                })?
                .rows_affected
                == 1
            {
                selected = Some(level);
                break;
            }
        }

        let selected = selected.ok_or_else(|| {
            PortError::new(
                PortErrorKind::Conflict,
                "inventory.insufficient_inventory",
                "insufficient inventory for reservation",
                false,
            )
        })?;
        let now = Utc::now();
        let mut metadata = reservation_metadata(
            request.metadata.clone(),
            request.reservation_id,
            request.external_id.as_str(),
            request.variant_id,
        );
        metadata["allocated_channel_slug"] = channel_slug.map(Value::String).unwrap_or(Value::Null);
        let inserted = reservation_item::Entity::insert(reservation_item::ActiveModel {
            id: Set(request.reservation_id),
            inventory_item_id: Set(inventory_item.id),
            location_id: Set(selected.location_id),
            quantity: Set(request.quantity),
            line_item_id: Set(request.line_item_id),
            description: Set(Some("Identity inventory reservation".to_string())),
            external_id: Set(Some(request.external_id.clone())),
            metadata: Set(metadata),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            deleted_at: Set(None),
        })
        .exec_without_returning(&txn)
        .await;

        if inserted.as_ref().is_ok_and(|affected| *affected != 1) {
            return Err(PortError::invariant_violation(
                "inventory.reservation_persistence_mismatch",
                "reservation row was not persisted exactly once; the transaction is refused",
            ));
        }

        if let Err(error) = inserted {
            txn.rollback().await.map_err(|error| {
                storage_unavailable_with_context(&context, owner_operation, error)
            })?;
            let existing = match reservation_item::Entity::find_by_id(request.reservation_id)
                .one(&self.db)
                .await
                .map_err(|error| {
                    storage_unavailable_with_context(&context, owner_operation, error)
                })? {
                Some(existing) => Some(existing),
                None => {
                    find_reservation_by_external_id(
                        &context,
                        owner_operation,
                        &self.db,
                        inventory_item.id,
                        request.external_id.as_str(),
                    )
                    .await?
                }
            };
            if let Some(existing) = existing {
                return existing_reservation_snapshot(
                    &context,
                    owner_operation,
                    &self.db,
                    &variant,
                    &inventory_item,
                    existing,
                    &request,
                )
                .await;
            }
            return Err(storage_unavailable_with_context(
                &context,
                owner_operation,
                error,
            ));
        }

        let available_quantity =
            available_quantity(&context, owner_operation, &txn, inventory_item.id).await?;
        event_plan
            .publish(&txn)
            .await
            .map_err(|error| inventory_error_to_port_error(&context, owner_operation, error))?;
        txn.commit()
            .await
            .map_err(|error| storage_unavailable_with_context(&context, owner_operation, error))?;
        Ok(InventoryIdentityReservationSnapshot {
            reservation_id: request.reservation_id,
            external_id: request.external_id,
            variant_id: request.variant_id,
            location_id: selected.location_id,
            reserved_quantity: request.quantity,
            available_quantity,
            in_stock: available_quantity > 0 || allows_backorder,
        })
    }

    async fn release_inventory_by_identity(
        &self,
        context: PortContext,
        mut request: InventoryIdentityReservationReleaseRequest,
    ) -> Result<InventoryIdentityReservationReleaseSnapshot, PortError> {
        let owner_operation = "release_inventory_by_identity";
        tracing::debug!(
            target: "rustok_inventory::ports",
            operation = owner_operation,
            reservation_id_non_nil = !request.reservation_id.is_nil(),
            external_id_length = request.external_id.chars().count(),
            "release_inventory_by_identity port request received"
        );
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        validate_non_nil_uuid(request.reservation_id, "reservation_id")?;
        request.external_id = normalize_external_id(request.external_id)?;

        let txn =
            self.db.begin().await.map_err(|error| {
                storage_unavailable_with_context(&context, owner_operation, error)
            })?;
        let observed = reservation_item::Entity::find_by_id(request.reservation_id)
            .one(&txn)
            .await
            .map_err(|error| storage_unavailable_with_context(&context, owner_operation, error))?
            .ok_or_else(|| {
                PortError::not_found(
                    "inventory.reservation_not_found",
                    "inventory reservation was not found",
                )
            })?;
        if observed.external_id.as_deref() != Some(request.external_id.as_str()) {
            return Err(PortError::conflict(
                "inventory.reservation_identity_conflict",
                "reservation id is bound to another external identity",
            ));
        }
        let item = load_inventory_item_by_id_for_update(
            &context,
            owner_operation,
            &txn,
            observed.inventory_item_id,
        )
        .await?
        .ok_or_else(|| {
            PortError::invariant_violation(
                "inventory.reservation_item_missing",
                "reservation inventory item is missing",
            )
        })?;
        let reservation = reservation_item::Entity::find_by_id(request.reservation_id)
            .one(&txn)
            .await
            .map_err(|error| storage_unavailable_with_context(&context, owner_operation, error))?
            .ok_or_else(|| {
                PortError::not_found(
                    "inventory.reservation_not_found",
                    "inventory reservation was not found",
                )
            })?;
        if reservation.inventory_item_id != item.id
            || reservation.external_id.as_deref() != Some(request.external_id.as_str())
        {
            return Err(PortError::conflict(
                "inventory.reservation_identity_conflict",
                "reservation identity changed while acquiring the owner lock",
            ));
        }
        let variant =
            load_tenant_variant(&context, owner_operation, &txn, tenant_id, item.variant_id)
                .await?;
        let allows_backorder = variant.allows_backorder();

        if reservation.deleted_at.is_some() || reservation.quantity == 0 {
            let available_quantity =
                available_quantity(&context, owner_operation, &txn, item.id).await?;
            txn.commit().await.map_err(|error| {
                storage_unavailable_with_context(&context, owner_operation, error)
            })?;
            return Ok(InventoryIdentityReservationReleaseSnapshot {
                reservation_id: reservation.id,
                external_id: request.external_id,
                variant_id: variant.variant_id,
                location_id: reservation.location_id,
                released_quantity: 0,
                available_quantity,
                in_stock: available_quantity > 0 || allows_backorder,
            });
        }

        if reservation.quantity < 0 {
            return Err(PortError::invariant_violation(
                "inventory.reservation_ledger_inconsistent",
                "reservation quantity is inconsistent; reconcile the original identity",
            ));
        }
        let event_plan = ReservationEventPlan::prepare(&txn, &context, owner_operation, &variant)
            .await
            .map_err(|error| inventory_error_to_port_error(&context, owner_operation, error))?;
        let released_quantity = reservation.quantity;
        let level_update = inventory_level::Entity::update_many()
            .col_expr(
                inventory_level::Column::ReservedQuantity,
                Expr::col(inventory_level::Column::ReservedQuantity).sub(released_quantity),
            )
            .col_expr(
                inventory_level::Column::UpdatedAt,
                Expr::current_timestamp(),
            )
            .filter(inventory_level::Column::InventoryItemId.eq(item.id))
            .filter(inventory_level::Column::LocationId.eq(reservation.location_id))
            .filter(inventory_level::Column::ReservedQuantity.gte(released_quantity))
            .exec(&txn)
            .await
            .map_err(|error| storage_unavailable_with_context(&context, owner_operation, error))?;
        if level_update.rows_affected != 1 {
            return Err(PortError::invariant_violation(
                "inventory.reservation_ledger_inconsistent",
                "inventory reservation ledger is inconsistent",
            ));
        }

        let mut metadata = reservation.metadata.clone();
        if let Value::Object(object) = &mut metadata {
            object.insert("inventory_disposition".to_string(), Value::from("released"));
            object.insert(
                "released_at".to_string(),
                Value::from(Utc::now().to_rfc3339()),
            );
        }
        let now = Utc::now();
        let updated = reservation_item::Entity::update_many()
            .col_expr(reservation_item::Column::Quantity, Expr::value(0))
            .col_expr(reservation_item::Column::Metadata, Expr::value(metadata))
            .col_expr(reservation_item::Column::UpdatedAt, Expr::value(now))
            .col_expr(reservation_item::Column::DeletedAt, Expr::value(now))
            .filter(reservation_item::Column::Id.eq(reservation.id))
            .filter(reservation_item::Column::InventoryItemId.eq(item.id))
            .filter(reservation_item::Column::LocationId.eq(reservation.location_id))
            .filter(reservation_item::Column::ExternalId.eq(request.external_id.as_str()))
            .filter(reservation_item::Column::Quantity.eq(released_quantity))
            .filter(reservation_item::Column::DeletedAt.is_null())
            .exec(&txn)
            .await
            .map_err(|error| storage_unavailable_with_context(&context, owner_operation, error))?;
        if updated.rows_affected != 1 {
            return Err(PortError::invariant_violation(
                "inventory.reservation_persistence_mismatch",
                "reservation release was not persisted exactly once; the transaction is refused",
            ));
        }

        let available_quantity =
            available_quantity(&context, owner_operation, &txn, item.id).await?;
        event_plan
            .publish(&txn)
            .await
            .map_err(|error| inventory_error_to_port_error(&context, owner_operation, error))?;
        txn.commit()
            .await
            .map_err(|error| storage_unavailable_with_context(&context, owner_operation, error))?;
        Ok(InventoryIdentityReservationReleaseSnapshot {
            reservation_id: reservation.id,
            external_id: request.external_id,
            variant_id: variant.id(),
            location_id: reservation.location_id,
            released_quantity,
            available_quantity,
            in_stock: available_quantity > 0 || allows_backorder,
        })
    }
}

/// An explicit selector and trusted context must agree. Absence borrows the trusted
/// context; a present blank must not downgrade a scoped request to global inventory.
fn inventory_request_channel(
    context: &PortContext,
    selected: Option<&str>,
) -> Result<Option<String>, PortError> {
    let trusted = crate::normalize_public_channel_slug(context.channel.as_deref());
    let selected_normalized = crate::normalize_public_channel_slug(selected);
    if (context.channel.is_some() && trusted.is_none())
        || (selected.is_some() && selected_normalized.is_none())
    {
        return Err(PortError::validation(
            "inventory.channel_invalid",
            "Inventory channel must not be blank",
        ));
    }
    if let (Some(trusted), Some(selected)) = (&trusted, &selected_normalized)
        && trusted != selected
    {
        return Err(PortError::validation(
            "inventory.channel_mismatch",
            "Inventory selector and request context must identify the same channel",
        ));
    }
    Ok(selected_normalized.or(trusted))
}

pub(super) async fn load_tenant_variant<C>(
    context: &PortContext,
    owner_operation: &'static str,
    conn: &C,
    tenant_id: Uuid,
    variant_id: Uuid,
) -> Result<crate::product_port::InventoryProductVariantInfo, PortError>
where
    C: ConnectionTrait,
{
    crate::product_port::find_variant_info_from_conn(conn, tenant_id, variant_id)
        .await
        .map_err(|error| storage_unavailable_with_context(context, owner_operation, error))?
        .ok_or_else(|| {
            let facts = InventoryOwnerErrorFacts::uuid("variant_not_found", variant_id);
            log_inventory_port_failure(
                context,
                owner_operation,
                "inventory.variant_not_found",
                &facts,
                false,
            );
            PortError::not_found(
                "inventory.variant_not_found",
                "inventory variant was not found",
            )
        })
}

pub(super) async fn load_inventory_item_for_update<C>(
    context: &PortContext,
    owner_operation: &'static str,
    conn: &C,
    variant_id: Uuid,
) -> Result<Option<inventory_item::Model>, PortError>
where
    C: ConnectionTrait,
{
    crate::services::inventory::inventory_item_write_query(conn, variant_id)
        .one(conn)
        .await
        .map_err(|error| storage_unavailable_with_context(context, owner_operation, error))
}

async fn load_inventory_item_by_id_for_update<C>(
    context: &PortContext,
    owner_operation: &'static str,
    conn: &C,
    inventory_item_id: Uuid,
) -> Result<Option<inventory_item::Model>, PortError>
where
    C: ConnectionTrait,
{
    let query = || inventory_item::Entity::find_by_id(inventory_item_id);
    match conn.get_database_backend() {
        DbBackend::Sqlite => query()
            .one(conn)
            .await
            .map_err(|error| storage_unavailable_with_context(context, owner_operation, error)),
        _ => query()
            .lock_exclusive()
            .one(conn)
            .await
            .map_err(|error| storage_unavailable_with_context(context, owner_operation, error)),
    }
}

async fn find_reservation_by_external_id<C>(
    context: &PortContext,
    owner_operation: &'static str,
    conn: &C,
    inventory_item_id: Uuid,
    external_id: &str,
) -> Result<Option<reservation_item::Model>, PortError>
where
    C: ConnectionTrait,
{
    reservation_item::Entity::find()
        .filter(reservation_item::Column::InventoryItemId.eq(inventory_item_id))
        .filter(reservation_item::Column::ExternalId.eq(external_id))
        .order_by_desc(reservation_item::Column::CreatedAt)
        .one(conn)
        .await
        .map_err(|error| storage_unavailable_with_context(context, owner_operation, error))
}

async fn existing_reservation_snapshot<C>(
    context: &PortContext,
    owner_operation: &'static str,
    conn: &C,
    variant: &crate::product_port::InventoryProductVariantInfo,
    item: &inventory_item::Model,
    existing: reservation_item::Model,
    request: &InventoryIdentityReservationRequest,
) -> Result<InventoryIdentityReservationSnapshot, PortError>
where
    C: ConnectionTrait,
{
    if existing.id != request.reservation_id
        || existing.inventory_item_id != item.id
        || existing.external_id.as_deref() != Some(request.external_id.as_str())
        || existing.quantity != request.quantity
        || existing.line_item_id != request.line_item_id
        || existing.deleted_at.is_some()
    {
        return Err(PortError::conflict(
            "inventory.reservation_identity_conflict",
            "reservation identity is already bound to different reservation data",
        ));
    }
    let available_quantity = available_quantity(context, owner_operation, conn, item.id).await?;
    Ok(InventoryIdentityReservationSnapshot {
        reservation_id: existing.id,
        external_id: request.external_id.clone(),
        variant_id: variant.variant_id,
        location_id: existing.location_id,
        reserved_quantity: existing.quantity,
        available_quantity,
        in_stock: available_quantity > 0 || variant.allows_backorder(),
    })
}

async fn available_quantity<C>(
    context: &PortContext,
    owner_operation: &'static str,
    conn: &C,
    inventory_item_id: Uuid,
) -> Result<i32, PortError>
where
    C: ConnectionTrait,
{
    let levels = inventory_level::Entity::find()
        .filter(inventory_level::Column::InventoryItemId.eq(inventory_item_id))
        .order_by_asc(inventory_level::Column::Id)
        .limit((MAX_RESERVATION_CANDIDATE_LEVELS + 1) as u64)
        .all(conn)
        .await
        .map_err(|error| storage_unavailable_with_context(context, owner_operation, error))?;
    // The quantity and identity APIs use one checked signed evaluator. Per-level clamping
    // could acknowledge an allocation whose real final availability is unrepresentable.
    crate::services::inventory::checked_available_quantity(&levels).map_err(|_| {
        PortError::invariant_violation(
            "inventory.available_quantity_overflow",
            "inventory available quantity or level layout is outside the supported range; reconcile saved state",
        )
    })
}

fn level_available(level: &inventory_level::Model) -> i64 {
    i64::from(level.stocked_quantity) - i64::from(level.reserved_quantity)
}

fn reservation_metadata(
    metadata: Value,
    reservation_id: Uuid,
    external_id: &str,
    variant_id: Uuid,
) -> Value {
    let mut object = match metadata {
        Value::Object(object) => object,
        _ => serde_json::Map::new(),
    };
    object.insert("source".to_string(), Value::from("inventory_identity_port"));
    object.insert(
        "reservation_id".to_string(),
        Value::from(reservation_id.to_string()),
    );
    object.insert("external_id".to_string(), Value::from(external_id));
    object.insert(
        "variant_id".to_string(),
        Value::from(variant_id.to_string()),
    );
    Value::Object(object)
}

pub(super) fn normalize_external_id(value: String) -> Result<String, PortError> {
    let value = value.trim().to_string();
    if value.is_empty() || value.chars().count() > MAX_RESERVATION_EXTERNAL_ID_LENGTH {
        return Err(PortError::validation(
            "inventory.reservation_external_id_invalid",
            format!(
                "reservation external_id must contain 1 to {MAX_RESERVATION_EXTERNAL_ID_LENGTH} characters"
            ),
        ));
    }
    Ok(value)
}

struct InventoryPortContextFacts {
    tenant_id_length: usize,
    actor_kind: &'static str,
    actor_id_length: usize,
    claim_count: usize,
    role_count: usize,
    channel_present: bool,
    channel_length: Option<usize>,
    locale_length: usize,
    causation_id_present: bool,
    causation_id_length: Option<usize>,
    traceparent_present: bool,
    traceparent_length: Option<usize>,
    idempotency_key_present: bool,
    idempotency_key_length: Option<usize>,
    deadline_ms: Option<u64>,
}

struct InventoryOwnerErrorFacts {
    error_variant: &'static str,
    text_field_count: usize,
    text_total_length: usize,
    uuid_field_count: usize,
    uuid_non_nil_count: usize,
    numeric_field_count: usize,
    numeric_nonzero_count: usize,
    numeric_negative_count: usize,
    opaque_payload_present: bool,
}

impl InventoryOwnerErrorFacts {
    fn empty(error_variant: &'static str) -> Self {
        Self {
            error_variant,
            text_field_count: 0,
            text_total_length: 0,
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            numeric_field_count: 0,
            numeric_nonzero_count: 0,
            numeric_negative_count: 0,
            opaque_payload_present: false,
        }
    }

    fn uuid(error_variant: &'static str, value: Uuid) -> Self {
        Self {
            uuid_field_count: 1,
            uuid_non_nil_count: usize::from(!value.is_nil()),
            ..Self::empty(error_variant)
        }
    }

    fn opaque(error_variant: &'static str) -> Self {
        Self {
            opaque_payload_present: true,
            ..Self::empty(error_variant)
        }
    }
}

fn inventory_port_context_facts(context: &PortContext) -> InventoryPortContextFacts {
    let actor_kind = match &context.actor.kind {
        rustok_api::PortActorKind::User => "user",
        rustok_api::PortActorKind::Service => "service",
        rustok_api::PortActorKind::System => "system",
    };
    InventoryPortContextFacts {
        tenant_id_length: context.tenant_id.chars().count(),
        actor_kind,
        actor_id_length: context.actor.id.chars().count(),
        claim_count: context.claims.len(),
        role_count: context.roles.len(),
        channel_present: context.channel.is_some(),
        channel_length: context.channel.as_ref().map(|value| value.chars().count()),
        locale_length: context.locale.chars().count(),
        causation_id_present: context.causation_id.is_some(),
        causation_id_length: context
            .causation_id
            .as_ref()
            .map(|value| value.chars().count()),
        traceparent_present: context.traceparent.is_some(),
        traceparent_length: context
            .traceparent
            .as_ref()
            .map(|value| value.chars().count()),
        idempotency_key_present: context.idempotency_key.is_some(),
        idempotency_key_length: context
            .idempotency_key
            .as_ref()
            .map(|value| value.chars().count()),
        deadline_ms: context.deadline_ms,
    }
}

fn inventory_owner_error_facts(error: &CommerceError) -> InventoryOwnerErrorFacts {
    match error {
        CommerceError::Database(_) => InventoryOwnerErrorFacts::opaque("database"),
        CommerceError::ProductNotFound(value) => {
            InventoryOwnerErrorFacts::uuid("product_not_found", *value)
        }
        CommerceError::VariantNotFound(value) => {
            InventoryOwnerErrorFacts::uuid("variant_not_found", *value)
        }
        CommerceError::DuplicateHandle { handle, locale } => InventoryOwnerErrorFacts {
            text_field_count: 2,
            text_total_length: handle.chars().count() + locale.chars().count(),
            ..InventoryOwnerErrorFacts::empty("duplicate_handle")
        },
        CommerceError::DuplicateSku(value) => InventoryOwnerErrorFacts {
            text_field_count: 1,
            text_total_length: value.chars().count(),
            ..InventoryOwnerErrorFacts::empty("duplicate_sku")
        },
        CommerceError::InvalidPrice(value) => InventoryOwnerErrorFacts {
            text_field_count: 1,
            text_total_length: value.chars().count(),
            ..InventoryOwnerErrorFacts::empty("invalid_price")
        },
        CommerceError::InsufficientInventory {
            requested,
            available,
        } => InventoryOwnerErrorFacts {
            numeric_field_count: 2,
            numeric_nonzero_count: usize::from(*requested != 0) + usize::from(*available != 0),
            numeric_negative_count: usize::from(*requested < 0) + usize::from(*available < 0),
            ..InventoryOwnerErrorFacts::empty("insufficient_inventory")
        },
        CommerceError::InvalidOptionCombination => {
            InventoryOwnerErrorFacts::empty("invalid_option_combination")
        }
        CommerceError::Validation(value) => InventoryOwnerErrorFacts {
            text_field_count: 1,
            text_total_length: value.chars().count(),
            ..InventoryOwnerErrorFacts::empty("validation")
        },
        CommerceError::ShippingProfileNotFound(value) => {
            InventoryOwnerErrorFacts::uuid("shipping_profile_not_found", *value)
        }
        CommerceError::DuplicateShippingProfileSlug(value) => InventoryOwnerErrorFacts {
            text_field_count: 1,
            text_total_length: value.chars().count(),
            ..InventoryOwnerErrorFacts::empty("duplicate_shipping_profile_slug")
        },
        CommerceError::NoVariants => InventoryOwnerErrorFacts::empty("no_variants"),
        CommerceError::CannotDeletePublished => {
            InventoryOwnerErrorFacts::empty("cannot_delete_published")
        }
        CommerceError::Rich(_) => InventoryOwnerErrorFacts::opaque("rich"),
        CommerceError::Core(_) => InventoryOwnerErrorFacts::opaque("core"),
    }
}

fn log_inventory_port_failure(
    context: &PortContext,
    owner_operation: &'static str,
    code: &'static str,
    error_facts: &InventoryOwnerErrorFacts,
    technical_failure: bool,
) {
    let context_facts = inventory_port_context_facts(context);
    if technical_failure {
        tracing::error!(
            owner = "rustok_inventory",
            correlation_id = %context.correlation_id,
            tenant_id_length = context_facts.tenant_id_length,
            actor_kind = context_facts.actor_kind,
            actor_id_length = context_facts.actor_id_length,
            claim_count = context_facts.claim_count,
            role_count = context_facts.role_count,
            channel_present = context_facts.channel_present,
            channel_length = ?context_facts.channel_length,
            locale_length = context_facts.locale_length,
            causation_id_present = context_facts.causation_id_present,
            causation_id_length = ?context_facts.causation_id_length,
            traceparent_present = context_facts.traceparent_present,
            traceparent_length = ?context_facts.traceparent_length,
            idempotency_key_present = context_facts.idempotency_key_present,
            idempotency_key_length = ?context_facts.idempotency_key_length,
            deadline_ms = ?context_facts.deadline_ms,
            operation = owner_operation,
            code,
            error_variant = error_facts.error_variant,
            text_field_count = error_facts.text_field_count,
            text_total_length = error_facts.text_total_length,
            uuid_field_count = error_facts.uuid_field_count,
            uuid_non_nil_count = error_facts.uuid_non_nil_count,
            numeric_field_count = error_facts.numeric_field_count,
            numeric_nonzero_count = error_facts.numeric_nonzero_count,
            numeric_negative_count = error_facts.numeric_negative_count,
            opaque_payload_present = error_facts.opaque_payload_present,
            boundary = INVENTORY_PORT_BOUNDARY,
            "inventory owner operation failed with bounded diagnostics"
        );
    } else {
        tracing::warn!(
            owner = "rustok_inventory",
            correlation_id = %context.correlation_id,
            tenant_id_length = context_facts.tenant_id_length,
            actor_kind = context_facts.actor_kind,
            actor_id_length = context_facts.actor_id_length,
            claim_count = context_facts.claim_count,
            role_count = context_facts.role_count,
            channel_present = context_facts.channel_present,
            channel_length = ?context_facts.channel_length,
            locale_length = context_facts.locale_length,
            causation_id_present = context_facts.causation_id_present,
            causation_id_length = ?context_facts.causation_id_length,
            traceparent_present = context_facts.traceparent_present,
            traceparent_length = ?context_facts.traceparent_length,
            idempotency_key_present = context_facts.idempotency_key_present,
            idempotency_key_length = ?context_facts.idempotency_key_length,
            deadline_ms = ?context_facts.deadline_ms,
            operation = owner_operation,
            code,
            error_variant = error_facts.error_variant,
            text_field_count = error_facts.text_field_count,
            text_total_length = error_facts.text_total_length,
            uuid_field_count = error_facts.uuid_field_count,
            uuid_non_nil_count = error_facts.uuid_non_nil_count,
            numeric_field_count = error_facts.numeric_field_count,
            numeric_nonzero_count = error_facts.numeric_nonzero_count,
            numeric_negative_count = error_facts.numeric_negative_count,
            opaque_payload_present = error_facts.opaque_payload_present,
            boundary = INVENTORY_PORT_BOUNDARY,
            "inventory owner operation was rejected with bounded diagnostics"
        );
    }
}

fn log_inventory_tenant_parse_rejection(context: &PortContext, owner_operation: &'static str) {
    let context_facts = inventory_port_context_facts(context);
    tracing::warn!(
        owner = "rustok_inventory",
        correlation_id = %context.correlation_id,
        tenant_id_length = context_facts.tenant_id_length,
        actor_kind = context_facts.actor_kind,
        actor_id_length = context_facts.actor_id_length,
        claim_count = context_facts.claim_count,
        role_count = context_facts.role_count,
        channel_present = context_facts.channel_present,
        channel_length = ?context_facts.channel_length,
        locale_length = context_facts.locale_length,
        causation_id_present = context_facts.causation_id_present,
        causation_id_length = ?context_facts.causation_id_length,
        traceparent_present = context_facts.traceparent_present,
        traceparent_length = ?context_facts.traceparent_length,
        idempotency_key_present = context_facts.idempotency_key_present,
        idempotency_key_length = ?context_facts.idempotency_key_length,
        deadline_ms = ?context_facts.deadline_ms,
        operation = owner_operation,
        code = "inventory.context_invalid",
        tenant_id_parse_failed = true,
        boundary = INVENTORY_PORT_BOUNDARY,
        "inventory port context was rejected with bounded diagnostics"
    );
}

pub(super) fn validate_non_nil_uuid(id: Uuid, field_name: &'static str) -> Result<(), PortError> {
    if id.is_nil() {
        return Err(PortError::validation(
            format!("inventory.{field_name}_nil"),
            format!("{field_name} cannot be nil"),
        ));
    }
    Ok(())
}

pub(super) fn parse_port_tenant_id(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<Uuid, PortError> {
    let tenant_id = Uuid::parse_str(context.tenant_id.trim()).map_err(|_| {
        log_inventory_tenant_parse_rejection(context, owner_operation);
        PortError::validation(
            "inventory.context_invalid",
            "inventory request context is invalid",
        )
    })?;
    if tenant_id.is_nil() {
        log_inventory_tenant_parse_rejection(context, owner_operation);
        return Err(PortError::validation(
            "inventory.tenant_id_nil",
            "inventory tenant id cannot be nil",
        ));
    }
    Ok(tenant_id)
}

pub(super) fn storage_unavailable_with_context(
    context: &PortContext,
    owner_operation: &'static str,
    _error: sea_orm::DbErr,
) -> PortError {
    let facts = InventoryOwnerErrorFacts::opaque("database");
    log_inventory_port_failure(
        context,
        owner_operation,
        "inventory.database_unavailable",
        &facts,
        true,
    );
    storage_unavailable()
}

pub(super) fn storage_unavailable() -> PortError {
    PortError::unavailable(
        "inventory.database_unavailable",
        "inventory storage is temporarily unavailable",
    )
}

fn inventory_error_to_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: CommerceError,
) -> PortError {
    let error_facts = inventory_owner_error_facts(&error);
    match error {
        CommerceError::Database(_) => {
            log_inventory_port_failure(
                context,
                owner_operation,
                "inventory.database_unavailable",
                &error_facts,
                true,
            );
            PortError::unavailable(
                "inventory.database_unavailable",
                "inventory storage is temporarily unavailable",
            )
        }
        CommerceError::VariantNotFound(_) => {
            log_inventory_port_failure(
                context,
                owner_operation,
                "inventory.variant_not_found",
                &error_facts,
                false,
            );
            PortError::new(
                PortErrorKind::NotFound,
                "inventory.variant_not_found",
                "inventory variant was not found",
                false,
            )
        }
        CommerceError::InsufficientInventory { .. } => {
            log_inventory_port_failure(
                context,
                owner_operation,
                "inventory.insufficient_inventory",
                &error_facts,
                false,
            );
            PortError::new(
                PortErrorKind::Conflict,
                "inventory.insufficient_inventory",
                "inventory reservation conflicts with available stock",
                false,
            )
        }
        CommerceError::InvalidPrice(_) | CommerceError::Validation(_) => {
            log_inventory_port_failure(
                context,
                owner_operation,
                "inventory.validation",
                &error_facts,
                false,
            );
            PortError::validation("inventory.validation", "inventory request is invalid")
        }
        _ => {
            log_inventory_port_failure(
                context,
                owner_operation,
                "inventory.invariant_violation",
                &error_facts,
                true,
            );
            PortError::invariant_violation(
                "inventory.invariant_violation",
                "inventory operation violated an owner invariant",
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn missing_product_schema_returns_the_public_inventory_storage_error() {
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let tenant_id = Uuid::new_v4();
        let context = PortContext::new(
            tenant_id.to_string(),
            rustok_api::PortActor::system(),
            "en",
            "test",
        );
        let error = load_tenant_variant(&context, "test", &db, tenant_id, Uuid::new_v4())
            .await
            .unwrap_err();
        assert_eq!(error.kind, PortErrorKind::Unavailable);
        assert_eq!(error.code, "inventory.database_unavailable");
        assert_eq!(
            error.message,
            "the requested capability is temporarily unavailable"
        );
    }

    #[test]
    fn reservation_external_id_is_trimmed_and_bounded() {
        assert_eq!(
            normalize_external_id("  checkout:1  ".to_string()).unwrap(),
            "checkout:1"
        );
        assert!(normalize_external_id(String::new()).is_err());
        assert!(normalize_external_id("x".repeat(MAX_RESERVATION_EXTERNAL_ID_LENGTH + 1)).is_err());
    }

    #[test]
    fn reservation_metadata_overrides_identity_fields() {
        let reservation_id = Uuid::new_v4();
        let variant_id = Uuid::new_v4();
        let metadata = reservation_metadata(
            serde_json::json!({"source": "caller", "reservation_id": "wrong"}),
            reservation_id,
            "checkout:line",
            variant_id,
        );
        assert_eq!(metadata["source"], "inventory_identity_port");
        assert_eq!(metadata["reservation_id"], reservation_id.to_string());
        assert_eq!(metadata["variant_id"], variant_id.to_string());
    }

    #[test]
    fn parse_port_tenant_id_rejects_empty_and_nil_uuid() {
        let empty_context = PortContext::new("", rustok_api::PortActor::system(), "en", "test");
        let err = parse_port_tenant_id(&empty_context, "test").unwrap_err();
        assert_eq!(err.code, "inventory.context_invalid");

        let nil_context = PortContext::new(
            Uuid::nil().to_string(),
            rustok_api::PortActor::system(),
            "en",
            "test",
        );
        let err = parse_port_tenant_id(&nil_context, "test").unwrap_err();
        assert_eq!(err.code, "inventory.tenant_id_nil");

        let valid_id = Uuid::new_v4();
        let valid_context = PortContext::new(
            valid_id.to_string(),
            rustok_api::PortActor::system(),
            "en",
            "test",
        );
        assert_eq!(
            parse_port_tenant_id(&valid_context, "test").unwrap(),
            valid_id
        );
    }

    #[test]
    fn validate_non_nil_uuid_rejects_nil() {
        let err = validate_non_nil_uuid(Uuid::nil(), "variant_id").unwrap_err();
        assert_eq!(err.code, "inventory.variant_id_nil");

        assert!(validate_non_nil_uuid(Uuid::new_v4(), "variant_id").is_ok());
    }

    #[test]
    fn inventory_context_is_inherited_only_when_no_explicit_selector_conflicts() {
        let context = PortContext::new(
            Uuid::new_v4().to_string(),
            rustok_api::PortActor::system(),
            "en",
            "channel-test",
        )
        .with_channel(" WEB ");
        assert_eq!(
            inventory_request_channel(&context, None)
                .unwrap()
                .as_deref(),
            Some("web")
        );
        assert_eq!(
            inventory_request_channel(&context, Some("web"))
                .unwrap()
                .as_deref(),
            Some("web")
        );
        assert_eq!(
            inventory_request_channel(&context, Some("mobile"))
                .unwrap_err()
                .code,
            "inventory.channel_mismatch"
        );
        assert!(inventory_request_channel(&context, Some(" ")).is_err());
        let mut global = context;
        global.channel = None;
        assert!(inventory_request_channel(&global, None).unwrap().is_none());
        assert_eq!(
            inventory_request_channel(&global, Some("WEB"))
                .unwrap()
                .as_deref(),
            Some("web")
        );
        global.channel = Some(String::new());
        assert!(inventory_request_channel(&global, None).is_err());
    }
}
