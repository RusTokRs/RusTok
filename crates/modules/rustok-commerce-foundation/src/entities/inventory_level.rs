use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "inventory_levels")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub inventory_item_id: Uuid,
    pub location_id: Uuid,
    pub stocked_quantity: i32,
    pub reserved_quantity: i32,
    pub incoming_quantity: i32,
    pub low_stock_threshold: Option<i32>,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::inventory_item::Entity",
        from = "Column::InventoryItemId",
        to = "super::inventory_item::Column::Id"
    )]
    InventoryItem,
    #[sea_orm(
        belongs_to = "super::stock_location::Entity",
        from = "Column::LocationId",
        to = "super::stock_location::Column::Id"
    )]
    Location,
}

impl Related<super::inventory_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::InventoryItem.def()
    }
}

impl Related<super::stock_location::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Location.def()
    }
}

impl Model {
    /// Calculates available stock quantity using saturating subtraction (stocked - reserved).
    #[inline]
    pub fn available_quantity(&self) -> i32 {
        self.stocked_quantity.saturating_sub(self.reserved_quantity)
    }

    /// Returns true if available quantity is at or below the low stock threshold.
    #[inline]
    pub fn is_low_stock(&self) -> bool {
        if let Some(threshold) = self.low_stock_threshold {
            self.available_quantity() <= threshold
        } else {
            false
        }
    }

    /// Checks if the requested quantity can be reserved without stock deficit.
    #[inline]
    pub fn can_reserve(&self, requested: i32) -> bool {
        requested > 0 && self.available_quantity() >= requested
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_inventory_level_calculations() {
        let level = Model {
            id: Uuid::new_v4(),
            inventory_item_id: Uuid::new_v4(),
            location_id: Uuid::new_v4(),
            stocked_quantity: 10,
            reserved_quantity: 3,
            incoming_quantity: 5,
            low_stock_threshold: Some(7),
            updated_at: Utc::now().into(),
        };

        assert_eq!(level.available_quantity(), 7);
        assert!(level.is_low_stock()); // available (7) <= threshold (7)
        assert!(level.can_reserve(5));
        assert!(level.can_reserve(7));
        assert!(!level.can_reserve(8)); // exceeds available
        assert!(!level.can_reserve(0)); // non-positive requested
        assert!(!level.can_reserve(-2));
    }
}
