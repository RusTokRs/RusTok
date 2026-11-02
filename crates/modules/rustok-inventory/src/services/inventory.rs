use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction,
    DbBackend, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
    sea_query::{Expr, ExprTrait},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::instrument;
use uuid::Uuid;

use crate::reservation_events::{ReservationEventPlan, quantity_tenant};
use rustok_api::PortContext;
use rustok_outbox::TransactionalEventBus;

use rustok_commerce_foundation::dto::AdjustInventoryInput;
use rustok_commerce_foundation::entities;
use rustok_commerce_foundation::error::{CommerceError, CommerceResult};

use super::policy::inventory_policy_allows_backorder;

#[path = "inventory/reservation_release.rs"]
mod reservation_release;

use reservation_release::{QUANTITY_RESERVATION_ROW_LIMIT, SelectedQuantityRelease};

const STOCK_LEVEL_READ_LIMIT: usize = 4096;

/// Native compatibility and durable identity writes share one item lock. The caller must first
/// prove the Variant through the tenant owner boundary; inventory_items itself has no tenant key.
pub(crate) fn inventory_item_write_query<C>(
    conn: &C,
    variant_id: Uuid,
) -> sea_orm::Select<entities::inventory_item::Entity>
where
    C: sea_orm::ConnectionTrait,
{
    let query = entities::inventory_item::Entity::find()
        .filter(entities::inventory_item::Column::VariantId.eq(variant_id));
    if conn.get_database_backend() == DbBackend::Sqlite {
        query
    } else {
        query.lock_exclusive()
    }
}

pub struct InventoryService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    low_stock_threshold: i32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InventoryQuantityWriteResult {
    pub quantity: i32,
    #[serde(rename = "inStock")]
    pub in_stock: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InventoryReservationWriteResult {
    #[serde(rename = "reservedQuantity")]
    pub reserved_quantity: i32,
    #[serde(rename = "availableQuantity")]
    pub available_quantity: i32,
    #[serde(rename = "inStock")]
    pub in_stock: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InventoryAvailabilityCheckResult {
    pub available: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InventoryReservationReleaseWriteResult {
    #[serde(rename = "releasedQuantity")]
    pub released_quantity: i32,
    #[serde(rename = "availableQuantity")]
    pub available_quantity: i32,
    #[serde(rename = "inStock")]
    pub in_stock: bool,
}

impl InventoryQuantityWriteResult {
    #[cfg(test)]
    fn from_quantity(quantity: i32) -> Self {
        Self::from_quantity_and_policy(quantity, "deny")
    }

    fn from_quantity_and_policy(quantity: i32, inventory_policy: &str) -> Self {
        Self {
            quantity,
            in_stock: quantity > 0 || inventory_policy_allows_backorder(inventory_policy),
        }
    }
}

impl InventoryReservationWriteResult {
    fn from_quantities(
        reserved_quantity: i32,
        available_quantity: i32,
        inventory_policy: &str,
    ) -> Self {
        Self {
            reserved_quantity,
            available_quantity,
            in_stock: available_quantity > 0 || inventory_policy_allows_backorder(inventory_policy),
        }
    }
}

impl InventoryReservationReleaseWriteResult {
    fn from_quantities(
        released_quantity: i32,
        available_quantity: i32,
        inventory_policy: &str,
    ) -> Self {
        Self {
            released_quantity,
            available_quantity,
            in_stock: available_quantity > 0 || inventory_policy_allows_backorder(inventory_policy),
        }
    }
}

impl InventoryService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            db,
            event_bus,
            low_stock_threshold: 5,
        }
    }

    pub fn event_bus(&self) -> &TransactionalEventBus {
        &self.event_bus
    }

    pub fn with_threshold(mut self, threshold: i32) -> Self {
        self.low_stock_threshold = threshold;
        self
    }

    #[instrument(skip(self))]
    pub async fn adjust_variant_inventory(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        adjustment: i32,
    ) -> Result<i32, super::QuantityWriteError> {
        super::quantity::validate_request(tenant_id, actor_id, variant_id)?;
        tracing::debug!(
            target: "rustok_inventory::services",
            tenant_id = %tenant_id,
            actor_id = %actor_id,
            variant_id = %variant_id,
            adjustment,
            "adjusting variant inventory"
        );
        Ok(self
            .adjust_variant_quantity(
                tenant_id,
                actor_id,
                variant_id,
                adjustment,
                Some("Inventory admin native adjust endpoint".to_string()),
            )
            .await?
            .quantity)
    }

