//! Contract and domain invariant tests for rustok-content-orchestration.

use uuid::Uuid;

#[test]
fn crate_api_defines_minimal_contract_sections() {
    let api = include_str!("../CRATE_API.md");
    for marker in [
        "## Public surface",
        "## Platform Capability Bindings",
        "## Domain Collaborations",
        "## Minimum Contract Set",
        "### Input DTOs/Commands",
        "### Domain Invariants",
        "### Events / Outbox Side Effects",
        "### Errors / Failure Codes",
    ] {
        assert!(
            api.contains(marker),
            "CRATE_API.md must contain section: {marker}"
        );
    }
}

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
mod full_contract_tests {
    use super::*;
    use crate::bridge::helpers::{locales_from_strs, normalize_slug};
    use crate::bridge::merge::unique_source_ids;
    use crate::graphql::{
        ContentOrchestrationMutation, ContentOrchestrationPayload, DemotePostToTopicInput,
        MergeTopicsInput, PromoteTopicToPostInput, SplitTopicInput,
    };
    use crate::{
        OwnerCanonicalRouteResolver, ServerContentOrchestrationBridge,
        SharedContentOrchestrationService,
    };
    use rustok_content::{CanonicalRouteResolver, OrchestrationResult};
    use sea_orm::Database;

    #[tokio::test]
    async fn shared_service_implements_debug_and_clone() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let event_bus = rustok_outbox::TransactionalEventBus::new(std::sync::Arc::new(
            rustok_outbox::OutboxTransport::new(db.clone()),
        ));
        let service = crate::build_content_orchestration_service(db, event_bus);
        let cloned = service.clone();
        let formatted = format!("{:?}", cloned);
        assert!(formatted.contains("SharedContentOrchestrationService"));
        let _service: SharedContentOrchestrationService = cloned;
    }

    #[tokio::test]
    async fn route_resolver_implements_debug_and_validates_nil_tenant() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let resolver = OwnerCanonicalRouteResolver::new(db);
        let debug_str = format!("{:?}", resolver);
        assert!(debug_str.contains("OwnerCanonicalRouteResolver"));

        // Nil tenant must be rejected
        let res = resolver
            .resolve_route(Uuid::nil(), "en", "/modules/blog?slug=test")
            .await;
        assert!(res.is_err());
        assert!(
            res.unwrap_err()
                .to_string()
                .contains("tenant_id must not be nil")
        );

        // Empty locale must be rejected
        let tenant_id = Uuid::new_v4();
        let res_locale = resolver
            .resolve_route(tenant_id, "  ", "/modules/blog?slug=test")
            .await;
        assert!(res_locale.is_err());
        assert!(
            res_locale
                .unwrap_err()
                .to_string()
                .contains("locale must not be empty")
        );
    }

    #[tokio::test]
    async fn bridge_implements_debug() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let event_bus = rustok_outbox::TransactionalEventBus::new(std::sync::Arc::new(
            rustok_outbox::OutboxTransport::new(db.clone()),
        ));
        let bridge = ServerContentOrchestrationBridge::new(db, event_bus);
        let debug_str = format!("{:?}", bridge);
        assert!(debug_str.contains("ServerContentOrchestrationBridge"));
    }

    #[test]
    fn helper_slug_normalization() {
        assert_eq!(normalize_slug("Hello World"), "hello-world");
        assert_eq!(
            normalize_slug("Special !@# Characters---Here"),
            "special-characters-here"
        );
        assert_eq!(normalize_slug("---already-dashed---"), "already-dashed");
        assert_eq!(normalize_slug(""), "");
    }

    #[test]
    fn helper_locales_from_strs_dedup_and_validate() {
        let locales = vec!["en", "EN", "ru", "fr", "  "];
        let normalized = locales_from_strs(locales).expect("normalization should succeed");
        assert!(normalized.contains(&"en".to_string()));
        assert!(normalized.contains(&"ru".to_string()));
        assert!(normalized.contains(&"fr".to_string()));

        // Empty locales set should return an error
        let empty_locales: Vec<&str> = vec!["", "   "];
        let err = locales_from_strs(empty_locales).expect_err("empty locales must fail");
        assert!(err.to_string().contains("at least one normalized locale"));
    }

    #[test]
    fn merge_unique_source_ids_contract() {
        let target_id = Uuid::new_v4();
        let source_1 = Uuid::new_v4();
        let source_2 = Uuid::new_v4();

        // Valid distinct sources
        let unique = unique_source_ids(target_id, &[source_1, source_2, source_1]).unwrap();
        assert_eq!(unique.len(), 2);
        assert_eq!(unique[0], source_1);
        assert_eq!(unique[1], source_2);

        // Nil source ID rejected
        let nil_err = unique_source_ids(target_id, &[Uuid::nil()]).unwrap_err();
        assert!(nil_err.to_string().contains("must not be nil"));

        // Source including target rejected
        let target_err = unique_source_ids(target_id, &[source_1, target_id]).unwrap_err();
        assert!(
            target_err
                .to_string()
                .contains("cannot include the target topic")
        );

        // Empty sources rejected
        let empty_err = unique_source_ids(target_id, &[]).unwrap_err();
        assert!(
            empty_err
                .to_string()
                .contains("requires at least one source topic")
        );
    }

    #[test]
    fn graphql_input_dtos_and_payload_contracts() {
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let promote_input = PromoteTopicToPostInput {
            topic_id: id1,
            locale: "en".to_string(),
            blog_category_id: Some(id2),
            reason: Some("promotion test".to_string()),
            idempotency_key: "idem-1".to_string(),
        };
        let debug_promote = format!("{:?}", promote_input);
        assert!(debug_promote.contains("PromoteTopicToPostInput"));
        let cloned_promote = promote_input.clone();
        assert_eq!(cloned_promote.topic_id, id1);

        let demote_input = DemotePostToTopicInput {
            post_id: id1,
            locale: "en".to_string(),
            forum_category_id: id2,
            reason: None,
            idempotency_key: "idem-2".to_string(),
        };
        assert_eq!(demote_input.clone().post_id, id1);

        let split_input = SplitTopicInput {
            topic_id: id1,
            locale: "en".to_string(),
            reply_ids: vec![id2],
            new_title: "Split topic".to_string(),
            reason: None,
            idempotency_key: "idem-3".to_string(),
        };
        assert_eq!(split_input.clone().new_title, "Split topic");

        let merge_input = MergeTopicsInput {
            target_topic_id: id1,
            source_topic_ids: vec![id2],
            reason: None,
            idempotency_key: "idem-4".to_string(),
        };
        assert_eq!(merge_input.clone().target_topic_id, id1);

        // Payload conversion from OrchestrationResult
        let orch_result = OrchestrationResult {
            source_id: id1,
            target_id: id2,
            moved_comments: 5,
        };
        let payload: ContentOrchestrationPayload = orch_result.into();
        assert_eq!(payload.source_id, id1);
        assert_eq!(payload.target_id, id2);
        assert_eq!(payload.moved_comments, 5);

        // Mutation default
        let _mutation = ContentOrchestrationMutation;
    }
}
