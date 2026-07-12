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
                "rustok-forum status lifecycle migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum status lifecycle migration does not support {backend:?}"
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
        SELECT 1
        FROM forum_topics
        WHERE status NOT IN ('open', 'closed', 'archived')
    ) THEN
        RAISE EXCEPTION
            'forum status lifecycle migration blocked: unknown topic status';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM forum_replies
        WHERE status NOT IN (
            'pending',
            'approved',
            'rejected',
            'hidden',
            'flagged',
            'deleted'
        )
    ) THEN
        RAISE EXCEPTION
            'forum status lifecycle migration blocked: unknown reply status';
    END IF;
END $$;

ALTER TABLE forum_topics
    DROP CONSTRAINT IF EXISTS chk_forum_topics_status;
ALTER TABLE forum_topics
    ADD CONSTRAINT chk_forum_topics_status
    CHECK (status IN ('open', 'closed', 'archived'));

ALTER TABLE forum_replies
    DROP CONSTRAINT IF EXISTS chk_forum_replies_status;
ALTER TABLE forum_replies
    ADD CONSTRAINT chk_forum_replies_status
    CHECK (
        status IN (
            'pending',
            'approved',
            'rejected',
            'hidden',
            'flagged',
            'deleted'
        )
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
    DROP CONSTRAINT IF EXISTS chk_forum_topics_status;
ALTER TABLE forum_replies
    DROP CONSTRAINT IF EXISTS chk_forum_replies_status;
"#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    ensure_no_invalid_statuses(
        manager,
        "SELECT COUNT(*) AS invalid_count
         FROM forum_topics
         WHERE status NOT IN ('open', 'closed', 'archived')",
        "forum status lifecycle migration blocked: unknown topic status",
    )
    .await?;
    ensure_no_invalid_statuses(
        manager,
        "SELECT COUNT(*) AS invalid_count
         FROM forum_replies
         WHERE status NOT IN (
             'pending',
             'approved',
             'rejected',
             'hidden',
             'flagged',
             'deleted'
         )",
        "forum status lifecycle migration blocked: unknown reply status",
    )
    .await?;

    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS forum_topics_status_insert",
        "DROP TRIGGER IF EXISTS forum_topics_status_update",
        "DROP TRIGGER IF EXISTS forum_replies_status_insert",
        "DROP TRIGGER IF EXISTS forum_replies_status_update",
        r#"CREATE TRIGGER forum_topics_status_insert
           BEFORE INSERT ON forum_topics
           FOR EACH ROW
           WHEN NEW.status NOT IN ('open', 'closed', 'archived')
           BEGIN
               SELECT RAISE(ABORT, 'invalid forum topic status');
           END"#,
        r#"CREATE TRIGGER forum_topics_status_update
           BEFORE UPDATE OF status ON forum_topics
           FOR EACH ROW
           WHEN NEW.status NOT IN ('open', 'closed', 'archived')
           BEGIN
               SELECT RAISE(ABORT, 'invalid forum topic status');
           END"#,
        r#"CREATE TRIGGER forum_replies_status_insert
           BEFORE INSERT ON forum_replies
           FOR EACH ROW
           WHEN NEW.status NOT IN (
               'pending',
               'approved',
               'rejected',
               'hidden',
               'flagged',
               'deleted'
           )
           BEGIN
               SELECT RAISE(ABORT, 'invalid forum reply status');
           END"#,
        r#"CREATE TRIGGER forum_replies_status_update
           BEFORE UPDATE OF status ON forum_replies
           FOR EACH ROW
           WHEN NEW.status NOT IN (
               'pending',
               'approved',
               'rejected',
               'hidden',
               'flagged',
               'deleted'
           )
           BEGIN
               SELECT RAISE(ABORT, 'invalid forum reply status');
           END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }

    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS forum_topics_status_insert",
        "DROP TRIGGER IF EXISTS forum_topics_status_update",
        "DROP TRIGGER IF EXISTS forum_replies_status_insert",
        "DROP TRIGGER IF EXISTS forum_replies_status_update",
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}

async fn ensure_no_invalid_statuses(
    manager: &SchemaManager<'_>,
    query: &str,
    message: &str,
) -> Result<(), DbErr> {
    let row = manager
        .get_connection()
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            query.to_string(),
        ))
        .await?
        .ok_or_else(|| DbErr::Custom("failed to validate forum lifecycle statuses".to_string()))?;
    let invalid_count: i64 = row.try_get("", "invalid_count")?;
    if invalid_count != 0 {
        return Err(DbErr::Custom(message.to_string()));
    }
    Ok(())
}
