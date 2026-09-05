use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    ForumCounterDriftKind, ForumCounterReconciliationService, ForumModule,
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

async fn seed_category(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    category_id: Uuid,
    topic_count: i64,
    reply_count: i64,
) {
    db.execute_unprepared(&format!(
        "INSERT INTO forum_categories \
         (id, tenant_id, position, moderated, topic_count, reply_count) \
         VALUES ({}, {}, 0, 0, {}, {})",
        sql_uuid(category_id),
        sql_uuid(tenant_id),
        topic_count,
        reply_count,
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
    reply_count: i64,
) {
    db.execute_unprepared(&format!(
        "INSERT INTO forum_topics \
         (id, tenant_id, category_id, author_id, status, metadata, is_pinned, is_locked, reply_count) \
         VALUES ({}, {}, {}, {}, 'open', '{{}}', 0, 0, {}); \
         INSERT INTO forum_topic_translations \
         (id, topic_id, tenant_id, locale, title, slug, body) \
         VALUES ({}, {}, {}, 'en', 'Topic', 'topic-{topic_id}', 'Topic body')",
        sql_uuid(topic_id),
        sql_uuid(tenant_id),
        sql_uuid(category_id),
        sql_uuid(author_id),
        reply_count,
        sql_uuid(Uuid::new_v4()),
        sql_uuid(topic_id),
        sql_uuid(tenant_id),
    ))
    .await
    .expect("topic seed should succeed");
}

async fn seed_reply(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
    reply_id: Uuid,
    author_id: Uuid,
    status: &str,
    position: i64,
) {
    db.execute_unprepared(&format!(
        "INSERT INTO forum_replies \
         (id, tenant_id, topic_id, author_id, status, position) \
         VALUES ({}, {}, {}, {}, '{}', {}); \
         INSERT INTO forum_reply_bodies \
         (id, reply_id, tenant_id, locale, body) \
         VALUES ({}, {}, {}, 'en', 'Reply body')",
        sql_uuid(reply_id),
        sql_uuid(tenant_id),
        sql_uuid(topic_id),
        sql_uuid(author_id),
        status,
        position,
        sql_uuid(Uuid::new_v4()),
        sql_uuid(reply_id),
        sql_uuid(tenant_id),
    ))
    .await
    .expect("reply seed should succeed");
}

#[tokio::test]
async fn counter_reconciliation_detects_clean_state_and_all_drifts_sqlite() {
    let db = setup_db().await;
    let tenant_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let cat1 = Uuid::new_v4();
    let cat2 = Uuid::new_v4();
    let top1 = Uuid::new_v4();
    let top2 = Uuid::new_v4();
    let top3 = Uuid::new_v4();

    // Cat1 has Top1 and Top2. Top1 has 2 approved replies. Top2 has 1 approved and 1 hidden.
    // So for Cat1: expected topic_count = 2, expected reply_count = 3 (2 + 1).
    // For Top1: expected reply_count = 2.
    // For Top2: expected reply_count = 1.
    seed_category(&db, tenant_id, cat1, 2, 3).await;
    seed_topic(&db, tenant_id, cat1, top1, author_id, 2).await;
    seed_topic(&db, tenant_id, cat1, top2, author_id, 1).await;

    seed_reply(&db, tenant_id, top1, Uuid::new_v4(), author_id, "approved", 1).await;
    seed_reply(&db, tenant_id, top1, Uuid::new_v4(), author_id, "approved", 2).await;
    seed_reply(&db, tenant_id, top2, Uuid::new_v4(), author_id, "approved", 1).await;
    seed_reply(&db, tenant_id, top2, Uuid::new_v4(), author_id, "hidden", 2).await;

    // Cat2 has Top3. Top3 has 0 replies.
    // For Cat2: expected topic_count = 1, expected reply_count = 0.
    seed_category(&db, tenant_id, cat2, 1, 0).await;
    seed_topic(&db, tenant_id, cat2, top3, author_id, 0).await;

    let service = ForumCounterReconciliationService::new(db.clone());
    let admin = SecurityContext::system();

    // 1. Clean verification
    let clean_report = service
        .report(tenant_id, &admin, None)
        .await
        .expect("clean report should succeed");
    assert!(clean_report.is_clean());
    assert_eq!(clean_report.drift_count(), 0);
    assert_eq!(clean_report.inspected_topics, 3);
    assert_eq!(clean_report.inspected_categories, 2);
    assert!(!clean_report.has_more_topics);
    assert!(!clean_report.has_more_categories);

    // Drop self-correcting triggers so we can simulate drifted state
    db.execute_unprepared("DROP TRIGGER IF EXISTS forum_topics_public_reply_count_update;").await.unwrap();
    db.execute_unprepared("DROP TRIGGER IF EXISTS forum_categories_public_reply_count_update;").await.unwrap();

    // 2. Introduce TopicReplyCount drift on top1: stored = 99, expected = 2
    db.execute_unprepared(&format!(
        "UPDATE forum_topics SET reply_count = 99 WHERE id = {}",
        sql_uuid(top1)
    ))
    .await
    .expect("update should succeed");

    let drift_report1 = service
        .report(tenant_id, &admin, None)
        .await
        .expect("drift report should succeed");
    assert!(!drift_report1.is_clean());
    assert_eq!(drift_report1.drift_count(), 1);
    let drift = &drift_report1.drifts[0];
    assert_eq!(drift.kind, ForumCounterDriftKind::TopicReplyCount);
    assert_eq!(drift.subject_id, top1);
    assert_eq!(drift.stored, 99);
    assert_eq!(drift.expected, 2);

    // Restore top1
    db.execute_unprepared(&format!(
        "UPDATE forum_topics SET reply_count = 2 WHERE id = {}",
        sql_uuid(top1)
    ))
    .await
    .expect("restore should succeed");

    // 3. Introduce CategoryTopicCount drift on cat1: stored = 10, expected = 2
    db.execute_unprepared(&format!(
        "UPDATE forum_categories SET topic_count = 10 WHERE id = {}",
        sql_uuid(cat1)
    ))
    .await
    .expect("update should succeed");

    let drift_report2 = service
        .report(tenant_id, &admin, None)
        .await
        .expect("drift report should succeed");
    assert_eq!(drift_report2.drift_count(), 1);
    let drift = &drift_report2.drifts[0];
    assert_eq!(drift.kind, ForumCounterDriftKind::CategoryTopicCount);
    assert_eq!(drift.subject_id, cat1);
    assert_eq!(drift.stored, 10);
    assert_eq!(drift.expected, 2);

    // Restore cat1 topic_count
    db.execute_unprepared(&format!(
        "UPDATE forum_categories SET topic_count = 2 WHERE id = {}",
        sql_uuid(cat1)
    ))
    .await
    .expect("restore should succeed");

    // 4. Introduce CategoryReplyCount drift on cat1: stored = 88, expected = 3
    db.execute_unprepared(&format!(
        "UPDATE forum_categories SET reply_count = 88 WHERE id = {}",
        sql_uuid(cat1)
    ))
    .await
    .expect("update should succeed");

    let drift_report3 = service
        .report(tenant_id, &admin, None)
        .await
        .expect("drift report should succeed");
    assert_eq!(drift_report3.drift_count(), 1);
    let drift = &drift_report3.drifts[0];
    assert_eq!(drift.kind, ForumCounterDriftKind::CategoryReplyCount);
    assert_eq!(drift.subject_id, cat1);
    assert_eq!(drift.stored, 88);
    assert_eq!(drift.expected, 3);
}

#[tokio::test]
async fn counter_reconciliation_pagination_and_independent_cursors_sqlite() {
    let db = setup_db().await;
    let tenant_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let cat1 = Uuid::new_v4();
    let cat2 = Uuid::new_v4();

    // Create 2 categories and 4 topics
    seed_category(&db, tenant_id, cat1, 2, 0).await;
    seed_category(&db, tenant_id, cat2, 2, 0).await;

    let mut topic_ids = Vec::new();
    for _ in 0..4 {
        let t_id = Uuid::new_v4();
        seed_topic(&db, tenant_id, cat1, t_id, author_id, 0).await;
        topic_ids.push(t_id);
    }

    let service = ForumCounterReconciliationService::new(db.clone());
    let admin = SecurityContext::system();

    // Fetch with limit = 2
    let page1 = service
        .report_page(tenant_id, &admin, Some(2), None, None)
        .await
        .expect("page1 should succeed");
    assert_eq!(page1.effective_limit, 2);
    assert_eq!(page1.inspected_topics, 2);
    assert_eq!(page1.inspected_categories, 2);
    assert!(page1.has_more_topics);
    assert!(!page1.has_more_categories);
    assert!(page1.topic_cursor.is_some());
    assert!(page1.category_cursor.is_some());

    // Page 2: continue with topic_cursor from page1, and category_cursor from page1
    let page2 = service
        .report_page(
            tenant_id,
            &admin,
            Some(2),
            page1.topic_cursor,
            page1.category_cursor,
        )
        .await
        .expect("page2 should succeed");
    assert_eq!(page2.inspected_topics, 2);
    assert_eq!(page2.inspected_categories, 0); // categories already exhausted
    assert!(!page2.has_more_topics);
    assert!(!page2.has_more_categories);
}

#[tokio::test]
async fn counter_reconciliation_requires_manage_permissions() {
    let db = setup_db().await;
    let tenant_id = Uuid::new_v4();
    let service = ForumCounterReconciliationService::new(db);
    let anonymous = SecurityContext::new(UserRole::Customer, None);

    let err = service
        .report(tenant_id, &anonymous, None)
        .await
        .expect_err("anonymous caller must be rejected");
    assert!(matches!(err, rustok_forum::ForumError::Forbidden(_)));
}
