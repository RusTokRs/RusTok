use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "order_checkout_identities")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub checkout_operation_id: Uuid,
    pub tenant_id: Uuid,
    pub order_id: Uuid,
    pub source_cart_id: Option<Uuid>,
    pub payment_collection_id: Option<Uuid>,
    pub shipping_option_id: Option<Uuid>,
    pub snapshot_hash: Option<String>,
    pub request_hash: Option<String>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::order::Entity",
        from = "Column::OrderId",
        to = "super::order::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Order,
}

impl Related<super::order::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Order.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
