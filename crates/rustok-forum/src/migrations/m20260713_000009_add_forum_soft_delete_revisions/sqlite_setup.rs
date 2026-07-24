use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub(super) async fn apply_setup(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "ALTER TABLE forum_topics ADD COLUMN deleted_at TEXT",
        "ALTER TABLE forum_replies ADD COLUMN deleted_at TEXT",
        "CREATE INDEX idx_forum_topics_tenant_deleted
         ON forum_topics (tenant_id, deleted_at, updated_at)",
        "CREATE INDEX idx_forum_replies_tenant_topic_deleted
         ON forum_replies (tenant_id, topic_id, deleted_at, position)",
        r#"CREATE TABLE forum_topic_revisions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tenant_id TEXT NOT NULL,
            topic_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            title TEXT NOT NULL,
            slug TEXT,
            body TEXT NOT NULL,
            body_format TEXT NOT NULL,
            metadata TEXT NOT NULL DEFAULT '{}',
            revision_reason TEXT NOT NULL
                CHECK (revision_reason IN ('edit', 'delete')),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (topic_id)
                REFERENCES forum_topics (id)
                ON UPDATE CASCADE
                ON DELETE CASCADE
        )"#,
        "CREATE INDEX idx_forum_topic_revisions_tenant_topic_created
         ON forum_topic_revisions (tenant_id, topic_id, created_at DESC, id DESC)",
        r#"CREATE TABLE forum_reply_revisions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            tenant_id TEXT NOT NULL,
            reply_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            body TEXT NOT NULL,
            body_format TEXT NOT NULL,
            revision_reason TEXT NOT NULL
                CHECK (revision_reason IN ('edit', 'delete')),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (reply_id)
                REFERENCES forum_replies (id)
                ON UPDATE CASCADE
                ON DELETE CASCADE
        )"#,
        "CREATE INDEX idx_forum_reply_revisions_tenant_reply_created
         ON forum_reply_revisions (tenant_id, reply_id, created_at DESC, id DESC)",
        r#"CREATE TABLE forum_hard_delete_context (
            category_id TEXT NOT NULL,
            topic_id TEXT PRIMARY KEY NOT NULL
        )"#,
        "DROP TRIGGER IF EXISTS forum_topic_translation_revision_update",
        "DROP TRIGGER IF EXISTS forum_topic_metadata_revision_update",
        "DROP TRIGGER IF EXISTS forum_reply_body_revision_update",
        "DROP TRIGGER IF EXISTS forum_topics_deleted_update_guard",
        "DROP TRIGGER IF EXISTS forum_replies_deleted_update_guard",
        "DROP TRIGGER IF EXISTS forum_categories_hard_delete_context_before",
        "DROP TRIGGER IF EXISTS forum_categories_hard_delete_context_after",
        "DROP TRIGGER IF EXISTS forum_topics_soft_delete",
        "DROP TRIGGER IF EXISTS forum_replies_soft_delete",
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}
