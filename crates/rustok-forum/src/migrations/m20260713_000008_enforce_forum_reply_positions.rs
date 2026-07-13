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
                "rustok-forum reply position migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum reply position migration does not support {backend:?}"
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
        FROM forum_replies
        WHERE position < 1
    ) THEN
        RAISE EXCEPTION
            'forum reply position migration blocked: non-positive position';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM forum_replies
        GROUP BY tenant_id, topic_id, position
        HAVING COUNT(*) > 1
    ) THEN
        RAISE EXCEPTION
            'forum reply position migration blocked: duplicate topic position';
    END IF;
END $$;

CREATE UNIQUE INDEX IF NOT EXISTS
    uq_forum_replies_tenant_topic_position
ON forum_replies (tenant_id, topic_id, position);

ALTER TABLE forum_replies
    DROP CONSTRAINT IF EXISTS chk_forum_replies_position_positive;
ALTER TABLE forum_replies
    ADD CONSTRAINT chk_forum_replies_position_positive
    CHECK (position > 0);

CREATE OR REPLACE FUNCTION forum_lock_reply_counter_mutation()
RETURNS trigger AS $$
DECLARE
    row_tenant_id uuid;
    row_category_id uuid;
    row_topic_id uuid;
    row_author_id uuid;
BEGIN
    IF TG_OP = 'DELETE' THEN
        row_tenant_id := OLD.tenant_id;
        row_topic_id := OLD.topic_id;
        row_author_id := OLD.author_id;
    ELSE
        row_tenant_id := NEW.tenant_id;
        row_topic_id := NEW.topic_id;
        row_author_id := NEW.author_id;
    END IF;

    SELECT category_id
      INTO row_category_id
      FROM forum_topics
     WHERE tenant_id = row_tenant_id
       AND id = row_topic_id;

    -- Keep the existing global lock order from FORUM-05. The topic lock is
    -- also the critical section for allocating the next reply position.
    IF row_category_id IS NOT NULL THEN
        PERFORM forum_counter_lock(
            format('forum:category:%s:%s', row_tenant_id, row_category_id)
        );
    END IF;
    PERFORM forum_counter_lock(
        format('forum:topic:%s:%s', row_tenant_id, row_topic_id)
    );
    IF row_author_id IS NOT NULL THEN
        PERFORM forum_counter_lock(
            format('forum:user:%s:%s', row_tenant_id, row_author_id)
        );
    END IF;

    IF TG_OP = 'INSERT' THEN
        SELECT COALESCE(MAX(reply.position), 0) + 1
          INTO NEW.position
          FROM forum_replies reply
         WHERE reply.tenant_id = row_tenant_id
           AND reply.topic_id = row_topic_id;
    END IF;

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
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
CREATE OR REPLACE FUNCTION forum_lock_reply_counter_mutation()
RETURNS trigger AS $$
DECLARE
    row_tenant_id uuid;
    row_category_id uuid;
    row_topic_id uuid;
    row_author_id uuid;
BEGIN
    IF TG_OP = 'DELETE' THEN
        row_tenant_id := OLD.tenant_id;
        row_topic_id := OLD.topic_id;
        row_author_id := OLD.author_id;
    ELSE
        row_tenant_id := NEW.tenant_id;
        row_topic_id := NEW.topic_id;
        row_author_id := NEW.author_id;
    END IF;

    SELECT category_id
      INTO row_category_id
      FROM forum_topics
     WHERE tenant_id = row_tenant_id
       AND id = row_topic_id;

    IF row_category_id IS NOT NULL THEN
        PERFORM forum_counter_lock(
            format('forum:category:%s:%s', row_tenant_id, row_category_id)
        );
    END IF;
    PERFORM forum_counter_lock(
        format('forum:topic:%s:%s', row_tenant_id, row_topic_id)
    );
    IF row_author_id IS NOT NULL THEN
        PERFORM forum_counter_lock(
            format('forum:user:%s:%s', row_tenant_id, row_author_id)
        );
    END IF;

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

ALTER TABLE forum_replies
    DROP CONSTRAINT IF EXISTS chk_forum_replies_position_positive;
DROP INDEX IF EXISTS uq_forum_replies_tenant_topic_position;
"#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    ensure_no_invalid_positions(manager).await?;

    let connection = manager.get_connection();
    for statement in [
        "CREATE UNIQUE INDEX IF NOT EXISTS
             uq_forum_replies_tenant_topic_position
         ON forum_replies (tenant_id, topic_id, position)",
        "DROP TRIGGER IF EXISTS forum_replies_position_positive_insert",
        "DROP TRIGGER IF EXISTS forum_replies_position_positive_update",
        r#"CREATE TRIGGER forum_replies_position_positive_insert
           BEFORE INSERT ON forum_replies
           FOR EACH ROW
           WHEN NEW.position < 1
           BEGIN
               SELECT RAISE(ABORT, 'forum reply position must be positive');
           END"#,
        r#"CREATE TRIGGER forum_replies_position_positive_update
           BEFORE UPDATE OF position ON forum_replies
           FOR EACH ROW
           WHEN NEW.position < 1
           BEGIN
               SELECT RAISE(ABORT, 'forum reply position must be positive');
           END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS forum_replies_position_positive_update",
        "DROP TRIGGER IF EXISTS forum_replies_position_positive_insert",
        "DROP INDEX IF EXISTS uq_forum_replies_tenant_topic_position",
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}

async fn ensure_no_invalid_positions(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let row = manager
        .get_connection()
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            r#"
SELECT COUNT(*) AS invalid_count
FROM (
    SELECT id
    FROM forum_replies
    WHERE position < 1

    UNION ALL

    SELECT MIN(id)
    FROM forum_replies
    GROUP BY tenant_id, topic_id, position
    HAVING COUNT(*) > 1
)
"#
            .to_string(),
        ))
        .await?
        .ok_or_else(|| DbErr::Custom("failed to validate forum reply positions".to_string()))?;
    let invalid_count: i64 = row.try_get("", "invalid_count")?;
    if invalid_count != 0 {
        return Err(DbErr::Custom(
            "forum reply position migration blocked: invalid existing positions".to_string(),
        ));
    }
    Ok(())
}
