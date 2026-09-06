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
                "rustok-forum user trust state migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => execute(manager, POSTGRES_DOWN).await,
            DatabaseBackend::Sqlite => execute(manager, SQLITE_DOWN).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum user trust state migration does not support {backend:?}"
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
CREATE UNIQUE INDEX IF NOT EXISTS ux_users_tenant_identity
    ON users (tenant_id, id);

CREATE TABLE IF NOT EXISTS forum_user_trust_states (
    tenant_id UUID NOT NULL,
    user_id UUID NOT NULL,
    trust_level SMALLINT NOT NULL,
    revision BIGINT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT pk_forum_user_trust_states PRIMARY KEY (tenant_id, user_id),
    CONSTRAINT fk_forum_user_trust_state_user
        FOREIGN KEY (tenant_id, user_id)
        REFERENCES users (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT ck_forum_user_trust_state_user_id
        CHECK (user_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT ck_forum_user_trust_state_level
        CHECK (trust_level BETWEEN 0 AND 100),
    CONSTRAINT ck_forum_user_trust_state_revision
        CHECK (revision > 0)
);

CREATE TABLE IF NOT EXISTS forum_user_trust_revisions (
    tenant_id UUID NOT NULL,
    user_id UUID NOT NULL,
    revision BIGINT NOT NULL,
    previous_trust_level SMALLINT NULL,
    trust_level SMALLINT NOT NULL,
    change_kind VARCHAR(32) NOT NULL,
    reason_code VARCHAR(64) NOT NULL,
    reason_summary VARCHAR(256) NOT NULL,
    changed_by_user_id UUID NULL,
    idempotency_key VARCHAR(128) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT pk_forum_user_trust_revisions
        PRIMARY KEY (tenant_id, user_id, revision),
    CONSTRAINT uq_forum_user_trust_revision_idempotency
        UNIQUE (tenant_id, idempotency_key),
    CONSTRAINT fk_forum_user_trust_revision_user
        FOREIGN KEY (tenant_id, user_id)
        REFERENCES users (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CONSTRAINT ck_forum_user_trust_revision_user_id
        CHECK (user_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT ck_forum_user_trust_revision_actor_id
        CHECK (
            changed_by_user_id IS NULL
            OR changed_by_user_id <> '00000000-0000-0000-0000-000000000000'::uuid
        ),
    CONSTRAINT ck_forum_user_trust_revision_number
        CHECK (revision > 0),
    CONSTRAINT ck_forum_user_trust_revision_previous_level
        CHECK (previous_trust_level IS NULL OR previous_trust_level BETWEEN 0 AND 100),
    CONSTRAINT ck_forum_user_trust_revision_level
        CHECK (trust_level BETWEEN 0 AND 100),
    CONSTRAINT ck_forum_user_trust_revision_kind
        CHECK (change_kind IN ('manual_override', 'policy_evaluation', 'reconciliation', 'migration')),
    CONSTRAINT ck_forum_user_trust_revision_reason_code
        CHECK (
            length(reason_code) BETWEEN 1 AND 64
            AND reason_code = lower(reason_code)
            AND reason_code = btrim(reason_code)
            AND reason_code ~ '^[a-z0-9][a-z0-9_.-]{0,63}$'
        ),
    CONSTRAINT ck_forum_user_trust_revision_reason_summary
        CHECK (
            length(reason_summary) BETWEEN 1 AND 256
            AND reason_summary = btrim(reason_summary)
            AND position(E'\n' in reason_summary) = 0
            AND position(E'\r' in reason_summary) = 0
        ),
    CONSTRAINT ck_forum_user_trust_revision_idempotency
        CHECK (
            length(idempotency_key) BETWEEN 1 AND 128
            AND idempotency_key = btrim(idempotency_key)
            AND position(E'\n' in idempotency_key) = 0
            AND position(E'\r' in idempotency_key) = 0
        )
);

CREATE INDEX IF NOT EXISTS idx_forum_user_trust_revisions_history
    ON forum_user_trust_revisions (tenant_id, user_id, revision DESC);

CREATE OR REPLACE FUNCTION forum_reject_user_trust_revision_mutation()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'forum user trust revisions are append-only';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_reject_user_trust_state_delete()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'forum user trust state cannot be deleted directly';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_validate_user_trust_revision_insert()
RETURNS trigger AS $$
DECLARE
    current_revision BIGINT;
    current_level SMALLINT;
BEGIN
    PERFORM pg_advisory_xact_lock(
        hashtextextended(NEW.tenant_id::text || ':' || NEW.user_id::text || ':trust', 26)
    );

    SELECT state.revision, state.trust_level
      INTO current_revision, current_level
      FROM forum_user_trust_states state
     WHERE state.tenant_id = NEW.tenant_id
       AND state.user_id = NEW.user_id;

    IF current_revision IS NULL THEN
        IF NEW.revision <> 1 OR NEW.previous_trust_level IS NOT NULL THEN
            RAISE EXCEPTION 'forum user trust first revision must start at one without a previous level';
        END IF;
        IF EXISTS (
            SELECT 1
              FROM forum_user_trust_revisions existing
             WHERE existing.tenant_id = NEW.tenant_id
               AND existing.user_id = NEW.user_id
        ) THEN
            RAISE EXCEPTION 'forum user trust revision sequence is inconsistent';
        END IF;
    ELSE
        IF NEW.revision <> current_revision + 1 THEN
            RAISE EXCEPTION 'forum user trust revision must advance exactly once';
        END IF;
        IF NEW.previous_trust_level IS DISTINCT FROM current_level THEN
            RAISE EXCEPTION 'forum user trust previous level does not match current state';
        END IF;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_validate_user_trust_state_insert()
RETURNS trigger AS $$
BEGIN
    PERFORM pg_advisory_xact_lock(
        hashtextextended(NEW.tenant_id::text || ':' || NEW.user_id::text || ':trust', 26)
    );
    IF NEW.revision <> 1 OR NOT EXISTS (
        SELECT 1
          FROM forum_user_trust_revisions revision
         WHERE revision.tenant_id = NEW.tenant_id
           AND revision.user_id = NEW.user_id
           AND revision.revision = NEW.revision
           AND revision.trust_level = NEW.trust_level
    ) THEN
        RAISE EXCEPTION 'forum user trust state must match its first immutable revision';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_validate_user_trust_state_update()
RETURNS trigger AS $$
BEGIN
    PERFORM pg_advisory_xact_lock(
        hashtextextended(NEW.tenant_id::text || ':' || NEW.user_id::text || ':trust', 26)
    );
    IF NEW.tenant_id <> OLD.tenant_id OR NEW.user_id <> OLD.user_id THEN
        RAISE EXCEPTION 'forum user trust state identity is immutable';
    END IF;
    IF NEW.revision <> OLD.revision + 1 OR NOT EXISTS (
        SELECT 1
          FROM forum_user_trust_revisions revision
         WHERE revision.tenant_id = NEW.tenant_id
           AND revision.user_id = NEW.user_id
           AND revision.revision = NEW.revision
           AND revision.previous_trust_level = OLD.trust_level
           AND revision.trust_level = NEW.trust_level
    ) THEN
        RAISE EXCEPTION 'forum user trust state update must match the next immutable revision';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_apply_user_trust_revision()
RETURNS trigger AS $$
BEGIN
    INSERT INTO forum_user_trust_states (
        tenant_id, user_id, trust_level, revision, updated_at
    ) VALUES (
        NEW.tenant_id, NEW.user_id, NEW.trust_level, NEW.revision, NEW.created_at
    )
    ON CONFLICT (tenant_id, user_id) DO UPDATE
       SET trust_level = EXCLUDED.trust_level,
           revision = EXCLUDED.revision,
           updated_at = EXCLUDED.updated_at;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_user_trust_revision_insert ON forum_user_trust_revisions;
CREATE TRIGGER forum_user_trust_revision_insert
BEFORE INSERT ON forum_user_trust_revisions
FOR EACH ROW EXECUTE FUNCTION forum_validate_user_trust_revision_insert();

DROP TRIGGER IF EXISTS forum_user_trust_revision_apply ON forum_user_trust_revisions;
CREATE TRIGGER forum_user_trust_revision_apply
AFTER INSERT ON forum_user_trust_revisions
FOR EACH ROW EXECUTE FUNCTION forum_apply_user_trust_revision();

DROP TRIGGER IF EXISTS forum_user_trust_revision_update ON forum_user_trust_revisions;
CREATE TRIGGER forum_user_trust_revision_update
BEFORE UPDATE ON forum_user_trust_revisions
FOR EACH ROW EXECUTE FUNCTION forum_reject_user_trust_revision_mutation();

DROP TRIGGER IF EXISTS forum_user_trust_revision_delete ON forum_user_trust_revisions;
CREATE TRIGGER forum_user_trust_revision_delete
BEFORE DELETE ON forum_user_trust_revisions
FOR EACH ROW EXECUTE FUNCTION forum_reject_user_trust_revision_mutation();

DROP TRIGGER IF EXISTS forum_user_trust_state_insert ON forum_user_trust_states;
CREATE TRIGGER forum_user_trust_state_insert
BEFORE INSERT ON forum_user_trust_states
FOR EACH ROW EXECUTE FUNCTION forum_validate_user_trust_state_insert();

DROP TRIGGER IF EXISTS forum_user_trust_state_update ON forum_user_trust_states;
CREATE TRIGGER forum_user_trust_state_update
BEFORE UPDATE ON forum_user_trust_states
FOR EACH ROW EXECUTE FUNCTION forum_validate_user_trust_state_update();

DROP TRIGGER IF EXISTS forum_user_trust_state_delete ON forum_user_trust_states;
CREATE TRIGGER forum_user_trust_state_delete
BEFORE DELETE ON forum_user_trust_states
FOR EACH ROW EXECUTE FUNCTION forum_reject_user_trust_state_delete();
"#;

const POSTGRES_DOWN: &str = r#"
DROP TABLE IF EXISTS forum_user_trust_revisions;
DROP TABLE IF EXISTS forum_user_trust_states;
DROP FUNCTION IF EXISTS forum_apply_user_trust_revision();
DROP FUNCTION IF EXISTS forum_validate_user_trust_state_update();
DROP FUNCTION IF EXISTS forum_validate_user_trust_state_insert();
DROP FUNCTION IF EXISTS forum_validate_user_trust_revision_insert();
DROP FUNCTION IF EXISTS forum_reject_user_trust_state_delete();
DROP FUNCTION IF EXISTS forum_reject_user_trust_revision_mutation();
"#;

const SQLITE_UP: &str = r#"
CREATE UNIQUE INDEX IF NOT EXISTS ux_users_tenant_identity
    ON users (tenant_id, id);

CREATE TABLE IF NOT EXISTS forum_user_trust_states (
    tenant_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    trust_level INTEGER NOT NULL,
    revision INTEGER NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (tenant_id, user_id),
    FOREIGN KEY (tenant_id, user_id)
        REFERENCES users (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CHECK (user_id <> '00000000-0000-0000-0000-000000000000'),
    CHECK (trust_level BETWEEN 0 AND 100),
    CHECK (revision > 0)
);

CREATE TABLE IF NOT EXISTS forum_user_trust_revisions (
    tenant_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    revision INTEGER NOT NULL,
    previous_trust_level INTEGER NULL,
    trust_level INTEGER NOT NULL,
    change_kind TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    reason_summary TEXT NOT NULL,
    changed_by_user_id TEXT NULL,
    idempotency_key TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (tenant_id, user_id, revision),
    UNIQUE (tenant_id, idempotency_key),
    FOREIGN KEY (tenant_id, user_id)
        REFERENCES users (tenant_id, id)
        ON UPDATE CASCADE ON DELETE RESTRICT,
    CHECK (user_id <> '00000000-0000-0000-0000-000000000000'),
    CHECK (
        changed_by_user_id IS NULL
        OR changed_by_user_id <> '00000000-0000-0000-0000-000000000000'
    ),
    CHECK (revision > 0),
    CHECK (previous_trust_level IS NULL OR previous_trust_level BETWEEN 0 AND 100),
    CHECK (trust_level BETWEEN 0 AND 100),
    CHECK (change_kind IN ('manual_override', 'policy_evaluation', 'reconciliation', 'migration')),
    CHECK (
        length(reason_code) BETWEEN 1 AND 64
        AND reason_code = lower(reason_code)
        AND reason_code = trim(reason_code)
        AND reason_code NOT GLOB '*[^a-z0-9_.-]*'
        AND substr(reason_code, 1, 1) GLOB '[a-z0-9]'
    ),
    CHECK (
        length(reason_summary) BETWEEN 1 AND 256
        AND reason_summary = trim(reason_summary)
        AND instr(reason_summary, char(10)) = 0
        AND instr(reason_summary, char(13)) = 0
    ),
    CHECK (
        length(idempotency_key) BETWEEN 1 AND 128
        AND idempotency_key = trim(idempotency_key)
        AND instr(idempotency_key, char(10)) = 0
        AND instr(idempotency_key, char(13)) = 0
    )
);

CREATE INDEX IF NOT EXISTS idx_forum_user_trust_revisions_history
    ON forum_user_trust_revisions (tenant_id, user_id, revision DESC);

CREATE TRIGGER IF NOT EXISTS forum_user_trust_revision_insert_first
BEFORE INSERT ON forum_user_trust_revisions
WHEN NOT EXISTS (
    SELECT 1 FROM forum_user_trust_states state
     WHERE state.tenant_id = NEW.tenant_id
       AND state.user_id = NEW.user_id
)
BEGIN
    SELECT CASE WHEN NEW.revision <> 1 OR NEW.previous_trust_level IS NOT NULL
        THEN RAISE(ABORT, 'forum user trust first revision must start at one without a previous level') END;
    SELECT CASE WHEN EXISTS (
        SELECT 1 FROM forum_user_trust_revisions existing
         WHERE existing.tenant_id = NEW.tenant_id
           AND existing.user_id = NEW.user_id
    ) THEN RAISE(ABORT, 'forum user trust revision sequence is inconsistent') END;
END;

CREATE TRIGGER IF NOT EXISTS forum_user_trust_revision_insert_next
BEFORE INSERT ON forum_user_trust_revisions
WHEN EXISTS (
    SELECT 1 FROM forum_user_trust_states state
     WHERE state.tenant_id = NEW.tenant_id
       AND state.user_id = NEW.user_id
)
BEGIN
    SELECT CASE WHEN NEW.revision <> (
        SELECT state.revision + 1 FROM forum_user_trust_states state
         WHERE state.tenant_id = NEW.tenant_id
           AND state.user_id = NEW.user_id
    ) THEN RAISE(ABORT, 'forum user trust revision must advance exactly once') END;
    SELECT CASE WHEN NEW.previous_trust_level IS NOT (
        SELECT state.trust_level FROM forum_user_trust_states state
         WHERE state.tenant_id = NEW.tenant_id
           AND state.user_id = NEW.user_id
    ) THEN RAISE(ABORT, 'forum user trust previous level does not match current state') END;
END;

CREATE TRIGGER IF NOT EXISTS forum_user_trust_revision_apply
AFTER INSERT ON forum_user_trust_revisions
BEGIN
    INSERT INTO forum_user_trust_states (
        tenant_id, user_id, trust_level, revision, updated_at
    ) VALUES (
        NEW.tenant_id, NEW.user_id, NEW.trust_level, NEW.revision, NEW.created_at
    )
    ON CONFLICT (tenant_id, user_id) DO UPDATE SET
        trust_level = excluded.trust_level,
        revision = excluded.revision,
        updated_at = excluded.updated_at;
END;

CREATE TRIGGER IF NOT EXISTS forum_user_trust_revision_update
BEFORE UPDATE ON forum_user_trust_revisions
BEGIN
    SELECT RAISE(ABORT, 'forum user trust revisions are append-only');
END;

CREATE TRIGGER IF NOT EXISTS forum_user_trust_revision_delete
BEFORE DELETE ON forum_user_trust_revisions
BEGIN
    SELECT RAISE(ABORT, 'forum user trust revisions are append-only');
END;

CREATE TRIGGER IF NOT EXISTS forum_user_trust_state_insert
BEFORE INSERT ON forum_user_trust_states
WHEN NOT EXISTS (
    SELECT 1 FROM forum_user_trust_states state
     WHERE state.tenant_id = NEW.tenant_id
       AND state.user_id = NEW.user_id
)
BEGIN
    SELECT CASE WHEN NEW.revision <> 1 OR NOT EXISTS (
        SELECT 1 FROM forum_user_trust_revisions revision
         WHERE revision.tenant_id = NEW.tenant_id
           AND revision.user_id = NEW.user_id
           AND revision.revision = NEW.revision
           AND revision.trust_level = NEW.trust_level
    ) THEN RAISE(ABORT, 'forum user trust state must match its first immutable revision') END;
END;

CREATE TRIGGER IF NOT EXISTS forum_user_trust_state_update
BEFORE UPDATE ON forum_user_trust_states
BEGIN
    SELECT CASE WHEN NEW.tenant_id <> OLD.tenant_id OR NEW.user_id <> OLD.user_id
        THEN RAISE(ABORT, 'forum user trust state identity is immutable') END;
    SELECT CASE WHEN NEW.revision <> OLD.revision + 1 OR NOT EXISTS (
        SELECT 1 FROM forum_user_trust_revisions revision
         WHERE revision.tenant_id = NEW.tenant_id
           AND revision.user_id = NEW.user_id
           AND revision.revision = NEW.revision
           AND revision.previous_trust_level = OLD.trust_level
           AND revision.trust_level = NEW.trust_level
    ) THEN RAISE(ABORT, 'forum user trust state update must match the next immutable revision') END;
END;

CREATE TRIGGER IF NOT EXISTS forum_user_trust_state_delete
BEFORE DELETE ON forum_user_trust_states
BEGIN
    SELECT RAISE(ABORT, 'forum user trust state cannot be deleted directly');
END;
"#;

const SQLITE_DOWN: &str = r#"
DROP TABLE IF EXISTS forum_user_trust_revisions;
DROP TABLE IF EXISTS forum_user_trust_states;
"#;
