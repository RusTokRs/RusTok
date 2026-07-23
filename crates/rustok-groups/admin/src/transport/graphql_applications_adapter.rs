#![allow(dead_code)]

#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

use crate::application_model::{
    GroupsAdminApplicationAnswer, GroupsAdminApplicationPolicy, GroupsAdminApplicationPolicyQuery,
    GroupsAdminApplicationQuestion, GroupsAdminApplicationRule, GroupsAdminMembership,
    GroupsAdminMembershipApplication, GroupsAdminMembershipApplicationConnection,
    GroupsAdminMembershipApplicationQuery, GroupsAdminReviewApplicationResult,
    GroupsAdminUpsertApplicationPolicyResult, ReviewGroupMembershipApplicationCommand,
    UpsertGroupApplicationPolicyCommand,
};

pub type GraphqlGroupsApplicationError = String;

const POLICY_FIELDS: &str = "id group_id: groupId revision enabled locale questions { key prompt help_text: helpText required max_answer_chars: maxAnswerChars } rules { key title body required }";
const APPLICATION_FIELDS: &str = "id group_id: groupId user_id: userId policy_id: policyId policy_revision: policyRevision policy_locale: policyLocale questions { key prompt help_text: helpText required max_answer_chars: maxAnswerChars } rules { key title body required } answers { key value } acknowledged_rule_keys: acknowledgedRuleKeys status submitted_at: submittedAt reviewed_at: reviewedAt reviewed_by_user_id: reviewedByUserId review_note: reviewNote";

fn policy_query() -> String {
    format!(
        "query GroupsAdminApplicationPolicy($groupId: UUID!) {{ group_application_policy: groupApplicationPolicy(groupId: $groupId) {{ {POLICY_FIELDS} }} }}"
    )
}

fn upsert_policy_mutation() -> String {
    format!(
        "mutation GroupsAdminUpsertApplicationPolicy($idempotencyKey: String!, $groupId: UUID!, $input: UpsertGroupApplicationPolicyInputGql!) {{ upsert_group_application_policy: upsertGroupApplicationPolicy(idempotencyKey: $idempotencyKey, groupId: $groupId, input: $input) {{ policy {{ {POLICY_FIELDS} }} group_version: groupVersion created replayed }} }}"
    )
}

fn list_applications_query() -> String {
    format!(
        "query GroupsAdminMembershipApplications($groupId: UUID!, $status: GroupApplicationStatusGql, $page: Int, $perPage: Int) {{ group_membership_applications: groupMembershipApplications(groupId: $groupId, status: $status, page: $page, perPage: $perPage) {{ total page per_page: perPage items {{ {APPLICATION_FIELDS} }} }} }}"
    )
}

fn review_application_mutation() -> String {
    format!(
        "mutation GroupsAdminReviewMembershipApplication($idempotencyKey: String!, $applicationId: UUID!, $decision: GroupApplicationReviewDecisionGql!, $note: String) {{ review_group_membership_application: reviewGroupMembershipApplication(idempotencyKey: $idempotencyKey, applicationId: $applicationId, decision: $decision, note: $note) {{ application {{ {APPLICATION_FIELDS} }} membership {{ id group_id: groupId user_id: userId role status }} group_version: groupVersion replayed }} }}"
    )
}

#[derive(Debug, Serialize)]
struct PolicyVariables {
    #[serde(rename = "groupId")]
    group_id: String,
}

#[derive(Debug, Serialize)]
struct UpsertPolicyVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    #[serde(rename = "groupId")]
    group_id: String,
    input: UpsertPolicyInput,
}

#[derive(Debug, Serialize)]
struct UpsertPolicyInput {
    locale: String,
    enabled: bool,
    questions: Vec<QuestionInput>,
    rules: Vec<RuleInput>,
}

#[derive(Debug, Serialize)]
struct QuestionInput {
    key: String,
    prompt: String,
    #[serde(rename = "helpText")]
    help_text: Option<String>,
    required: bool,
    #[serde(rename = "maxAnswerChars")]
    max_answer_chars: i32,
}

#[derive(Debug, Serialize)]
struct RuleInput {
    key: String,
    title: String,
    body: String,
    required: bool,
}

#[derive(Debug, Serialize)]
struct ListApplicationsVariables {
    #[serde(rename = "groupId")]
    group_id: String,
    status: Option<String>,
    page: i32,
    #[serde(rename = "perPage")]
    per_page: i32,
}

#[derive(Debug, Serialize)]
struct ReviewApplicationVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    #[serde(rename = "applicationId")]
    application_id: String,
    decision: String,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PolicyResponse {
    group_application_policy: PolicyWire,
}

#[derive(Debug, Deserialize)]
struct UpsertPolicyResponse {
    upsert_group_application_policy: UpsertPolicyWire,
}

#[derive(Debug, Deserialize)]
struct ListApplicationsResponse {
    group_membership_applications: ApplicationConnectionWire,
}

