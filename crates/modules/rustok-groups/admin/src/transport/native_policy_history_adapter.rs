use leptos::prelude::*;
use std::fmt::{Display, Formatter};

use crate::application_model::{
    GroupsAdminApplicationPolicyRevision, GroupsAdminApplicationPolicyRevisionConnection,
    GroupsAdminApplicationPolicyRevisionQuery,
};

#[derive(Debug, Clone)]
pub struct NativeGroupsPolicyHistoryError(pub String);

impl Display for NativeGroupsPolicyHistoryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

impl std::error::Error for NativeGroupsPolicyHistoryError {}

impl From<ServerFnError> for NativeGroupsPolicyHistoryError {
    fn from(value: ServerFnError) -> Self {
        Self(value.to_string())
    }
}

pub async fn load_group_application_policy_revisions(
    query: GroupsAdminApplicationPolicyRevisionQuery,
) -> Result<GroupsAdminApplicationPolicyRevisionConnection, NativeGroupsPolicyHistoryError> {
    groups_admin_application_policy_revisions_native(query)
        .await
        .map_err(Into::into)
}

#[server(
    prefix = "/api/fn",
    endpoint = "groups/admin/applications/policy-revisions"
)]
async fn groups_admin_application_policy_revisions_native(
    query: GroupsAdminApplicationPolicyRevisionQuery,
) -> Result<GroupsAdminApplicationPolicyRevisionConnection, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            AuthContext, HostRuntimeContext, PortActor, PortContext, TenantContext,
            request::RequestContext,
        };
        use rustok_groups::{
            GroupApplicationPolicyHistoryReadPort, GroupApplicationPolicyHistoryService,
            ListGroupApplicationPolicyRevisionsRequest,
        };
        use std::time::Duration;
        use uuid::Uuid;

        let runtime = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request = leptos_axum::extract::<RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        if auth.tenant_id != tenant.id {
            return Err(ServerFnError::new("groups tenant mismatch"));
        }
        let group_id = Uuid::parse_str(&query.group_id)
            .map_err(|_| ServerFnError::new("group_id must be a UUID"))?;
        let mut context = PortContext::new(
            tenant.id.to_string(),
            PortActor::user(auth.user_id.to_string()),
            request.locale,
            format!("groups-admin-policy-history-native-{}", Uuid::new_v4()),
        )
        .with_deadline(Duration::from_secs(5));
        for permission in auth.permissions {
            context = context.with_claim(permission.to_string());
        }
        let result =
            GroupApplicationPolicyHistoryReadPort::list_group_application_policy_revisions(
                &GroupApplicationPolicyHistoryService::new(runtime.db_clone()),
                context,
                ListGroupApplicationPolicyRevisionsRequest {
                    group_id,
                    page: query.page,
                    per_page: query.per_page,
                },
            )
            .await
            .map_err(|error| ServerFnError::new(error.message))?;
        Ok(GroupsAdminApplicationPolicyRevisionConnection {
            items: result.items.into_iter().map(map_revision).collect(),
            total: result.total,
            page: result.page,
            per_page: result.per_page,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = query;
        Err(ServerFnError::new(
            "groups admin policy history native transport requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn map_revision(
    value: rustok_groups::GroupApplicationPolicyRevision,
) -> GroupsAdminApplicationPolicyRevision {
    GroupsAdminApplicationPolicyRevision {
        group_id: value.group_id.to_string(),
        policy_id: value.policy_id.to_string(),
        revision: value.revision,
        locale: value.locale,
        enabled: value.enabled,
        questions: value
            .questions
            .into_iter()
            .map(
                |question| crate::application_model::GroupsAdminApplicationQuestion {
                    key: question.key,
                    prompt: question.prompt,
                    help_text: question.help_text,
                    required: question.required,
                    max_answer_chars: question.max_answer_chars,
                },
            )
            .collect(),
        rules: value
            .rules
            .into_iter()
            .map(
                |rule| crate::application_model::GroupsAdminApplicationRule {
                    key: rule.key,
                    title: rule.title,
                    body: rule.body,
                    required: rule.required,
                },
            )
            .collect(),
        created_by_user_id: value.created_by_user_id.to_string(),
        created_at: value.created_at.to_rfc3339(),
    }
}
