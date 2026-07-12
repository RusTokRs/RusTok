use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RegistryModuleOwners::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RegistryModuleOwners::Slug)
                            .string_len(96)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(RegistryModuleOwners::OwnerActor)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryModuleOwners::BoundBy)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RegistryModuleOwners::BoundAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(RegistryModuleOwners::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(RegistryModuleOwners::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum RegistryModuleOwners {
    Table,
    Slug,
    OwnerActor,
    BoundBy,
    BoundAt,
    UpdatedAt,
}
