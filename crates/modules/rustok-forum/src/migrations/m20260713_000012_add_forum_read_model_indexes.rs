use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let is_sqlite = manager.get_database_backend() == DatabaseBackend::Sqlite;
        let index_sql = if is_sqlite {
            r#"
CREATE INDEX IF NOT EXISTS idx_forum_categories_cursor
    ON forum_categories (tenant_id, position, id);

CREATE INDEX IF NOT EXISTS idx_forum_topics_cursor
    ON forum_topics (tenant_id, updated_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_forum_topics_list_keyset
    ON forum_topics (tenant_id, is_pinned DESC, last_reply_at DESC, updated_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_forum_replies_keyset
    ON forum_replies (tenant_id, topic_id, created_at, id);
"#
        } else {
            r#"
CREATE INDEX IF NOT EXISTS idx_forum_categories_cursor
    ON forum_categories (tenant_id, position, id);

CREATE INDEX IF NOT EXISTS idx_forum_topics_cursor
    ON forum_topics (tenant_id, updated_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_forum_topics_list_keyset
    ON forum_topics (tenant_id, is_pinned DESC, last_reply_at DESC NULLS LAST, updated_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_forum_replies_keyset
    ON forum_replies (tenant_id, topic_id, created_at, id);
"#
        };

        manager
            .get_connection()
            .execute_unprepared(index_sql)
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
DROP INDEX IF EXISTS idx_forum_replies_keyset;
DROP INDEX IF EXISTS idx_forum_topics_list_keyset;
DROP INDEX IF EXISTS idx_forum_topics_cursor;
DROP INDEX IF EXISTS idx_forum_categories_cursor;
"#,
            )
            .await?;
        Ok(())
    }
}
