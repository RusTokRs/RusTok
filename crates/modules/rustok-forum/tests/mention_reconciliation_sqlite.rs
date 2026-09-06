use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    ForumMentionDriftKind, ForumMentionReconciliationService, ForumModule,
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
    locale: &str,
) {
    db.execute_unprepared(&format!(
        "INSERT INTO forum_topics \
         (id, tenant_id, category_id, author_id, status, metadata, is_pinned, is_locked, reply_count) \
         VALUES ({}, {}, {}, {}, 'open', '{{}}', 0, 0, 0); \
         INSERT INTO forum_topic_translations \
         (id, topic_id, tenant_id, locale, title, slug, body) \
         VALUES ({}, {}, {}, '{}', 'Topic', 'topic-{topic_id}', 'Topic body')",
        sql_uuid(topic_id),
        sql_uuid(tenant_id),
        sql_uuid(category_id),
        sql_uuid(author_id),
        sql_uuid(Uuid::new_v4()),
        sql_uuid(topic_id),
        sql_uuid(tenant_id),
        locale,
    ))
    .await
    .expect("topic seed should succeed");
}

async fn seed_relation_revision(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    revision_id: i64,
    target_kind: &str,
    target_id: Uuid,
    locale: &str,
    fingerprint: &str,
) {
    db.execute_unprepared(&format!(
        "INSERT INTO forum_relation_revisions \
         (revision_id, tenant_id, target_kind, target_id, locale, projection_fingerprint, created_at) \
         VALUES ({}, {}, '{}', {}, '{}', '{}', CURRENT_TIMESTAMP)",
        revision_id,
        sql_uuid(tenant_id),
        target_kind,
        sql_uuid(target_id),
        locale,
        fingerprint,
    ))
    .await
    .expect("relation revision seed should succeed");
}

async fn seed_user_mention(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    source_kind: &str,
    source_id: Uuid,
    source_locale: &str,
    source_revision_id: i64,
    mentioned_user_id: Uuid,
    handle: &str,
) {
    db.execute_unprepared(&format!(
        "INSERT INTO forum_user_mentions \
         (tenant_id, source_kind, source_id, source_locale, source_revision_id, mentioned_user_id, handle_snapshot, created_at) \
         VALUES ({}, '{}', {}, '{}', {}, {}, '{}', CURRENT_TIMESTAMP)",
        sql_uuid(tenant_id),
        source_kind,
        sql_uuid(source_id),
        source_locale,
        source_revision_id,
        sql_uuid(mentioned_user_id),
        handle,
    ))
    .await
    .expect("user mention seed should succeed");
}

#[tokio::test]
async fn mention_reconciliation_requires_manage_permissions() {
    let db = setup_db().await;
    let service = ForumMentionReconciliationService::new(db);
    let tenant_id = Uuid::new_v4();

    // 1. Customer rejected
    let customer = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let err = service
        .report_page(tenant_id, &customer, None, None)
        .await
        .expect_err("customer must be rejected");
    assert!(matches!(err, rustok_forum::ForumError::Forbidden(_)));

    // 2. Anonymous rejected
    let anonymous = SecurityContext::new(UserRole::Customer, None);
    let err = service
        .report_page(tenant_id, &anonymous, None, None)
        .await
        .expect_err("anonymous must be rejected");
    assert!(matches!(err, rustok_forum::ForumError::Forbidden(_)));

    // 3. Admin succeeds
    let admin = SecurityContext::system();
    let report = service
        .report_page(tenant_id, &admin, None, None)
        .await
        .expect("admin should succeed");
    assert!(report.is_clean());
}

