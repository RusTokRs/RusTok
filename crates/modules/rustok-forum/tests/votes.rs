use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ForumError,
    ForumModule, ListRepliesFilter, ListTopicsFilter, ReplyService, TopicService, VoteService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

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
    let topic_service = TopicService::new(db.clone(), event_bus.clone());
    let reply_service = ReplyService::new(db.clone(), event_bus.clone());
    let vote_service = VoteService::new(db);

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
        .set_topic_vote(tenant_id, topic.id, voter.clone(), 1)
        .await
        .expect("topic upvote should succeed");
    vote_service
        .set_reply_vote(tenant_id, reply.id, voter.clone(), -1)
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

    let (topics, total_topics) = topic_service
        .list(
            tenant_id,
            voter.clone(),
            ListTopicsFilter {
                category_id: Some(category.id),
                status: None,
                locale: Some("en".to_string()),
                page: 1,
                per_page: 20,
            },
        )
        .await
        .expect("topic list should load");
    assert_eq!(total_topics, 1);
    assert_eq!(topics[0].vote_score, 1);
    assert_eq!(topics[0].current_user_vote, Some(1));

    let (replies, total_replies) = reply_service
        .list_response_for_topic_with_locale_fallback(
            tenant_id,
            voter.clone(),
            topic.id,
            ListRepliesFilter {
                locale: Some("en".to_string()),
                page: 1,
                per_page: 20,
            },
            Some("en"),
        )
        .await
        .expect("reply list should load");
    assert_eq!(total_replies, 1);
    assert_eq!(replies[0].vote_score, -1);
    assert_eq!(replies[0].current_user_vote, Some(-1));

    vote_service
        .set_topic_vote(tenant_id, topic.id, voter.clone(), -1)
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
    let topic_service = TopicService::new(db.clone(), event_bus.clone());
    let reply_service = ReplyService::new(db.clone(), event_bus.clone());
    let vote_service = VoteService::new(db.clone());

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
    db.execute_unprepared(&format!(
        "INSERT INTO tenant_modules (id, tenant_id, module_slug, enabled, settings)
         VALUES ('{}', '{}', 'reactions', 1, '{{}}'),
                ('{}', '{}', 'forum', 1, '{{"useReactions": false}}');",
        Uuid::new_v4(),
        tenant_id,
        Uuid::new_v4(),
        tenant_id
    ))
    .await
    .expect("module settings should be created");

    vote_service
        .set_topic_vote(tenant_id, topic.id, voter.clone(), 1)
        .await
        .expect("topic vote should be available while Forum uses internal voting");
    vote_service
        .set_reply_vote(tenant_id, reply.id, voter.clone(), 1)
        .await
        .expect("reply vote should be available while Forum uses internal voting");

    db.execute_unprepared(&format!(
        "UPDATE tenant_modules
         SET settings = '{{"useReactions": true}}'
         WHERE tenant_id = '{}' AND module_slug = 'forum';",
        tenant_id
    ))
    .await
    .expect("forum reactions setting should be enabled");

    let topic_vote_when_reactions_selected = vote_service
        .set_topic_vote(tenant_id, topic.id, voter.clone(), -1)
        .await
        .expect_err("internal topic voting must be disabled by the Forum setting");
    assert!(matches!(
        topic_vote_when_reactions_selected,
        ForumError::InternalVotingDisabled
    ));

    let reply_vote_when_reactions_selected = vote_service
        .set_reply_vote(tenant_id, reply.id, voter.clone(), -1)
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

    db.execute_unprepared(&format!(
        "UPDATE tenant_modules
         SET settings = '{{"useReactions": false}}'
         WHERE tenant_id = '{}' AND module_slug = 'forum';",
        tenant_id
    ))
    .await
    .expect("forum reactions setting should be disabled");

    vote_service
        .set_topic_vote(tenant_id, topic.id, voter.clone(), -1)
        .await
        .expect("topic internal voting should resume when Forum selects voting");
    vote_service
        .set_reply_vote(tenant_id, reply.id, voter, -1)
        .await
        .expect("reply internal voting should resume when Forum selects voting");
}


#[tokio::test]
async fn vote_validation_rejects_invalid_values_and_pending_replies() {
    let (db, event_bus, tenant_id) = setup().await;
    let category_service = CategoryService::new(db.clone());
    let topic_service = TopicService::new(db.clone(), event_bus.clone());
    let reply_service = ReplyService::new(db.clone(), event_bus.clone());
    let vote_service = VoteService::new(db);

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
        .set_topic_vote(tenant_id, topic.id, voter.clone(), 0)
        .await
        .expect_err("invalid vote value should be rejected");
    assert!(matches!(invalid_vote, ForumError::Validation(_)));

    let pending_reply_vote = vote_service
        .set_reply_vote(tenant_id, reply.id, voter.clone(), 1)
        .await
        .expect_err("pending reply must not be votable");
    assert!(matches!(pending_reply_vote, ForumError::Validation(_)));

    let missing_user = vote_service
        .set_topic_vote(tenant_id, topic.id, SecurityContext::system(), 1)
        .await
        .expect_err("system context without user should not vote");
    assert!(matches!(missing_user, ForumError::Forbidden(_)));
}
