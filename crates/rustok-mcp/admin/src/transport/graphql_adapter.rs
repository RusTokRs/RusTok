//! Headless GraphQL adapter contract for MCP admin transports.

use serde::{Deserialize, Serialize};

use crate::model::{
    ApplyMcpScaffoldDraftPayload, CreateMcpClientPayload, RotateMcpTokenPayload,
    StageMcpScaffoldDraftPayload, UpdateMcpPolicyPayload,
};

pub const MCP_SCAFFOLD_DRAFTS_OPERATION: &str = "McpScaffoldDrafts";
pub const STAGE_MCP_SCAFFOLD_DRAFT_OPERATION: &str = "StageMcpScaffoldDraft";
pub const APPLY_MCP_SCAFFOLD_DRAFT_OPERATION: &str = "ApplyMcpScaffoldDraft";
pub const MCP_AUDIT_EVENTS_OPERATION: &str = "McpAuditEvents";
pub const MCP_CLIENTS_OPERATION: &str = "McpClients";
pub const MCP_CLIENT_DETAILS_OPERATION: &str = "McpClientDetails";
pub const CREATE_MCP_CLIENT_OPERATION: &str = "CreateMcpClient";
pub const ROTATE_MCP_TOKEN_OPERATION: &str = "RotateMcpToken";
pub const UPDATE_MCP_POLICY_OPERATION: &str = "UpdateMcpPolicy";
pub const REVOKE_MCP_TOKEN_OPERATION: &str = "RevokeMcpToken";
pub const DEACTIVATE_MCP_CLIENT_OPERATION: &str = "DeactivateMcpClient";

pub const MCP_SCAFFOLD_DRAFTS_QUERY: &str = r#"
query McpScaffoldDrafts {
  mcpModuleScaffoldDrafts(limit: 20) {
    id
    clientId
    slug
    crateName
    status
    requestJson
    previewJson
    workspaceRoot
    appliedAt
    createdAt
    updatedAt
  }
}
"#;

pub const STAGE_MCP_SCAFFOLD_DRAFT_MUTATION: &str = r#"
mutation StageMcpScaffoldDraft($input: StageMcpModuleScaffoldDraftInput!) {
  stageMcpModuleScaffoldDraft(input: $input) {
    id
    clientId
    slug
    crateName
    status
    requestJson
    previewJson
    workspaceRoot
    appliedAt
    createdAt
    updatedAt
  }
}
"#;

pub const APPLY_MCP_SCAFFOLD_DRAFT_MUTATION: &str = r#"
mutation ApplyMcpScaffoldDraft(
  $draftId: UUID!
  $input: ApplyMcpModuleScaffoldDraftInput!
) {
  applyMcpModuleScaffoldDraft(draftId: $draftId, input: $input) {
    id
    clientId
    slug
    crateName
    status
    requestJson
    previewJson
    workspaceRoot
    appliedAt
    createdAt
    updatedAt
  }
}
"#;

pub const MCP_AUDIT_EVENTS_QUERY: &str = r#"
query McpAuditEvents {
  mcpAuditEvents(limit: 30) {
    id
    clientId
    actorId
    actorType
    action
    outcome
    toolName
    reason
    correlationId
    createdAt
  }
}
"#;

pub const MCP_CLIENTS_QUERY: &str = r#"
query McpClients {
  mcpClients(limit: 50) {
    id
    slug
    displayName
    description
    actorType
    isActive
    lastUsedAt
    createdAt
  }
}
"#;

pub const MCP_CLIENT_DETAILS_QUERY: &str = r#"
query McpClientDetails($id: UUID!) {
  mcpClient(id: $id) {
    client {
      id
      slug
      displayName
      description
      actorType
      isActive
      lastUsedAt
      createdAt
    }
    policy {
      allowedTools
      deniedTools
      grantedPermissions
      grantedScopes
      updatedAt
    }
    tokens {
      id
      tokenName
      tokenPreview
      isActive
      lastUsedAt
      expiresAt
      createdAt
    }
  }
}
"#;

pub const CREATE_MCP_CLIENT_MUTATION: &str = r#"
mutation CreateMcpClient($input: CreateMcpClientInput!) {
  createMcpClient(input: $input) {
    client { id slug displayName actorType isActive }
    token { id tokenName tokenPreview isActive }
    plaintextToken
  }
}
"#;

