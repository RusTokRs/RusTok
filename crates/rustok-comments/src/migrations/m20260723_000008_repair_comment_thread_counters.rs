use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => repair_postgres(manager).await?,
            DatabaseBackend::Sqlite => repair_sqlite(manager).await?,
            backend => {
                return Err(DbErr::Custom(format!(
                    "rustok-comments counter repair does not support {backend:?}"
                )));
            }
        }

        manager
            .drop_index(
                Index::drop()
                    .name("idx_comments_thread_position")
                    .table(Comments::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_comments_thread_position")
                    .table(Comments::Table)
                    .col(Comments::ThreadId)
                    .col(Comments::Position)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_comments_thread_position")
                    .table(Comments::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_comments_thread_position")
                    .table(Comments::Table)
                    .col(Comments::ThreadId)
                    .col(Comments::Position)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

async fn repair_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
UPDATE comment_threads AS thread
SET comment_count = counts.comment_count
FROM (
    SELECT
        thread_source.id,
        COUNT(comment.id)::INTEGER AS comment_count
    FROM comment_threads AS thread_source
    LEFT JOIN comments AS comment
        ON comment.thread_id = thread_source.id
       AND comment.tenant_id = thread_source.tenant_id
       AND comment.deleted_at IS NULL
    GROUP BY thread_source.id
) AS counts
WHERE thread.id = counts.id;

WITH ranked AS (
    SELECT
        id,
        ROW_NUMBER() OVER (
            PARTITION BY thread_id
            ORDER BY position ASC, created_at ASC, id ASC
        ) AS repaired_position
    FROM comments
)
UPDATE comments AS comment
SET position = ranked.repaired_position
FROM ranked
WHERE comment.id = ranked.id;
"#,
        )
        .await?;
    Ok(())
}

async fn repair_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
UPDATE comment_threads
SET comment_count = (
    SELECT COUNT(*)
    FROM comments AS comment
    WHERE comment.thread_id = comment_threads.id
      AND comment.tenant_id = comment_threads.tenant_id
      AND comment.deleted_at IS NULL
);

WITH ranked AS (
    SELECT
        id,
        ROW_NUMBER() OVER (
            PARTITION BY thread_id
            ORDER BY position ASC, created_at ASC, id ASC
        ) AS repaired_position
    FROM comments
)
UPDATE comments
SET position = (
    SELECT ranked.repaired_position
    FROM ranked
    WHERE ranked.id = comments.id
);
"#,
        )
        .await?;
    Ok(())
}

#[derive(Iden)]
enum Comments {
    Table,
    ThreadId,
    Position,
}
