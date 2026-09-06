use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ExchangeJobs::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ExchangeJobs::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ExchangeJobs::TenantId).uuid().not_null())
                    .col(ColumnDef::new(ExchangeJobs::JobId).uuid().not_null())
                    .col(ColumnDef::new(ExchangeJobs::Direction).string_len(16).not_null())
                    .col(ColumnDef::new(ExchangeJobs::Status).string_len(16).not_null())
                    .col(
                        ColumnDef::new(ExchangeJobs::ObjectKey)
                            .string_len(512)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExchangeJobs::ContentLength)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ExchangeJobs::ChecksumSha256)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExchangeJobs::CreatedByActorKind)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExchangeJobs::CreatedByActorId)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExchangeJobs::IdempotencyKey)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExchangeJobs::RequestHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExchangeJobs::ProcessingIdempotencyKey).string_len(191),
                    )
                    .col(
                        ColumnDef::new(ExchangeJobs::ProcessingRequestHash).string_len(64),
                    )
                    .col(
                        ColumnDef::new(ExchangeJobs::ProcessedByActorKind).string_len(16),
                    )
                    .col(
                        ColumnDef::new(ExchangeJobs::ProcessedByActorId).string_len(191),
                    )
                    .col(ColumnDef::new(ExchangeJobs::ProcessingLeaseToken).uuid())
                    .col(
                        ColumnDef::new(ExchangeJobs::ProcessingLeaseExpiresAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(ColumnDef::new(ExchangeJobs::ProcessedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(ExchangeJobs::Report).json_binary().not_null())
                    .col(
                        ColumnDef::new(ExchangeJobs::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExchangeJobs::StorageDeletedAt).timestamp_with_time_zone(),
                    )
                    .col(
                        ColumnDef::new(ExchangeJobs::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ExchangeJobs::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_translation_exchange_jobs_tenant_job")
                            .from(ExchangeJobs::Table, ExchangeJobs::TenantId)
                            .from_col(ExchangeJobs::JobId)
                            .to(Jobs::Table, Jobs::TenantId)
                            .to_col(Jobs::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::cust("direction IN ('export', 'import')"))
                    .check(Expr::cust(
                        "status IN ('writing', 'ready', 'processing', 'completed', 'failed', 'expired')",
                    ))
                    .check(Expr::cust("content_length >= 0"))
                    .check(Expr::cust(
                        "created_by_actor_kind IN ('user', 'service', 'system')",
                    ))
                    .check(Expr::cust(
                        "(processing_idempotency_key IS NULL AND processing_request_hash IS NULL AND processed_by_actor_kind IS NULL AND processed_by_actor_id IS NULL AND processing_lease_token IS NULL AND processing_lease_expires_at IS NULL AND processed_at IS NULL) OR (processing_idempotency_key IS NOT NULL AND processing_request_hash IS NOT NULL AND processed_by_actor_kind IN ('user', 'service', 'system') AND processed_by_actor_id IS NOT NULL AND ((processing_lease_token IS NULL AND processing_lease_expires_at IS NULL) OR (processing_lease_token IS NOT NULL AND processing_lease_expires_at IS NOT NULL)))",
                    ))
                    .check(Expr::cust(
                        "(status = 'processing' AND processing_lease_token IS NOT NULL AND processing_lease_expires_at IS NOT NULL) OR (status <> 'processing' AND processing_lease_token IS NULL AND processing_lease_expires_at IS NULL)",
                    ))
                    .to_owned(),
            )
            .await?;
        for (name, columns) in [
            (
                "uq_translation_exchange_jobs_tenant_id",
                vec![ExchangeJobs::TenantId, ExchangeJobs::Id],
            ),
            (
                "uq_translation_exchange_jobs_idempotency",
                vec![ExchangeJobs::TenantId, ExchangeJobs::IdempotencyKey],
            ),
            (
                "uq_translation_exchange_jobs_processing_idempotency",
                vec![
                    ExchangeJobs::TenantId,
                    ExchangeJobs::ProcessingIdempotencyKey,
                ],
            ),
        ] {
            let mut index = Index::create();
            index.name(name).table(ExchangeJobs::Table);
            for column in columns {
                index.col(column);
            }
            manager.create_index(index.unique().to_owned()).await?;
        }
        manager
            .create_index(
                Index::create()
                    .name("idx_translation_exchange_jobs_tenant_job_created")
                    .table(ExchangeJobs::Table)
                    .col(ExchangeJobs::TenantId)
                    .col(ExchangeJobs::JobId)
                    .col(ExchangeJobs::CreatedAt)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_translation_exchange_jobs_expiry")
                    .table(ExchangeJobs::Table)
                    .col(ExchangeJobs::TenantId)
                    .col(ExchangeJobs::ExpiresAt)
                    .col(ExchangeJobs::StorageDeletedAt)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_translation_exchange_jobs_expiry_cleanup")
                    .table(ExchangeJobs::Table)
                    .col(ExchangeJobs::ExpiresAt)
                    .col(ExchangeJobs::StorageDeletedAt)
                    .col(ExchangeJobs::Id)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only: exchange records are tenant audit evidence.
        Ok(())
    }
}

#[derive(Iden)]
enum ExchangeJobs {
    #[iden = "translation_exchange_jobs"]
    Table,
    Id,
    TenantId,
    JobId,
    Direction,
    Status,
    ObjectKey,
    ContentLength,
    ChecksumSha256,
    CreatedByActorKind,
    CreatedByActorId,
    IdempotencyKey,
    RequestHash,
    ProcessingIdempotencyKey,
    ProcessingRequestHash,
    ProcessedByActorKind,
    ProcessedByActorId,
    ProcessingLeaseToken,
    ProcessingLeaseExpiresAt,
    ProcessedAt,
    Report,
    ExpiresAt,
    StorageDeletedAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Jobs {
    #[iden = "translation_jobs"]
    Table,
    Id,
    TenantId,
}
