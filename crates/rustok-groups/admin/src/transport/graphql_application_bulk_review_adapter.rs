#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{execute as execute_graphql, GraphqlRequest};
use serde::{Deserialize, Serialize};

use crate::application_model::{
    BulkReviewGroupMembershipApplicationsCommand, GroupsAdminApplicationAnswer,
    GroupsAdminApplicationQuestion, GroupsAdminApplicationRule,
    GroupsAdminBulkReviewApplicationError, GroupsAdminBulkReviewApplicationItemResult,
    GroupsAdminBulkReviewApplicationsResult, GroupsAdminMembership,
    GroupsAdminMembershipApplication, GroupsAdminReviewApplicationResult,
};

pub type GraphqlGroupsBulkReviewError = String;

const APPLICATION_FIELDS: &str = "id group_id: groupId user_id: userId policy_id: policyId policy_revision: policyRevision policy_locale: policyLocale questions { key prompt help_text: helpText required max_answer_chars: maxAnswerChars } rules { key title body required } answers { key value } acknowledged_rule_keys: acknowledgedRuleKeys status submitted_at: submittedAt reviewed_at: reviewedAt reviewed_by_user_id: reviewedByUserId review_note: reviewNote";

fn bulk_review_mutation() -> String {
    format!("mutation GroupsAdminBulkReviewMembershipApplications($idempotencyKey: String!, $input: BulkReviewGroupMembershipApplicationsInputGql!) {{ bulk_review_group_membership_applications: bulkReviewGroupMembershipApplications(idempotencyKey: $idempotencyKey, input: $input) {{ succeeded failed items {{ application_id: applicationId result {{ application {{ {APPLICATION_FIELDS} }} membership {{ id group_id: groupId user_id: userId role status }} group_version: groupVersion replayed }} error {{ code message retryable }} }} }} }}")
}

#[derive(Debug, Serialize)]
struct BulkReviewVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    input: BulkReviewInput,
}

#[derive(Debug, Serialize)]
struct BulkReviewInput {
    #[serde(rename = "applicationIds")]
    application_ids: Vec<String>,
    decision: String,
    note: Option<String>,
    confirmed: bool,
}

#[derive(Debug, Deserialize)]
struct BulkReviewResponse {
    bulk_review_group_membership_applications: BulkReviewWire,
}

#[derive(Debug, Deserialize)]
struct BulkReviewWire {
    items: Vec<BulkReviewItemWire>,
    succeeded: u32,
    failed: u32,
}

#[derive(Debug, Deserialize)]
struct BulkReviewItemWire {
    application_id: String,
    result: Option<ReviewWire>,
    error: Option<ErrorWire>,
}

#[derive(Debug, Deserialize)]
struct ErrorWire {
    code: String,
    message: String,
    retryable: bool,
}

#[derive(Debug, Deserialize)]
struct ReviewWire {
    application: ApplicationWire,
    membership: MembershipWire,
    group_version: u64,
    replayed: bool,
}

#[derive(Debug, Deserialize)]
struct QuestionWire {
    key: String,
    prompt: String,
    help_text: Option<String>,
    required: bool,
    max_answer_chars: u32,
}

#[derive(Debug, Deserialize)]
struct RuleWire {
    key: String,
    title: String,
    body: String,
    required: bool,
}

#[derive(Debug, Deserialize)]
struct ApplicationAnswerWire {
    key: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct ApplicationWire {
    id: String,
    group_id: String,
    user_id: String,
    policy_id: String,
    policy_revision: u64,
    policy_locale: String,
    questions: Vec<QuestionWire>,
    rules: Vec<RuleWire>,
    answers: Vec<ApplicationAnswerWire>,
    acknowledged_rule_keys: Vec<String>,
    status: String,
    submitted_at: String,
    reviewed_at: Option<String>,
    reviewed_by_user_id: Option<String>,
    review_note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MembershipWire {
    id: String,
    group_id: String,
    user_id: String,
    role: String,
    status: String,
}

pub async fn bulk_review_group_membership_applications(
    token: Option<String>,
    tenant_slug: Option<String>,
    command: BulkReviewGroupMembershipApplicationsCommand,
) -> Result<GroupsAdminBulkReviewApplicationsResult, GraphqlGroupsBulkReviewError> {
    let response: BulkReviewResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            bulk_review_mutation(),
            Some(BulkReviewVariables {
                idempotency_key: command.idempotency_key,
                input: BulkReviewInput {
                    application_ids: command.application_ids,
                    decision: command.decision.as_graphql_enum().to_string(),
                    note: command.note,
                    confirmed: command.confirmed,
                },
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())?;

    let BulkReviewWire {
        items,
        succeeded,
        failed,
    } = response.bulk_review_group_membership_applications;
    Ok(GroupsAdminBulkReviewApplicationsResult {
        items: items
            .into_iter()
            .map(|item| GroupsAdminBulkReviewApplicationItemResult {
                application_id: item.application_id,
                result: item.result.map(Into::into),
                error: item.error.map(|error| GroupsAdminBulkReviewApplicationError {
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

impl From<ReviewWire> for GroupsAdminReviewApplicationResult {
    fn from(value: ReviewWire) -> Self {
        Self {
            application: value.application.into(),
            membership: value.membership.into(),
            group_version: value.group_version,
            replayed: value.replayed,
        }
    }
}

impl From<ApplicationWire> for GroupsAdminMembershipApplication {
    fn from(value: ApplicationWire) -> Self {
        Self {
            id: value.id,
            group_id: value.group_id,
            user_id: value.user_id,
            policy_id: value.policy_id,
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
                .map(|answer| GroupsAdminApplicationAnswer {
                    key: answer.key,
                    value: answer.value,
                })
                .collect(),
            acknowledged_rule_keys: value.acknowledged_rule_keys,
            status: value.status,
            submitted_at: value.submitted_at,
            reviewed_at: value.reviewed_at,
            reviewed_by_user_id: value.reviewed_by_user_id,
            review_note: value.review_note,
        }
    }
}

impl From<MembershipWire> for GroupsAdminMembership {
    fn from(value: MembershipWire) -> Self {
        Self {
            id: value.id,
            group_id: value.group_id,
            user_id: value.user_id,
            role: value.role,
            status: value.status,
        }
    }
}

fn graphql_url() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            if let Ok(origin) = window.location().origin() {
                return format!("{origin}/graphql");
            }
        }
    }
    "/graphql".to_string()
}
