#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{execute as execute_graphql, GraphqlRequest};
use serde::{Deserialize, Serialize};

use crate::application_model::{
    CancelGroupMembershipApplicationCommand, GroupsStorefrontApplicationLifecycleResult,
    GroupsStorefrontApplicationMembership, GroupsStorefrontMembershipApplication,
    GroupsStorefrontMyApplicationQuery,
};

pub type GraphqlGroupsApplicationLifecycleError = String;

const MY_APPLICATION_QUERY: &str = "query GroupsStorefrontMyApplication($groupId: UUID!) { my_group_membership_application: myGroupMembershipApplication(groupId: $groupId) { id group_id: groupId user_id: userId policy_id: policyId policy_revision: policyRevision policy_locale: policyLocale status submitted_at: submittedAt } }";
const CANCEL_APPLICATION_MUTATION: &str = "mutation GroupsStorefrontCancelApplication($idempotencyKey: String!, $applicationId: UUID!) { cancel_group_membership_application: cancelGroupMembershipApplication(idempotencyKey: $idempotencyKey, applicationId: $applicationId) { application { id group_id: groupId user_id: userId policy_id: policyId policy_revision: policyRevision policy_locale: policyLocale status submitted_at: submittedAt } membership { id group_id: groupId user_id: userId role status } group_version: groupVersion replayed } }";

#[derive(Debug, Serialize)]
struct MyApplicationVariables {
    #[serde(rename = "groupId")]
    group_id: String,
}

#[derive(Debug, Serialize)]
struct CancelVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    #[serde(rename = "applicationId")]
    application_id: String,
}

#[derive(Debug, Deserialize)]
struct MyApplicationResponse {
    my_group_membership_application: Option<ApplicationWire>,
}

#[derive(Debug, Deserialize)]
struct CancelResponse {
    cancel_group_membership_application: LifecycleWire,
}

#[derive(Debug, Deserialize)]
struct ApplicationWire {
    id: String,
    group_id: String,
    user_id: String,
    policy_id: String,
    policy_revision: u64,
    policy_locale: String,
    status: String,
    submitted_at: String,
}

#[derive(Debug, Deserialize)]
struct MembershipWire {
    id: String,
    group_id: String,
    user_id: String,
    role: String,
    status: String,
}

#[derive(Debug, Deserialize)]
struct LifecycleWire {
    application: ApplicationWire,
    membership: MembershipWire,
    group_version: u64,
    replayed: bool,
}

pub async fn load_my_group_membership_application(
    token: Option<String>,
    tenant_slug: Option<String>,
    query: GroupsStorefrontMyApplicationQuery,
) -> Result<Option<GroupsStorefrontMembershipApplication>, GraphqlGroupsApplicationLifecycleError> {
    let response: MyApplicationResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            MY_APPLICATION_QUERY,
            Some(MyApplicationVariables {
                group_id: query.group_id,
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok(response
        .my_group_membership_application
        .map(map_application))
}

pub async fn cancel_group_membership_application(
    token: Option<String>,
    tenant_slug: Option<String>,
    command: CancelGroupMembershipApplicationCommand,
) -> Result<GroupsStorefrontApplicationLifecycleResult, GraphqlGroupsApplicationLifecycleError> {
    let response: CancelResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            CANCEL_APPLICATION_MUTATION,
            Some(CancelVariables {
                idempotency_key: command.idempotency_key,
                application_id: command.application_id,
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())?;
    let value = response.cancel_group_membership_application;
    Ok(GroupsStorefrontApplicationLifecycleResult {
        application: map_application(value.application),
        membership: GroupsStorefrontApplicationMembership {
            id: value.membership.id,
            group_id: value.membership.group_id,
            user_id: value.membership.user_id,
            role: value.membership.role.to_ascii_lowercase(),
            status: value.membership.status.to_ascii_lowercase(),
        },
        group_version: value.group_version,
        replayed: value.replayed,
    })
}

fn map_application(value: ApplicationWire) -> GroupsStorefrontMembershipApplication {
    GroupsStorefrontMembershipApplication {
        id: value.id,
        group_id: value.group_id,
        user_id: value.user_id,
        policy_id: value.policy_id,
        policy_revision: value.policy_revision,
        policy_locale: value.policy_locale,
        status: value.status.to_ascii_lowercase(),
        submitted_at: value.submitted_at,
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
