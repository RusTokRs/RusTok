use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{
    PortActor, PortContext, PortError, SharedStaticModuleSettingsReader,
    SharedStaticModuleSettingsTransactionReader, StaticModuleSettingsReader,
    StaticModuleSettingsSnapshot, StaticModuleSettingsTransactionReader,
};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput,
    ForumAudienceConstraints, ForumAudienceFacts, ForumAudienceFactsPort,
    ForumAudienceFactsRequest, ForumCategoryAudiencePolicyService, ForumError, ForumModule,
    ListRepliesFilter, ListTopicsFilter, ReplyService, SetForumCategoryAudiencePolicyInput,
    SubscriptionService, TopicService, UpdateForumSubscriptionInput, VoteService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectOptions, ConnectionTrait, Database,
    DatabaseConnection, EntityTrait, QueryFilter,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

/// Keeps the stored or default settings and disables the posting cooldowns, so tests that create
/// several posts by one author are not rejected. The cooldown has its own test file.
fn without_posting_cooldown(mut settings: serde_json::Value) -> serde_json::Value {
    if !settings.is_object() {
        settings = serde_json::json!({});
    }
    if let Some(object) = settings.as_object_mut() {
        object.insert(
            "rate_limit_new_topic_seconds".to_string(),
            serde_json::json!(0),
        );
        object.insert(
            "rate_limit_new_reply_seconds".to_string(),
            serde_json::json!(0),
        );
    }
    settings
}

struct TestForumSettingsReader {
    db: DatabaseConnection,
}

#[async_trait]
impl StaticModuleSettingsReader for TestForumSettingsReader {
    async fn settings(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        let module = rustok_tenant::entities::tenant_module::Entity::find()
            .filter(rustok_tenant::entities::tenant_module::Column::TenantId.eq(tenant_id))
            .filter(rustok_tenant::entities::tenant_module::Column::ModuleSlug.eq(module_slug))
            .one(&self.db)
            .await
            .map_err(|e| PortError::unavailable("database_error", e.to_string()))?;

        if let Some(m) = module {
            return Ok(Some(StaticModuleSettingsSnapshot {
                enabled: m.enabled,
                settings: without_posting_cooldown(m.settings),
            }));
        }

        Ok(Some(StaticModuleSettingsSnapshot {
            enabled: module_slug == "forum",
            settings: without_posting_cooldown(serde_json::json!({ "use_reactions": false })),
        }))
    }
}

#[async_trait]
impl StaticModuleSettingsTransactionReader for TestForumSettingsReader {
    async fn settings_in_tx(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<Option<StaticModuleSettingsSnapshot>, PortError> {
        let module = rustok_tenant::entities::tenant_module::Entity::find()
            .filter(rustok_tenant::entities::tenant_module::Column::TenantId.eq(tenant_id))
            .filter(rustok_tenant::entities::tenant_module::Column::ModuleSlug.eq(module_slug))
            .one(txn)
            .await
            .map_err(|e| PortError::unavailable("database_error", e.to_string()))?;

        if let Some(m) = module {
            return Ok(Some(StaticModuleSettingsSnapshot {
                enabled: m.enabled,
                settings: without_posting_cooldown(m.settings),
            }));
        }

        Ok(Some(StaticModuleSettingsSnapshot {
            enabled: module_slug == "forum",
            settings: without_posting_cooldown(serde_json::json!({ "use_reactions": false })),
        }))
    }
}

fn test_settings_providers(db: DatabaseConnection) -> rustok_forum::ForumSettingsProviders {
    let reader = Arc::new(TestForumSettingsReader { db });
    rustok_forum::ForumSettingsProviders::default().with_static_readers(
        SharedStaticModuleSettingsReader(reader.clone()),
        SharedStaticModuleSettingsTransactionReader(reader),
    )
}

async fn setup_forum_test_db() -> DatabaseConnection {
    let db_url = format!(
        "sqlite:file:forum_votes_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut opts = ConnectOptions::new(db_url);
    opts.max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);

    Database::connect(opts)
        .await
        .expect("failed to connect forum sqlite database")
}

async fn setup() -> (DatabaseConnection, TransactionalEventBus, Uuid) {
    let db = setup_forum_test_db().await;
    let schema = SchemaManager::new(&db);
    for migration in OutboxModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("outbox migration should apply");
    }
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("taxonomy migration should apply");
    }
    db.execute_unprepared(
        "CREATE TABLE IF NOT EXISTS users (
            id TEXT NOT NULL PRIMARY KEY,
            tenant_id TEXT NOT NULL
        );",
    )
    .await
    .expect("users table fixture should apply");

    db.execute_unprepared(
        "CREATE TABLE IF NOT EXISTS tenant_modules (
            id TEXT NOT NULL PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            module_slug TEXT NOT NULL,
            enabled BOOLEAN NOT NULL DEFAULT 1,
            settings TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );",
    )
    .await
    .expect("tenant modules table fixture should apply");
    let module = ForumModule;
    for migration in module.migrations() {
        migration
            .up(&schema)
            .await
            .expect("forum migration should apply");
    }

    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    (db, event_bus, Uuid::new_v4())
}

