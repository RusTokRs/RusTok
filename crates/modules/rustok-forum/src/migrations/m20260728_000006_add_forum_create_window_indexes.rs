use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => execute(manager, POSTGRES_UP).await,
            DatabaseBackend::Sqlite => execute(manager, SQLITE_UP).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum create-window index migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => execute(manager, POSTGRES_DOWN).await,
            DatabaseBackend::Sqlite => execute(manager, SQLITE_DOWN).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum create-window index migration does not support {backend:?}"
            ))),
        }
    }
}

async fn execute(manager: &SchemaManager<'_>, sql: &str) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(sql)
        .await
        .map(|_| ())
}

const POSTGRES_UP: &str = r#"
CREATE INDEX IF NOT EXISTS idx_forum_topics_tenant_author_created_at
    ON forum_topics (tenant_id, author_id, created_at DESC)
    WHERE author_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_forum_replies_tenant_author_created_at
    ON forum_replies (tenant_id, author_id, created_at DESC)
    WHERE author_id IS NOT NULL;
"#;

const SQLITE_UP: &str = r#"
CREATE INDEX IF NOT EXISTS idx_forum_topics_tenant_author_created_at
    ON forum_topics (tenant_id, author_id, created_at DESC)
    WHERE author_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_forum_replies_tenant_author_created_at
    ON forum_replies (tenant_id, author_id, created_at DESC)
    WHERE author_id IS NOT NULL;
"#;

const POSTGRES_DOWN: &str = r#"
DROP INDEX IF EXISTS idx_forum_replies_tenant_author_created_at;
DROP INDEX IF EXISTS idx_forum_topics_tenant_author_created_at;
"#;

const SQLITE_DOWN: &str = r#"
DROP INDEX IF EXISTS idx_forum_replies_tenant_author_created_at;
DROP INDEX IF EXISTS idx_forum_topics_tenant_author_created_at;
"#;
