use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "product_variants")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub product_id: Uuid,
    pub tenant_id: Uuid,
    pub sku: Option<String>,
    pub barcode: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub ean: Option<String>,
    pub upc: Option<String>,
    pub inventory_policy: String,
    pub inventory_management: String,
    pub inventory_quantity: i32,
    pub weight: Option<Decimal>,
    pub weight_unit: Option<String>,
    pub combination_identity: Option<String>,
    pub position: i32,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::product::Entity",
        from = "Column::ProductId",
        to = "super::product::Column::Id"
    )]
    Product,
}

impl Related<super::product::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Product.def()
    }
}

impl Model {
    /// Returns true if variant has positive inventory or allows backorders.
    #[inline]
    pub fn is_in_stock(&self) -> bool {
        self.inventory_quantity > 0 || self.inventory_policy == "continue"
    }

    /// Returns true if variant allows backorders when out of stock.
    #[inline]
    pub fn allows_backorders(&self) -> bool {
        self.inventory_policy == "continue"
    }

    /// Returns true if inventory is tracked by the platform.
    #[inline]
    pub fn is_inventory_tracked(&self) -> bool {
        self.inventory_management != "none" && !self.inventory_management.trim().is_empty()
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_product_variant_model_helpers() {
        let in_stock_variant = Model {
            id: Uuid::new_v4(),
            product_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            sku: Some("SKU-1".into()),
            barcode: None,
            shipping_profile_slug: None,
            ean: None,
            upc: None,
            inventory_policy: "deny".into(),
            inventory_management: "rustok".into(),
            inventory_quantity: 5,
            weight: None,
            weight_unit: None,
            combination_identity: None,
            position: 1,
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
        };
        assert!(in_stock_variant.is_in_stock());
        assert!(!in_stock_variant.allows_backorders());
        assert!(in_stock_variant.is_inventory_tracked());

        let out_of_stock_backorder = Model {
            inventory_quantity: 0,
            inventory_policy: "continue".into(),
            ..in_stock_variant.clone()
        };
        assert!(out_of_stock_backorder.is_in_stock());
        assert!(out_of_stock_backorder.allows_backorders());

        let untracked_variant = Model {
            inventory_management: "none".into(),
            ..in_stock_variant
        };
        assert!(!untracked_variant.is_inventory_tracked());
    }
}
