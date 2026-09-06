use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ModuleOperationOverrideStates::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ModuleOperationOverrideStates::OperationId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ModuleOperationOverrideStates::PreviousOverrideEnabled)
                            .boolean()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ModuleOperationOverrideStates::RequestedOverrideEnabled)
                            .boolean()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ModuleOperationOverrideStates::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_module_operation_override_states_operation_id")
                            .from(
                                ModuleOperationOverrideStates::Table,
                                ModuleOperationOverrideStates::OperationId,
                            )
                            .to(ModuleOperations::Table, ModuleOperations::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(ModuleOperationOverrideStates::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum ModuleOperationOverrideStates {
    Table,
    OperationId,
    PreviousOverrideEnabled,
    RequestedOverrideEnabled,
    CreatedAt,
}

#[derive(DeriveIden)]
enum ModuleOperations {
    Table,
    Id,
}
