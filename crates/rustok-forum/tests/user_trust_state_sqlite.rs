use rustok_api::Permission;
use rustok_core::SecurityContext;
use rustok_forum::{
    ForumError, ForumUserTrustService, SetForumUserTrustInput, entities::forum_user_trust_revision,
};
use sea_orm::{ConnectionTrait, Database, EntityTrait, PaginatorTrait};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup() -> (sea_orm::DatabaseConnection, Uuid, Uuid, Uuid) {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("SQLite trust test database should connect");
    db.execute_unprepared(
        r#"
PRAGMA foreign_keys = ON;
CREATE TABLE users (
    id BLOB PRIMARY KEY NOT NULL,
    tenant_id BLOB NOT NULL,
    email TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE forum_user_stats (
    tenant_id BLOB NOT NULL,
    user_id BLOB NOT NULL,
    topic_count INTEGER NOT NULL DEFAULT 0,
    reply_count INTEGER NOT NULL DEFAULT 0,
    solution_count INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (tenant_id, user_id)
);
"#,
    )
    .await
    .expect("trust prerequisites should be created");

    let manager = SchemaManager::new(&db);
    let migration = rustok_forum::migrations::migrations()
        .into_iter()
        .find(|migration| migration.name() == "m20260728_000004_add_forum_user_trust_state")
        .expect("FORUM-26A migration should be registered");
    migration
        .up(&manager)
        .await
        .expect("FORUM-26A migration should apply");

    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    db.execute_unprepared(&format!(
        "INSERT INTO users (id, tenant_id, email) VALUES \
         (X'{}', X'{}', 'actor@example.invalid'), \
         (X'{}', X'{}', 'member@example.invalid')",
        actor_id.simple().to_string().to_uppercase(),
        tenant_id.simple().to_string().to_uppercase(),
        user_id.simple().to_string().to_uppercase(),
        tenant_id.simple().to_string().to_uppercase(),
    ))
    .await
    .expect("trust users should be inserted");

    (db, tenant_id, actor_id, user_id)
}

fn manager(actor_id: Uuid) -> SecurityContext {
    SecurityContext::from_permission_snapshot(Some(actor_id), &[Permission::FORUM_TOPICS_MANAGE])
}

fn input(level: u8, key: &str, code: &str, summary: &str) -> SetForumUserTrustInput {
    SetForumUserTrustInput {
        trust_level: level,
        reason_code: code.to_string(),
        reason_summary: summary.to_string(),
        idempotency_key: key.to_string(),
    }
}

#[tokio::test]
async fn trust_state_is_authoritative_versioned_and_independent_from_activity_counters() {
    let (db, tenant_id, actor_id, user_id) = setup().await;
    let service = ForumUserTrustService::new(db.clone());

    let initial = service
        .get(tenant_id, user_id, manager(actor_id))
        .await
        .expect("missing state should use the fail-closed default");
    assert!(!initial.configured);
    assert_eq!(initial.trust_level, 0);
    assert_eq!(initial.revision, 0);

    db.execute_unprepared(&format!(
        "INSERT INTO forum_user_stats \
         (tenant_id, user_id, topic_count, reply_count, solution_count) \
         VALUES ('{tenant_id}', '{user_id}', 500, 800, 100)"
    ))
    .await
    .expect("activity counters should be writable independently");
    let still_default = service
        .get(tenant_id, user_id, manager(actor_id))
        .await
        .expect("activity counters must not become trust");
    assert_eq!(still_default.trust_level, 0);
    assert!(!still_default.configured);

    let first = service
        .set(
            tenant_id,
            user_id,
            manager(actor_id),
            input(
                20,
                "trust-change-1",
                "manual_review",
                "Initial moderator review",
            ),
        )
        .await
        .expect("first managed trust change should succeed");
    assert_eq!(first.state.trust_level, 20);
    assert_eq!(first.state.revision, 1);
    assert_eq!(first.revision.previous_trust_level, None);
    assert_eq!(first.revision.changed_by_user_id, Some(actor_id));

    let replay = service
        .set(
            tenant_id,
            user_id,
            manager(actor_id),
            input(
                20,
                "trust-change-1",
                "manual_review",
                "Initial moderator review",
            ),
        )
        .await
        .expect("an identical idempotent replay should succeed");
    assert_eq!(replay, first);
    assert_eq!(
        forum_user_trust_revision::Entity::find()
            .count(&db)
            .await
            .expect("revision count should load"),
        1
    );

    let second = service
        .set(
            tenant_id,
            user_id,
            manager(actor_id),
            input(
                35,
                "trust-change-2",
                "appeal_approved",
                "Appeal review completed",
            ),
        )
        .await
        .expect("second managed trust change should succeed");
    assert_eq!(second.state.trust_level, 35);
    assert_eq!(second.state.revision, 2);
    assert_eq!(second.revision.previous_trust_level, Some(20));

    let history = service
        .history(tenant_id, user_id, manager(actor_id), None, 100)
        .await
        .expect("bounded trust history should load");
    assert_eq!(history.items.len(), 2);
    assert_eq!(history.items[0].revision, 2);
    assert_eq!(history.items[1].revision, 1);
    assert_eq!(history.next_before_revision, None);

    let current = service
        .get(tenant_id, user_id, manager(actor_id))
        .await
        .expect("current trust state should load");
    assert!(current.configured);
    assert_eq!(current.trust_level, 35);
    assert_eq!(current.revision, 2);
}

#[tokio::test]
async fn trust_database_guards_reject_orphans_gaps_and_direct_mutation() {
    let (db, tenant_id, actor_id, user_id) = setup().await;
    let service = ForumUserTrustService::new(db.clone());
    service
        .set(
            tenant_id,
            user_id,
            manager(actor_id),
            input(10, "trust-guard-1", "manual_review", "Guard baseline"),
        )
        .await
        .expect("baseline trust state should be written");

    let update_revision = db
        .execute_unprepared(&format!(
            "UPDATE forum_user_trust_revisions SET trust_level = 99 \
             WHERE tenant_id = X'{}' AND user_id = X'{}' AND revision = 1",
            tenant_id.simple().to_string().to_uppercase(),
            user_id.simple().to_string().to_uppercase()
        ))
        .await;
    assert!(update_revision.is_err());

    let delete_revision = db
        .execute_unprepared(&format!(
            "DELETE FROM forum_user_trust_revisions \
             WHERE tenant_id = X'{}' AND user_id = X'{}' AND revision = 1",
            tenant_id.simple().to_string().to_uppercase(),
            user_id.simple().to_string().to_uppercase()
        ))
        .await;
    assert!(delete_revision.is_err());

    let direct_state_update = db
        .execute_unprepared(&format!(
            "UPDATE forum_user_trust_states SET trust_level = 50, revision = 2 \
             WHERE tenant_id = X'{}' AND user_id = X'{}'",
            tenant_id.simple().to_string().to_uppercase(),
            user_id.simple().to_string().to_uppercase()
        ))
        .await;
    assert!(direct_state_update.is_err());

    let direct_state_delete = db
        .execute_unprepared(&format!(
            "DELETE FROM forum_user_trust_states \
             WHERE tenant_id = X'{}' AND user_id = X'{}'",
            tenant_id.simple().to_string().to_uppercase(),
            user_id.simple().to_string().to_uppercase()
        ))
        .await;
    assert!(direct_state_delete.is_err());

    let gap = db
        .execute_unprepared(&format!(
            "INSERT INTO forum_user_trust_revisions \
             (tenant_id, user_id, revision, previous_trust_level, trust_level, change_kind, \
              reason_code, reason_summary, changed_by_user_id, idempotency_key, created_at) \
             VALUES (X'{}', X'{}', 3, 10, 20, 'manual_override', \
                     'gap_attempt', 'Gap attempt', X'{}', 'trust-gap', CURRENT_TIMESTAMP)",
            tenant_id.simple().to_string().to_uppercase(),
            user_id.simple().to_string().to_uppercase(),
            actor_id.simple().to_string().to_uppercase()
        ))
        .await;
    assert!(gap.is_err());

    let foreign_tenant = Uuid::new_v4();
    let foreign_state = db
        .execute_unprepared(&format!(
            "INSERT INTO forum_user_trust_revisions \
             (tenant_id, user_id, revision, previous_trust_level, trust_level, change_kind, \
              reason_code, reason_summary, changed_by_user_id, idempotency_key, created_at) \
             VALUES (X'{}', X'{}', 1, NULL, 10, 'manual_override', \
                     'foreign_attempt', 'Foreign attempt', NULL, 'trust-foreign', CURRENT_TIMESTAMP)",
            foreign_tenant.simple().to_string().to_uppercase(),
            user_id.simple().to_string().to_uppercase()
        ))
        .await;
    assert!(foreign_state.is_err());
}

#[tokio::test]
async fn trust_owner_requires_manage_scope_and_exact_idempotent_payload() {
    let (db, tenant_id, actor_id, user_id) = setup().await;
    let service = ForumUserTrustService::new(db);
    let denied = service
        .get(
            tenant_id,
            user_id,
            SecurityContext::from_permission_snapshot(Some(actor_id), &[]),
        )
        .await;
    assert!(matches!(denied, Err(ForumError::Forbidden(_))));

    service
        .set(
            tenant_id,
            user_id,
            manager(actor_id),
            input(15, "trust-replay", "manual_review", "Original payload"),
        )
        .await
        .expect("original idempotent command should succeed");
    let conflict = service
        .set(
            tenant_id,
            user_id,
            manager(actor_id),
            input(16, "trust-replay", "manual_review", "Changed payload"),
        )
        .await;
    assert!(
        matches!(conflict, Err(ForumError::Validation(message)) if message.contains("idempotency"))
    );
}
