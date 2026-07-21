#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{execute as execute_graphql, GraphqlRequest};
use serde::{Deserialize, Serialize};

use crate::model::{
    AcceptGroupInvitationCommand, GroupsStorefrontAcceptInvitationResult,
    GroupsStorefrontDirectory, GroupsStorefrontFilters, GroupsStorefrontListItem,
    GroupsStorefrontMembership,
};

pub type GraphqlGroupsStorefrontError = String;

const DIRECTORY_QUERY: &str = "query GroupsStorefrontDirectory($page: Int, $perPage: Int, $search: String) { groups(page: $page, perPage: $perPage, search: $search, includeNonPublic: false) { total page per_page: perPage items { id handle title summary visibility join_policy: joinPolicy member_count: memberCount effective_locale: effectiveLocale } } }";
const ACCEPT_INVITATION_MUTATION: &str = "mutation GroupsStorefrontAcceptInvitation($idempotencyKey: String!, $token: String!) { accept_group_invitation: acceptGroupInvitation(idempotencyKey: $idempotencyKey, token: $token) { invitation_id: invitationId group_id: groupId membership { id group_id: groupId user_id: userId role status } group_version: groupVersion replayed } }";

#[derive(Debug, Serialize)]
struct DirectoryVariables {
    page: i32,
    #[serde(rename = "perPage")]
    per_page: i32,
    search: Option<String>,
}

#[derive(Debug, Serialize)]
struct AcceptInvitationVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    token: String,
}

#[derive(Debug, Deserialize)]
struct DirectoryResponse {
    groups: DirectoryWire,
}

#[derive(Debug, Deserialize)]
struct AcceptInvitationResponse {
    accept_group_invitation: AcceptInvitationWire,
}

#[derive(Debug, Deserialize)]
struct DirectoryWire {
    items: Vec<GroupWire>,
    total: u64,
    page: u64,
    per_page: u64,
}

#[derive(Debug, Deserialize)]
struct GroupWire {
    id: String,
    handle: String,
    title: String,
    summary: Option<String>,
    visibility: String,
    join_policy: String,
    member_count: u64,
    effective_locale: String,
}

#[derive(Debug, Deserialize)]
struct AcceptInvitationWire {
    invitation_id: String,
    group_id: String,
    membership: MembershipWire,
    group_version: u64,
    replayed: bool,
}

#[derive(Debug, Deserialize)]
struct MembershipWire {
    id: String,
    group_id: String,
    user_id: String,
    role: String,
    status: String,
}

pub async fn load_directory(
    tenant_slug: Option<String>,
    filters: GroupsStorefrontFilters,
) -> Result<GroupsStorefrontDirectory, GraphqlGroupsStorefrontError> {
    let page = filters.page.max(1);
    let per_page = filters.per_page.clamp(1, 100);
    let response: DirectoryResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            DIRECTORY_QUERY,
            Some(DirectoryVariables {
                page: page.min(i32::MAX as u64) as i32,
                per_page: per_page.min(i32::MAX as u64) as i32,
                search: filters.search,
            }),
        ),
        None,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())?;

    Ok(GroupsStorefrontDirectory {
        items: response
            .groups
            .items
            .into_iter()
            .map(|group| GroupsStorefrontListItem {
                id: group.id,
                handle: group.handle,
                title: group.title,
                summary: group.summary,
                visibility: group.visibility.to_ascii_lowercase(),
                join_policy: group.join_policy.to_ascii_lowercase(),
                member_count: group.member_count,
                effective_locale: group.effective_locale,
            })
            .collect(),
        total: response.groups.total,
        page: response.groups.page,
        per_page: response.groups.per_page,
    })
}

pub async fn accept_invitation(
    access_token: Option<String>,
    tenant_slug: Option<String>,
    command: AcceptGroupInvitationCommand,
) -> Result<GroupsStorefrontAcceptInvitationResult, GraphqlGroupsStorefrontError> {
    let response: AcceptInvitationResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            ACCEPT_INVITATION_MUTATION,
            Some(AcceptInvitationVariables {
                idempotency_key: command.idempotency_key,
                token: command.token,
            }),
        ),
        access_token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())?;

    Ok(GroupsStorefrontAcceptInvitationResult {
        invitation_id: response.accept_group_invitation.invitation_id,
        group_id: response.accept_group_invitation.group_id,
        membership: response.accept_group_invitation.membership.into(),
        group_version: response.accept_group_invitation.group_version,
        replayed: response.accept_group_invitation.replayed,
    })
}

impl From<MembershipWire> for GroupsStorefrontMembership {
    fn from(value: MembershipWire) -> Self {
        Self {
            id: value.id,
            group_id: value.group_id,
            user_id: value.user_id,
            role: value.role.to_ascii_lowercase(),
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
