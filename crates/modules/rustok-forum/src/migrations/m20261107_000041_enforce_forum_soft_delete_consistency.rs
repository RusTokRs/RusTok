use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => up_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "forum soft-delete consistency migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "forum soft-delete consistency migration does not support {backend:?}"
            ))),
        }
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM forum_topics
        WHERE deleted_at IS NOT NULL AND status != 'archived'
    ) THEN
        RAISE EXCEPTION
            'forum soft-delete consistency migration blocked: forum_topics deleted_at requires status archived';
    END IF;

    IF EXISTS (
        SELECT 1 FROM forum_replies
        WHERE (deleted_at IS NULL AND status = 'deleted')
           OR (deleted_at IS NOT NULL AND status != 'deleted')
    ) THEN
        RAISE EXCEPTION
            'forum soft-delete consistency migration blocked: forum_replies deleted_at/status mismatch';
    END IF;
END $$;

ALTER TABLE forum_topics
    DROP CONSTRAINT IF EXISTS chk_forum_topics_deleted_consistency;
ALTER TABLE forum_topics
    ADD CONSTRAINT chk_forum_topics_deleted_consistency
    CHECK ((deleted_at IS NULL) OR (deleted_at IS NOT NULL AND status = 'archived'));

ALTER TABLE forum_replies
    DROP CONSTRAINT IF EXISTS chk_forum_replies_deleted_consistency;
ALTER TABLE forum_replies
    ADD CONSTRAINT chk_forum_replies_deleted_consistency
    CHECK (
        (deleted_at IS NULL AND status != 'deleted')
        OR (deleted_at IS NOT NULL AND status = 'deleted')
    );
"#,
        )
        .await?;
    Ok(())
}

async fn down_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
ALTER TABLE forum_topics
    DROP CONSTRAINT IF EXISTS chk_forum_topics_deleted_consistency;
ALTER TABLE forum_replies
    DROP CONSTRAINT IF EXISTS chk_forum_replies_deleted_consistency;
"#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    ensure_no_invalid_deleted_rows(
        manager,
        "SELECT COUNT(*) AS invalid_count FROM forum_topics WHERE deleted_at IS NOT NULL AND status != 'archived'",
        "forum soft-delete consistency migration blocked: forum_topics deleted_at requires status archived",
    )
    .await?;
    ensure_no_invalid_deleted_rows(
        manager,
        "SELECT COUNT(*) AS invalid_count FROM forum_replies WHERE (deleted_at IS NULL AND status = 'deleted') OR (deleted_at IS NOT NULL AND status != 'deleted')",
        "forum soft-delete consistency migration blocked: forum_replies deleted_at/status mismatch",
    )
    .await?;

    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS forum_topics_deleted_consistency_insert",
        "DROP TRIGGER IF EXISTS forum_topics_deleted_consistency_update",
        "DROP TRIGGER IF EXISTS forum_replies_deleted_consistency_insert",
        "DROP TRIGGER IF EXISTS forum_replies_deleted_consistency_update",
        r#"CREATE TRIGGER forum_topics_deleted_consistency_insert
           BEFORE INSERT ON forum_topics
           FOR EACH ROW
           WHEN NEW.deleted_at IS NOT NULL AND NEW.status != 'archived'
           BEGIN
               SELECT RAISE(ABORT, 'forum soft-delete consistency violation: topic deleted_at requires archived status');
           END"#,
        r#"CREATE TRIGGER forum_topics_deleted_consistency_update
           BEFORE UPDATE OF deleted_at, status ON forum_topics
           FOR EACH ROW
           WHEN NEW.deleted_at IS NOT NULL AND NEW.status != 'archived'
           BEGIN
               SELECT RAISE(ABORT, 'forum soft-delete consistency violation: topic deleted_at requires archived status');
           END"#,
        r#"CREATE TRIGGER forum_replies_deleted_consistency_insert
           BEFORE INSERT ON forum_replies
           FOR EACH ROW
           WHEN (NEW.deleted_at IS NULL AND NEW.status = 'deleted') OR (NEW.deleted_at IS NOT NULL AND NEW.status != 'deleted')
           BEGIN
               SELECT RAISE(ABORT, 'forum soft-delete consistency violation: reply deleted_at/status mismatch');
           END"#,
        r#"CREATE TRIGGER forum_replies_deleted_consistency_update
           BEFORE UPDATE OF deleted_at, status ON forum_replies
           FOR EACH ROW
           WHEN (NEW.deleted_at IS NULL AND NEW.status = 'deleted') OR (NEW.deleted_at IS NOT NULL AND NEW.status != 'deleted')
           BEGIN
               SELECT RAISE(ABORT, 'forum soft-delete consistency violation: reply deleted_at/status mismatch');
           END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS forum_topics_deleted_consistency_insert",
        "DROP TRIGGER IF EXISTS forum_topics_deleted_consistency_update",
        "DROP TRIGGER IF EXISTS forum_replies_deleted_consistency_insert",
        "DROP TRIGGER IF EXISTS forum_replies_deleted_consistency_update",
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}

async fn ensure_no_invalid_deleted_rows(
    manager: &SchemaManager<'_>,
    query: &str,
    message: &str,
) -> Result<(), DbErr> {
    let row = manager
        .get_connection()
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            query.to_string(),
        ))
        .await?
        .ok_or_else(|| DbErr::Custom("failed to validate forum soft-delete consistency".to_string()))?;
    let invalid_count: i64 = row.try_get("", "invalid_count")?;
    if invalid_count != 0 {
        return Err(DbErr::Custom(message.to_string()));
    }
    Ok(())
}
