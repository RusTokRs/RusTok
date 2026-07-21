#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{execute as execute_graphql, GraphqlRequest};
use serde::{Deserialize, Serialize};

use crate::model::{
    CreateGroupInvitationCommand, GroupsAdminCreateInvitationResult, GroupsAdminInvitation,
    GroupsAdminInvitationConnection, GroupsAdminInvitationQuery,
    GroupsAdminRevokeInvitationResult, RevokeGroupInvitationCommand,
};

pub type GraphqlGroupsInvitationError = String;

const LIST_INVITATIONS_QUERY: &str = "query GroupsAdminInvitations($groupId: UUID!, $page: Int, $perPage: Int, $includeInactive: Boolean) { group_invitations: groupInvitations(groupId: $groupId, page: $page, perPage: $perPage, includeInactive: $includeInactive) { total page per_page: perPage items { id group_id: groupId invited_by_user_id: invitedByUserId target_user_id: targetUserId max_uses: maxUses use_count: useCount expires_at: expiresAt revoked_at: revokedAt revoked_by_user_id: revokedByUserId created_at: createdAt status } } }";
const CREATE_INVITATION_MUTATION: &str = "mutation GroupsAdminCreateInvitation($idempotencyKey: String!, $groupId: UUID!, $input: CreateGroupInvitationInputGql!) { create_group_invitation: createGroupInvitation(idempotencyKey: $idempotencyKey, groupId: $groupId, input: $input) { invitation { id group_id: groupId invited_by_user_id: invitedByUserId target_user_id: targetUserId max_uses: maxUses use_count: useCount expires_at: expiresAt revoked_at: revokedAt revoked_by_user_id: revokedByUserId created_at: createdAt status } token group_version: groupVersion replayed } }";
const REVOKE_INVITATION_MUTATION: &str = "mutation GroupsAdminRevokeInvitation($idempotencyKey: String!, $invitationId: UUID!) { revoke_group_invitation: revokeGroupInvitation(idempotencyKey: $idempotencyKey, invitationId: $invitationId) { invitation { id group_id: groupId invited_by_user_id: invitedByUserId target_user_id: targetUserId max_uses: maxUses use_count: useCount expires_at: expiresAt revoked_at: revokedAt revoked_by_user_id: revokedByUserId created_at: createdAt status } group_version: groupVersion replayed } }";

#[derive(Debug, Serialize)]
struct ListVariables {
    #[serde(rename = "groupId")]
    group_id: String,
    page: i32,
    #[serde(rename = "perPage")]
    per_page: i32,
    #[serde(rename = "includeInactive")]
    include_inactive: bool,
}

#[derive(Debug, Serialize)]
struct CreateVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    #[serde(rename = "groupId")]
    group_id: String,
    input: CreateInput,
}

#[derive(Debug, Serialize)]
struct CreateInput {
    #[serde(rename = "targetUserId")]
    target_user_id: Option<String>,
    #[serde(rename = "expiresInSeconds")]
    expires_in_seconds: i64,
    #[serde(rename = "maxUses")]
    max_uses: i64,
}

#[derive(Debug, Serialize)]
struct RevokeVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    #[serde(rename = "invitationId")]
    invitation_id: String,
}

#[derive(Debug, Deserialize)]
struct ListResponse {
    group_invitations: InvitationConnectionWire,
}

#[derive(Debug, Deserialize)]
struct CreateResponse {
    create_group_invitation: CreateInvitationWire,
}

#[derive(Debug, Deserialize)]
struct RevokeResponse {
    revoke_group_invitation: RevokeInvitationWire,
}

#[derive(Debug, Deserialize)]
struct InvitationConnectionWire {
    items: Vec<InvitationWire>,
    total: u64,
    page: u64,
    per_page: u64,
}

#[derive(Debug, Deserialize)]
struct InvitationWire {
    id: String,
    group_id: String,
    invited_by_user_id: String,
    target_user_id: Option<String>,
    max_uses: u32,
    use_count: u32,
    expires_at: String,
    revoked_at: Option<String>,
    revoked_by_user_id: Option<String>,
    created_at: String,
    status: String,
}

#[derive(Debug, Deserialize)]
struct CreateInvitationWire {
    invitation: InvitationWire,
    token: Option<String>,
    group_version: u64,
    replayed: bool,
}

#[derive(Debug, Deserialize)]
struct RevokeInvitationWire {
    invitation: InvitationWire,
    group_version: u64,
    replayed: bool,
}

pub async fn load_group_invitations(
    token: Option<String>,
    tenant_slug: Option<String>,
    query: GroupsAdminInvitationQuery,
) -> Result<GroupsAdminInvitationConnection, GraphqlGroupsInvitationError> {
    let response: ListResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            LIST_INVITATIONS_QUERY,
            Some(ListVariables {
                group_id: query.group_id,
                page: query.page.max(1).min(i32::MAX as u64) as i32,
                per_page: query.per_page.clamp(1, 100).min(i32::MAX as u64) as i32,
                include_inactive: query.include_inactive,
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok(GroupsAdminInvitationConnection {
        items: response
            .group_invitations
            .items
            .into_iter()
            .map(Into::into)
            .collect(),
        total: response.group_invitations.total,
        page: response.group_invitations.page,
        per_page: response.group_invitations.per_page,
    })
}

pub async fn create_group_invitation(
    token: Option<String>,
    tenant_slug: Option<String>,
    command: CreateGroupInvitationCommand,
) -> Result<GroupsAdminCreateInvitationResult, GraphqlGroupsInvitationError> {
    let response: CreateResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            CREATE_INVITATION_MUTATION,
            Some(CreateVariables {
                idempotency_key: command.idempotency_key,
                group_id: command.group_id,
                input: CreateInput {
                    target_user_id: command.target_user_id,
                    expires_in_seconds: command.expires_in_seconds.min(i64::MAX as u64) as i64,
                    max_uses: command.max_uses as i64,
                },
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok(GroupsAdminCreateInvitationResult {
        invitation: response.create_group_invitation.invitation.into(),
        token: response.create_group_invitation.token,
        group_version: response.create_group_invitation.group_version,
        replayed: response.create_group_invitation.replayed,
    })
}

pub async fn revoke_group_invitation(
    token: Option<String>,
    tenant_slug: Option<String>,
    command: RevokeGroupInvitationCommand,
) -> Result<GroupsAdminRevokeInvitationResult, GraphqlGroupsInvitationError> {
    let response: RevokeResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            REVOKE_INVITATION_MUTATION,
            Some(RevokeVariables {
                idempotency_key: command.idempotency_key,
                invitation_id: command.invitation_id,
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok(GroupsAdminRevokeInvitationResult {
        invitation: response.revoke_group_invitation.invitation.into(),
        group_version: response.revoke_group_invitation.group_version,
        replayed: response.revoke_group_invitation.replayed,
    })
}

impl From<InvitationWire> for GroupsAdminInvitation {
    fn from(value: InvitationWire) -> Self {
        Self {
            id: value.id,
            group_id: value.group_id,
            invited_by_user_id: value.invited_by_user_id,
            target_user_id: value.target_user_id,
            max_uses: value.max_uses,
            use_count: value.use_count,
            expires_at: value.expires_at,
            revoked_at: value.revoked_at,
            revoked_by_user_id: value.revoked_by_user_id,
            created_at: value.created_at,
            status: value.status.to_ascii_lowercase(),
        }
    }
}

fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }
    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}/api/graphql")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}/api/graphql")
    }
}
