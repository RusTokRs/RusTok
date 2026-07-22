#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{execute as execute_graphql, GraphqlRequest};
use serde::{Deserialize, Serialize};

use crate::application_model::{
    GroupsStorefrontApplicationMembership, GroupsStorefrontApplicationPolicy,
    GroupsStorefrontApplicationPolicyPrecondition, GroupsStorefrontApplicationPolicyQuery,
    GroupsStorefrontApplicationQuestion, GroupsStorefrontApplicationRule,
    GroupsStorefrontMembershipApplication, GroupsStorefrontSubmitApplicationResult,
    SubmitGroupMembershipApplicationCommand,
};

pub type GraphqlGroupsApplicationError = String;

const POLICY_QUERY: &str = "query GroupsStorefrontApplicationPolicy($groupId: UUID!) { group_application_policy: groupApplicationPolicy(groupId: $groupId) { id group_id: groupId revision enabled locale questions { key prompt help_text: helpText required max_answer_chars: maxAnswerChars } rules { key title body required } } }";
const SUBMIT_APPLICATION_MUTATION: &str = "mutation GroupsStorefrontSubmitApplicationIfCurrent($idempotencyKey: String!, $groupId: UUID!, $expectedPolicy: GroupApplicationPolicyPreconditionInputGql!, $input: SubmitGroupMembershipApplicationInputGql!) { submit_group_membership_application: submitGroupMembershipApplicationIfCurrent(idempotencyKey: $idempotencyKey, groupId: $groupId, expectedPolicy: $expectedPolicy, input: $input) { application { id group_id: groupId user_id: userId policy_id: policyId policy_revision: policyRevision policy_locale: policyLocale status submitted_at: submittedAt } membership { id group_id: groupId user_id: userId role status } group_version: groupVersion replayed } }";

#[derive(Debug, Serialize)]
struct PolicyVariables {
    #[serde(rename = "groupId")]
    group_id: String,
}

#[derive(Debug, Serialize)]
struct SubmitVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    #[serde(rename = "groupId")]
    group_id: String,
    #[serde(rename = "expectedPolicy")]
    expected_policy: PolicyPreconditionInput,
    input: SubmitInput,
}

#[derive(Debug, Serialize)]
struct PolicyPreconditionInput {
    #[serde(rename = "policyId")]
    policy_id: String,
    revision: u64,
    locale: String,
}

impl From<GroupsStorefrontApplicationPolicyPrecondition> for PolicyPreconditionInput {
    fn from(value: GroupsStorefrontApplicationPolicyPrecondition) -> Self {
        Self {
            policy_id: value.policy_id,
            revision: value.revision,
            locale: value.locale,
        }
    }
}

#[derive(Debug, Serialize)]
struct SubmitInput {
    answers: Vec<AnswerInput>,
    #[serde(rename = "acknowledgedRuleKeys")]
    acknowledged_rule_keys: Vec<String>,
}

#[derive(Debug, Serialize)]
struct AnswerInput {
    key: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct PolicyResponse {
    group_application_policy: PolicyWire,
}

#[derive(Debug, Deserialize)]
struct SubmitResponse {
    submit_group_membership_application: SubmitWire,
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
struct SubmitWire {
    application: ApplicationWire,
    membership: MembershipWire,
    group_version: u64,
    replayed: bool,
}

pub async fn load_group_application_policy(
    token: Option<String>,
    tenant_slug: Option<String>,
    query: GroupsStorefrontApplicationPolicyQuery,
) -> Result<GroupsStorefrontApplicationPolicy, GraphqlGroupsApplicationError> {
    let locale = Some(query.locale.clone());
    let response: PolicyResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            POLICY_QUERY,
            Some(PolicyVariables {
                group_id: query.group_id,
            }),
        ),
        token,
        tenant_slug,
        locale,
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok(response.group_application_policy.into())
}

pub async fn submit_group_membership_application(
    token: Option<String>,
    tenant_slug: Option<String>,
    command: SubmitGroupMembershipApplicationCommand,
) -> Result<GroupsStorefrontSubmitApplicationResult, GraphqlGroupsApplicationError> {
    let locale = Some(command.expected_policy.locale.clone());
    let response: SubmitResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            SUBMIT_APPLICATION_MUTATION,
            Some(SubmitVariables {
                idempotency_key: command.idempotency_key,
                group_id: command.group_id,
                expected_policy: command.expected_policy.into(),
                input: SubmitInput {
                    answers: command
                        .answers
                        .into_iter()
                        .map(|answer| AnswerInput {
                            key: answer.key,
                            value: answer.value,
                        })
                        .collect(),
                    acknowledged_rule_keys: command.acknowledged_rule_keys,
                },
            }),
        ),
        token,
        tenant_slug,
        locale,
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok(GroupsStorefrontSubmitApplicationResult {
        application: GroupsStorefrontMembershipApplication {
            id: response.submit_group_membership_application.application.id,
            group_id: response.submit_group_membership_application.application.group_id,
            user_id: response.submit_group_membership_application.application.user_id,
            policy_id: response.submit_group_membership_application.application.policy_id,
            policy_revision: response
                .submit_group_membership_application
                .application
                .policy_revision,
            policy_locale: response
                .submit_group_membership_application
                .application
                .policy_locale,
            status: response
                .submit_group_membership_application
                .application
                .status
                .to_ascii_lowercase(),
            submitted_at: response
                .submit_group_membership_application
                .application
                .submitted_at,
        },
        membership: GroupsStorefrontApplicationMembership {
            id: response.submit_group_membership_application.membership.id,
            group_id: response.submit_group_membership_application.membership.group_id,
            user_id: response.submit_group_membership_application.membership.user_id,
            role: response
                .submit_group_membership_application
                .membership
                .role
                .to_ascii_lowercase(),
            status: response
                .submit_group_membership_application
                .membership
                .status
                .to_ascii_lowercase(),
        },
        group_version: response.submit_group_membership_application.group_version,
        replayed: response.submit_group_membership_application.replayed,
    })
}

impl From<QuestionWire> for GroupsStorefrontApplicationQuestion {
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

impl From<RuleWire> for GroupsStorefrontApplicationRule {
    fn from(value: RuleWire) -> Self {
        Self {
            key: value.key,
            title: value.title,
            body: value.body,
            required: value.required,
        }
    }
}

impl From<PolicyWire> for GroupsStorefrontApplicationPolicy {
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
