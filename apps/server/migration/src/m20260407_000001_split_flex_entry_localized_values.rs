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
                    .table(FlexEntryLocalizedValues::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FlexEntryLocalizedValues::EntryId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FlexEntryLocalizedValues::Locale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FlexEntryLocalizedValues::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FlexEntryLocalizedValues::Data)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(FlexEntryLocalizedValues::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(FlexEntryLocalizedValues::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(FlexEntryLocalizedValues::EntryId)
                            .col(FlexEntryLocalizedValues::Locale),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                FlexEntryLocalizedValues::Table,
                                FlexEntryLocalizedValues::EntryId,
                            )
                            .to(Alias::new("flex_entries"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                FlexEntryLocalizedValues::Table,
                                FlexEntryLocalizedValues::TenantId,
                            )
                            .to(Alias::new("tenants"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_flex_entry_localized_values_owner")
                    .table(FlexEntryLocalizedValues::Table)
                    .col(FlexEntryLocalizedValues::TenantId)
                    .col(FlexEntryLocalizedValues::EntryId)
                    .to_owned(),
            )
            .await?;

        if manager.get_database_backend() == DatabaseBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared(
                    r#"
INSERT INTO flex_entry_localized_values (entry_id, locale, tenant_id, data, created_at, updated_at)
SELECT
    entry_row.id,
    COALESCE(NULLIF(tenant.default_locale, ''), 'en'),
    entry_row.tenant_id,
    COALESCE(
        jsonb_object_agg(localized_kv.field_key, localized_kv.field_value)
            FILTER (WHERE localized_kv.field_value IS NOT NULL),
        '{}'::jsonb
    ),
    entry_row.created_at,
    entry_row.updated_at
FROM flex_entries AS entry_row
JOIN flex_schemas AS schema_row ON schema_row.id = entry_row.schema_id
JOIN tenants AS tenant ON tenant.id = entry_row.tenant_id
LEFT JOIN LATERAL (
    SELECT
        definition ->> 'field_key' AS field_key,
        entry_row.data -> (definition ->> 'field_key') AS field_value
    FROM jsonb_array_elements(schema_row.fields_config) AS definition
    WHERE COALESCE((definition ->> 'is_localized')::boolean, false)
      AND (definition ->> 'field_key') IS NOT NULL
      AND NULLIF(definition ->> 'field_key', '') IS NOT NULL
) AS localized_kv ON TRUE
GROUP BY
    entry_row.id,
    entry_row.tenant_id,
    COALESCE(NULLIF(tenant.default_locale, ''), 'en'),
    entry_row.created_at,
    entry_row.updated_at
ON CONFLICT (entry_id, locale) DO UPDATE
SET
    data = EXCLUDED.data,
    updated_at = EXCLUDED.updated_at
"#,
                )
                .await?;

            manager
                .get_connection()
                .execute_unprepared(
                    r#"
UPDATE flex_entries AS entry_row
SET data = COALESCE(
    (
        SELECT jsonb_object_agg(item.key, item.value)
        FROM jsonb_each(entry_row.data) AS item
        WHERE NOT EXISTS (
            SELECT 1
            FROM flex_schemas AS schema_row,
                 jsonb_array_elements(schema_row.fields_config) AS definition
            WHERE schema_row.id = entry_row.schema_id
              AND definition ->> 'field_key' = item.key
              AND COALESCE((definition ->> 'is_localized')::boolean, false)
        )
    ),
    '{}'::jsonb
)
WHERE EXISTS (
    SELECT 1
    FROM flex_schemas AS schema_row,
         jsonb_array_elements(schema_row.fields_config) AS definition
    WHERE schema_row.id = entry_row.schema_id
      AND entry_row.data ? (definition ->> 'field_key')
      AND COALESCE((definition ->> 'is_localized')::boolean, false)
)
"#,
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == DatabaseBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared(
                    r#"
UPDATE flex_entries AS entry_row
SET data = COALESCE(entry_row.data, '{}'::jsonb) || COALESCE(localized.locales_data, '{}'::jsonb)
FROM (
    SELECT
        localized_row.entry_id,
        jsonb_object_agg(localized_row.locale, localized_row.data) AS locales_data
    FROM flex_entry_localized_values AS localized_row
    GROUP BY localized_row.entry_id
) AS localized
WHERE entry_row.id = localized.entry_id
"#,
                )
                .await?;
        }

        manager
            .drop_table(
                Table::drop()
                    .table(FlexEntryLocalizedValues::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum FlexEntryLocalizedValues {
    Table,
    EntryId,
    Locale,
    TenantId,
    Data,
    CreatedAt,
    UpdatedAt,
}
