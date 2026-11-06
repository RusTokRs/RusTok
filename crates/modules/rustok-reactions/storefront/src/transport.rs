use leptos::prelude::*;
use leptos_auth::AuthContext;
use rustok_graphql::{GraphqlRequest, execute as execute_graphql, graphql_url};
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

#[derive(Clone, Default, Eq, Hash, PartialEq)]
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

impl std::fmt::Debug for ReactionStorefrontTransportContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ReactionStorefrontTransportContext")
            .field(
                "access_token",
                &self.access_token.as_ref().map(|_| "<redacted>"),
            )
            .field("tenant_slug", &self.tenant_slug)
            .finish()
    }
}

impl ReactionStorefrontTransportContext {
    /// Read the canonical reactive Auth owner without interpreting token claims.
    pub fn from_auth(auth: Option<&AuthContext>) -> Self {
        let access_token = auth.and_then(AuthContext::get_token);
        let tenant_slug = auth
            .and_then(AuthContext::get_tenant)
            .or_else(|| option_env!("RUSTOK_TENANT_SLUG").map(str::to_string));
        Self::new(access_token, tenant_slug)
    }
}

pub fn current_reaction_storefront_transport_context() -> ReactionStorefrontTransportContext {
    let auth = use_context::<AuthContext>();
    ReactionStorefrontTransportContext::from_auth(auth.as_ref())
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

    #[test]
    fn apply_input_wire_uses_camel_case_and_screaming_snake_case_action() {
        let subject =
            ReactionSubjectUiRef::new("forum", "topic", Uuid::new_v4(), "1").expect("subject");
        let command_id = Uuid::new_v4();
        let wire = super::ApplyInputWire {
            command_id,
            subject: subject.into(),
            reaction: "heart".to_string(),
            action: super::ActionWire::Add,
        };
        let value = serde_json::to_value(&wire).expect("serialize");
        assert_eq!(
            value.get("commandId").and_then(|v| v.as_str()),
            Some(command_id.to_string().as_str())
        );
        assert_eq!(
            value.get("reaction").and_then(|v| v.as_str()),
            Some("heart")
        );
        assert_eq!(value.get("action").and_then(|v| v.as_str()), Some("ADD"));
        assert!(value.get("subject").is_some());
    }
    #[test]
    fn credential_debug_does_not_expose_an_access_token() {
        let context = super::ReactionStorefrontTransportContext::new(
            Some("secret-token".into()),
            Some("tenant".into()),
        );
        let debug = format!("{context:?}");
        assert!(!debug.contains("secret-token"));
        assert!(debug.contains("<redacted>"));
    }
}
