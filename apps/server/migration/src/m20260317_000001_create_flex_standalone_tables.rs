use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FlexSchemas::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FlexSchemas::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(FlexSchemas::TenantId).uuid().not_null())
                    .col(ColumnDef::new(FlexSchemas::Slug).string_len(64).not_null())
                    .col(ColumnDef::new(FlexSchemas::Name).string_len(255).not_null())
                    .col(ColumnDef::new(FlexSchemas::Description).text().null())
                    .col(
                        ColumnDef::new(FlexSchemas::FieldsConfig)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FlexSchemas::Settings)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(FlexSchemas::IsActive)
                            .boolean()
                            .not_null()
                            .default(true),
                    )
                    .col(
                        ColumnDef::new(FlexSchemas::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(FlexSchemas::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(FlexSchemas::Table, FlexSchemas::TenantId)
                            .to(Alias::new("tenants"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .col(FlexSchemas::TenantId)
                            .col(FlexSchemas::Slug),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(FlexEntries::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FlexEntries::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(FlexEntries::TenantId).uuid().not_null())
                    .col(ColumnDef::new(FlexEntries::SchemaId).uuid().not_null())
                    .col(
                        ColumnDef::new(FlexEntries::EntityType)
                            .string_len(64)
                            .null(),
                    )
                    .col(ColumnDef::new(FlexEntries::EntityId).uuid().null())
                    .col(ColumnDef::new(FlexEntries::Data).json_binary().not_null())
                    .col(
                        ColumnDef::new(FlexEntries::Status)
                            .string_len(32)
                            .not_null()
                            .default("draft"),
                    )
                    .col(
                        ColumnDef::new(FlexEntries::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(FlexEntries::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(FlexEntries::Table, FlexEntries::TenantId)
                            .to(Alias::new("tenants"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(FlexEntries::Table, FlexEntries::SchemaId)
                            .to(FlexSchemas::Table, FlexSchemas::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .index(
                        Index::create()
                            .name("idx_flex_entries_entity")
                            .col(FlexEntries::EntityType)
                            .col(FlexEntries::EntityId),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_flex_entries_data ON flex_entries USING GIN (data)",
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX IF NOT EXISTS uq_flex_entries_attached ON flex_entries (tenant_id, schema_id, entity_type, entity_id) WHERE entity_type IS NOT NULL AND entity_id IS NOT NULL",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX IF EXISTS uq_flex_entries_attached")
            .await?;

        manager
            .get_connection()
            .execute_unprepared("DROP INDEX IF EXISTS idx_flex_entries_data")
            .await?;

        manager
            .drop_table(Table::drop().table(FlexEntries::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(FlexSchemas::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum FlexSchemas {
    Table,
    Id,
    TenantId,
    Slug,
    Name,
    Description,
    FieldsConfig,
    Settings,
    IsActive,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum FlexEntries {
    Table,
    Id,
    TenantId,
    SchemaId,
    EntityType,
    EntityId,
    Data,
    Status,
    CreatedAt,
    UpdatedAt,
}
