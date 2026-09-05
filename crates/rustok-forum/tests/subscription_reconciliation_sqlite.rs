use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    ForumModule, ForumSubscriptionDriftKind,
    ForumSubscriptionReconciliationService, ForumSubscriptionTargetKind,
};
use rustok_outbox::OutboxModule;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup_db() -> DatabaseConnection {
    let mut opt = ConnectOptions::new("sqlite::memory:".to_string());
    opt.max_connections(1);
    let db = Database::connect(opt)
        .await
        .expect("in-memory sqlite should connect");

    let manager = SchemaManager::new(&db);
    for migration in OutboxModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("outbox migration should apply");
    }
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&manager)
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
    for migration in ForumModule.migrations() {
        migration
            .up(&manager)
            .await
            .expect("forum migration should apply");
    }

    db
}

fn sql_uuid(id: Uuid) -> String {
    format!("X'{}'", id.simple().to_string().to_uppercase())
}

async fn seed_category(db: &DatabaseConnection, tenant_id: Uuid, category_id: Uuid) {
    db.execute_unprepared(&format!(
        "INSERT INTO forum_categories \
         (id, tenant_id, position, moderated, topic_count, reply_count) \
         VALUES ({}, {}, 0, 0, 0, 0)",
        sql_uuid(category_id),
        sql_uuid(tenant_id),
    ))
    .await
    .expect("category seed should succeed");
}

async fn seed_topic(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
    topic_id: Uuid,
    author_id: Uuid,
) {
    db.execute_unprepared(&format!(
        "INSERT INTO forum_topics \
         (id, tenant_id, category_id, author_id, status, metadata, is_pinned, is_locked, reply_count) \
         VALUES ({}, {}, {}, {}, 'open', '{{}}', 0, 0, 0); \
         INSERT INTO forum_topic_translations \
         (id, topic_id, tenant_id, locale, title, slug, body) \
         VALUES ({}, {}, {}, 'en', 'Topic', 'topic-{topic_id}', 'Topic body')",
        sql_uuid(topic_id),
        sql_uuid(tenant_id),
        sql_uuid(category_id),
        sql_uuid(author_id),
        sql_uuid(Uuid::new_v4()),
        sql_uuid(topic_id),
        sql_uuid(tenant_id),
    ))
    .await
    .expect("topic seed should succeed");
}

async fn seed_topic_subscription(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
    user_id: Uuid,
    level: &str,
    notify_mentions: i64,
    notify_replies: i64,
    notify_new_topics: i64,
    digest_mode: &str,
    revision: i64,
) {
    db.execute_unprepared(&format!(
        "INSERT INTO forum_topic_subscriptions \
         (topic_id, user_id, tenant_id, level, notify_mentions, notify_replies, notify_new_topics, digest_mode, revision, created_at, updated_at) \
         VALUES ({}, {}, {}, '{}', {}, {}, {}, '{}', {}, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP) \
         ON CONFLICT (tenant_id, topic_id, user_id) DO UPDATE SET \
           level = excluded.level, notify_mentions = excluded.notify_mentions, \
           notify_replies = excluded.notify_replies, notify_new_topics = excluded.notify_new_topics, \
           digest_mode = excluded.digest_mode, revision = forum_topic_subscriptions.revision + 1, \
           updated_at = CURRENT_TIMESTAMP",
        sql_uuid(topic_id),
        sql_uuid(user_id),
        sql_uuid(tenant_id),
        level,
        notify_mentions,
        notify_replies,
        notify_new_topics,
        digest_mode,
        revision,
    ))
    .await
    .expect("topic subscription seed should succeed");
}

async fn seed_category_subscription(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
    user_id: Uuid,
    level: &str,
    notify_mentions: i64,
    notify_replies: i64,
    notify_new_topics: i64,
    digest_mode: &str,
    revision: i64,
) {
    db.execute_unprepared(&format!(
        "INSERT INTO forum_category_subscriptions \
         (category_id, user_id, tenant_id, level, notify_mentions, notify_replies, notify_new_topics, digest_mode, revision, created_at, updated_at) \
         VALUES ({}, {}, {}, '{}', {}, {}, {}, '{}', {}, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP) \
         ON CONFLICT (tenant_id, category_id, user_id) DO UPDATE SET \
           level = excluded.level, notify_mentions = excluded.notify_mentions, \
           notify_replies = excluded.notify_replies, notify_new_topics = excluded.notify_new_topics, \
           digest_mode = excluded.digest_mode, revision = forum_category_subscriptions.revision + 1, \
           updated_at = CURRENT_TIMESTAMP",
        sql_uuid(category_id),
        sql_uuid(user_id),
        sql_uuid(tenant_id),
        level,
        notify_mentions,
        notify_replies,
        notify_new_topics,
        digest_mode,
        revision,
    ))
    .await
    .expect("category subscription seed should succeed");
}

