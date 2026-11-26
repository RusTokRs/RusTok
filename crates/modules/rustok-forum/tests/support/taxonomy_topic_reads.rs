//! Topic/reply inherited-read regressions sharing the Forum-only provider fixture.
use super::*;

pub(super) async fn insert_topic_fixture(db: &DatabaseConnection, port: &TestTaxonomy) -> Uuid {
    let topic = Uuid::new_v4();
    forum_topic::ActiveModel {
        id: Set(topic),
        tenant_id: Set(port.tenant),
        category_id: Set(port.category),
        author_id: Set(None),
        status: Set(rustok_forum::TopicStatus::Open),
        metadata: Set(serde_json::json!({})),
        is_pinned: Set(false),
        is_locked: Set(false),
        reply_count: Set(0),
        last_reply_at: Set(None),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    }
    .insert(db)
    .await
    .unwrap();
    topic
}

fn public_read_settings() -> ForumSettingsProviders {
    ForumSettingsProviders::default()
        .with_static_reader(SharedStaticModuleSettingsReader(Arc::new(OpenForum)))
}

fn read_event_bus(db: &DatabaseConnection) -> rustok_outbox::TransactionalEventBus {
    rustok_outbox::TransactionalEventBus::new(Arc::new(rustok_outbox::OutboxTransport::new(
        db.clone(),
    )))
}

