use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(EventDeliverySettings::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(EventDeliverySettings::Id)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(EventDeliverySettings::Profile)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EventDeliverySettings::UpdatedBy)
                            .uuid()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(EventDeliverySettings::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(EventDeliverySettings::UpdatedAt)
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
            .drop_table(Table::drop().table(EventDeliverySettings::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum EventDeliverySettings {
    Table,
    Id,
    Profile,
    UpdatedBy,
    CreatedAt,
    UpdatedAt,
}
