//! Regression tests for the 2026-11-02 forum logic-review remediation
//! (`docs/logic-review-2026-11-02.md`): Rust-owned auto-subscribe that never
//! overwrites explicit subscription rows, digest-selection rejection, reply
//! parent/depth rules, edit re-moderation, and the pending-topic vote freeze.

#[path = "support/posting_cooldown.rs"]
mod posting_cooldown;

use std::sync::Arc;

use rustok_api::{PortActor, PortContext, RichTextDocument};
use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateReplyInput, CreateTopicInput, ForumDigestMode,
    ForumModule, ForumSubscriptionLevel, ModerationService, ReplyService, SubscriptionService,
    TopicService, UpdateForumSubscriptionInput, UpdateReplyInput, VoteService,
};
use rustok_outbox::{OutboxModule, OutboxTransport, TransactionalEventBus};
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup() -> (DatabaseConnection, TransactionalEventBus, Uuid) {
    let db_url = format!(
        "sqlite:file:forum_logic_review_remediation_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(db_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options)
        .await
        .expect("forum remediation sqlite database should connect");
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
        "CREATE TABLE IF NOT EXISTS users (\
            id TEXT NOT NULL PRIMARY KEY,\
            tenant_id TEXT NOT NULL\
        );",
    )
    .await
    .expect("users table fixture should apply");
    for migration in ForumModule.migrations() {
        migration
            .up(&schema)
            .await
            .expect("forum migration should apply");
    }
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    (db, event_bus, Uuid::new_v4())
}

fn write_context(tenant_id: Uuid, security: &SecurityContext) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(security.user_id.unwrap_or_else(Uuid::nil).to_string()),
        "en",
        "test-remediation-write",
    )
}

