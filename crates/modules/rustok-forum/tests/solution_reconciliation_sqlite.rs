use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    ForumModule, ForumSolutionDriftKind, ForumSolutionReconciliationService,
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
         VALUES ({}, {}, 0, 0, 1, 1)",
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
         VALUES ({}, {}, {}, {}, 'open', '{{}}', 0, 0, 1); \
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

async fn seed_reply(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
    reply_id: Uuid,
    author_id: Uuid,
    status: &str,
) {
    db.execute_unprepared(&format!(
        "INSERT INTO forum_replies \
         (id, tenant_id, topic_id, author_id, status, position) \
         VALUES ({}, {}, {}, {}, '{}', 1); \
         INSERT INTO forum_reply_bodies \
         (id, reply_id, tenant_id, locale, body) \
         VALUES ({}, {}, {}, 'en', 'Reply body')",
        sql_uuid(reply_id),
        sql_uuid(tenant_id),
        sql_uuid(topic_id),
        sql_uuid(author_id),
        status,
        sql_uuid(Uuid::new_v4()),
        sql_uuid(reply_id),
        sql_uuid(tenant_id),
    ))
    .await
    .expect("reply seed should succeed");
}

async fn seed_solution(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    topic_id: Uuid,
    reply_id: Uuid,
) {
    db.execute_unprepared(&format!(
        "INSERT INTO forum_solutions (topic_id, tenant_id, reply_id) \
         VALUES ({}, {}, {})",
        sql_uuid(topic_id),
        sql_uuid(tenant_id),
        sql_uuid(reply_id),
    ))
    .await
    .expect("solution seed should succeed");
}

async fn seed_user_stat(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    user_id: Uuid,
    solution_count: i64,
) {
    db.execute_unprepared(&format!(
        "INSERT INTO forum_user_stats (tenant_id, user_id, topic_count, reply_count, solution_count) \
         VALUES ({}, {}, 1, 1, {}) \
         ON CONFLICT (tenant_id, user_id) DO UPDATE SET solution_count = excluded.solution_count",
        sql_uuid(tenant_id),
        sql_uuid(user_id),
        solution_count,
    ))
    .await
    .expect("user stat seed should succeed");
}

#[tokio::test]
async fn solution_reconciliation_requires_manage_permissions() {
    let db = setup_db().await;
    let service = ForumSolutionReconciliationService::new(db);
    let tenant_id = Uuid::new_v4();

    // 1. Customer fails
    let customer = SecurityContext::new(UserRole::Customer, Some(Uuid::new_v4()));
    let err = service
        .report_page(tenant_id, &customer, None, None, None)
        .await
        .expect_err("customer must not run solution reconciliation");
    assert!(
        matches!(err, rustok_forum::ForumError::Forbidden(_)),
        "expected Forbidden, got {err:?}"
    );

    // 2. Anonymous / without permissions fails
    let anonymous = SecurityContext::new(UserRole::Customer, None);
    let err = service
        .report_page(tenant_id, &anonymous, None, None, None)
        .await
        .expect_err("anonymous must not run solution reconciliation");
    assert!(
        matches!(err, rustok_forum::ForumError::Forbidden(_)),
        "expected Forbidden, got {err:?}"
    );

    // 3. Admin / System succeeds
    let admin = SecurityContext::system();
    let report = service
        .report_page(tenant_id, &admin, None, None, None)
        .await
        .expect("system admin should succeed");
    assert!(report.is_clean());
}

