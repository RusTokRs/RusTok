use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => up_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum category moderation audience migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum category moderation audience migration does not support {backend:?}"
            ))),
        }
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
CREATE TABLE IF NOT EXISTS forum_category_moderation_audience_policies (
    tenant_id UUID NOT NULL,
    category_id UUID NOT NULL,
    minimum_trust_level SMALLINT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT pk_forum_category_moderation_audience_policies
        PRIMARY KEY (tenant_id, category_id),
    CONSTRAINT fk_forum_category_moderation_audience_policy_category
        FOREIGN KEY (tenant_id, category_id)
        REFERENCES forum_categories (tenant_id, id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CONSTRAINT ck_forum_category_moderation_audience_minimum_trust
        CHECK (minimum_trust_level IS NULL OR minimum_trust_level BETWEEN 0 AND 100)
);

CREATE TABLE IF NOT EXISTS forum_category_moderation_audience_roles (
    tenant_id UUID NOT NULL,
    category_id UUID NOT NULL,
    role VARCHAR(32) NOT NULL,
    CONSTRAINT pk_forum_category_moderation_audience_roles
        PRIMARY KEY (tenant_id, category_id, role),
    CONSTRAINT fk_forum_category_moderation_audience_roles_policy
        FOREIGN KEY (tenant_id, category_id)
        REFERENCES forum_category_moderation_audience_policies (tenant_id, category_id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CONSTRAINT ck_forum_category_moderation_audience_role
        CHECK (role IN ('super_admin', 'admin', 'manager', 'customer'))
);

CREATE TABLE IF NOT EXISTS forum_category_moderation_audience_channels (
    tenant_id UUID NOT NULL,
    category_id UUID NOT NULL,
    channel_slug VARCHAR(128) NOT NULL,
    CONSTRAINT pk_forum_category_moderation_audience_channels
        PRIMARY KEY (tenant_id, category_id, channel_slug),
    CONSTRAINT fk_forum_category_moderation_audience_channels_policy
        FOREIGN KEY (tenant_id, category_id)
        REFERENCES forum_category_moderation_audience_policies (tenant_id, category_id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CONSTRAINT ck_forum_category_moderation_audience_channel_slug
        CHECK (
            length(channel_slug) BETWEEN 1 AND 128
            AND channel_slug = lower(channel_slug)
            AND channel_slug = btrim(channel_slug)
        )
);

CREATE TABLE IF NOT EXISTS forum_category_moderation_audience_groups (
    tenant_id UUID NOT NULL,
    category_id UUID NOT NULL,
    group_id UUID NOT NULL,
    CONSTRAINT pk_forum_category_moderation_audience_groups
        PRIMARY KEY (tenant_id, category_id, group_id),
    CONSTRAINT fk_forum_category_moderation_audience_groups_policy
        FOREIGN KEY (tenant_id, category_id)
        REFERENCES forum_category_moderation_audience_policies (tenant_id, category_id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CONSTRAINT ck_forum_category_moderation_audience_group_id
        CHECK (group_id <> '00000000-0000-0000-0000-000000000000'::uuid)
);

CREATE TABLE IF NOT EXISTS forum_category_moderation_audience_users (
    tenant_id UUID NOT NULL,
    category_id UUID NOT NULL,
    user_id UUID NOT NULL,
    effect VARCHAR(16) NOT NULL,
    CONSTRAINT pk_forum_category_moderation_audience_users
        PRIMARY KEY (tenant_id, category_id, user_id, effect),
    CONSTRAINT fk_forum_category_moderation_audience_users_policy
        FOREIGN KEY (tenant_id, category_id)
        REFERENCES forum_category_moderation_audience_policies (tenant_id, category_id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CONSTRAINT ck_forum_category_moderation_audience_user_id
        CHECK (user_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT ck_forum_category_moderation_audience_user_effect
        CHECK (effect IN ('allow', 'deny'))
);

CREATE OR REPLACE FUNCTION forum_reject_category_moderation_audience_update()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'forum category moderation audience rows are immutable';
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_validate_category_moderation_audience_channel_insert()
RETURNS trigger AS $$
BEGIN
    PERFORM pg_advisory_xact_lock(hashtextextended(NEW.tenant_id::text || ':' || NEW.category_id::text || ':moderation', 5));
    IF (
        SELECT count(*)
        FROM forum_category_moderation_audience_channels item
        WHERE item.tenant_id = NEW.tenant_id
          AND item.category_id = NEW.category_id
    ) >= 32 THEN
        RAISE EXCEPTION 'forum category moderation audience channels exceed bounded limit';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_validate_category_moderation_audience_group_insert()
RETURNS trigger AS $$
BEGIN
    PERFORM pg_advisory_xact_lock(hashtextextended(NEW.tenant_id::text || ':' || NEW.category_id::text || ':moderation', 5));
    IF (
        SELECT count(*)
        FROM forum_category_moderation_audience_groups item
        WHERE item.tenant_id = NEW.tenant_id
          AND item.category_id = NEW.category_id
    ) >= 32 THEN
        RAISE EXCEPTION 'forum category moderation audience groups exceed bounded limit';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_validate_category_moderation_audience_user_insert()
RETURNS trigger AS $$
BEGIN
    PERFORM pg_advisory_xact_lock(hashtextextended(NEW.tenant_id::text || ':' || NEW.category_id::text || ':moderation', 5));
    IF (
        SELECT count(*)
        FROM forum_category_moderation_audience_users item
        WHERE item.tenant_id = NEW.tenant_id
          AND item.category_id = NEW.category_id
          AND item.effect = NEW.effect
    ) >= 100 THEN
        RAISE EXCEPTION 'forum category moderation audience users exceed bounded limit';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_category_moderation_audience_policy_update ON forum_category_moderation_audience_policies;
CREATE TRIGGER forum_category_moderation_audience_policy_update
BEFORE UPDATE ON forum_category_moderation_audience_policies
FOR EACH ROW EXECUTE FUNCTION forum_reject_category_moderation_audience_update();

DROP TRIGGER IF EXISTS forum_category_moderation_audience_roles_update ON forum_category_moderation_audience_roles;
CREATE TRIGGER forum_category_moderation_audience_roles_update
BEFORE UPDATE ON forum_category_moderation_audience_roles
FOR EACH ROW EXECUTE FUNCTION forum_reject_category_moderation_audience_update();

DROP TRIGGER IF EXISTS forum_category_moderation_audience_channels_insert ON forum_category_moderation_audience_channels;
CREATE TRIGGER forum_category_moderation_audience_channels_insert
BEFORE INSERT ON forum_category_moderation_audience_channels
FOR EACH ROW EXECUTE FUNCTION forum_validate_category_moderation_audience_channel_insert();
DROP TRIGGER IF EXISTS forum_category_moderation_audience_channels_update ON forum_category_moderation_audience_channels;
CREATE TRIGGER forum_category_moderation_audience_channels_update
BEFORE UPDATE ON forum_category_moderation_audience_channels
FOR EACH ROW EXECUTE FUNCTION forum_reject_category_moderation_audience_update();

DROP TRIGGER IF EXISTS forum_category_moderation_audience_groups_insert ON forum_category_moderation_audience_groups;
CREATE TRIGGER forum_category_moderation_audience_groups_insert
BEFORE INSERT ON forum_category_moderation_audience_groups
FOR EACH ROW EXECUTE FUNCTION forum_validate_category_moderation_audience_group_insert();
DROP TRIGGER IF EXISTS forum_category_moderation_audience_groups_update ON forum_category_moderation_audience_groups;
CREATE TRIGGER forum_category_moderation_audience_groups_update
BEFORE UPDATE ON forum_category_moderation_audience_groups
FOR EACH ROW EXECUTE FUNCTION forum_reject_category_moderation_audience_update();

DROP TRIGGER IF EXISTS forum_category_moderation_audience_users_insert ON forum_category_moderation_audience_users;
CREATE TRIGGER forum_category_moderation_audience_users_insert
BEFORE INSERT ON forum_category_moderation_audience_users
FOR EACH ROW EXECUTE FUNCTION forum_validate_category_moderation_audience_user_insert();
DROP TRIGGER IF EXISTS forum_category_moderation_audience_users_update ON forum_category_moderation_audience_users;
CREATE TRIGGER forum_category_moderation_audience_users_update
BEFORE UPDATE ON forum_category_moderation_audience_users
FOR EACH ROW EXECUTE FUNCTION forum_reject_category_moderation_audience_update();
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
DROP TABLE IF EXISTS forum_category_moderation_audience_users;
DROP TABLE IF EXISTS forum_category_moderation_audience_groups;
DROP TABLE IF EXISTS forum_category_moderation_audience_channels;
DROP TABLE IF EXISTS forum_category_moderation_audience_roles;
DROP TABLE IF EXISTS forum_category_moderation_audience_policies;
DROP FUNCTION IF EXISTS forum_validate_category_moderation_audience_user_insert();
DROP FUNCTION IF EXISTS forum_validate_category_moderation_audience_group_insert();
DROP FUNCTION IF EXISTS forum_validate_category_moderation_audience_channel_insert();
DROP FUNCTION IF EXISTS forum_reject_category_moderation_audience_update();
"#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
CREATE UNIQUE INDEX IF NOT EXISTS uq_forum_categories_tenant_id
    ON forum_categories (tenant_id, id);

CREATE TABLE IF NOT EXISTS forum_category_moderation_audience_policies (
    tenant_id TEXT NOT NULL,
    category_id TEXT NOT NULL,
    minimum_trust_level INTEGER NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (tenant_id, category_id),
    FOREIGN KEY (tenant_id, category_id)
        REFERENCES forum_categories (tenant_id, id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CHECK (minimum_trust_level IS NULL OR minimum_trust_level BETWEEN 0 AND 100)
);

CREATE TABLE IF NOT EXISTS forum_category_moderation_audience_roles (
    tenant_id TEXT NOT NULL,
    category_id TEXT NOT NULL,
    role TEXT NOT NULL,
    PRIMARY KEY (tenant_id, category_id, role),
    FOREIGN KEY (tenant_id, category_id)
        REFERENCES forum_category_moderation_audience_policies (tenant_id, category_id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CHECK (role IN ('super_admin', 'admin', 'manager', 'customer'))
);

CREATE TABLE IF NOT EXISTS forum_category_moderation_audience_channels (
    tenant_id TEXT NOT NULL,
    category_id TEXT NOT NULL,
    channel_slug TEXT NOT NULL,
    PRIMARY KEY (tenant_id, category_id, channel_slug),
    FOREIGN KEY (tenant_id, category_id)
        REFERENCES forum_category_moderation_audience_policies (tenant_id, category_id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CHECK (
        length(channel_slug) BETWEEN 1 AND 128
        AND channel_slug = lower(channel_slug)
        AND channel_slug = trim(channel_slug)
    )
);

CREATE TABLE IF NOT EXISTS forum_category_moderation_audience_groups (
    tenant_id TEXT NOT NULL,
    category_id TEXT NOT NULL,
    group_id TEXT NOT NULL,
    PRIMARY KEY (tenant_id, category_id, group_id),
    FOREIGN KEY (tenant_id, category_id)
        REFERENCES forum_category_moderation_audience_policies (tenant_id, category_id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CHECK (group_id <> '00000000-0000-0000-0000-000000000000')
);

CREATE TABLE IF NOT EXISTS forum_category_moderation_audience_users (
    tenant_id TEXT NOT NULL,
    category_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    effect TEXT NOT NULL,
    PRIMARY KEY (tenant_id, category_id, user_id, effect),
    FOREIGN KEY (tenant_id, category_id)
        REFERENCES forum_category_moderation_audience_policies (tenant_id, category_id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CHECK (user_id <> '00000000-0000-0000-0000-000000000000'),
    CHECK (effect IN ('allow', 'deny'))
);

CREATE TRIGGER IF NOT EXISTS forum_category_moderation_audience_policy_update
BEFORE UPDATE ON forum_category_moderation_audience_policies
BEGIN
    SELECT RAISE(ABORT, 'forum category moderation audience rows are immutable');
END;

CREATE TRIGGER IF NOT EXISTS forum_category_moderation_audience_roles_update
BEFORE UPDATE ON forum_category_moderation_audience_roles
BEGIN
    SELECT RAISE(ABORT, 'forum category moderation audience rows are immutable');
END;

CREATE TRIGGER IF NOT EXISTS forum_category_moderation_audience_channels_insert
BEFORE INSERT ON forum_category_moderation_audience_channels
WHEN (
    SELECT count(*)
    FROM forum_category_moderation_audience_channels item
    WHERE item.tenant_id = NEW.tenant_id
      AND item.category_id = NEW.category_id
) >= 32
BEGIN
    SELECT RAISE(ABORT, 'forum category moderation audience channels exceed bounded limit');
END;
CREATE TRIGGER IF NOT EXISTS forum_category_moderation_audience_channels_update
BEFORE UPDATE ON forum_category_moderation_audience_channels
BEGIN
    SELECT RAISE(ABORT, 'forum category moderation audience rows are immutable');
END;

CREATE TRIGGER IF NOT EXISTS forum_category_moderation_audience_groups_insert
BEFORE INSERT ON forum_category_moderation_audience_groups
WHEN (
    SELECT count(*)
    FROM forum_category_moderation_audience_groups item
    WHERE item.tenant_id = NEW.tenant_id
      AND item.category_id = NEW.category_id
) >= 32
BEGIN
    SELECT RAISE(ABORT, 'forum category moderation audience groups exceed bounded limit');
END;
CREATE TRIGGER IF NOT EXISTS forum_category_moderation_audience_groups_update
BEFORE UPDATE ON forum_category_moderation_audience_groups
BEGIN
    SELECT RAISE(ABORT, 'forum category moderation audience rows are immutable');
END;

CREATE TRIGGER IF NOT EXISTS forum_category_moderation_audience_users_insert
BEFORE INSERT ON forum_category_moderation_audience_users
WHEN (
    SELECT count(*)
    FROM forum_category_moderation_audience_users item
    WHERE item.tenant_id = NEW.tenant_id
      AND item.category_id = NEW.category_id
      AND item.effect = NEW.effect
) >= 100
BEGIN
    SELECT RAISE(ABORT, 'forum category moderation audience users exceed bounded limit');
END;
CREATE TRIGGER IF NOT EXISTS forum_category_moderation_audience_users_update
BEFORE UPDATE ON forum_category_moderation_audience_users
BEGIN
    SELECT RAISE(ABORT, 'forum category moderation audience rows are immutable');
END;
"#,
        )
        .await?;
    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
DROP TABLE IF EXISTS forum_category_moderation_audience_users;
DROP TABLE IF EXISTS forum_category_moderation_audience_groups;
DROP TABLE IF EXISTS forum_category_moderation_audience_channels;
DROP TABLE IF EXISTS forum_category_moderation_audience_roles;
DROP TABLE IF EXISTS forum_category_moderation_audience_policies;
"#,
        )
        .await?;
    Ok(())
}