#[tokio::test]
async fn mention_reconciliation_detects_clean_state_and_all_drifts_sqlite() {
    let db = setup_db().await;
    let tenant_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let user1 = Uuid::new_v4();
    let cat_id = Uuid::new_v4();
    let top1 = Uuid::new_v4();

    seed_category(&db, tenant_id, cat_id).await;
    seed_topic(&db, tenant_id, cat_id, top1, author_id, "en").await;

    // Clean relation revision #1 pointing to top1 in "en" with valid fingerprint
    let clean_fingerprint = "0".repeat(64);
    seed_relation_revision(&db, tenant_id, 1, "topic", top1, "en", &clean_fingerprint).await;
    seed_user_mention(&db, tenant_id, "topic", top1, "en", 1, user1, "alice").await;

    let service = ForumMentionReconciliationService::new(db.clone());
    let admin = SecurityContext::system();

    // 1. Clean check
    let clean = service
        .report_page(tenant_id, &admin, None, None)
        .await
        .expect("clean report");
    assert!(clean.is_clean());
    assert_eq!(clean.drift_count(), 0);
    assert_eq!(clean.inspected_relation_revisions, 1);
    assert_eq!(clean.inspected_mention_revisions, 1);

    // Drop validation triggers to simulate drifted states
    db.execute_unprepared("DROP TRIGGER IF EXISTS forum_relation_revision_source_guard;").await.unwrap();
    db.execute_unprepared("DROP TRIGGER IF EXISTS forum_user_mentions_source_guard;").await.unwrap();

    // 2. SourceUnavailable drift: revision pointing to non-existent topic translation
    let missing_top_id = Uuid::new_v4();
    seed_relation_revision(&db, tenant_id, 2, "topic", missing_top_id, "en", &clean_fingerprint).await;
    seed_user_mention(&db, tenant_id, "topic", missing_top_id, "en", 2, user1, "bob").await;

    let drift1 = service
        .report_page(tenant_id, &admin, None, None)
        .await
        .expect("drift report 1");
    let d1 = drift1
        .drifts
        .iter()
        .find(|d| d.kind == ForumMentionDriftKind::SourceUnavailable)
        .expect("SourceUnavailable should be detected");
    assert_eq!(d1.revision_id, 2);
    assert_eq!(d1.source_id, missing_top_id);

    // Clean up revision 2
    db.execute_unprepared("DELETE FROM forum_user_mentions WHERE source_revision_id = 2").await.unwrap();
    db.execute_unprepared("DELETE FROM forum_relation_revisions WHERE revision_id = 2").await.unwrap();

    // 3. ChildSourceMismatch drift: user mention has different source_locale than parent revision
    seed_relation_revision(&db, tenant_id, 3, "topic", top1, "en", &clean_fingerprint).await;
    // user mention says source_locale = "fr" while revision has locale = "en"
    seed_user_mention(&db, tenant_id, "topic", top1, "fr", 3, user1, "carol").await;

    let drift2 = service
        .report_page(tenant_id, &admin, None, None)
        .await
        .expect("drift report 2");
    let d2 = drift2
        .drifts
        .iter()
        .find(|d| d.kind == ForumMentionDriftKind::ChildSourceMismatch)
        .expect("ChildSourceMismatch should be detected");
    assert_eq!(d2.revision_id, 3);

    // Clean up revision 3
    db.execute_unprepared("DELETE FROM forum_user_mentions WHERE source_revision_id = 3").await.unwrap();
    db.execute_unprepared("DELETE FROM forum_relation_revisions WHERE revision_id = 3").await.unwrap();

    // 4. ProjectionFingerprintInvalid drift: fingerprint is not 64 hex characters and not "legacy"
    seed_relation_revision(&db, tenant_id, 4, "topic", top1, "en", "invalid-short-fingerprint").await;
    seed_user_mention(&db, tenant_id, "topic", top1, "en", 4, user1, "dave").await;

    let drift3 = service
        .report_page(tenant_id, &admin, None, None)
        .await
        .expect("drift report 3");
    let d3 = drift3
        .drifts
        .iter()
        .find(|d| d.kind == ForumMentionDriftKind::ProjectionFingerprintInvalid)
        .expect("ProjectionFingerprintInvalid should be detected");
    assert_eq!(d3.revision_id, 4);

    // Clean up revision 4
    db.execute_unprepared("DELETE FROM forum_user_mentions WHERE source_revision_id = 4").await.unwrap();
    db.execute_unprepared("DELETE FROM forum_relation_revisions WHERE revision_id = 4").await.unwrap();
}

#[tokio::test]
async fn mention_reconciliation_pagination_sqlite() {
    let db = setup_db().await;
    let tenant_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let cat_id = Uuid::new_v4();
    let top_id = Uuid::new_v4();

    seed_category(&db, tenant_id, cat_id).await;
    seed_topic(&db, tenant_id, cat_id, top_id, author_id, "en").await;

    let clean_fingerprint = "0".repeat(64);
    // Seed 3 revisions with mentions
    for rev_id in 1..=3 {
        seed_relation_revision(&db, tenant_id, rev_id, "topic", top_id, "en", &clean_fingerprint).await;
        seed_user_mention(
            &db, tenant_id, "topic", top_id, "en", rev_id, user_id, &format!("user{rev_id}"),
        )
        .await;
    }

    let service = ForumMentionReconciliationService::new(db);
    let admin = SecurityContext::system();

    // Page 1: limit 2
    let page1 = service
        .report_page(tenant_id, &admin, Some(2), None)
        .await
        .expect("page 1");
    assert_eq!(page1.effective_limit, 2);
    assert_eq!(page1.inspected_relation_revisions, 2);
    assert!(page1.has_more_relation_revisions);
    assert_eq!(page1.relation_cursor, Some(2));

    // Page 2: after cursor 2
    let page2 = service
        .report_page(tenant_id, &admin, Some(2), page1.relation_cursor)
        .await
        .expect("page 2");
    assert_eq!(page2.inspected_relation_revisions, 1);
    assert!(!page2.has_more_relation_revisions);
    assert_eq!(page2.relation_cursor, Some(3));
}
