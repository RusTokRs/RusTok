mod keys;
mod model;
#[cfg(feature = "server")]
mod provider;
#[cfg(feature = "server")]
mod reconciliation;

pub use keys::*;
pub use model::*;
#[cfg(feature = "server")]
pub use provider::*;
#[cfg(feature = "server")]
pub use reconciliation::*;
#[cfg(feature = "server")]
pub use rustok_api::{PortContext, PortError};

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use uuid::Uuid;

    use super::*;

    #[test]
    fn semantic_keys_fail_closed_for_ambiguous_values() {
        assert!(NotificationSourceSlug::new("forum").is_ok());
        assert!(NotificationTypeKey::new("forum.reply.approved").is_ok());
        assert!(NotificationTemplateKey::new("forum.reply.approved").is_ok());

        for invalid in ["Forum", " forum", "forum ", "forum/source", "forum..reply"] {
            assert!(
                NotificationTypeKey::new(invalid).is_err(),
                "accepted {invalid:?}"
            );
        }
        assert!(NotificationSourceSlug::new("forum.reply").is_err());
    }

    #[test]
    fn template_data_is_bounded_and_validated_during_deserialization() {
        let mut values = BTreeMap::new();
        values.insert("topic_title".to_string(), "Welcome".to_string());
        let data = NotificationTemplateData::try_new(values).expect("valid template data");
        assert_eq!(data.get("topic_title"), Some("Welcome"));

        let invalid = serde_json::json!({"unsafe key": "value"});
        assert!(serde_json::from_value::<NotificationTemplateData>(invalid).is_err());
    }

    #[test]
    fn source_revision_is_private_and_validated_during_deserialization() {
        let tenant_id = Uuid::new_v4();
        let event_id = Uuid::new_v4();
        let event = NotificationSourceEventRef::new(
            tenant_id,
            event_id,
            NotificationSourceSlug::new("forum").expect("source"),
            NotificationTypeKey::new("forum.reply.approved").expect("type"),
            1,
        )
        .expect("event");
        assert_eq!(event.tenant_id(), tenant_id);
        assert_eq!(event.event_id(), event_id);
        assert_eq!(event.source_revision(), 1);

        for invalid in [
            serde_json::json!({
                "tenant_id": tenant_id,
                "event_id": event_id,
                "source": "forum",
                "event_type": "forum.reply.approved",
                "source_revision": 0
            }),
            serde_json::json!({
                "tenant_id": Uuid::nil(),
                "event_id": event_id,
                "source": "forum",
                "event_type": "forum.reply.approved",
                "source_revision": 1
            }),
            serde_json::json!({
                "tenant_id": tenant_id,
                "event_id": Uuid::nil(),
                "source": "forum",
                "event_type": "forum.reply.approved",
                "source_revision": 1
            }),
            serde_json::json!({
                "tenant_id": tenant_id,
                "event_id": event_id,
                "source": "forum",
                "event_type": "forum.reply.approved",
                "source_revision": (i64::MAX as u64) + 1
            }),
        ] {
            assert!(serde_json::from_value::<NotificationSourceEventRef>(invalid).is_err());
        }

        assert_eq!(
            NotificationSourceEventRef::new(
                Uuid::nil(),
                event_id,
                NotificationSourceSlug::new("forum").unwrap(),
                NotificationTypeKey::new("forum.reply.approved").unwrap(),
                1,
            ),
            Err(NotificationContractError::NilTenantId)
        );
        assert_eq!(
            NotificationSourceEventRef::new(
                tenant_id,
                Uuid::nil(),
                NotificationSourceSlug::new("forum").unwrap(),
                NotificationTypeKey::new("forum.reply.approved").unwrap(),
                1,
            ),
            Err(NotificationContractError::NilEventId)
        );
        assert_eq!(
            NotificationSourceEventRef::new(
                tenant_id,
                event_id,
                NotificationSourceSlug::new("forum").unwrap(),
                NotificationTypeKey::new("forum.reply.approved").unwrap(),
                (i64::MAX as u64) + 1,
            ),
            Err(NotificationContractError::InvalidSourceRevision)
        );
    }

    #[test]
    fn nil_uuids_are_rejected_in_contracts() {
        let slug = NotificationSourceSlug::new("forum").unwrap();
        let kind = NotificationTargetKind::new("forum.topic").unwrap();

        assert_eq!(
            NotificationTargetRef::new(slug.clone(), kind.clone(), Uuid::nil()),
            Err(NotificationContractError::NilTargetId)
        );

        let valid_target = NotificationTargetRef::new(slug, kind, Uuid::new_v4()).unwrap();
        assert!(valid_target.validate().is_ok());

        let invalid_target = NotificationTargetRef {
            owner: NotificationSourceSlug::new("forum").unwrap(),
            kind: NotificationTargetKind::new("forum.topic").unwrap(),
            id: Uuid::nil(),
        };
        assert_eq!(
            invalid_target.validate(),
            Err(NotificationContractError::NilTargetId)
        );

        let type_key = NotificationTypeKey::new("forum.reply.approved").unwrap();
        let tmpl_key = NotificationTemplateKey::new("forum.reply.approved").unwrap();

        let desc = NotificationSemanticDescriptor {
            notification_type: type_key.clone(),
            template_key: tmpl_key.clone(),
            target: invalid_target,
            actor_id: None,
            priority: NotificationPriority::Normal,
            template_data: NotificationTemplateData::default(),
        };
        assert_eq!(desc.validate(), Err(NotificationContractError::NilTargetId));

        let desc_nil_actor = NotificationSemanticDescriptor {
            notification_type: type_key,
            template_key: tmpl_key,
            target: valid_target,
            actor_id: Some(Uuid::nil()),
            priority: NotificationPriority::Normal,
            template_data: NotificationTemplateData::default(),
        };
        assert_eq!(
            desc_nil_actor.validate(),
            Err(NotificationContractError::NilActorId)
        );

        assert_eq!(
            NotificationAudienceCandidate::new(Uuid::nil()),
            Err(NotificationContractError::NilRecipientId)
        );
        let nil_candidate = NotificationAudienceCandidate {
            recipient_id: Uuid::nil(),
        };
        assert_eq!(
            nil_candidate.validate(),
            Err(NotificationContractError::NilRecipientId)
        );
        assert_eq!(
            NotificationAudiencePage::try_new(vec![nil_candidate], None),
            Err(NotificationContractError::NilRecipientId)
        );
    }

    #[test]
    fn audience_pages_reject_duplicates_and_excessive_fanout() {
        let recipient_id = Uuid::new_v4();
        let duplicate = vec![
            NotificationAudienceCandidate { recipient_id },
            NotificationAudienceCandidate { recipient_id },
        ];
        assert!(NotificationAudiencePage::try_new(duplicate.clone(), None).is_err());
        assert!(
            serde_json::from_value::<NotificationAudiencePage>(serde_json::json!({
                "recipients": duplicate,
                "next_cursor": null
            }))
            .is_err()
        );

        let oversized = (0..=MAX_NOTIFICATION_AUDIENCE_PAGE_SIZE)
            .map(|_| NotificationAudienceCandidate {
                recipient_id: Uuid::new_v4(),
            })
            .collect::<Vec<_>>();
        assert!(NotificationAudiencePage::try_new(oversized.clone(), None).is_err());
        assert!(
            serde_json::from_value::<NotificationAudiencePage>(serde_json::json!({
                "recipients": oversized,
                "next_cursor": null
            }))
            .is_err()
        );

        let page = NotificationAudiencePage::try_new(
            vec![NotificationAudienceCandidate { recipient_id }],
            None,
        )
        .expect("bounded page");
        assert_eq!(page.recipients().len(), 1);
        assert!(page.is_complete());
    }

    #[test]
    fn target_routes_allow_only_bounded_internal_queries() {
        assert!(NotificationTargetRoute::new("/forum/topic/123").is_ok());
        assert!(NotificationTargetRoute::new(
            "/modules/forum?category=79ff97c4-1811-4e8e-bc12-3cfe49529ee4&topic=307c7a02-6298-4fea-a722-39c10202aef5"
        )
        .is_ok());
        for invalid in [
            "https://example.invalid/topic/123",
            "//example.invalid/topic/123",
            "forum/topic/123",
            "/forum/topic/123\nset-cookie:x",
            "/../admin",
            "/forum/./topic",
            "/forum?preview",
            "/forum?preview=true&",
            "/forum?redirect=https://example.invalid",
            "/forum?topic=%2e%2e",
            "/forum#topic",
            "/forum\\topic",
        ] {
            assert!(
                NotificationTargetRoute::new(invalid).is_err(),
                "accepted {invalid:?}"
            );
        }
    }

    #[cfg(feature = "server")]
    mod server {
        use std::sync::Arc;

        use async_trait::async_trait;
        use rustok_api::HostRuntimeContext;
        use rustok_core::ModuleRuntimeExtensions;

        use super::*;

        struct DummySource;

        #[async_trait]
        impl NotificationSourceProvider for DummySource {
            fn slug(&self) -> NotificationSourceSlug {
                NotificationSourceSlug::new("forum").expect("valid dummy source")
            }

            fn display_name(&self) -> &'static str {
                "Forum"
            }

            fn supported_types(&self) -> Vec<NotificationTypeKey> {
                vec![
                    NotificationTypeKey::new("forum.reply.approved").expect("type"),
                    NotificationTypeKey::new("forum.reply.approved").expect("type"),
                ]
            }

            async fn describe_event(
                &self,
                _request: DescribeNotificationRequest,
            ) -> NotificationProviderResult<Option<NotificationSemanticDescriptor>> {
                Ok(None)
            }

            async fn resolve_audience(
                &self,
                _request: ResolveNotificationAudienceRequest,
            ) -> NotificationProviderResult<NotificationAudiencePage> {
                Ok(NotificationAudiencePage::empty())
            }

            async fn authorize_target_open(
                &self,
                _request: AuthorizeNotificationTargetRequest,
            ) -> NotificationProviderResult<NotificationOpenAuthorization> {
                Ok(NotificationOpenAuthorization::Unavailable)
            }
        }

        struct DummyFactory;

        impl NotificationSourceProviderFactory for DummyFactory {
            fn slug(&self) -> NotificationSourceSlug {
                NotificationSourceSlug::new("forum").expect("valid dummy source")
            }

            fn build(
                &self,
                _host: &HostRuntimeContext,
            ) -> NotificationProviderResult<Arc<dyn NotificationSourceProvider>> {
                Ok(Arc::new(DummySource))
            }
        }

        #[test]
        fn runtime_registries_are_unique_and_discoverable() {
            let mut extensions = ModuleRuntimeExtensions::default();
            register_notification_source_provider(&mut extensions, DummySource)
                .expect("first source registration");
            assert!(register_notification_source_provider(&mut extensions, DummySource).is_err());

            let registry = notification_source_registry_from_extensions(&extensions)
                .expect("registry should be available");
            assert_eq!(registry.len(), 1);
            assert_eq!(registry.entries()[0].supported_types.len(), 1);
            assert!(registry.get_by_str("forum").is_some());

            let mut factories = ModuleRuntimeExtensions::default();
            register_notification_source_provider_factory(&mut factories, DummyFactory)
                .expect("first factory registration");
            assert!(
                register_notification_source_provider_factory(&mut factories, DummyFactory)
                    .is_err()
            );
            let registry = notification_source_factory_registry_from_extensions(&factories)
                .expect("factory registry should be available");
            assert_eq!(registry.len(), 1);
        }

        #[test]
        fn provider_error_converts_to_core_error() {
            let invalid = NotificationProviderError::InvalidEvent;
            let core_err: rustok_core::Error = invalid.into();
            assert!(matches!(core_err, rustok_core::Error::Validation(_)));

            let rejected = NotificationProviderError::Rejected;
            let core_err: rustok_core::Error = rejected.into();
            assert!(matches!(core_err, rustok_core::Error::Validation(_)));

            let unavail = NotificationProviderError::CapabilityUnavailable { retryable: true };
            let core_err: rustok_core::Error = unavail.into();
            assert!(matches!(core_err, rustok_core::Error::Internal(_)));

            let contract_err = NotificationContractError::NilTenantId;
            let provider_err: NotificationProviderError = contract_err.clone().into();
            assert_eq!(provider_err, NotificationProviderError::InvalidEvent);
            let core_err: rustok_core::Error = contract_err.into();
            assert!(matches!(core_err, rustok_core::Error::Validation(_)));
        }

        #[test]
        fn server_requests_validate_against_nil_ids() {
            let target = NotificationTargetRef::new(
                NotificationSourceSlug::new("forum").unwrap(),
                NotificationTargetKind::new("forum.topic").unwrap(),
                Uuid::new_v4(),
            )
            .unwrap();

            let auth_req_nil_tenant = AuthorizeNotificationTargetRequest {
                tenant_id: Uuid::nil(),
                recipient_id: Uuid::new_v4(),
                target: target.clone(),
            };
            assert_eq!(
                auth_req_nil_tenant.validate(),
                Err(NotificationContractError::NilTenantId)
            );

            let auth_req_nil_recipient = AuthorizeNotificationTargetRequest {
                tenant_id: Uuid::new_v4(),
                recipient_id: Uuid::nil(),
                target: target.clone(),
            };
            assert_eq!(
                auth_req_nil_recipient.validate(),
                Err(NotificationContractError::NilRecipientId)
            );

            let auth_req_nil_target = AuthorizeNotificationTargetRequest {
                tenant_id: Uuid::new_v4(),
                recipient_id: Uuid::new_v4(),
                target: NotificationTargetRef {
                    owner: NotificationSourceSlug::new("forum").unwrap(),
                    kind: NotificationTargetKind::new("forum.topic").unwrap(),
                    id: Uuid::nil(),
                },
            };
            assert_eq!(
                auth_req_nil_target.validate(),
                Err(NotificationContractError::NilTargetId)
            );

            let valid_auth_req = AuthorizeNotificationTargetRequest {
                tenant_id: Uuid::new_v4(),
                recipient_id: Uuid::new_v4(),
                target,
            };
            assert!(valid_auth_req.validate().is_ok());

            let reconcile_req_nil = NotificationInboxReconciliationInspectRequest {
                recipient_id: Uuid::nil(),
                cursor: None,
                limit: 10,
            };
            assert_eq!(
                reconcile_req_nil.validate(),
                Err(NotificationContractError::NilRecipientId)
            );

            let valid_reconcile = NotificationInboxReconciliationInspectRequest {
                recipient_id: Uuid::new_v4(),
                cursor: None,
                limit: 10,
            };
            assert!(valid_reconcile.validate().is_ok());
        }
    }
}