#[tokio::test]
async fn subscription_reconciliation_requires_manage_permissions() {
    let db = setup_db().await;
    let service = ForumSubscriptionReconciliationService::new(db);
    let tenant_id = Uuid::new_v4();

    // 1. Customer rejected
    let customer = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let err = service
        .report_page(tenant_id, &customer, None, None, None, None, None)
        .await
        .expect_err("customer must be rejected");
    assert!(matches!(err, rustok_forum::ForumError::Forbidden(_)));

    // 2. Anonymous rejected
    let anonymous = SecurityContext::new(UserRole::Customer, None);
    let err = service
        .report_page(tenant_id, &anonymous, None, None, None, None, None)
        .await
        .expect_err("anonymous must be rejected");
    assert!(matches!(err, rustok_forum::ForumError::Forbidden(_)));

    // 3. Admin succeeds
    let admin = SecurityContext::system();
    let report = service
        .report_page(tenant_id, &admin, None, None, None, None, None)
        .await
        .expect("admin should succeed");
    assert!(report.is_clean());
}

#[tokio::test]
async fn subscription_reconciliation_detects_clean_state_and_all_drifts_sqlite() {
    let db = setup_db().await;
    let tenant_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let user2 = Uuid::new_v4();
    let cat_id = Uuid::new_v4();
    let top_id = Uuid::new_v4();

    seed_category(&db, tenant_id, cat_id).await;
    // seed_topic automatically creates a watching subscription for author_id with revision 1
    seed_topic(&db, tenant_id, cat_id, top_id, author_id).await;
    seed_category_subscription(
        &db, tenant_id, cat_id, user2, "normal", 1, 0, 0, "disabled", 1,
    )
    .await;

    let service = ForumSubscriptionReconciliationService::new(db.clone());
    let admin = SecurityContext::system();

    // 1. Clean check
    let clean = service
        .report_page(tenant_id, &admin, None, None, None, None, None)
        .await
        .expect("clean report");
    assert!(clean.is_clean());
    assert_eq!(clean.drift_count(), 0);
    assert_eq!(clean.inspected_topic_subscriptions, 1);
    assert_eq!(clean.inspected_category_subscriptions, 1);

    // 2. TargetMissing drift: insert subscription pointing to non-existent topic
    db.execute_unprepared("PRAGMA foreign_keys = OFF;").await.unwrap();
    db.execute_unprepared("DROP TRIGGER IF EXISTS forum_topic_subscriptions_tenant_insert;").await.unwrap();
    let missing_top_id = Uuid::new_v4();
    seed_topic_subscription(
        &db, tenant_id, missing_top_id, user2, "watching", 1, 1, 1, "immediate", 1,
    )
    .await;

    let drift1 = service
        .report_page(tenant_id, &admin, None, None, None, None, None)
        .await
        .expect("drift report 1");
    let d1 = drift1
        .drifts
        .iter()
        .find(|d| d.kind == ForumSubscriptionDriftKind::TargetMissing)
        .expect("TargetMissing should be detected");
    assert_eq!(d1.target_kind, ForumSubscriptionTargetKind::Topic);
    assert_eq!(d1.target_id, missing_top_id);
    assert_eq!(d1.user_id, user2);

    // Clean up missing topic sub
    db.execute_unprepared(&format!(
        "DELETE FROM forum_topic_subscriptions WHERE topic_id = {} AND user_id = {}",
        sql_uuid(missing_top_id),
        sql_uuid(user2)
    ))
    .await
    .unwrap();

    // 3. MergedTopicSourceSubscription drift: topic has an entry in forum_topic_merge_operations as source
    db.execute_unprepared("DROP TRIGGER IF EXISTS forum_05_topic_merge_redirect_edge;").await.unwrap();
    let op_id = Uuid::new_v4();
    db.execute_unprepared(&format!(
        "INSERT INTO forum_topic_merge_operations \
         (tenant_id, operation_id, source_topic_id, target_topic_id, category_id, actor_id, reason, moved_reply_count, moved_published_reply_count, resulting_published_reply_count, position_offset, event_id, merged_at) \
         VALUES ({}, {}, {}, {}, {}, {}, 'merge reason', 0, 0, 0, 0, {}, CURRENT_TIMESTAMP)",
        sql_uuid(tenant_id),
        sql_uuid(op_id),
        sql_uuid(top_id),
        sql_uuid(Uuid::new_v4()),
        sql_uuid(cat_id),
        sql_uuid(author_id),
        sql_uuid(op_id),
    ))
    .await
    .unwrap();

    let drift2 = service
        .report_page(tenant_id, &admin, None, None, None, None, None)
        .await
        .expect("drift report 2");
    let d2 = drift2
        .drifts
        .iter()
        .find(|d| d.kind == ForumSubscriptionDriftKind::MergedTopicSourceSubscription)
        .expect("MergedTopicSourceSubscription should be detected");
    assert_eq!(d2.target_id, top_id);

    // Remove merge operation
    db.execute_unprepared("DROP TRIGGER IF EXISTS forum_topic_merge_operation_delete;").await.unwrap();
    db.execute_unprepared(&format!(
        "DELETE FROM forum_topic_merge_operations WHERE tenant_id = {} AND source_topic_id = {}",
        sql_uuid(tenant_id),
        sql_uuid(top_id)
    ))
    .await
    .unwrap();

    // 4. MutedPreferencesInvalid drift:
    // Drop validation triggers to simulate drifted state
    db.execute_unprepared("DROP TRIGGER IF EXISTS forum_validate_category_subscription_update;").await.unwrap();
    db.execute_unprepared(&format!(
        "UPDATE forum_category_subscriptions SET level = 'muted', notify_replies = 1 \
         WHERE category_id = {} AND user_id = {}",
        sql_uuid(cat_id),
        sql_uuid(user2)
    ))
    .await
    .unwrap();

    let drift3 = service
        .report_page(tenant_id, &admin, None, None, None, None, None)
        .await
        .expect("drift report 3");
    let d3 = drift3
        .drifts
        .iter()
        .find(|d| d.kind == ForumSubscriptionDriftKind::MutedPreferencesInvalid)
        .expect("MutedPreferencesInvalid should be detected");
    assert_eq!(d3.target_kind, ForumSubscriptionTargetKind::Category);
    assert_eq!(d3.target_id, cat_id);

    // Restore category subscription
    db.execute_unprepared(&format!(
        "UPDATE forum_category_subscriptions SET level = 'normal', notify_replies = 0 \
         WHERE category_id = {} AND user_id = {}",
        sql_uuid(cat_id),
        sql_uuid(user2)
    ))
    .await
    .unwrap();

    // 5. RevisionInvalid drift: revision <= 0
    db.execute_unprepared("DROP TRIGGER IF EXISTS forum_validate_topic_subscription_update;").await.unwrap();
    db.execute_unprepared(&format!(
        "UPDATE forum_topic_subscriptions SET revision = 0 \
         WHERE topic_id = {} AND user_id = {}",
        sql_uuid(top_id),
        sql_uuid(author_id)
    ))
    .await
    .unwrap();

    let drift4 = service
        .report_page(tenant_id, &admin, None, None, None, None, None)
        .await
        .expect("drift report 4");
    let d4 = drift4
        .drifts
        .iter()
        .find(|d| d.kind == ForumSubscriptionDriftKind::RevisionInvalid)
        .expect("RevisionInvalid should be detected");
    assert_eq!(d4.target_kind, ForumSubscriptionTargetKind::Topic);
    assert_eq!(d4.target_id, top_id);
    assert_eq!(d4.stored, 0);
}

