//! Inventory-owned physical-lifecycle projection, not Fulfillment progress or a retry receipt.
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "inventory_shipment_stock_proofs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub fulfillment_item_id: Uuid,
    pub tenant_id: Uuid,
    pub order_id: Uuid,
    pub fulfillment_id: Uuid,
    pub order_line_item_id: Uuid,
    pub variant_id: Option<Uuid>,
    pub fulfillment_quantity: i32,
    pub inventory_item_id: Option<Uuid>,
    pub reservation_id: Option<Uuid>,
    pub location_id: Option<Uuid>,
    pub shipped_quantity: i32,
    pub restored_quantity: i32,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
