use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ModuleOperations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ModuleOperations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ModuleOperations::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(ModuleOperations::ModuleSlug)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ModuleOperations::RequestedEnabled)
                            .boolean()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ModuleOperations::PreviousEffectiveEnabled)
                            .boolean()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ModuleOperations::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(ColumnDef::new(ModuleOperations::RequestedBy).string_len(255))
                    .col(ColumnDef::new(ModuleOperations::TraceId).string_len(255))
                    .col(ColumnDef::new(ModuleOperations::CorrelationId).string_len(255))
                    .col(ColumnDef::new(ModuleOperations::IdempotencyKey).uuid())
                    .col(ColumnDef::new(ModuleOperations::ExpectedRevision).big_integer())
                    .col(ColumnDef::new(ModuleOperations::ErrorMessage).text())
                    .col(
                        ColumnDef::new(ModuleOperations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ModuleOperations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_module_operations_tenant_id")
                            .from(ModuleOperations::Table, ModuleOperations::TenantId)
                            .to(Alias::new("tenants"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_module_operations_tenant_module")
                    .table(ModuleOperations::Table)
                    .col(ModuleOperations::TenantId)
                    .col(ModuleOperations::ModuleSlug)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ModuleOperations::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum ModuleOperations {
    Table,
    Id,
    TenantId,
    ModuleSlug,
    RequestedEnabled,
    PreviousEffectiveEnabled,
    Status,
    RequestedBy,
    TraceId,
    CorrelationId,
    IdempotencyKey,
    ExpectedRevision,
    ErrorMessage,
    CreatedAt,
    UpdatedAt,
}
