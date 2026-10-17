use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let index_sql = if manager.get_database_backend() == DatabaseBackend::Sqlite {
            r#"
CREATE INDEX IF NOT EXISTS idx_forum_topics_top_keyset
    ON forum_topics (tenant_id, reply_count DESC, is_pinned DESC, last_reply_at DESC, updated_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_forum_topics_category_top_keyset
    ON forum_topics (tenant_id, category_id, reply_count DESC, is_pinned DESC, last_reply_at DESC, updated_at DESC, id DESC);
"#
        } else {
            r#"
CREATE INDEX IF NOT EXISTS idx_forum_topics_top_keyset
    ON forum_topics (tenant_id, reply_count DESC, is_pinned DESC, last_reply_at DESC NULLS LAST, updated_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_forum_topics_category_top_keyset
    ON forum_topics (tenant_id, category_id, reply_count DESC, is_pinned DESC, last_reply_at DESC NULLS LAST, updated_at DESC, id DESC);
"#
        };
        manager
            .get_connection()
            .execute_unprepared(index_sql)
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(
            "DROP INDEX IF EXISTS idx_forum_topics_category_top_keyset; DROP INDEX IF EXISTS idx_forum_topics_top_keyset;"
        ).await?;
        Ok(())
    }
}
