use super::shared::*;
use sea_orm_migration::prelude::*;

/// Issuance ledger for invitation tokens.
///
/// Invite JWTs are stateless, so without this table an issued invite
/// could neither be listed nor revoked, and acceptance could not
/// distinguish an administrator-issued token from any other correctly
/// signed token. Acceptance now requires a matching, unrevoked ledger
/// row (looked up by the SHA-256 token digest); consumption remains
/// tracked separately in `auth_invite_consumptions`.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AuthInvites::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AuthInvites::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(AuthInvites::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(AuthInvites::TokenHash)
                            .string_len(64)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(AuthInvites::Email)
                            .string_len(320)
                            .not_null(),
                    )
                    .col(ColumnDef::new(AuthInvites::Role).string_len(64).not_null())
                    .col(ColumnDef::new(AuthInvites::IssuedBy).uuid())
                    .col(
                        ColumnDef::new(AuthInvites::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(AuthInvites::RevokedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(AuthInvites::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_auth_invites_tenant")
                            .from(AuthInvites::Table, AuthInvites::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_auth_invites_issued_by")
                            .from(AuthInvites::Table, AuthInvites::IssuedBy)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_auth_invites_tenant_created")
                    .table(AuthInvites::Table)
                    .col(AuthInvites::TenantId)
                    .col(AuthInvites::CreatedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AuthInvites::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum AuthInvites {
    #[iden = "auth_invites"]
    Table,
    Id,
    TenantId,
    TokenHash,
    Email,
    Role,
    IssuedBy,
    ExpiresAt,
    RevokedAt,
    CreatedAt,
}
