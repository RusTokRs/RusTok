use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::instrument;
use uuid::Uuid;

use rustok_core::events::ValidateEvent;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

use rustok_commerce_foundation::dto::AdjustInventoryInput;
use rustok_commerce_foundation::entities;
use rustok_commerce_foundation::error::{CommerceError, CommerceResult};

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

impl InventoryQuantityWriteResult {
    fn from_quantity(quantity: i32) -> Self {
        Self {
            quantity,
            in_stock: quantity > 0,
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
            in_stock: available_quantity > 0 || inventory_policy == "continue",
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
    ) -> CommerceResult<i32> {
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

    #[instrument(skip(self))]
    pub async fn adjust_variant_quantity(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        adjustment: i32,
        reason: Option<String>,
    ) -> CommerceResult<InventoryQuantityWriteResult> {
        let quantity = self
            .adjust_inventory(
                tenant_id,
                actor_id,
                AdjustInventoryInput {
                    variant_id,
                    adjustment,
                    reason,
                },
            )
            .await?;

        Ok(InventoryQuantityWriteResult::from_quantity(quantity))
    }

    #[instrument(skip(self))]
    pub async fn adjust_inventory(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: AdjustInventoryInput,
    ) -> CommerceResult<i32> {
        let txn = self.db.begin().await?;

        let variant = self.load_variant(&txn, tenant_id, input.variant_id).await?;
        let state = self
            .ensure_inventory_state(&txn, tenant_id, &variant)
            .await?;
        let old_quantity = self
            .available_quantity(&txn, state.inventory_item.id)
            .await?;
        let new_quantity = old_quantity + input.adjustment;

        if new_quantity < 0 && variant.inventory_policy == "deny" {
            return Err(CommerceError::InsufficientInventory {
                requested: -input.adjustment,
                available: old_quantity,
            });
        }

        let mut level_active: entities::inventory_level::ActiveModel = state.level.clone().into();
        level_active.stocked_quantity = Set(state.level.stocked_quantity + input.adjustment);
        level_active.updated_at = Set(Utc::now().into());
        level_active.update(&txn).await?;

        // Create and validate event
        let event = DomainEvent::InventoryUpdated {
            variant_id: input.variant_id,
            product_id: variant.product_id,
            location_id: state.location.id,
            old_quantity,
            new_quantity,
        };
        event
            .validate()
            .map_err(|e| CommerceError::Validation(format!("Invalid inventory event: {}", e)))?;

        self.event_bus
            .publish_in_tx(&txn, tenant_id, Some(actor_id), event)
            .await?;

        if new_quantity < self.low_stock_threshold && new_quantity > 0 {
            // Create and validate low inventory event
            let low_event = DomainEvent::InventoryLow {
                variant_id: input.variant_id,
                product_id: variant.product_id,
                remaining: new_quantity,
                threshold: self.low_stock_threshold,
            };
            low_event.validate().map_err(|e| {
                CommerceError::Validation(format!("Invalid low inventory event: {}", e))
            })?;

            self.event_bus
                .publish_in_tx(&txn, tenant_id, Some(actor_id), low_event)
                .await?;
        }

        txn.commit().await?;
        Ok(new_quantity)
    }

    #[instrument(skip(self))]
    pub async fn set_variant_quantity(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        quantity: i32,
    ) -> CommerceResult<InventoryQuantityWriteResult> {
        let quantity = self
            .set_inventory(tenant_id, actor_id, variant_id, quantity)
            .await?;

        Ok(InventoryQuantityWriteResult::from_quantity(quantity))
    }

    #[instrument(skip(self))]
    pub async fn set_inventory(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        quantity: i32,
    ) -> CommerceResult<i32> {
        let txn = self.db.begin().await?;

        let variant = self.load_variant(&txn, tenant_id, variant_id).await?;
        let state = self
            .ensure_inventory_state(&txn, tenant_id, &variant)
            .await?;
        let old_quantity = self
            .available_quantity(&txn, state.inventory_item.id)
            .await?;

        if quantity < 0 && variant.inventory_policy == "deny" {
            return Err(CommerceError::InsufficientInventory {
                requested: -quantity,
                available: old_quantity,
            });
        }

        let mut level_active: entities::inventory_level::ActiveModel = state.level.clone().into();
        level_active.stocked_quantity = Set(quantity);
        level_active.updated_at = Set(Utc::now().into());
        level_active.update(&txn).await?;

        // Create and validate event
        let event = DomainEvent::InventoryUpdated {
            variant_id,
            product_id: variant.product_id,
            location_id: state.location.id,
            old_quantity,
            new_quantity: quantity,
        };
        event
            .validate()
            .map_err(|e| CommerceError::Validation(format!("Invalid inventory event: {}", e)))?;

        self.event_bus
            .publish_in_tx(&txn, tenant_id, Some(actor_id), event)
            .await?;

        txn.commit().await?;
        Ok(quantity)
    }

    #[instrument(skip(self))]
    pub async fn check_availability(
        &self,
        tenant_id: Uuid,
        variant_id: Uuid,
        requested_quantity: i32,
    ) -> CommerceResult<bool> {
        let variant = self.load_variant(&self.db, tenant_id, variant_id).await?;

        if variant.inventory_policy == "continue" {
            return Ok(true);
        }

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

    #[instrument(skip(self))]
    pub async fn reserve(
        &self,
        tenant_id: Uuid,
        variant_id: Uuid,
        quantity: i32,
    ) -> CommerceResult<InventoryReservationWriteResult> {
        if quantity < 0 {
            return Err(CommerceError::Validation(
                "Reservation quantity must be non-negative".to_string(),
            ));
        }

        let txn = self.db.begin().await?;
        let variant = self.load_variant(&txn, tenant_id, variant_id).await?;

        if quantity == 0 {
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
        let available = self
            .available_quantity(&txn, state.inventory_item.id)
            .await?;

        if variant.inventory_policy != "continue" && available < quantity {
            return Err(CommerceError::InsufficientInventory {
                requested: quantity,
                available,
            });
        }

        let mut level_active: entities::inventory_level::ActiveModel = state.level.clone().into();
        level_active.reserved_quantity = Set(state.level.reserved_quantity + quantity);
        level_active.updated_at = Set(Utc::now().into());
        level_active.update(&txn).await?;

        entities::reservation_item::ActiveModel {
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
        }
        .insert(&txn)
        .await?;

        txn.commit().await?;
        Ok(InventoryReservationWriteResult::from_quantities(
            quantity,
            available - quantity,
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

    async fn ensure_inventory_state<C>(
        &self,
        conn: &C,
        tenant_id: Uuid,
        variant: &entities::product_variant::Model,
    ) -> CommerceResult<InventoryState>
    where
        C: sea_orm::ConnectionTrait,
    {
        let location = self.ensure_default_location(conn, tenant_id).await?;
        let inventory_item = self.ensure_inventory_item(conn, variant).await?;
        let level = self
            .ensure_inventory_level(conn, &inventory_item, &location, 0)
            .await?;

        Ok(InventoryState {
            location,
            inventory_item,
            level,
        })
    }

    async fn ensure_default_location<C>(
        &self,
        conn: &C,
        tenant_id: Uuid,
    ) -> CommerceResult<entities::stock_location::Model>
    where
        C: sea_orm::ConnectionTrait,
    {
        if let Some(location) = entities::stock_location::Entity::find()
            .filter(entities::stock_location::Column::TenantId.eq(tenant_id))
            .filter(entities::stock_location::Column::DeletedAt.is_null())
            .one(conn)
            .await?
        {
            return Ok(location);
        }

        let now = Utc::now();
        let location = entities::stock_location::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            code: Set(Some("default".to_string())),
            address_line1: Set(None),
            address_line2: Set(None),
            city: Set(None),
            province: Set(None),
            postal_code: Set(None),
            country_code: Set(None),
            phone: Set(None),
            metadata: Set(json!({ "source": "legacy_inventory_service" })),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            deleted_at: Set(None),
        }
        .insert(conn)
        .await
        .map_err(CommerceError::from)?;

        entities::stock_location_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            stock_location_id: Set(location.id),
            locale: Set("en".to_string()),
            name: Set("Default".to_string()),
        }
        .insert(conn)
        .await
        .map_err(CommerceError::from)?;

        Ok(location)
    }

    async fn ensure_inventory_item<C>(
        &self,
        conn: &C,
        variant: &entities::product_variant::Model,
    ) -> CommerceResult<entities::inventory_item::Model>
    where
        C: sea_orm::ConnectionTrait,
    {
        if let Some(item) = entities::inventory_item::Entity::find()
            .filter(entities::inventory_item::Column::VariantId.eq(variant.id))
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
            .all(conn)
            .await?;

        Ok(levels
            .into_iter()
            .map(|level| level.stocked_quantity - level.reserved_quantity)
            .sum())
    }
}

#[cfg(test)]
mod tests {
    use super::{InventoryQuantityWriteResult, InventoryReservationWriteResult};

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
}

struct InventoryState {
    location: entities::stock_location::Model,
    inventory_item: entities::inventory_item::Model,
    level: entities::inventory_level::Model,
}
