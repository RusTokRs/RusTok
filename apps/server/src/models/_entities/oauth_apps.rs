use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "oauth_apps")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    #[sea_orm(ignore)]
    pub name: String,
    pub slug: String,
    #[sea_orm(ignore)]
    pub description: Option<String>,
    pub app_type: String,
    pub icon_url: Option<String>,
    pub client_id: Uuid,
    #[serde(skip)]
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

impl std::fmt::Debug for Model {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Model")
            .field("id", &self.id)
            .field("tenant_id", &self.tenant_id)
            .field("name", &self.name)
            .field("slug", &self.slug)
            .field("description", &self.description)
            .field("app_type", &self.app_type)
            .field("icon_url", &self.icon_url)
            .field("client_id", &self.client_id)
            .field("client_secret_hash", &"<redacted>")
            .field("redirect_uris", &self.redirect_uris)
            .field("scopes", &self.scopes)
            .field("grant_types", &self.grant_types)
            .field("granted_permissions", &self.granted_permissions)
            .field("manifest_ref", &self.manifest_ref)
            .field("auto_created", &self.auto_created)
            .field("is_active", &self.is_active)
            .field("revoked_at", &self.revoked_at)
            .field("last_used_at", &self.last_used_at)
            .field("metadata", &self.metadata)
            .field("created_at", &self.created_at)
            .field("updated_at", &self.updated_at)
            .finish()
    }
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
