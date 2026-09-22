use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "mcp_audit_logs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub client_id: Option<Uuid>,
    pub token_id: Option<Uuid>,
    pub actor_id: Option<String>,
    pub actor_type: Option<String>,
    pub action: String,
    pub outcome: String,
    pub tool_name: Option<String>,
    pub reason: Option<String>,
    pub correlation_id: Option<String>,
    pub metadata: Json,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
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
        belongs_to = "super::mcp_clients::Entity",
        from = "Column::ClientId",
        to = "super::mcp_clients::Column::Id"
    )]
    McpClient,
    #[sea_orm(
        belongs_to = "super::mcp_tokens::Entity",
        from = "Column::TokenId",
        to = "super::mcp_tokens::Column::Id"
    )]
    McpToken,
}

impl Related<super::tenants::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tenant.def()
    }
}

impl Related<super::mcp_clients::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::McpClient.def()
    }
}

impl Related<super::mcp_tokens::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::McpToken.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