    #[instrument(skip(self, reason))]
    pub async fn adjust_variant_quantity(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        adjustment: i32,
        reason: Option<String>,
    ) -> Result<InventoryQuantityWriteResult, super::QuantityWriteError> {
        super::quantity::validate_request(tenant_id, actor_id, variant_id)?;
        tracing::debug!(
            target: "rustok_inventory::services",
            tenant_id = %tenant_id,
            actor_id = %actor_id,
            variant_id = %variant_id,
            adjustment,
            "adjusting variant quantity"
        );
        let update = self
            .adjust_inventory_update(
                tenant_id,
                actor_id,
                AdjustInventoryInput {
                    variant_id,
                    adjustment,
                    reason,
                },
            )
            .await?;

        Ok(InventoryQuantityWriteResult::from_quantity_and_policy(
            update.quantity,
            &update.inventory_policy,
        ))
    }

    #[instrument(skip(self, input))]
    pub async fn adjust_inventory(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: AdjustInventoryInput,
    ) -> Result<i32, super::QuantityWriteError> {
        super::quantity::validate_request(tenant_id, actor_id, input.variant_id)?;
        tracing::debug!(
            target: "rustok_inventory::services",
            tenant_id = %tenant_id,
            actor_id = %actor_id,
            variant_id = %input.variant_id,
            adjustment = input.adjustment,
            "adjusting inventory from input"
        );
        Ok(self
            .adjust_inventory_update(tenant_id, actor_id, input)
            .await?
            .quantity)
    }

    async fn adjust_inventory_update(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: AdjustInventoryInput,
    ) -> Result<super::quantity::WriteOutcome, super::QuantityWriteError> {
        self.execute_quantity_write(
            tenant_id,
            actor_id,
            input.variant_id,
            super::quantity::Change::Adjust(input.adjustment),
            Some(self.low_stock_threshold),
        )
        .await
    }

    #[instrument(skip(self))]
    pub async fn set_variant_quantity(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        quantity: i32,
    ) -> Result<InventoryQuantityWriteResult, super::QuantityWriteError> {
        super::quantity::validate_request(tenant_id, actor_id, variant_id)?;
        tracing::debug!(
            target: "rustok_inventory::services",
            tenant_id = %tenant_id,
            actor_id = %actor_id,
            variant_id = %variant_id,
            quantity,
            "setting variant quantity"
        );
        let update = self
            .set_inventory_update(tenant_id, actor_id, variant_id, quantity)
            .await?;

        Ok(InventoryQuantityWriteResult::from_quantity_and_policy(
            update.quantity,
            &update.inventory_policy,
        ))
    }

    #[instrument(skip(self))]
    pub async fn set_inventory(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        quantity: i32,
    ) -> Result<i32, super::QuantityWriteError> {
        super::quantity::validate_request(tenant_id, actor_id, variant_id)?;
        tracing::debug!(
            target: "rustok_inventory::services",
            tenant_id = %tenant_id,
            actor_id = %actor_id,
            variant_id = %variant_id,
            quantity,
            "setting inventory"
        );
        Ok(self
            .set_inventory_update(tenant_id, actor_id, variant_id, quantity)
            .await?
            .quantity)
    }

    async fn set_inventory_update(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        quantity: i32,
    ) -> Result<super::quantity::WriteOutcome, super::QuantityWriteError> {
        self.execute_quantity_write(
            tenant_id,
            actor_id,
            variant_id,
            super::quantity::Change::Set(quantity),
            None,
        )
        .await
    }

    async fn execute_quantity_write(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        change: super::quantity::Change,
        low_stock_threshold: Option<i32>,
    ) -> Result<super::quantity::WriteOutcome, super::QuantityWriteError> {
        let txn = self.db.begin().await?;
        let update = match super::quantity::write_quantity(
            &txn,
            tenant_id,
            actor_id,
            variant_id,
            change,
            low_stock_threshold,
        )
        .await
        {
            Ok(update) => update,
            Err(error) => {
                txn.rollback().await?;
                return Err(error);
            }
        };
        txn.commit().await?;
        Ok(update)
    }

    #[instrument(skip(self))]
    pub async fn check_variant_availability(
        &self,
        tenant_id: Uuid,
        variant_id: Uuid,
        requested_quantity: i32,
    ) -> CommerceResult<InventoryAvailabilityCheckResult> {
        validate_non_nil_uuid(tenant_id, "tenant_id")?;
        validate_non_nil_uuid(variant_id, "variant_id")?;
        tracing::debug!(
            target: "rustok_inventory::services",
            tenant_id = %tenant_id,
            variant_id = %variant_id,
            requested_quantity,
            "checking variant availability"
        );
        let available = self
            .check_availability(tenant_id, variant_id, requested_quantity)
            .await?;

        Ok(InventoryAvailabilityCheckResult { available })
    }

