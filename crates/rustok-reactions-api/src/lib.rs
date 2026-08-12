mod model;
#[cfg(feature = "server")]
mod provider;

pub use model::*;
#[cfg(feature = "server")]
pub use provider::*;
#[cfg(feature = "server")]
pub use rustok_api::{PortContext, PortError};

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    fn subject(kind: &str) -> ReactionSubjectRef {
        ReactionSubjectRef::new(
            Uuid::new_v4(),
            ReactionSourceSlug::new("forum").expect("source"),
            ReactionSubjectKind::new(kind).expect("kind"),
            Uuid::new_v4(),
            1,
        )
        .expect("subject")
    }

    #[test]
    fn semantic_keys_fail_closed_for_ambiguous_values() {
        assert!(ReactionSourceSlug::new("forum").is_ok());
        assert!(ReactionSubjectKind::new("reply").is_ok());
        assert!(ReactionKey::new("thumbs-up").is_ok());

        for invalid in ["Forum", " forum", "forum ", "forum/reply", "forum.reply"] {
            assert!(
                ReactionSourceSlug::new(invalid).is_err(),
                "accepted {invalid:?}"
            );
        }
        for invalid in ["Reply", "reply-kind", "reply/kind", "reply kind"] {
            assert!(
                ReactionSubjectKind::new(invalid).is_err(),
                "accepted {invalid:?}"
            );
        }
    }

    #[test]
    fn catalogs_validate_on_construction_and_deserialization() {
        let catalog = ReactionCatalog::try_new(
            ReactionSelectionPolicy::Single,
            vec![ReactionKey::new("like").expect("key")],
        )
        .expect("catalog");
        assert_eq!(catalog.selection().maximum_selected(), 1);

        let duplicate = serde_json::json!({
            "selection": { "mode": "single" },
            "keys": ["like", "like"]
        });
        assert!(serde_json::from_value::<ReactionCatalog>(duplicate).is_err());

        let invalid_multiple = serde_json::json!({
            "selection": { "mode": "multiple", "max_selected": 0 },
            "keys": ["like", "love"]
        });
        assert!(serde_json::from_value::<ReactionCatalog>(invalid_multiple).is_err());
    }

    #[test]
    fn identities_require_non_nil_ids_and_positive_subject_revision() {
        assert!(ReactionCommandIdentity::new(Uuid::nil(), Uuid::new_v4()).is_err());
        assert!(
            ReactionSubjectRef::new(
                Uuid::new_v4(),
                ReactionSourceSlug::new("forum").expect("source"),
                ReactionSubjectKind::new("reply").expect("kind"),
                Uuid::new_v4(),
                0,
            )
            .is_err()
        );
    }

    #[test]
    fn snapshot_rejects_values_outside_the_authorized_catalog() {
        let catalog = ReactionCatalog::try_new(
            ReactionSelectionPolicy::Single,
            vec![ReactionKey::new("like").expect("key")],
        )
        .expect("catalog");
        let aggregates = vec![ReactionAggregate {
            reaction: ReactionKey::new("love").expect("key"),
            count: 1,
        }];
        assert!(ReactionSnapshot::try_new(subject("topic"), catalog, None, aggregates).is_err());
    }

    #[cfg(feature = "server")]
    mod server {
        use async_trait::async_trait;
        use rustok_core::ModuleRuntimeExtensions;

        use super::*;

        struct DummyProvider;

        #[async_trait]
        impl ReactionSubjectProvider for DummyProvider {
            fn source(&self) -> ReactionSourceSlug {
                ReactionSourceSlug::new("forum").expect("source")
            }

            fn display_name(&self) -> &'static str {
                "Forum"
            }

            fn supported_kinds(&self) -> Vec<ReactionSubjectKind> {
                vec![
                    ReactionSubjectKind::new("topic").expect("kind"),
                    ReactionSubjectKind::new("reply").expect("kind"),
                ]
            }

            async fn authorize(
                &self,
                _context: PortContext,
                _request: ReactionSubjectRequest,
            ) -> ReactionProviderResult<ReactionSubjectAuthorization> {
                Ok(ReactionSubjectAuthorization::Unavailable)
            }
        }

        #[test]
        fn runtime_registry_is_unique_and_discoverable() {
            let mut extensions = ModuleRuntimeExtensions::default();
            register_reaction_subject_provider(&mut extensions, DummyProvider)
                .expect("first registration");
            assert!(register_reaction_subject_provider(&mut extensions, DummyProvider).is_err());

            let registry = reaction_subject_registry_from_extensions(&extensions)
                .expect("registry should be present");
            assert_eq!(registry.len(), 1);
            assert_eq!(registry.entries()[0].supported_kinds.len(), 2);
            assert!(registry.get_by_str("forum").is_some());
        }
    }
}
