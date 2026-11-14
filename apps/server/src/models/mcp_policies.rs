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
    pub fn allowed_tools_list(&self) -> Result<Vec<String>, String> {
        parse_string_list("allowed_tools", &self.allowed_tools)
    }

    pub fn denied_tools_list(&self) -> Result<Vec<String>, String> {
        parse_string_list("denied_tools", &self.denied_tools)
    }

    pub fn granted_permissions_list(&self) -> Result<Vec<String>, String> {
        parse_string_list("granted_permissions", &self.granted_permissions)
    }

    pub fn granted_scopes_list(&self) -> Result<Vec<String>, String> {
        parse_string_list("granted_scopes", &self.granted_scopes)
    }

    pub fn to_access_context(
        &self,
        client: &mcp_clients::Model,
        identity: Option<McpIdentity>,
    ) -> Result<McpAccessContext, String> {
        let granted_scopes = self.granted_scopes_list()?;
        let identity = identity.or_else(|| {
            Some(McpIdentity {
                actor_id: client.id.to_string(),
                actor_type: client.actor_type(),
                tenant_id: Some(client.tenant_id.to_string()),
                delegated_user_id: client.delegated_user_id.map(|value| value.to_string()),
                display_name: Some(client.display_name.clone()),
                scopes: granted_scopes.clone(),
            })
        });

        Ok(McpAccessContext {
            identity,
            granted_permissions: self.granted_permissions_list()?,
            policy: McpAccessPolicy {
                allowed_tools: Some(self.allowed_tools_list()?),
                denied_tools: self.denied_tools_list()?,
            },
        })
    }
}

fn parse_string_list(field: &str, value: &Json) -> Result<Vec<String>, String> {
    serde_json::from_value(value.clone())
        .map_err(|error| format!("persisted MCP policy field `{field}` is invalid: {error}"))
}

#[cfg(test)]
mod tests {
    use super::parse_string_list;
    use serde_json::json;

    #[test]
    fn policy_list_parser_accepts_string_arrays() {
        assert_eq!(
            parse_string_list("denied_tools", &json!(["tool:read", "tool:write"]))
                .unwrap(),
            vec!["tool:read", "tool:write"]
        );
    }

    #[test]
    fn policy_list_parser_rejects_malformed_or_non_string_values() {
        for invalid in [json!("not-an-array"), json!(["tool:read", 7])] {
            let error = parse_string_list("denied_tools", &invalid).unwrap_err();
            assert!(error.contains("denied_tools"));
        }
    }
}
