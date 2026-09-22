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
                "rustok-forum category route alias migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => execute(manager, POSTGRES_DOWN).await,
            DatabaseBackend::Sqlite => execute(manager, SQLITE_DOWN).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum category route alias rollback does not support {backend:?}"
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
CREATE TABLE IF NOT EXISTS forum_category_route_aliases (
    tenant_id UUID NOT NULL,
    alias_id UUID NOT NULL,
    category_id UUID NOT NULL,
    locale VARCHAR(64) NOT NULL,
    slug VARCHAR(255) NOT NULL,
    reason VARCHAR(500) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT pk_forum_category_route_aliases
        PRIMARY KEY (tenant_id, alias_id),
    CONSTRAINT uq_forum_category_route_alias
        UNIQUE (tenant_id, locale, slug),
    CONSTRAINT fk_forum_category_route_alias_category
        FOREIGN KEY (tenant_id, category_id)
        REFERENCES forum_categories (tenant_id, id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    CONSTRAINT ck_forum_category_route_alias_ids CHECK (
        alias_id <> '00000000-0000-0000-0000-000000000000'::uuid
        AND category_id <> '00000000-0000-0000-0000-000000000000'::uuid
    ),
    CONSTRAINT ck_forum_category_route_alias_locale CHECK (
        length(locale) BETWEEN 1 AND 64
        AND locale = btrim(locale)
        AND position(E'\n' in locale) = 0
        AND position(E'\r' in locale) = 0
    ),
    CONSTRAINT ck_forum_category_route_alias_slug CHECK (
        length(slug) BETWEEN 1 AND 255
        AND slug = lower(slug)
        AND slug = btrim(slug)
        AND slug ~ '^[a-z0-9]+(?:-[a-z0-9]+)*$'
    ),
    CONSTRAINT ck_forum_category_route_alias_reason CHECK (
        length(reason) BETWEEN 1 AND 500
        AND reason = btrim(reason)
        AND position(E'\n' in reason) = 0
        AND position(E'\r' in reason) = 0
    )
);

CREATE INDEX IF NOT EXISTS idx_forum_category_route_alias_category
    ON forum_category_route_aliases (
        tenant_id, category_id, locale, created_at, alias_id
    );

CREATE OR REPLACE FUNCTION forum_reject_category_route_alias_mutation()
RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'forum category route aliases are append-only';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_guard_category_translation_route_alias()
RETURNS trigger AS $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM forum_category_route_aliases alias
        WHERE alias.tenant_id = NEW.tenant_id
          AND alias.locale = NEW.locale
          AND alias.slug = NEW.slug
    ) THEN
        RAISE EXCEPTION 'forum category route is reserved by alias';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_guard_category_route_alias_insert()
RETURNS trigger AS $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM forum_category_translations translation
        WHERE translation.tenant_id = NEW.tenant_id
          AND translation.locale = NEW.locale
          AND translation.slug = NEW.slug
    ) THEN
        RAISE EXCEPTION 'forum category route alias conflicts with current route';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_category_route_alias_update
    ON forum_category_route_aliases;
CREATE TRIGGER forum_category_route_alias_update
BEFORE UPDATE ON forum_category_route_aliases
FOR EACH ROW EXECUTE FUNCTION forum_reject_category_route_alias_mutation();

DROP TRIGGER IF EXISTS forum_category_route_alias_delete
    ON forum_category_route_aliases;
CREATE TRIGGER forum_category_route_alias_delete
BEFORE DELETE ON forum_category_route_aliases
FOR EACH ROW EXECUTE FUNCTION forum_reject_category_route_alias_mutation();

DROP TRIGGER IF EXISTS forum_category_translation_route_alias_guard
    ON forum_category_translations;
CREATE TRIGGER forum_category_translation_route_alias_guard
BEFORE INSERT OR UPDATE OF tenant_id, locale, slug ON forum_category_translations
FOR EACH ROW EXECUTE FUNCTION forum_guard_category_translation_route_alias();

DROP TRIGGER IF EXISTS forum_category_route_alias_insert_guard
    ON forum_category_route_aliases;
CREATE TRIGGER forum_category_route_alias_insert_guard
BEFORE INSERT ON forum_category_route_aliases
FOR EACH ROW EXECUTE FUNCTION forum_guard_category_route_alias_insert();
"#;

const POSTGRES_DOWN: &str = r#"
DROP TRIGGER IF EXISTS forum_category_route_alias_insert_guard
    ON forum_category_route_aliases;
DROP TRIGGER IF EXISTS forum_category_translation_route_alias_guard
    ON forum_category_translations;
DROP TRIGGER IF EXISTS forum_category_route_alias_delete
    ON forum_category_route_aliases;
