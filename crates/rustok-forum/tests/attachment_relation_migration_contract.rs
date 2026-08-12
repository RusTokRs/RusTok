use sea_orm::{ConnectionTrait, Database};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

async fn setup() -> sea_orm::DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("SQLite attachment migration test database should connect");
    db.execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .expect("SQLite foreign keys should be enabled");

    let manager = SchemaManager::new(&db);
    let migration = rustok_forum::migrations::migrations()
        .into_iter()
        .find(|migration| migration.name() == "m20260812_000028_add_forum_attachment_relations")
        .expect("FORUM-14 attachment migration should be registered");
    migration
        .up(&manager)
        .await
        .expect("FORUM-14 attachment migration should apply");
    db
}

#[tokio::test]
async fn attachment_schema_keeps_revision_snapshots_immutable_and_media_owner_independent() {
    let db = setup().await;
    let tenant_id = Uuid::new_v4();
    let topic_id = Uuid::new_v4();
    let media_id = Uuid::new_v4();
    let fingerprint = "a".repeat(64);

    db.execute_unprepared(&format!(
        "INSERT INTO forum_attachment_relation_revisions \
         (tenant_id, target_kind, target_id, source_revision, locale, projection_fingerprint) \
         VALUES ('{tenant_id}', 'topic', '{topic_id}', 7, 'en', '{fingerprint}')"
    ))
    .await
    .expect("empty attachment snapshot header should persist independently of child rows");

    db.execute_unprepared(&format!(
        "INSERT INTO forum_attachment_relations \
         (tenant_id, target_kind, target_id, source_revision, locale, position, media_id, usage) \
         VALUES ('{tenant_id}', 'topic', '{topic_id}', 7, 'en', 0, '{media_id}', 'inline')"
    ))
    .await
    .expect("admitted Media identity should fit the Forum-owned relation schema");

    let immutable_child = db
        .execute_unprepared(&format!(
            "UPDATE forum_attachment_relations SET usage = 'attachment' \
             WHERE tenant_id = '{tenant_id}' AND target_id = '{topic_id}' \
               AND source_revision = 7 AND locale = 'en' AND position = 0"
        ))
        .await;
    assert!(immutable_child.is_err());

    let immutable_header = db
        .execute_unprepared(&format!(
            "UPDATE forum_attachment_relation_revisions \
             SET projection_fingerprint = '{}' \
             WHERE tenant_id = '{tenant_id}' AND target_id = '{topic_id}' \
               AND source_revision = 7 AND locale = 'en'",
            "b".repeat(64)
        ))
        .await;
    assert!(immutable_header.is_err());

    let invalid_usage = db
        .execute_unprepared(&format!(
            "INSERT INTO forum_attachment_relations \
             (tenant_id, target_kind, target_id, source_revision, locale, position, media_id, usage) \
             VALUES ('{tenant_id}', 'topic', '{topic_id}', 7, 'en', 1, '{}', 'avatar')",
            Uuid::new_v4()
        ))
        .await;
    assert!(invalid_usage.is_err());

    let invalid_position = db
        .execute_unprepared(&format!(
            "INSERT INTO forum_attachment_relations \
             (tenant_id, target_kind, target_id, source_revision, locale, position, media_id, usage) \
             VALUES ('{tenant_id}', 'topic', '{topic_id}', 7, 'en', 32, '{}', 'attachment')",
            Uuid::new_v4()
        ))
        .await;
    assert!(invalid_position.is_err());

    let orphan = db
        .execute_unprepared(&format!(
            "INSERT INTO forum_attachment_relations \
             (tenant_id, target_kind, target_id, source_revision, locale, position, media_id, usage) \
             VALUES ('{tenant_id}', 'reply', '{}', 1, 'en', 0, '{}', 'attachment')",
            Uuid::new_v4(),
            Uuid::new_v4()
        ))
        .await;
    assert!(orphan.is_err());
}
