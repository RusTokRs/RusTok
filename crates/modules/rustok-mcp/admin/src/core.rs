use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct McpScaffoldDraftPayload {
    pub id: String,
    pub client_id: Option<String>,
    pub slug: String,
    pub crate_name: String,
    pub status: String,
    pub request_json: String,
    pub preview_json: String,
    pub workspace_root: Option<String>,
    pub applied_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StageMcpScaffoldDraftPayload {
    pub client_id: Option<String>,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub dependencies: Vec<String>,
    pub with_graphql: bool,
    pub with_rest: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApplyMcpScaffoldDraftPayload {
    pub draft_id: String,
    pub workspace_root: String,
    pub confirm: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct McpAuditEventPayload {
    pub id: String,
    pub client_id: Option<String>,
    pub actor_id: Option<String>,
    pub actor_type: Option<String>,
    pub action: String,
    pub outcome: String,
    pub tool_name: Option<String>,
    pub reason: Option<String>,
    pub correlation_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct McpClientPayload {
    pub id: String,
    pub slug: String,
    pub display_name: String,
    pub description: Option<String>,
    pub actor_type: String,
    pub is_active: bool,
    pub last_used_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct McpPolicyPayload {
    pub allowed_tools: Vec<String>,
    pub denied_tools: Vec<String>,
    pub granted_permissions: Vec<String>,
    pub granted_scopes: Vec<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct McpTokenPayload {
    pub id: String,
    pub token_name: String,
    pub token_preview: String,
    pub is_active: bool,
    pub last_used_at: Option<String>,
    pub expires_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct McpClientDetailsPayload {
    pub client: McpClientPayload,
    pub policy: Option<McpPolicyPayload>,
    pub tokens: Vec<McpTokenPayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateMcpClientPayload {
    pub slug: String,
    pub display_name: String,
    pub description: String,
    pub actor_type: String,
    pub token_name: String,
    pub token_expires_at: String,
    pub allowed_tools: Vec<String>,
    pub denied_tools: Vec<String>,
    pub granted_permissions: Vec<String>,
    pub granted_scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RotateMcpTokenPayload {
    pub client_id: String,
    pub token_name: String,
    pub expires_at: String,
    pub revoke_existing_tokens: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMcpPolicyPayload {
    pub client_id: String,
    pub allowed_tools: Vec<String>,
    pub denied_tools: Vec<String>,
    pub granted_permissions: Vec<String>,
    pub granted_scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct McpTokenSecretPayload {
    pub client_id: String,
    pub token_id: String,
    pub token_name: String,
    pub token_preview: String,
    pub plaintext_token: String,
}