#[derive(Debug, Deserialize)]
struct ReviewApplicationResponse {
    review_group_membership_application: ReviewApplicationWire,
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
struct PolicyWire {
    id: String,
    group_id: String,
    revision: u64,
    enabled: bool,
    locale: String,
    questions: Vec<QuestionWire>,
    rules: Vec<RuleWire>,
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

#[derive(Debug, Deserialize)]
struct UpsertPolicyWire {
    policy: PolicyWire,
    group_version: u64,
    created: bool,
    replayed: bool,
}

#[derive(Debug, Deserialize)]
struct ApplicationConnectionWire {
    items: Vec<ApplicationWire>,
    total: u64,
    page: u64,
    per_page: u64,
}

#[derive(Debug, Deserialize)]
struct ReviewApplicationWire {
    application: ApplicationWire,
    membership: MembershipWire,
    group_version: u64,
    replayed: bool,
}

pub async fn load_group_application_policy(
    token: Option<String>,
    tenant_slug: Option<String>,
    query: GroupsAdminApplicationPolicyQuery,
) -> Result<GroupsAdminApplicationPolicy, GraphqlGroupsApplicationError> {
    let response: PolicyResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            &policy_query(),
            Some(PolicyVariables {
                group_id: query.group_id,
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok(response.group_application_policy.into())
}

pub async fn upsert_group_application_policy(
    token: Option<String>,
    tenant_slug: Option<String>,
    command: UpsertGroupApplicationPolicyCommand,
) -> Result<GroupsAdminUpsertApplicationPolicyResult, GraphqlGroupsApplicationError> {
    let response: UpsertPolicyResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            &upsert_policy_mutation(),
            Some(UpsertPolicyVariables {
                idempotency_key: command.idempotency_key,
                group_id: command.group_id,
                input: UpsertPolicyInput {
                    locale: command.locale,
                    enabled: command.enabled,
                    questions: command
                        .questions
                        .into_iter()
                        .map(|question| QuestionInput {
                            key: question.key,
                            prompt: question.prompt,
                            help_text: question.help_text,
                            required: question.required,
                            max_answer_chars: question.max_answer_chars.min(i32::MAX as u32) as i32,
                        })
                        .collect(),
                    rules: command
                        .rules
                        .into_iter()
                        .map(|rule| RuleInput {
                            key: rule.key,
                            title: rule.title,
                            body: rule.body,
                            required: rule.required,
                        })
                        .collect(),
                },
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok(GroupsAdminUpsertApplicationPolicyResult {
        policy: response.upsert_group_application_policy.policy.into(),
        group_version: response.upsert_group_application_policy.group_version,
        created: response.upsert_group_application_policy.created,
        replayed: response.upsert_group_application_policy.replayed,
    })
}

pub async fn load_group_membership_applications(
    token: Option<String>,
    tenant_slug: Option<String>,
    query: GroupsAdminMembershipApplicationQuery,
) -> Result<GroupsAdminMembershipApplicationConnection, GraphqlGroupsApplicationError> {
    let response: ListApplicationsResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            &list_applications_query(),
            Some(ListApplicationsVariables {
                group_id: query.group_id,
                status: query.status.map(|status| status.to_ascii_uppercase()),
                page: query.page.max(1).min(i32::MAX as u64) as i32,
                per_page: query.per_page.clamp(1, 100).min(i32::MAX as u64) as i32,
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok(GroupsAdminMembershipApplicationConnection {
        items: response
            .group_membership_applications
            .items
            .into_iter()
            .map(Into::into)
            .collect(),
        total: response.group_membership_applications.total,
        page: response.group_membership_applications.page,
        per_page: response.group_membership_applications.per_page,
    })
}

pub async fn review_group_membership_application(
    token: Option<String>,
    tenant_slug: Option<String>,
    command: ReviewGroupMembershipApplicationCommand,
) -> Result<GroupsAdminReviewApplicationResult, GraphqlGroupsApplicationError> {
    let response: ReviewApplicationResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            &review_application_mutation(),
            Some(ReviewApplicationVariables {
                idempotency_key: command.idempotency_key,
                application_id: command.application_id,
                decision: command.decision.as_graphql_enum().to_string(),
                note: command.note,
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok(GroupsAdminReviewApplicationResult {
        application: response
            .review_group_membership_application
            .application
            .into(),
        membership: response
            .review_group_membership_application
            .membership
            .into(),
        group_version: response.review_group_membership_application.group_version,
        replayed: response.review_group_membership_application.replayed,
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

impl From<PolicyWire> for GroupsAdminApplicationPolicy {
    fn from(value: PolicyWire) -> Self {
        Self {
            id: value.id,
            group_id: value.group_id,
            revision: value.revision,
            enabled: value.enabled,
            locale: value.locale,
            questions: value.questions.into_iter().map(Into::into).collect(),
            rules: value.rules.into_iter().map(Into::into).collect(),
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
