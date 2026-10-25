use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{
    PortError, SharedStaticModuleSettingsReader, StaticModuleSettingsReader,
    StaticModuleSettingsSnapshot,
};
use rustok_core::ModuleRuntimeExtensions;
use rustok_core::search_projection::SearchProjectionSourceFactory;
use rustok_forum::{
    ForumCategoryAudienceReadService, ForumError, ForumPublicDiscoveryService,
    ForumReplyAudienceReadService, ForumSearchProjectionSourceFactory, ForumSearchResultCandidate,
    ForumSearchResultCandidateKind, ForumSearchResultEligibilityService, ForumSettingsProviders,
    ForumTopicAudienceListService, ForumTopicAudienceReadService, ListTopicsFilter,
};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::Database;
use uuid::Uuid;

struct ClosedForum {
    tenant: Uuid,
}

#[async_trait]
impl StaticModuleSettingsReader for ClosedForum {
    async fn settings(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        assert_eq!(module_slug, "forum");
        Ok(Some(StaticModuleSettingsSnapshot {
            enabled: Some(true),
            settings: serde_json::json!({"allow_anonymous_reading": tenant_id != self.tenant}),
        }))
    }
}

#[tokio::test]
async fn saved_private_tenant_denies_all_public_owner_shapes_before_loading_candidates() {
    // With no forum tables, any attempt to fetch a category, topic, reply or
    // raw Search/SEO candidate instead of checking settings fails this test.
    let db = Database::connect("sqlite::memory:").await.expect("sqlite");
    let tenant = Uuid::new_v4();
    let id = Uuid::new_v4();
    let reader = SharedStaticModuleSettingsReader(Arc::new(ClosedForum { tenant }));
    let settings = ForumSettingsProviders::default().with_static_reader(reader.clone());
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let category =
        ForumCategoryAudienceReadService::new(db.clone()).with_settings_providers(settings.clone());
    assert!(matches!(
        category
            .get_public_storefront_visible_with_locale_fallback(tenant, id, "en", None)
            .await,
        Err(ForumError::CategoryNotFound(_))
    ));
    let categories = category
        .list_public_storefront_visible_with_locale_fallback(tenant, "en", 1, 10, None)
        .await
        .expect("closed category page");
    assert_eq!(categories.total, 0);
    assert!(categories.items.is_empty());
    let tree = category
        .tree_public_storefront_visible_with_locale_fallback(tenant, "en", None)
        .await
        .expect("closed tree");
    assert_eq!(tree.total_nodes, 0);
    assert!(tree.roots.is_empty());

    let topics = ForumTopicAudienceReadService::new(db.clone(), event_bus.clone())
        .with_settings_providers(settings.clone());
    assert!(
        topics
            .get_public_storefront_visible_with_locale_fallback(tenant, id, "en", None, None)
            .await
            .expect("closed topic")
            .is_none()
    );
    let list = ForumTopicAudienceListService::new(db.clone(), event_bus.clone())
        .with_settings_providers(settings.clone())
        .list_public_storefront_visible_with_locale_fallback(
            tenant,
            ListTopicsFilter {
                search: None,
                order: Default::default(),
                solved_only: false,
                category_id: None,
                status: None,
                locale: Some("en".into()),
                after: None,
                per_page: 10,
            },
            None,
            None,
        )
        .await
        .expect("closed topic list");
    assert!(list.items.is_empty());
    assert!(
        list.next_cursor.is_none(),
        "a raw cursor must not identify hidden candidates"
    );

    let replies = ForumReplyAudienceReadService::new(db.clone(), event_bus.clone())
        .with_settings_providers(settings.clone());
    assert!(
        replies
            .get_public_storefront_visible_with_locale_fallback(tenant, id, "en", None, None, None)
            .await
            .expect("closed reply")
            .is_none()
    );
    let discovery = ForumPublicDiscoveryService::new(db.clone(), event_bus)
        .with_settings_providers(settings.clone());
    assert!(!discovery.anonymous_reading_allowed(tenant).await.unwrap());
    assert!(
        discovery
            .get_public_category_with_locale_fallback(tenant, id, "en", None)
            .await
            .expect("closed SEO category")
            .is_none()
    );
    assert!(
        discovery
            .get_public_topic_with_locale_fallback(tenant, id, "en", None, None)
            .await
            .expect("closed SEO topic")
            .is_none()
    );

    // Query-time Search rechecks even documents indexed before the setting was
    // changed: categories, topics and replies are all removed before facets.
    let eligibility =
        ForumSearchResultEligibilityService::new(db.clone()).with_settings_providers(settings);
    let allowed = eligibility
        .filter_public_storefront_visible(
            tenant,
            None,
            &[
                ForumSearchResultCandidate {
                    kind: ForumSearchResultCandidateKind::Category,
                    document_id: id,
                },
                ForumSearchResultCandidate {
                    kind: ForumSearchResultCandidateKind::Topic,
                    document_id: id,
                },
                ForumSearchResultCandidate {
                    kind: ForumSearchResultCandidateKind::Reply,
                    document_id: id,
                },
            ],
        )
        .await
        .expect("closed search eligibility");
    assert!(allowed.is_empty());

    let mut extensions = ModuleRuntimeExtensions::default();
    extensions.insert(reader);
    let projection = ForumSearchProjectionSourceFactory.build_with_extensions(db, &extensions);
    let page = projection
        .list_public_documents(tenant, None, 10)
        .await
        .expect("closed projection");
    assert!(page.documents.is_empty());
    assert!(page.next_cursor.is_none());
    assert!(
        projection
            .load_public_entity(tenant, "forum_topic", id)
            .await
            .expect("closed entity")
            .is_empty()
    );
}