async fn create_category(
    service: &CategoryService,
    tenant_id: Uuid,
    security: SecurityContext,
    moderated: bool,
) -> rustok_forum::CategoryResponse {
    service
        .create(
            tenant_id,
            security,
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "General".to_string(),
                slug: "general".to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated,
            },
        )
        .await
        .expect("category should be created")
}

#[tokio::test]
async fn topic_and_reply_votes_round_trip_through_read_paths() {
    let (db, event_bus, tenant_id) = setup().await;
    let category_service = CategoryService::new(db.clone());
    let topic_service = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()));
    let reply_service = ReplyService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()));
    let vote_service =
        VoteService::new(db.clone()).with_settings_providers(test_settings_providers(db.clone()));

    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let author = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let voter = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let other_viewer = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));

    let category = create_category(&category_service, tenant_id, admin, false).await;
    let topic = topic_service
        .create(
            tenant_id,
            author.clone(),
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: category.id,
                title: "Vote me".to_string(),
                slug: Some("vote-me".to_string()),
                body: rustok_api::RichTextDocument::single_paragraph("Body"),
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created");
    let reply = reply_service
        .create(
            tenant_id,
            author,
            topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph("Reply"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("reply should be created");

    vote_service
        .set_topic_vote(
            tenant_id,
            topic.id,
            voter.clone(),
            write_context(tenant_id, &voter),
            1,
        )
        .await
        .expect("topic upvote should succeed");
    vote_service
        .set_reply_vote(
            tenant_id,
            reply.id,
            voter.clone(),
            write_context(tenant_id, &voter),
            -1,
        )
        .await
        .expect("reply downvote should succeed");

    let topic_after_vote = topic_service
        .get(tenant_id, voter.clone(), topic.id, "en")
        .await
        .expect("topic should load for voter");
    assert_eq!(topic_after_vote.vote_score, 1);
    assert_eq!(topic_after_vote.current_user_vote, Some(1));

    let reply_after_vote = reply_service
        .get(tenant_id, voter.clone(), reply.id, "en")
        .await
        .expect("reply should load for voter");
    assert_eq!(reply_after_vote.vote_score, -1);
    assert_eq!(reply_after_vote.current_user_vote, Some(-1));

    let page = topic_service
        .list(
            tenant_id,
            voter.clone(),
            ListTopicsFilter {
                category_id: Some(category.id),
                status: None,
                locale: Some("en".to_string()),
                after: None,
                per_page: 20,
            },
        )
        .await
        .expect("topic list should load");
    let topics = page.items;
    assert_eq!(topics.len(), 1);
    assert_eq!(topics[0].vote_score, 1);
    assert_eq!(topics[0].current_user_vote, Some(1));

    let replies_page = reply_service
        .list_response_for_topic_with_locale_fallback(
            tenant_id,
            voter.clone(),
            topic.id,
            ListRepliesFilter {
                locale: Some("en".to_string()),
                after: None,
                per_page: 20,
            },
            Some("en"),
        )
        .await
        .expect("reply list should load");
    assert!(replies_page.next_cursor.is_none());
    assert_eq!(replies_page.items.len(), 1);
    assert_eq!(replies_page.items[0].vote_score, -1);
    assert_eq!(replies_page.items[0].current_user_vote, Some(-1));

    vote_service
        .set_topic_vote(
            tenant_id,
            topic.id,
            voter.clone(),
            write_context(tenant_id, &voter),
            -1,
        )
        .await
        .expect("topic vote should be replaceable");
    let topic_after_flip = topic_service
        .get(tenant_id, voter.clone(), topic.id, "en")
        .await
        .expect("topic should load after vote flip");
    assert_eq!(topic_after_flip.vote_score, -1);
    assert_eq!(topic_after_flip.current_user_vote, Some(-1));

    let topic_for_other_user = topic_service
        .get(tenant_id, other_viewer, topic.id, "en")
        .await
        .expect("topic should load for another viewer");
    assert_eq!(topic_for_other_user.vote_score, -1);
    assert_eq!(topic_for_other_user.current_user_vote, None);

    vote_service
        .clear_topic_vote(tenant_id, topic.id, voter.clone())
        .await
        .expect("topic vote should clear");
    vote_service
        .clear_reply_vote(tenant_id, reply.id, voter.clone())
        .await
        .expect("reply vote should clear");

    let topic_after_clear = topic_service
        .get(tenant_id, voter.clone(), topic.id, "en")
        .await
        .expect("topic should load after clear");
    assert_eq!(topic_after_clear.vote_score, 0);
    assert_eq!(topic_after_clear.current_user_vote, None);

    let reply_after_clear = reply_service
        .get(tenant_id, voter, reply.id, "en")
        .await
        .expect("reply should load after clear");
    assert_eq!(reply_after_clear.vote_score, 0);
    assert_eq!(reply_after_clear.current_user_vote, None);
}

#[tokio::test]
async fn internal_votes_switch_by_forum_setting_independently_of_reactions_module() {
    let (db, event_bus, tenant_id) = setup().await;
    let category_service = CategoryService::new(db.clone());
    let topic_service = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()));
    let reply_service = ReplyService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()));
    let vote_service =
        VoteService::new(db.clone()).with_settings_providers(test_settings_providers(db.clone()));

    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let author = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let voter = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));

    let category = create_category(&category_service, tenant_id, admin, false).await;
    let topic = topic_service
        .create(
            tenant_id,
            author.clone(),
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: category.id,
                title: "Switch engagement mode".to_string(),
                slug: Some("switch-engagement-mode".to_string()),
                body: rustok_api::RichTextDocument::single_paragraph("Body"),
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created");
    let reply = reply_service
        .create(
            tenant_id,
            author,
            topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph("Reply"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("reply should be created");

    // The shared Reactions module may be enabled for other modules; Forum still
    // uses internal voting until its own setting explicitly selects Reactions.
    let now = chrono::Utc::now();
    rustok_tenant::entities::tenant_module::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        module_slug: Set("reactions".to_string()),
        enabled: Set(true),
        settings: Set(serde_json::json!({})),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await
    .expect("module settings should be created");

    rustok_tenant::entities::tenant_module::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        module_slug: Set("forum".to_string()),
        enabled: Set(true),
        settings: Set(serde_json::json!({"useReactions": false})),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await
    .expect("module settings should be created");

    vote_service
        .set_topic_vote(
            tenant_id,
            topic.id,
            voter.clone(),
            write_context(tenant_id, &voter),
            1,
        )
        .await
        .expect("topic vote should be available while Forum uses internal voting");
    vote_service
        .set_reply_vote(
            tenant_id,
            reply.id,
            voter.clone(),
            write_context(tenant_id, &voter),
            1,
        )
        .await
        .expect("reply vote should be available while Forum uses internal voting");

    let mut forum_mod: rustok_tenant::entities::tenant_module::ActiveModel =
        rustok_tenant::entities::tenant_module::Entity::find()
            .filter(rustok_tenant::entities::tenant_module::Column::TenantId.eq(tenant_id))
            .filter(rustok_tenant::entities::tenant_module::Column::ModuleSlug.eq("forum"))
            .one(&db)
            .await
            .expect("load forum module")
            .expect("forum module must exist")
            .into();
    forum_mod.settings = Set(serde_json::json!({"useReactions": true}));
    forum_mod
        .update(&db)
        .await
        .expect("forum reactions setting should be enabled");

    let topic_vote_when_reactions_selected = vote_service
        .set_topic_vote(
            tenant_id,
            topic.id,
            voter.clone(),
            write_context(tenant_id, &voter),
            -1,
        )
        .await
        .expect_err("internal topic voting must be disabled by the Forum setting");
    assert!(matches!(
        topic_vote_when_reactions_selected,
        ForumError::InternalVotingDisabled
    ));

    let reply_vote_when_reactions_selected = vote_service
        .set_reply_vote(
            tenant_id,
            reply.id,
            voter.clone(),
            write_context(tenant_id, &voter),
            -1,
        )
        .await
        .expect_err("internal reply voting must be disabled by the Forum setting");
    assert!(matches!(
        reply_vote_when_reactions_selected,
        ForumError::InternalVotingDisabled
    ));

    let topic_clear_when_reactions_selected = vote_service
        .clear_topic_vote(tenant_id, topic.id, voter.clone())
        .await
        .expect_err("clearing internal topic voting must be disabled by the Forum setting");
    assert!(matches!(
        topic_clear_when_reactions_selected,
        ForumError::InternalVotingDisabled
    ));

    let reply_clear_when_reactions_selected = vote_service
        .clear_reply_vote(tenant_id, reply.id, voter.clone())
        .await
        .expect_err("clearing internal reply voting must be disabled by the Forum setting");
    assert!(matches!(
        reply_clear_when_reactions_selected,
        ForumError::InternalVotingDisabled
    ));

    let topic_summary = vote_service
        .topic_vote_summary(tenant_id, topic.id, None)
        .await
        .expect("topic summary should load");
    assert_eq!(topic_summary.score, 0);
    assert_eq!(topic_summary.current_user_vote, None);

    let reply_summary = vote_service
        .reply_vote_summary(tenant_id, reply.id, None)
        .await
        .expect("reply summary should load");
    assert_eq!(reply_summary.score, 0);
    assert_eq!(reply_summary.current_user_vote, None);

    let mut forum_mod: rustok_tenant::entities::tenant_module::ActiveModel =
        rustok_tenant::entities::tenant_module::Entity::find()
            .filter(rustok_tenant::entities::tenant_module::Column::TenantId.eq(tenant_id))
            .filter(rustok_tenant::entities::tenant_module::Column::ModuleSlug.eq("forum"))
            .one(&db)
            .await
            .expect("load forum module")
            .expect("forum module must exist")
            .into();
    forum_mod.settings = Set(serde_json::json!({"useReactions": false}));
    forum_mod
        .update(&db)
        .await
        .expect("forum reactions setting should be disabled");

    vote_service
        .set_topic_vote(
            tenant_id,
            topic.id,
            voter.clone(),
            write_context(tenant_id, &voter),
            -1,
        )
        .await
        .expect("topic internal voting should resume when Forum selects voting");
    vote_service
        .set_reply_vote(
            tenant_id,
            reply.id,
            voter.clone(),
            write_context(tenant_id, &voter),
            -1,
        )
        .await
        .expect("reply internal voting should resume when Forum selects voting");
}

#[tokio::test]
async fn vote_validation_rejects_invalid_values_and_pending_replies() {
    let (db, event_bus, tenant_id) = setup().await;
    let category_service = CategoryService::new(db.clone());
    let topic_service = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()));
    let reply_service = ReplyService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()));
    let vote_service =
        VoteService::new(db.clone()).with_settings_providers(test_settings_providers(db.clone()));

    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let author = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let voter = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));

    let category = create_category(&category_service, tenant_id, admin, true).await;
    let topic = topic_service
        .create(
            tenant_id,
            author.clone(),
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: category.id,
                title: "Pending votes".to_string(),
                slug: Some("pending-votes".to_string()),
                body: rustok_api::RichTextDocument::single_paragraph("Body"),
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created");
    let reply = reply_service
        .create(
            tenant_id,
            author,
            topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph("Pending reply"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("reply should be created");

    let invalid_vote = vote_service
        .set_topic_vote(
            tenant_id,
            topic.id,
            voter.clone(),
            write_context(tenant_id, &voter),
            0,
        )
        .await
        .expect_err("invalid vote value should be rejected");
    assert!(matches!(invalid_vote, ForumError::Validation(_)));

    let pending_reply_vote = vote_service
        .set_reply_vote(
            tenant_id,
            reply.id,
            voter.clone(),
            write_context(tenant_id, &voter),
            1,
        )
        .await
        .expect_err("pending reply must not be votable");
    assert!(matches!(pending_reply_vote, ForumError::Validation(_)));

    let missing_user = vote_service
        .set_topic_vote(
            tenant_id,
            topic.id,
            SecurityContext::system(),
            write_context(tenant_id, &SecurityContext::system()),
            1,
        )
        .await
        .expect_err("system context without user should not vote");
    assert!(matches!(missing_user, ForumError::Forbidden(_)));
}

/// Builds the exact user port context used by vote and subscription writes in tests.
fn write_context(tenant_id: Uuid, security: &SecurityContext) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(security.user_id.unwrap_or_else(Uuid::nil).to_string()),
        "en",
        "test-audience-write",
    )
}