#[tokio::test]
async fn inherited_base_floor_hides_descendant_topic_across_read_and_discovery_owners() {
    use rustok_forum::{
        ForumTopicAudienceViewer, ForumTopicAudienceVisibilityService, ForumTopicVisibilityScope,
        ForumTopicVisibilityService,
    };
    let port = TestTaxonomy::new();
    let (db, schema) = fixture(&port, true).await;
    let topic = insert_topic_fixture(&db, &port).await;
    let visibility = ForumTopicAudienceVisibilityService::new(db.clone(), None)
        .with_taxonomy_port(Arc::new(port.clone()))
        .with_settings_providers(public_read_settings());
    assert!(
        visibility
            .is_topic_visible(
                port.tenant,
                topic,
                None,
                &ForumTopicAudienceViewer::public()
            )
            .await
            .unwrap()
    );
    forum_category_policy::ActiveModel {
        category_id: Set(port.parent),
        tenant_id: Set(port.tenant),
        allows_topics: Set(true),
        visibility_override: Set(Some(rustok_forum::ForumCategoryVisibility::Authenticated)),
        updated_at: Set(chrono::Utc::now().into()),
    }
    .insert(&db)
    .await
    .unwrap();
    assert!(
        !visibility
            .is_topic_visible(
                port.tenant,
                topic,
                None,
                &ForumTopicAudienceViewer::public()
            )
            .await
            .unwrap()
    );
    assert!(
        !visibility
            .is_topic_owner_visible(port.tenant, topic, &ForumTopicAudienceViewer::public())
            .await
            .unwrap()
    );
    let base =
        ForumTopicVisibilityService::new(db.clone()).with_taxonomy_port(Arc::new(port.clone()));
    assert!(
        base.filter_visible_topic_ids(
            port.tenant,
            &[topic, topic],
            &ForumTopicVisibilityScope::storefront(None).unwrap()
        )
        .await
        .unwrap()
        .is_empty()
    );
    // No translation/body/revision tables exist: denial must happen before any content hydration.
    for field in [
        "forumStorefrontAudienceTopic",
        "forumStorefrontTopicCurrentRevision",
        "forumStorefrontTopicQuoteRevision",
    ] {
        let selection = if field == "forumStorefrontAudienceTopic" {
            " { id }"
        } else {
            ""
        };
        let response = schema
            .execute(format!(
                "{{ result: {field}(id: \"{topic}\", locale: \"en\"){selection} }}"
            ))
            .await;
        assert!(response.errors.is_empty(), "{field}: {:?}", response.errors);
        assert_eq!(
            response.data.into_json().unwrap(),
            serde_json::json!({"result": null})
        );
    }
    let discovery = rustok_forum::ForumPublicDiscoveryService::new(db.clone(), read_event_bus(&db))
        .with_taxonomy_port(Arc::new(port.clone()))
        .with_settings_providers(public_read_settings());
    assert!(
        discovery
            .get_public_topic_with_locale_fallback(port.tenant, topic, "en", None, None)
            .await
            .unwrap()
            .is_none()
    );
    let search = rustok_forum::ForumSearchResultEligibilityService::new(db.clone())
        .with_taxonomy_port(Arc::new(port.clone()))
        .with_settings_providers(public_read_settings());
    assert!(
        search
            .filter_public_storefront_visible(
                port.tenant,
                None,
                &[rustok_forum::ForumSearchResultCandidate {
                    document_id: topic,
                    kind: rustok_forum::ForumSearchResultCandidateKind::Topic,
                }]
            )
            .await
            .unwrap()
            .is_empty()
    );
    let reply = Uuid::new_v4();
    forum_reply::ActiveModel {
        id: Set(reply),
        tenant_id: Set(port.tenant),
        topic_id: Set(topic),
        author_id: Set(None),
        parent_reply_id: Set(None),
        status: Set(rustok_forum::ReplyStatus::Approved),
        position: Set(1),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    }
    .insert(&db)
    .await
    .unwrap();
    assert!(
        discovery
            .get_public_reply_with_locale_fallback(port.tenant, reply, "en", None, None, None)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn inherited_richer_topic_layer_is_required_even_when_topic_local_layer_allows() {
    use rustok_forum::{ForumTopicAudienceViewer, ForumTopicAudienceVisibilityService};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let port = TestTaxonomy::new();
    let (db, schema) = fixture(&port, true).await;
    let topic = insert_topic_fixture(&db, &port).await;
    let user = Uuid::new_v4();
    forum_category_audience_policy::ActiveModel {
        tenant_id: Set(port.tenant),
        category_id: Set(port.parent),
        minimum_trust_level: Set(None),
        updated_at: Set(chrono::Utc::now().into()),
    }
    .insert(&db)
    .await
    .unwrap();
    forum_category_audience_user::ActiveModel {
        tenant_id: Set(port.tenant),
        category_id: Set(port.parent),
        user_id: Set(Uuid::new_v4()),
        effect: Set(forum_category_audience_user::ForumCategoryAudienceUserEffect::Allow),
    }
    .insert(&db)
    .await
    .unwrap();
    forum_topic_audience_policy::ActiveModel {
        tenant_id: Set(port.tenant),
        topic_id: Set(topic),
        minimum_trust_level: Set(None),
        updated_at: Set(chrono::Utc::now().into()),
    }
    .insert(&db)
    .await
    .unwrap();
    forum_topic_audience_user::ActiveModel {
        tenant_id: Set(port.tenant),
        topic_id: Set(topic),
        user_id: Set(user),
        effect: Set(forum_category_audience_user::ForumCategoryAudienceUserEffect::Allow),
    }
    .insert(&db)
    .await
    .unwrap();
    let viewer = ForumTopicAudienceViewer::authenticated(
        rustok_core::SecurityContext::from_permission_snapshot(
            Some(user),
            &[Permission::FORUM_TOPICS_READ],
        ),
        rustok_api::PortContext::new(
            port.tenant.to_string(),
            rustok_api::PortActor::user(user.to_string()),
            "en",
            "inherited-topic",
        ),
    )
    .unwrap();
    let visibility = ForumTopicAudienceVisibilityService::new(db.clone(), None)
        .with_taxonomy_port(Arc::new(port.clone()))
        .with_settings_providers(public_read_settings());
    assert!(
        !visibility
            .is_topic_owner_visible(port.tenant, topic, &viewer)
            .await
            .unwrap()
    );
    assert!(
        !visibility
            .is_topic_visible(port.tenant, topic, None, &viewer)
            .await
            .unwrap()
    );
    let response = schema
        .execute(format!(
            "{{ result: forumStorefrontAudienceTopic(id: \"{topic}\", locale: \"en\") {{ id }} }}"
        ))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap(),
        serde_json::json!({"result": null})
    );
    forum_category_audience_user::Entity::delete_many()
        .filter(forum_category_audience_user::Column::CategoryId.eq(port.parent))
        .exec(&db)
        .await
        .unwrap();
    forum_category_audience_user::ActiveModel {
        tenant_id: Set(port.tenant),
        category_id: Set(port.parent),
        user_id: Set(user),
        effect: Set(forum_category_audience_user::ForumCategoryAudienceUserEffect::Allow),
    }
    .insert(&db)
    .await
    .unwrap();
    assert!(
        visibility
            .is_topic_owner_visible(port.tenant, topic, &viewer)
            .await
            .unwrap()
    );
    assert!(
        visibility
            .is_topic_visible(port.tenant, topic, None, &viewer)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn topic_provider_failure_is_not_a_successful_absence_or_public_list() {
    use rustok_forum::{ForumTopicAudienceViewer, ForumTopicAudienceVisibilityService};
    let mut port = TestTaxonomy::new();
    port.fail_at = Some("parents");
    let (db, schema) = fixture(&port, true).await;
    let topic = insert_topic_fixture(&db, &port).await;
    let response = schema
        .execute(format!(
            "{{ result: forumStorefrontAudienceTopic(id: \"{topic}\", locale: \"en\") {{ id }} }}"
        ))
        .await;
    assert_eq!(response.errors.len(), 1);
    assert!(matches!(
        response.errors[0].source::<ForumError>(),
        Some(ForumError::CapabilityFailure { .. })
    ));
    // Reset the fake's nth-failure counter for the next independent entry point.
    port.calls.lock().unwrap().clear();
    let list = rustok_forum::ForumTopicAudienceListService::new(db.clone(), read_event_bus(&db))
        .with_taxonomy_port(Arc::new(port.clone()))
        .with_settings_providers(public_read_settings());
    assert!(matches!(
        list.list_public_storefront_visible_with_locale_fallback(
            port.tenant,
            Default::default(),
            None,
            None
        )
        .await,
        Err(ForumError::CapabilityFailure { .. })
    ));
    port.calls.lock().unwrap().clear();
    let viewer = ForumTopicAudienceViewer::authenticated(
        rustok_core::SecurityContext::from_permission_snapshot(
            Some(port.category),
            &[Permission::FORUM_TOPICS_READ],
        ),
        rustok_api::PortContext::new(
            port.tenant.to_string(),
            rustok_api::PortActor::user(port.category.to_string()),
            "en",
            "provider-failure",
        ),
    )
    .unwrap();
    let visibility = ForumTopicAudienceVisibilityService::new(db, None)
        .with_taxonomy_port(Arc::new(port.clone()));
    assert!(matches!(
        visibility
            .is_topic_owner_visible(port.tenant, topic, &viewer)
            .await,
        Err(ForumError::CapabilityFailure { .. })
    ));
}

#[tokio::test]
async fn topic_policy_read_retains_transaction_for_inherited_provider_projection() {
    let mut port = TestTaxonomy::new();
    port.require_transaction = true;
    let (db, _) = fixture(&port, true).await;
    let topic = insert_topic_fixture(&db, &port).await;
    forum_category_audience_policy::ActiveModel {
        tenant_id: Set(port.tenant),
        category_id: Set(port.parent),
        minimum_trust_level: Set(Some(1)),
        updated_at: Set(chrono::Utc::now().into()),
    }
    .insert(&db)
    .await
    .unwrap();
    let reader = rustok_forum::ForumTopicAudiencePolicyService::new(db.clone())
        .with_taxonomy_port(Arc::new(port.clone()));
    let result = reader
        .get(port.tenant, topic, rustok_core::SecurityContext::system())
        .await
        .unwrap();
    assert_eq!(result.inherited_category_layers.len(), 1);
    assert_eq!(result.inherited_category_layers[0].category_id, port.parent);
    assert_eq!(port.calls.lock().unwrap().as_slice(), &["parents"]);
    port.calls.lock().unwrap().clear();
    port.fail_at = Some("parents");
    let failing = rustok_forum::ForumTopicAudiencePolicyService::new(db)
        .with_taxonomy_port(Arc::new(port.clone()));
    assert!(matches!(
        failing
            .get(port.tenant, topic, rustok_core::SecurityContext::system())
            .await,
        Err(ForumError::CapabilityFailure { .. })
    ));
}

#[tokio::test]
async fn engagement_writes_keep_provider_transaction_and_refuse_inherited_denial_before_mutation() {
    let port = TestTaxonomy::new();
    let (db, _) = fixture(&port, true).await;
    let topic = insert_topic_fixture(&db, &port).await;
    let user = Uuid::new_v4();
    let security = rustok_core::SecurityContext::from_permission_snapshot(
        Some(user),
        &[
            Permission::FORUM_TOPICS_READ,
            Permission::FORUM_REPLIES_READ,
        ],
    );
    let context = rustok_api::PortContext::new(
        port.tenant.to_string(),
        rustok_api::PortActor::user(user.to_string()),
        "en",
        "inherited-write-denial",
    );
    forum_category_audience_policy::ActiveModel {
        tenant_id: Set(port.tenant),
        category_id: Set(port.parent),
        minimum_trust_level: Set(None),
        updated_at: Set(chrono::Utc::now().into()),
    }
    .insert(&db)
    .await
    .unwrap();
    forum_category_audience_user::ActiveModel {
        tenant_id: Set(port.tenant),
        category_id: Set(port.parent),
        user_id: Set(Uuid::new_v4()),
        effect: Set(forum_category_audience_user::ForumCategoryAudienceUserEffect::Allow),
    }
    .insert(&db)
    .await
    .unwrap();
    let reply = Uuid::new_v4();
    forum_reply::ActiveModel {
        id: Set(reply),
        tenant_id: Set(port.tenant),
        topic_id: Set(topic),
        author_id: Set(None),
        parent_reply_id: Set(None),
        status: Set(rustok_forum::ReplyStatus::Approved),
        position: Set(1),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    }
    .insert(&db)
    .await
    .unwrap();
    let subscriptions = rustok_forum::SubscriptionService::new(db.clone())
        .with_taxonomy_port(Arc::new(port.clone()));
    assert!(
        matches!(subscriptions.get_topic_subscription(port.tenant, topic, security.clone(), context.clone()).await, Err(ForumError::TopicNotFound(id)) if id == topic)
    );
    let reports = rustok_forum::ForumModerationReportService::new(db.clone(), None, None)
        .with_taxonomy_port(Arc::new(port.clone()));
    assert!(
        matches!(reports.report_topic(port.tenant, topic, security.clone(), context.clone(), rustok_moderation::ModerationReasonCode::Spam).await, Err(ForumError::TopicNotFound(id)) if id == topic)
    );
    assert!(
        matches!(reports.report_reply(port.tenant, reply, security.clone(), context.clone(), rustok_moderation::ModerationReasonCode::Spam).await, Err(ForumError::ReplyNotFound(id)) if id == reply)
    );
    let read_state = rustok_forum::ForumVisibilityScopedReadStateService::new(db.clone())
        .with_taxonomy_port(Arc::new(port.clone()));
    assert!(
        matches!(read_state.get_topic_read_state_with_audience_context(port.tenant, topic, security.clone(), context.clone()).await, Err(ForumError::TopicNotFound(id)) if id == topic)
    );

    let mut transactional = port.clone();
    transactional.require_transaction = true;
    let votes = rustok_forum::VoteService::new(db.clone())
        .with_taxonomy_port(Arc::new(transactional.clone()));
    assert!(
        matches!(votes.set_topic_vote(port.tenant, topic, security.clone(), context.clone(), 1).await, Err(ForumError::TopicNotFound(id)) if id == topic)
    );
    assert!(
        matches!(votes.set_reply_vote(port.tenant, reply, security.clone(), context.clone(), 1).await, Err(ForumError::ReplyNotFound(id)) if id == reply)
    );
    let subscriptions = rustok_forum::SubscriptionService::new(db.clone())
        .with_taxonomy_port(Arc::new(transactional.clone()));
    assert!(
        matches!(subscriptions.set_topic_subscription(port.tenant, topic, security.clone(), context.clone()).await, Err(ForumError::TopicNotFound(id)) if id == topic)
    );
    // This fixture intentionally has no vote/topic-subscription/read-state tables:
    // any mutation or downstream settings hydration would produce Database,
    // not the precise denied-target class asserted above.
    transactional.calls.lock().unwrap().clear();
    transactional.fail_at = Some("parents");
    let failing = rustok_forum::VoteService::new(db).with_taxonomy_port(Arc::new(transactional));
    assert!(matches!(
        failing
            .set_topic_vote(port.tenant, topic, security, context, 1)
            .await,
        Err(ForumError::CapabilityFailure { .. })
    ));
}