    #[instrument(skip(self))]
    pub async fn check_variant_availability_for_channel(
        &self,
        tenant_id: Uuid,
        variant_id: Uuid,
        requested_quantity: i32,
        public_channel_slug: Option<&str>,
    ) -> CommerceResult<InventoryAvailabilityCheckResult> {
        validate_non_nil_uuid(tenant_id, "tenant_id")?;
        validate_non_nil_uuid(variant_id, "variant_id")?;
        tracing::debug!(
            target: "rustok_inventory::services",
            tenant_id = %tenant_id,
            variant_id = %variant_id,
            requested_quantity,
            channel_slug = ?public_channel_slug,
            "checking variant availability for channel"
        );
        let variant = self.load_variant(&self.db, tenant_id, variant_id).await?;
        let available = super::public_channel::check_variant_availability_for_public_channel(
            &self.db,
            tenant_id,
            super::public_channel::PublicChannelInventoryVariantProjectionInput {
                variant_id: variant.id,
                inventory_policy: &variant.inventory_policy,
            },
            requested_quantity,
            public_channel_slug,
        )
        .await?;

        Ok(InventoryAvailabilityCheckResult { available })
    }

    pub async fn check_availability(
        &self,
        tenant_id: Uuid,
        variant_id: Uuid,
        requested_quantity: i32,
    ) -> CommerceResult<bool> {
        validate_non_nil_uuid(tenant_id, "tenant_id")?;
        validate_non_nil_uuid(variant_id, "variant_id")?;
        validate_availability_request_quantity(requested_quantity)?;
        tracing::debug!(
            target: "rustok_inventory::services",
            tenant_id = %tenant_id,
            variant_id = %variant_id,
            requested_quantity,
            "checking availability"
        );

        let variant = self.load_variant(&self.db, tenant_id, variant_id).await?;

        if inventory_policy_allows_backorder(&variant.inventory_policy) {
            return Ok(true);
        }

        // INVARIANT: `inventory_item` has no tenant column; the tenant boundary is
        // the variant loaded tenant-scoped above (`load_variant`).
        let available = if let Some(inventory_item) = entities::inventory_item::Entity::find()
            .filter(entities::inventory_item::Column::VariantId.eq(variant_id))
            .one(&self.db)
            .await?
        {
            self.available_quantity(&self.db, inventory_item.id).await?
        } else {
            0
        };

        Ok(available >= requested_quantity)
    }

    #[instrument(skip(self, context))]
    pub async fn reserve(
        &self,
        context: &PortContext,
        variant_id: Uuid,
        quantity: i32,
    ) -> CommerceResult<InventoryReservationWriteResult> {
        let tenant_id = quantity_tenant(context, "reserve_inventory")?;
        validate_non_nil_uuid(tenant_id, "tenant_id")?;
        validate_non_nil_uuid(variant_id, "variant_id")?;
        validate_reservation_quantity(quantity)?;
        tracing::debug!(
            target: "rustok_inventory::services",
            tenant_id = %tenant_id,
            variant_id = %variant_id,
            quantity,
            "reserving inventory"
        );

        let txn = self.db.begin().await?;
        let variant = self.load_variant(&txn, tenant_id, variant_id).await?;

        if quantity == 0 {
            // INVARIANT: `inventory_item` has no tenant column; the tenant boundary
            // is the variant loaded tenant-scoped above (`load_variant`).
            let available = if let Some(inventory_item) = entities::inventory_item::Entity::find()
                .filter(entities::inventory_item::Column::VariantId.eq(variant_id))
                .one(&txn)
                .await?
            {
                self.available_quantity(&txn, inventory_item.id).await?
            } else {
                variant.inventory_quantity
            };
            txn.commit().await?;
            return Ok(InventoryReservationWriteResult::from_quantities(
                0,
                available,
                variant.inventory_policy.as_str(),
            ));
        }

        let state = self
            .ensure_inventory_state(&txn, tenant_id, &variant)
            .await?;

        reservation_release::admit_new_reservation(
            state.inventory_item.id,
            variant_id,
            load_quantity_reservation_rows(&txn, state.inventory_item.id).await?,
        )?;

        if state.level.reserved_quantity < 0
            || state
                .level
                .reserved_quantity
                .checked_add(quantity)
                .is_none()
        {
            return Err(CommerceError::Validation(
                "Inventory reserved quantity exceeds the supported range or is inconsistent"
                    .to_string(),
            ));
        }

        let event_plan =
            ReservationEventPlan::prepare(&txn, context, "reserve_inventory", &variant).await?;

        let mut level_update = entities::inventory_level::Entity::update_many()
            .col_expr(
                entities::inventory_level::Column::ReservedQuantity,
                Expr::col(entities::inventory_level::Column::ReservedQuantity).add(quantity),
            )
            .col_expr(
                entities::inventory_level::Column::UpdatedAt,
                Expr::value(Utc::now()),
            )
            .filter(entities::inventory_level::Column::Id.eq(state.level.id));

        if !inventory_policy_allows_backorder(&variant.inventory_policy) {
            level_update = level_update.filter(
                Expr::col(entities::inventory_level::Column::StockedQuantity)
                    .sub(Expr::col(
                        entities::inventory_level::Column::ReservedQuantity,
                    ))
                    .gte(quantity),
            );
        }

        if level_update.exec(&txn).await?.rows_affected != 1 {
            let available = self
                .available_quantity(&txn, state.inventory_item.id)
                .await?;
            return Err(CommerceError::InsufficientInventory {
                requested: quantity,
                available,
            });
        }

        let available = self
            .available_quantity(&txn, state.inventory_item.id)
            .await?;

        let inserted =
            entities::reservation_item::Entity::insert(entities::reservation_item::ActiveModel {
                id: Set(Uuid::new_v4()),
                inventory_item_id: Set(state.inventory_item.id),
                location_id: Set(state.location.id),
                quantity: Set(quantity),
                line_item_id: Set(None),
                description: Set(Some("Legacy inventory reservation".to_string())),
                external_id: Set(None),
                metadata: Set(json!({
                    "source": "legacy_inventory_service",
                    "variant_id": variant_id,
                })),
                created_at: Set(Utc::now().into()),
                updated_at: Set(Utc::now().into()),
                deleted_at: Set(None),
            })
            .exec_without_returning(&txn)
            .await?;
        if inserted != 1 {
            return Err(CommerceError::Validation(
                "Anonymous reservation row persistence mismatch; transaction refused".to_string(),
            ));
        }

        event_plan.publish(&txn).await?;
        txn.commit().await?;
        Ok(InventoryReservationWriteResult::from_quantities(
            quantity,
            available,
            variant.inventory_policy.as_str(),
        ))
    }

