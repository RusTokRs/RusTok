use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FlexAttachedLocalizedValues::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FlexAttachedLocalizedValues::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(FlexAttachedLocalizedValues::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FlexAttachedLocalizedValues::EntityType)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FlexAttachedLocalizedValues::EntityId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FlexAttachedLocalizedValues::FieldKey)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FlexAttachedLocalizedValues::Locale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FlexAttachedLocalizedValues::Value)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FlexAttachedLocalizedValues::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(FlexAttachedLocalizedValues::UpdatedAt)
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
                    .name("idx_flex_attached_localized_values_owner")
                    .table(FlexAttachedLocalizedValues::Table)
                    .col(FlexAttachedLocalizedValues::TenantId)
                    .col(FlexAttachedLocalizedValues::EntityType)
                    .col(FlexAttachedLocalizedValues::EntityId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_flex_attached_localized_values_locale")
                    .table(FlexAttachedLocalizedValues::Table)
                    .unique()
                    .col(FlexAttachedLocalizedValues::TenantId)
                    .col(FlexAttachedLocalizedValues::EntityType)
                    .col(FlexAttachedLocalizedValues::EntityId)
                    .col(FlexAttachedLocalizedValues::FieldKey)
                    .col(FlexAttachedLocalizedValues::Locale)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(FlexAttachedLocalizedValues::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum FlexAttachedLocalizedValues {
    Table,
    Id,
    TenantId,
    EntityType,
    EntityId,
    FieldKey,
    Locale,
    Value,
    CreatedAt,
    UpdatedAt,
}
