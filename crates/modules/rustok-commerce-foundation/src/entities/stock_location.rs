use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "stock_locations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub code: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub city: Option<String>,
    pub province: Option<String>,
    pub postal_code: Option<String>,
    pub country_code: Option<String>,
    pub phone: Option<String>,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub deleted_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::inventory_level::Entity")]
    InventoryLevels,
    #[sea_orm(has_many = "super::reservation_item::Entity")]
    ReservationItems,
}

impl Related<super::inventory_level::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::InventoryLevels.def()
    }
}

impl Related<super::reservation_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ReservationItems.def()
    }
}

impl Model {
    /// Returns true if stock location is active (not soft-deleted).
    #[inline]
    pub fn is_active(&self) -> bool {
        self.deleted_at.is_none()
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_stock_location_is_active() {
        let active = Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            code: Some("WH-01".into()),
            address_line1: None,
            address_line2: None,
            city: None,
            province: None,
            postal_code: None,
            country_code: Some("RU".into()),
            phone: None,
            metadata: serde_json::json!({}),
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
            deleted_at: None,
        };
        assert!(active.is_active());

        let deleted = Model {
            deleted_at: Some(Utc::now().into()),
            ..active
        };
        assert!(!deleted.is_active());
    }
}