    #[instrument(skip(self, context))]
    pub async fn release_reservation_quantity(
        &self,
        context: &PortContext,
        variant_id: Uuid,
        quantity: i32,
    ) -> CommerceResult<InventoryReservationReleaseWriteResult> {
        let tenant_id = quantity_tenant(context, "release_inventory_reservation")?;
        validate_non_nil_uuid(tenant_id, "tenant_id")?;
        validate_non_nil_uuid(variant_id, "variant_id")?;
        validate_release_quantity(quantity)?;
        tracing::debug!(
            target: "rustok_inventory::services",
            tenant_id = %tenant_id,
            variant_id = %variant_id,
            quantity,
            "releasing reservation quantity"
        );

        let txn = self.db.begin().await?;
        let variant = self.load_variant(&txn, tenant_id, variant_id).await?;

        if quantity == 0 {
            // INVARIANT: `inventory_item` has no tenant column; the tenant boundary
            // is the variant loaded tenant-scoped above (`load_variant`).
            let available = if let Some(inventory_item) = entities::inventory_item::Entity::find()
                .filter(entities::inventory_item::Column::VariantId.eq(variant_id))
                .one(&txn)
                .await?
            {
                self.available_quantity(&txn, inventory_item.id).await?
            } else {
                variant.inventory_quantity
            };
            txn.commit().await?;
            return Ok(InventoryReservationReleaseWriteResult::from_quantities(
                0,
                available,
                variant.inventory_policy.as_str(),
            ));
        }

        // INVARIANT: tenant proof above precedes the shared item lock and every ledger read.
        let Some(inventory_item) = inventory_item_write_query(&txn, variant_id)
            .one(&txn)
            .await?
        else {
            return Err(insufficient_reserved_release_error(quantity, 0));
        };
        let selected = SelectedQuantityRelease::select(
            inventory_item.id,
            variant_id,
            quantity,
            load_quantity_reservation_rows(&txn, inventory_item.id).await?,
        )?;
        let location_ids = selected.location_ids();
        // Match the native owner order: item -> reservations -> sorted locations -> exact levels.
        let locations_query = entities::stock_location::Entity::find()
            .filter(entities::stock_location::Column::TenantId.eq(tenant_id))
            .filter(entities::stock_location::Column::Id.is_in(location_ids.clone()))
            .order_by_asc(entities::stock_location::Column::Id)
            .limit((QUANTITY_RESERVATION_ROW_LIMIT + 1) as u64);
        let locations_query = if txn.get_database_backend() == DbBackend::Sqlite {
            locations_query
        } else {
            locations_query.lock_exclusive()
        };
        let locations = locations_query.all(&txn).await?;
        if locations.len() != location_ids.len() {
            return Err(CommerceError::Validation(
                "Anonymous reservation original location is missing or outside the tenant; reconcile without relocation".to_string(),
            ));
        }
        let levels_query = entities::inventory_level::Entity::find()
            .filter(entities::inventory_level::Column::InventoryItemId.eq(inventory_item.id))
            .filter(entities::inventory_level::Column::LocationId.is_in(location_ids.clone()))
            .order_by_asc(entities::inventory_level::Column::LocationId)
            .order_by_asc(entities::inventory_level::Column::Id)
            .limit((QUANTITY_RESERVATION_ROW_LIMIT + 1) as u64);
        let levels_query = if txn.get_database_backend() == DbBackend::Sqlite {
            levels_query
        } else {
            levels_query.lock_exclusive()
        };
        let levels = levels_query.all(&txn).await?;
        let active_claims =
            load_active_reservation_claims(&txn, inventory_item.id, &location_ids).await?;
        let plan = selected.bind_levels(tenant_id, locations, levels, active_claims)?;
        let event_plan =
            ReservationEventPlan::prepare(&txn, context, "release_inventory_reservation", &variant)
                .await?;
        plan.apply(&txn).await?;

        let available_quantity = self.available_quantity(&txn, inventory_item.id).await?;
        event_plan.publish(&txn).await?;
        txn.commit().await?;

        Ok(InventoryReservationReleaseWriteResult::from_quantities(
            quantity,
            available_quantity,
            variant.inventory_policy.as_str(),
        ))
    }

