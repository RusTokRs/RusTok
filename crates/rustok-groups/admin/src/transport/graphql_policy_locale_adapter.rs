#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

use crate::application_model::{
    GroupsAdminApplicationPolicy, GroupsAdminApplicationPolicyQuery,
    GroupsAdminApplicationQuestion, GroupsAdminApplicationRule,
    GroupsAdminUpsertApplicationPolicyResult, UpsertGroupApplicationPolicyCommand,
};

pub type GraphqlGroupsPolicyLocaleError = String;

const POLICY_FIELDS: &str = "id group_id: groupId revision enabled locale questions { key prompt help_text: helpText required max_answer_chars: maxAnswerChars } rules { key title body required }";

fn policy_query() -> String {
    format!(
        "query GroupsAdminApplicationPolicyLocale($groupId: UUID!) {{ group_application_policy: groupApplicationPolicy(groupId: $groupId) {{ {POLICY_FIELDS} }} }}"
    )
}

fn upsert_policy_mutation() -> String {
    format!(
        "mutation GroupsAdminUpsertApplicationPolicyLocale($idempotencyKey: String!, $groupId: UUID!, $input: UpsertGroupApplicationPolicyInputGql!) {{ upsert_group_application_policy: upsertGroupApplicationPolicy(idempotencyKey: $idempotencyKey, groupId: $groupId, input: $input) {{ policy {{ {POLICY_FIELDS} }} group_version: groupVersion created replayed }} }}"
    )
}

#[derive(Debug, Serialize)]
struct PolicyVariables {
    #[serde(rename = "groupId")]
    group_id: String,
}

#[derive(Debug, Serialize)]
struct UpsertVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    #[serde(rename = "groupId")]
    group_id: String,
    input: UpsertInput,
}

#[derive(Debug, Serialize)]
struct UpsertInput {
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

#[derive(Debug, Deserialize)]
struct PolicyResponse {
    group_application_policy: PolicyWire,
}

#[derive(Debug, Deserialize)]
struct UpsertResponse {
    upsert_group_application_policy: UpsertWire,
}

#[derive(Debug, Deserialize)]
struct UpsertWire {
    policy: PolicyWire,
    group_version: u64,
    created: bool,
    replayed: bool,
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

pub async fn load_group_application_policy(
    token: Option<String>,
    tenant_slug: Option<String>,
    query: GroupsAdminApplicationPolicyQuery,
) -> Result<GroupsAdminApplicationPolicy, GraphqlGroupsPolicyLocaleError> {
    let locale = Some(query.locale.clone());
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
        locale,
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok(response.group_application_policy.into())
}

pub async fn upsert_group_application_policy(
    token: Option<String>,
    tenant_slug: Option<String>,
    command: UpsertGroupApplicationPolicyCommand,
) -> Result<GroupsAdminUpsertApplicationPolicyResult, GraphqlGroupsPolicyLocaleError> {
    let locale = Some(command.locale.clone());
    let response: UpsertResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            &upsert_policy_mutation(),
            Some(UpsertVariables {
                idempotency_key: command.idempotency_key,
                group_id: command.group_id,
                input: UpsertInput {
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
        locale,
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
