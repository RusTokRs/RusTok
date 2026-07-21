use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "oauth_apps")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub slug: String,
    pub app_type: String,
    pub icon_url: Option<String>,
    pub client_id: Uuid,
    pub client_secret_hash: Option<String>,
    pub redirect_uris: Json,
    pub scopes: Json,
    pub grant_types: Json,
    pub granted_permissions: Json,
    pub manifest_ref: Option<String>,
    pub auto_created: bool,
    pub is_active: bool,
    pub revoked_at: Option<DateTimeWithTimeZone>,
    pub last_used_at: Option<DateTimeWithTimeZone>,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::tenants::Entity",
        from = "Column::TenantId",
        to = "super::tenants::Column::Id"
    )]
    Tenant,
    #[sea_orm(has_many = "super::oauth_app_translations::Entity")]
    Translations,
    #[sea_orm(has_many = "super::oauth_tokens::Entity")]
    OAuthTokens,
}

impl Related<super::tenants::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tenant.def()
    }
}

impl Related<super::oauth_app_translations::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Translations.def()
    }
}

impl Related<super::oauth_tokens::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OAuthTokens.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
