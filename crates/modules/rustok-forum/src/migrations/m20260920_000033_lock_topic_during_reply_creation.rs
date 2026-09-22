use sea_orm::DatabaseBackend;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => Ok(()),
            backend => Err(DbErr::Custom(format!(
                "rustok-forum reply creation lock migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => Ok(()),
            backend => Err(DbErr::Custom(format!(
                "rustok-forum reply creation lock migration does not support {backend:?}"
            ))),
        }
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
CREATE OR REPLACE FUNCTION forum_validate_reply_creation()
RETURNS trigger AS $$
DECLARE
    topic_status text;
    topic_locked boolean;
BEGIN
    SELECT status::text, is_locked
      INTO topic_status, topic_locked
      FROM forum_topics
     WHERE tenant_id = NEW.tenant_id
       AND id = NEW.topic_id
     FOR UPDATE;

    IF topic_locked THEN
        RAISE EXCEPTION 'forum topic is locked';
    END IF;
    IF topic_status IS NOT NULL AND topic_status <> 'open' THEN
        RAISE EXCEPTION 'forum topic is not open';
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
CREATE OR REPLACE FUNCTION forum_validate_reply_creation()
RETURNS trigger AS $$
DECLARE
    topic_status text;
    topic_locked boolean;
BEGIN
    SELECT status::text, is_locked
      INTO topic_status, topic_locked
      FROM forum_topics
     WHERE tenant_id = NEW.tenant_id
       AND id = NEW.topic_id;

    IF topic_locked THEN
        RAISE EXCEPTION 'forum topic is locked';
    END IF;
    IF topic_status IS NOT NULL AND topic_status <> 'open' THEN
        RAISE EXCEPTION 'forum topic is not open';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
"#,
        )
        .await?;
    Ok(())
}
