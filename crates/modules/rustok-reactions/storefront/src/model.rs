use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReactionSubjectUiRef {
    pub source: String,
    pub kind: String,
    pub subject_id: Uuid,
    pub subject_revision: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReactionAction {
    Add,
    Remove,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReactionSelectionMode {
    Single,
    Multiple,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct ReactionCatalogView {
    #[serde(rename = "selectionMode")]
    pub selection_mode: ReactionSelectionMode,
    #[serde(rename = "maxSelected")]
    pub max_selected: i32,
    pub keys: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct ReactionActorStateView {
    pub revision: String,
    pub selected: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct ReactionAggregateView {
    pub reaction: String,
    pub count: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct ReactionSnapshotView {
    pub catalog: ReactionCatalogView,
    #[serde(rename = "actorState")]
    pub actor_state: Option<ReactionActorStateView>,
    pub aggregates: Vec<ReactionAggregateView>,
}

/// Pure action/presentation policy, shared independently of a UI runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReactionControlView {
    pub reaction: String,
    pub action: ReactionAction,
    pub selected: bool,
    pub count: String,
    pub label: String,
    pub aria_label: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct ReactionWriteResultView {
    #[serde(rename = "commandId")]
    pub command_id: Uuid,
    pub changed: bool,
}
