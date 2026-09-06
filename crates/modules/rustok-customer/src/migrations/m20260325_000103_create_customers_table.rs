use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Customers::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Customers::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Customers::TenantId).uuid().not_null())
                    .col(ColumnDef::new(Customers::UserId).uuid())
                    .col(ColumnDef::new(Customers::Email).string_len(255).not_null())
                    .col(ColumnDef::new(Customers::FirstName).string_len(100))
                    .col(ColumnDef::new(Customers::LastName).string_len(100))
                    .col(ColumnDef::new(Customers::Phone).string_len(50))
                    .col(ColumnDef::new(Customers::Locale).string_len(32))
                    .col(
                        ColumnDef::new(Customers::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(Customers::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Customers::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_customers_tenant_email")
                    .table(Customers::Table)
                    .col(Customers::TenantId)
                    .col(Customers::Email)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_customers_tenant_user")
                    .table(Customers::Table)
                    .col(Customers::TenantId)
                    .col(Customers::UserId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Customers::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum Customers {
    Table,
    Id,
    TenantId,
    UserId,
    Email,
    FirstName,
    LastName,
    Phone,
    Locale,
    Metadata,
    CreatedAt,
    UpdatedAt,
}
