use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "checkout_marketplace_economics_checkpoints")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub checkout_operation_id: Uuid,
    pub tenant_id: Uuid,
    pub order_id: Uuid,
    pub plan_hash: String,
    pub currency_code: String,
    pub allocation_count: i32,
    pub allocation_total_amount: i64,
    pub allocation_set_hash: String,
    pub assessment_count: i32,
    pub commission_total_amount: i64,
    pub seller_proceeds_total_amount: i64,
    pub assessment_set_hash: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::checkout_operation::Entity",
        from = "Column::CheckoutOperationId",
        to = "super::checkout_operation::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    CheckoutOperation,
}

impl Related<super::checkout_operation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CheckoutOperation.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
