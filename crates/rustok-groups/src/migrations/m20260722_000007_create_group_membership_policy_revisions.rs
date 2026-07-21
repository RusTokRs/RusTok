use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DbBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DbBackend::Postgres => postgres_up(manager).await,
            DbBackend::Sqlite => sqlite_up(manager).await,
            DbBackend::MySql => Err(DbErr::Migration(
                "Groups membership policy revision history supports PostgreSQL and SQLite only"
                    .to_string(),
            )),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DbBackend::Postgres => postgres_down(manager).await,
            DbBackend::Sqlite => sqlite_down(manager).await,
            DbBackend::MySql => Ok(()),
        }
    }
}

async fn postgres_up(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
CREATE TABLE IF NOT EXISTS group_membership_policy_revisions (
    tenant_id UUID NOT NULL,
    group_id UUID NOT NULL,
    policy_id UUID NOT NULL,
    revision BIGINT NOT NULL,
    locale VARCHAR(32) NOT NULL,
    enabled BOOLEAN NOT NULL,
    questions JSONB NOT NULL,
    rules JSONB NOT NULL,
    created_by_user_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tenant_id, policy_id, revision, locale),
    CONSTRAINT fk_group_membership_policy_revisions_tenant_group
        FOREIGN KEY (tenant_id, group_id)
        REFERENCES groups (tenant_id, id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CONSTRAINT fk_group_membership_policy_revisions_tenant_policy
        FOREIGN KEY (tenant_id, policy_id)
        REFERENCES group_membership_policies (tenant_id, id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CONSTRAINT ck_group_membership_policy_revisions_revision CHECK (revision >= 1),
    CONSTRAINT ck_group_membership_policy_revisions_locale CHECK (
        locale = lower(locale)
        AND char_length(locale) BETWEEN 2 AND 32
    )
);

CREATE INDEX IF NOT EXISTS idx_group_membership_policy_revisions_tenant_group_revision
    ON group_membership_policy_revisions (tenant_id, group_id, revision DESC, locale);

INSERT INTO group_membership_policy_revisions (
    tenant_id,
    group_id,
    policy_id,
    revision,
    locale,
    enabled,
    questions,
    rules,
    created_by_user_id,
    created_at
)
SELECT
    policy.tenant_id,
    policy.group_id,
    policy.id,
    policy.revision,
    translation.locale,
    policy.enabled,
    translation.questions,
    translation.rules,
    policy.updated_by_user_id,
    policy.updated_at
FROM group_membership_policies policy
JOIN group_membership_policy_translations translation
  ON translation.tenant_id = policy.tenant_id
 AND translation.policy_id = policy.id
ON CONFLICT DO NOTHING;

CREATE OR REPLACE FUNCTION groups_capture_membership_policy_revision()
RETURNS trigger AS $$
BEGIN
    INSERT INTO group_membership_policy_revisions (
        tenant_id,
        group_id,
        policy_id,
        revision,
        locale,
        enabled,
        questions,
        rules,
        created_by_user_id,
        created_at
    )
    SELECT
        policy.tenant_id,
        policy.group_id,
        policy.id,
        policy.revision,
        NEW.locale,
        policy.enabled,
        NEW.questions,
        NEW.rules,
        policy.updated_by_user_id,
        policy.updated_at
    FROM group_membership_policies policy
    WHERE policy.tenant_id = NEW.tenant_id
      AND policy.id = NEW.policy_id
    ON CONFLICT DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS groups_membership_policy_revision_capture_insert
    ON group_membership_policy_translations;
CREATE TRIGGER groups_membership_policy_revision_capture_insert
AFTER INSERT ON group_membership_policy_translations
FOR EACH ROW EXECUTE FUNCTION groups_capture_membership_policy_revision();

DROP TRIGGER IF EXISTS groups_membership_policy_revision_capture_update
    ON group_membership_policy_translations;
CREATE TRIGGER groups_membership_policy_revision_capture_update
AFTER UPDATE OF questions, rules ON group_membership_policy_translations
FOR EACH ROW EXECUTE FUNCTION groups_capture_membership_policy_revision();

CREATE OR REPLACE FUNCTION groups_reject_membership_policy_revision_mutation()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'group membership policy revisions are append-only';
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS group_membership_policy_revisions_immutable_update
    ON group_membership_policy_revisions;
CREATE TRIGGER group_membership_policy_revisions_immutable_update
BEFORE UPDATE ON group_membership_policy_revisions
FOR EACH ROW EXECUTE FUNCTION groups_reject_membership_policy_revision_mutation();

DROP TRIGGER IF EXISTS group_membership_policy_revisions_immutable_delete
    ON group_membership_policy_revisions;
CREATE TRIGGER group_membership_policy_revisions_immutable_delete
BEFORE DELETE ON group_membership_policy_revisions
FOR EACH ROW EXECUTE FUNCTION groups_reject_membership_policy_revision_mutation();
"#,
        )
        .await?;
    Ok(())
}

