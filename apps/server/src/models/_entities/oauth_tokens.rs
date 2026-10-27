use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "oauth_tokens")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub app_id: Uuid,
    pub user_id: Option<Uuid>,
    pub tenant_id: Uuid,
    #[serde(skip)]
    pub token_hash: String,
    /// Id of the first token in this refresh-token lineage (the root
    /// token carries its own id). NULL on pre-migration rows, which
    /// code coalesces to the row id.
    pub family_id: Option<Uuid>,
    pub grant_type: String,
    pub scopes: Json,
    pub expires_at: DateTimeWithTimeZone,
    pub revoked_at: Option<DateTimeWithTimeZone>,
    pub last_used_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

impl std::fmt::Debug for Model {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Model")
            .field("id", &self.id)
            .field("app_id", &self.app_id)
            .field("user_id", &self.user_id)
            .field("tenant_id", &self.tenant_id)
            .field("token_hash", &"<redacted>")
            .field("family_id", &self.family_id)
            .field("grant_type", &self.grant_type)
            .field("scopes", &self.scopes)
            .field("expires_at", &self.expires_at)
            .field("revoked_at", &self.revoked_at)
            .field("last_used_at", &self.last_used_at)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .finish()
    }
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
