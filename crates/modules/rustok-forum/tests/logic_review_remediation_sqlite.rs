//! Regression tests for the 2026-11-02 forum logic-review remediation
//! (`docs/logic-review-2026-11-02.md`): Rust-owned auto-subscribe that never
//! overwrites explicit subscription rows, digest-selection rejection, reply
//! parent/depth rules, edit re-moderation, and the pending-topic vote freeze.

#[path = "support/posting_cooldown.rs"]
mod posting_cooldown;

#[path = "support/delete_policy.rs"]
mod delete_policy;

#[path = "support/edit_policy.rs"]
mod edit_policy;

#[path = "support/moderation_effects.rs"]
mod moderation_effects;

mod support;

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
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection,
    EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set, Statement,
};
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
    let reply_service = ReplyService::new(db.clone(), event_bus).with_settings_providers(providers);
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
    let topic_service = TopicService::new(db.clone(), event_bus).with_settings_providers(providers);
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
    let moderated_id = make_category(&category_service, tenant_id, &admin, "moderated", true).await;
    let moderated_topic = make_topic(
        &topic_service,
        tenant_id,
        &author,
        moderated_id,
        "moderated",
    )
    .await;
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
    assert!(child.is_err(), "a pending reply must not acquire children");

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
    while let Ok(reply) = reply_service
        .create(
            tenant_id,
            author.clone(),
            open_topic,
            reply_input("nested", parent_id),
        )
        .await
    {
        parent_id = Some(reply.id);
        created += 1;
        assert!(
            created <= 32,
            "nesting must terminate at the depth cap, not run away"
        );
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
    let (db, event_bus, _) = setup().await;
    for solution in [false, true] {
        assert_reply_repending(db.clone(), event_bus.clone(), Uuid::new_v4(), solution).await;
    }
}

#[tokio::test]
async fn approved_reply_edit_reconciles_counters_on_postgres() -> support::TestResult<()> {
    let Some(context) = support::postgres::PostgresForumTestDb::setup("reply_repending").await?
    else {
        return Ok(());
    };
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(context.db.clone())));
    for solution in [false, true] {
        assert_reply_repending(
            context.db.clone(),
            event_bus.clone(),
            Uuid::new_v4(),
            solution,
        )
        .await;
    }
    context.cleanup().await?;
    Ok(())
}

