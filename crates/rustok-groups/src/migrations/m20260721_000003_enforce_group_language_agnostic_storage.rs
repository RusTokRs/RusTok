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

    IF EXISTS (
        SELECT 1
        FROM group_memberships
        WHERE jsonb_typeof(metadata) <> 'object'
           OR metadata ?| ARRAY[
                'title', 'summary', 'body', 'name', 'description',
                'translations', 'localized', 'locales', 'i18n', 'seo'
           ]
    ) THEN
        RAISE EXCEPTION 'groups language-agnostic migration blocked: membership metadata contains localized presentation copy';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM group_feature_bindings
        WHERE jsonb_typeof(configuration) <> 'object'
           OR configuration ?| ARRAY[
                'title', 'summary', 'body', 'name', 'description',
                'translations', 'localized', 'locales', 'i18n', 'seo'
           ]
    ) THEN
        RAISE EXCEPTION 'groups language-agnostic migration blocked: feature configuration contains localized presentation copy';
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

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'ck_group_memberships_metadata_language_agnostic'
    ) THEN
        ALTER TABLE group_memberships
            ADD CONSTRAINT ck_group_memberships_metadata_language_agnostic
            CHECK (
                jsonb_typeof(metadata) = 'object'
                AND NOT metadata ?| ARRAY[
                    'title', 'summary', 'body', 'name', 'description',
                    'translations', 'localized', 'locales', 'i18n', 'seo'
                ]
            );
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'ck_group_feature_bindings_configuration_language_agnostic'
    ) THEN
        ALTER TABLE group_feature_bindings
            ADD CONSTRAINT ck_group_feature_bindings_configuration_language_agnostic
            CHECK (
                jsonb_typeof(configuration) = 'object'
                AND NOT configuration ?| ARRAY[
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
    let existing_locale_violation = sqlite_locale_violation("locale");
    ensure_sqlite_zero(
        manager,
        &format!(
            r#"
SELECT COUNT(*) AS invalid_count
FROM group_translations
WHERE ({existing_locale_violation})
   OR trim(title) = ''
   OR length(title) > 240
   OR (summary IS NOT NULL AND length(summary) > 500)
"#
        ),
        "groups language-agnostic migration blocked: invalid localized presentation",
    )
    .await?;

    for (table, column, message) in [
        (
            "groups",
            "metadata",
            "groups language-agnostic migration blocked: group metadata contains localized presentation copy",
        ),
        (
            "group_memberships",
            "metadata",
            "groups language-agnostic migration blocked: membership metadata contains localized presentation copy",
        ),
        (
            "group_feature_bindings",
            "configuration",
            "groups language-agnostic migration blocked: feature configuration contains localized presentation copy",
        ),
    ] {
        ensure_sqlite_zero(
            manager,
            &format!(
                "SELECT COUNT(*) AS invalid_count FROM {table} WHERE {}",
                sqlite_language_agnostic_json_violation(column)
            ),
            message,
        )
        .await?;
    }

    let new_locale_violation = sqlite_locale_violation("NEW.locale");
    let statements = [
        format!(
            r#"CREATE TRIGGER IF NOT EXISTS group_translations_language_agnostic_insert BEFORE INSERT ON group_translations FOR EACH ROW WHEN ({new_locale_violation}) OR trim(NEW.title) = '' OR length(NEW.title) > 240 OR (NEW.summary IS NOT NULL AND length(NEW.summary) > 500) BEGIN SELECT RAISE(ABORT, 'group translation locale/presentation contract violation'); END"#
        ),
        format!(
            r#"CREATE TRIGGER IF NOT EXISTS group_translations_language_agnostic_update BEFORE UPDATE OF locale, title, summary ON group_translations FOR EACH ROW WHEN ({new_locale_violation}) OR trim(NEW.title) = '' OR length(NEW.title) > 240 OR (NEW.summary IS NOT NULL AND length(NEW.summary) > 500) BEGIN SELECT RAISE(ABORT, 'group translation locale/presentation contract violation'); END"#
        ),
        format!(
            "CREATE TRIGGER IF NOT EXISTS groups_language_agnostic_metadata_insert BEFORE INSERT ON groups FOR EACH ROW WHEN {} BEGIN SELECT RAISE(ABORT, 'group metadata must remain language-agnostic'); END",
            sqlite_language_agnostic_json_violation("NEW.metadata")
        ),
        format!(
            "CREATE TRIGGER IF NOT EXISTS groups_language_agnostic_metadata_update BEFORE UPDATE OF metadata ON groups FOR EACH ROW WHEN {} BEGIN SELECT RAISE(ABORT, 'group metadata must remain language-agnostic'); END",
            sqlite_language_agnostic_json_violation("NEW.metadata")
        ),
        format!(
            "CREATE TRIGGER IF NOT EXISTS group_memberships_language_agnostic_metadata_insert BEFORE INSERT ON group_memberships FOR EACH ROW WHEN {} BEGIN SELECT RAISE(ABORT, 'group membership metadata must remain language-agnostic'); END",
            sqlite_language_agnostic_json_violation("NEW.metadata")
        ),
        format!(
            "CREATE TRIGGER IF NOT EXISTS group_memberships_language_agnostic_metadata_update BEFORE UPDATE OF metadata ON group_memberships FOR EACH ROW WHEN {} BEGIN SELECT RAISE(ABORT, 'group membership metadata must remain language-agnostic'); END",
            sqlite_language_agnostic_json_violation("NEW.metadata")
        ),
        format!(
            "CREATE TRIGGER IF NOT EXISTS group_feature_bindings_language_agnostic_configuration_insert BEFORE INSERT ON group_feature_bindings FOR EACH ROW WHEN {} BEGIN SELECT RAISE(ABORT, 'group feature configuration must remain language-agnostic'); END",
            sqlite_language_agnostic_json_violation("NEW.configuration")
        ),
        format!(
            "CREATE TRIGGER IF NOT EXISTS group_feature_bindings_language_agnostic_configuration_update BEFORE UPDATE OF configuration ON group_feature_bindings FOR EACH ROW WHEN {} BEGIN SELECT RAISE(ABORT, 'group feature configuration must remain language-agnostic'); END",
            sqlite_language_agnostic_json_violation("NEW.configuration")
        ),
    ];
    for statement in statements {
        manager
            .get_connection()
            .execute_unprepared(&statement)
            .await?;
    }

    Ok(())
}

fn sqlite_locale_violation(locale_expression: &str) -> String {
    format!(
        r#"
length({locale_expression}) < 2
OR length({locale_expression}) > 32
OR {locale_expression} <> trim({locale_expression})
OR {locale_expression} GLOB '*[^A-Za-z0-9-]*'
OR {locale_expression} GLOB '*--*'
OR substr({locale_expression}, 1, 1) = '-'
OR substr({locale_expression}, -1, 1) = '-'
OR EXISTS (
    WITH RECURSIVE locale_parts(rest, part, position) AS (
        SELECT {locale_expression} || '-', '', 0
        UNION ALL
        SELECT
            substr(rest, instr(rest, '-') + 1),
            substr(rest, 1, instr(rest, '-') - 1),
            position + 1
        FROM locale_parts
        WHERE rest <> ''
    )
    SELECT 1
    FROM locale_parts
    WHERE position > 0
      AND (
          (
              position = 1
              AND (
                  length(part) NOT BETWEEN 2 AND 8
                  OR part GLOB '*[^a-z]*'
              )
          )
          OR (
              position > 1
              AND NOT (
                  (length(part) = 2 AND part NOT GLOB '*[^A-Z]*')
                  OR (
                      length(part) = 4
                      AND substr(part, 1, 1) GLOB '[A-Z]'
                      AND substr(part, 2) NOT GLOB '*[^a-z]*'
                  )
                  OR (length(part) = 3 AND part NOT GLOB '*[^0-9]*')
                  OR (
                      length(part) BETWEEN 5 AND 8
                      AND part NOT GLOB '*[^a-z0-9]*'
                  )
              )
          )
      )
)
"#
    )
}

fn sqlite_language_agnostic_json_violation(json_expression: &str) -> String {
    const LOCALIZED_COPY_PATHS: &[&str] = &[
        "title",
        "summary",
        "body",
        "name",
        "description",
        "translations",
        "localized",
        "locales",
        "i18n",
        "seo",
    ];

    let reserved_paths = LOCALIZED_COPY_PATHS
        .iter()
        .map(|key| format!("json_type({json_expression}, '$.{key}') IS NOT NULL"))
        .collect::<Vec<_>>()
        .join(" OR ");
    format!(
        "json_valid({json_expression}) = 0 OR json_type({json_expression}) <> 'object' OR {reserved_paths}"
    )
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