/// Trust facts for the write-audience tests: one low-trust user, everyone else trusted.
struct WriteTrustFactsPort {
    low_trust_user_id: Uuid,
}

#[async_trait]
impl ForumAudienceFactsPort for WriteTrustFactsPort {
    async fn resolve_forum_audience_facts(
        &self,
        _context: PortContext,
        request: ForumAudienceFactsRequest,
    ) -> Result<ForumAudienceFacts, PortError> {
        Ok(ForumAudienceFacts {
            tenant_id: request.tenant_id,
            user_id: request.user_id,
            trust_level: request.include_trust_level.then_some(
                if request.user_id == self.low_trust_user_id {
                    1
                } else {
                    8
                },
            ),
            channel_memberships: Vec::new(),
            group_memberships: Vec::new(),
        })
    }
}

#[tokio::test]
async fn vote_and_subscription_writes_require_owner_audience_and_reject_self_votes() {
    let (db, event_bus, tenant_id) = setup().await;
    let admin_id = Uuid::new_v4();
    let low_trust_id = Uuid::new_v4();
    let trusted_id = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(admin_id));
    let low = SecurityContext::new(UserRole::Customer, Some(low_trust_id));
    let trusted = SecurityContext::new(UserRole::Customer, Some(trusted_id));

    // Content is created before the trust layer is attached, so the restriction is applied
    // to already-persisted topics and replies, which is the case the write gate must cover.
    let category = CategoryService::new(db.clone())
        .create(
            tenant_id,
            admin.clone(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "Restricted".to_string(),
                slug: "restricted-area".to_string(),
                description: None,
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await
        .expect("restricted category should be created");
    let topic = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()))
        .create(
            tenant_id,
            admin.clone(),
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: category.id,
                title: "Restricted topic".to_string(),
                slug: Some("restricted-topic".to_string()),
                body: rustok_api::RichTextDocument::single_paragraph("Body"),
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("restricted topic should be created");
    let reply = ReplyService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()))
        .create(
            tenant_id,
            admin.clone(),
            topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph("Reply"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("restricted reply should be created");
    ForumCategoryAudiencePolicyService::new(db.clone())
        .set(
            tenant_id,
            category.id,
            admin.clone(),
            SetForumCategoryAudiencePolicyInput {
                constraints: ForumAudienceConstraints {
                    minimum_trust_level: Some(5),
                    ..ForumAudienceConstraints::default()
                },
            },
        )
        .await
        .expect("restricted category trust layer should persist");

    let vote_service = VoteService::new(db.clone())
        .with_settings_providers(test_settings_providers(db.clone()))
        .with_audience_facts(Arc::new(WriteTrustFactsPort {
            low_trust_user_id: low_trust_id,
        }));

    // A low-trust user cannot vote on a topic or reply they cannot read; denial is
    // indistinguishable from absence, so the error names the requested id only.
    let low_topic_vote = vote_service
        .set_topic_vote(
            tenant_id,
            topic.id,
            low.clone(),
            write_context(tenant_id, &low),
            1,
        )
        .await;
    assert!(
        matches!(low_topic_vote, Err(ForumError::TopicNotFound(id)) if id == topic.id),
        "restricted topic vote must be denied as not found"
    );
    let low_reply_vote = vote_service
        .set_reply_vote(
            tenant_id,
            reply.id,
            low.clone(),
            write_context(tenant_id, &low),
            1,
        )
        .await;
    assert!(
        matches!(low_reply_vote, Err(ForumError::ReplyNotFound(id)) if id == reply.id),
        "restricted reply vote must be denied as not found"
    );

    // A trusted user passes the same gate.
    vote_service
        .set_topic_vote(
            tenant_id,
            topic.id,
            trusted.clone(),
            write_context(tenant_id, &trusted),
            1,
        )
        .await
        .expect("trusted user should vote on the restricted topic");
    vote_service
        .set_reply_vote(
            tenant_id,
            reply.id,
            trusted.clone(),
            write_context(tenant_id, &trusted),
            1,
        )
        .await
        .expect("trusted user should vote on the restricted reply");

    // Authors cannot vote on their own content. The admin created both fixtures.
    let self_topic_vote = vote_service
        .set_topic_vote(
            tenant_id,
            topic.id,
            admin.clone(),
            write_context(tenant_id, &admin),
            1,
        )
        .await;
    assert!(
        matches!(self_topic_vote, Err(ForumError::Forbidden(_))),
        "topic author self-vote must be forbidden"
    );
    let self_reply_vote = vote_service
        .set_reply_vote(
            tenant_id,
            reply.id,
            admin.clone(),
            write_context(tenant_id, &admin),
            1,
        )
        .await;
    assert!(
        matches!(self_reply_vote, Err(ForumError::Forbidden(_))),
        "reply author self-vote must be forbidden"
    );

    // Clearing a vote is not audience-gated, so a user who lost access can still remove
    // their own vote.
    vote_service
        .clear_topic_vote(tenant_id, topic.id, low.clone())
        .await
        .expect("clearing a vote must not require audience access");

    let missing_topic_vote = vote_service
        .set_topic_vote(
            tenant_id,
            Uuid::new_v4(),
            trusted.clone(),
            write_context(tenant_id, &trusted),
            1,
        )
        .await;
    assert!(matches!(
        missing_topic_vote,
        Err(ForumError::TopicNotFound(_))
    ));

    let subscription_service =
        SubscriptionService::new(db.clone()).with_audience_facts(Arc::new(WriteTrustFactsPort {
            low_trust_user_id: low_trust_id,
        }));
    let low_subscription = subscription_service
        .update_topic_subscription(
            tenant_id,
            topic.id,
            low.clone(),
            write_context(tenant_id, &low),
            UpdateForumSubscriptionInput::watching(),
        )
        .await;
    assert!(
        matches!(low_subscription, Err(ForumError::TopicNotFound(id)) if id == topic.id),
        "restricted topic subscription must be denied as not found"
    );
    subscription_service
        .set_topic_subscription(
            tenant_id,
            topic.id,
            trusted.clone(),
            write_context(tenant_id, &trusted),
        )
        .await
        .expect("trusted user should subscribe to the restricted topic");
    subscription_service
        .clear_topic_subscription(tenant_id, topic.id, low.clone())
        .await
        .expect("clearing a subscription must not require audience access");
}

/// Writes the Forum module settings row that the vote policy reads for this tenant.
async fn set_forum_module_settings(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    settings: serde_json::Value,
) {
    let existing = rustok_tenant::entities::tenant_module::Entity::find()
        .filter(rustok_tenant::entities::tenant_module::Column::TenantId.eq(tenant_id))
        .filter(rustok_tenant::entities::tenant_module::Column::ModuleSlug.eq("forum"))
        .one(db)
        .await
        .expect("load forum module");
    match existing {
        Some(row) => {
            let mut active: rustok_tenant::entities::tenant_module::ActiveModel = row.into();
            active.settings = Set(settings);
            active
                .update(db)
                .await
                .expect("forum settings should update");
        }
        None => {
            let now = chrono::Utc::now();
            rustok_tenant::entities::tenant_module::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                module_slug: Set("forum".to_string()),
                enabled: Set(true),
                settings: Set(settings),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(db)
            .await
            .expect("forum settings should be created");
        }
    }
}

/// Creates an unmoderated topic and an approved reply, both written by `author`.
async fn create_topic_with_approved_reply(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    admin: SecurityContext,
    author: SecurityContext,
) -> (Uuid, Uuid) {
    let category =
        create_category(&CategoryService::new(db.clone()), tenant_id, admin, false).await;
    let topic = TopicService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()))
        .create(
            tenant_id,
            author.clone(),
            CreateTopicInput {
                locale: "en".to_string(),
                category_id: category.id,
                title: "Policy topic".to_string(),
                slug: Some("policy-topic".to_string()),
                body: rustok_api::RichTextDocument::single_paragraph("Body"),
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created");
    let reply = ReplyService::new(db.clone(), event_bus.clone())
        .with_settings_providers(test_settings_providers(db.clone()))
        .create(
            tenant_id,
            author,
            topic.id,
            CreateReplyInput {
                locale: "en".to_string(),
                content: rustok_api::RichTextDocument::single_paragraph("Approved reply"),
                parent_reply_id: None,
            },
        )
        .await
        .expect("reply should be created");
    (topic.id, reply.id)
}

#[tokio::test]
async fn downvotes_are_rejected_only_while_the_tenant_disables_them() {
    let (db, event_bus, tenant_id) = setup().await;
    let vote_service =
        VoteService::new(db.clone()).with_settings_providers(test_settings_providers(db.clone()));
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let author = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let voter = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let (topic_id, reply_id) =
        create_topic_with_approved_reply(&db, &event_bus, tenant_id, admin, author).await;

    set_forum_module_settings(
        &db,
        tenant_id,
        serde_json::json!({ "allow_downvotes": false }),
    )
    .await;

    let topic_downvote = vote_service
        .set_topic_vote(
            tenant_id,
            topic_id,
            voter.clone(),
            write_context(tenant_id, &voter),
            -1,
        )
        .await
        .expect_err("downvotes must be rejected when the tenant disables them");
    assert!(matches!(topic_downvote, ForumError::Validation(_)));

    let reply_downvote = vote_service
        .set_reply_vote(
            tenant_id,
            reply_id,
            voter.clone(),
            write_context(tenant_id, &voter),
            -1,
        )
        .await
        .expect_err("reply downvotes must be rejected when the tenant disables them");
    assert!(matches!(reply_downvote, ForumError::Validation(_)));

    vote_service
        .set_topic_vote(
            tenant_id,
            topic_id,
            voter.clone(),
            write_context(tenant_id, &voter),
            1,
        )
        .await
        .expect("upvotes stay available when downvotes are disabled");
    let summary = vote_service
        .topic_vote_summary(tenant_id, topic_id, voter.user_id)
        .await
        .expect("topic vote summary should load");
    assert_eq!(summary.score, 1);

    set_forum_module_settings(
        &db,
        tenant_id,
        serde_json::json!({ "allow_downvotes": true }),
    )
    .await;
    vote_service
        .set_topic_vote(
            tenant_id,
            topic_id,
            voter.clone(),
            write_context(tenant_id, &voter),
            -1,
        )
        .await
        .expect("downvotes are accepted again once the tenant enables them");
}

#[tokio::test]
async fn self_votes_are_forbidden_by_default_and_allowed_by_the_tenant_setting() {
    let (db, event_bus, tenant_id) = setup().await;
    let vote_service =
        VoteService::new(db.clone()).with_settings_providers(test_settings_providers(db.clone()));
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let author = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let (topic_id, reply_id) =
        create_topic_with_approved_reply(&db, &event_bus, tenant_id, admin, author.clone()).await;

    // No settings row: the default keeps the F2 rule that authors cannot vote on their own content.
    let topic_self_vote = vote_service
        .set_topic_vote(
            tenant_id,
            topic_id,
            author.clone(),
            write_context(tenant_id, &author),
            1,
        )
        .await
        .expect_err("authors must not vote on their topic by default");
    assert!(matches!(topic_self_vote, ForumError::Forbidden(_)));
    let reply_self_vote = vote_service
        .set_reply_vote(
            tenant_id,
            reply_id,
            author.clone(),
            write_context(tenant_id, &author),
            1,
        )
        .await
        .expect_err("authors must not vote on their reply by default");
    assert!(matches!(reply_self_vote, ForumError::Forbidden(_)));

    set_forum_module_settings(
        &db,
        tenant_id,
        serde_json::json!({ "allow_self_voting": true }),
    )
    .await;

    vote_service
        .set_topic_vote(
            tenant_id,
            topic_id,
            author.clone(),
            write_context(tenant_id, &author),
            1,
        )
        .await
        .expect("authors may vote on their topic when the tenant allows self votes");
    vote_service
        .set_reply_vote(
            tenant_id,
            reply_id,
            author.clone(),
            write_context(tenant_id, &author),
            1,
        )
        .await
        .expect("authors may vote on their reply when the tenant allows self votes");
    let topic_summary = vote_service
        .topic_vote_summary(tenant_id, topic_id, author.user_id)
        .await
        .expect("topic vote summary should load");
    assert_eq!(topic_summary.score, 1);
}
