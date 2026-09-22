use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ModerationApplicationOperations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ModerationApplicationOperations::DecisionId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ModerationApplicationOperations::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ModerationApplicationOperations::CaseId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ModerationApplicationOperations::DecisionHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ModerationApplicationOperations::SubjectModule)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ModerationApplicationOperations::SubjectKind)
                            .string_len(80)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ModerationApplicationOperations::SubjectId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ModerationApplicationOperations::SubjectRevision)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ModerationApplicationOperations::Status)
                            .string_len(32)
                            .not_null()
                            .default("pending"),
                    )
                    .col(
                        ColumnDef::new(ModerationApplicationOperations::AttemptCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ModerationApplicationOperations::NextAttemptAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(ModerationApplicationOperations::LeaseToken).uuid())
                    .col(
                        ColumnDef::new(ModerationApplicationOperations::LeaseOwner)
                            .string_len(120),
                    )
                    .col(
                        ColumnDef::new(ModerationApplicationOperations::LeaseExpiresAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(
                        ColumnDef::new(ModerationApplicationOperations::LastErrorCode)
                            .string_len(120),
                    )
                    .col(ColumnDef::new(ModerationApplicationOperations::LastErrorMessage).text())
                    .col(
                        ColumnDef::new(ModerationApplicationOperations::AppliedRevision)
                            .big_integer(),
                    )
                    .col(
                        ColumnDef::new(ModerationApplicationOperations::AppliedAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(
                        ColumnDef::new(ModerationApplicationOperations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ModerationApplicationOperations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .check(Expr::cust("subject_revision >= 1"))
                    .check(Expr::cust("attempt_count >= 0"))
                    .check(Expr::cust(
                        "status IN ('pending','applying','retryable','applied','rejected','operator_review')",
                    ))
                    .check(Expr::cust(
                        "status <> 'applying' OR (lease_token IS NOT NULL AND lease_owner IS NOT NULL AND lease_expires_at IS NOT NULL)",
                    ))
                    .check(Expr::cust(
                        "applied_revision IS NULL OR applied_revision >= subject_revision",
                    ))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_moderation_application_operations_tenant_decision")
                            .from(
                                ModerationApplicationOperations::Table,
                                ModerationApplicationOperations::TenantId,
                            )
                            .from_col(ModerationApplicationOperations::DecisionId)
                            .to(ModerationDecisions::Table, ModerationDecisions::TenantId)
                            .to_col(ModerationDecisions::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
INSERT INTO moderation_application_operations (
    decision_id,
    tenant_id,
    case_id,
    decision_hash,
    subject_module,
    subject_kind,
    subject_id,
    subject_revision,
    status,
    attempt_count,
    next_attempt_at,
    created_at,
    updated_at
)
SELECT
    d.id,
    d.tenant_id,
    d.case_id,
    d.decision_hash,
    c.subject_module,
    c.subject_kind,
    c.subject_id,
    d.subject_revision,
    'pending',
    0,
    d.created_at,
    d.created_at,
    d.created_at
FROM moderation_decisions d
JOIN moderation_cases c
  ON c.tenant_id = d.tenant_id
 AND c.id = d.case_id
JOIN moderation_decision_effects e
  ON e.tenant_id = d.tenant_id
 AND e.decision_id = d.id
WHERE NOT EXISTS (
    SELECT 1
    FROM moderation_application_operations a
    WHERE a.decision_id = d.id
)
"#,
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_moderation_application_operations_due")
                    .table(ModerationApplicationOperations::Table)
                    .col(ModerationApplicationOperations::TenantId)
                    .col(ModerationApplicationOperations::Status)
                    .col(ModerationApplicationOperations::NextAttemptAt)
                    .col(ModerationApplicationOperations::LeaseExpiresAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_moderation_application_operations_case")
                    .table(ModerationApplicationOperations::Table)
                    .col(ModerationApplicationOperations::TenantId)
                    .col(ModerationApplicationOperations::CaseId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(ModerationApplicationOperations::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum ModerationDecisions {
    Table,
    Id,
    TenantId,
}

#[derive(DeriveIden)]
enum ModerationApplicationOperations {
    Table,
    DecisionId,
    TenantId,
    CaseId,
    DecisionHash,
    SubjectModule,
    SubjectKind,
    SubjectId,
    SubjectRevision,
    Status,
    AttemptCount,
    NextAttemptAt,
    LeaseToken,
    LeaseOwner,
    LeaseExpiresAt,
    LastErrorCode,
    LastErrorMessage,
    AppliedRevision,
    AppliedAt,
    CreatedAt,
    UpdatedAt,
}