pub const ROTATE_MCP_TOKEN_MUTATION: &str = r#"
mutation RotateMcpToken($clientId: UUID!, $input: RotateMcpTokenInput!) {
  rotateMcpClientToken(clientId: $clientId, input: $input) {
    client { id slug displayName actorType isActive }
    token { id tokenName tokenPreview isActive }
    plaintextToken
  }
}
"#;

pub const UPDATE_MCP_POLICY_MUTATION: &str = r#"
mutation UpdateMcpPolicy($clientId: UUID!, $input: UpdateMcpPolicyInput!) {
  updateMcpClientPolicy(clientId: $clientId, input: $input) {
    clientId
    allowedTools
    deniedTools
    grantedPermissions
    grantedScopes
  }
}
"#;

pub const REVOKE_MCP_TOKEN_MUTATION: &str = r#"
mutation RevokeMcpToken($tokenId: UUID!, $reason: String) {
  revokeMcpToken(tokenId: $tokenId, reason: $reason)
}
"#;

pub const DEACTIVATE_MCP_CLIENT_MUTATION: &str = r#"
mutation DeactivateMcpClient($clientId: UUID!, $reason: String) {
  deactivateMcpClient(clientId: $clientId, reason: $reason)
}
"#;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct McpGraphqlRequest<V> {
    pub operation_name: &'static str,
    pub query: &'static str,
    pub variables: V,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct EmptyVariables {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpClientDetailsVariables {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateMcpClientVariables {
    pub input: CreateMcpClientPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RotateMcpTokenVariables {
    pub client_id: String,
    pub input: RotateMcpTokenGraphqlInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RotateMcpTokenGraphqlInput {
    pub token_name: Option<String>,
    pub expires_at: Option<String>,
    pub revoke_existing_tokens: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMcpPolicyVariables {
    pub client_id: String,
    pub input: UpdateMcpPolicyGraphqlInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMcpPolicyGraphqlInput {
    pub allowed_tools: Vec<String>,
    pub denied_tools: Vec<String>,
    pub granted_permissions: Vec<String>,
    pub granted_scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RevokeMcpTokenVariables {
    pub token_id: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeactivateMcpClientVariables {
    pub client_id: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StageMcpScaffoldDraftVariables {
    pub input: StageMcpScaffoldDraftPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApplyMcpScaffoldDraftVariables {
    pub draft_id: String,
    pub input: ApplyMcpScaffoldDraftPayload,
}

pub fn scaffold_drafts_request() -> McpGraphqlRequest<EmptyVariables> {
    McpGraphqlRequest {
        operation_name: MCP_SCAFFOLD_DRAFTS_OPERATION,
        query: MCP_SCAFFOLD_DRAFTS_QUERY,
        variables: EmptyVariables {},
    }
}

pub fn audit_events_request() -> McpGraphqlRequest<EmptyVariables> {
    McpGraphqlRequest {
        operation_name: MCP_AUDIT_EVENTS_OPERATION,
        query: MCP_AUDIT_EVENTS_QUERY,
        variables: EmptyVariables {},
    }
}

pub fn clients_request() -> McpGraphqlRequest<EmptyVariables> {
    McpGraphqlRequest {
        operation_name: MCP_CLIENTS_OPERATION,
        query: MCP_CLIENTS_QUERY,
        variables: EmptyVariables {},
    }
}

pub fn client_details_request(id: String) -> McpGraphqlRequest<McpClientDetailsVariables> {
    McpGraphqlRequest {
        operation_name: MCP_CLIENT_DETAILS_OPERATION,
        query: MCP_CLIENT_DETAILS_QUERY,
        variables: McpClientDetailsVariables { id },
    }
}

pub fn create_client_request(
    input: CreateMcpClientPayload,
) -> McpGraphqlRequest<CreateMcpClientVariables> {
    McpGraphqlRequest {
        operation_name: CREATE_MCP_CLIENT_OPERATION,
        query: CREATE_MCP_CLIENT_MUTATION,
        variables: CreateMcpClientVariables { input },
    }
}

pub fn rotate_token_request(
    input: RotateMcpTokenPayload,
) -> McpGraphqlRequest<RotateMcpTokenVariables> {
    McpGraphqlRequest {
        operation_name: ROTATE_MCP_TOKEN_OPERATION,
        query: ROTATE_MCP_TOKEN_MUTATION,
        variables: RotateMcpTokenVariables {
            client_id: input.client_id.clone(),
            input: RotateMcpTokenGraphqlInput {
                token_name: optional_graphql_string(input.token_name),
                expires_at: optional_graphql_string(input.expires_at),
                revoke_existing_tokens: input.revoke_existing_tokens,
            },
        },
    }
}

pub fn update_policy_request(
    input: UpdateMcpPolicyPayload,
) -> McpGraphqlRequest<UpdateMcpPolicyVariables> {
    McpGraphqlRequest {
        operation_name: UPDATE_MCP_POLICY_OPERATION,
        query: UPDATE_MCP_POLICY_MUTATION,
        variables: UpdateMcpPolicyVariables {
            client_id: input.client_id.clone(),
            input: UpdateMcpPolicyGraphqlInput {
                allowed_tools: input.allowed_tools,
                denied_tools: input.denied_tools,
                granted_permissions: input.granted_permissions,
                granted_scopes: input.granted_scopes,
            },
        },
    }
}

fn optional_graphql_string(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotate_token_variables_keep_client_id_outside_input() {
        let request = rotate_token_request(RotateMcpTokenPayload {
            client_id: "client-1".to_string(),
            token_name: "rotated".to_string(),
            expires_at: String::new(),
            revoke_existing_tokens: true,
        });
        let json = serde_json::to_value(request.variables).expect("variables serialize");

        assert_eq!(json["clientId"], "client-1");
        assert!(json["input"].get("clientId").is_none());
        assert_eq!(json["input"]["tokenName"], "rotated");
        assert!(json["input"]["expiresAt"].is_null());
        assert_eq!(json["input"]["revokeExistingTokens"], true);
    }

    #[test]
    fn policy_variables_use_graphql_camel_case() {
        let request = update_policy_request(UpdateMcpPolicyPayload {
            client_id: "client-1".to_string(),
            allowed_tools: vec!["modules.list".to_string()],
            denied_tools: Vec::new(),
            granted_permissions: vec!["mcp:read".to_string()],
            granted_scopes: vec!["tenant".to_string()],
        });
        let json = serde_json::to_value(request.variables).expect("variables serialize");

        assert_eq!(json["clientId"], "client-1");
        assert!(json["input"].get("clientId").is_none());
        assert_eq!(json["input"]["allowedTools"][0], "modules.list");
        assert_eq!(json["input"]["grantedPermissions"][0], "mcp:read");
        assert_eq!(json["input"]["grantedScopes"][0], "tenant");
    }
}

pub fn revoke_token_request(
    token_id: String,
    reason: Option<String>,
) -> McpGraphqlRequest<RevokeMcpTokenVariables> {
    McpGraphqlRequest {
        operation_name: REVOKE_MCP_TOKEN_OPERATION,
        query: REVOKE_MCP_TOKEN_MUTATION,
        variables: RevokeMcpTokenVariables { token_id, reason },
    }
}

pub fn deactivate_client_request(
    client_id: String,
    reason: Option<String>,
) -> McpGraphqlRequest<DeactivateMcpClientVariables> {
    McpGraphqlRequest {
        operation_name: DEACTIVATE_MCP_CLIENT_OPERATION,
        query: DEACTIVATE_MCP_CLIENT_MUTATION,
        variables: DeactivateMcpClientVariables { client_id, reason },
    }
}

pub fn stage_scaffold_draft_request(
    input: StageMcpScaffoldDraftPayload,
) -> McpGraphqlRequest<StageMcpScaffoldDraftVariables> {
    McpGraphqlRequest {
        operation_name: STAGE_MCP_SCAFFOLD_DRAFT_OPERATION,
        query: STAGE_MCP_SCAFFOLD_DRAFT_MUTATION,
        variables: StageMcpScaffoldDraftVariables { input },
    }
}

pub fn apply_scaffold_draft_request(
    input: ApplyMcpScaffoldDraftPayload,
) -> McpGraphqlRequest<ApplyMcpScaffoldDraftVariables> {
    McpGraphqlRequest {
        operation_name: APPLY_MCP_SCAFFOLD_DRAFT_OPERATION,
        query: APPLY_MCP_SCAFFOLD_DRAFT_MUTATION,
        variables: ApplyMcpScaffoldDraftVariables {
            draft_id: input.draft_id.clone(),
            input,
        },
    }
}