#[tokio::test]
async fn subscription_reconciliation_pagination_and_composite_cursors_sqlite() {
    let db = setup_db().await;
    let tenant_id = Uuid::new_v4();
    let cat_id = Uuid::new_v4();
    seed_category(&db, tenant_id, cat_id).await;

    // Seed 3 topic subscriptions and 3 category subscriptions
    for _ in 0..3 {
        let user_id = Uuid::new_v4();
        let top_id = Uuid::new_v4();
        seed_topic(&db, tenant_id, cat_id, top_id, user_id).await;
        // topic auto-subscribes user_id.
        seed_category_subscription(
            &db, tenant_id, cat_id, user_id, "watching", 1, 1, 1, "immediate", 1,
        )
        .await;
    }

    let service = ForumSubscriptionReconciliationService::new(db);
    let admin = SecurityContext::system();

    // Page 1: limit 2
    let page1 = service
        .report_page(tenant_id, &admin, Some(2), None, None, None, None)
        .await
        .expect("page 1 should succeed");
    assert_eq!(page1.effective_limit, 2);
    assert_eq!(page1.inspected_topic_subscriptions, 2);
    assert_eq!(page1.inspected_category_subscriptions, 2);
    assert!(page1.has_more_topic_subscriptions);
    assert!(page1.has_more_category_subscriptions);
    assert!(page1.topic_cursor.is_some());
    assert!(page1.category_cursor.is_some());

    let top_cursor = page1.topic_cursor.unwrap();
    let cat_cursor = page1.category_cursor.unwrap();

    // Page 2: continue with cursors
    let page2 = service
        .report_page(
            tenant_id,
            &admin,
            Some(2),
            Some(top_cursor.target_id),
            Some(top_cursor.user_id),
            Some(cat_cursor.target_id),
            Some(cat_cursor.user_id),
        )
        .await
        .expect("page 2 should succeed");
    assert_eq!(page2.inspected_topic_subscriptions, 1);
    assert_eq!(page2.inspected_category_subscriptions, 1);
    assert!(!page2.has_more_topic_subscriptions);
    assert!(!page2.has_more_category_subscriptions);
}
