use super::shared::*;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ChannelResolutionPolicySets::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ChannelResolutionPolicySets::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ChannelResolutionPolicySets::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ChannelResolutionPolicySets::Slug)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ChannelResolutionPolicySets::Name)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ChannelResolutionPolicySets::SchemaVersion)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(ChannelResolutionPolicySets::IsActive)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(ChannelResolutionPolicySets::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ChannelResolutionPolicySets::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_channel_resolution_policy_sets_tenant_id")
                            .from(
                                ChannelResolutionPolicySets::Table,
                                ChannelResolutionPolicySets::TenantId,
                            )
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_channel_resolution_policy_sets_tenant_slug")
                    .table(ChannelResolutionPolicySets::Table)
                    .col(ChannelResolutionPolicySets::TenantId)
                    .col(ChannelResolutionPolicySets::Slug)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_channel_resolution_policy_sets_tenant_active")
                    .table(ChannelResolutionPolicySets::Table)
                    .col(ChannelResolutionPolicySets::TenantId)
                    .col(ChannelResolutionPolicySets::IsActive)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(ChannelResolutionPolicySets::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum ChannelResolutionPolicySets {
    Table,
    Id,
    TenantId,
    Slug,
    Name,
    SchemaVersion,
    IsActive,
    CreatedAt,
    UpdatedAt,
}
