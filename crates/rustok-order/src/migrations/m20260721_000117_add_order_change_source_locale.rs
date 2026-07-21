use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

const LEGACY_UNDETERMINED_LOCALE: &str = "und";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(OrderChanges::Table)
                    .add_column(
                        ColumnDef::new(OrderChanges::SourceLocale)
                            .string_len(32)
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager
                    .get_connection()
                    .execute_unprepared(&format!(
                        r#"
UPDATE order_changes
SET source_locale = '{LEGACY_UNDETERMINED_LOCALE}'
WHERE description IS NOT NULL AND source_locale IS NULL;

ALTER TABLE order_changes
    ADD CONSTRAINT ck_order_changes_description_source_locale
    CHECK (description IS NULL OR source_locale IS NOT NULL);
ALTER TABLE order_changes
    ADD CONSTRAINT ck_order_changes_source_locale_shape
    CHECK (
        source_locale IS NULL OR (
            octet_length(source_locale) BETWEEN 2 AND 32
            AND source_locale = btrim(source_locale)
            AND source_locale ~ '^[A-Za-z0-9]+([_-][A-Za-z0-9]+)*$'
        )
    );
"#
                    ))
                    .await?;
            }
            DatabaseBackend::MySql => {
                manager
                    .get_connection()
                    .execute_unprepared(&format!(
                        "UPDATE order_changes SET source_locale = '{LEGACY_UNDETERMINED_LOCALE}' WHERE description IS NOT NULL AND source_locale IS NULL",
                    ))
                    .await?;
                manager
                    .get_connection()
                    .execute_unprepared(
                        "ALTER TABLE order_changes ADD CONSTRAINT ck_order_changes_description_source_locale CHECK (description IS NULL OR source_locale IS NOT NULL)",
                    )
                    .await?;
                manager
                    .get_connection()
                    .execute_unprepared(
                        "ALTER TABLE order_changes ADD CONSTRAINT ck_order_changes_source_locale_shape CHECK (source_locale IS NULL OR (OCTET_LENGTH(source_locale) BETWEEN 2 AND 32 AND source_locale = TRIM(source_locale) AND source_locale REGEXP '^[A-Za-z0-9]+([_-][A-Za-z0-9]+)*$'))",
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(&format!(
                        r#"
UPDATE order_changes
SET source_locale = '{LEGACY_UNDETERMINED_LOCALE}'
WHERE description IS NOT NULL AND source_locale IS NULL;

CREATE TRIGGER order_changes_source_locale_insert
BEFORE INSERT ON order_changes FOR EACH ROW BEGIN
    SELECT CASE WHEN NEW.description IS NOT NULL AND NEW.source_locale IS NULL
        THEN RAISE(ABORT, 'order change description requires source locale') END;
    SELECT CASE WHEN NEW.source_locale IS NOT NULL AND (
        length(trim(NEW.source_locale)) < 2 OR length(trim(NEW.source_locale)) > 32
        OR NEW.source_locale <> trim(NEW.source_locale)
        OR trim(NEW.source_locale) GLOB '*[^A-Za-z0-9_-]*'
    ) THEN RAISE(ABORT, 'invalid order change source locale') END;
END;

CREATE TRIGGER order_changes_source_locale_update
BEFORE UPDATE OF description, source_locale ON order_changes FOR EACH ROW BEGIN
    SELECT CASE WHEN NEW.description IS NOT NULL AND NEW.source_locale IS NULL
        THEN RAISE(ABORT, 'order change description requires source locale') END;
    SELECT CASE WHEN NEW.source_locale IS NOT NULL AND (
        length(trim(NEW.source_locale)) < 2 OR length(trim(NEW.source_locale)) > 32
        OR NEW.source_locale <> trim(NEW.source_locale)
        OR trim(NEW.source_locale) GLOB '*[^A-Za-z0-9_-]*'
    ) THEN RAISE(ABORT, 'invalid order change source locale') END;
END;
"#
                    ))
                    .await?;
            }
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only: dropping source_locale would destroy attribution for immutable
        // human prose and would make legacy `und` provenance indistinguishable again.
        Ok(())
    }
}

#[derive(DeriveIden)]
enum OrderChanges {
    Table,
    SourceLocale,
}
