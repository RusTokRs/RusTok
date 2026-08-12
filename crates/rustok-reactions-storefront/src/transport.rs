use leptos::prelude::*;
#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use leptos_auth::AuthContext;
use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{
    ReactionAction, ReactionSnapshotView, ReactionSubjectUiRef, ReactionWriteResultView,
};

pub type ReactionStorefrontTransportError = String;

const SNAPSHOT_QUERY: &str = r#"
query ReactionStorefrontSnapshot($subject: ReactionSubjectInput!) {
  reactionSnapshot(subject: $subject) {
    catalog {
      selectionMode
      maxSelected
      keys
    }
    actorState {
      revision
      selected
    }
    aggregates {
      reaction
      count
    }
  }
}
"#;

const APPLY_MUTATION: &str = r#"
mutation ReactionStorefrontApply($input: ApplyReactionInput!) {
  applyReaction(input: $input) {
    commandId
    changed
  }
}
"#;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReactionStorefrontTransportContext {
    pub access_token: Option<String>,
    pub tenant_slug: Option<String>,
}

impl ReactionStorefrontTransportContext {
    pub fn new(access_token: Option<String>, tenant_slug: Option<String>) -> Self {
        Self {
            access_token,
            tenant_slug,
        }
    }
}

pub fn current_reaction_storefront_transport_context() -> ReactionStorefrontTransportContext {
    let auth = use_context::<AuthContext>();
    let access_token = auth.as_ref().and_then(AuthContext::get_token);
    let tenant_slug = auth
        .as_ref()
        .and_then(AuthContext::get_tenant)
        .or_else(|| option_env!("RUSTOK_TENANT_SLUG").map(str::to_string));
    ReactionStorefrontTransportContext::new(access_token, tenant_slug)
}

#[derive(Debug, Serialize)]
struct SnapshotVariables {
    subject: SubjectInputWire,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SubjectInputWire {
    source: String,
    kind: String,
    subject_id: Uuid,
    subject_revision: String,
}

impl From<ReactionSubjectUiRef> for SubjectInputWire {
    fn from(subject: ReactionSubjectUiRef) -> Self {
        Self {
            source: subject.source,
            kind: subject.kind,
            subject_id: subject.subject_id,
            subject_revision: subject.subject_revision,
        }
    }
}

#[derive(Debug, Serialize)]
struct ApplyVariables {
    input: ApplyInputWire,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApplyInputWire {
    command_id: Uuid,
    subject: SubjectInputWire,
    reaction: String,
    action: ActionWire,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ActionWire {
    Add,
    Remove,
}

impl From<ReactionAction> for ActionWire {
    fn from(action: ReactionAction) -> Self {
        match action {
            ReactionAction::Add => Self::Add,
            ReactionAction::Remove => Self::Remove,
        }
    }
}

#[derive(Debug, Deserialize)]
struct SnapshotResponse {
    #[serde(rename = "reactionSnapshot")]
    snapshot: ReactionSnapshotView,
}

#[derive(Debug, Deserialize)]
struct ApplyResponse {
    #[serde(rename = "applyReaction")]
    result: ReactionWriteResultView,
}

pub async fn load_reaction_snapshot(
    context: ReactionStorefrontTransportContext,
    subject: ReactionSubjectUiRef,
) -> Result<ReactionSnapshotView, ReactionStorefrontTransportError> {
    let response: SnapshotResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            SNAPSHOT_QUERY,
            Some(SnapshotVariables {
                subject: subject.into(),
            }),
        ),
        context.access_token,
        context.tenant_slug,
        None,
    )
    .await
    .map_err(|_| "reaction snapshot is unavailable".to_string())?;

    Ok(response.snapshot)
}

pub async fn apply_reaction(
    context: ReactionStorefrontTransportContext,
    subject: ReactionSubjectUiRef,
    reaction: String,
    action: ReactionAction,
) -> Result<ReactionWriteResultView, ReactionStorefrontTransportError> {
    let response: ApplyResponse = execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(
            APPLY_MUTATION,
            Some(ApplyVariables {
                input: ApplyInputWire {
                    command_id: Uuid::new_v4(),
                    subject: subject.into(),
                    reaction,
                    action: action.into(),
                },
            }),
        ),
        context.access_token,
        context.tenant_slug,
        None,
    )
    .await
    .map_err(|_| "reaction update is unavailable".to_string())?;

    Ok(response.result)
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

#[cfg(test)]
mod tests {
    use super::{APPLY_MUTATION, SNAPSHOT_QUERY, SubjectInputWire};
    use crate::model::ReactionSubjectUiRef;
    use uuid::Uuid;

    #[test]
    fn graphql_inputs_never_accept_tenant_or_actor_identity() {
        for operation in [SNAPSHOT_QUERY, APPLY_MUTATION] {
            assert!(!operation.contains("tenantId"));
            assert!(!operation.contains("actorId"));
        }
    }

    #[test]
    fn mutation_preserves_owner_command_identity_contract() {
        assert!(APPLY_MUTATION.contains("$input: ApplyReactionInput!"));
        assert!(APPLY_MUTATION.contains("commandId"));
        assert!(APPLY_MUTATION.contains("applyReaction"));
    }

    #[test]
    fn subject_wire_uses_graphql_camel_case_fields() {
        let subject =
            ReactionSubjectUiRef::new("forum", "topic", Uuid::new_v4(), "42").expect("subject");
        let value = serde_json::to_value(SubjectInputWire::from(subject)).expect("serialize");
        assert!(value.get("subjectId").is_some());
        assert_eq!(
            value
                .get("subjectRevision")
                .and_then(|value| value.as_str()),
            Some("42")
        );
        assert!(value.get("tenantId").is_none());
        assert!(value.get("actorId").is_none());
    }
}
