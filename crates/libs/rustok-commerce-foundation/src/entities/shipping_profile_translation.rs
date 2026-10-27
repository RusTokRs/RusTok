use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "shipping_profile_translations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub shipping_profile_id: Uuid,
    pub locale: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::shipping_profile::Entity",
        from = "Column::ShippingProfileId",
        to = "super::shipping_profile::Column::Id"
    )]
    ShippingProfile,
}

impl Related<super::shipping_profile::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ShippingProfile.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
