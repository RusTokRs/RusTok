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
                "rustok-groups language-agnostic migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Irreversible by design. Removing normalized-locale and language-agnostic
        // metadata constraints would make current canonical rows ambiguous.
        Ok(())
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
        FROM group_translations
        WHERE locale !~ '^[a-z]{2,8}(-([A-Z]{2}|[A-Z][a-z]{3}|[0-9]{3}|[a-z0-9]{5,8}))*$'
    ) THEN
        RAISE EXCEPTION 'groups language-agnostic migration blocked: non-normalized locale';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM group_translations
        WHERE btrim(title) = ''
           OR char_length(title) > 240
           OR (summary IS NOT NULL AND char_length(summary) > 500)
    ) THEN
        RAISE EXCEPTION 'groups language-agnostic migration blocked: invalid localized presentation';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM groups
        WHERE jsonb_typeof(metadata) <> 'object'
           OR metadata ?| ARRAY[
                'title', 'summary', 'body', 'name', 'description',
                'translations', 'localized', 'locales', 'i18n', 'seo'
           ]
    ) THEN
        RAISE EXCEPTION 'groups language-agnostic migration blocked: group metadata contains localized presentation copy';
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'ck_group_translations_locale_normalized'
    ) THEN
        ALTER TABLE group_translations
            ADD CONSTRAINT ck_group_translations_locale_normalized
            CHECK (
                locale ~ '^[a-z]{2,8}(-([A-Z]{2}|[A-Z][a-z]{3}|[0-9]{3}|[a-z0-9]{5,8}))*$'
            );
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'ck_group_translations_presentation_shape'
    ) THEN
        ALTER TABLE group_translations
            ADD CONSTRAINT ck_group_translations_presentation_shape
            CHECK (
                btrim(title) <> ''
                AND char_length(title) <= 240
                AND (summary IS NULL OR char_length(summary) <= 500)
            );
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'ck_groups_metadata_language_agnostic'
    ) THEN
        ALTER TABLE groups
            ADD CONSTRAINT ck_groups_metadata_language_agnostic
            CHECK (
                jsonb_typeof(metadata) = 'object'
                AND NOT metadata ?| ARRAY[
                    'title', 'summary', 'body', 'name', 'description',
                    'translations', 'localized', 'locales', 'i18n', 'seo'
                ]
            );
    END IF;
END $$;
"#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    ensure_sqlite_zero(
        manager,
        r#"
SELECT COUNT(*) AS invalid_count
FROM group_translations
WHERE length(locale) < 2
   OR length(locale) > 32
   OR locale <> trim(locale)
   OR locale GLOB '*[^A-Za-z0-9-]*'
   OR locale GLOB '*--*'
   OR substr(locale, 1, 1) = '-'
   OR substr(locale, -1, 1) = '-'
   OR length(substr(locale, 1, CASE WHEN instr(locale, '-') = 0 THEN length(locale) ELSE instr(locale, '-') - 1 END)) NOT BETWEEN 2 AND 8
   OR substr(locale, 1, CASE WHEN instr(locale, '-') = 0 THEN length(locale) ELSE instr(locale, '-') - 1 END) GLOB '*[^a-z]*'
   OR trim(title) = ''
   OR length(title) > 240
   OR (summary IS NOT NULL AND length(summary) > 500)
