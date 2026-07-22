use leptos::prelude::*;
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};

use crate::application_model::{
    GroupsStorefrontApplicationMembership, GroupsStorefrontApplicationPolicy,
    GroupsStorefrontApplicationPolicyQuery, GroupsStorefrontApplicationQuestion,
    GroupsStorefrontApplicationRule, GroupsStorefrontMembershipApplication,
    GroupsStorefrontSubmitApplicationResult, SubmitGroupMembershipApplicationCommand,
};

#[derive(Debug, Clone)]
pub struct NativeGroupsApplicationError(pub String);

impl Display for NativeGroupsApplicationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

impl std::error::Error for NativeGroupsApplicationError {}

impl From<ServerFnError> for NativeGroupsApplicationError {
    fn from(value: ServerFnError) -> Self {
        Self(value.to_string())
    }
}

pub async fn load_group_application_policy(
    query: GroupsStorefrontApplicationPolicyQuery,
) -> Result<GroupsStorefrontApplicationPolicy, NativeGroupsApplicationError> {
    groups_storefront_application_policy_native(query)
        .await
        .map_err(Into::into)
}

pub async fn submit_group_membership_application(
    command: SubmitGroupMembershipApplicationCommand,
) -> Result<GroupsStorefrontSubmitApplicationResult, NativeGroupsApplicationError> {
    groups_storefront_submit_application_native(command)
        .await
        .map_err(Into::into)
}

#[server(prefix = "/api/fn", endpoint = "groups/storefront/applications/policy")]
async fn groups_storefront_application_policy_native(
    query: GroupsStorefrontApplicationPolicyQuery,
) -> Result<GroupsStorefrontApplicationPolicy, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{AuthContext, HostRuntimeContext, PortActor, PortContext, TenantContext};
        use rustok_groups::{
            GroupApplicationReadPort, GroupApplicationService, ReadGroupApplicationPolicyRequest,
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
        if auth.tenant_id != tenant.id {
            return Err(ServerFnError::new("groups tenant mismatch"));
        }
        let group_id = Uuid::parse_str(&query.group_id)
            .map_err(|_| ServerFnError::new("group_id must be a UUID"))?;
        let mut context = PortContext::new(
            tenant.id.to_string(),
            PortActor::user(auth.user_id.to_string()),
            query.locale,
            format!("groups-storefront-applications-native-{}", Uuid::new_v4()),
        )
        .with_deadline(Duration::from_secs(5));
        for permission in auth.permissions {
            context = context.with_claim(permission.to_string());
        }
        let result = GroupApplicationReadPort::read_group_application_policy(
            &GroupApplicationService::new(runtime.db_clone()),
            context,
            ReadGroupApplicationPolicyRequest { group_id },
        )
        .await
        .map_err(|error| ServerFnError::new(format!("{}: {}", error.code, error.message)))?;
        Ok(map_policy(result))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = query;
        Err(ServerFnError::new(
            "groups storefront application native transport requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "groups/storefront/applications/submit-if-current"
)]
async fn groups_storefront_submit_application_native(
    command: SubmitGroupMembershipApplicationCommand,
) -> Result<GroupsStorefrontSubmitApplicationResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{AuthContext, HostRuntimeContext, PortActor, PortContext, TenantContext};
        use rustok_groups::{
            GroupApplicationCasCommandPort, GroupApplicationPolicyPrecondition,
            GroupApplicationService, SubmitGroupMembershipApplicationIfCurrentRequest,
            SubmitGroupMembershipApplicationRequest,
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
        if auth.tenant_id != tenant.id {
            return Err(ServerFnError::new("groups tenant mismatch"));
        }
        let group_id = Uuid::parse_str(&command.group_id)
            .map_err(|_| ServerFnError::new("group_id must be a UUID"))?;
        let expected_policy_id = Uuid::parse_str(&command.expected_policy.policy_id)
            .map_err(|_| ServerFnError::new("policy_id must be a UUID"))?;
        let answers = command
            .answers
            .into_iter()
            .map(|answer| (answer.key, answer.value))
            .collect::<BTreeMap<_, _>>();
        let mut context = PortContext::new(
            tenant.id.to_string(),
            PortActor::user(auth.user_id.to_string()),
            command.expected_policy.locale.clone(),
            format!("groups-storefront-application-cas-native-{}", Uuid::new_v4()),
        )
        .with_deadline(Duration::from_secs(5))
        .with_idempotency_key(command.idempotency_key);
        for permission in auth.permissions {
            context = context.with_claim(permission.to_string());
        }
        let result =
            GroupApplicationCasCommandPort::submit_group_membership_application_if_current(
                &GroupApplicationService::new(runtime.db_clone()),
                context,
                SubmitGroupMembershipApplicationIfCurrentRequest {
                    expected_policy: GroupApplicationPolicyPrecondition {
                        policy_id: expected_policy_id,
                        revision: command.expected_policy.revision,
                        locale: command.expected_policy.locale,
                    },
                    submission: SubmitGroupMembershipApplicationRequest {
                        group_id,
                        answers,
                        acknowledged_rule_keys: command.acknowledged_rule_keys,
                    },
                },
            )
            .await
            .map_err(|error| ServerFnError::new(format!("{}: {}", error.code, error.message)))?;
        Ok(GroupsStorefrontSubmitApplicationResult {
            application: GroupsStorefrontMembershipApplication {
                id: result.application.id.to_string(),
                group_id: result.application.group_id.to_string(),
                user_id: result.application.user_id.to_string(),
                policy_id: result.application.policy_id.to_string(),
                policy_revision: result.application.policy_revision,
                policy_locale: result.application.policy_locale,
                status: result.application.status.as_str().to_string(),
                submitted_at: result.application.submitted_at.to_rfc3339(),
            },
            membership: GroupsStorefrontApplicationMembership {
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
            "groups storefront application native transport requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn map_policy(value: rustok_groups::GroupApplicationPolicy) -> GroupsStorefrontApplicationPolicy {
    GroupsStorefrontApplicationPolicy {
        id: value.id.to_string(),
        group_id: value.group_id.to_string(),
        revision: value.revision,
        enabled: value.enabled,
        locale: value.locale,
        questions: value
            .questions
            .into_iter()
            .map(|question| GroupsStorefrontApplicationQuestion {
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
            .map(|rule| GroupsStorefrontApplicationRule {
                key: rule.key,
                title: rule.title,
                body: rule.body,
                required: rule.required,
            })
            .collect(),
    }
}
