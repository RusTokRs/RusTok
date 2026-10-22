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

        #[test]
        fn test_error_conversions() {
            let contract_err = ReactionContractError::NilIdentity;
            let core_err: rustok_core::Error = contract_err.into();
            assert!(matches!(core_err, rustok_core::Error::Validation(_)));

            let invalid_err = ReactionProviderError::InvalidRequest;
            let core_err: rustok_core::Error = invalid_err.into();
            assert!(matches!(core_err, rustok_core::Error::Validation(_)));

            let unavail_err = ReactionProviderError::Unavailable;
            let core_err: rustok_core::Error = unavail_err.into();
            assert!(matches!(core_err, rustok_core::Error::NotFound(_)));

            let internal_err = ReactionProviderError::Internal { retryable: false };
            let core_err: rustok_core::Error = internal_err.into();
            assert!(matches!(core_err, rustok_core::Error::Internal(_)));
        }

        #[test]
        fn test_provider_error_retryability() {
            assert!(ReactionProviderError::Conflict.is_retryable());
            assert!(ReactionProviderError::CapabilityUnavailable { retryable: true }.is_retryable());
            assert!(!ReactionProviderError::CapabilityUnavailable { retryable: false }.is_retryable());
            assert!(ReactionProviderError::Internal { retryable: true }.is_retryable());
            assert!(!ReactionProviderError::Internal { retryable: false }.is_retryable());
            assert!(!ReactionProviderError::InvalidRequest.is_retryable());
            assert!(!ReactionProviderError::Unavailable.is_retryable());
        }

        #[test]
        fn test_subject_request_and_authorization_validation() {
            let s1 = subject("topic");
            let s2 = subject("reply");

            // Read request validation
            let read_req_ok = ReactionSubjectRequest {
                subject: s1.clone(),
                access: ReactionSubjectAccess::Read {
                    actor_id: Some(Uuid::new_v4()),
                },
            };
            assert!(read_req_ok.validate().is_ok());

            let read_req_nil = ReactionSubjectRequest {
                subject: s1.clone(),
                access: ReactionSubjectAccess::Read {
                    actor_id: Some(Uuid::nil()),
                },
            };
            assert_eq!(
                read_req_nil.validate().expect_err("nil actor"),
                ReactionContractError::NilIdentity
            );

            // Apply request validation
            let cmd_ok = ApplyReactionCommand::new(
                ReactionCommandIdentity::new(Uuid::new_v4(), Uuid::new_v4()).expect("identity"),
                s1.clone(),
                ReactionKey::new("like").expect("key"),
                ReactionAction::Add,
            );
            let apply_req_ok = ReactionSubjectRequest {
                subject: s1.clone(),
                access: ReactionSubjectAccess::Apply { command: cmd_ok },
            };
            assert!(apply_req_ok.validate().is_ok());

            let cmd_mismatch = ApplyReactionCommand::new(
                ReactionCommandIdentity::new(Uuid::new_v4(), Uuid::new_v4()).expect("identity"),
                s2.clone(),
                ReactionKey::new("like").expect("key"),
                ReactionAction::Add,
            );
            let apply_req_mismatch = ReactionSubjectRequest {
                subject: s1.clone(),
                access: ReactionSubjectAccess::Apply {
                    command: cmd_mismatch,
                },
            };
            assert_eq!(
                apply_req_mismatch.validate().expect_err("mismatch"),
                ReactionContractError::ProviderSubjectMismatch
            );

            // Authorization validation
            let cat = ReactionCatalog::try_new(
                ReactionSelectionPolicy::Single,
                vec![ReactionKey::new("like").expect("key")],
            )
            .expect("catalog");

            let auth_allowed_ok = ReactionSubjectAuthorization::Allowed {
                canonical_subject: s1.clone(),
                catalog: cat.clone(),
            };
            assert!(auth_allowed_ok.validate_for(&apply_req_ok).is_ok());

            let auth_allowed_mismatch = ReactionSubjectAuthorization::Allowed {
                canonical_subject: s2,
                catalog: cat,
            };
            assert_eq!(
                auth_allowed_mismatch
                    .validate_for(&apply_req_ok)
                    .expect_err("mismatch"),
                ReactionContractError::ProviderSubjectMismatch
            );

            let auth_unavail = ReactionSubjectAuthorization::Unavailable;
            assert!(auth_unavail.validate_for(&apply_req_ok).is_ok());
        }

        #[test]
        fn test_registry_deletion_bindings_and_invalid_kinds() {
            struct BoundProvider;

            #[async_trait]
            impl ReactionSubjectProvider for BoundProvider {
                fn source(&self) -> ReactionSourceSlug {
                    ReactionSourceSlug::new("blog").expect("source")
                }
                fn display_name(&self) -> &'static str {
                    "Blog"
                }
                fn supported_kinds(&self) -> Vec<ReactionSubjectKind> {
                    vec![ReactionSubjectKind::new("post").expect("kind")]
                }
                fn deletion_bindings(&self) -> Vec<ReactionSubjectDeletionBinding> {
                    vec![ReactionSubjectDeletionBinding {
                        source: self.source(),
                        kind: ReactionSubjectKind::new("post").expect("kind"),
                        target_type: "blog_post".to_string(),
                    }]
                }
                async fn authorize(
                    &self,
                    _context: PortContext,
                    _request: ReactionSubjectRequest,
                ) -> ReactionProviderResult<ReactionSubjectAuthorization> {
                    Ok(ReactionSubjectAuthorization::Unavailable)
                }
            }

            let mut registry = ReactionSubjectRegistry::default();
            assert!(registry.is_empty());
            assert_eq!(registry.len(), 0);

            registry.register(BoundProvider).expect("register");
            assert!(!registry.is_empty());
            assert_eq!(registry.len(), 1);

            let bindings = registry.deletion_bindings();
            assert_eq!(bindings.len(), 1);
            assert_eq!(bindings[0].target_type, "blog_post");

            // Empty kinds rejected
            struct EmptyKindProvider;
            #[async_trait]
            impl ReactionSubjectProvider for EmptyKindProvider {
                fn source(&self) -> ReactionSourceSlug {
                    ReactionSourceSlug::new("empty").expect("source")
                }
                fn display_name(&self) -> &'static str {
                    "Empty"
                }
                fn supported_kinds(&self) -> Vec<ReactionSubjectKind> {
                    Vec::new()
                }
                async fn authorize(
                    &self,
                    _ctx: PortContext,
                    _req: ReactionSubjectRequest,
                ) -> ReactionProviderResult<ReactionSubjectAuthorization> {
                    Ok(ReactionSubjectAuthorization::Unavailable)
                }
            }

            assert!(matches!(
                registry.register(EmptyKindProvider),
                Err(ReactionSubjectRegistryError::InvalidKinds { .. })
            ));
        }
    }

    #[test]
    fn test_selection_policy_bounds() {
        assert_eq!(ReactionSelectionPolicy::Single.maximum_selected(), 1);

        assert_eq!(
            ReactionSelectionPolicy::multiple(0).expect_err("0 rejected"),
            ReactionContractError::InvalidSelectionLimit
        );
        assert_eq!(
            ReactionSelectionPolicy::multiple(1).expect_err("1 rejected"),
            ReactionContractError::InvalidSelectionLimit
        );
        assert_eq!(
            ReactionSelectionPolicy::multiple(65).expect_err("65 rejected"),
            ReactionContractError::InvalidSelectionLimit
        );

        let multi = ReactionSelectionPolicy::multiple(5).expect("valid multiple");
        assert_eq!(multi.maximum_selected(), 5);
    }

    #[test]
    fn test_subject_ref_identity_comparison() {
        let tenant_id = Uuid::new_v4();
        let s_id = Uuid::new_v4();
        let ref_v1 = ReactionSubjectRef::new(
            tenant_id,
            ReactionSourceSlug::new("forum").expect("source"),
            ReactionSubjectKind::new("topic").expect("kind"),
            s_id,
            1,
        )
        .expect("ref");

        let ref_v2 = ReactionSubjectRef::new(
            tenant_id,
            ReactionSourceSlug::new("forum").expect("source"),
            ReactionSubjectKind::new("topic").expect("kind"),
            s_id,
            2,
        )
        .expect("ref");

        let ref_other = ReactionSubjectRef::new(
            tenant_id,
            ReactionSourceSlug::new("forum").expect("source"),
            ReactionSubjectKind::new("reply").expect("kind"),
            s_id,
            1,
        )
        .expect("ref");

        assert!(ref_v1.has_same_identity(&ref_v2));
        assert!(!ref_v1.has_same_identity(&ref_other));
        assert_eq!(ref_v1.tenant_id(), tenant_id);
        assert_eq!(ref_v1.subject_id(), s_id);
        assert_eq!(ref_v1.subject_revision(), 1);
    }

    #[test]
    fn test_actor_state_and_aggregates() {
        let valid_state = ReactionActorState::try_new(
            1,
            vec![
                ReactionKey::new("like").expect("key"),
                ReactionKey::new("love").expect("key"),
            ],
        );
        assert!(valid_state.is_ok());
        let state = valid_state.unwrap();
        assert_eq!(state.revision(), 1);
        assert_eq!(state.selected().len(), 2);

        let dup_state = ReactionActorState::try_new(
            1,
            vec![
                ReactionKey::new("like").expect("key"),
                ReactionKey::new("like").expect("key"),
            ],
        );
        assert_eq!(
            dup_state.expect_err("duplicate"),
            ReactionContractError::DuplicateActorReaction
        );

        let agg = ReactionAggregate::new(ReactionKey::new("clap").expect("key"), 42);
        assert_eq!(agg.reaction.as_str(), "clap");
        assert_eq!(agg.count, 42);
    }

    #[test]
    fn test_read_request_and_write_receipt_getters() {
        let s = subject("topic");
        let actor = Uuid::new_v4();

        let req_none = ReactionReadRequest::new(s.clone(), None).expect("read req");
        assert_eq!(req_none.actor_id(), None);
        assert_eq!(req_none.subject(), &s);

        let req_some = ReactionReadRequest::new(s.clone(), Some(actor)).expect("read req");
        assert_eq!(req_some.actor_id(), Some(actor));

        let cmd_id = Uuid::new_v4();
        let receipt = ReactionWriteReceipt::new(
            cmd_id,
            actor,
            s.clone(),
            ReactionKey::new("like").expect("key"),
            ReactionAction::Add,
            true,
            1,
        )
        .expect("receipt");

        assert_eq!(receipt.command_id(), cmd_id);
        assert_eq!(receipt.actor_id(), actor);
        assert_eq!(receipt.subject(), &s);
        assert_eq!(receipt.reaction().as_str(), "like");
        assert_eq!(receipt.action(), ReactionAction::Add);
        assert!(receipt.changed());
        assert_eq!(receipt.actor_state_revision(), 1);
    }
}