DROP TRIGGER IF EXISTS forum_category_route_alias_update
    ON forum_category_route_aliases;
DROP TABLE IF EXISTS forum_category_route_aliases;
DROP FUNCTION IF EXISTS forum_guard_category_route_alias_insert();
DROP FUNCTION IF EXISTS forum_guard_category_translation_route_alias();
DROP FUNCTION IF EXISTS forum_reject_category_route_alias_mutation();
"#;

const SQLITE_UP: &str = r#"
CREATE TABLE IF NOT EXISTS forum_category_route_aliases (
    tenant_id TEXT NOT NULL,
    alias_id TEXT NOT NULL,
    category_id TEXT NOT NULL,
    locale TEXT NOT NULL,
    slug TEXT NOT NULL,
    reason TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (tenant_id, alias_id),
    UNIQUE (tenant_id, locale, slug),
    FOREIGN KEY (tenant_id, category_id)
        REFERENCES forum_categories (tenant_id, id)
        ON UPDATE RESTRICT ON DELETE RESTRICT,
    CHECK (
        alias_id <> '00000000-0000-0000-0000-000000000000'
        AND category_id <> '00000000-0000-0000-0000-000000000000'
    ),
    CHECK (
        length(locale) BETWEEN 1 AND 64
        AND locale = trim(locale)
        AND instr(locale, char(10)) = 0
        AND instr(locale, char(13)) = 0
    ),
    CHECK (
        length(slug) BETWEEN 1 AND 255
        AND slug = lower(slug)
        AND slug = trim(slug)
        AND slug NOT GLOB '*[^a-z0-9-]*'
        AND slug NOT LIKE '-%'
        AND slug NOT LIKE '%-'
        AND slug NOT LIKE '%--%'
    ),
    CHECK (
        length(reason) BETWEEN 1 AND 500
        AND reason = trim(reason)
        AND instr(reason, char(10)) = 0
        AND instr(reason, char(13)) = 0
    )
);

CREATE INDEX IF NOT EXISTS idx_forum_category_route_alias_category
    ON forum_category_route_aliases (
        tenant_id, category_id, locale, created_at, alias_id
    );

CREATE TRIGGER IF NOT EXISTS forum_category_route_alias_update
BEFORE UPDATE ON forum_category_route_aliases
BEGIN
    SELECT RAISE(ABORT, 'forum category route aliases are append-only');
END;

CREATE TRIGGER IF NOT EXISTS forum_category_route_alias_delete
BEFORE DELETE ON forum_category_route_aliases
BEGIN
    SELECT RAISE(ABORT, 'forum category route aliases are append-only');
END;

CREATE TRIGGER IF NOT EXISTS forum_category_translation_route_alias_insert_guard
BEFORE INSERT ON forum_category_translations
WHEN EXISTS (
    SELECT 1
    FROM forum_category_route_aliases alias
    WHERE alias.tenant_id = NEW.tenant_id
      AND alias.locale = NEW.locale
      AND alias.slug = NEW.slug
)
BEGIN
    SELECT RAISE(ABORT, 'forum category route is reserved by alias');
END;

CREATE TRIGGER IF NOT EXISTS forum_category_translation_route_alias_update_guard
BEFORE UPDATE OF tenant_id, locale, slug ON forum_category_translations
WHEN EXISTS (
    SELECT 1
    FROM forum_category_route_aliases alias
    WHERE alias.tenant_id = NEW.tenant_id
      AND alias.locale = NEW.locale
      AND alias.slug = NEW.slug
)
BEGIN
    SELECT RAISE(ABORT, 'forum category route is reserved by alias');
END;

CREATE TRIGGER IF NOT EXISTS forum_category_route_alias_insert_guard
BEFORE INSERT ON forum_category_route_aliases
WHEN EXISTS (
    SELECT 1
    FROM forum_category_translations translation
    WHERE translation.tenant_id = NEW.tenant_id
      AND translation.locale = NEW.locale
      AND translation.slug = NEW.slug
)
BEGIN
    SELECT RAISE(ABORT, 'forum category route alias conflicts with current route');
END;
"#;

const SQLITE_DOWN: &str = r#"
DROP TRIGGER IF EXISTS forum_category_route_alias_insert_guard;
DROP TRIGGER IF EXISTS forum_category_translation_route_alias_update_guard;
DROP TRIGGER IF EXISTS forum_category_translation_route_alias_insert_guard;
DROP TRIGGER IF EXISTS forum_category_route_alias_delete;
DROP TRIGGER IF EXISTS forum_category_route_alias_update;
DROP TABLE IF EXISTS forum_category_route_aliases;
"#;
