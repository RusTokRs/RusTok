use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Policies::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Policies::TenantId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Policies::RequiredTargetLocales)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Policies::TenantLocalePolicyRevision)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Policies::Revision).big_integer().not_null())
                    .col(
                        ColumnDef::new(Policies::LastIdempotencyKey)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Policies::LastRequestHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Policies::UpdatedByActorKind)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Policies::UpdatedByActorId)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Policies::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_translation_policies_tenant")
                            .from(Policies::Table, Policies::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::cust("tenant_locale_policy_revision >= 0"))
                    .check(Expr::cust("revision > 0"))
                    .check(Expr::cust(
                        "updated_by_actor_kind IN ('user', 'service', 'system')",
                    ))
                    .to_owned(),
            )
            .await?;
        manager
            .create_table(
                Table::create()
                    .table(PolicyReceipts::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PolicyReceipts::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PolicyReceipts::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(PolicyReceipts::IdempotencyKey)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PolicyReceipts::RequestHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PolicyReceipts::RequestedByActorKind)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PolicyReceipts::RequestedByActorId)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PolicyReceipts::ResultingPolicyRevision)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PolicyReceipts::Response)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PolicyReceipts::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_translation_policy_receipts_tenant")
                            .from(PolicyReceipts::Table, PolicyReceipts::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::cust(
                        "requested_by_actor_kind IN ('user', 'service', 'system')",
                    ))
                    .check(Expr::cust("resulting_policy_revision > 0"))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_translation_policy_receipts_idempotency")
                    .table(PolicyReceipts::Table)
                    .col(PolicyReceipts::TenantId)
                    .col(PolicyReceipts::IdempotencyKey)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only: policy receipts are durable operator audit evidence.
        Ok(())
    }
}

#[derive(Iden)]
enum Tenants {
    #[iden = "tenants"]
    Table,
    Id,
}

#[derive(Iden)]
enum Policies {
    #[iden = "translation_policies"]
    Table,
    TenantId,
    RequiredTargetLocales,
    TenantLocalePolicyRevision,
    Revision,
    LastIdempotencyKey,
    LastRequestHash,
    UpdatedByActorKind,
    UpdatedByActorId,
    UpdatedAt,
}

#[derive(Iden)]
enum PolicyReceipts {
    #[iden = "translation_policy_receipts"]
    Table,
    Id,
    TenantId,
    IdempotencyKey,
    RequestHash,
    RequestedByActorKind,
    RequestedByActorId,
    ResultingPolicyRevision,
    Response,
    CreatedAt,
}
