use leptos::prelude::*;
use std::fmt::{Display, Formatter};

use crate::application_model::{
    BulkReviewGroupMembershipApplicationsCommand, GroupsAdminApplicationAnswer,
    GroupsAdminApplicationQuestion, GroupsAdminApplicationRule,
    GroupsAdminBulkReviewApplicationError, GroupsAdminBulkReviewApplicationItemResult,
    GroupsAdminBulkReviewApplicationsResult, GroupsAdminMembership,
    GroupsAdminMembershipApplication, GroupsAdminReviewApplicationResult,
};

#[derive(Debug, Clone)]
pub struct NativeGroupsBulkReviewError(pub String);

impl Display for NativeGroupsBulkReviewError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0.as_str())
    }
}

impl std::error::Error for NativeGroupsBulkReviewError {}

impl From<ServerFnError> for NativeGroupsBulkReviewError {
    fn from(value: ServerFnError) -> Self {
        Self(value.to_string())
    }
}

pub async fn bulk_review_group_membership_applications(
    command: BulkReviewGroupMembershipApplicationsCommand,
) -> Result<GroupsAdminBulkReviewApplicationsResult, NativeGroupsBulkReviewError> {
    groups_admin_bulk_review_membership_applications_native(command)
        .await
        .map_err(Into::into)
}

#[server(prefix = "/api/fn", endpoint = "groups/admin/applications/bulk-review")]
async fn groups_admin_bulk_review_membership_applications_native(
    command: BulkReviewGroupMembershipApplicationsCommand,
) -> Result<GroupsAdminBulkReviewApplicationsResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            AuthContext, HostRuntimeContext, PortActor, PortContext, TenantContext,
            request::RequestContext,
        };
        use rustok_groups::{
            BulkReviewGroupMembershipApplicationsRequest, GroupApplicationBulkReviewCommandPort,
            GroupApplicationReviewDecision, GroupApplicationService,
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

        let application_ids = command
            .application_ids
            .into_iter()
            .map(|application_id| {
                Uuid::parse_str(&application_id)
                    .map_err(|_| ServerFnError::new("application_ids must contain UUID values"))
            })
            .collect::<Result<Vec<_>, _>>()?;
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
            format!(
                "groups-admin-applications-bulk-review-native-{}",
                Uuid::new_v4()
            ),
        )
        .with_deadline(Duration::from_secs(30))
        .with_idempotency_key(command.idempotency_key);
        for permission in auth.permissions {
            context = context.with_claim(permission.to_string());
        }

        let result =
            GroupApplicationBulkReviewCommandPort::bulk_review_group_membership_applications(
                &GroupApplicationService::new(runtime.db_clone()),
                context,
                BulkReviewGroupMembershipApplicationsRequest {
                    application_ids,
                    decision,
                    note: command.note,
                    confirmed: command.confirmed,
                },
            )
            .await
            .map_err(|error| ServerFnError::new(format!("{}: {}", error.code, error.message)))?;

        let succeeded = result.succeeded;
        let failed = result.failed;
        let items = result.items;
        Ok(GroupsAdminBulkReviewApplicationsResult {
            items: items
                .into_iter()
                .map(|item| GroupsAdminBulkReviewApplicationItemResult {
                    application_id: item.application_id.to_string(),
                    result: item.result.map(map_review_result),
                    error: item
                        .error
                        .map(|error| GroupsAdminBulkReviewApplicationError {
                            code: error.code,
                            message: error.message,
                            retryable: error.retryable,
                        }),
                })
                .collect(),
            succeeded,
            failed,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = command;
        Err(ServerFnError::new(
            "groups admin application bulk-review native transport requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn map_review_result(
    value: rustok_groups::ReviewGroupMembershipApplicationResult,
) -> GroupsAdminReviewApplicationResult {
    GroupsAdminReviewApplicationResult {
        application: map_application(value.application),
        membership: map_membership(value.membership),
        group_version: value.group_version,
        replayed: value.replayed,
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
