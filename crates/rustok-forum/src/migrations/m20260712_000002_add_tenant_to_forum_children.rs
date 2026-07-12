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
                "rustok-forum tenant child migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum tenant child migration does not support {backend:?}"
            ))),
        }
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
ALTER TABLE forum_topic_translations
    ADD COLUMN IF NOT EXISTS tenant_id UUID;
ALTER TABLE forum_reply_bodies
    ADD COLUMN IF NOT EXISTS tenant_id UUID;
ALTER TABLE forum_topic_channel_access
    ADD COLUMN IF NOT EXISTS tenant_id UUID;

UPDATE forum_topic_translations translation
SET tenant_id = topic.tenant_id
FROM forum_topics topic
WHERE translation.topic_id = topic.id
  AND translation.tenant_id IS NULL;

UPDATE forum_reply_bodies body
SET tenant_id = reply.tenant_id
FROM forum_replies reply
WHERE body.reply_id = reply.id
  AND body.tenant_id IS NULL;

UPDATE forum_topic_channel_access access
SET tenant_id = topic.tenant_id
FROM forum_topics topic
WHERE access.topic_id = topic.id
  AND access.tenant_id IS NULL;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM forum_topic_translations WHERE tenant_id IS NULL
    ) THEN
        RAISE EXCEPTION
            'forum tenant child migration blocked: orphan topic translation';
    END IF;
    IF EXISTS (
        SELECT 1 FROM forum_reply_bodies WHERE tenant_id IS NULL
    ) THEN
        RAISE EXCEPTION
            'forum tenant child migration blocked: orphan reply body';
    END IF;
    IF EXISTS (
        SELECT 1 FROM forum_topic_channel_access WHERE tenant_id IS NULL
    ) THEN
        RAISE EXCEPTION
            'forum tenant child migration blocked: orphan topic channel access';
    END IF;
END $$;

ALTER TABLE forum_topic_translations
    ALTER COLUMN tenant_id SET NOT NULL;
ALTER TABLE forum_reply_bodies
    ALTER COLUMN tenant_id SET NOT NULL;
ALTER TABLE forum_topic_channel_access
    ALTER COLUMN tenant_id SET NOT NULL;

ALTER TABLE forum_topic_translations
    DROP CONSTRAINT IF EXISTS fk_forum_topic_translations_topic;
ALTER TABLE forum_topic_translations
    DROP CONSTRAINT IF EXISTS fk_forum_topic_translations_topic_tenant;
ALTER TABLE forum_reply_bodies
    DROP CONSTRAINT IF EXISTS fk_forum_reply_bodies_reply;
ALTER TABLE forum_reply_bodies
    DROP CONSTRAINT IF EXISTS fk_forum_reply_bodies_reply_tenant;
ALTER TABLE forum_topic_channel_access
    DROP CONSTRAINT IF EXISTS fk_forum_topic_channel_access_topic;
ALTER TABLE forum_topic_channel_access
    DROP CONSTRAINT IF EXISTS fk_forum_topic_channel_access_topic_tenant;

ALTER TABLE forum_topic_translations
    ADD CONSTRAINT fk_forum_topic_translations_topic_tenant
    FOREIGN KEY (tenant_id, topic_id)
    REFERENCES forum_topics (tenant_id, id)
    ON UPDATE CASCADE
    ON DELETE CASCADE;

ALTER TABLE forum_reply_bodies
    ADD CONSTRAINT fk_forum_reply_bodies_reply_tenant
    FOREIGN KEY (tenant_id, reply_id)
    REFERENCES forum_replies (tenant_id, id)
    ON UPDATE CASCADE
    ON DELETE CASCADE;

ALTER TABLE forum_topic_channel_access
    ADD CONSTRAINT fk_forum_topic_channel_access_topic_tenant
    FOREIGN KEY (tenant_id, topic_id)
    REFERENCES forum_topics (tenant_id, id)
    ON UPDATE CASCADE
    ON DELETE CASCADE;