async fn sqlite_up(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        r#"CREATE TABLE IF NOT EXISTS group_membership_policy_revisions (
    tenant_id TEXT NOT NULL,
    group_id TEXT NOT NULL,
    policy_id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK (revision >= 1),
    locale TEXT NOT NULL CHECK (
        locale = lower(locale)
        AND length(locale) BETWEEN 2 AND 32
    ),
    enabled INTEGER NOT NULL,
    questions TEXT NOT NULL,
    rules TEXT NOT NULL,
    created_by_user_id TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tenant_id, policy_id, revision, locale),
    FOREIGN KEY (tenant_id, group_id)
        REFERENCES groups (tenant_id, id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, policy_id)
        REFERENCES group_membership_policies (tenant_id, id)
        ON UPDATE CASCADE ON DELETE CASCADE
)"#,
        r#"CREATE INDEX IF NOT EXISTS idx_group_membership_policy_revisions_tenant_group_revision
    ON group_membership_policy_revisions (tenant_id, group_id, revision DESC, locale)"#,
        r#"INSERT OR IGNORE INTO group_membership_policy_revisions (
    tenant_id,
    group_id,
    policy_id,
    revision,
    locale,
    enabled,
    questions,
    rules,
    created_by_user_id,
    created_at
)
SELECT
    policy.tenant_id,
    policy.group_id,
    policy.id,
    policy.revision,
    translation.locale,
    policy.enabled,
    translation.questions,
    translation.rules,
    policy.updated_by_user_id,
    policy.updated_at
FROM group_membership_policies policy
JOIN group_membership_policy_translations translation
  ON translation.tenant_id = policy.tenant_id
 AND translation.policy_id = policy.id"#,
        "DROP TRIGGER IF EXISTS groups_membership_policy_revision_capture_insert",
        "DROP TRIGGER IF EXISTS groups_membership_policy_revision_capture_update",
        r#"CREATE TRIGGER groups_membership_policy_revision_capture_insert
AFTER INSERT ON group_membership_policy_translations
FOR EACH ROW
BEGIN
    INSERT OR IGNORE INTO group_membership_policy_revisions (
        tenant_id, group_id, policy_id, revision, locale, enabled,
        questions, rules, created_by_user_id, created_at
    )
    SELECT
        policy.tenant_id, policy.group_id, policy.id, policy.revision, NEW.locale,
        policy.enabled, NEW.questions, NEW.rules, policy.updated_by_user_id,
        policy.updated_at
    FROM group_membership_policies policy
    WHERE policy.tenant_id = NEW.tenant_id
      AND policy.id = NEW.policy_id;
END"#,
        r#"CREATE TRIGGER groups_membership_policy_revision_capture_update
AFTER UPDATE OF questions, rules ON group_membership_policy_translations
FOR EACH ROW
BEGIN
    INSERT OR IGNORE INTO group_membership_policy_revisions (
        tenant_id, group_id, policy_id, revision, locale, enabled,
        questions, rules, created_by_user_id, created_at
    )
    SELECT
        policy.tenant_id, policy.group_id, policy.id, policy.revision, NEW.locale,
        policy.enabled, NEW.questions, NEW.rules, policy.updated_by_user_id,
        policy.updated_at
    FROM group_membership_policies policy
    WHERE policy.tenant_id = NEW.tenant_id
      AND policy.id = NEW.policy_id;
END"#,
        "DROP TRIGGER IF EXISTS group_membership_policy_revisions_immutable_update",
        "DROP TRIGGER IF EXISTS group_membership_policy_revisions_immutable_delete",
        r#"CREATE TRIGGER group_membership_policy_revisions_immutable_update
BEFORE UPDATE ON group_membership_policy_revisions
FOR EACH ROW
BEGIN
    SELECT RAISE(ABORT, 'group membership policy revisions are append-only');
END"#,
        r#"CREATE TRIGGER group_membership_policy_revisions_immutable_delete
BEFORE DELETE ON group_membership_policy_revisions
FOR EACH ROW
BEGIN
    SELECT RAISE(ABORT, 'group membership policy revisions are append-only');
END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}

async fn postgres_down(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
DROP TRIGGER IF EXISTS groups_membership_policy_revision_capture_insert
    ON group_membership_policy_translations;
DROP TRIGGER IF EXISTS groups_membership_policy_revision_capture_update
    ON group_membership_policy_translations;
DROP FUNCTION IF EXISTS groups_capture_membership_policy_revision();
DROP TRIGGER IF EXISTS group_membership_policy_revisions_immutable_update
    ON group_membership_policy_revisions;
DROP TRIGGER IF EXISTS group_membership_policy_revisions_immutable_delete
    ON group_membership_policy_revisions;
DROP FUNCTION IF EXISTS groups_reject_membership_policy_revision_mutation();
DROP TABLE IF EXISTS group_membership_policy_revisions;
"#,
        )
        .await?;
    Ok(())
}

async fn sqlite_down(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS groups_membership_policy_revision_capture_insert",
        "DROP TRIGGER IF EXISTS groups_membership_policy_revision_capture_update",
        "DROP TRIGGER IF EXISTS group_membership_policy_revisions_immutable_update",
        "DROP TRIGGER IF EXISTS group_membership_policy_revisions_immutable_delete",
        "DROP TABLE IF EXISTS group_membership_policy_revisions",
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}
