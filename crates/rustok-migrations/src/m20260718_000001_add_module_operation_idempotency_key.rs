use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ModuleOperations::Table)
                    .add_column(
                        ColumnDef::new(ModuleOperations::IdempotencyKey)
                            .uuid()
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_module_operations_tenant_idempotency_key")
                    .table(ModuleOperations::Table)
                    .col(ModuleOperations::TenantId)
                    .col(ModuleOperations::IdempotencyKey)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("uq_module_operations_tenant_idempotency_key")
                    .table(ModuleOperations::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(ModuleOperations::Table)
                    .drop_column(ModuleOperations::IdempotencyKey)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum ModuleOperations {
    Table,
    TenantId,
    IdempotencyKey,
}