DROP INDEX IF EXISTS idx_forum_topic_translations_topic_locale;
CREATE UNIQUE INDEX IF NOT EXISTS
    uq_forum_topic_translations_tenant_topic_locale
    ON forum_topic_translations (tenant_id, topic_id, locale);

DROP INDEX IF EXISTS idx_forum_reply_bodies_reply_locale;
CREATE UNIQUE INDEX IF NOT EXISTS
    uq_forum_reply_bodies_tenant_reply_locale
    ON forum_reply_bodies (tenant_id, reply_id, locale);

ALTER TABLE forum_topic_channel_access
    DROP CONSTRAINT IF EXISTS forum_topic_channel_access_pkey;
ALTER TABLE forum_topic_channel_access
    ADD CONSTRAINT forum_topic_channel_access_pkey
    PRIMARY KEY (tenant_id, topic_id, channel_slug);

DROP INDEX IF EXISTS idx_forum_topic_channel_access_channel;
CREATE INDEX IF NOT EXISTS idx_forum_topic_channel_access_tenant_channel
    ON forum_topic_channel_access (tenant_id, channel_slug, topic_id);
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
ALTER TABLE forum_topic_translations
    DROP CONSTRAINT IF EXISTS fk_forum_topic_translations_topic_tenant;
ALTER TABLE forum_reply_bodies
    DROP CONSTRAINT IF EXISTS fk_forum_reply_bodies_reply_tenant;
ALTER TABLE forum_topic_channel_access
    DROP CONSTRAINT IF EXISTS fk_forum_topic_channel_access_topic_tenant;

ALTER TABLE forum_topic_translations
    ADD CONSTRAINT fk_forum_topic_translations_topic
    FOREIGN KEY (topic_id) REFERENCES forum_topics (id)
    ON UPDATE CASCADE ON DELETE CASCADE;
ALTER TABLE forum_reply_bodies
    ADD CONSTRAINT fk_forum_reply_bodies_reply
    FOREIGN KEY (reply_id) REFERENCES forum_replies (id)
    ON UPDATE CASCADE ON DELETE CASCADE;
ALTER TABLE forum_topic_channel_access
    ADD CONSTRAINT fk_forum_topic_channel_access_topic
    FOREIGN KEY (topic_id) REFERENCES forum_topics (id)
    ON UPDATE CASCADE ON DELETE CASCADE;

DROP INDEX IF EXISTS uq_forum_topic_translations_tenant_topic_locale;
CREATE UNIQUE INDEX IF NOT EXISTS idx_forum_topic_translations_topic_locale
    ON forum_topic_translations (topic_id, locale);
DROP INDEX IF EXISTS uq_forum_reply_bodies_tenant_reply_locale;
CREATE UNIQUE INDEX IF NOT EXISTS idx_forum_reply_bodies_reply_locale
    ON forum_reply_bodies (reply_id, locale);

ALTER TABLE forum_topic_channel_access
    DROP CONSTRAINT IF EXISTS forum_topic_channel_access_pkey;
ALTER TABLE forum_topic_channel_access
    ADD CONSTRAINT forum_topic_channel_access_pkey
    PRIMARY KEY (topic_id, channel_slug);

DROP INDEX IF EXISTS idx_forum_topic_channel_access_tenant_channel;
CREATE INDEX IF NOT EXISTS idx_forum_topic_channel_access_channel
    ON forum_topic_channel_access (channel_slug, topic_id);

