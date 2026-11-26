//! Inherited create eligibility with Forum-only storage and a selected fake provider.
use super::topic_reads::insert_topic_fixture;
use super::*;
use rustok_forum::{
    ForumAudienceConstraints, ForumCategoryReplyCreateAudiencePolicyService,
    ForumCategoryTopicCreateAudiencePolicyService, ForumReplyCreateAudienceAuthorizationService,
    ForumTopicCreateAudienceAuthorizationService, ForumTopicReplyCreateAudiencePolicyService,
    SetForumCategoryReplyCreateAudiencePolicyInput, SetForumCategoryTopicCreateAudiencePolicyInput,
    SetForumTopicReplyCreateAudiencePolicyInput,
};
use sea_orm::{EntityTrait, PaginatorTrait};

mod topic_fields {
    rustok_core::define_field_definitions_entity!("topic_field_definitions");
}

async fn create_fixture(port: &TestTaxonomy) -> (DatabaseConnection, Uuid) {
    let (db, _) = fixture(port, true).await;
    let schema = DatabaseSchema::new(DatabaseBackend::Sqlite);
    macro_rules! table {
        ($module:ident) => {
            db.execute_raw(
                DatabaseBackend::Sqlite.build(&schema.create_table_from_entity($module::Entity)),
            )
            .await
            .unwrap();
        };
    }
    table!(forum_category_topic_create_audience_policy);
    table!(forum_category_topic_create_audience_role);
    table!(forum_category_topic_create_audience_channel);
    table!(forum_category_topic_create_audience_group);
    table!(forum_category_topic_create_audience_user);
    table!(forum_category_reply_create_audience_policy);
    table!(forum_category_reply_create_audience_role);
    table!(forum_category_reply_create_audience_channel);
    table!(forum_category_reply_create_audience_group);
    table!(forum_category_reply_create_audience_user);
    table!(forum_topic_reply_create_audience_policy);
    table!(forum_topic_reply_create_audience_role);
    table!(forum_topic_reply_create_audience_channel);
    table!(forum_topic_reply_create_audience_group);
    table!(forum_topic_reply_create_audience_user);
    table!(topic_fields);
    let topic = insert_topic_fixture(&db, port).await;
    (db, topic)
}

fn constraints(user: Uuid) -> ForumAudienceConstraints {
    ForumAudienceConstraints {
        allow_user_ids: vec![user],
        ..Default::default()
    }
}

