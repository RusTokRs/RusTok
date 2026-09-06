use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "orders")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub customer_id: Option<Uuid>,
    pub status: String,
    pub currency_code: String,
    pub shipping_total: Decimal,
    pub total_amount: Decimal,
    pub tax_total: Decimal,
    pub tax_included: bool,
    pub metadata: Json,
    pub payment_id: Option<String>,
    pub payment_method: Option<String>,
    pub tracking_number: Option<String>,
    pub carrier: Option<String>,
    pub cancellation_reason: Option<String>,
    pub delivered_signature: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub confirmed_at: Option<DateTimeWithTimeZone>,
    pub paid_at: Option<DateTimeWithTimeZone>,
    pub shipped_at: Option<DateTimeWithTimeZone>,
    pub delivered_at: Option<DateTimeWithTimeZone>,
    pub cancelled_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::order_line_item::Entity")]
    LineItems,
    #[sea_orm(has_many = "super::order_adjustment::Entity")]
    Adjustments,
    #[sea_orm(has_many = "super::order_tax_line::Entity")]
    TaxLines,
}

impl Related<super::order_line_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::LineItems.def()
    }
}

impl Related<super::order_adjustment::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Adjustments.def()
    }
}

impl Related<super::order_tax_line::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TaxLines.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