ALTER TABLE forum_topic_translations DROP COLUMN tenant_id;
ALTER TABLE forum_reply_bodies DROP COLUMN tenant_id;
ALTER TABLE forum_topic_channel_access DROP COLUMN tenant_id;
"#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();

    for statement in [
        "ALTER TABLE forum_topic_translations ADD COLUMN tenant_id TEXT",
        "ALTER TABLE forum_reply_bodies ADD COLUMN tenant_id TEXT",
        "ALTER TABLE forum_topic_channel_access ADD COLUMN tenant_id TEXT",
        "UPDATE forum_topic_translations
         SET tenant_id = (
             SELECT topic.tenant_id FROM forum_topics topic
             WHERE topic.id = forum_topic_translations.topic_id
         ) WHERE tenant_id IS NULL",
        "UPDATE forum_reply_bodies
         SET tenant_id = (
             SELECT reply.tenant_id FROM forum_replies reply
             WHERE reply.id = forum_reply_bodies.reply_id
         ) WHERE tenant_id IS NULL",
        "UPDATE forum_topic_channel_access
         SET tenant_id = (
             SELECT topic.tenant_id FROM forum_topics topic
             WHERE topic.id = forum_topic_channel_access.topic_id
         ) WHERE tenant_id IS NULL",
    ] {
        connection.execute_unprepared(statement).await?;
    }

    ensure_no_null_tenants(
        manager,
        "forum_topic_translations",
        "forum tenant child migration blocked: orphan topic translation",
    )
    .await?;
    ensure_no_null_tenants(
        manager,
        "forum_reply_bodies",
        "forum tenant child migration blocked: orphan reply body",
    )
    .await?;
    ensure_no_null_tenants(
        manager,
        "forum_topic_channel_access",
        "forum tenant child migration blocked: orphan topic channel access",
    )
    .await?;

    for statement in [
        "DROP INDEX IF EXISTS idx_forum_topic_translations_topic_locale",
        "CREATE UNIQUE INDEX uq_forum_topic_translations_tenant_topic_locale
         ON forum_topic_translations (tenant_id, topic_id, locale)",
        "DROP INDEX IF EXISTS idx_forum_reply_bodies_reply_locale",
        "CREATE UNIQUE INDEX uq_forum_reply_bodies_tenant_reply_locale
         ON forum_reply_bodies (tenant_id, reply_id, locale)",
        "DROP INDEX IF EXISTS idx_forum_topic_channel_access_channel",
        "CREATE UNIQUE INDEX uq_forum_topic_channel_access_tenant_topic_channel
         ON forum_topic_channel_access (tenant_id, topic_id, channel_slug)",
        "CREATE INDEX idx_forum_topic_channel_access_tenant_channel
         ON forum_topic_channel_access (tenant_id, channel_slug, topic_id)",
        r#"CREATE TRIGGER forum_topic_translations_tenant_insert
           BEFORE INSERT ON forum_topic_translations
           FOR EACH ROW
           WHEN NEW.tenant_id IS NULL OR NOT EXISTS (
               SELECT 1 FROM forum_topics topic
               WHERE topic.id = NEW.topic_id
                 AND topic.tenant_id = NEW.tenant_id
           )
           BEGIN
               SELECT RAISE(ABORT, 'forum topic translation tenant mismatch');
           END"#,
        r#"CREATE TRIGGER forum_topic_translations_tenant_update
           BEFORE UPDATE OF tenant_id, topic_id ON forum_topic_translations
           FOR EACH ROW
           WHEN NEW.tenant_id IS NULL OR NOT EXISTS (
               SELECT 1 FROM forum_topics topic
               WHERE topic.id = NEW.topic_id
                 AND topic.tenant_id = NEW.tenant_id
           )
           BEGIN
               SELECT RAISE(ABORT, 'forum topic translation tenant mismatch');
           END"#,
        r#"CREATE TRIGGER forum_reply_bodies_tenant_insert
           BEFORE INSERT ON forum_reply_bodies
           FOR EACH ROW
           WHEN NEW.tenant_id IS NULL OR NOT EXISTS (
               SELECT 1 FROM forum_replies reply
               WHERE reply.id = NEW.reply_id
                 AND reply.tenant_id = NEW.tenant_id
           )
           BEGIN
               SELECT RAISE(ABORT, 'forum reply body tenant mismatch');
           END"#,
        r#"CREATE TRIGGER forum_reply_bodies_tenant_update
           BEFORE UPDATE OF tenant_id, reply_id ON forum_reply_bodies
           FOR EACH ROW
           WHEN NEW.tenant_id IS NULL OR NOT EXISTS (
               SELECT 1 FROM forum_replies reply
               WHERE reply.id = NEW.reply_id
                 AND reply.tenant_id = NEW.tenant_id
           )
           BEGIN
               SELECT RAISE(ABORT, 'forum reply body tenant mismatch');
           END"#,
        r#"CREATE TRIGGER forum_topic_channel_access_tenant_insert
           BEFORE INSERT ON forum_topic_channel_access
           FOR EACH ROW
           WHEN NEW.tenant_id IS NULL OR NOT EXISTS (
               SELECT 1 FROM forum_topics topic
               WHERE topic.id = NEW.topic_id
                 AND topic.tenant_id = NEW.tenant_id
           )
           BEGIN
               SELECT RAISE(ABORT, 'forum topic channel tenant mismatch');
           END"#,
        r#"CREATE TRIGGER forum_topic_channel_access_tenant_update
           BEFORE UPDATE OF tenant_id, topic_id ON forum_topic_channel_access
           FOR EACH ROW
           WHEN NEW.tenant_id IS NULL OR NOT EXISTS (
               SELECT 1 FROM forum_topics topic
               WHERE topic.id = NEW.topic_id
                 AND topic.tenant_id = NEW.tenant_id
           )
           BEGIN
               SELECT RAISE(ABORT, 'forum topic channel tenant mismatch');
           END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }

    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS forum_topic_translations_tenant_insert",
        "DROP TRIGGER IF EXISTS forum_topic_translations_tenant_update",
        "DROP TRIGGER IF EXISTS forum_reply_bodies_tenant_insert",
        "DROP TRIGGER IF EXISTS forum_reply_bodies_tenant_update",
        "DROP TRIGGER IF EXISTS forum_topic_channel_access_tenant_insert",
        "DROP TRIGGER IF EXISTS forum_topic_channel_access_tenant_update",
        "DROP INDEX IF EXISTS uq_forum_topic_translations_tenant_topic_locale",
        "DROP INDEX IF EXISTS uq_forum_reply_bodies_tenant_reply_locale",
        "DROP INDEX IF EXISTS uq_forum_topic_channel_access_tenant_topic_channel",
        "DROP INDEX IF EXISTS idx_forum_topic_channel_access_tenant_channel",
        "ALTER TABLE forum_topic_translations DROP COLUMN tenant_id",
        "ALTER TABLE forum_reply_bodies DROP COLUMN tenant_id",
        "ALTER TABLE forum_topic_channel_access DROP COLUMN tenant_id",
        "CREATE UNIQUE INDEX idx_forum_topic_translations_topic_locale
         ON forum_topic_translations (topic_id, locale)",
        "CREATE UNIQUE INDEX idx_forum_reply_bodies_reply_locale
         ON forum_reply_bodies (reply_id, locale)",
        "CREATE INDEX idx_forum_topic_channel_access_channel
         ON forum_topic_channel_access (channel_slug, topic_id)",
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}

async fn ensure_no_null_tenants(
    manager: &SchemaManager<'_>,
    table: &str,
    message: &str,
) -> Result<(), DbErr> {
    let row = manager
        .get_connection()
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!("SELECT COUNT(*) AS invalid_count FROM {table} WHERE tenant_id IS NULL"),
        ))
        .await?
        .ok_or_else(|| DbErr::Custom(format!("failed to validate {table} tenant backfill")))?;
    let invalid_count: i64 = row.try_get("", "invalid_count")?;
    if invalid_count != 0 {
        return Err(DbErr::Custom(message.to_owned()));
    }
    Ok(())
}
