use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
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
    pub async fn adjust_inventory(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: AdjustInventoryInput,
    ) -> CommerceResult<i32> {
        let txn = self.db.begin().await?;

        let variant = entities::product_variant::Entity::find_by_id(input.variant_id)
            .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(CommerceError::VariantNotFound(input.variant_id))?;

        let old_quantity = variant.inventory_quantity;
        let new_quantity = old_quantity + input.adjustment;

        if new_quantity < 0 && variant.inventory_policy == "deny" {
            return Err(CommerceError::InsufficientInventory {
                requested: -input.adjustment,
                available: old_quantity,
            });
        }

        let mut variant_active: entities::product_variant::ActiveModel = variant.clone().into();
        variant_active.inventory_quantity = Set(new_quantity);
        variant_active.updated_at = Set(Utc::now().into());
        variant_active.update(&txn).await?;

        // Create and validate event
        let event = DomainEvent::InventoryUpdated {
            variant_id: input.variant_id,
            product_id: variant.product_id,
            location_id: Uuid::new_v4(),
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
    pub async fn set_inventory(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        quantity: i32,
    ) -> CommerceResult<i32> {
        let txn = self.db.begin().await?;

        let variant = entities::product_variant::Entity::find_by_id(variant_id)
            .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(CommerceError::VariantNotFound(variant_id))?;

        let old_quantity = variant.inventory_quantity;

        let mut variant_active: entities::product_variant::ActiveModel = variant.clone().into();
        variant_active.inventory_quantity = Set(quantity);
        variant_active.updated_at = Set(Utc::now().into());
        variant_active.update(&txn).await?;

        // Create and validate event
        let event = DomainEvent::InventoryUpdated {
            variant_id,
            product_id: variant.product_id,
            location_id: Uuid::new_v4(),
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
        let variant = entities::product_variant::Entity::find_by_id(variant_id)
            .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(CommerceError::VariantNotFound(variant_id))?;

        if variant.inventory_policy == "continue" {
            return Ok(true);
        }

        Ok(variant.inventory_quantity >= requested_quantity)
    }

    #[instrument(skip(self))]
    pub async fn reserve(
        &self,
        tenant_id: Uuid,
        variant_id: Uuid,
        quantity: i32,
    ) -> CommerceResult<()> {
        if !self
            .check_availability(tenant_id, variant_id, quantity)
            .await?
        {
            let variant = entities::product_variant::Entity::find_by_id(variant_id)
                .one(&self.db)
                .await?
                .ok_or(CommerceError::VariantNotFound(variant_id))?;

            return Err(CommerceError::InsufficientInventory {
                requested: quantity,
                available: variant.inventory_quantity,
            });
        }

        Ok(())
    }
}
