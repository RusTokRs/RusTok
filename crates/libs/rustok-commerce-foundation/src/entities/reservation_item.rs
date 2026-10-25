use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "reservation_items")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub inventory_item_id: Uuid,
    pub location_id: Uuid,
    pub quantity: i32,
    pub line_item_id: Option<Uuid>,
    pub description: Option<String>,
    pub external_id: Option<String>,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub deleted_at: Option<DateTimeWithTimeZone>,
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
    /// Returns true if reservation item is active (not soft-deleted and has positive quantity).
    #[inline]
    pub fn is_active(&self) -> bool {
        self.deleted_at.is_none() && self.quantity > 0
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_reservation_item_is_active() {
        let active = Model {
            id: Uuid::new_v4(),
            inventory_item_id: Uuid::new_v4(),
            location_id: Uuid::new_v4(),
            quantity: 3,
            line_item_id: None,
            description: None,
            external_id: None,
            metadata: serde_json::json!({}),
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
            deleted_at: None,
        };
        assert!(active.is_active());

        let deleted = Model {
            deleted_at: Some(Utc::now().into()),
            ..active.clone()
        };
        assert!(!deleted.is_active());

        let zero_qty = Model {
            quantity: 0,
            ..active
        };
        assert!(!zero_qty.is_active());
    }
}
