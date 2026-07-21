use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => up_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum mention relation migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum mention relation rollback does not support {backend:?}"
            ))),
        }
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
CREATE TABLE IF NOT EXISTS forum_relation_revisions (
    revision_id BIGSERIAL PRIMARY KEY,
    tenant_id UUID NOT NULL,
    target_kind VARCHAR(16) NOT NULL,
    target_id UUID NOT NULL,
    locale VARCHAR(32) NOT NULL,
    projection_fingerprint VARCHAR(64) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT chk_forum_relation_revision_kind
        CHECK (target_kind IN ('topic', 'reply')),
    CONSTRAINT uq_forum_relation_revision_identity
        UNIQUE (tenant_id, target_kind, target_id, locale, revision_id)
);

CREATE INDEX IF NOT EXISTS idx_forum_relation_revision_stream
    ON forum_relation_revisions
        (tenant_id, target_kind, target_id, locale, revision_id DESC);

CREATE TABLE IF NOT EXISTS forum_user_mentions (
    tenant_id UUID NOT NULL,
    source_kind VARCHAR(16) NOT NULL,
    source_id UUID NOT NULL,
    source_locale VARCHAR(32) NOT NULL,
    source_revision_id BIGINT NOT NULL,
    mentioned_user_id UUID NOT NULL,
    handle_snapshot VARCHAR(32) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (
        tenant_id,
        source_kind,
        source_id,
        source_locale,
        source_revision_id,
        mentioned_user_id
    ),
    CONSTRAINT chk_forum_user_mentions_source_kind
        CHECK (source_kind IN ('topic', 'reply')),
    CONSTRAINT chk_forum_user_mentions_handle
        CHECK (char_length(handle_snapshot) BETWEEN 3 AND 32),
    CONSTRAINT fk_forum_user_mentions_revision
        FOREIGN KEY (source_revision_id)
        REFERENCES forum_relation_revisions (revision_id)
        ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS forum_audience_mentions (
    tenant_id UUID NOT NULL,
    source_kind VARCHAR(16) NOT NULL,
    source_id UUID NOT NULL,
    source_locale VARCHAR(32) NOT NULL,
    source_revision_id BIGINT NOT NULL,
    audience VARCHAR(32) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (
        tenant_id,
        source_kind,
        source_id,
        source_locale,
        source_revision_id,
        audience
    ),
    CONSTRAINT chk_forum_audience_mentions_source_kind
        CHECK (source_kind IN ('topic', 'reply')),
    CONSTRAINT chk_forum_audience_mentions_audience
        CHECK (audience IN ('moderators')),
    CONSTRAINT fk_forum_audience_mentions_revision
        FOREIGN KEY (source_revision_id)
        REFERENCES forum_relation_revisions (revision_id)
        ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS forum_quotes (
    tenant_id UUID NOT NULL,
    source_kind VARCHAR(16) NOT NULL,
    source_id UUID NOT NULL,
    source_locale VARCHAR(32) NOT NULL,
    source_revision_id BIGINT NOT NULL,
    quoted_kind VARCHAR(16) NOT NULL,
    quoted_id UUID NOT NULL,
    quoted_revision_id BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (
        tenant_id,
        source_kind,
        source_id,
        source_locale,
        source_revision_id,
        quoted_kind,
        quoted_id,
        quoted_revision_id
    ),
    CONSTRAINT chk_forum_quotes_source_kind
        CHECK (source_kind IN ('topic', 'reply')),
    CONSTRAINT chk_forum_quotes_quoted_kind
        CHECK (quoted_kind IN ('topic', 'reply')),
    CONSTRAINT fk_forum_quotes_source_revision
        FOREIGN KEY (source_revision_id)
        REFERENCES forum_relation_revisions (revision_id)
        ON DELETE CASCADE,
    CONSTRAINT fk_forum_quotes_quoted_revision
        FOREIGN KEY (quoted_revision_id)
        REFERENCES forum_relation_revisions (revision_id)
        ON DELETE RESTRICT
);

CREATE OR REPLACE FUNCTION forum_validate_relation_revision_source()
RETURNS trigger AS $$
BEGIN
    IF NEW.target_kind = 'topic' THEN
        IF NOT EXISTS (
            SELECT 1
            FROM forum_topic_translations translation
            WHERE translation.tenant_id = NEW.tenant_id
              AND translation.topic_id = NEW.target_id
              AND translation.locale = NEW.locale
        ) THEN
            RAISE EXCEPTION 'forum relation revision topic source mismatch';
        END IF;
    ELSIF NEW.target_kind = 'reply' THEN
        IF NOT EXISTS (
            SELECT 1
            FROM forum_reply_bodies body
            WHERE body.tenant_id = NEW.tenant_id
              AND body.reply_id = NEW.target_id
              AND body.locale = NEW.locale
        ) THEN
            RAISE EXCEPTION 'forum relation revision reply source mismatch';
        END IF;
    ELSE
        RAISE EXCEPTION 'forum relation revision target kind is invalid';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_reject_relation_update()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'forum relation projections are immutable';
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_validate_relation_child_source()
RETURNS trigger AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM forum_relation_revisions revision
        WHERE revision.revision_id = NEW.source_revision_id
          AND revision.tenant_id = NEW.tenant_id
          AND revision.target_kind = NEW.source_kind
          AND revision.target_id = NEW.source_id
          AND revision.locale = NEW.source_locale
    ) THEN
        RAISE EXCEPTION 'forum relation child source mismatch';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_validate_quote_target()
RETURNS trigger AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM forum_relation_revisions revision
        WHERE revision.revision_id = NEW.quoted_revision_id
          AND revision.tenant_id = NEW.tenant_id
          AND revision.target_kind = NEW.quoted_kind
          AND revision.target_id = NEW.quoted_id
    ) THEN
        RAISE EXCEPTION 'forum quote target mismatch';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_relation_revision_source_guard
    ON forum_relation_revisions;
