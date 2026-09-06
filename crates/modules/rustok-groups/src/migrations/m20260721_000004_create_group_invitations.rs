use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(GroupInvitations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(GroupInvitations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(GroupInvitations::TenantId).uuid().not_null())
                    .col(ColumnDef::new(GroupInvitations::GroupId).uuid().not_null())
                    .col(
                        ColumnDef::new(GroupInvitations::InvitedByUserId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(GroupInvitations::TargetUserId).uuid())
                    .col(
                        ColumnDef::new(GroupInvitations::TokenHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GroupInvitations::MaxUses)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GroupInvitations::UseCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(GroupInvitations::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GroupInvitations::RevokedAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(ColumnDef::new(GroupInvitations::RevokedByUserId).uuid())
                    .col(
                        ColumnDef::new(GroupInvitations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(GroupInvitations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .check(Expr::cust("length(token_hash) = 64"))
                    .check(Expr::cust("token_hash = lower(token_hash)"))
                    .check(Expr::cust("max_uses BETWEEN 1 AND 100"))
                    .check(Expr::cust("use_count >= 0 AND use_count <= max_uses"))
                    .check(Expr::cust("target_user_id IS NULL OR max_uses = 1"))
                    .check(Expr::cust("expires_at > created_at"))
                    .check(Expr::cust(
                        "(revoked_at IS NULL AND revoked_by_user_id IS NULL) OR (revoked_at IS NOT NULL AND revoked_by_user_id IS NOT NULL)",
                    ))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_group_invitations_tenant_group")
                            .from(GroupInvitations::Table, GroupInvitations::TenantId)
                            .from_col(GroupInvitations::GroupId)
                            .to(Groups::Table, Groups::TenantId)
                            .to_col(Groups::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        for index in [
            Index::create()
                .name("ux_group_invitations_token_hash")
                .table(GroupInvitations::Table)
                .col(GroupInvitations::TokenHash)
                .unique()
                .to_owned(),
            Index::create()
                .name("ux_group_invitations_tenant_id")
                .table(GroupInvitations::Table)
                .col(GroupInvitations::TenantId)
                .col(GroupInvitations::Id)
                .unique()
                .to_owned(),
            Index::create()
                .name("idx_group_invitations_tenant_group_created")
                .table(GroupInvitations::Table)
                .col(GroupInvitations::TenantId)
                .col(GroupInvitations::GroupId)
                .col(GroupInvitations::CreatedAt)
                .to_owned(),
            Index::create()
                .name("idx_group_invitations_tenant_target_created")
                .table(GroupInvitations::Table)
                .col(GroupInvitations::TenantId)
                .col(GroupInvitations::TargetUserId)
                .col(GroupInvitations::CreatedAt)
                .to_owned(),
        ] {
            manager.create_index(index).await?;
        }

        manager
            .create_table(
                Table::create()
                    .table(GroupInvitationRedemptions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(GroupInvitationRedemptions::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(GroupInvitationRedemptions::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GroupInvitationRedemptions::InvitationId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GroupInvitationRedemptions::GroupId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GroupInvitationRedemptions::UserId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GroupInvitationRedemptions::RedeemedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_group_invitation_redemptions_tenant_invitation")
                            .from(
                                GroupInvitationRedemptions::Table,
                                GroupInvitationRedemptions::TenantId,
                            )
                            .from_col(GroupInvitationRedemptions::InvitationId)
                            .to(GroupInvitations::Table, GroupInvitations::TenantId)
                            .to_col(GroupInvitations::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_group_invitation_redemptions_tenant_group")
                            .from(
                                GroupInvitationRedemptions::Table,
                                GroupInvitationRedemptions::TenantId,
                            )
                            .from_col(GroupInvitationRedemptions::GroupId)
                            .to(Groups::Table, Groups::TenantId)
                            .to_col(Groups::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        for index in [
            Index::create()
                .name("ux_group_invitation_redemptions_tenant_invitation_user")
                .table(GroupInvitationRedemptions::Table)
                .col(GroupInvitationRedemptions::TenantId)
                .col(GroupInvitationRedemptions::InvitationId)
                .col(GroupInvitationRedemptions::UserId)
                .unique()
                .to_owned(),
            Index::create()
                .name("idx_group_invitation_redemptions_tenant_group_redeemed")
                .table(GroupInvitationRedemptions::Table)
                .col(GroupInvitationRedemptions::TenantId)
                .col(GroupInvitationRedemptions::GroupId)
                .col(GroupInvitationRedemptions::RedeemedAt)
                .to_owned(),
        ] {
            manager.create_index(index).await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(GroupInvitationRedemptions::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(GroupInvitations::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Groups {
    Table,
    Id,
    TenantId,
}

#[derive(DeriveIden)]
enum GroupInvitations {
    Table,
    Id,
    TenantId,
    GroupId,
    InvitedByUserId,
    TargetUserId,
    TokenHash,
    MaxUses,
    UseCount,
    ExpiresAt,
    RevokedAt,
    RevokedByUserId,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum GroupInvitationRedemptions {
    Table,
    Id,
    TenantId,
    InvitationId,
    GroupId,
    UserId,
    RedeemedAt,
}
