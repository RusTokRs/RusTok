use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

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

        manager
            .get_connection()
            .execute_unprepared(
                r#"
INSERT INTO flex_schema_translations (schema_id, locale, name, description, created_at, updated_at)
SELECT
    flex_schema.id,
    COALESCE(NULLIF(tenant.default_locale, ''), 'en'),
    flex_schema.name,
    flex_schema.description,
    flex_schema.created_at,
    flex_schema.updated_at
FROM flex_schemas AS flex_schema
INNER JOIN tenants AS tenant ON tenant.id = flex_schema.tenant_id
ON CONFLICT (schema_id, locale) DO NOTHING;
"#,
            )
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

        manager
            .get_connection()
            .execute_unprepared(
                r#"
UPDATE flex_schemas AS flex_schema
SET
    name = chosen.name,
    description = chosen.description
FROM (
    SELECT DISTINCT ON (translation.schema_id)
        translation.schema_id,
        translation.name,
        translation.description
    FROM flex_schema_translations AS translation
    INNER JOIN flex_schemas AS source_flex_schema ON source_flex_schema.id = translation.schema_id
    INNER JOIN tenants AS tenant ON tenant.id = source_flex_schema.tenant_id
    ORDER BY
        translation.schema_id,
        CASE
            WHEN translation.locale = tenant.default_locale THEN 0
            WHEN translation.locale = 'en' THEN 1
            ELSE 2
        END,
        translation.locale
) AS chosen
WHERE flex_schema.id = chosen.schema_id;
"#,
            )
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
