use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "mcp_clients")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub client_key: Uuid,
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    pub actor_type: String,
    pub delegated_user_id: Option<Uuid>,
    pub is_active: bool,
    pub revoked_at: Option<DateTimeWithTimeZone>,
    pub last_used_at: Option<DateTimeWithTimeZone>,
    pub metadata: Json,
    pub created_by: Option<Uuid>,
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
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::DelegatedUserId",
        to = "super::users::Column::Id"
    )]
    DelegatedUser,
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::CreatedBy",
        to = "super::users::Column::Id"
    )]
    CreatedByUser,
    #[sea_orm(has_many = "super::mcp_tokens::Entity")]
    McpTokens,
    #[sea_orm(has_one = "super::mcp_policies::Entity")]
    McpPolicy,
    #[sea_orm(has_many = "super::mcp_audit_logs::Entity")]
    McpAuditLogs,
}

impl Related<super::tenants::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tenant.def()
    }
}

impl Related<super::mcp_tokens::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::McpTokens.def()
    }
}

impl Related<super::mcp_policies::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::McpPolicy.def()
    }
}

impl Related<super::mcp_audit_logs::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::McpAuditLogs.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
