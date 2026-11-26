//! Inherited moderation/category policy ownership without Taxonomy storage.
use super::topic_reads::insert_topic_fixture;
use super::*;
use rustok_forum::{
    ForumAudienceConstraints, ForumCategoryAudiencePolicyService,
    ForumCategoryModerationAudiencePolicyService, ForumModerationAudienceAuthorizationService,
    ModerationService, SetForumCategoryAudiencePolicyInput,
    SetForumCategoryModerationAudiencePolicyInput,
};
use sea_orm::{EntityTrait, TransactionTrait};

async fn moderation_fixture(port: &TestTaxonomy) -> (DatabaseConnection, Uuid, Uuid) {
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
    table!(forum_category_moderation_audience_policy);
    table!(forum_category_moderation_audience_role);
    table!(forum_category_moderation_audience_channel);
    table!(forum_category_moderation_audience_group);
    table!(forum_category_moderation_audience_user);
    let topic = insert_topic_fixture(&db, port).await;
    let reply = Uuid::new_v4();
    forum_reply::ActiveModel {
        id: Set(reply),
        tenant_id: Set(port.tenant),
        topic_id: Set(topic),
        author_id: Set(None),
        parent_reply_id: Set(None),
        status: Set(rustok_forum::ReplyStatus::Pending),
        position: Set(1),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    }
    .insert(&db)
    .await
    .unwrap();
    (db, topic, reply)
}

fn constraints(user: Uuid) -> ForumAudienceConstraints {
    ForumAudienceConstraints {
        allow_user_ids: vec![user],
        ..Default::default()
    }
}

fn security(user: Uuid) -> rustok_core::SecurityContext {
    rustok_core::SecurityContext::from_permission_snapshot(
        Some(user),
        &[
            Permission::FORUM_TOPICS_MODERATE,
            Permission::FORUM_REPLIES_MODERATE,
            Permission::FORUM_TOPICS_MANAGE,
        ],
    )
}

fn context(port: &TestTaxonomy, user: Uuid) -> rustok_api::PortContext {
    rustok_api::PortContext::new(
        port.tenant.to_string(),
        rustok_api::PortActor::user(user.to_string()),
        "en",
        "inherited-moderation",
    )
}

