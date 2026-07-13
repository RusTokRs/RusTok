use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
CREATE INDEX IF NOT EXISTS idx_forum_categories_cursor
    ON forum_categories (tenant_id, position, id);

CREATE INDEX IF NOT EXISTS idx_forum_topics_cursor
    ON forum_topics (tenant_id, updated_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_forum_replies_cursor
    ON forum_replies (tenant_id, topic_id, position, id);
"#,
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
DROP INDEX IF EXISTS idx_forum_replies_cursor;
DROP INDEX IF EXISTS idx_forum_topics_cursor;
DROP INDEX IF EXISTS idx_forum_categories_cursor;
"#,
            )
            .await?;
        Ok(())
    }
}
