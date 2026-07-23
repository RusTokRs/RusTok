#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

use crate::application_model::{
    GroupsAdminApplicationPolicy, GroupsAdminApplicationPolicyLocaleCatalog,
    GroupsAdminApplicationPolicyLocaleCatalogQuery, GroupsAdminApplicationPolicyManagementView,
    GroupsAdminApplicationPolicyPrecondition, GroupsAdminApplicationPolicyQuery,
    GroupsAdminApplicationQuestion, GroupsAdminApplicationRule,
    GroupsAdminUpsertApplicationPolicyResult, UpsertGroupApplicationPolicyCommand,
};

pub type GraphqlGroupsPolicyLocaleError = String;

const POLICY_FIELDS: &str = "id group_id: groupId revision enabled locale questions { key prompt help_text: helpText required max_answer_chars: maxAnswerChars } rules { key title body required }";
const MANAGEMENT_FIELDS: &str = "group_id: groupId policy_id: policyId revision enabled locale translation_exists: translationExists questions { key prompt help_text: helpText required max_answer_chars: maxAnswerChars } rules { key title body required }";

fn locale_catalog_query() -> String {
    "query GroupsAdminApplicationPolicyLocaleCatalog($groupId: UUID!) { group_application_policy_locale_catalog: groupApplicationPolicyLocaleCatalog(groupId: $groupId) { group_id: groupId policy_id: policyId revision enabled locales } }".to_string()
}

fn management_policy_query() -> String {
    format!(
        "query GroupsAdminApplicationPolicyForManagement($groupId: UUID!, $locale: String!) {{ group_application_policy_for_management: groupApplicationPolicyForManagement(groupId: $groupId, locale: $locale) {{ {MANAGEMENT_FIELDS} }} }}"
    )
}

fn upsert_policy_mutation() -> String {
    format!(
        "mutation GroupsAdminUpsertApplicationPolicyIfCurrent($idempotencyKey: String!, $groupId: UUID!, $expectedPolicy: GroupApplicationPolicyPreconditionInputGql, $input: UpsertGroupApplicationPolicyInputGql!) {{ upsert_group_application_policy: upsertGroupApplicationPolicyIfCurrent(idempotencyKey: $idempotencyKey, groupId: $groupId, expectedPolicy: $expectedPolicy, input: $input) {{ policy {{ {POLICY_FIELDS} }} group_version: groupVersion created replayed }} }}"
    )
}

#[derive(Debug, Serialize)]
struct CatalogVariables {
    #[serde(rename = "groupId")]
    group_id: String,
}

#[derive(Debug, Serialize)]
struct ManagementVariables {
    #[serde(rename = "groupId")]
    group_id: String,
    locale: String,
}

#[derive(Debug, Serialize)]
struct UpsertVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    #[serde(rename = "groupId")]
    group_id: String,
    #[serde(rename = "expectedPolicy")]
    expected_policy: Option<PolicyPreconditionInput>,
    input: UpsertInput,
}

#[derive(Debug, Serialize)]
struct PolicyPreconditionInput {
    #[serde(rename = "policyId")]
    policy_id: String,
    revision: u64,
    locale: String,
}

impl From<GroupsAdminApplicationPolicyPrecondition> for PolicyPreconditionInput {
    fn from(value: GroupsAdminApplicationPolicyPrecondition) -> Self {
        Self {
            policy_id: value.policy_id,
            revision: value.revision,
            locale: value.locale,
        }
    }
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
struct CatalogResponse {
    group_application_policy_locale_catalog: CatalogWire,
}

#[derive(Debug, Deserialize)]
struct ManagementResponse {
    group_application_policy_for_management: ManagementWire,
}

#[derive(Debug, Deserialize)]
struct UpsertResponse {
    upsert_group_application_policy: UpsertWire,
}

#[derive(Debug, Deserialize)]
struct CatalogWire {
    group_id: String,
    policy_id: Option<String>,
    revision: Option<u64>,
    enabled: bool,
    locales: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ManagementWire {
    group_id: String,
    policy_id: Option<String>,
    revision: Option<u64>,
    enabled: bool,
    locale: String,
    translation_exists: bool,
    questions: Vec<QuestionWire>,
    rules: Vec<RuleWire>,
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

pub async fn load_group_application_policy_locale_catalog(
    token: Option<String>,
    tenant_slug: Option<String>,
    query: GroupsAdminApplicationPolicyLocaleCatalogQuery,
) -> Result<GroupsAdminApplicationPolicyLocaleCatalog, GraphqlGroupsPolicyLocaleError> {
    let response: CatalogResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            locale_catalog_query(),
            Some(CatalogVariables {
                group_id: query.group_id,
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok(GroupsAdminApplicationPolicyLocaleCatalog {
        group_id: response.group_application_policy_locale_catalog.group_id,
        policy_id: response.group_application_policy_locale_catalog.policy_id,
        revision: response.group_application_policy_locale_catalog.revision,
        enabled: response.group_application_policy_locale_catalog.enabled,
        locales: response.group_application_policy_locale_catalog.locales,
    })
}

pub async fn load_group_application_policy_for_management(
    token: Option<String>,
    tenant_slug: Option<String>,
    query: GroupsAdminApplicationPolicyQuery,
) -> Result<GroupsAdminApplicationPolicyManagementView, GraphqlGroupsPolicyLocaleError> {
    let response: ManagementResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            management_policy_query(),
            Some(ManagementVariables {
                group_id: query.group_id,
                locale: query.locale,
            }),
        ),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok(response.group_application_policy_for_management.into())
}

pub async fn upsert_group_application_policy(
    token: Option<String>,
    tenant_slug: Option<String>,
    command: UpsertGroupApplicationPolicyCommand,
) -> Result<GroupsAdminUpsertApplicationPolicyResult, GraphqlGroupsPolicyLocaleError> {
    let response: UpsertResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            upsert_policy_mutation(),
            Some(UpsertVariables {
                idempotency_key: command.idempotency_key,
                group_id: command.group_id,
                expected_policy: command.expected_policy.map(Into::into),
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

impl From<ManagementWire> for GroupsAdminApplicationPolicyManagementView {
    fn from(value: ManagementWire) -> Self {
        Self {
            group_id: value.group_id,
            policy_id: value.policy_id,
            revision: value.revision,
            enabled: value.enabled,
            locale: value.locale,
            translation_exists: value.translation_exists,
            questions: value.questions.into_iter().map(Into::into).collect(),
            rules: value.rules.into_iter().map(Into::into).collect(),
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