async fn assert_reply_repending(
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    tenant_id: Uuid,
    solution: bool,
) {
    let providers = posting_cooldown::zero_cooldown_providers();
    let category_service = CategoryService::new(db.clone());
    let topic_service =
        TopicService::new(db.clone(), event_bus.clone()).with_settings_providers(providers.clone());
    let reply_service =
        ReplyService::new(db.clone(), event_bus.clone()).with_settings_providers(providers);
    let moderation = ModerationService::new(db.clone(), event_bus);

    let author = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let category_id = make_category(&category_service, tenant_id, &admin, "reviewed", true).await;
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

    if solution {
        moderation
            .mark_solution(tenant_id, topic_id, reply.id, admin.clone())
            .await
            .expect("mark solution before editing");
    }
    assert_public_reply_counts(
        &db,
        tenant_id,
        topic_id,
        category_id,
        author.user_id.unwrap(),
        1,
        i32::from(solution),
    )
    .await;

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
    assert_public_reply_counts(
        &db,
        tenant_id,
        topic_id,
        category_id,
        author.user_id.unwrap(),
        0,
        0,
    )
    .await;
    // The same canonical effects must also work on reapproval and an explicit
    // moderation transition, including the author's only accepted reply.
    moderation
        .approve_reply(tenant_id, reply.id, topic_id, admin.clone())
        .await
        .unwrap();
    if solution {
        moderation
            .mark_solution(tenant_id, topic_id, reply.id, admin.clone())
            .await
            .unwrap();
    }
    assert_public_reply_counts(
        &db,
        tenant_id,
        topic_id,
        category_id,
        author.user_id.unwrap(),
        1,
        i32::from(solution),
    )
    .await;
    moderation
        .hide_reply(tenant_id, reply.id, topic_id, admin.clone())
        .await
        .unwrap();
    assert_public_reply_counts(
        &db,
        tenant_id,
        topic_id,
        category_id,
        author.user_id.unwrap(),
        0,
        0,
    )
    .await;
    moderation
        .approve_reply(tenant_id, reply.id, topic_id, admin.clone())
        .await
        .unwrap();
    if solution {
        moderation
            .mark_solution(tenant_id, topic_id, reply.id, admin.clone())
            .await
            .unwrap();
    }
    reply_service
        .delete(tenant_id, reply.id, admin.clone())
        .await
        .unwrap();
    assert_public_reply_counts(
        &db,
        tenant_id,
        topic_id,
        category_id,
        author.user_id.unwrap(),
        0,
        0,
    )
    .await;
    reply_service
        .restore(tenant_id, reply.id, admin.clone())
        .await
        .unwrap();
    assert_public_reply_counts(
        &db,
        tenant_id,
        topic_id,
        category_id,
        author.user_id.unwrap(),
        1,
        i32::from(solution),
    )
    .await;
    let expected_solution =
        rustok_forum::entities::forum_solution::Entity::find_by_id((topic_id, tenant_id))
            .one(&db)
            .await
            .unwrap();
    moderation
        .archive_topic(tenant_id, topic_id, admin.clone())
        .await
        .unwrap();
    // Archive has already released category/member counts. Removing a reply
    // there updates only the archived thread's own count and canonical solution.
    reply_service
        .delete(tenant_id, reply.id, admin.clone())
        .await
        .unwrap();
    assert_public_reply_counts(
        &db,
        tenant_id,
        topic_id,
        category_id,
        author.user_id.unwrap(),
        0,
        0,
    )
    .await;
    let before = reply_restore_checkpoint(&db, tenant_id, topic_id, reply.id).await;
    let rejected = reply_service
        .restore(tenant_id, reply.id, admin.clone())
        .await;
    assert!(
        matches!(rejected, Err(rustok_forum::ForumError::TopicArchived)),
        "archived parent must reject ordinary and accepted reply restoration with a typed conflict: {rejected:?}"
    );
    assert_eq!(
        reply_restore_checkpoint(&db, tenant_id, topic_id, reply.id).await,
        before,
        "rejected restoration must preserve the tombstone, snapshot, aggregates, revisions and outbox"
    );

    moderation
        .reopen_topic(tenant_id, topic_id, admin.clone())
        .await
        .unwrap();
    assert_public_reply_counts(
        &db,
        tenant_id,
        topic_id,
        category_id,
        author.user_id.unwrap(),
        0,
        0,
    )
    .await;
    reply_service
        .restore(tenant_id, reply.id, admin.clone())
        .await
        .unwrap();
    assert_public_reply_counts(
        &db,
        tenant_id,
        topic_id,
        category_id,
        author.user_id.unwrap(),
        1,
        i32::from(solution),
    )
    .await;
    assert_eq!(
        rustok_forum::entities::forum_solution::Entity::find_by_id((topic_id, tenant_id))
            .one(&db)
            .await
            .unwrap(),
        expected_solution,
        "successful retry must restore the original solution marker and timestamp",
    );
    let restored = reply_restore_checkpoint(&db, tenant_id, topic_id, reply.id).await;
    let repeated = reply_service.restore(tenant_id, reply.id, admin).await;
    assert!(matches!(
        repeated,
        Err(rustok_forum::ForumError::ReplyRestoreUnavailable(_))
    ));
    assert_eq!(
        reply_restore_checkpoint(&db, tenant_id, topic_id, reply.id).await,
        restored,
        "repeated restoration must not double-count or emit another event"
    );
}