async fn make_category(
    service: &CategoryService,
    tenant_id: Uuid,
    admin: &SecurityContext,
    slug: &str,
    moderated: bool,
) -> Uuid {
    service
        .create(
            tenant_id,
            admin.clone(),
            CreateCategoryInput {
                locale: "en".into(),
                name: slug.into(),
                slug: slug.into(),
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
        .id
}

async fn make_topic(
    service: &TopicService,
    tenant_id: Uuid,
    author: &SecurityContext,
    category_id: Uuid,
    slug: &str,
) -> Uuid {
    service
        .create(
            tenant_id,
            author.clone(),
            CreateTopicInput {
                locale: "en".into(),
                category_id,
                title: format!("Topic {slug}"),
                slug: Some(slug.into()),
                body: RichTextDocument::single_paragraph("Body"),
                metadata: serde_json::json!({}),
                tags: vec![],
                channel_slugs: None,
            },
        )
        .await
        .expect("topic should be created")
        .id
}

fn reply_input(text: &str, parent_reply_id: Option<Uuid>) -> CreateReplyInput {
    CreateReplyInput {
        locale: "en".into(),
        content: RichTextDocument::single_paragraph(text),
        parent_reply_id,
    }
}

/// H1: implicit auto-subscribe must never overwrite an explicit row. A
/// cleared (`Normal`) author subscription and a `Muted` participant
/// subscription both survive later approved replies by the same users.
#[tokio::test]
async fn auto_subscribe_never_overwrites_explicit_rows() {
    let (db, event_bus, tenant_id) = setup().await;
    let providers = posting_cooldown::zero_cooldown_providers();
    let category_service = CategoryService::new(db.clone());
    let topic_service =
        TopicService::new(db.clone(), event_bus.clone()).with_settings_providers(providers.clone());
    let reply_service =
        ReplyService::new(db.clone(), event_bus).with_settings_providers(providers);
    let subscriptions = SubscriptionService::new(db.clone());

    let author_id = Uuid::new_v4();
    let participant_id = Uuid::new_v4();
    let author = SecurityContext::new(UserRole::Customer, Some(author_id));
    let participant = SecurityContext::new(UserRole::Customer, Some(participant_id));
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let category_id = make_category(&category_service, tenant_id, &admin, "general", false).await;
    let topic_id = make_topic(&topic_service, tenant_id, &author, category_id, "auto-sub").await;

    // Topic creation auto-subscribes the author at the default level.
    let author_sub = subscriptions
        .get_topic_subscription(
            tenant_id,
            topic_id,
            author.clone(),
            write_context(tenant_id, &author),
        )
        .await
        .expect("author subscription should load");
    assert_eq!(author_sub.level, ForumSubscriptionLevel::Watching);

    // The author explicitly clears; a later approved reply by the author
    // (participant path) must not resurrect the subscription.
    subscriptions
        .clear_topic_subscription(tenant_id, topic_id, author.clone())
        .await
        .expect("clear should succeed");
    reply_service
        .create(
            tenant_id,
            author.clone(),
            topic_id,
            reply_input("author follow-up", None),
        )
        .await
        .expect("author reply should be created");
    let author_sub = subscriptions
        .get_topic_subscription(
            tenant_id,
            topic_id,
            author.clone(),
            write_context(tenant_id, &author),
        )
        .await
        .expect("author subscription should still load");
    assert_eq!(
        author_sub.level,
        ForumSubscriptionLevel::Normal,
        "cleared subscription must survive later replies"
    );

    // A participant is auto-subscribed by their first approved reply...
    reply_service
        .create(
            tenant_id,
            participant.clone(),
            topic_id,
            reply_input("participant first", None),
        )
        .await
        .expect("participant reply should be created");
    let participant_sub = subscriptions
        .get_topic_subscription(
            tenant_id,
            topic_id,
            participant.clone(),
            write_context(tenant_id, &participant),
        )
        .await
        .expect("participant subscription should load");
    assert_eq!(participant_sub.level, ForumSubscriptionLevel::Tracking);

    // ...mutes explicitly, and keeps the mute after further replies.
    subscriptions
        .update_topic_subscription(
            tenant_id,
            topic_id,
            participant.clone(),
            write_context(tenant_id, &participant),
            UpdateForumSubscriptionInput {
                level: ForumSubscriptionLevel::Muted,
                notify_mentions: None,
                notify_replies: None,
                notify_new_topics: None,
                digest_mode: None,
                expected_revision: Some(participant_sub.revision),
            },
        )
        .await
        .expect("mute should be stored");
    reply_service
        .create(
            tenant_id,
            participant.clone(),
            topic_id,
            reply_input("participant second", None),
        )
        .await
        .expect("participant second reply should be created");
    let participant_sub = subscriptions
        .get_topic_subscription(
            tenant_id,
            topic_id,
            participant.clone(),
            write_context(tenant_id, &participant),
        )
        .await
        .expect("participant subscription should still load");
    assert_eq!(
        participant_sub.level,
        ForumSubscriptionLevel::Muted,
        "muted subscription must survive later replies"
    );
}

/// H2: an effective daily/weekly digest selection fails closed until the
/// notifications owner ships digest batching.
#[tokio::test]
async fn digest_selection_is_rejected() {
    let (db, event_bus, tenant_id) = setup().await;
    let providers = posting_cooldown::zero_cooldown_providers();
    let category_service = CategoryService::new(db.clone());
    let topic_service =
        TopicService::new(db.clone(), event_bus).with_settings_providers(providers);
    let subscriptions = SubscriptionService::new(db.clone());

    let author = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let category_id = make_category(&category_service, tenant_id, &admin, "digest", false).await;
    let topic_id = make_topic(&topic_service, tenant_id, &author, category_id, "digest").await;

    for digest_mode in [ForumDigestMode::Daily, ForumDigestMode::Weekly] {
        let result = subscriptions
            .update_topic_subscription(
                tenant_id,
                topic_id,
                author.clone(),
                write_context(tenant_id, &author),
                UpdateForumSubscriptionInput {
                    level: ForumSubscriptionLevel::Watching,
                    notify_mentions: None,
                    notify_replies: None,
                    notify_new_topics: None,
                    digest_mode: Some(digest_mode),
                    expected_revision: None,
                },
            )
            .await;
        assert!(
            result.is_err(),
            "digest mode {digest_mode:?} must be rejected until digest delivery exists"
        );
    }
}

/// M5: only approved replies can acquire children, and nesting depth is
/// capped so clients never render unbounded recursion.
#[tokio::test]
async fn pending_parent_rejected_and_depth_capped() {
    let (db, event_bus, tenant_id) = setup().await;
    let providers = posting_cooldown::zero_cooldown_providers();
    let category_service = CategoryService::new(db.clone());
    let topic_service =
        TopicService::new(db.clone(), event_bus.clone()).with_settings_providers(providers.clone());
    let reply_service =
        ReplyService::new(db.clone(), event_bus.clone()).with_settings_providers(providers);
    let moderation = ModerationService::new(db.clone(), event_bus);

    let author = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    // Pending parent: moderated category holds the first reply.
    let moderated_id =
        make_category(&category_service, tenant_id, &admin, "moderated", true).await;
    let moderated_topic =
        make_topic(&topic_service, tenant_id, &author, moderated_id, "moderated").await;
    let parent = reply_service
        .create(
            tenant_id,
            author.clone(),
            moderated_topic,
            reply_input("held parent", None),
        )
        .await
        .expect("held reply should be created");
    assert_eq!(parent.status, "pending");
    let child = reply_service
        .create(
            tenant_id,
            author.clone(),
            moderated_topic,
            reply_input("child of held parent", Some(parent.id)),
        )
        .await;
    assert!(
        child.is_err(),
        "a pending reply must not acquire children"
    );

    // After approval the same child is accepted.
    moderation
        .approve_reply(tenant_id, parent.id, moderated_topic, admin.clone())
        .await
        .expect("parent should be approved");
    reply_service
        .create(
            tenant_id,
            author.clone(),
            moderated_topic,
            reply_input("child of approved parent", Some(parent.id)),
        )
        .await
        .expect("child of approved parent should be created");

    // Depth cap: a chain of nested replies stops at the configured maximum.
    let open_id = make_category(&category_service, tenant_id, &admin, "open", false).await;
    let open_topic = make_topic(&topic_service, tenant_id, &author, open_id, "open").await;
    let mut parent_id = None;
    let mut created = 0_u32;
    loop {
        match reply_service
            .create(
                tenant_id,
                author.clone(),
                open_topic,
                reply_input("nested", parent_id),
            )
            .await
        {
            Ok(reply) => {
                parent_id = Some(reply.id);
                created += 1;
                assert!(
                    created <= 32,
                    "nesting must terminate at the depth cap, not run away"
                );
            }
            Err(_) => break,
        }
    }
    assert!(
        created >= 10,
        "a reasonable nesting depth must remain available, got {created}"
    );
    assert!(
        created <= 12,
        "nesting must stop at the depth cap, got {created} levels"
    );
}

/// M6: editing the body of an approved reply in a moderated category sends
/// it back to pending instead of leaving unreviewed content public.
#[tokio::test]
async fn approved_reply_edit_returns_to_pending_in_moderated_category() {
    let (db, event_bus, tenant_id) = setup().await;
    let providers = posting_cooldown::zero_cooldown_providers();
    let category_service = CategoryService::new(db.clone());
    let topic_service =
        TopicService::new(db.clone(), event_bus.clone()).with_settings_providers(providers.clone());
    let reply_service =
        ReplyService::new(db.clone(), event_bus.clone()).with_settings_providers(providers);
    let moderation = ModerationService::new(db.clone(), event_bus);

    let author = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let category_id =
        make_category(&category_service, tenant_id, &admin, "reviewed", true).await;
    let topic_id = make_topic(&topic_service, tenant_id, &author, category_id, "reviewed").await;

    let reply = reply_service
        .create(
            tenant_id,
            author.clone(),
            topic_id,
            reply_input("original body", None),
        )
        .await
        .expect("reply should be created");
    assert_eq!(reply.status, "pending");
    moderation
        .approve_reply(tenant_id, reply.id, topic_id, admin.clone())
        .await
        .expect("reply should be approved");

    let edited = reply_service
        .update(
            tenant_id,
            reply.id,
            author.clone(),
            UpdateReplyInput {
                locale: "en".into(),
                content: Some(RichTextDocument::single_paragraph("rewritten body")),
            },
        )
        .await
        .expect("edit should succeed");
    assert_eq!(
        edited.status, "pending",
        "content edit in a moderated category must re-enter moderation"
    );
}

/// M3: votes are frozen while a topic is held by pre-moderation.
#[tokio::test]
async fn pending_topic_rejects_votes() {
    let (db, event_bus, tenant_id) = setup().await;
    let providers = posting_cooldown::zero_cooldown_providers_with(serde_json::json!({
        "pre_moderation_enabled": true
    }));
    let category_service = CategoryService::new(db.clone());
    let topic_service =
        TopicService::new(db.clone(), event_bus).with_settings_providers(providers);
    let votes = VoteService::new(db.clone());

    let author = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let voter = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let category_id = make_category(&category_service, tenant_id, &admin, "held", false).await;
    let topic_id = make_topic(&topic_service, tenant_id, &author, category_id, "held").await;

    let result = votes
        .set_topic_vote(
            tenant_id,
            topic_id,
            voter.clone(),
            write_context(tenant_id, &voter),
            1,
        )
        .await;
    assert!(
        result.is_err(),
        "a pending topic must not collect votes before publication"
    );
}
