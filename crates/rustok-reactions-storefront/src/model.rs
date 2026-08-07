use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReactionSubjectUiRef {
    pub source: String,
    pub kind: String,
    pub subject_id: Uuid,
    pub subject_revision: String,
}

impl ReactionSubjectUiRef {
    pub fn new(
        source: impl Into<String>,
        kind: impl Into<String>,
        subject_id: Uuid,
        subject_revision: impl Into<String>,
    ) -> Result<Self, String> {
        let source = source.into();
        let kind = kind.into();
        let subject_revision = subject_revision.into();
        if source.trim().is_empty() || kind.trim().is_empty() {
            return Err("reaction subject source and kind are required".to_string());
        }
        let revision = subject_revision
            .trim()
            .parse::<u64>()
            .map_err(|_| "reaction subject revision must be a positive u64 string".to_string())?;
        if revision == 0 {
            return Err("reaction subject revision must be a positive u64 string".to_string());
        }
        Ok(Self {
            source,
            kind,
            subject_id,
            subject_revision,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReactionAction {
    Add,
    Remove,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReactionSelectionMode {
    Single,
    Multiple,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct ReactionCatalogView {
    #[serde(rename = "selectionMode")]
    pub selection_mode: ReactionSelectionMode,
    #[serde(rename = "maxSelected")]
    pub max_selected: i32,
    pub keys: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct ReactionActorStateView {
    pub revision: String,
    pub selected: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct ReactionAggregateView {
    pub reaction: String,
    pub count: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct ReactionSnapshotView {
    pub catalog: ReactionCatalogView,
    #[serde(rename = "actorState")]
    pub actor_state: Option<ReactionActorStateView>,
    pub aggregates: Vec<ReactionAggregateView>,
}

impl ReactionSnapshotView {
    pub fn is_selected(&self, reaction: &str) -> bool {
        self.actor_state
            .as_ref()
            .is_some_and(|state| state.selected.iter().any(|key| key == reaction))
    }

    pub fn aggregate_count(&self, reaction: &str) -> &str {
        self.aggregates
            .iter()
            .find(|aggregate| aggregate.reaction == reaction)
            .map(|aggregate| aggregate.count.as_str())
            .unwrap_or("0")
    }

    pub fn can_mutate(&self) -> bool {
        self.actor_state.is_some()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct ReactionWriteResultView {
    #[serde(rename = "commandId")]
    pub command_id: Uuid,
    pub changed: bool,
}

#[cfg(test)]
mod tests {
    use super::{ReactionActorStateView, ReactionSnapshotView, ReactionSubjectUiRef};
    use uuid::Uuid;

    #[test]
    fn subject_ref_requires_positive_revision() {
        assert!(ReactionSubjectUiRef::new("forum", "topic", Uuid::new_v4(), "1").is_ok());
        assert!(ReactionSubjectUiRef::new("forum", "topic", Uuid::new_v4(), "0").is_err());
        assert!(ReactionSubjectUiRef::new("forum", "topic", Uuid::new_v4(), "-1").is_err());
    }

    #[test]
    fn actor_state_controls_mutation_availability() {
        let mut snapshot = ReactionSnapshotView {
            catalog: super::ReactionCatalogView {
                selection_mode: super::ReactionSelectionMode::Single,
                max_selected: 1,
                keys: vec!["like".to_string()],
            },
            actor_state: None,
            aggregates: vec![],
        };
        assert!(!snapshot.can_mutate());
        snapshot.actor_state = Some(ReactionActorStateView {
            revision: "1".to_string(),
            selected: vec!["like".to_string()],
        });
        assert!(snapshot.can_mutate());
        assert!(snapshot.is_selected("like"));
    }
}
