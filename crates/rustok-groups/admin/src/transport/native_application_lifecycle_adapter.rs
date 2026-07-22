use leptos::prelude::*;
use std::fmt::{Display, Formatter};

use crate::application_model::{
    GroupsAdminApplicationAnswer, GroupsAdminApplicationQuestion, GroupsAdminApplicationRule,
    GroupsAdminMembership, GroupsAdminMembershipApplication, GroupsAdminReviewApplicationResult,
    ReopenGroupMembershipApplicationCommand,
};

#[derive(Debug, Clone)]
pub struct NativeGroupsApplicationLifecycleError(pub String);

impl Display for NativeGroupsApplicationLifecycleError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

impl std::error::Error for NativeGroupsApplicationLifecycleError {}

impl From<ServerFnError> for NativeGroupsApplicationLifecycleError {
    fn from(value: ServerFnError) -> Self {
        Self(value.to_string())
    }
}

pub async fn reopen_group_membership_application(
    command: ReopenGroupMembershipApplicationCommand,
) -> Result<GroupsAdminReviewApplicationResult, NativeGroupsApplicationLifecycleError> {
    groups_admin_reopen_membership_application_native(command)
        .await
        .map_err(Into::into)
}

#[server(
    prefix = "/api/fn",
    endpoint = "groups/admin/applications/reopen"
)]
async fn groups_admin_reopen_membership_application_native(
    command: ReopenGroupMembershipApplicationCommand,
) -> Result<GroupsAdminReviewApplicationResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            request::RequestContext, AuthContext, HostRuntimeContext, PortActor, PortContext,
            TenantContext,
        };
        use rustok_groups::{
            GroupApplicationLifecycleCommandPort, GroupApplicationService,
            ReopenGroupMembershipApplicationRequest,
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
        let application_id = Uuid::parse_str(&command.application_id)
            .map_err(|_| ServerFnError::new("application_id must be a UUID"))?;
        let mut context = PortContext::new(
            tenant.id.to_string(),
            PortActor::user(auth.user_id.to_string()),
            request.locale,
            format!("groups-admin-application-lifecycle-native-{}", Uuid::new_v4()),
        )
        .with_deadline(Duration::from_secs(5))
        .with_idempotency_key(command.idempotency_key);
        for permission in auth.permissions {
            context = context.with_claim(permission.to_string());
        }
        let result = GroupApplicationLifecycleCommandPort::reopen_group_membership_application(
            &GroupApplicationService::new(runtime.db_clone()),
            context,
            ReopenGroupMembershipApplicationRequest { application_id },
        )
        .await
        .map_err(|error| ServerFnError::new(format!("{}: {}", error.code, error.message)))?;
        Ok(GroupsAdminReviewApplicationResult {
            application: map_application(result.application),
            membership: GroupsAdminMembership {
                id: result.membership.id.to_string(),
                group_id: result.membership.group_id.to_string(),
                user_id: result.membership.user_id.to_string(),
                role: result.membership.role.as_str().to_string(),
                status: result.membership.status.as_str().to_string(),
            },
            group_version: result.group_version,
            replayed: result.replayed,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = command;
        Err(ServerFnError::new(
            "groups admin application lifecycle native transport requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn map_application(value: rustok_groups::GroupMembershipApplication) -> GroupsAdminMembershipApplication {
    GroupsAdminMembershipApplication {
        id: value.id.to_string(),
        group_id: value.group_id.to_string(),
        user_id: value.user_id.to_string(),
        policy_id: value.policy_id.to_string(),
        policy_revision: value.policy_revision,
        policy_locale: value.policy_locale,
        questions: value
            .questions
            .into_iter()
            .map(|question| GroupsAdminApplicationQuestion {
                key: question.key,
                prompt: question.prompt,
                help_text: question.help_text,
                required: question.required,
                max_answer_chars: question.max_answer_chars,
            })
            .collect(),
        rules: value
            .rules
            .into_iter()
            .map(|rule| GroupsAdminApplicationRule {
                key: rule.key,
                title: rule.title,
                body: rule.body,
                required: rule.required,
            })
            .collect(),
        answers: value
            .answers
            .into_iter()
            .map(|(key, value)| GroupsAdminApplicationAnswer { key, value })
            .collect(),
        acknowledged_rule_keys: value.acknowledged_rule_keys,
        status: value.status.as_str().to_string(),
        submitted_at: value.submitted_at.to_rfc3339(),
        reviewed_at: value.reviewed_at.map(|value| value.to_rfc3339()),
        reviewed_by_user_id: value.reviewed_by_user_id.map(|value| value.to_string()),
        review_note: value.review_note,
    }
}