    async fn load_variant<C>(
        &self,
        conn: &C,
        tenant_id: Uuid,
        variant_id: Uuid,
    ) -> CommerceResult<entities::product_variant::Model>
    where
        C: sea_orm::ConnectionTrait,
    {
        entities::product_variant::Entity::find_by_id(variant_id)
            .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
            .one(conn)
            .await?
            .ok_or(CommerceError::VariantNotFound(variant_id))
    }

    async fn ensure_inventory_state(
        &self,
        conn: &DatabaseTransaction,
        tenant_id: Uuid,
        variant: &entities::product_variant::Model,
    ) -> CommerceResult<InventoryState> {
        let inventory_item = self.ensure_inventory_item(conn, variant).await?;
        let location = self.ensure_default_location(conn, tenant_id).await?;
        let location_query = entities::stock_location::Entity::find_by_id(location.id)
            .filter(entities::stock_location::Column::TenantId.eq(tenant_id))
            .filter(entities::stock_location::Column::DeletedAt.is_null());
        let location_query = if conn.get_database_backend() == DbBackend::Sqlite {
            location_query
        } else {
            location_query.lock_exclusive()
        };
        let location = location_query.one(conn).await?.ok_or_else(|| {
            CommerceError::Validation("The saved default stock location is missing or archived; no replacement allocation was made".to_string())
        })?;
        let level = self
            .ensure_inventory_level(conn, &inventory_item, &location, 0)
            .await?;

        Ok(InventoryState {
            location,
            inventory_item,
            level,
        })
    }

    async fn ensure_default_location(
        &self,
        conn: &DatabaseTransaction,
        tenant_id: Uuid,
    ) -> CommerceResult<entities::stock_location::Model> {
        super::bootstrap::BootstrapService::ensure_default_location_in_tx(conn, tenant_id)
            .await
            .map_err(CommerceError::from)
    }

