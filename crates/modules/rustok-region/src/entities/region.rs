use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "regions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub currency_code: String,
    pub tax_provider_id: Option<String>,
    pub tax_rate: Decimal,
    pub tax_included: bool,
    pub countries: Json,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl Related<super::region_country_tax_policy::Entity> for Entity {
    fn to() -> RelationDef {
        super::region_country_tax_policy::Relation::Region.def().rev()
    }
}

impl ActiveModelBehavior for ActiveModel {}