#[tokio::test]
async fn solution_reconciliation_detects_clean_state_and_all_drifts_sqlite() {
    let db = setup_db().await;
    let tenant_id = Uuid::new_v4();
    let author_id = Uuid::new_v4();
    let cat_id = Uuid::new_v4();
    let top1 = Uuid::new_v4();
    let rep1 = Uuid::new_v4();

    seed_category(&db, tenant_id, cat_id).await;
    seed_topic(&db, tenant_id, cat_id, top1, author_id).await;
    seed_reply(&db, tenant_id, top1, rep1, author_id, "approved").await;
    seed_solution(&db, tenant_id, top1, rep1).await;
    seed_user_stat(&db, tenant_id, author_id, 1).await;

    let service = ForumSolutionReconciliationService::new(db.clone());
    let admin = SecurityContext::system();

    // 1. Clean verification
    let clean = service
        .report_page(tenant_id, &admin, None, None, None)
        .await
        .expect("clean report");
    assert!(clean.is_clean());
    assert_eq!(clean.drift_count(), 0);
    assert_eq!(clean.inspected_solutions, 1);
    assert_eq!(clean.inspected_solution_stats, 1);

    // 2. Introduce AcceptedReplyEligibility drift: reply status changed from 'approved' to 'hidden'
    db.execute_unprepared(&format!(
        "UPDATE forum_replies SET status = 'hidden' WHERE id = {}",
        sql_uuid(rep1)
    ))
    .await
    .expect("update reply status to hidden");

    let drift1 = service
        .report_page(tenant_id, &admin, None, None, None)
        .await
        .expect("drift report 1");
    assert!(!drift1.is_clean());
    let d1 = drift1
        .drifts
        .iter()
        .find(|d| d.kind == ForumSolutionDriftKind::AcceptedReplyEligibility)
        .expect("should detect AcceptedReplyEligibility drift");
    assert_eq!(d1.subject_id, top1);
    assert_eq!(d1.stored, 1);
    assert_eq!(d1.expected, 0);

    // Restore reply status to 'approved'
    db.execute_unprepared(&format!(
        "UPDATE forum_replies SET status = 'approved' WHERE id = {}",
        sql_uuid(rep1)
    ))
    .await
    .expect("restore reply status");

    // 3. Introduce SolutionAuthorStatMissing drift: delete author stat row
    db.execute_unprepared(&format!(
        "DELETE FROM forum_user_stats WHERE tenant_id = {} AND user_id = {}",
        sql_uuid(tenant_id),
        sql_uuid(author_id)
    ))
    .await
    .expect("delete author stat");

    let drift2 = service
        .report_page(tenant_id, &admin, None, None, None)
        .await
        .expect("drift report 2");
    let d2 = drift2
        .drifts
        .iter()
        .find(|d| d.kind == ForumSolutionDriftKind::SolutionAuthorStatMissing)
        .expect("should detect SolutionAuthorStatMissing drift");
    assert_eq!(d2.subject_id, author_id);
    assert_eq!(d2.stored, 0);
    assert_eq!(d2.expected, 1);

    // Restore author stat
    seed_user_stat(&db, tenant_id, author_id, 1).await;

    // 4. Introduce SolutionAuthorStatCount drift: solution_count = 99
    // Drop self-healing trigger so SQLite trigger doesn't auto-correct it
    db.execute_unprepared("DROP TRIGGER IF EXISTS forum_user_stats_public_reply_count_update;").await.unwrap();

    db.execute_unprepared(&format!(
        "UPDATE forum_user_stats SET solution_count = 99 WHERE tenant_id = {} AND user_id = {}",
        sql_uuid(tenant_id),
        sql_uuid(author_id)
    ))
    .await
    .expect("update stat to 99");

    let drift3 = service
        .report_page(tenant_id, &admin, None, None, None)
        .await
        .expect("drift report 3");
    let d3 = drift3
        .drifts
        .iter()
        .find(|d| d.kind == ForumSolutionDriftKind::SolutionAuthorStatCount)
        .expect("should detect SolutionAuthorStatCount drift");
    assert_eq!(d3.subject_id, author_id);
    assert_eq!(d3.stored, 99);
    assert_eq!(d3.expected, 1);
}

#[tokio::test]
async fn solution_reconciliation_pagination_and_independent_cursors_sqlite() {
    let db = setup_db().await;
    let tenant_id = Uuid::new_v4();
    let cat_id = Uuid::new_v4();
    seed_category(&db, tenant_id, cat_id).await;

    let mut topic_ids = Vec::new();
    let mut author_ids = Vec::new();

    // Create 3 topics, each with a reply, an accepted solution, and author stat
    for _ in 0..3 {
        let author_id = Uuid::new_v4();
        let top_id = Uuid::new_v4();
        let rep_id = Uuid::new_v4();
        seed_topic(&db, tenant_id, cat_id, top_id, author_id).await;
        seed_reply(&db, tenant_id, top_id, rep_id, author_id, "approved").await;
        seed_solution(&db, tenant_id, top_id, rep_id).await;
        seed_user_stat(&db, tenant_id, author_id, 1).await;
        topic_ids.push(top_id);
        author_ids.push(author_id);
    }

    let service = ForumSolutionReconciliationService::new(db);
    let admin = SecurityContext::system();

    // Page 1 with limit = 2
    let page1 = service
        .report_page(tenant_id, &admin, Some(2), None, None)
        .await
        .expect("page 1 should succeed");
    assert_eq!(page1.effective_limit, 2);
    assert_eq!(page1.inspected_solutions, 2);
    assert_eq!(page1.inspected_solution_stats, 2);
    assert!(page1.has_more_solutions);
    assert!(page1.has_more_solution_stats);
    assert!(page1.solution_cursor.is_some());
    assert!(page1.solution_stat_cursor.is_some());

    // Page 2: advance both cursors
    let page2 = service
        .report_page(
            tenant_id,
            &admin,
            Some(2),
            page1.solution_cursor,
            page1.solution_stat_cursor,
        )
        .await
        .expect("page 2 should succeed");
    assert_eq!(page2.inspected_solutions, 1);
    assert_eq!(page2.inspected_solution_stats, 1);
    assert!(!page2.has_more_solutions);
    assert!(!page2.has_more_solution_stats);

    // Page 3: exhausted-one-side behavior
    // If solution cursor is exhausted, repeating it should return 0 inspected solutions
    let page3 = service
        .report_page(
            tenant_id,
            &admin,
            Some(2),
            page2.solution_cursor,
            None, // reset solution_stat_cursor to beginning
        )
        .await
        .expect("page 3 should succeed");
    assert_eq!(page3.inspected_solutions, 0);
    assert!(!page3.has_more_solutions);
    assert_eq!(page3.inspected_solution_stats, 2);
    assert!(page3.has_more_solution_stats);
}
