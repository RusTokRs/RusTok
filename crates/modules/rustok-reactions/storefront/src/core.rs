//! Framework-independent subject validation, controls and write/read settlement.
use crate::model::*;
use std::future::Future;
use uuid::Uuid;

const MAX_REACTION_SOURCE_SLUG_BYTES: usize = 64;
const MAX_REACTION_SUBJECT_KIND_BYTES: usize = 64;

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

impl ReactionSnapshotView {
    pub fn controls(&self, like_label: &str, selected_label: &str) -> Vec<ReactionControlView> {
        self.catalog
            .keys
            .iter()
            .map(|reaction| {
                let selected = self.is_selected(reaction);
                let count = self.aggregate_count(reaction).to_owned();
                let label = if reaction == "like" {
                    like_label.to_string()
                } else {
                    reaction.clone()
                };
                let aria_label = if selected {
                    format!("{label}, {count}, {selected_label}")
                } else {
                    format!("{label}, {count}")
                };
                ReactionControlView {
                    reaction: reaction.clone(),
                    action: if selected {
                        ReactionAction::Remove
                    } else {
                        ReactionAction::Add
                    },
                    selected,
                    count,
                    label,
                    aria_label,
                }
            })
            .collect()
    }
}

/// A transport failure can happen after commit. Retain no local aggregate and
/// do not release the interaction lock until the canonical read settles.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettledReactionMutation {
    pub write_failed: bool,
    pub snapshot: ReactionSnapshotView,
}

/// `None` explicitly denotes a superseded authentication/request scope.
/// Transport failures still propagate when the request remains current.
pub async fn settle_reaction_mutation<E, Write, Read>(
    write: impl FnOnce() -> Write,
    reload: impl FnOnce() -> Read,
    is_current: impl Fn() -> bool,
) -> Result<Option<SettledReactionMutation>, E>
where
    Write: Future<Output = Result<ReactionWriteResultView, E>>,
    Read: Future<Output = Result<ReactionSnapshotView, E>>,
{
    if !is_current() {
        return Ok(None);
    }
    let write_failed = write().await.is_err();
    if !is_current() {
        return Ok(None);
    }
    let snapshot = reload().await?;
    if !is_current() {
        return Ok(None);
    }
    Ok(Some(SettledReactionMutation {
        write_failed,
        snapshot,
    }))
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

#[cfg(test)]
mod settlement_tests {
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        future::ready,
        task::{Context, Poll, Waker},
    };

    fn finish<T>(future: impl Future<Output = T>) -> T {
        let mut future = std::pin::pin!(future);
        match future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("ready test transport unexpectedly suspended"),
        }
    }

    fn snapshot() -> ReactionSnapshotView {
        ReactionSnapshotView {
            catalog: ReactionCatalogView {
                selection_mode: ReactionSelectionMode::Single,
                max_selected: 1,
                keys: vec!["like".into(), "rocket".into()],
            },
            actor_state: Some(ReactionActorStateView {
                revision: "42".into(),
                selected: vec!["like".into()],
            }),
            aggregates: vec![ReactionAggregateView {
                reaction: "like".into(),
                count: u64::MAX.to_string(),
            }],
        }
    }

    #[test]
    fn controls_preserve_exact_counts_and_derive_owner_actions() {
        let controls = snapshot().controls("Aimer", "Selectionne");
        assert_eq!(controls[0].count, "18446744073709551615");
        assert_eq!(controls[0].action, ReactionAction::Remove);
        assert!(controls[0].aria_label.contains("18446744073709551615"));
        assert_eq!(controls[1].count, "0");
        assert_eq!(controls[1].action, ReactionAction::Add);
    }

    #[test]
    fn uncertain_write_reads_owner_state_and_failed_reload_propagates() {
        let calls = RefCell::new(Vec::new());
        let settled = finish(settle_reaction_mutation(
            || {
                calls.borrow_mut().push("write");
                ready(Err::<ReactionWriteResultView, _>("timeout after commit"))
            },
            || {
                calls.borrow_mut().push("read");
                ready(Ok(snapshot()))
            },
            || true,
        ))
        .unwrap()
        .unwrap();
        assert_eq!(*calls.borrow(), ["write", "read"]);
        assert!(settled.write_failed);
        assert_eq!(
            settled.snapshot.aggregate_count("like"),
            "18446744073709551615"
        );
        let failed = finish(settle_reaction_mutation(
            || {
                ready(Ok(ReactionWriteResultView {
                    command_id: Uuid::new_v4(),
                    changed: true,
                }))
            },
            || ready(Err::<ReactionSnapshotView, _>("owner read unavailable")),
            || true,
        ));
        assert_eq!(failed.unwrap_err(), "owner read unavailable");
    }
    #[test]
    fn auth_change_before_or_during_reload_discards_the_old_actor_snapshot() {
        let active = Cell::new(true);
        let reads = Cell::new(0);
        let writes = Cell::new(0);
        active.set(false);
        let rejected = finish(settle_reaction_mutation(
            || {
                writes.set(writes.get() + 1);
                ready(Ok::<_, &str>(ReactionWriteResultView {
                    command_id: Uuid::new_v4(),
                    changed: true,
                }))
            },
            || ready(Ok(snapshot())),
            || active.get(),
        ))
        .unwrap();
        assert!(rejected.is_none());
        assert_eq!(
            writes.get(),
            0,
            "a stale credential scope must not start another write"
        );
        active.set(true);
        let result = finish(settle_reaction_mutation(
            || {
                active.set(false);
                ready(Ok::<_, &str>(ReactionWriteResultView {
                    command_id: Uuid::new_v4(),
                    changed: true,
                }))
            },
            || {
                reads.set(reads.get() + 1);
                ready(Ok(snapshot()))
            },
            || active.get(),
        ))
        .unwrap();
        assert!(result.is_none());
        assert_eq!(
            reads.get(),
            0,
            "a changed auth scope must not issue a stale reload"
        );

        active.set(true);
        let result = finish(settle_reaction_mutation(
            || {
                ready(Ok::<_, &str>(ReactionWriteResultView {
                    command_id: Uuid::new_v4(),
                    changed: true,
                }))
            },
            || {
                active.set(false);
                ready(Ok(snapshot()))
            },
            || active.get(),
        ))
        .unwrap();
        assert!(
            result.is_none(),
            "a late old-actor read must not enter current UI state"
        );
    }
}