// Exact persisted checkpoint: a rejected lifecycle inverse must not consume its
// immutable snapshot or publish a success-shaped event before failing.
async fn reply_restore_checkpoint(
    db: &DatabaseConnection,
    tenant: Uuid,
    topic_id: Uuid,
    reply_id: Uuid,
) -> serde_json::Value {
    use rustok_forum::entities::{
        forum_category, forum_reply, forum_reply_body, forum_reply_revision, forum_solution,
        forum_topic, forum_user_stat,
    };
    use rustok_outbox::entity as outbox;

    let topic = forum_topic::Entity::find_by_id(topic_id)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    let category = forum_category::Entity::find_by_id(topic.category_id)
        .one(db)
        .await
        .unwrap();
    let reply = forum_reply::Entity::find_by_id(reply_id)
        .one(db)
        .await
        .unwrap();
    let bodies = forum_reply_body::Entity::find()
        .filter(forum_reply_body::Column::TenantId.eq(tenant))
        .filter(forum_reply_body::Column::ReplyId.eq(reply_id))
        .order_by_asc(forum_reply_body::Column::Locale)
        .all(db)
        .await
        .unwrap();
    let revisions = forum_reply_revision::Entity::find()
        .filter(forum_reply_revision::Column::TenantId.eq(tenant))
        .filter(forum_reply_revision::Column::ReplyId.eq(reply_id))
        .order_by_asc(forum_reply_revision::Column::Id)
        .all(db)
        .await
        .unwrap();
    let stats = forum_user_stat::Entity::find()
        .filter(forum_user_stat::Column::TenantId.eq(tenant))
        .order_by_asc(forum_user_stat::Column::UserId)
        .all(db)
        .await
        .unwrap();
    let solution = forum_solution::Entity::find_by_id((topic_id, tenant))
        .one(db)
        .await
        .unwrap();
    let events = outbox::Entity::find()
        .filter(outbox::Column::TenantId.eq(tenant))
        .count(db)
        .await
        .unwrap();
    // $1/$2 are supported by both PostgreSQL and SQLite. UUIDs stay typed,
    // including SQLite's BLOB representation; never interpolate TEXT UUIDs.
    let snapshot = db.query_one_raw(Statement::from_sql_and_values(
        db.get_database_backend(),
        "SELECT topic_id, author_id, previous_status, solution_marked_by_user_id, solution_marked_at \
         FROM forum_reply_delete_snapshots WHERE tenant_id = $1 AND reply_id = $2",
        vec![tenant.into(), reply_id.into()],
    )).await.unwrap().map(|row| {
        serde_json::json!({
            "topic_id": row.try_get::<Uuid>("", "topic_id").unwrap(),
            "author_id": row.try_get::<Option<Uuid>>("", "author_id").unwrap(),
            "previous_status": row.try_get::<String>("", "previous_status").unwrap(),
            "marked_by": row.try_get::<Option<Uuid>>("", "solution_marked_by_user_id").unwrap(),
            "marked_at": row.try_get::<Option<String>>("", "solution_marked_at").unwrap(),
        })
    });
    serde_json::json!({
        "topic": topic, "category": category, "reply": reply, "bodies": bodies,
        "revisions": revisions, "stats": stats, "solution": solution,
        "events": events, "snapshot": snapshot,
    })
}

#[tokio::test]
async fn archived_topic_snapshot_restores_original_solution_on_sqlite() {
    let (db, events, _) = setup().await;
    for solution in [false, true] {
        assert_archived_topic_snapshot_restore(db.clone(), events.clone(), solution).await;
    }
}

#[tokio::test]
async fn archived_topic_snapshot_restores_original_solution_on_postgres() -> support::TestResult<()>
{
    let Some(context) =
        support::postgres::PostgresForumTestDb::setup("archived_topic_restore").await?
    else {
        return Ok(());
    };
    let events = TransactionalEventBus::new(Arc::new(OutboxTransport::new(context.db.clone())));
    for solution in [false, true] {
        assert_archived_topic_snapshot_restore(context.db.clone(), events.clone(), solution).await;
    }
    context.cleanup().await?;
    Ok(())
}