async fn set_moderator(db: &DatabaseConnection, port: &TestTaxonomy, category: Uuid, user: Uuid) {
    ForumCategoryModerationAudiencePolicyService::new(db.clone())
        .with_taxonomy_port(Arc::new(port.clone()))
        .set(
            port.tenant,
            category,
            rustok_core::SecurityContext::system(),
            SetForumCategoryModerationAudiencePolicyInput {
                constraints: constraints(user),
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn category_policy_management_retains_selected_transaction_and_inheritance_after_local_clear()
{
    let mut port = TestTaxonomy::new();
    port.require_transaction = true;
    let (db, _, _) = moderation_fixture(&port).await;
    let owner = ForumCategoryAudiencePolicyService::new(db.clone())
        .with_taxonomy_port(Arc::new(port.clone()));
    let manager = rustok_core::SecurityContext::system();
    let parent_user = Uuid::new_v4();
    owner
        .set(
            port.tenant,
            port.parent,
            manager.clone(),
            SetForumCategoryAudiencePolicyInput {
                constraints: constraints(parent_user),
            },
        )
        .await
        .unwrap();
    let narrowed = owner
        .set(
            port.tenant,
            port.category,
            manager.clone(),
            SetForumCategoryAudiencePolicyInput {
                constraints: constraints(Uuid::new_v4()),
            },
        )
        .await
        .unwrap();
    assert_eq!(narrowed.effective_layers.len(), 2);
    assert_eq!(narrowed.effective_layers[0].category_id, port.parent);
    let cleared = owner
        .set(
            port.tenant,
            port.category,
            manager.clone(),
            SetForumCategoryAudiencePolicyInput {
                constraints: Default::default(),
            },
        )
        .await
        .unwrap();
    assert!(cleared.configured_constraints.is_none());
    assert_eq!(cleared.effective_layers.len(), 1);
    assert_eq!(
        cleared.effective_layers[0].constraints.allow_user_ids,
        vec![parent_user]
    );
    assert_eq!(
        owner
            .get(port.tenant, port.category, manager.clone())
            .await
            .unwrap(),
        cleared
    );

    set_moderator(&db, &port, port.parent, parent_user).await;
    let moderators = ForumCategoryModerationAudiencePolicyService::new(db)
        .with_taxonomy_port(Arc::new(port.clone()));
    let narrowed = moderators
        .set(
            port.tenant,
            port.category,
            manager.clone(),
            SetForumCategoryModerationAudiencePolicyInput {
                constraints: constraints(Uuid::new_v4()),
            },
        )
        .await
        .unwrap();
    assert!(narrowed.configured_constraints.is_some());
    assert_eq!(narrowed.effective_layers.len(), 2);
    let cleared = moderators
        .set(
            port.tenant,
            port.category,
            manager.clone(),
            SetForumCategoryModerationAudiencePolicyInput {
                constraints: Default::default(),
            },
        )
        .await
        .unwrap();
    assert!(cleared.configured_constraints.is_none());
    assert_eq!(cleared.effective_layers.len(), 1);
    assert_eq!(cleared.effective_layers[0].category_id, port.parent);
    assert_eq!(
        moderators
            .get(port.tenant, port.category, manager)
            .await
            .unwrap(),
        cleared
    );
}

#[tokio::test]
async fn category_policy_replacement_rolls_back_when_selected_provider_reload_fails() {
    let mut port = TestTaxonomy::new();
    port.require_transaction = true;
    let (db, _, _) = moderation_fixture(&port).await;
    let manager = rustok_core::SecurityContext::system();
    let user = Uuid::new_v4();
    ForumCategoryAudiencePolicyService::new(db.clone())
        .with_taxonomy_port(Arc::new(port.clone()))
        .set(
            port.tenant,
            port.category,
            manager.clone(),
            SetForumCategoryAudiencePolicyInput {
                constraints: constraints(user),
            },
        )
        .await
        .unwrap();
    set_moderator(&db, &port, port.category, user).await;
    port.calls.lock().unwrap().clear();
    port.fail_at = Some("parents");
    port.fail_on_call = 2; // First hierarchy check succeeds; fail after normalized replacement.
    let failing = ForumCategoryAudiencePolicyService::new(db.clone())
        .with_taxonomy_port(Arc::new(port.clone()));
    assert!(matches!(
        failing
            .set(
                port.tenant,
                port.category,
                manager.clone(),
                SetForumCategoryAudiencePolicyInput {
                    constraints: Default::default()
                }
            )
            .await,
        Err(ForumError::CapabilityFailure { .. })
    ));
    port.calls.lock().unwrap().clear();
    let failing = ForumCategoryModerationAudiencePolicyService::new(db.clone())
        .with_taxonomy_port(Arc::new(port.clone()));
    assert!(matches!(
        failing
            .set(
                port.tenant,
                port.category,
                manager.clone(),
                SetForumCategoryModerationAudiencePolicyInput {
                    constraints: Default::default()
                }
            )
            .await,
        Err(ForumError::CapabilityFailure { .. })
    ));
    port.fail_at = None;
    let intact = ForumCategoryAudiencePolicyService::new(db.clone())
        .with_taxonomy_port(Arc::new(port.clone()))
        .get(port.tenant, port.category, manager.clone())
        .await
        .unwrap();
    assert_eq!(
        intact.configured_constraints.unwrap().allow_user_ids,
        vec![user]
    );
    let intact = ForumCategoryModerationAudiencePolicyService::new(db)
        .with_taxonomy_port(Arc::new(port.clone()))
        .get(port.tenant, port.category, manager)
        .await
        .unwrap();
    assert_eq!(
        intact.configured_constraints.unwrap().allow_user_ids,
        vec![user]
    );
}

#[tokio::test]
async fn moderation_local_allow_cannot_broaden_ancestor_and_positive_authorization_recovers() {
    let port = TestTaxonomy::new();
    let (db, topic, reply) = moderation_fixture(&port).await;
    let user = Uuid::new_v4();
    set_moderator(&db, &port, port.parent, Uuid::new_v4()).await;
    set_moderator(&db, &port, port.category, user).await;
    let owner = ForumModerationAudienceAuthorizationService::new(db.clone(), None)
        .with_taxonomy_port(Arc::new(port.clone()));
    let denied = owner
        .evaluate_topic(port.tenant, topic, &security(user), None)
        .await
        .unwrap();
    assert!(!denied.allowed);
    assert_eq!(denied.denied_by_category_id, Some(port.parent));
    let denied = owner
        .evaluate_reply(
            port.tenant,
            reply,
            topic,
            &security(user),
            Some(context(&port, user)),
        )
        .await
        .unwrap();
    assert!(!denied.allowed);
    assert_eq!(denied.denied_by_category_id, Some(port.parent));
    assert!(matches!(
        owner
            .require_topic(port.tenant, topic, &security(user), None)
            .await,
        Err(ForumError::Forbidden(_))
    ));
    set_moderator(&db, &port, port.parent, user).await;
    assert_eq!(
        owner
            .require_topic(port.tenant, topic, &security(user), None)
            .await
            .unwrap()
            .evaluated_layers,
        2
    );
    assert!(
        owner
            .require_reply(port.tenant, reply, topic, &security(user), None)
            .await
            .unwrap()
            .allowed
    );
}

#[tokio::test]
async fn moderation_admission_reads_uncommitted_ancestor_policy_on_the_writer_transaction() {
    let mut port = TestTaxonomy::new();
    port.require_transaction = true;
    let (db, topic, reply) = moderation_fixture(&port).await;
    let user = Uuid::new_v4();
    let owner = ForumModerationAudienceAuthorizationService::new(db.clone(), None)
        .with_taxonomy_port(Arc::new(port.clone()));
    let txn = db.begin().await.unwrap();
    assert!(
        owner
            .evaluate_topic_in_tx(&txn, port.tenant, topic, &security(user), None)
            .await
            .unwrap()
            .allowed
    );
    forum_category_moderation_audience_policy::ActiveModel {
        tenant_id: Set(port.tenant),
        category_id: Set(port.parent),
        minimum_trust_level: Set(Some(1)),
        updated_at: Set(chrono::Utc::now().into()),
    }
    .insert(&txn)
    .await
    .unwrap();
    // An unresolved trust selector requires exact facts; missing context is an
    // unavailable capability, never unrestricted access or a successful absence.
    assert!(matches!(
        owner
            .evaluate_topic_in_tx(&txn, port.tenant, topic, &security(user), None)
            .await,
        Err(ForumError::CapabilityUnavailable { .. })
    ));
    assert!(matches!(
        owner
            .require_reply_in_tx(&txn, port.tenant, reply, topic, &security(user), None)
            .await,
        Err(ForumError::CapabilityUnavailable { .. })
    ));
    txn.rollback().await.unwrap();
    let txn = db.begin().await.unwrap();
    assert!(
        owner
            .require_topic_in_tx(&txn, port.tenant, topic, &security(user), None)
            .await
            .unwrap()
            .allowed
    );
    txn.rollback().await.unwrap();
}

#[tokio::test]
async fn moderation_commands_deny_inherited_audience_before_mutation_and_keep_provider_failure() {
    let mut port = TestTaxonomy::new();
    port.require_transaction = true;
    let (db, topic, reply) = moderation_fixture(&port).await;
    let user = Uuid::new_v4();
    set_moderator(&db, &port, port.parent, Uuid::new_v4()).await;
    let bus = rustok_outbox::TransactionalEventBus::new(Arc::new(
        rustok_outbox::OutboxTransport::new(db.clone()),
    ));
    let owner =
        ModerationService::new(db.clone(), bus.clone()).with_taxonomy_port(Arc::new(port.clone()));
    for result in [
        owner.pin_topic(port.tenant, topic, security(user)).await,
        owner.close_topic(port.tenant, topic, security(user)).await,
        owner
            .lock_topic_with_audience_context(
                port.tenant,
                topic,
                security(user),
                context(&port, user),
            )
            .await,
        owner
            .approve_reply(port.tenant, reply, topic, security(user))
            .await,
        owner
            .approve_reply_with_audience_context(
                port.tenant,
                reply,
                topic,
                security(user),
                context(&port, user),
            )
            .await,
        owner
            .mark_solution(port.tenant, topic, reply, security(user))
            .await,
        owner
            .clear_solution(port.tenant, topic, security(user))
            .await,
    ] {
        assert!(
            matches!(result, Err(ForumError::Forbidden(message)) if message == "Forum moderation is unavailable for the current audience")
        );
    }
    let unchanged = forum_topic::Entity::find_by_id(topic)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert!(!unchanged.is_pinned);
    assert!(!unchanged.is_locked);
    assert_eq!(unchanged.status, rustok_forum::TopicStatus::Open);
    assert_eq!(
        forum_reply::Entity::find_by_id(reply)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .status,
        rustok_forum::ReplyStatus::Pending
    );
    set_moderator(&db, &port, port.parent, user).await;
    // Positive idempotent command still authorizes, but needs no event journal.
    owner
        .unpin_topic(port.tenant, topic, security(user))
        .await
        .unwrap();
    port.calls.lock().unwrap().clear();
    port.fail_at = Some("parents");
    let failing = ModerationService::new(db, bus).with_taxonomy_port(Arc::new(port));
    assert!(matches!(
        failing
            .pin_topic(unchanged.tenant_id, topic, security(user))
            .await,
        Err(ForumError::CapabilityFailure { .. })
    ));
    // No journal/outbox/solution/content tables exist: denied commands must never
    // reach those writes. This fixture is not a real-owner concurrency proof.
}

#[tokio::test]
async fn graphql_and_mounted_http_moderation_factories_retain_selected_provider() {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use rustok_api::{AuthContextExtension, TenantContextExtension};
    use rustok_forum::{ForumMutation, controllers::axum_router};
    use tower::ServiceExt;

    let mut port = TestTaxonomy::new();
    port.require_transaction = true;
    let (db, topic, _) = moderation_fixture(&port).await;
    let user = Uuid::new_v4();
    set_moderator(&db, &port, port.parent, Uuid::new_v4()).await;
    let tenant = TenantContext {
        id: port.tenant,
        name: "Moderation wiring".into(),
        slug: "moderation-wiring".into(),
        domain: None,
        settings: serde_json::json!({}),
        default_locale: "en".into(),
        is_active: true,
    };
    let auth = AuthContext {
        user_id: user,
        session_id: Uuid::new_v4(),
        tenant_id: port.tenant,
        permissions: vec![Permission::FORUM_TOPICS_MANAGE],
        client_id: None,
        scopes: vec![],
        grant_type: "user".into(),
    };
    let bus = rustok_outbox::TransactionalEventBus::new(Arc::new(
        rustok_outbox::OutboxTransport::new(db.clone()),
    ));
    let selected: Arc<dyn ForumTaxonomyPort> = Arc::new(port.clone());
    let host = HostRuntimeContext::new(db.clone())
        .with_shared_value(selected)
        .with_shared_value(bus.clone());
    let runtime = attach_schema_data(&GraphqlRuntimeInputs::new(host.clone())).unwrap();
    let schema = Schema::build(
        ForumQuery::default(),
        ForumMutation::default(),
        EmptySubscription,
    )
    .extension(ForumGraphqlErrorExtension)
    .data(db.clone())
    .data(bus)
    .data(runtime)
    .data(tenant.clone())
    .data(auth.clone())
    .finish();
    let result = schema
        .execute(format!(
            "mutation {{ pinForumTopic(id: \"{topic}\", pinned: true) }}"
        ))
        .await;
    assert_eq!(result.errors.len(), 1, "{result:?}");
    assert_eq!(
        result.errors[0].extensions.as_ref().unwrap().get("code"),
        Some(&Value::from("FORUM_FORBIDDEN"))
    );
    assert_eq!(
        result.errors[0].message,
        "Forbidden: Forum moderation is unavailable for the current audience"
    );

    let app = axum_router(&host).unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/forum/topics/{topic}/solution"))
                .extension(TenantContextExtension(tenant))
                .extension(AuthContextExtension(auth))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = to_bytes(response.into_body(), 16 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["code"], "FORUM_FORBIDDEN");
    assert!(
        !forum_topic::Entity::find_by_id(topic)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .is_pinned
    );
}
