use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(IndexJobs::Table)
                    .col(ColumnDef::new(IndexJobs::TenantId).uuid().not_null())
                    .col(ColumnDef::new(IndexJobs::JobId).uuid().not_null())
                    .col(
                        ColumnDef::new(IndexJobs::Kind)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexJobs::State)
                            .string_len(16)
                            .not_null()
                            .default("pending"),
                    )
                    .col(
                        ColumnDef::new(IndexJobs::ScopeKind)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(ColumnDef::new(IndexJobs::ModuleName).string_len(128))
                    .col(ColumnDef::new(IndexJobs::EntityName).string_len(128))
                    .col(ColumnDef::new(IndexJobs::SchemaVersion).integer())
                    .col(ColumnDef::new(IndexJobs::EntityId).uuid())
                    .col(ColumnDef::new(IndexJobs::LocaleKey).string_len(32))
                    .col(ColumnDef::new(IndexJobs::Request).json_binary().not_null())
                    .col(ColumnDef::new(IndexJobs::Cursor).json_binary())
                    .col(
                        ColumnDef::new(IndexJobs::AttemptCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(IndexJobs::AvailableAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(IndexJobs::LeaseOwner).string_len(191))
                    .col(ColumnDef::new(IndexJobs::LeaseExpiresAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(IndexJobs::HeartbeatAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(IndexJobs::CancelRequested)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(IndexJobs::LastErrorCode).string_len(128))
                    .col(ColumnDef::new(IndexJobs::LastErrorDetails).json_binary())
                    .col(
                        ColumnDef::new(IndexJobs::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(IndexJobs::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(IndexJobs::CompletedAt).timestamp_with_time_zone())
                    .primary_key(
                        Index::create()
                            .name("pk_index_jobs")
                            .col(IndexJobs::TenantId)
                            .col(IndexJobs::JobId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_index_jobs_tenant")
                            .from(IndexJobs::Table, IndexJobs::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::cust(
                        "kind IN ('schema_apply', 'secondary_index', 'rebuild', 'reconcile', 'consistency_check')",
                    ))
                    .check(Expr::cust(
                        "state IN ('pending', 'running', 'succeeded', 'failed', 'cancelled')",
                    ))
                    .check(Expr::cust("attempt_count >= 0"))
                    .check(Expr::cust(
                        "schema_version IS NULL OR schema_version > 0",
                    ))
                    .check(Expr::cust(
                        "locale_key IS NULL OR (length(locale_key) <= 32 AND locale_key = trim(locale_key))",
                    ))
                    .check(Expr::cust(
                        "(scope_kind = 'global' AND module_name IS NULL AND entity_name IS NULL AND schema_version IS NULL AND entity_id IS NULL AND locale_key IS NULL) OR (scope_kind = 'schema' AND module_name IS NOT NULL AND entity_name IS NOT NULL AND schema_version IS NOT NULL AND entity_id IS NULL AND locale_key IS NULL) OR (scope_kind = 'entity' AND module_name IS NOT NULL AND entity_name IS NOT NULL AND schema_version IS NOT NULL AND entity_id IS NOT NULL AND locale_key IS NOT NULL)",
                    ))
                    .check(Expr::cust(
                        "(state = 'running' AND lease_owner IS NOT NULL AND lease_expires_at IS NOT NULL) OR (state <> 'running' AND lease_owner IS NULL AND lease_expires_at IS NULL)",
                    ))
                    .check(Expr::cust(
                        "(state IN ('succeeded', 'failed', 'cancelled') AND completed_at IS NOT NULL) OR (state IN ('pending', 'running') AND completed_at IS NULL)",
                    ))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_index_jobs_claim")
                    .table(IndexJobs::Table)
                    .col(IndexJobs::TenantId)
                    .col(IndexJobs::State)
                    .col(IndexJobs::AvailableAt)
                    .col(IndexJobs::LeaseExpiresAt)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_index_jobs_scope")
                    .table(IndexJobs::Table)
                    .col(IndexJobs::TenantId)
                    .col(IndexJobs::ScopeKind)
                    .col(IndexJobs::ModuleName)
                    .col(IndexJobs::EntityName)
                    .col(IndexJobs::SchemaVersion)
                    .col(IndexJobs::State)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(IndexConsistencyFindings::Table)
                    .col(
                        ColumnDef::new(IndexConsistencyFindings::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexConsistencyFindings::FindingId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexConsistencyFindings::FindingKey)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexConsistencyFindings::CheckName)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexConsistencyFindings::Severity)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexConsistencyFindings::State)
                            .string_len(16)
                            .not_null()
                            .default("open"),
                    )
                    .col(
                        ColumnDef::new(IndexConsistencyFindings::ScopeKind)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexConsistencyFindings::ModuleName).string_len(128),
                    )
                    .col(
                        ColumnDef::new(IndexConsistencyFindings::EntityName).string_len(128),
                    )
                    .col(ColumnDef::new(IndexConsistencyFindings::SchemaVersion).integer())
                    .col(ColumnDef::new(IndexConsistencyFindings::EntityId).uuid())
                    .col(
                        ColumnDef::new(IndexConsistencyFindings::LocaleKey).string_len(32),
                    )
                    .col(
                        ColumnDef::new(IndexConsistencyFindings::ExpectedDigest).string_len(64),
                    )
                    .col(
                        ColumnDef::new(IndexConsistencyFindings::ActualDigest).string_len(64),
                    )
                    .col(
                        ColumnDef::new(IndexConsistencyFindings::Details)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexConsistencyFindings::FirstDetectedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(IndexConsistencyFindings::LastDetectedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(IndexConsistencyFindings::ClosedAt)
                            .timestamp_with_time_zone(),
                    )
                    .primary_key(
                        Index::create()
                            .name("pk_index_consistency_findings")
                            .col(IndexConsistencyFindings::TenantId)
                            .col(IndexConsistencyFindings::FindingId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_index_consistency_findings_tenant")
                            .from(
                                IndexConsistencyFindings::Table,
                                IndexConsistencyFindings::TenantId,
                            )
                            .to(Tenants::Table, Tenants::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::cust(
                        "length(finding_key) = 64 AND finding_key = lower(finding_key)",
                    ))
                    .check(Expr::cust(
                        "length(check_name) BETWEEN 1 AND 128 AND check_name = trim(check_name)",
                    ))
                    .check(Expr::cust("severity IN ('info', 'warning', 'error')"))
                    .check(Expr::cust("state IN ('open', 'resolved', 'ignored')"))
                    .check(Expr::cust(
                        "schema_version IS NULL OR schema_version > 0",
                    ))
                    .check(Expr::cust(
                        "locale_key IS NULL OR (length(locale_key) <= 32 AND locale_key = trim(locale_key))",
                    ))
                    .check(Expr::cust(
                        "expected_digest IS NULL OR (length(expected_digest) = 64 AND expected_digest = lower(expected_digest))",
                    ))
                    .check(Expr::cust(
                        "actual_digest IS NULL OR (length(actual_digest) = 64 AND actual_digest = lower(actual_digest))",
                    ))
                    .check(Expr::cust(
                        "(scope_kind = 'global' AND module_name IS NULL AND entity_name IS NULL AND schema_version IS NULL AND entity_id IS NULL AND locale_key IS NULL) OR (scope_kind = 'schema' AND module_name IS NOT NULL AND entity_name IS NOT NULL AND schema_version IS NOT NULL AND entity_id IS NULL AND locale_key IS NULL) OR (scope_kind = 'entity' AND module_name IS NOT NULL AND entity_name IS NOT NULL AND schema_version IS NOT NULL AND entity_id IS NOT NULL AND locale_key IS NOT NULL)",
                    ))
                    .check(Expr::cust(
                        "(state = 'open' AND closed_at IS NULL) OR (state IN ('resolved', 'ignored') AND closed_at IS NOT NULL)",
                    ))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_index_consistency_finding_key")
                    .table(IndexConsistencyFindings::Table)
                    .col(IndexConsistencyFindings::TenantId)
                    .col(IndexConsistencyFindings::FindingKey)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_index_consistency_open")
                    .table(IndexConsistencyFindings::Table)
                    .col(IndexConsistencyFindings::TenantId)
                    .col(IndexConsistencyFindings::State)
                    .col(IndexConsistencyFindings::Severity)
                    .col(IndexConsistencyFindings::LastDetectedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(IndexConsistencyFindings::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(IndexJobs::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum IndexJobs {
    Table,
    TenantId,
    JobId,
    Kind,
    State,
    ScopeKind,
    ModuleName,
    EntityName,
    SchemaVersion,
    EntityId,
    LocaleKey,
    Request,
    Cursor,
    AttemptCount,
    AvailableAt,
    LeaseOwner,
    LeaseExpiresAt,
    HeartbeatAt,
    CancelRequested,
    LastErrorCode,
    LastErrorDetails,
    CreatedAt,
    UpdatedAt,
    CompletedAt,
}

#[derive(DeriveIden)]
enum IndexConsistencyFindings {
    Table,
    TenantId,
    FindingId,
    FindingKey,
    CheckName,
    Severity,
    State,
    ScopeKind,
    ModuleName,
    EntityName,
    SchemaVersion,
    EntityId,
    LocaleKey,
    ExpectedDigest,
    ActualDigest,
    Details,
    FirstDetectedAt,
    LastDetectedAt,
    ClosedAt,
}

#[derive(DeriveIden)]
enum Tenants {
    Table,
    Id,
}