    async fn ensure_inventory_item<C>(
        &self,
        conn: &C,
        variant: &entities::product_variant::Model,
    ) -> CommerceResult<entities::inventory_item::Model>
    where
        C: sea_orm::ConnectionTrait,
    {
        // INVARIANT: `inventory_item` has no tenant column; the tenant boundary is
        // the caller-provided variant, which is loaded tenant-scoped.
        if let Some(item) = inventory_item_write_query(conn, variant.id)
            .one(conn)
            .await?
        {
            return Ok(item);
        }

        let now = Utc::now();
        entities::inventory_item::ActiveModel {
            id: Set(Uuid::new_v4()),
            variant_id: Set(variant.id),
            sku: Set(variant.sku.clone()),
            requires_shipping: Set(true),
            metadata: Set(json!({ "source": "legacy_inventory_service" })),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(conn)
        .await
        .map_err(CommerceError::from)
    }

    async fn ensure_inventory_level<C>(
        &self,
        conn: &C,
        inventory_item: &entities::inventory_item::Model,
        location: &entities::stock_location::Model,
        initial_available_quantity: i32,
    ) -> CommerceResult<entities::inventory_level::Model>
    where
        C: sea_orm::ConnectionTrait,
    {
        if let Some(level) = entities::inventory_level::Entity::find()
            .filter(entities::inventory_level::Column::InventoryItemId.eq(inventory_item.id))
            .filter(entities::inventory_level::Column::LocationId.eq(location.id))
            .one(conn)
            .await?
        {
            return Ok(level);
        }

        entities::inventory_level::ActiveModel {
            id: Set(Uuid::new_v4()),
            inventory_item_id: Set(inventory_item.id),
            location_id: Set(location.id),
            stocked_quantity: Set(initial_available_quantity),
            reserved_quantity: Set(0),
            incoming_quantity: Set(0),
            low_stock_threshold: Set(Some(self.low_stock_threshold)),
            updated_at: Set(Utc::now().into()),
        }
        .insert(conn)
        .await
        .map_err(CommerceError::from)
    }

    async fn available_quantity<C>(&self, conn: &C, inventory_item_id: Uuid) -> CommerceResult<i32>
    where
        C: sea_orm::ConnectionTrait,
    {
        let levels = entities::inventory_level::Entity::find()
            .filter(entities::inventory_level::Column::InventoryItemId.eq(inventory_item_id))
            .order_by_asc(entities::inventory_level::Column::Id)
            .limit((STOCK_LEVEL_READ_LIMIT + 1) as u64)
            .all(conn)
            .await?;
        checked_available_quantity(&levels)
    }
}

async fn load_quantity_reservation_rows(
    txn: &DatabaseTransaction,
    inventory_item_id: Uuid,
) -> CommerceResult<Vec<entities::reservation_item::Model>> {
    let query = entities::reservation_item::Entity::find()
        .filter(entities::reservation_item::Column::InventoryItemId.eq(inventory_item_id))
        .filter(entities::reservation_item::Column::DeletedAt.is_null())
        .filter(entities::reservation_item::Column::ExternalId.is_null())
        .filter(entities::reservation_item::Column::LineItemId.is_null())
        .order_by_asc(entities::reservation_item::Column::CreatedAt)
        .order_by_asc(entities::reservation_item::Column::Id)
        .limit((QUANTITY_RESERVATION_ROW_LIMIT + 1) as u64);
    let query = if txn.get_database_backend() == DbBackend::Sqlite {
        query
    } else {
        query.lock_exclusive()
    };
    Ok(query.all(txn).await?)
}

async fn load_active_reservation_claims(
    txn: &DatabaseTransaction,
    inventory_item_id: Uuid,
    location_ids: &[Uuid],
) -> CommerceResult<std::collections::BTreeMap<Uuid, i64>> {
    if location_ids.is_empty() || location_ids.len() > QUANTITY_RESERVATION_ROW_LIMIT {
        return Err(CommerceError::Validation(
            "Invalid anonymous release location bound".to_string(),
        ));
    }
    let claims = || {
        let query = entities::reservation_item::Entity::find()
            .filter(entities::reservation_item::Column::InventoryItemId.eq(inventory_item_id))
            .filter(entities::reservation_item::Column::DeletedAt.is_null())
            .filter(
                entities::reservation_item::Column::LocationId.is_in(location_ids.iter().copied()),
            );
        if txn.get_database_backend() == DbBackend::MySql {
            // MySQL's default repeatable-read snapshot may predate the item/level locks.
            // Both the invalid-row probe and aggregate must be current reads, not stale claims.
            query.lock_exclusive()
        } else {
            query
        }
    };
    if claims()
        .filter(entities::reservation_item::Column::Quantity.lte(0))
        .one(txn)
        .await?
        .is_some()
    {
        return Err(CommerceError::Validation(
            "Active reservation claim has invalid quantity; reconcile saved history".to_string(),
        ));
    }
    // Aggregate every claim at the locked exact locations, including any number of keyed rows.
    // Do not cap/materialize another consumer's ledger or mistake it for releasable pool units.
    let sum = Expr::col(entities::reservation_item::Column::Quantity).sum();
    let sum_result = if txn.get_database_backend() == DbBackend::MySql {
        // MySQL exposes SUM(integer) as DECIMAL. Admit only an exact representable group below,
        // then decode its signed integer cast; never clamp a quantity into the plan.
        sum.clone()
            .cast_as(sea_orm::sea_query::Alias::new("SIGNED"))
    } else {
        sum.clone()
    };
    let grouped = claims()
        .select_only()
        .column(entities::reservation_item::Column::LocationId)
        .column_as(sum_result, "claimed_quantity")
        .group_by(entities::reservation_item::Column::LocationId)
        .having(sum.lte(i32::MAX))
        .into_tuple::<(Uuid, i64)>()
        .all(txn)
        .await?;
    Ok(grouped.into_iter().collect())
}

pub(crate) fn checked_available_quantity(
    levels: &[entities::inventory_level::Model],
) -> CommerceResult<i32> {
    if levels.len() > STOCK_LEVEL_READ_LIMIT {
        return Err(CommerceError::Validation(format!(
            "Inventory availability exceeds the {STOCK_LEVEL_READ_LIMIT} level read bound"
        )));
    }
    let total = levels.iter().try_fold(0_i64, |total, level| {
        if level.reserved_quantity < 0 {
            return Err(CommerceError::Validation(
                "Inventory has a negative reserved counter; reconcile saved state".to_string(),
            ));
        }
        let available = i64::from(level.stocked_quantity) - i64::from(level.reserved_quantity);
        total.checked_add(available).ok_or_else(|| {
            CommerceError::Validation(
                "Inventory availability exceeds the supported range".to_string(),
            )
        })
    })?;
    // Negative availability is legitimate for continue backorders. This is not v1 event admission.
    i32::try_from(total).map_err(|_| {
        CommerceError::Validation(
            "Inventory availability exceeds the signed i32 response range".to_string(),
        )
    })
}

fn insufficient_reserved_release_error(requested: i32, reserved: i32) -> CommerceError {
    CommerceError::Validation(format!(
        "Cannot release {requested} reserved units; only {reserved} are reserved"
    ))
}

fn insufficient_reservation_items_release_error(requested: i32, tracked: i64) -> CommerceError {
    CommerceError::Validation(format!(
        "Cannot release {requested} reservation item units; only {tracked} are tracked in the proved anonymous quantity pool; use the identity port for keyed allocations"
    ))
}

fn validate_reservation_quantity(quantity: i32) -> CommerceResult<()> {
    if quantity < 0 {
        return Err(CommerceError::Validation(
            "Reservation quantity must be non-negative".to_string(),
        ));
    }

    Ok(())
}

fn validate_release_quantity(quantity: i32) -> CommerceResult<()> {
    if quantity < 0 {
        return Err(CommerceError::Validation(
            "Reservation release quantity must be non-negative".to_string(),
        ));
    }

    Ok(())
}

fn validate_non_nil_uuid(id: Uuid, field_name: &'static str) -> CommerceResult<()> {
    if id.is_nil() {
        return Err(CommerceError::Validation(format!(
            "{field_name} cannot be nil"
        )));
    }
    Ok(())
}

fn validate_availability_request_quantity(requested_quantity: i32) -> CommerceResult<()> {
    if requested_quantity < 0 {
        return Err(CommerceError::Validation(
            "Availability check quantity must be non-negative".to_string(),
        ));
    }

    Ok(())
}

struct InventoryState {
    location: entities::stock_location::Model,
    inventory_item: entities::inventory_item::Model,
    level: entities::inventory_level::Model,
}

#[cfg(test)]
mod tests {
    use super::{
        InventoryAvailabilityCheckResult, InventoryQuantityWriteResult,
        InventoryReservationReleaseWriteResult, InventoryReservationWriteResult,
        insufficient_reservation_items_release_error, insufficient_reserved_release_error,
        validate_availability_request_quantity, validate_release_quantity,
        validate_reservation_quantity,
    };

