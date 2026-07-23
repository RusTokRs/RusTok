#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

use crate::application_model::{
    GroupsAdminApplicationAnswer, GroupsAdminApplicationQuestion, GroupsAdminApplicationRule,
    GroupsAdminMembership, GroupsAdminMembershipApplication, GroupsAdminReviewApplicationResult,
    ReopenGroupMembershipApplicationCommand,
};

pub type GraphqlGroupsApplicationLifecycleError = String;

const APPLICATION_FIELDS: &str = "id group_id: groupId user_id: userId policy_id: policyId policy_revision: policyRevision policy_locale: policyLocale questions { key prompt help_text: helpText required max_answer_chars: maxAnswerChars } rules { key title body required } answers { key value } acknowledged_rule_keys: acknowledgedRuleKeys status submitted_at: submittedAt reviewed_at: reviewedAt reviewed_by_user_id: reviewedByUserId review_note: reviewNote";

fn reopen_application_mutation() -> String {
    format!(
        "mutation GroupsAdminReopenMembershipApplication($idempotencyKey: String!, $applicationId: UUID!) {{ reopen_group_membership_application: reopenGroupMembershipApplication(idempotencyKey: $idempotencyKey, applicationId: $applicationId) {{ application {{ {APPLICATION_FIELDS} }} membership {{ id group_id: groupId user_id: userId role status }} group_version: groupVersion replayed }} }}"
    )
}

#[derive(Debug, Serialize)]
struct ReopenVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    #[serde(rename = "applicationId")]
    application_id: String,
}

#[derive(Debug, Deserialize)]
struct ReopenResponse {
    reopen_group_membership_application: LifecycleWire,
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
struct AnswerWire {
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
    answers: Vec<AnswerWire>,
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

#[derive(Debug, Deserialize)]
struct LifecycleWire {
    application: ApplicationWire,
    membership: MembershipWire,
    group_version: u64,
    replayed: bool,
}

pub async fn reopen_group_membership_application(
    token: Option<String>,
    tenant_slug: Option<String>,
    command: ReopenGroupMembershipApplicationCommand,
) -> Result<GroupsAdminReviewApplicationResult, GraphqlGroupsApplicationLifecycleError> {
    let mutation = reopen_application_mutation();
    let response: ReopenResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            mutation,
            Some(ReopenVariables {
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
    let value = response.reopen_group_membership_application;
    Ok(GroupsAdminReviewApplicationResult {
        application: value.application.into(),
        membership: value.membership.into(),
        group_version: value.group_version,
        replayed: value.replayed,
    })
}

impl From<QuestionWire> for GroupsAdminApplicationQuestion {
    fn from(value: QuestionWire) -> Self {
        Self {
            key: value.key,
            prompt: value.prompt,
            help_text: value.help_text,
            required: value.required,
            max_answer_chars: value.max_answer_chars,
        }
    }
}

impl From<RuleWire> for GroupsAdminApplicationRule {
    fn from(value: RuleWire) -> Self {
        Self {
            key: value.key,
            title: value.title,
            body: value.body,
            required: value.required,
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
            questions: value.questions.into_iter().map(Into::into).collect(),
            rules: value.rules.into_iter().map(Into::into).collect(),
            answers: value
                .answers
                .into_iter()
                .map(|answer| GroupsAdminApplicationAnswer {
                    key: answer.key,
                    value: answer.value,
                })
                .collect(),
            acknowledged_rule_keys: value.acknowledged_rule_keys,
            status: value.status.to_ascii_lowercase(),
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
