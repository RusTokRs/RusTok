use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "checkout_inventory_reservations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub reservation_id: Uuid,
    pub tenant_id: Uuid,
    pub checkout_operation_id: Uuid,
    pub cart_line_item_id: Uuid,
    pub order_line_item_id: Option<Uuid>,
    pub external_id: String,
    pub variant_id: Uuid,
    pub quantity: i32,
    pub location_id: Option<Uuid>,
    pub status: String,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub released_at: Option<DateTimeWithTimeZone>,
    pub consumed_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