    #[test]
    fn set_quantity_preserves_reserved_units_when_targeting_available_quantity() {
        use super::super::quantity::stocked_target;
        assert_eq!(stocked_target(0, 10, 0, 0, false).unwrap(), 10);
        assert_eq!(stocked_target(-3, 10, 0, 3, false).unwrap(), 13);
        assert_eq!(stocked_target(-3, 0, 0, 3, false).unwrap(), 3);
    }

    #[test]
    fn reservation_release_error_reports_current_reserved_quantity_without_creating_state() {
        let error = insufficient_reserved_release_error(3, 1);

        assert!(
            error
                .to_string()
                .contains("Cannot release 3 reserved units; only 1 are reserved"),
            "reservation release errors should report observed reserved quantity"
        );
    }

    #[test]
    fn reservation_release_error_reports_tracked_reservation_item_quantity() {
        let error = insufficient_reservation_items_release_error(4, 2);

        assert!(
            error
                .to_string()
                .contains("Cannot release 4 reservation item units; only 2 are tracked"),
            "reservation item release errors should report tracked item quantity"
        );
    }

    #[test]
    fn reservation_quantity_rejects_negative_requests_before_db_access() {
        let error = validate_reservation_quantity(-1)
            .expect_err("negative reservations must be rejected before DB lookup");

        assert!(
            error.to_string().contains("non-negative"),
            "reservation validation should explain the non-negative invariant"
        );
        assert!(validate_reservation_quantity(0).is_ok());
        assert!(validate_reservation_quantity(3).is_ok());
    }

    #[test]
    fn reservation_release_quantity_rejects_negative_requests_before_db_access() {
        let error = validate_release_quantity(-1)
            .expect_err("negative reservation releases must be rejected before DB lookup");

        assert!(
            error.to_string().contains("non-negative"),
            "reservation release validation should explain the non-negative invariant"
        );
        assert!(validate_release_quantity(0).is_ok());
        assert!(validate_release_quantity(3).is_ok());
    }

    #[test]
    fn reservation_release_result_reports_released_available_and_stock_state() {
        let result = InventoryReservationReleaseWriteResult::from_quantities(3, 9, "deny");

        assert_eq!(result.released_quantity, 3);
        assert_eq!(result.available_quantity, 9);
        assert!(result.in_stock);

        let depleted = InventoryReservationReleaseWriteResult::from_quantities(1, 0, "deny");
        assert!(!depleted.in_stock);

        let backorderable =
            InventoryReservationReleaseWriteResult::from_quantities(1, -1, "continue");
        assert!(backorderable.in_stock);
    }

