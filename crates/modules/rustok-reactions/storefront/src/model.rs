use serde::{Deserialize, Serialize};
use uuid::Uuid;

const MAX_REACTION_SOURCE_SLUG_BYTES: usize = 64;
const MAX_REACTION_SUBJECT_KIND_BYTES: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
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
        if !valid_segment(source.as_str(), MAX_REACTION_SOURCE_SLUG_BYTES, true, true)
            || !valid_segment(kind.as_str(), MAX_REACTION_SUBJECT_KIND_BYTES, false, true)
        {
            return Err("reaction subject source or kind is invalid".to_string());
        }
        if subject_id.is_nil() {
            return Err("reaction subject id must be non-nil".to_string());
        }
        let revision = subject_revision.parse::<u64>().map_err(|_| {
            "reaction subject revision must be a positive canonical u64 string".to_string()
        })?;
        if revision == 0 || revision.to_string() != subject_revision {
            return Err(
                "reaction subject revision must be a positive canonical u64 string".to_string(),
            );
        }
        Ok(Self {
            source,
            kind,
            subject_id,
            subject_revision,
        })
    }
}

fn valid_segment(
    value: &str,
    maximum_bytes: usize,
    allow_hyphen: bool,
    allow_underscore: bool,
) -> bool {
    if value.is_empty()
        || value.trim() != value
        || value.len() > maximum_bytes
        || value.starts_with('-')
        || value.starts_with('_')
        || value.ends_with('-')
        || value.ends_with('_')
    {
        return false;
    }

    value.bytes().all(|byte| {
        byte.is_ascii_lowercase()
            || byte.is_ascii_digit()
            || (allow_hyphen && byte == b'-')
            || (allow_underscore && byte == b'_')
    })
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

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct ReactionWriteResultView {
    #[serde(rename = "commandId")]
    pub command_id: Uuid,
    pub changed: bool,
}

#[cfg(test)]
mod tests {
    use super::{
        ReactionAction, ReactionActorStateView, ReactionAggregateView, ReactionCatalogView,
        ReactionSelectionMode, ReactionSnapshotView, ReactionSubjectUiRef, ReactionWriteResultView,
    };
    use uuid::Uuid;

    #[test]
    fn subject_ref_requires_exact_neutral_identity_and_revision() {
        assert!(ReactionSubjectUiRef::new("forum", "topic", Uuid::new_v4(), "1").is_ok());
        for revision in ["0", "-1", "01", " 1 ", "+1", "", "abc", "1.0"] {
            assert!(
                ReactionSubjectUiRef::new("forum", "topic", Uuid::new_v4(), revision).is_err(),
                "revision {revision:?} must be rejected",
            );
        }
        assert!(ReactionSubjectUiRef::new("Forum", "topic", Uuid::new_v4(), "1").is_err());
        assert!(ReactionSubjectUiRef::new("forum", "topic-kind", Uuid::new_v4(), "1").is_err());
        assert!(ReactionSubjectUiRef::new("forum", "topic", Uuid::nil(), "1").is_err());
    }

    #[test]
    fn subject_ref_validates_segment_boundaries_and_special_characters() {
        let valid_uuid = Uuid::new_v4();
        // Valid sources and kinds
        assert!(ReactionSubjectUiRef::new("forum_source-1", "topic_kind", valid_uuid, "1").is_ok());

        // Leading/trailing delimiters rejected
        assert!(ReactionSubjectUiRef::new("-forum", "topic", valid_uuid, "1").is_err());
        assert!(ReactionSubjectUiRef::new("forum-", "topic", valid_uuid, "1").is_err());
        assert!(ReactionSubjectUiRef::new("_forum", "topic", valid_uuid, "1").is_err());
        assert!(ReactionSubjectUiRef::new("forum_", "topic", valid_uuid, "1").is_err());
        assert!(ReactionSubjectUiRef::new("forum", "_topic", valid_uuid, "1").is_err());
        assert!(ReactionSubjectUiRef::new("forum", "topic_", valid_uuid, "1").is_err());

        // Whitespace rejected
        assert!(ReactionSubjectUiRef::new(" forum", "topic", valid_uuid, "1").is_err());
        assert!(ReactionSubjectUiRef::new("forum ", "topic", valid_uuid, "1").is_err());
        assert!(ReactionSubjectUiRef::new("forum", " topic", valid_uuid, "1").is_err());
        assert!(ReactionSubjectUiRef::new("forum", "topic ", valid_uuid, "1").is_err());

        // Maximum 64 bytes
        let slug_64 = "a".repeat(64);
        assert!(ReactionSubjectUiRef::new(&slug_64, "topic", valid_uuid, "1").is_ok());
        let slug_65 = "a".repeat(65);
        assert!(ReactionSubjectUiRef::new(&slug_65, "topic", valid_uuid, "1").is_err());
        assert!(ReactionSubjectUiRef::new("forum", &slug_65, valid_uuid, "1").is_err());

        // Empty segments rejected
        assert!(ReactionSubjectUiRef::new("", "topic", valid_uuid, "1").is_err());
        assert!(ReactionSubjectUiRef::new("forum", "", valid_uuid, "1").is_err());
    }

    #[test]
    fn actor_state_controls_mutation_availability() {
        let mut snapshot = ReactionSnapshotView {
            catalog: ReactionCatalogView {
                selection_mode: ReactionSelectionMode::Single,
                max_selected: 1,
                keys: vec!["like".to_string()],
            },
            actor_state: None,
            aggregates: vec![],
        };
        assert!(!snapshot.can_mutate());
        assert!(!snapshot.is_selected("like"));
        snapshot.actor_state = Some(ReactionActorStateView {
            revision: "1".to_string(),
            selected: vec!["like".to_string()],
        });
        assert!(snapshot.can_mutate());
        assert!(snapshot.is_selected("like"));
        assert!(!snapshot.is_selected("dislike"));
    }

    #[test]
    fn aggregate_count_returns_existing_or_defaults_to_zero() {
        let snapshot = ReactionSnapshotView {
            catalog: ReactionCatalogView {
                selection_mode: ReactionSelectionMode::Multiple,
                max_selected: 5,
                keys: vec!["like".to_string(), "rocket".to_string()],
            },
            actor_state: None,
            aggregates: vec![ReactionAggregateView {
                reaction: "like".to_string(),
                count: "42".to_string(),
            }],
        };
        assert_eq!(snapshot.aggregate_count("like"), "42");
        assert_eq!(snapshot.aggregate_count("rocket"), "0");
        assert_eq!(snapshot.aggregate_count("nonexistent"), "0");
    }

    #[test]
    fn serde_roundtrips_for_actions_and_modes() {
        assert_eq!(
            serde_json::to_string(&ReactionAction::Add).expect("serialize"),
            "\"ADD\""
        );
        assert_eq!(
            serde_json::to_string(&ReactionAction::Remove).expect("serialize"),
            "\"REMOVE\""
        );
        assert_eq!(
            serde_json::from_str::<ReactionAction>("\"ADD\"").expect("deserialize"),
            ReactionAction::Add
        );
        assert_eq!(
            serde_json::from_str::<ReactionAction>("\"REMOVE\"").expect("deserialize"),
            ReactionAction::Remove
        );

        assert_eq!(
            serde_json::to_string(&ReactionSelectionMode::Single).expect("serialize"),
            "\"SINGLE\""
        );
        assert_eq!(
            serde_json::to_string(&ReactionSelectionMode::Multiple).expect("serialize"),
            "\"MULTIPLE\""
        );
        assert_eq!(
            serde_json::from_str::<ReactionSelectionMode>("\"SINGLE\"").expect("deserialize"),
            ReactionSelectionMode::Single
        );
        assert_eq!(
            serde_json::from_str::<ReactionSelectionMode>("\"MULTIPLE\"").expect("deserialize"),
            ReactionSelectionMode::Multiple
        );

        let command_id = Uuid::new_v4();
        let json = format!(r#"{{"commandId":"{command_id}","changed":true}}"#);
        let result: ReactionWriteResultView =
            serde_json::from_str(&json).expect("deserialize ReactionWriteResultView");
        assert_eq!(result.command_id, command_id);
        assert!(result.changed);
    }
}
