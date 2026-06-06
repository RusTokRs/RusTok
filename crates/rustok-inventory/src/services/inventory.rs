use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
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

impl InventoryReservationReleaseWriteResult {
    fn from_quantities(
        released_quantity: i32,
        available_quantity: i32,
        inventory_policy: &str,
    ) -> Self {
        Self {
            released_quantity,
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
    pub async fn check_variant_availability(
        &self,
        tenant_id: Uuid,
        variant_id: Uuid,
        requested_quantity: i32,
    ) -> CommerceResult<InventoryAvailabilityCheckResult> {
        let available = self
            .check_availability(tenant_id, variant_id, requested_quantity)
            .await?;

        Ok(InventoryAvailabilityCheckResult { available })
    }

    pub async fn check_availability(
        &self,
        tenant_id: Uuid,
        variant_id: Uuid,
        requested_quantity: i32,
    ) -> CommerceResult<bool> {
        validate_availability_request_quantity(requested_quantity)?;

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

    #[instrument(skip(self))]
    pub async fn release_reservation_quantity(
        &self,
        tenant_id: Uuid,
        variant_id: Uuid,
        quantity: i32,
    ) -> CommerceResult<InventoryReservationReleaseWriteResult> {
        validate_release_quantity(quantity)?;

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
            return Ok(InventoryReservationReleaseWriteResult::from_quantities(
                0,
                available,
                variant.inventory_policy.as_str(),
            ));
        }

        let Some(inventory_item) = entities::inventory_item::Entity::find()
            .filter(entities::inventory_item::Column::VariantId.eq(variant_id))
            .one(&txn)
            .await?
        else {
            return Err(insufficient_reserved_release_error(quantity, 0));
        };

        let levels = entities::inventory_level::Entity::find()
            .filter(entities::inventory_level::Column::InventoryItemId.eq(inventory_item.id))
            .all(&txn)
            .await?;
        let total_reserved_quantity = levels
            .iter()
            .map(|level| level.reserved_quantity)
            .sum::<i32>();
        if quantity > total_reserved_quantity {
            return Err(insufficient_reserved_release_error(
                quantity,
                total_reserved_quantity,
            ));
        }

        let reservation_items = entities::reservation_item::Entity::find()
            .filter(entities::reservation_item::Column::InventoryItemId.eq(inventory_item.id))
            .filter(entities::reservation_item::Column::DeletedAt.is_null())
            .order_by_asc(entities::reservation_item::Column::CreatedAt)
            .all(&txn)
            .await?;
        let tracked_reservation_quantity = reservation_items
            .iter()
            .map(|item| item.quantity.max(0))
            .sum::<i32>();
        if quantity > tracked_reservation_quantity {
            return Err(insufficient_reservation_items_release_error(
                quantity,
                tracked_reservation_quantity,
            ));
        }

        release_reservation_items(&txn, reservation_items, quantity).await?;
        release_reserved_quantity_from_levels(&txn, levels, quantity).await?;

        let available_quantity = self.available_quantity(&txn, inventory_item.id).await?;
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

async fn release_reservation_items<C>(
    conn: &C,
    reservation_items: Vec<entities::reservation_item::Model>,
    quantity: i32,
) -> CommerceResult<()>
where
    C: sea_orm::ConnectionTrait,
{
    let mut remaining_quantity = quantity;
    for item in reservation_items {
        if remaining_quantity == 0 {
            break;
        }
        if item.quantity <= 0 {
            continue;
        }

        let released_quantity = remaining_quantity.min(item.quantity);
        let new_quantity = item.quantity - released_quantity;
        remaining_quantity -= released_quantity;

        let mut item_active: entities::reservation_item::ActiveModel = item.into();
        item_active.quantity = Set(new_quantity);
        item_active.updated_at = Set(Utc::now().into());
        if new_quantity == 0 {
            item_active.deleted_at = Set(Some(Utc::now().into()));
        }
        item_active.update(conn).await?;
    }

    Ok(())
}

async fn release_reserved_quantity_from_levels<C>(
    conn: &C,
    levels: Vec<entities::inventory_level::Model>,
    quantity: i32,
) -> CommerceResult<()>
where
    C: sea_orm::ConnectionTrait,
{
    let mut remaining_quantity = quantity;
    for level in levels {
        if remaining_quantity == 0 {
            break;
        }
        if level.reserved_quantity <= 0 {
            continue;
        }

        let released_quantity = remaining_quantity.min(level.reserved_quantity);
        let new_reserved_quantity = level.reserved_quantity - released_quantity;
        remaining_quantity -= released_quantity;

        let mut level_active: entities::inventory_level::ActiveModel = level.into();
        level_active.reserved_quantity = Set(new_reserved_quantity);
        level_active.updated_at = Set(Utc::now().into());
        level_active.update(conn).await?;
    }

    Ok(())
}

fn insufficient_reserved_release_error(requested: i32, reserved: i32) -> CommerceError {
    CommerceError::Validation(format!(
        "Cannot release {requested} reserved units; only {reserved} are reserved"
    ))
}

fn insufficient_reservation_items_release_error(requested: i32, tracked: i32) -> CommerceError {
    CommerceError::Validation(format!(
        "Cannot release {requested} reservation item units; only {tracked} are tracked"
    ))
}

fn validate_release_quantity(quantity: i32) -> CommerceResult<()> {
    if quantity < 0 {
        return Err(CommerceError::Validation(
            "Reservation release quantity must be non-negative".to_string(),
        ));
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

#[cfg(test)]
mod tests {
    use super::{
        insufficient_reservation_items_release_error, insufficient_reserved_release_error,
        validate_availability_request_quantity, validate_release_quantity,
        InventoryAvailabilityCheckResult, InventoryQuantityWriteResult,
        InventoryReservationReleaseWriteResult, InventoryReservationWriteResult,
    };

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
