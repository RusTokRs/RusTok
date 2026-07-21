use sea_orm_migration::prelude::*;

use sea_orm_migration::sea_orm::DatabaseBackend;

const LEGACY_UNDETERMINED_LOCALE: &str = "und";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FlexSchemaTranslations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FlexSchemaTranslations::SchemaId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FlexSchemaTranslations::Locale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FlexSchemaTranslations::Name)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FlexSchemaTranslations::Description)
                            .text()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(FlexSchemaTranslations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(FlexSchemaTranslations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(FlexSchemaTranslations::SchemaId)
                            .col(FlexSchemaTranslations::Locale),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                FlexSchemaTranslations::Table,
                                FlexSchemaTranslations::SchemaId,
                            )
                            .to(Alias::new("flex_schemas"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // The original schema row did not record the language of name/description.
        // Runtime tenant defaults are selection policy, not historical provenance.
        manager
            .get_connection()
            .execute_unprepared(&format!(
                r#"
INSERT INTO flex_schema_translations (schema_id, locale, name, description, created_at, updated_at)
SELECT
    flex_schema.id,
    '{LEGACY_UNDETERMINED_LOCALE}',
    flex_schema.name,
    flex_schema.description,
    flex_schema.created_at,
    flex_schema.updated_at
FROM flex_schemas AS flex_schema
"#
            ))
            .await?;

        if manager.get_database_backend() == DatabaseBackend::Sqlite {
            manager
                .alter_table(
                    Table::alter()
                        .table(FlexSchemas::Table)
                        .drop_column(FlexSchemas::Name)
                        .to_owned(),
                )
                .await?;
            manager
                .alter_table(
                    Table::alter()
                        .table(FlexSchemas::Table)
                        .drop_column(FlexSchemas::Description)
                        .to_owned(),
                )
                .await
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(FlexSchemas::Table)
                        .drop_column(FlexSchemas::Name)
                        .drop_column(FlexSchemas::Description)
                        .to_owned(),
                )
                .await
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == DatabaseBackend::Sqlite {
            manager
                .alter_table(
                    Table::alter()
                        .table(FlexSchemas::Table)
                        .add_column(
                            ColumnDef::new(FlexSchemas::Name)
                                .string_len(255)
                                .not_null()
                                .default(""),
                        )
                        .to_owned(),
                )
                .await?;
            manager
                .alter_table(
                    Table::alter()
                        .table(FlexSchemas::Table)
                        .add_column(ColumnDef::new(FlexSchemas::Description).text().null())
                        .to_owned(),
                )
                .await?;
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(FlexSchemas::Table)
                        .add_column(
                            ColumnDef::new(FlexSchemas::Name)
                                .string_len(255)
                                .not_null()
                                .default(""),
                        )
                        .add_column(ColumnDef::new(FlexSchemas::Description).text().null())
                        .to_owned(),
                )
                .await?;
        }

        // Restore one compatibility copy deterministically. The preserved `und` row
        // is preferred; a real locale is used only when no provenance row exists.
        manager
            .get_connection()
            .execute_unprepared(&format!(
                r#"
UPDATE flex_schemas
SET
    name = COALESCE((
        SELECT translation.name
        FROM flex_schema_translations AS translation
        WHERE translation.schema_id = flex_schemas.id
        ORDER BY CASE WHEN translation.locale = '{LEGACY_UNDETERMINED_LOCALE}' THEN 0 ELSE 1 END,
                 translation.locale
        LIMIT 1
    ), ''),
    description = (
        SELECT translation.description
        FROM flex_schema_translations AS translation
        WHERE translation.schema_id = flex_schemas.id
        ORDER BY CASE WHEN translation.locale = '{LEGACY_UNDETERMINED_LOCALE}' THEN 0 ELSE 1 END,
                 translation.locale
        LIMIT 1
    )
"#
            ))
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .table(FlexSchemaTranslations::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum FlexSchemas {
    Table,
    Name,
    Description,
}

#[derive(DeriveIden)]
enum FlexSchemaTranslations {
    Table,
    SchemaId,
    Locale,
    Name,
    Description,
    CreatedAt,
    UpdatedAt,
}
