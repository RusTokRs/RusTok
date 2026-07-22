use leptos::prelude::*;
use std::fmt::{Display, Formatter};

use crate::application_model::{
    GroupsAdminApplicationAnswer, GroupsAdminApplicationPolicy,
    GroupsAdminApplicationPolicyQuery, GroupsAdminApplicationQuestion,
    GroupsAdminApplicationRule, GroupsAdminMembership, GroupsAdminMembershipApplication,
    GroupsAdminMembershipApplicationConnection, GroupsAdminMembershipApplicationQuery,
    GroupsAdminReviewApplicationResult, GroupsAdminUpsertApplicationPolicyResult,
    ReviewGroupMembershipApplicationCommand, UpsertGroupApplicationPolicyCommand,
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
    query: GroupsAdminApplicationPolicyQuery,
) -> Result<GroupsAdminApplicationPolicy, NativeGroupsApplicationError> {
    groups_admin_application_policy_native(query)
        .await
        .map_err(Into::into)
}

pub async fn upsert_group_application_policy(
    command: UpsertGroupApplicationPolicyCommand,
) -> Result<GroupsAdminUpsertApplicationPolicyResult, NativeGroupsApplicationError> {
    groups_admin_upsert_application_policy_native(command)
        .await
        .map_err(Into::into)
}

pub async fn load_group_membership_applications(
    query: GroupsAdminMembershipApplicationQuery,
) -> Result<GroupsAdminMembershipApplicationConnection, NativeGroupsApplicationError> {
    groups_admin_membership_applications_native(query)
        .await
        .map_err(Into::into)
}

pub async fn review_group_membership_application(
    command: ReviewGroupMembershipApplicationCommand,
) -> Result<GroupsAdminReviewApplicationResult, NativeGroupsApplicationError> {
    groups_admin_review_membership_application_native(command)
        .await
        .map_err(Into::into)
}

