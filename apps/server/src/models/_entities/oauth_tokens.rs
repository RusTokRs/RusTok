use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "oauth_tokens")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub app_id: Uuid,
    pub user_id: Option<Uuid>,
    pub tenant_id: Uuid,
    pub token_hash: String,
    pub grant_type: String,
    pub scopes: Json,
    pub expires_at: DateTimeWithTimeZone,
    pub revoked_at: Option<DateTimeWithTimeZone>,
    pub last_used_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::oauth_apps::Entity",
        from = "Column::AppId",
        to = "super::oauth_apps::Column::Id"
    )]
    OAuthApp,
    #[sea_orm(
        belongs_to = "super::tenants::Entity",
        from = "Column::TenantId",
        to = "super::tenants::Column::Id"
    )]
    Tenant,
}

impl Related<super::oauth_apps::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OAuthApp.def()
    }
}

impl Related<super::tenants::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tenant.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
