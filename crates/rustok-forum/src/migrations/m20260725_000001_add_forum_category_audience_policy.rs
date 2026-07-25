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
                "rustok-forum category audience migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres | DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
DROP TABLE IF EXISTS forum_category_audience_users;
DROP TABLE IF EXISTS forum_category_audience_groups;
DROP TABLE IF EXISTS forum_category_audience_channels;
DROP TABLE IF EXISTS forum_category_audience_roles;
DROP TABLE IF EXISTS forum_category_audience_policies;
"#,
                    )
                    .await?;
                Ok(())
            }
            backend => Err(DbErr::Custom(format!(
                "rustok-forum category audience migration does not support {backend:?}"
            ))),
        }
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
CREATE TABLE IF NOT EXISTS forum_category_audience_policies (
    tenant_id UUID NOT NULL,
    category_id UUID NOT NULL,
    minimum_trust_level SMALLINT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT pk_forum_category_audience_policies
        PRIMARY KEY (tenant_id, category_id),
    CONSTRAINT fk_forum_category_audience_policy_category
        FOREIGN KEY (tenant_id, category_id)
        REFERENCES forum_categories (tenant_id, id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CONSTRAINT ck_forum_category_audience_minimum_trust
        CHECK (minimum_trust_level IS NULL OR minimum_trust_level BETWEEN 0 AND 100)
);

CREATE TABLE IF NOT EXISTS forum_category_audience_roles (
    tenant_id UUID NOT NULL,
    category_id UUID NOT NULL,
    role VARCHAR(32) NOT NULL,
    CONSTRAINT pk_forum_category_audience_roles
        PRIMARY KEY (tenant_id, category_id, role),
    CONSTRAINT fk_forum_category_audience_roles_policy
        FOREIGN KEY (tenant_id, category_id)
        REFERENCES forum_category_audience_policies (tenant_id, category_id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CONSTRAINT ck_forum_category_audience_role
        CHECK (role IN ('super_admin', 'admin', 'manager', 'customer'))
);

CREATE TABLE IF NOT EXISTS forum_category_audience_channels (
    tenant_id UUID NOT NULL,
    category_id UUID NOT NULL,
    channel_slug VARCHAR(128) NOT NULL,
    CONSTRAINT pk_forum_category_audience_channels
        PRIMARY KEY (tenant_id, category_id, channel_slug),
    CONSTRAINT fk_forum_category_audience_channels_policy
        FOREIGN KEY (tenant_id, category_id)
        REFERENCES forum_category_audience_policies (tenant_id, category_id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CONSTRAINT ck_forum_category_audience_channel_slug
        CHECK (
            length(channel_slug) BETWEEN 1 AND 128
            AND channel_slug = lower(channel_slug)
            AND channel_slug = btrim(channel_slug)
        )
);

CREATE TABLE IF NOT EXISTS forum_category_audience_groups (
    tenant_id UUID NOT NULL,
    category_id UUID NOT NULL,
    group_id UUID NOT NULL,
    CONSTRAINT pk_forum_category_audience_groups
        PRIMARY KEY (tenant_id, category_id, group_id),
    CONSTRAINT fk_forum_category_audience_groups_policy
        FOREIGN KEY (tenant_id, category_id)
        REFERENCES forum_category_audience_policies (tenant_id, category_id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CONSTRAINT ck_forum_category_audience_group_id
        CHECK (group_id <> '00000000-0000-0000-0000-000000000000'::uuid)
);

CREATE TABLE IF NOT EXISTS forum_category_audience_users (
    tenant_id UUID NOT NULL,
    category_id UUID NOT NULL,
    user_id UUID NOT NULL,
    effect VARCHAR(16) NOT NULL,
    CONSTRAINT pk_forum_category_audience_users
        PRIMARY KEY (tenant_id, category_id, user_id, effect),
    CONSTRAINT fk_forum_category_audience_users_policy
        FOREIGN KEY (tenant_id, category_id)
        REFERENCES forum_category_audience_policies (tenant_id, category_id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CONSTRAINT ck_forum_category_audience_user_id
        CHECK (user_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT ck_forum_category_audience_user_effect
        CHECK (effect IN ('allow', 'deny'))
);
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
CREATE TABLE IF NOT EXISTS forum_category_audience_policies (
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

CREATE TABLE IF NOT EXISTS forum_category_audience_roles (
    tenant_id TEXT NOT NULL,
    category_id TEXT NOT NULL,
    role TEXT NOT NULL,
    PRIMARY KEY (tenant_id, category_id, role),
    FOREIGN KEY (tenant_id, category_id)
        REFERENCES forum_category_audience_policies (tenant_id, category_id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CHECK (role IN ('super_admin', 'admin', 'manager', 'customer'))
);

CREATE TABLE IF NOT EXISTS forum_category_audience_channels (
    tenant_id TEXT NOT NULL,
    category_id TEXT NOT NULL,
    channel_slug TEXT NOT NULL,
    PRIMARY KEY (tenant_id, category_id, channel_slug),
    FOREIGN KEY (tenant_id, category_id)
        REFERENCES forum_category_audience_policies (tenant_id, category_id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CHECK (
        length(channel_slug) BETWEEN 1 AND 128
        AND channel_slug = lower(channel_slug)
        AND channel_slug = trim(channel_slug)
    )
);

CREATE TABLE IF NOT EXISTS forum_category_audience_groups (
    tenant_id TEXT NOT NULL,
    category_id TEXT NOT NULL,
    group_id TEXT NOT NULL,
    PRIMARY KEY (tenant_id, category_id, group_id),
    FOREIGN KEY (tenant_id, category_id)
        REFERENCES forum_category_audience_policies (tenant_id, category_id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CHECK (group_id <> '00000000-0000-0000-0000-000000000000')
);

CREATE TABLE IF NOT EXISTS forum_category_audience_users (
    tenant_id TEXT NOT NULL,
    category_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    effect TEXT NOT NULL,
    PRIMARY KEY (tenant_id, category_id, user_id, effect),
    FOREIGN KEY (tenant_id, category_id)
        REFERENCES forum_category_audience_policies (tenant_id, category_id)
        ON UPDATE CASCADE ON DELETE CASCADE,
    CHECK (user_id <> '00000000-0000-0000-0000-000000000000'),
    CHECK (effect IN ('allow', 'deny'))
);
"#,
        )
        .await?;
    Ok(())
}