CREATE TRIGGER forum_relation_revision_source_guard
BEFORE INSERT ON forum_relation_revisions
FOR EACH ROW
EXECUTE FUNCTION forum_validate_relation_revision_source();

DROP TRIGGER IF EXISTS forum_relation_revision_immutable_guard
    ON forum_relation_revisions;
CREATE TRIGGER forum_relation_revision_immutable_guard
BEFORE UPDATE ON forum_relation_revisions
FOR EACH ROW
EXECUTE FUNCTION forum_reject_relation_update();

DROP TRIGGER IF EXISTS forum_user_mentions_source_guard
    ON forum_user_mentions;
CREATE TRIGGER forum_user_mentions_source_guard
BEFORE INSERT ON forum_user_mentions
FOR EACH ROW
EXECUTE FUNCTION forum_validate_relation_child_source();

DROP TRIGGER IF EXISTS forum_user_mentions_immutable_guard
    ON forum_user_mentions;
CREATE TRIGGER forum_user_mentions_immutable_guard
BEFORE UPDATE ON forum_user_mentions
FOR EACH ROW
EXECUTE FUNCTION forum_reject_relation_update();

DROP TRIGGER IF EXISTS forum_audience_mentions_source_guard
    ON forum_audience_mentions;
CREATE TRIGGER forum_audience_mentions_source_guard
BEFORE INSERT ON forum_audience_mentions
FOR EACH ROW
EXECUTE FUNCTION forum_validate_relation_child_source();

DROP TRIGGER IF EXISTS forum_audience_mentions_immutable_guard
    ON forum_audience_mentions;
CREATE TRIGGER forum_audience_mentions_immutable_guard
BEFORE UPDATE ON forum_audience_mentions
FOR EACH ROW
EXECUTE FUNCTION forum_reject_relation_update();

DROP TRIGGER IF EXISTS forum_quotes_source_guard ON forum_quotes;
CREATE TRIGGER forum_quotes_source_guard
BEFORE INSERT ON forum_quotes
FOR EACH ROW
EXECUTE FUNCTION forum_validate_relation_child_source();

DROP TRIGGER IF EXISTS forum_quotes_target_guard ON forum_quotes;
CREATE TRIGGER forum_quotes_target_guard
BEFORE INSERT ON forum_quotes
FOR EACH ROW
EXECUTE FUNCTION forum_validate_quote_target();

DROP TRIGGER IF EXISTS forum_quotes_immutable_guard ON forum_quotes;
CREATE TRIGGER forum_quotes_immutable_guard
BEFORE UPDATE ON forum_quotes
FOR EACH ROW
EXECUTE FUNCTION forum_reject_relation_update();

