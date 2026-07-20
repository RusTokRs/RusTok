use rustok_mcp::{McpAccessContext, McpAccessPolicy, McpIdentity};
use sea_orm::{QueryFilter, entity::prelude::*};

use crate::models::mcp_clients;

pub use super::_entities::mcp_policies::{ActiveModel, Column, Entity, Model, Relation};

impl Entity {
    pub async fn find_by_client(
        db: &DatabaseConnection,
        client_id: Uuid,
    ) -> Result<Option<Model>, DbErr> {
        Self::find()
            .filter(Column::ClientId.eq(client_id))
            .one(db)
            .await
    }
}

impl Model {
    pub fn allowed_tools_list(&self) -> Vec<String> {
        serde_json::from_value(self.allowed_tools.clone()).unwrap_or_default()
    }

    pub fn denied_tools_list(&self) -> Vec<String> {
        serde_json::from_value(self.denied_tools.clone()).unwrap_or_default()
    }

    pub fn granted_permissions_list(&self) -> Vec<String> {
        serde_json::from_value(self.granted_permissions.clone()).unwrap_or_default()
    }

    pub fn granted_scopes_list(&self) -> Vec<String> {
        serde_json::from_value(self.granted_scopes.clone()).unwrap_or_default()
    }

    pub fn to_access_context(
        &self,
        client: &mcp_clients::Model,
        identity: Option<McpIdentity>,
    ) -> McpAccessContext {
        let identity = identity.or_else(|| {
            Some(McpIdentity {
                actor_id: client.id.to_string(),
                actor_type: client.actor_type(),
                tenant_id: Some(client.tenant_id.to_string()),
                delegated_user_id: client.delegated_user_id.map(|value| value.to_string()),
                display_name: Some(client.display_name.clone()),
                scopes: self.granted_scopes_list(),
            })
        });

        McpAccessContext {
            identity,
            granted_permissions: self.granted_permissions_list(),
            policy: McpAccessPolicy {
                allowed_tools: Some(self.allowed_tools_list()),
                denied_tools: self.denied_tools_list(),
            },
        }
    }
}
