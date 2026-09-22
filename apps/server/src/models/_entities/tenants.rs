use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "tenants")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub name: String,
    #[sea_orm(unique)]
    pub slug: String,
    #[sea_orm(unique)]
    pub domain: Option<String>,
    pub settings: Json,
    pub default_locale: String,
    pub is_active: bool,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::users::Entity")]
    Users,
    #[sea_orm(has_many = "super::tenant_modules::Entity")]
    TenantModules,
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Users.def()
    }
}

impl Related<super::tenant_modules::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TenantModules.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
