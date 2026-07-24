use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use rustok_commerce_foundation::entities::{
    inventory_item, inventory_level, product_variant, reservation_item, stock_location,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
    sea_query::{Expr, ExprTrait},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashSet, sync::Arc};
use uuid::Uuid;

const MAX_RESERVATION_EXTERNAL_ID_LENGTH: usize = 191;

/// Transport-neutral owner boundary for checkout/storefront inventory availability and legacy
/// quantity-based reservations.
///
/// New reservation consumers must use [`InventoryReservationIdentityPort`]. The quantity-based
/// mutation methods remain temporarily for compatibility and are unsafe when multiple consumers
/// reserve the same variant because release has no durable reservation identity.
#[async_trait]
pub trait InventoryReservationPort: Send + Sync {
    async fn check_availability(
        &self,
        context: PortContext,
        request: InventoryAvailabilityRequest,
    ) -> Result<InventoryAvailabilitySnapshot, PortError>;

    #[deprecated(note = "use InventoryReservationIdentityPort::reserve_inventory_by_identity")]
    async fn reserve_inventory(
        &self,
        context: PortContext,
        request: InventoryReservationRequest,
    ) -> Result<InventoryReservationSnapshot, PortError>;

    #[deprecated(note = "use InventoryReservationIdentityPort::release_inventory_by_identity")]
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

#[derive(Clone)]
pub struct PersistentInventoryReservationIdentityPort {
    db: DatabaseConnection,
}

impl PersistentInventoryReservationIdentityPort {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

pub fn in_process_inventory_reservation_identity_port(
    db: DatabaseConnection,
) -> Arc<dyn InventoryReservationIdentityPort> {
    Arc::new(PersistentInventoryReservationIdentityPort::new(db))
}

#[async_trait]
impl InventoryReservationPort for crate::InventoryService {
    async fn check_availability(
        &self,
        context: PortContext,
        request: InventoryAvailabilityRequest,
    ) -> Result<InventoryAvailabilitySnapshot, PortError> {
        let owner_operation = "check_availability";
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let result = self
            .check_variant_availability_for_channel(
                tenant_id,
                request.variant_id,
                request.requested_quantity,
                request.channel_slug.as_deref(),
            )
            .await
            .map_err(|error| inventory_error_to_port_error(&context, owner_operation, error))?;

        Ok(InventoryAvailabilitySnapshot {
            variant_id: request.variant_id,
            requested_quantity: request.requested_quantity,
            available: result.available,
        })
    }

    #[allow(deprecated)]
    async fn reserve_inventory(
        &self,
        context: PortContext,
        request: InventoryReservationRequest,
    ) -> Result<InventoryReservationSnapshot, PortError> {
        let owner_operation = "reserve_inventory";
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let result = self
            .reserve(tenant_id, request.variant_id, request.quantity)
            .await
            .map_err(|error| inventory_error_to_port_error(&context, owner_operation, error))?;

        Ok(InventoryReservationSnapshot {
            variant_id: request.variant_id,
            reserved_quantity: result.reserved_quantity,
            available_quantity: result.available_quantity,
            in_stock: result.in_stock,
        })
    }