    #[test]
    fn reservation_release_result_keeps_backend_wire_shape() {
        let result = InventoryReservationReleaseWriteResult::from_quantities(2, 10, "deny");

        let serialized = serde_json::to_value(&result).expect(
            "inventory reservation release result should serialize for native endpoint transport",
        );

        assert_eq!(
            serialized,
            serde_json::json!({
                "releasedQuantity": 2,
                "availableQuantity": 10,
                "inStock": true
            })
        );
    }

    #[test]
    fn availability_check_quantity_rejects_negative_requests_before_db_access() {
        let error = validate_availability_request_quantity(-1)
            .expect_err("negative availability requests must be rejected before DB lookup");

        assert!(
            error.to_string().contains("non-negative"),
            "availability request validation should explain the non-negative invariant"
        );
        assert!(validate_availability_request_quantity(0).is_ok());
        assert!(validate_availability_request_quantity(3).is_ok());
    }

    #[test]
    fn availability_check_result_keeps_backend_wire_shape() {
        let result = InventoryAvailabilityCheckResult { available: true };

        let serialized = serde_json::to_value(&result)
            .expect("inventory availability result should serialize for native endpoint transport");

        assert_eq!(
            serialized,
            serde_json::json!({
                "available": true
            })
        );
    }

    #[test]
    fn quantity_write_result_derives_in_stock_from_committed_quantity() {
        for (quantity, expected_in_stock) in [(7, true), (1, true), (0, false), (-3, false)] {
            let result = InventoryQuantityWriteResult::from_quantity(quantity);

            assert_eq!(result.quantity, quantity);
            assert_eq!(
                result.in_stock, expected_in_stock,
                "inventory write result must report stock state from the committed quantity"
            );
        }
    }

    #[test]
    fn quantity_write_result_honors_backorder_policy_for_native_write_facades() {
        let depleted = InventoryQuantityWriteResult::from_quantity_and_policy(0, "deny");
        assert!(!depleted.in_stock);

        let backorderable = InventoryQuantityWriteResult::from_quantity_and_policy(0, "CONTINUE");
        assert!(
            backorderable.in_stock,
            "native set/adjust quantity results must keep backorderable variants in stock"
        );
    }

    #[test]
    fn reservation_write_result_reports_reserved_available_and_stock_state() {
        let result = InventoryReservationWriteResult::from_quantities(4, 6, "deny");

        assert_eq!(result.reserved_quantity, 4);
        assert_eq!(result.available_quantity, 6);
        assert!(result.in_stock);

        let depleted = InventoryReservationWriteResult::from_quantities(4, 0, "deny");
        assert!(!depleted.in_stock);

        let backorderable = InventoryReservationWriteResult::from_quantities(4, -2, "continue");
        assert!(backorderable.in_stock);
    }

    #[test]
    fn reservation_write_result_noop_preserves_available_quantity() {
        let result = InventoryReservationWriteResult::from_quantities(0, 8, "deny");

        assert_eq!(result.reserved_quantity, 0);
        assert_eq!(result.available_quantity, 8);
        assert!(result.in_stock);
    }

    #[test]
    fn reservation_write_result_keeps_backend_wire_shape() {
        let result = InventoryReservationWriteResult::from_quantities(2, 8, "deny");

        let serialized = serde_json::to_value(&result).expect(
            "inventory reservation write result should serialize for native endpoint transport",
        );

        assert_eq!(
            serialized,
            serde_json::json!({
                "reservedQuantity": 2,
                "availableQuantity": 8,
                "inStock": true
            })
        );
    }

    #[test]
    fn quantity_write_result_keeps_backend_wire_shape() {
        let result = InventoryQuantityWriteResult::from_quantity(0);

        let serialized = serde_json::to_value(&result)
            .expect("inventory write result should serialize for native endpoint transport");

        assert_eq!(
            serialized,
            serde_json::json!({
                "quantity": 0,
                "inStock": false
            })
        );
    }

    #[test]
    fn validate_non_nil_uuid_in_services_rejects_nil() {
        use super::validate_non_nil_uuid;
        use uuid::Uuid;

        let err = validate_non_nil_uuid(Uuid::nil(), "tenant_id").unwrap_err();
        assert!(err.to_string().contains("tenant_id cannot be nil"));

        let err = validate_non_nil_uuid(Uuid::nil(), "variant_id").unwrap_err();
        assert!(err.to_string().contains("variant_id cannot be nil"));

        assert!(validate_non_nil_uuid(Uuid::new_v4(), "tenant_id").is_ok());
    }
}
