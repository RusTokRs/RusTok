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
                            .not_null()
                            .default(LEGACY_UNDETERMINED_LOCALE),
                    )
                    .to_owned(),
            )
            .await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
ALTER TABLE order_changes
    ADD CONSTRAINT ck_order_changes_source_locale_shape
    CHECK (
        octet_length(source_locale) BETWEEN 2 AND 32
        AND source_locale = btrim(source_locale)
        AND source_locale ~ '^[A-Za-z0-9]+([_-][A-Za-z0-9]+)*$'
    );
"#,
                    )
                    .await?;
            }
            DatabaseBackend::MySql => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        "ALTER TABLE order_changes ADD CONSTRAINT ck_order_changes_source_locale_shape CHECK (OCTET_LENGTH(source_locale) BETWEEN 2 AND 32 AND source_locale = TRIM(source_locale) AND source_locale REGEXP '^[A-Za-z0-9]+([_-][A-Za-z0-9]+)*$')",
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
CREATE TRIGGER order_changes_source_locale_insert
BEFORE INSERT ON order_changes FOR EACH ROW BEGIN
    SELECT CASE WHEN (
        length(trim(NEW.source_locale)) < 2 OR length(trim(NEW.source_locale)) > 32
        OR NEW.source_locale <> trim(NEW.source_locale)
        OR trim(NEW.source_locale) GLOB '*[^A-Za-z0-9_-]*'
    ) THEN RAISE(ABORT, 'invalid order change source locale') END;
END;

CREATE TRIGGER order_changes_source_locale_update
BEFORE UPDATE OF source_locale ON order_changes FOR EACH ROW BEGIN
    SELECT CASE WHEN (
        length(trim(NEW.source_locale)) < 2 OR length(trim(NEW.source_locale)) > 32
        OR NEW.source_locale <> trim(NEW.source_locale)
        OR trim(NEW.source_locale) GLOB '*[^A-Za-z0-9_-]*'
    ) THEN RAISE(ABORT, 'invalid order change source locale') END;
END;
"#,
                    )
                    .await?;
            }
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only: dropping source_locale would destroy attribution for immutable
        // human prose and would make `und` provenance indistinguishable again.
        Ok(())
    }
}

#[derive(DeriveIden)]
enum OrderChanges {
    Table,
    SourceLocale,
}