    #[allow(deprecated)]
    async fn release_inventory_reservation(
        &self,
        context: PortContext,
        request: InventoryReservationReleaseRequest,
    ) -> Result<InventoryReservationReleaseSnapshot, PortError> {
        let owner_operation = "release_inventory_reservation";
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let result = self
            .release_reservation_quantity(tenant_id, request.variant_id, request.quantity)
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

#[async_trait]
impl InventoryReservationIdentityPort for PersistentInventoryReservationIdentityPort {
    async fn reserve_inventory_by_identity(
        &self,
        context: PortContext,
        mut request: InventoryIdentityReservationRequest,
    ) -> Result<InventoryIdentityReservationSnapshot, PortError> {
        let owner_operation = "reserve_inventory_by_identity";
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
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

        let active_location_ids = stock_location::Entity::find()
            .filter(stock_location::Column::TenantId.eq(tenant_id))
            .filter(stock_location::Column::DeletedAt.is_null())
            .all(&txn)
            .await
            .map_err(|error| storage_unavailable_with_context(&context, owner_operation, error))?
            .into_iter()
            .map(|location| location.id)
            .collect::<HashSet<_>>();
        let mut levels = inventory_level::Entity::find()
            .filter(inventory_level::Column::InventoryItemId.eq(inventory_item.id))
            .all(&txn)
            .await
            .map_err(|error| storage_unavailable_with_context(&context, owner_operation, error))?
            .into_iter()
            .filter(|level| active_location_ids.contains(&level.location_id))
            .collect::<Vec<_>>();
        levels.sort_by(|left, right| {
            level_available(right)
                .cmp(&level_available(left))
                .then_with(|| left.id.cmp(&right.id))
        });

        let allows_backorder = crate::inventory_policy_allows_backorder(&variant.inventory_policy);
        let mut selected = None;
        for level in levels {
            let mut update = inventory_level::Entity::update_many()
                .col_expr(
                    inventory_level::Column::ReservedQuantity,
                    Expr::col(inventory_level::Column::ReservedQuantity).add(request.quantity),
                )
                .col_expr(
                    inventory_level::Column::UpdatedAt,
                    Expr::current_timestamp().into(),
                )
                .filter(inventory_level::Column::Id.eq(level.id));
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
        let metadata = reservation_metadata(
            request.metadata.clone(),
            request.reservation_id,
            request.external_id.as_str(),
            request.variant_id,
        );
        let inserted = reservation_item::ActiveModel {
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
        }
        .insert(&txn)
        .await;

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
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
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
        let allows_backorder = crate::inventory_policy_allows_backorder(&variant.inventory_policy);

        if reservation.deleted_at.is_some() || reservation.quantity == 0 {
            let available_quantity =
                available_quantity(&context, owner_operation, &txn, item.id).await?;
            txn.commit().await.map_err(|error| {
                storage_unavailable_with_context(&context, owner_operation, error)
            })?;
            return Ok(InventoryIdentityReservationReleaseSnapshot {
                reservation_id: reservation.id,
                external_id: request.external_id,
                variant_id: variant.id,
                location_id: reservation.location_id,
                released_quantity: 0,
                available_quantity,
                in_stock: available_quantity > 0 || allows_backorder,
            });
        }

        let released_quantity = reservation.quantity;
        let level_update = inventory_level::Entity::update_many()
            .col_expr(
                inventory_level::Column::ReservedQuantity,
                Expr::col(inventory_level::Column::ReservedQuantity).sub(released_quantity),
            )
            .col_expr(
                inventory_level::Column::UpdatedAt,
                Expr::current_timestamp().into(),
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
        let mut active: reservation_item::ActiveModel = reservation.clone().into();
        active.quantity = Set(0);
        active.metadata = Set(metadata);
        active.updated_at = Set(Utc::now().into());
        active.deleted_at = Set(Some(Utc::now().into()));
        active
            .update(&txn)
            .await
            .map_err(|error| storage_unavailable_with_context(&context, owner_operation, error))?;

        let available_quantity =
            available_quantity(&context, owner_operation, &txn, item.id).await?;
        txn.commit()
            .await
            .map_err(|error| storage_unavailable_with_context(&context, owner_operation, error))?;
        Ok(InventoryIdentityReservationReleaseSnapshot {
            reservation_id: reservation.id,
            external_id: request.external_id,
            variant_id: variant.id,
            location_id: reservation.location_id,
            released_quantity,
            available_quantity,
            in_stock: available_quantity > 0 || allows_backorder,
        })
    }
}

async fn load_tenant_variant<C>(
    context: &PortContext,
    owner_operation: &'static str,
    conn: &C,
    tenant_id: Uuid,
    variant_id: Uuid,
) -> Result<product_variant::Model, PortError>
where
    C: ConnectionTrait,
{
    product_variant::Entity::find_by_id(variant_id)
        .filter(product_variant::Column::TenantId.eq(tenant_id))
        .one(conn)
        .await
        .map_err(|error| storage_unavailable_with_context(context, owner_operation, error))?
        .ok_or_else(|| {
            tracing::warn!(
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "inventory.variant_not_found",
                variant_id = %variant_id,
                "inventory variant was not found"
            );
            PortError::not_found(
                "inventory.variant_not_found",
                "inventory variant was not found",
            )
        })
}

async fn load_inventory_item_for_update<C>(
    context: &PortContext,
    owner_operation: &'static str,
    conn: &C,
    variant_id: Uuid,
) -> Result<Option<inventory_item::Model>, PortError>
where
    C: ConnectionTrait,
{
    let query =
        || inventory_item::Entity::find().filter(inventory_item::Column::VariantId.eq(variant_id));
    match conn.get_database_backend() {
        DbBackend::Sqlite => query()
            .one(conn)
            .await
            .map_err(|error| storage_unavailable_with_context(context, owner_operation, error)),
        DbBackend::Postgres | DbBackend::MySql => query()
            .lock_exclusive()
            .one(conn)
            .await
            .map_err(|error| storage_unavailable_with_context(context, owner_operation, error)),
    }
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
        DbBackend::Postgres | DbBackend::MySql => query()
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
    variant: &product_variant::Model,
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
        variant_id: variant.id,
        location_id: existing.location_id,
        reserved_quantity: existing.quantity,
        available_quantity,
        in_stock: available_quantity > 0
            || crate::inventory_policy_allows_backorder(&variant.inventory_policy),
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
        .all(conn)
        .await
        .map_err(|error| storage_unavailable_with_context(context, owner_operation, error))?;
    levels.into_iter().try_fold(0_i32, |total, level| {
        total.checked_add(level_available(&level)).ok_or_else(|| {
            PortError::invariant_violation(
                "inventory.available_quantity_overflow",
                "inventory available quantity is outside the supported range",
            )
        })
    })
}

fn level_available(level: &inventory_level::Model) -> i32 {
    level
        .stocked_quantity
        .saturating_sub(level.reserved_quantity)
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

fn normalize_external_id(value: String) -> Result<String, PortError> {
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

fn parse_port_tenant_id(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.tenant_id.trim()).map_err(|error| {
        tracing::warn!(
            error = ?error,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            operation = owner_operation,
            code = "inventory.context_invalid",
            "inventory port context is invalid"
        );
        PortError::validation(
            "inventory.context_invalid",
            "inventory request context is invalid",
        )
    })
}

fn storage_unavailable_with_context(
    context: &PortContext,
    owner_operation: &'static str,
    error: sea_orm::DbErr,
) -> PortError {
    tracing::error!(
        error = ?error,
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        operation = owner_operation,
        code = "inventory.database_unavailable",
        "inventory storage operation failed"
    );
    PortError::unavailable(
        "inventory.database_unavailable",
        "inventory storage is temporarily unavailable",
    )
}

fn inventory_error_to_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: rustok_commerce_foundation::error::CommerceError,
) -> PortError {
    use rustok_commerce_foundation::error::CommerceError;

    match error {
        CommerceError::Database(error) => {
            tracing::error!(
                error = ?error,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "inventory.database_unavailable",
                "inventory owner storage operation failed"
            );
            PortError::unavailable(
                "inventory.database_unavailable",
                "inventory storage is temporarily unavailable",
            )
        }
        CommerceError::VariantNotFound(variant_id) => {
            tracing::warn!(
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "inventory.variant_not_found",
                variant_id = %variant_id,
                "inventory variant was not found"
            );
            PortError::new(
                PortErrorKind::NotFound,
                "inventory.variant_not_found",
                "inventory variant was not found",
                false,
            )
        }
        CommerceError::InsufficientInventory {
            requested,
            available,
        } => {
            tracing::warn!(
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "inventory.insufficient_inventory",
                requested,
                available,
                "inventory reservation conflicts with available stock"
            );
            PortError::new(
                PortErrorKind::Conflict,
                "inventory.insufficient_inventory",
                "inventory reservation conflicts with available stock",
                false,
            )
        }
        CommerceError::InvalidPrice(message) | CommerceError::Validation(message) => {
            tracing::warn!(
                internal_message = %message,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "inventory.validation",
                "inventory request validation failed"
            );
            PortError::validation("inventory.validation", "inventory request is invalid")
        }
        other => {
            tracing::error!(
                error = ?other,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "inventory.invariant_violation",
                "inventory owner invariant failed"
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
}
