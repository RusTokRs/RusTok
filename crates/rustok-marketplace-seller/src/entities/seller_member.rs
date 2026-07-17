use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "marketplace_seller_members")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub seller_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
    pub status: String,
    pub invited_by_actor_id: Option<Uuid>,
    pub accepted_at: Option<DateTimeWithTimeZone>,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::seller::Entity",
        from = "Column::SellerId",
        to = "super::seller::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Seller,
}

impl Related<super::seller::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Seller.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