async fn restrict_parent(db: &DatabaseConnection, port: &TestTaxonomy, user: Uuid) {
    let manager = rustok_core::SecurityContext::system();
    ForumCategoryTopicCreateAudiencePolicyService::new(db.clone())
        .with_taxonomy_port(Arc::new(port.clone()))
        .set(
            port.tenant,
            port.parent,
            manager.clone(),
            SetForumCategoryTopicCreateAudiencePolicyInput {
                constraints: constraints(user),
            },
        )
        .await
        .unwrap();
    ForumCategoryReplyCreateAudiencePolicyService::new(db.clone())
        .with_taxonomy_port(Arc::new(port.clone()))
        .set(
            port.tenant,
            port.parent,
            manager,
            SetForumCategoryReplyCreateAudiencePolicyInput {
                constraints: constraints(user),
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn create_policy_owners_keep_transaction_and_clearing_local_layer_keeps_inheritance() {
    let mut port = TestTaxonomy::new();
    port.require_transaction = true;
    let (db, topic) = create_fixture(&port).await;
    let allowed = Uuid::new_v4();
    restrict_parent(&db, &port, allowed).await;
    let manager = rustok_core::SecurityContext::system();
    let topics = ForumCategoryTopicCreateAudiencePolicyService::new(db.clone())
        .with_taxonomy_port(Arc::new(port.clone()));
    let replies = ForumCategoryReplyCreateAudiencePolicyService::new(db.clone())
        .with_taxonomy_port(Arc::new(port.clone()));
    let local = ForumTopicReplyCreateAudiencePolicyService::new(db.clone())
        .with_taxonomy_port(Arc::new(port.clone()));
    assert_eq!(
        topics
            .get(port.tenant, port.category, manager.clone())
            .await
            .unwrap()
            .effective_layers[0]
            .category_id,
        port.parent
    );
    assert_eq!(
        replies
            .get(port.tenant, port.category, manager.clone())
            .await
            .unwrap()
            .effective_layers[0]
            .category_id,
        port.parent
    );
    let result = local
        .set(
            port.tenant,
            topic,
            manager.clone(),
            SetForumTopicReplyCreateAudiencePolicyInput {
                constraints: constraints(allowed),
            },
        )
        .await
        .unwrap();
    assert_eq!(result.inherited_category_layers[0].category_id, port.parent);
    assert!(result.configured_constraints.is_some());
    let result = local
        .set(
            port.tenant,
            topic,
            manager.clone(),
            SetForumTopicReplyCreateAudiencePolicyInput {
                constraints: Default::default(),
            },
        )
        .await
        .unwrap();
    assert!(result.configured_constraints.is_none());
    assert_eq!(
        result.inherited_category_layers[0]
            .constraints
            .allow_user_ids,
        vec![allowed]
    );
    assert_eq!(
        local.get(port.tenant, topic, manager).await.unwrap(),
        result
    );
}

#[tokio::test]
async fn create_authorization_requires_ancestor_despite_local_allow_and_recovers_when_ancestor_allows()
 {
    let port = TestTaxonomy::new();
    let (db, topic) = create_fixture(&port).await;
    let user = Uuid::new_v4();
    restrict_parent(&db, &port, Uuid::new_v4()).await;
    ForumTopicReplyCreateAudiencePolicyService::new(db.clone())
        .with_taxonomy_port(Arc::new(port.clone()))
        .set(
            port.tenant,
            topic,
            rustok_core::SecurityContext::system(),
            SetForumTopicReplyCreateAudiencePolicyInput {
                constraints: constraints(user),
            },
        )
        .await
        .unwrap();
    let security = rustok_core::SecurityContext::from_permission_snapshot(
        Some(user),
        &[
            Permission::FORUM_TOPICS_CREATE,
            Permission::FORUM_REPLIES_CREATE,
        ],
    );
    let topics = ForumTopicCreateAudienceAuthorizationService::new(db.clone(), None)
        .with_taxonomy_port(Arc::new(port.clone()));
    let replies = ForumReplyCreateAudienceAuthorizationService::new(db.clone(), None)
        .with_taxonomy_port(Arc::new(port.clone()));
    let denied = topics
        .evaluate(port.tenant, port.category, &security, None)
        .await
        .unwrap();
    assert!(!denied.allowed);
    assert_eq!(denied.denied_by_category_id, Some(port.parent));
    let denied = replies
        .evaluate(port.tenant, topic, &security, None)
        .await
        .unwrap();
    assert!(!denied.allowed);
    assert_eq!(denied.denied_by_category_id, Some(port.parent));
    assert!(matches!(
        topics
            .require(port.tenant, port.category, &security, None)
            .await,
        Err(ForumError::Forbidden(_))
    ));
    assert!(matches!(
        replies.require(port.tenant, topic, &security, None).await,
        Err(ForumError::Forbidden(_))
    ));
    restrict_parent(&db, &port, user).await;
    assert!(
        topics
            .require(port.tenant, port.category, &security, None)
            .await
            .unwrap()
            .allowed
    );
    assert_eq!(
        replies
            .require(port.tenant, topic, &security, None)
            .await
            .unwrap()
            .evaluated_layers,
        2
    );
}

#[tokio::test]
async fn create_commands_with_and_without_context_retain_selected_transaction_before_writes() {
    let mut port = TestTaxonomy::new();
    port.require_transaction = true;
    let (db, topic) = create_fixture(&port).await;
    restrict_parent(&db, &port, Uuid::new_v4()).await;
    let user = Uuid::new_v4();
    let security = rustok_core::SecurityContext::from_permission_snapshot(
        Some(user),
        &[
            Permission::FORUM_TOPICS_CREATE,
            Permission::FORUM_REPLIES_CREATE,
        ],
    );
    let context = rustok_api::PortContext::new(
        port.tenant.to_string(),
        rustok_api::PortActor::user(user.to_string()),
        "en",
        "create-denied",
    );
    let bus = rustok_outbox::TransactionalEventBus::new(Arc::new(
        rustok_outbox::OutboxTransport::new(db.clone()),
    ));
    let topics = rustok_forum::TopicService::new(db.clone(), bus.clone())
        .with_taxonomy_port(Arc::new(port.clone()));
    let replies =
        rustok_forum::ReplyService::new(db.clone(), bus).with_taxonomy_port(Arc::new(port.clone()));
    let topic_input = || rustok_forum::CreateTopicInput {
        locale: "en".into(),
        category_id: port.category,
        title: "Inherited creation restriction".into(),
        slug: None,
        body: rustok_api::RichTextDocument::single_paragraph("Creation audience regression body"),
        metadata: serde_json::json!({}),
        tags: vec![],
        channel_slugs: None,
    };
    let reply_input = || rustok_forum::CreateReplyInput {
        locale: "en".into(),
        content: rustok_api::RichTextDocument::single_paragraph(
            "Creation audience regression reply",
        ),
        parent_reply_id: None,
    };
    assert!(
        matches!(topics.create(port.tenant, security.clone(), topic_input()).await, Err(ForumError::Forbidden(message)) if message == "Forum topic creation is unavailable for the current audience")
    );
    assert!(
        matches!(topics.create_with_audience_context(port.tenant, security.clone(), context.clone(), topic_input()).await, Err(ForumError::Forbidden(message)) if message == "Forum topic creation is unavailable for the current audience")
    );
    assert!(
        matches!(replies.create(port.tenant, security.clone(), topic, reply_input()).await, Err(ForumError::Forbidden(message)) if message == "Forum reply creation is unavailable for the current audience")
    );
    assert!(
        matches!(replies.create_with_audience_context(port.tenant, security, topic, context, reply_input()).await, Err(ForumError::Forbidden(message)) if message == "Forum reply creation is unavailable for the current audience")
    );
    assert_eq!(forum_topic::Entity::find().count(&db).await.unwrap(), 1);
    assert_eq!(forum_reply::Entity::find().count(&db).await.unwrap(), 0);
    // No content/counter journal/outbox tables exist. The exact audience error
    // proves the selected denial precedes their writes rather than failing SQL later.
}

#[tokio::test]
async fn selected_create_provider_failure_is_not_unrestricted_and_owner_replace_rolls_back() {
    let mut port = TestTaxonomy::new();
    let (db, topic) = create_fixture(&port).await;
    let allowed = Uuid::new_v4();
    restrict_parent(&db, &port, allowed).await;
    port.calls.lock().unwrap().clear();
    port.fail_at = Some("parents");
    let security = rustok_core::SecurityContext::from_permission_snapshot(
        Some(allowed),
        &[
            Permission::FORUM_TOPICS_CREATE,
            Permission::FORUM_REPLIES_CREATE,
        ],
    );
    let topics = ForumTopicCreateAudienceAuthorizationService::new(db.clone(), None)
        .with_taxonomy_port(Arc::new(port.clone()));
    assert!(matches!(
        topics
            .evaluate(port.tenant, port.category, &security, None)
            .await,
        Err(ForumError::CapabilityFailure { .. })
    ));
    port.calls.lock().unwrap().clear();
    let replies = ForumReplyCreateAudienceAuthorizationService::new(db.clone(), None)
        .with_taxonomy_port(Arc::new(port.clone()));
    assert!(matches!(
        replies.evaluate(port.tenant, topic, &security, None).await,
        Err(ForumError::CapabilityFailure { .. })
    ));
    port.calls.lock().unwrap().clear();
    port.require_transaction = true;
    port.fail_on_call = 2; // Fail the post-write effective-policy reload, not admission.
    let owner = ForumCategoryTopicCreateAudiencePolicyService::new(db.clone())
        .with_taxonomy_port(Arc::new(port.clone()));
    assert!(matches!(
        owner
            .set(
                port.tenant,
                port.parent,
                rustok_core::SecurityContext::system(),
                SetForumCategoryTopicCreateAudiencePolicyInput {
                    constraints: Default::default()
                }
            )
            .await,
        Err(ForumError::CapabilityFailure { .. })
    ));
    port.fail_at = None;
    let intact = ForumCategoryTopicCreateAudiencePolicyService::new(db)
        .with_taxonomy_port(Arc::new(port.clone()))
        .get(
            port.tenant,
            port.parent,
            rustok_core::SecurityContext::system(),
        )
        .await
        .unwrap();
    assert_eq!(
        intact.configured_constraints.unwrap().allow_user_ids,
        vec![allowed]
    );
}
