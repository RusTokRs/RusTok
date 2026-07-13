use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "auth_invite_consumptions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub token_hash: String,
    pub email: String,
    pub role: String,
    pub user_id: Option<Uuid>,
    pub expires_at: DateTimeWithTimeZone,
    pub consumed_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::tenants::Entity",
        from = "Column::TenantId",
        to = "super::tenants::Column::Id"
    )]
    Tenant,
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",
        to = "super::users::Column::Id"
    )]
    User,
}

impl Related<super::tenants::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tenant.def()
    }
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