#[server(
    prefix = "/api/fn",
    endpoint = "groups/admin/applications/policy/read"
)]
async fn groups_admin_application_policy_native(
    query: GroupsAdminApplicationPolicyQuery,
) -> Result<GroupsAdminApplicationPolicy, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            request::RequestContext, AuthContext, HostRuntimeContext, PortActor, PortContext,
            TenantContext,
        };
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
            format!("groups-admin-applications-native-{}", Uuid::new_v4()),
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
        .map_err(|error| ServerFnError::new(error.message))?;
        Ok(map_policy(result))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = query;
        Err(ServerFnError::new(
            "groups admin application native transport requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "groups/admin/applications/policy/upsert"
)]
async fn groups_admin_upsert_application_policy_native(
    command: UpsertGroupApplicationPolicyCommand,
) -> Result<GroupsAdminUpsertApplicationPolicyResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            request::RequestContext, AuthContext, HostRuntimeContext, PortActor, PortContext,
            TenantContext,
        };
        use rustok_groups::{
            GroupApplicationCommandPort, GroupApplicationQuestion, GroupApplicationRule,
            GroupApplicationService, UpsertGroupApplicationPolicyRequest,
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
        let group_id = Uuid::parse_str(&command.group_id)
            .map_err(|_| ServerFnError::new("group_id must be a UUID"))?;
        let mut context = PortContext::new(
            tenant.id.to_string(),
            PortActor::user(auth.user_id.to_string()),
            request.locale,
            format!("groups-admin-applications-native-{}", Uuid::new_v4()),
        )
        .with_deadline(Duration::from_secs(5))
        .with_idempotency_key(command.idempotency_key);
        for permission in auth.permissions {
            context = context.with_claim(permission.to_string());
        }
        let result = GroupApplicationCommandPort::upsert_group_application_policy(
            &GroupApplicationService::new(runtime.db_clone()),
            context,
            UpsertGroupApplicationPolicyRequest {
                group_id,
                locale: command.locale,
                enabled: command.enabled,
                questions: command
                    .questions
                    .into_iter()
                    .map(|question| GroupApplicationQuestion {
                        key: question.key,
                        prompt: question.prompt,
                        help_text: question.help_text,
                        required: question.required,
                        max_answer_chars: question.max_answer_chars,
                    })
                    .collect(),
                rules: command
                    .rules
                    .into_iter()
                    .map(|rule| GroupApplicationRule {
                        key: rule.key,
                        title: rule.title,
                        body: rule.body,
                        required: rule.required,
                    })
                    .collect(),
            },
        )
        .await
        .map_err(|error| ServerFnError::new(error.message))?;
        Ok(GroupsAdminUpsertApplicationPolicyResult {
            policy: map_policy(result.policy),
            group_version: result.group_version,
            created: result.created,
            replayed: result.replayed,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = command;
        Err(ServerFnError::new(
            "groups admin application native transport requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "groups/admin/applications/list"
)]
async fn groups_admin_membership_applications_native(
    query: GroupsAdminMembershipApplicationQuery,
) -> Result<GroupsAdminMembershipApplicationConnection, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            request::RequestContext, AuthContext, HostRuntimeContext, PortActor, PortContext,
            TenantContext,
        };
        use rustok_groups::{
            GroupApplicationReadPort, GroupApplicationService, GroupApplicationStatus,
            ListGroupMembershipApplicationsRequest,
        };
        use std::str::FromStr;
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
        let status = query
            .status
            .as_deref()
            .map(GroupApplicationStatus::from_str)
            .transpose()
            .map_err(ServerFnError::new)?;
        let mut context = PortContext::new(
            tenant.id.to_string(),
            PortActor::user(auth.user_id.to_string()),
            request.locale,
            format!("groups-admin-applications-native-{}", Uuid::new_v4()),
        )
        .with_deadline(Duration::from_secs(5));
        for permission in auth.permissions {
            context = context.with_claim(permission.to_string());
        }
        let result = GroupApplicationReadPort::list_group_membership_applications(
            &GroupApplicationService::new(runtime.db_clone()),
            context,
            ListGroupMembershipApplicationsRequest {
                group_id,
                status,
                page: query.page,
                per_page: query.per_page,
            },
        )
        .await
        .map_err(|error| ServerFnError::new(error.message))?;
        Ok(GroupsAdminMembershipApplicationConnection {
            items: result.items.into_iter().map(map_application).collect(),
            total: result.total,
            page: result.page,
            per_page: result.per_page,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = query;
        Err(ServerFnError::new(
            "groups admin application native transport requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "groups/admin/applications/review"
)]
async fn groups_admin_review_membership_application_native(
    command: ReviewGroupMembershipApplicationCommand,
) -> Result<GroupsAdminReviewApplicationResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            request::RequestContext, AuthContext, HostRuntimeContext, PortActor, PortContext,
            TenantContext,
        };
        use rustok_groups::{
            GroupApplicationCommandPort, GroupApplicationReviewDecision, GroupApplicationService,
            ReviewGroupMembershipApplicationRequest,
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
        let decision = match command.decision {
            crate::application_model::GroupsAdminApplicationReviewDecision::Approve => {
                GroupApplicationReviewDecision::Approve
            }
            crate::application_model::GroupsAdminApplicationReviewDecision::Reject => {
                GroupApplicationReviewDecision::Reject
            }
        };
        let mut context = PortContext::new(
            tenant.id.to_string(),
            PortActor::user(auth.user_id.to_string()),
            request.locale,
            format!("groups-admin-applications-native-{}", Uuid::new_v4()),
        )
        .with_deadline(Duration::from_secs(5))
        .with_idempotency_key(command.idempotency_key);
        for permission in auth.permissions {
            context = context.with_claim(permission.to_string());
        }
        let result = GroupApplicationCommandPort::review_group_membership_application(
            &GroupApplicationService::new(runtime.db_clone()),
            context,
            ReviewGroupMembershipApplicationRequest {
                application_id,
                decision,
                note: command.note,
            },
        )
        .await
        .map_err(|error| ServerFnError::new(error.message))?;
        Ok(GroupsAdminReviewApplicationResult {
            application: map_application(result.application),
            membership: map_membership(result.membership),
            group_version: result.group_version,
            replayed: result.replayed,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = command;
        Err(ServerFnError::new(
            "groups admin application native transport requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn map_policy(value: rustok_groups::GroupApplicationPolicy) -> GroupsAdminApplicationPolicy {
    GroupsAdminApplicationPolicy {
        id: value.id.to_string(),
        group_id: value.group_id.to_string(),
        revision: value.revision,
        enabled: value.enabled,
        locale: value.locale,
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
    }
}

#[cfg(feature = "ssr")]
fn map_application(
    value: rustok_groups::GroupMembershipApplication,
) -> GroupsAdminMembershipApplication {
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
        reviewed_at: value.reviewed_at.map(|date| date.to_rfc3339()),
        reviewed_by_user_id: value.reviewed_by_user_id.map(|id| id.to_string()),
        review_note: value.review_note,
    }
}

#[cfg(feature = "ssr")]
fn map_membership(value: rustok_groups::GroupMembership) -> GroupsAdminMembership {
    GroupsAdminMembership {
        id: value.id.to_string(),
        group_id: value.group_id.to_string(),
        user_id: value.user_id.to_string(),
        role: value.role.as_str().to_string(),
        status: value.status.as_str().to_string(),
    }
}
