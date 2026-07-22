#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{execute as execute_graphql, GraphqlRequest};
use serde::{Deserialize, Serialize};

use crate::application_model::{
    GroupsAdminApplicationPolicyRevision, GroupsAdminApplicationPolicyRevisionConnection,
    GroupsAdminApplicationPolicyRevisionQuery, GroupsAdminApplicationQuestion,
    GroupsAdminApplicationRule,
};

pub type GraphqlGroupsPolicyHistoryError = String;

const POLICY_HISTORY_QUERY: &str = "query GroupsAdminApplicationPolicyHistory($groupId: UUID!, $page: Int, $perPage: Int) { group_application_policy_revisions: groupApplicationPolicyRevisions(groupId: $groupId, page: $page, perPage: $perPage) { total page per_page: perPage items { group_id: groupId policy_id: policyId revision locale enabled created_by_user_id: createdByUserId created_at: createdAt questions { key prompt help_text: helpText required max_answer_chars: maxAnswerChars } rules { key title body required } } } }";

#[derive(Debug, Serialize)]
struct Variables {
    #[serde(rename = "groupId")]
    group_id: String,
    page: i32,
    #[serde(rename = "perPage")]
    per_page: i32,
}

#[derive(Debug, Deserialize)]
struct Response {
    group_application_policy_revisions: ConnectionWire,
}

#[derive(Debug, Deserialize)]
struct ConnectionWire {
    items: Vec<RevisionWire>,
    total: u64,
    page: u64,
    per_page: u64,
}

#[derive(Debug, Deserialize)]
struct RevisionWire {
    group_id: String,
    policy_id: String,
    revision: u64,
    locale: String,
    enabled: bool,
    questions: Vec<QuestionWire>,
    rules: Vec<RuleWire>,
    created_by_user_id: String,
    created_at: String,
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

pub async fn load_group_application_policy_revisions(
    token: Option<String>,
    tenant_slug: Option<String>,
    query: GroupsAdminApplicationPolicyRevisionQuery,
) -> Result<GroupsAdminApplicationPolicyRevisionConnection, GraphqlGroupsPolicyHistoryError> {
    let response: Response = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            POLICY_HISTORY_QUERY,
            Some(Variables {
                group_id: query.group_id,
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

    Ok(GroupsAdminApplicationPolicyRevisionConnection {
        items: response
            .group_application_policy_revisions
            .items
            .into_iter()
            .map(Into::into)
            .collect(),
        total: response.group_application_policy_revisions.total,
        page: response.group_application_policy_revisions.page,
        per_page: response.group_application_policy_revisions.per_page,
    })
}

impl From<RevisionWire> for GroupsAdminApplicationPolicyRevision {
    fn from(value: RevisionWire) -> Self {
        Self {
            group_id: value.group_id,
            policy_id: value.policy_id,
            revision: value.revision,
            locale: value.locale,
            enabled: value.enabled,
            questions: value.questions.into_iter().map(Into::into).collect(),
            rules: value.rules.into_iter().map(Into::into).collect(),
            created_by_user_id: value.created_by_user_id,
            created_at: value.created_at,
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