"#,
        "groups language-agnostic migration blocked: invalid localized presentation",
    )
    .await?;

    ensure_sqlite_zero(
        manager,
        r#"
SELECT COUNT(*) AS invalid_count
FROM groups
WHERE json_valid(metadata) = 0
   OR json_type(metadata) <> 'object'
   OR json_type(metadata, '$.title') IS NOT NULL
   OR json_type(metadata, '$.summary') IS NOT NULL
   OR json_type(metadata, '$.body') IS NOT NULL
   OR json_type(metadata, '$.name') IS NOT NULL
   OR json_type(metadata, '$.description') IS NOT NULL
   OR json_type(metadata, '$.translations') IS NOT NULL
   OR json_type(metadata, '$.localized') IS NOT NULL
   OR json_type(metadata, '$.locales') IS NOT NULL
   OR json_type(metadata, '$.i18n') IS NOT NULL
   OR json_type(metadata, '$.seo') IS NOT NULL
"#,
        "groups language-agnostic migration blocked: group metadata contains localized presentation copy",
    )
    .await?;

    for statement in [
        r#"CREATE TRIGGER IF NOT EXISTS group_translations_language_agnostic_insert BEFORE INSERT ON group_translations FOR EACH ROW WHEN length(NEW.locale) < 2 OR length(NEW.locale) > 32 OR NEW.locale <> trim(NEW.locale) OR NEW.locale GLOB '*[^A-Za-z0-9-]*' OR NEW.locale GLOB '*--*' OR substr(NEW.locale, 1, 1) = '-' OR substr(NEW.locale, -1, 1) = '-' OR length(substr(NEW.locale, 1, CASE WHEN instr(NEW.locale, '-') = 0 THEN length(NEW.locale) ELSE instr(NEW.locale, '-') - 1 END)) NOT BETWEEN 2 AND 8 OR substr(NEW.locale, 1, CASE WHEN instr(NEW.locale, '-') = 0 THEN length(NEW.locale) ELSE instr(NEW.locale, '-') - 1 END) GLOB '*[^a-z]*' OR trim(NEW.title) = '' OR length(NEW.title) > 240 OR (NEW.summary IS NOT NULL AND length(NEW.summary) > 500) BEGIN SELECT RAISE(ABORT, 'group translation locale/presentation contract violation'); END"#,
        r#"CREATE TRIGGER IF NOT EXISTS group_translations_language_agnostic_update BEFORE UPDATE OF locale, title, summary ON group_translations FOR EACH ROW WHEN length(NEW.locale) < 2 OR length(NEW.locale) > 32 OR NEW.locale <> trim(NEW.locale) OR NEW.locale GLOB '*[^A-Za-z0-9-]*' OR NEW.locale GLOB '*--*' OR substr(NEW.locale, 1, 1) = '-' OR substr(NEW.locale, -1, 1) = '-' OR length(substr(NEW.locale, 1, CASE WHEN instr(NEW.locale, '-') = 0 THEN length(NEW.locale) ELSE instr(NEW.locale, '-') - 1 END)) NOT BETWEEN 2 AND 8 OR substr(NEW.locale, 1, CASE WHEN instr(NEW.locale, '-') = 0 THEN length(NEW.locale) ELSE instr(NEW.locale, '-') - 1 END) GLOB '*[^a-z]*' OR trim(NEW.title) = '' OR length(NEW.title) > 240 OR (NEW.summary IS NOT NULL AND length(NEW.summary) > 500) BEGIN SELECT RAISE(ABORT, 'group translation locale/presentation contract violation'); END"#,
        r#"CREATE TRIGGER IF NOT EXISTS groups_language_agnostic_metadata_insert BEFORE INSERT ON groups FOR EACH ROW WHEN json_valid(NEW.metadata) = 0 OR json_type(NEW.metadata) <> 'object' OR json_type(NEW.metadata, '$.title') IS NOT NULL OR json_type(NEW.metadata, '$.summary') IS NOT NULL OR json_type(NEW.metadata, '$.body') IS NOT NULL OR json_type(NEW.metadata, '$.name') IS NOT NULL OR json_type(NEW.metadata, '$.description') IS NOT NULL OR json_type(NEW.metadata, '$.translations') IS NOT NULL OR json_type(NEW.metadata, '$.localized') IS NOT NULL OR json_type(NEW.metadata, '$.locales') IS NOT NULL OR json_type(NEW.metadata, '$.i18n') IS NOT NULL OR json_type(NEW.metadata, '$.seo') IS NOT NULL BEGIN SELECT RAISE(ABORT, 'group metadata must remain language-agnostic'); END"#,
        r#"CREATE TRIGGER IF NOT EXISTS groups_language_agnostic_metadata_update BEFORE UPDATE OF metadata ON groups FOR EACH ROW WHEN json_valid(NEW.metadata) = 0 OR json_type(NEW.metadata) <> 'object' OR json_type(NEW.metadata, '$.title') IS NOT NULL OR json_type(NEW.metadata, '$.summary') IS NOT NULL OR json_type(NEW.metadata, '$.body') IS NOT NULL OR json_type(NEW.metadata, '$.name') IS NOT NULL OR json_type(NEW.metadata, '$.description') IS NOT NULL OR json_type(NEW.metadata, '$.translations') IS NOT NULL OR json_type(NEW.metadata, '$.localized') IS NOT NULL OR json_type(NEW.metadata, '$.locales') IS NOT NULL OR json_type(NEW.metadata, '$.i18n') IS NOT NULL OR json_type(NEW.metadata, '$.seo') IS NOT NULL BEGIN SELECT RAISE(ABORT, 'group metadata must remain language-agnostic'); END"#,
    ] {
        manager
            .get_connection()
            .execute_unprepared(statement)
            .await?;
    }

    Ok(())
}

async fn ensure_sqlite_zero(
    manager: &SchemaManager<'_>,
    sql: &str,
    message: &str,
) -> Result<(), DbErr> {
    let row = manager
        .get_connection()
        .query_one(Statement::from_string(DatabaseBackend::Sqlite, sql.to_owned()))
        .await?
        .ok_or_else(|| DbErr::Custom("failed to validate Groups language-agnostic migration".into()))?;
    let invalid_count: i64 = row.try_get("", "invalid_count")?;
    if invalid_count != 0 {
        return Err(DbErr::Custom(message.to_owned()));
    }
    Ok(())
}
