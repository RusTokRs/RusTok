mod keys;
mod model;
#[cfg(feature = "server")]
mod provider;

pub use keys::*;
pub use model::*;
#[cfg(feature = "server")]
pub use provider::*;

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
            assert!(NotificationTypeKey::new(invalid).is_err(), "accepted {invalid:?}");
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
    fn audience_pages_reject_duplicates_and_excessive_fanout() {
        let recipient_id = Uuid::new_v4();
        let duplicate = vec![
            NotificationAudienceCandidate { recipient_id },
            NotificationAudienceCandidate { recipient_id },
        ];
        assert!(NotificationAudiencePage::try_new(duplicate, None).is_err());

        let oversized = (0..=MAX_NOTIFICATION_AUDIENCE_PAGE_SIZE)
            .map(|_| NotificationAudienceCandidate {
                recipient_id: Uuid::new_v4(),
            })
            .collect();
        assert!(NotificationAudiencePage::try_new(oversized, None).is_err());
    }

    #[test]
    fn target_routes_are_root_relative_and_safe() {
        assert!(NotificationTargetRoute::new("/forum/topic/123").is_ok());
        for invalid in [
            "https://example.invalid/topic/123",
            "//example.invalid/topic/123",
            "forum/topic/123",
            "/forum/topic/123\nset-cookie:x",
        ] {
            assert!(NotificationTargetRoute::new(invalid).is_err(), "accepted {invalid:?}");
        }
    }

    #[cfg(feature = "server")]
    mod server {
        use async_trait::async_trait;
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

        #[test]
        fn runtime_registry_is_unique_and_discoverable() {
            let mut extensions = ModuleRuntimeExtensions::default();
            register_notification_source_provider(&mut extensions, DummySource)
                .expect("first source registration");
            assert!(register_notification_source_provider(&mut extensions, DummySource).is_err());

            let registry = notification_source_registry_from_extensions(&extensions)
                .expect("registry should be available");
            assert_eq!(registry.len(), 1);
            assert_eq!(registry.entries()[0].supported_types.len(), 1);
            assert!(registry.get_by_str("forum").is_some());
        }
    }
}