INSERT INTO forum_relation_revisions (
    tenant_id,
    target_kind,
    target_id,
    locale,
    projection_fingerprint
)
SELECT
    translation.tenant_id,
    'topic',
    translation.topic_id,
    translation.locale,
    'legacy'
FROM forum_topic_translations translation
WHERE NOT EXISTS (
    SELECT 1
    FROM forum_relation_revisions revision
    WHERE revision.tenant_id = translation.tenant_id
      AND revision.target_kind = 'topic'
      AND revision.target_id = translation.topic_id
      AND revision.locale = translation.locale
);

INSERT INTO forum_relation_revisions (
    tenant_id,
    target_kind,
    target_id,
    locale,
    projection_fingerprint
)
SELECT
    body.tenant_id,
    'reply',
    body.reply_id,
    body.locale,
    'legacy'
FROM forum_reply_bodies body
WHERE NOT EXISTS (
    SELECT 1
    FROM forum_relation_revisions revision
    WHERE revision.tenant_id = body.tenant_id
      AND revision.target_kind = 'reply'
      AND revision.target_id = body.reply_id
      AND revision.locale = body.locale
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
DROP TRIGGER IF EXISTS forum_quotes_immutable_guard ON forum_quotes;
DROP TRIGGER IF EXISTS forum_quotes_target_guard ON forum_quotes;
DROP TRIGGER IF EXISTS forum_quotes_source_guard ON forum_quotes;
DROP TRIGGER IF EXISTS forum_audience_mentions_immutable_guard ON forum_audience_mentions;
DROP TRIGGER IF EXISTS forum_audience_mentions_source_guard ON forum_audience_mentions;
DROP TRIGGER IF EXISTS forum_user_mentions_immutable_guard ON forum_user_mentions;
DROP TRIGGER IF EXISTS forum_user_mentions_source_guard ON forum_user_mentions;
DROP TRIGGER IF EXISTS forum_relation_revision_immutable_guard ON forum_relation_revisions;
DROP TRIGGER IF EXISTS forum_relation_revision_source_guard ON forum_relation_revisions;
DROP FUNCTION IF EXISTS forum_validate_quote_target();
DROP FUNCTION IF EXISTS forum_validate_relation_child_source();
DROP FUNCTION IF EXISTS forum_reject_relation_update();
DROP FUNCTION IF EXISTS forum_validate_relation_revision_source();
DROP TABLE IF EXISTS forum_quotes;
DROP TABLE IF EXISTS forum_audience_mentions;
DROP TABLE IF EXISTS forum_user_mentions;
DROP TABLE IF EXISTS forum_relation_revisions;
"#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        r#"CREATE TABLE IF NOT EXISTS forum_relation_revisions (
            revision_id INTEGER PRIMARY KEY AUTOINCREMENT,
            tenant_id TEXT NOT NULL,
            target_kind TEXT NOT NULL CHECK (target_kind IN ('topic', 'reply')),
            target_id TEXT NOT NULL,
            locale TEXT NOT NULL,
            projection_fingerprint TEXT NOT NULL CHECK (length(projection_fingerprint) <= 64),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE (tenant_id, target_kind, target_id, locale, revision_id)
        )"#,
        r#"CREATE INDEX IF NOT EXISTS idx_forum_relation_revision_stream
            ON forum_relation_revisions
                (tenant_id, target_kind, target_id, locale, revision_id DESC)"#,
        r#"CREATE TABLE IF NOT EXISTS forum_user_mentions (
            tenant_id TEXT NOT NULL,
            source_kind TEXT NOT NULL CHECK (source_kind IN ('topic', 'reply')),
            source_id TEXT NOT NULL,
            source_locale TEXT NOT NULL,
            source_revision_id INTEGER NOT NULL,
            mentioned_user_id TEXT NOT NULL,
            handle_snapshot TEXT NOT NULL CHECK (length(handle_snapshot) BETWEEN 3 AND 32),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (
                tenant_id, source_kind, source_id, source_locale,
                source_revision_id, mentioned_user_id
            ),
            FOREIGN KEY (source_revision_id)
                REFERENCES forum_relation_revisions (revision_id)
                ON DELETE CASCADE
        )"#,
        r#"CREATE TABLE IF NOT EXISTS forum_audience_mentions (
            tenant_id TEXT NOT NULL,
            source_kind TEXT NOT NULL CHECK (source_kind IN ('topic', 'reply')),
            source_id TEXT NOT NULL,
            source_locale TEXT NOT NULL,
            source_revision_id INTEGER NOT NULL,
            audience TEXT NOT NULL CHECK (audience IN ('moderators')),
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (
                tenant_id, source_kind, source_id, source_locale,
                source_revision_id, audience
            ),
            FOREIGN KEY (source_revision_id)
                REFERENCES forum_relation_revisions (revision_id)
                ON DELETE CASCADE
        )"#,
        r#"CREATE TABLE IF NOT EXISTS forum_quotes (
            tenant_id TEXT NOT NULL,
            source_kind TEXT NOT NULL CHECK (source_kind IN ('topic', 'reply')),
            source_id TEXT NOT NULL,
            source_locale TEXT NOT NULL,
            source_revision_id INTEGER NOT NULL,
            quoted_kind TEXT NOT NULL CHECK (quoted_kind IN ('topic', 'reply')),
            quoted_id TEXT NOT NULL,
            quoted_revision_id INTEGER NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (
                tenant_id, source_kind, source_id, source_locale,
                source_revision_id, quoted_kind, quoted_id, quoted_revision_id
            ),
            FOREIGN KEY (source_revision_id)
                REFERENCES forum_relation_revisions (revision_id)
                ON DELETE CASCADE,
            FOREIGN KEY (quoted_revision_id)
                REFERENCES forum_relation_revisions (revision_id)
                ON DELETE RESTRICT
        )"#,
        "DROP TRIGGER IF EXISTS forum_relation_revision_source_guard",
        r#"CREATE TRIGGER forum_relation_revision_source_guard
            BEFORE INSERT ON forum_relation_revisions
            FOR EACH ROW
            WHEN (
                NEW.target_kind = 'topic'
                AND NOT EXISTS (
                    SELECT 1 FROM forum_topic_translations translation
                    WHERE translation.tenant_id = NEW.tenant_id
                      AND translation.topic_id = NEW.target_id
                      AND translation.locale = NEW.locale
                )
            ) OR (
                NEW.target_kind = 'reply'
                AND NOT EXISTS (
                    SELECT 1 FROM forum_reply_bodies body
                    WHERE body.tenant_id = NEW.tenant_id
                      AND body.reply_id = NEW.target_id
                      AND body.locale = NEW.locale
                )
            )
            BEGIN
                SELECT RAISE(ABORT, 'forum relation revision source mismatch');
            END"#,
        "DROP TRIGGER IF EXISTS forum_relation_revision_immutable_guard",
        r#"CREATE TRIGGER forum_relation_revision_immutable_guard
            BEFORE UPDATE ON forum_relation_revisions
            FOR EACH ROW
            BEGIN
                SELECT RAISE(ABORT, 'forum relation projections are immutable');
            END"#,
        "DROP TRIGGER IF EXISTS forum_user_mentions_source_guard",
        r#"CREATE TRIGGER forum_user_mentions_source_guard
            BEFORE INSERT ON forum_user_mentions
            FOR EACH ROW
            WHEN NOT EXISTS (
                SELECT 1 FROM forum_relation_revisions revision
                WHERE revision.revision_id = NEW.source_revision_id
                  AND revision.tenant_id = NEW.tenant_id
                  AND revision.target_kind = NEW.source_kind
                  AND revision.target_id = NEW.source_id
                  AND revision.locale = NEW.source_locale
            )
            BEGIN
                SELECT RAISE(ABORT, 'forum relation child source mismatch');
            END"#,
        "DROP TRIGGER IF EXISTS forum_user_mentions_immutable_guard",
        r#"CREATE TRIGGER forum_user_mentions_immutable_guard
            BEFORE UPDATE ON forum_user_mentions
            FOR EACH ROW
            BEGIN
                SELECT RAISE(ABORT, 'forum relation projections are immutable');
            END"#,
        "DROP TRIGGER IF EXISTS forum_audience_mentions_source_guard",
        r#"CREATE TRIGGER forum_audience_mentions_source_guard
            BEFORE INSERT ON forum_audience_mentions
            FOR EACH ROW
            WHEN NOT EXISTS (
                SELECT 1 FROM forum_relation_revisions revision
                WHERE revision.revision_id = NEW.source_revision_id
                  AND revision.tenant_id = NEW.tenant_id
                  AND revision.target_kind = NEW.source_kind
                  AND revision.target_id = NEW.source_id
                  AND revision.locale = NEW.source_locale
            )
            BEGIN
                SELECT RAISE(ABORT, 'forum relation child source mismatch');
            END"#,
        "DROP TRIGGER IF EXISTS forum_audience_mentions_immutable_guard",
        r#"CREATE TRIGGER forum_audience_mentions_immutable_guard
            BEFORE UPDATE ON forum_audience_mentions
            FOR EACH ROW
            BEGIN
                SELECT RAISE(ABORT, 'forum relation projections are immutable');
            END"#,
        "DROP TRIGGER IF EXISTS forum_quotes_source_guard",
        r#"CREATE TRIGGER forum_quotes_source_guard
            BEFORE INSERT ON forum_quotes
            FOR EACH ROW
            WHEN NOT EXISTS (
                SELECT 1 FROM forum_relation_revisions revision
                WHERE revision.revision_id = NEW.source_revision_id
                  AND revision.tenant_id = NEW.tenant_id
                  AND revision.target_kind = NEW.source_kind
                  AND revision.target_id = NEW.source_id
                  AND revision.locale = NEW.source_locale
            )
            BEGIN
                SELECT RAISE(ABORT, 'forum relation child source mismatch');
            END"#,
        "DROP TRIGGER IF EXISTS forum_quotes_target_guard",
        r#"CREATE TRIGGER forum_quotes_target_guard
            BEFORE INSERT ON forum_quotes
            FOR EACH ROW
            WHEN NOT EXISTS (
                SELECT 1 FROM forum_relation_revisions revision
                WHERE revision.revision_id = NEW.quoted_revision_id
                  AND revision.tenant_id = NEW.tenant_id
                  AND revision.target_kind = NEW.quoted_kind
                  AND revision.target_id = NEW.quoted_id
            )
            BEGIN
                SELECT RAISE(ABORT, 'forum quote target mismatch');
            END"#,
        "DROP TRIGGER IF EXISTS forum_quotes_immutable_guard",
        r#"CREATE TRIGGER forum_quotes_immutable_guard
            BEFORE UPDATE ON forum_quotes
            FOR EACH ROW
            BEGIN
                SELECT RAISE(ABORT, 'forum relation projections are immutable');
            END"#,
        r#"INSERT INTO forum_relation_revisions (
                tenant_id, target_kind, target_id, locale, projection_fingerprint
            )
            SELECT
                translation.tenant_id,
                'topic',
                translation.topic_id,
                translation.locale,
                'legacy'
            FROM forum_topic_translations translation
            WHERE NOT EXISTS (
                SELECT 1 FROM forum_relation_revisions revision
                WHERE revision.tenant_id = translation.tenant_id
                  AND revision.target_kind = 'topic'
                  AND revision.target_id = translation.topic_id
                  AND revision.locale = translation.locale
            )"#,
        r#"INSERT INTO forum_relation_revisions (
                tenant_id, target_kind, target_id, locale, projection_fingerprint
            )
            SELECT
                body.tenant_id,
                'reply',
                body.reply_id,
                body.locale,
                'legacy'
            FROM forum_reply_bodies body
            WHERE NOT EXISTS (
                SELECT 1 FROM forum_relation_revisions revision
                WHERE revision.tenant_id = body.tenant_id
                  AND revision.target_kind = 'reply'
                  AND revision.target_id = body.reply_id
                  AND revision.locale = body.locale
            )"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS forum_quotes_immutable_guard",
        "DROP TRIGGER IF EXISTS forum_quotes_target_guard",
        "DROP TRIGGER IF EXISTS forum_quotes_source_guard",
        "DROP TRIGGER IF EXISTS forum_audience_mentions_immutable_guard",
        "DROP TRIGGER IF EXISTS forum_audience_mentions_source_guard",
        "DROP TRIGGER IF EXISTS forum_user_mentions_immutable_guard",
        "DROP TRIGGER IF EXISTS forum_user_mentions_source_guard",
        "DROP TRIGGER IF EXISTS forum_relation_revision_immutable_guard",
        "DROP TRIGGER IF EXISTS forum_relation_revision_source_guard",
        "DROP TABLE IF EXISTS forum_quotes",
        "DROP TABLE IF EXISTS forum_audience_mentions",
        "DROP TABLE IF EXISTS forum_user_mentions",
        "DROP TABLE IF EXISTS forum_relation_revisions",
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}
