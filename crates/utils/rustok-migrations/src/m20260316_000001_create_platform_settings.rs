use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PlatformSettings::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PlatformSettings::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PlatformSettings::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(PlatformSettings::Category)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlatformSettings::Settings)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(PlatformSettings::SchemaVersion)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    .col(ColumnDef::new(PlatformSettings::UpdatedBy).uuid().null())
                    .col(
                        ColumnDef::new(PlatformSettings::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(PlatformSettings::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(PlatformSettings::Table, PlatformSettings::TenantId)
                            .to(Alias::new("tenants"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .index(
                        Index::create()
                            .unique()
                            .col(PlatformSettings::TenantId)
                            .col(PlatformSettings::Category),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PlatformSettings::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum PlatformSettings {
    Table,
    Id,
    TenantId,
    Category,
    Settings,
    SchemaVersion,
    UpdatedBy,
    CreatedAt,
    UpdatedAt,
}