async fn assert_archived_topic_snapshot_restore(
    db: DatabaseConnection,
    events: TransactionalEventBus,
    solution: bool,
) {
    use rustok_forum::entities::{
        forum_category, forum_reply, forum_solution, forum_topic, forum_user_stat,
    };
    use rustok_forum::state_machine::{ReplyStatus, TopicStatus};

    let tenant = Uuid::new_v4();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let author = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let providers = posting_cooldown::zero_cooldown_providers();
    let topics =
        TopicService::new(db.clone(), events.clone()).with_settings_providers(providers.clone());
    let replies = ReplyService::new(db.clone(), events.clone()).with_settings_providers(providers);
    let moderation = ModerationService::new(db.clone(), events);
    let category = make_category(
        &CategoryService::new(db.clone()),
        tenant,
        &admin,
        "restore-archive",
        false,
    )
    .await;
    let topic = make_topic(&topics, tenant, &admin, category, "restore-archive").await;
    let reply = replies
        .create(
            tenant,
            author.clone(),
            topic,
            reply_input("original answer", None),
        )
        .await
        .unwrap();
    // A reply deleted independently must not be revived by the topic's inverse.
    let independent = replies
        .create(
            tenant,
            author.clone(),
            topic,
            reply_input("previously deleted", None),
        )
        .await
        .unwrap();
    replies
        .delete(tenant, independent.id, admin.clone())
        .await
        .unwrap();
    if solution {
        moderation
            .mark_solution(tenant, topic, reply.id, admin.clone())
            .await
            .unwrap();
    }
    let expected_solution = forum_solution::Entity::find_by_id((topic, tenant))
        .one(&db)
        .await
        .unwrap();
    moderation
        .archive_topic(tenant, topic, admin.clone())
        .await
        .unwrap();
    topics.delete(tenant, topic, admin.clone()).await.unwrap();
    assert_eq!(
        forum_reply::Entity::find_by_id(reply.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .status,
        ReplyStatus::Deleted
    );
    assert!(
        forum_solution::Entity::find_by_id((topic, tenant))
            .one(&db)
            .await
            .unwrap()
            .is_none()
    );

    let publication_events = rustok_outbox::entity::Entity::find()
        .filter(rustok_outbox::entity::Column::TenantId.eq(tenant))
        .filter(
            rustok_outbox::entity::Column::EventType
                .is_in(["forum.topic.created", "forum.topic.replied"]),
        );
    let publications_before = publication_events.clone().count(&db).await.unwrap();
    topics.restore(tenant, topic, admin.clone()).await
        .expect("an already-archived topic must restore its original solution, not fail the active-target guard");
    assert_eq!(
        publication_events.count(&db).await.unwrap(),
        publications_before,
        "restoring an archived thread must not publish it again"
    );
    let restored_topic = forum_topic::Entity::find_by_id(topic)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(restored_topic.status, TopicStatus::Archived);
    assert!(
        restored_topic.is_locked,
        "restore must not silently reopen or unlock an archived topic"
    );
    assert_eq!(restored_topic.reply_count, 1);
    assert!(restored_topic.last_reply_at.is_some());
    assert_eq!(
        forum_reply::Entity::find_by_id(reply.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .status,
        ReplyStatus::Approved
    );
    assert_eq!(
        forum_reply::Entity::find_by_id(independent.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .status,
        ReplyStatus::Deleted
    );
    assert_eq!(
        forum_solution::Entity::find_by_id((topic, tenant))
            .one(&db)
            .await
            .unwrap(),
        expected_solution,
        "archived snapshot replay must preserve solution identity, marker and timestamp"
    );
    let category_row = forum_category::Entity::find_by_id(category)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!((category_row.topic_count, category_row.reply_count), (0, 0));
    for stats in forum_user_stat::Entity::find()
        .filter(forum_user_stat::Column::TenantId.eq(tenant))
        .all(&db)
        .await
        .unwrap()
    {
        assert_eq!(
            (stats.topic_count, stats.reply_count, stats.solution_count),
            (0, 0, 0),
            "archived snapshot restoration contributes no public author statistics"
        );
    }
    let snapshots = db.query_one_raw(Statement::from_sql_and_values(db.get_database_backend(),
        "SELECT (SELECT COUNT(*) FROM forum_topic_delete_snapshots WHERE tenant_id = $1 AND topic_id = $2) + \
                (SELECT COUNT(*) FROM forum_topic_reply_delete_snapshots WHERE tenant_id = $1 AND topic_id = $2) AS count",
        vec![tenant.into(), topic.into()],
    )).await.unwrap().unwrap().try_get::<i64>("", "count").unwrap();
    assert_eq!(
        snapshots, 0,
        "successful restore must consume the topic replay admission"
    );
    let checkpoint = reply_restore_checkpoint(&db, tenant, topic, reply.id).await;
    assert!(matches!(
        topics.restore(tenant, topic, admin.clone()).await,
        Err(rustok_forum::ForumError::TopicRestoreUnavailable(_))
    ));
    assert_eq!(
        reply_restore_checkpoint(&db, tenant, topic, reply.id).await,
        checkpoint
    );
    moderation.reopen_topic(tenant, topic, admin).await.unwrap();
    assert_public_reply_counts(
        &db,
        tenant,
        topic,
        category,
        author.user_id.unwrap(),
        1,
        i32::from(solution),
    )
    .await;
    assert_eq!(
        forum_category::Entity::find_by_id(category)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .topic_count,
        1,
        "explicit reopen adds the topic contribution exactly once"
    );
}

async fn assert_public_reply_counts(
    db: &DatabaseConnection,
    tenant: Uuid,
    topic_id: Uuid,
    category_id: Uuid,
    author_id: Uuid,
    replies: i32,
    solutions: i32,
) {
    use rustok_forum::entities::{forum_category, forum_solution, forum_topic, forum_user_stat};
    let topic = forum_topic::Entity::find_by_id(topic_id)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    let category = forum_category::Entity::find_by_id(category_id)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    let stats = forum_user_stat::Entity::find_by_id((tenant, author_id))
        .one(db)
        .await
        .unwrap()
        .unwrap();
    let accepted = forum_solution::Entity::find_by_id((topic_id, tenant))
        .one(db)
        .await
        .unwrap();
    assert_eq!(topic.reply_count, replies, "topic counter");
    assert_eq!(category.reply_count, replies, "category counter");
    assert_eq!(stats.reply_count, replies, "author counter");
    assert_eq!(stats.solution_count, solutions, "solution counter");
    assert_eq!(accepted.is_some(), solutions > 0, "canonical solution");
    assert_eq!(
        topic.last_reply_at.is_some(),
        replies > 0,
        "last reply timestamp"
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
    let topic_service = TopicService::new(db.clone(), event_bus).with_settings_providers(providers);
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

#[tokio::test]
async fn relation_edits_obey_parent_lifecycle_and_leave_no_revision_on_rejection() {
    use rustok_forum::entities::forum_relation_revision;
    use rustok_forum::{ForumError, ForumQuoteCommandService, SetForumQuotesInput};
    use sea_orm::{ColumnTrait, PaginatorTrait, QueryFilter};

    let (db, event_bus, tenant) = setup().await;
    let providers = posting_cooldown::zero_cooldown_providers();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let categories = CategoryService::new(db.clone());
    let topics =
        TopicService::new(db.clone(), event_bus.clone()).with_settings_providers(providers.clone());
    let replies =
        ReplyService::new(db.clone(), event_bus.clone()).with_settings_providers(providers.clone());
    let moderation = ModerationService::new(db.clone(), event_bus);
    let quotes = ForumQuoteCommandService::new(db.clone()).with_settings_providers(providers);
    let category = make_category(&categories, tenant, &admin, "relation-guards", false).await;
    let topic = make_topic(&topics, tenant, &admin, category, "relation-guards").await;
    let reply = replies
        .create(tenant, admin.clone(), topic, reply_input("answer", None))
        .await
        .unwrap();
    let input = || SetForumQuotesInput {
        locale: "en".into(),
        quotes: vec![],
    };
    let before = forum_relation_revision::Entity::find()
        .filter(forum_relation_revision::Column::TenantId.eq(tenant))
        .count(&db)
        .await
        .unwrap();

    moderation
        .lock_topic(tenant, topic, admin.clone())
        .await
        .unwrap();
    assert!(matches!(
        quotes
            .set_reply_quotes(tenant, reply.id, admin.clone(), input())
            .await,
        Err(ForumError::TopicLocked)
    ));
    moderation
        .unlock_topic(tenant, topic, admin.clone())
        .await
        .unwrap();
    quotes
        .set_reply_quotes(tenant, reply.id, admin.clone(), input())
        .await
        .unwrap();
    moderation
        .archive_topic(tenant, topic, admin.clone())
        .await
        .unwrap();
    assert!(matches!(
        quotes
            .set_topic_quotes(tenant, topic, admin.clone(), input())
            .await,
        Err(ForumError::TopicArchived)
    ));
    assert!(matches!(
        quotes
            .set_reply_quotes(tenant, reply.id, admin.clone(), input())
            .await,
        Err(ForumError::TopicArchived)
    ));
    assert!(matches!(
        topics
            .rename_slug(
                tenant,
                topic,
                admin,
                rustok_forum::RenameForumTopicSlugInput {
                    locale: "en".into(),
                    slug: "forbidden".into()
                }
            )
            .await,
        Err(ForumError::TopicArchived)
    ));
    let after = forum_relation_revision::Entity::find()
        .filter(forum_relation_revision::Column::TenantId.eq(tenant))
        .count(&db)
        .await
        .unwrap();
    assert_eq!(
        before, after,
        "rejected or unchanged relation writes add no revision"
    );
}

#[tokio::test]
async fn slug_and_standalone_quotes_obey_the_configured_author_edit_window() {
    use rustok_forum::{
        ForumError, ForumQuoteCommandService, RenameForumTopicSlugInput, SetForumQuotesInput,
    };
    let (db, event_bus, tenant) = setup().await;
    let providers = posting_cooldown::zero_cooldown_providers_with(
        serde_json::json!({"max_edit_window_minutes": 10}),
    );
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let author = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let topics =
        TopicService::new(db.clone(), event_bus.clone()).with_settings_providers(providers.clone());
    let replies =
        ReplyService::new(db.clone(), event_bus).with_settings_providers(providers.clone());
    let quotes = ForumQuoteCommandService::new(db.clone()).with_settings_providers(providers);
    let category = make_category(
        &CategoryService::new(db.clone()),
        tenant,
        &admin,
        "edit-window",
        false,
    )
    .await;
    let topic = make_topic(&topics, tenant, &author, category, "edit-window").await;
    let reply = replies
        .create(tenant, author.clone(), topic, reply_input("answer", None))
        .await
        .unwrap();
    // SeaORM binds UUIDs as BLOBs on SQLite. Text interpolation would match
    // no rows and leave this regression inside the edit window.
    let old = chrono::DateTime::parse_from_rfc3339("2020-01-01T00:00:00+00:00").unwrap();
    use rustok_forum::entities::{forum_reply, forum_topic};
    let mut old_topic: forum_topic::ActiveModel = forum_topic::Entity::find_by_id(topic)
        .one(&db)
        .await
        .unwrap()
        .unwrap()
        .into();
    old_topic.created_at = Set(old);
    let old_topic = old_topic.update(&db).await.unwrap();
    assert_eq!(old_topic.created_at, old);
    let mut old_reply: forum_reply::ActiveModel = forum_reply::Entity::find_by_id(reply.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap()
        .into();
    old_reply.created_at = Set(old);
    let old_reply = old_reply.update(&db).await.unwrap();
    assert_eq!(old_reply.created_at, old);
    let rename = || RenameForumTopicSlugInput {
        locale: "en".into(),
        slug: "new-slug".into(),
    };
    let input = || SetForumQuotesInput {
        locale: "en".into(),
        quotes: vec![],
    };
    for result in [
        quotes
            .set_topic_quotes(tenant, topic, author.clone(), input())
            .await
            .map(|_| ()),
        quotes
            .set_reply_quotes(tenant, reply.id, author.clone(), input())
            .await
            .map(|_| ()),
        topics
            .rename_slug(tenant, topic, author, rename())
            .await
            .map(|_| ()),
    ] {
        assert!(
            matches!(result, Err(ForumError::Validation(ref message)) if message.contains("edit window has closed"))
        );
    }
    // Moderation scope, not update scope or a hard-coded role name, is the exemption.
    topics
        .rename_slug(tenant, topic, admin.clone(), rename())
        .await
        .unwrap();
    quotes
        .set_topic_quotes(tenant, topic, admin.clone(), input())
        .await
        .unwrap();
    quotes
        .set_reply_quotes(tenant, reply.id, admin, input())
        .await
        .unwrap();
}

#[tokio::test]
async fn publication_counter_migration_repairs_prior_drift_without_changing_content() {
    let (db, event_bus, tenant) = setup().await;
    assert_publication_counter_repair(db, event_bus, tenant).await;
}

#[tokio::test]
async fn publication_counter_migration_preserves_tombstones_on_postgres() -> support::TestResult<()>
{
    let Some(context) = support::postgres::PostgresForumTestDb::setup("publication_repair").await?
    else {
        return Ok(());
    };
    let events = TransactionalEventBus::new(Arc::new(OutboxTransport::new(context.db.clone())));
    assert_publication_counter_repair(context.db.clone(), events, Uuid::new_v4()).await;
    context.cleanup().await?;
    Ok(())
}

async fn assert_publication_counter_repair(
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    tenant: Uuid,
) {
    use rustok_forum::ReplyStatus;
    use rustok_forum::entities::{forum_reply, forum_topic, forum_topic_revision};
    let providers = posting_cooldown::zero_cooldown_providers();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let topics =
        TopicService::new(db.clone(), event_bus.clone()).with_settings_providers(providers.clone());
    let replies = ReplyService::new(db.clone(), event_bus).with_settings_providers(providers);
    let category = make_category(
        &CategoryService::new(db.clone()),
        tenant,
        &admin,
        "repair",
        false,
    )
    .await;
    let topic = make_topic(&topics, tenant, &admin, category, "repair").await;
    let reply = replies
        .create(
            tenant,
            admin.clone(),
            topic,
            reply_input("preserved body", None),
        )
        .await
        .unwrap();
    assert_public_reply_counts(&db, tenant, topic, category, admin.user_id.unwrap(), 1, 0).await;
    // A production installation may already contain immutable tombstones. The
    // repair must skip those rows, not disable their guards to update all topics.
    let deleted_topic =
        make_topic(&topics, tenant, &admin, category, "deleted-before-repair").await;
    topics
        .delete(tenant, deleted_topic, admin.clone())
        .await
        .unwrap();
    let tombstone = forum_topic::Entity::find_by_id(deleted_topic)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let revisions = forum_topic_revision::Entity::find()
        .filter(forum_topic_revision::Column::TenantId.eq(tenant))
        .filter(forum_topic_revision::Column::TopicId.eq(deleted_topic))
        .order_by_asc(forum_topic_revision::Column::Id)
        .all(&db)
        .await
        .unwrap();
    assert!(
        !revisions.is_empty(),
        "fixture must exercise a real delete history"
    );

    // Reproduce the previous status/counter ordering with a typed UUID binding:
    // status changes after the aggregate write, leaving a stale last_reply_at.
    let mut pending: forum_reply::ActiveModel = forum_reply::Entity::find_by_id(reply.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap()
        .into();
    pending.status = Set(ReplyStatus::Pending);
    let before = pending.update(&db).await.unwrap();
    assert_eq!(before.status, ReplyStatus::Pending);
    let migration = ForumModule
        .migrations()
        .into_iter()
        .find(|migration| {
            migration.name() == "m20261126_000046_reconcile_reply_publication_counters"
        })
        .unwrap();
    for _ in 0..2 {
        migration.up(&SchemaManager::new(&db)).await.unwrap();
        assert_public_reply_counts(&db, tenant, topic, category, admin.user_id.unwrap(), 0, 0)
            .await;
        assert_eq!(
            forum_reply::Entity::find_by_id(reply.id)
                .one(&db)
                .await
                .unwrap()
                .unwrap(),
            before
        );
        assert_eq!(
            forum_topic::Entity::find_by_id(deleted_topic)
                .one(&db)
                .await
                .unwrap()
                .unwrap(),
            tombstone
        );
        assert_eq!(
            forum_topic_revision::Entity::find()
                .filter(forum_topic_revision::Column::TenantId.eq(tenant))
                .filter(forum_topic_revision::Column::TopicId.eq(deleted_topic))
                .order_by_asc(forum_topic_revision::Column::Id)
                .all(&db)
                .await
                .unwrap(),
            revisions
        );
    }
    // Skipping tombstones is not permission to weaken their immutability.
    let mut forbidden: forum_topic::ActiveModel = tombstone.into();
    forbidden.updated_at = Set(chrono::Utc::now().into());
    assert!(forbidden.update(&db).await.is_err());
}

#[tokio::test]
async fn sqlite_category_counters_exclude_archived_threads_and_restore_them_once() {
    use rustok_forum::entities::forum_category;
    let (db, event_bus, tenant) = setup().await;
    let providers = posting_cooldown::zero_cooldown_providers();
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));
    let topics =
        TopicService::new(db.clone(), event_bus.clone()).with_settings_providers(providers.clone());
    let replies =
        ReplyService::new(db.clone(), event_bus.clone()).with_settings_providers(providers);
    let moderation = ModerationService::new(db.clone(), event_bus);
    let category = make_category(
        &CategoryService::new(db.clone()),
        tenant,
        &admin,
        "archive-count",
        false,
    )
    .await;
    let topic = make_topic(&topics, tenant, &admin, category, "archive-count").await;
    replies
        .create(tenant, admin.clone(), topic, reply_input("answer", None))
        .await
        .unwrap();
    moderation
        .archive_topic(tenant, topic, admin.clone())
        .await
        .unwrap();
    let archived = forum_category::Entity::find_by_id(category)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!((archived.topic_count, archived.reply_count), (0, 0));
    moderation
        .reopen_topic(tenant, topic, admin.clone())
        .await
        .unwrap();
    let restored = forum_category::Entity::find_by_id(category)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!((restored.topic_count, restored.reply_count), (1, 1));
}
