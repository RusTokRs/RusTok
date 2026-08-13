use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        create_executions(manager).await?;
        create_cancellation_intents(manager).await?;
        create_attempts(manager).await?;
        create_budgets(manager).await?;
        create_provider_policies(manager).await?;
        create_reservations(manager).await?;
        create_results(manager).await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "AI structured execution accounting is durable and intentionally irreversible"
                .to_string(),
        ))
    }
}

async fn create_cancellation_intents(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(CancellationIntents::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(CancellationIntents::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(CancellationIntents::TenantId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(CancellationIntents::Owner)
                        .string_len(128)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(CancellationIntents::ExecutionIdempotencyKey)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(CancellationIntents::CancellationIdempotencyKey)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(CancellationIntents::RequestDigest)
                        .string_len(64)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(CancellationIntents::ActorKind)
                        .string_len(16)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(CancellationIntents::ActorId)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(CancellationIntents::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_ai_structured_cancellation_intents_tenant")
                        .from(CancellationIntents::Table, CancellationIntents::TenantId)
                        .to(Tenants::Table, Tenants::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .check(Expr::cust("length(owner) > 0"))
                .check(Expr::cust("length(execution_idempotency_key) > 0"))
                .check(Expr::cust("length(cancellation_idempotency_key) > 0"))
                .check(Expr::cust("actor_kind IN ('user', 'service', 'system')"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uq_ai_structured_cancellation_execution_key")
                .table(CancellationIntents::Table)
                .col(CancellationIntents::TenantId)
                .col(CancellationIntents::Owner)
                .col(CancellationIntents::ExecutionIdempotencyKey)
                .unique()
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uq_ai_structured_cancellation_request_key")
                .table(CancellationIntents::Table)
                .col(CancellationIntents::TenantId)
                .col(CancellationIntents::CancellationIdempotencyKey)
                .unique()
                .to_owned(),
        )
        .await
}

async fn create_executions(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Executions::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(Executions::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(Executions::TenantId).uuid().not_null())
                .col(
                    ColumnDef::new(Executions::Owner)
                        .string_len(128)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Executions::TaskSlug)
                        .string_len(128)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Executions::IdempotencyKey)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Executions::RequestDigest)
                        .string_len(64)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Executions::PromptPolicyDigest)
                        .string_len(64)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Executions::InputSchemaDigest)
                        .string_len(64)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Executions::InputDigest)
                        .string_len(64)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Executions::OutputSchemaDigest)
                        .string_len(64)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Executions::Classification)
                        .string_len(32)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Executions::EvidenceDigest)
                        .string_len(64)
                        .not_null(),
                )
                .col(ColumnDef::new(Executions::InputBytes).big_integer().not_null())
                .col(
                    ColumnDef::new(Executions::MaxOutputBytes)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Executions::MaxAttempts)
                        .integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Executions::Status)
                        .string_len(16)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Executions::ActorKind)
                        .string_len(16)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Executions::ActorId)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Executions::CorrelationId)
                        .string_len(191)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Executions::CausationId)
                        .string_len(191)
                        .null(),
                )
                .col(ColumnDef::new(Executions::Traceparent).string_len(256).null())
                .col(ColumnDef::new(Executions::ErrorCode).string_len(128).null())
                .col(
                    ColumnDef::new(Executions::Retryable)
                        .boolean()
                        .not_null()
                        .default(false),
                )
                .col(ColumnDef::new(Executions::RetryAfterMs).big_integer().null())
                .col(ColumnDef::new(Executions::LeaseToken).uuid().null())
                .col(
                    ColumnDef::new(Executions::LeaseExpiresAt)
                        .timestamp_with_time_zone()
                        .null(),
                )
                .col(
                    ColumnDef::new(Executions::CancelRequestedAt)
                        .timestamp_with_time_zone()
                        .null(),
                )
                .col(
                    ColumnDef::new(Executions::CancelIdempotencyKey)
                        .string_len(191)
                        .null(),
                )
                .col(
                    ColumnDef::new(Executions::CancelRequestDigest)
                        .string_len(64)
                        .null(),
                )
                .col(
                    ColumnDef::new(Executions::CancelActorKind)
                        .string_len(16)
                        .null(),
                )
                .col(
                    ColumnDef::new(Executions::CancelActorId)
                        .string_len(191)
                        .null(),
                )
                .col(
                    ColumnDef::new(Executions::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .col(
                    ColumnDef::new(Executions::StartedAt)
                        .timestamp_with_time_zone()
                        .null(),
                )
                .col(
                    ColumnDef::new(Executions::CompletedAt)
                        .timestamp_with_time_zone()
                        .null(),
                )
                .col(
                    ColumnDef::new(Executions::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_ai_structured_executions_tenant")
                        .from(Executions::Table, Executions::TenantId)
                        .to(Tenants::Table, Tenants::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .check(Expr::cust("length(owner) > 0"))
                .check(Expr::cust("length(task_slug) > 0"))
                .check(Expr::cust("length(idempotency_key) > 0"))
                .check(Expr::cust(
                    "classification IN ('public', 'tenant_private', 'personal', 'sensitive')",
                ))
                .check(Expr::cust(
                    "status IN ('queued', 'running', 'completed', 'failed', 'cancelled')",
                ))
                .check(Expr::cust(
                    "actor_kind IN ('user', 'service', 'system')",
                ))
                .check(Expr::cust("max_output_bytes > 0"))
                .check(Expr::cust("input_bytes > 0"))
                .check(Expr::cust("max_attempts BETWEEN 1 AND 8"))
                .check(Expr::cust(
                    "(status = 'running' AND lease_token IS NOT NULL AND lease_expires_at IS NOT NULL AND started_at IS NOT NULL AND completed_at IS NULL) OR (status <> 'running' AND lease_token IS NULL AND lease_expires_at IS NULL)",
                ))
                .check(Expr::cust(
                    "(status IN ('completed', 'failed', 'cancelled') AND completed_at IS NOT NULL) OR (status IN ('queued', 'running') AND completed_at IS NULL)",
                ))
                .check(Expr::cust(
                    "(retry_after_ms IS NULL OR retry_after_ms >= 0)",
                ))
                .check(Expr::cust(
                    "(cancel_requested_at IS NULL AND cancel_idempotency_key IS NULL AND cancel_request_digest IS NULL AND cancel_actor_kind IS NULL AND cancel_actor_id IS NULL) OR (cancel_requested_at IS NOT NULL AND cancel_idempotency_key IS NOT NULL AND cancel_request_digest IS NOT NULL AND cancel_actor_kind IN ('user', 'service', 'system') AND cancel_actor_id IS NOT NULL)",
                ))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uq_ai_structured_execution_idempotency")
                .table(Executions::Table)
                .col(Executions::TenantId)
                .col(Executions::Owner)
                .col(Executions::IdempotencyKey)
                .unique()
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("idx_ai_structured_execution_recovery")
                .table(Executions::Table)
                .col(Executions::Status)
                .col(Executions::LeaseExpiresAt)
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("idx_ai_structured_execution_tenant_status")
                .table(Executions::Table)
                .col(Executions::TenantId)
                .col(Executions::Status)
                .col(Executions::CreatedAt)
                .to_owned(),
        )
        .await
}

async fn create_attempts(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Attempts::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(Attempts::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(Attempts::TenantId).uuid().not_null())
                .col(ColumnDef::new(Attempts::ExecutionId).uuid().not_null())
                .col(ColumnDef::new(Attempts::Attempt).integer().not_null())
                .col(
                    ColumnDef::new(Attempts::ProviderProfileId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Attempts::ProviderSlug)
                        .string_len(128)
                        .not_null(),
                )
                .col(ColumnDef::new(Attempts::Model).string_len(256).not_null())
                .col(ColumnDef::new(Attempts::Fallback).boolean().not_null())
                .col(
                    ColumnDef::new(Attempts::Status)
                        .string_len(16)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Attempts::PriceSnapshotDigest)
                        .string_len(64)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Attempts::CurrencyCode)
                        .string_len(3)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Attempts::InputCostPerMillionMinor)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Attempts::OutputCostPerMillionMinor)
                        .big_integer()
                        .not_null(),
                )
                .col(ColumnDef::new(Attempts::InputTokens).big_integer().null())
                .col(ColumnDef::new(Attempts::OutputTokens).big_integer().null())
                .col(ColumnDef::new(Attempts::TotalTokens).big_integer().null())
                .col(ColumnDef::new(Attempts::CostMinorUnits).big_integer().null())
                .col(ColumnDef::new(Attempts::ErrorCode).string_len(128).null())
                .col(
                    ColumnDef::new(Attempts::Retryable)
                        .boolean()
                        .not_null()
                        .default(false),
                )
                .col(ColumnDef::new(Attempts::RetryAfterMs).big_integer().null())
                .col(
                    ColumnDef::new(Attempts::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .col(
                    ColumnDef::new(Attempts::StartedAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Attempts::CompletedAt)
                        .timestamp_with_time_zone()
                        .null(),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_ai_structured_attempt_execution")
                        .from(Attempts::Table, Attempts::ExecutionId)
                        .to(Executions::Table, Executions::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_ai_structured_attempt_provider")
                        .from(Attempts::Table, Attempts::ProviderProfileId)
                        .to(ProviderProfiles::Table, ProviderProfiles::Id)
                        .on_delete(ForeignKeyAction::Restrict),
                )
                .check(Expr::cust("attempt BETWEEN 1 AND 8"))
                .check(Expr::cust("length(provider_slug) > 0"))
                .check(Expr::cust("length(model) > 0"))
                .check(Expr::cust(
                    "status IN ('running', 'completed', 'failed', 'cancelled')",
                ))
                .check(Expr::cust(
                    "input_cost_per_million_minor >= 0 AND output_cost_per_million_minor >= 0",
                ))
                .check(Expr::cust(
                    "(status = 'running' AND completed_at IS NULL) OR (status <> 'running' AND completed_at IS NOT NULL)",
                ))
                .check(Expr::cust(
                    "(input_tokens IS NULL AND output_tokens IS NULL AND total_tokens IS NULL AND cost_minor_units IS NULL) OR (input_tokens >= 0 AND output_tokens >= 0 AND total_tokens = input_tokens + output_tokens AND cost_minor_units >= 0)",
                ))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uq_ai_structured_attempt_number")
                .table(Attempts::Table)
                .col(Attempts::ExecutionId)
                .col(Attempts::Attempt)
                .unique()
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("idx_ai_structured_attempt_provider")
                .table(Attempts::Table)
                .col(Attempts::TenantId)
                .col(Attempts::ProviderProfileId)
                .col(Attempts::Status)
                .to_owned(),
        )
        .await
}

async fn create_budgets(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Budgets::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(Budgets::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(Budgets::TenantId).uuid().not_null())
                .col(
                    ColumnDef::new(Budgets::CurrencyCode)
                        .string_len(3)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Budgets::LimitMinorUnits)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Budgets::ReservedMinorUnits)
                        .big_integer()
                        .not_null()
                        .default(0),
                )
                .col(
                    ColumnDef::new(Budgets::CommittedMinorUnits)
                        .big_integer()
                        .not_null()
                        .default(0),
                )
                .col(
                    ColumnDef::new(Budgets::MaxConcurrent)
                        .integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Budgets::InFlight)
                        .integer()
                        .not_null()
                        .default(0),
                )
                .col(
                    ColumnDef::new(Budgets::Revision)
                        .big_integer()
                        .not_null()
                        .default(1),
                )
                .col(
                    ColumnDef::new(Budgets::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .col(
                    ColumnDef::new(Budgets::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_ai_structured_budgets_tenant")
                        .from(Budgets::Table, Budgets::TenantId)
                        .to(Tenants::Table, Tenants::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .check(Expr::cust(
                    "limit_minor_units >= 0 AND reserved_minor_units >= 0 AND committed_minor_units >= 0",
                ))
                .check(Expr::cust(
                    "reserved_minor_units + committed_minor_units <= limit_minor_units",
                ))
                .check(Expr::cust("max_concurrent > 0"))
                .check(Expr::cust(
                    "in_flight >= 0 AND in_flight <= max_concurrent",
                ))
                .check(Expr::cust("revision > 0"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uq_ai_structured_budget_currency")
                .table(Budgets::Table)
                .col(Budgets::TenantId)
                .col(Budgets::CurrencyCode)
                .unique()
                .to_owned(),
        )
        .await
}

async fn create_provider_policies(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(ProviderPolicies::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(ProviderPolicies::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(ProviderPolicies::TenantId).uuid().not_null())
                .col(
                    ColumnDef::new(ProviderPolicies::ProviderProfileId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProviderPolicies::AllowedClassifications)
                        .json()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProviderPolicies::CurrencyCode)
                        .string_len(3)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProviderPolicies::InputCostPerMillionMinor)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProviderPolicies::OutputCostPerMillionMinor)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProviderPolicies::MaxConcurrent)
                        .integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ProviderPolicies::InFlight)
                        .integer()
                        .not_null()
                        .default(0),
                )
                .col(
                    ColumnDef::new(ProviderPolicies::IsActive)
                        .boolean()
                        .not_null()
                        .default(true),
                )
                .col(
                    ColumnDef::new(ProviderPolicies::Revision)
                        .big_integer()
                        .not_null()
                        .default(1),
                )
                .col(
                    ColumnDef::new(ProviderPolicies::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .col(
                    ColumnDef::new(ProviderPolicies::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_ai_structured_policy_provider")
                        .from(ProviderPolicies::Table, ProviderPolicies::ProviderProfileId)
                        .to(ProviderProfiles::Table, ProviderProfiles::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .check(Expr::cust(
                    "input_cost_per_million_minor >= 0 AND output_cost_per_million_minor >= 0",
                ))
                .check(Expr::cust("max_concurrent > 0"))
                .check(Expr::cust("in_flight >= 0 AND in_flight <= max_concurrent"))
                .check(Expr::cust("revision > 0"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uq_ai_structured_provider_policy")
                .table(ProviderPolicies::Table)
                .col(ProviderPolicies::TenantId)
                .col(ProviderPolicies::ProviderProfileId)
                .unique()
                .to_owned(),
        )
        .await
}

async fn create_reservations(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Reservations::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(Reservations::Id)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(ColumnDef::new(Reservations::TenantId).uuid().not_null())
                .col(ColumnDef::new(Reservations::ExecutionId).uuid().not_null())
                .col(ColumnDef::new(Reservations::BudgetId).uuid().not_null())
                .col(
                    ColumnDef::new(Reservations::CurrencyCode)
                        .string_len(3)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Reservations::ReservedMinorUnits)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Reservations::CommittedMinorUnits)
                        .big_integer()
                        .not_null()
                        .default(0),
                )
                .col(
                    ColumnDef::new(Reservations::State)
                        .string_len(16)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Reservations::Revision)
                        .big_integer()
                        .not_null()
                        .default(1),
                )
                .col(
                    ColumnDef::new(Reservations::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .col(
                    ColumnDef::new(Reservations::UpdatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_ai_structured_reservation_execution")
                        .from(Reservations::Table, Reservations::ExecutionId)
                        .to(Executions::Table, Executions::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_ai_structured_reservation_budget")
                        .from(Reservations::Table, Reservations::BudgetId)
                        .to(Budgets::Table, Budgets::Id)
                        .on_delete(ForeignKeyAction::Restrict),
                )
                .check(Expr::cust(
                    "reserved_minor_units >= 0 AND committed_minor_units >= 0 AND committed_minor_units <= reserved_minor_units",
                ))
                .check(Expr::cust(
                    "state IN ('reserved', 'committed', 'released')",
                ))
                .check(Expr::cust("revision > 0"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uq_ai_structured_reservation_execution")
                .table(Reservations::Table)
                .col(Reservations::ExecutionId)
                .unique()
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("idx_ai_structured_reservation_state")
                .table(Reservations::Table)
                .col(Reservations::TenantId)
                .col(Reservations::State)
                .to_owned(),
        )
        .await
}

async fn create_results(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Results::Table)
                .if_not_exists()
                .col(ColumnDef::new(Results::Id).uuid().not_null().primary_key())
                .col(ColumnDef::new(Results::TenantId).uuid().not_null())
                .col(ColumnDef::new(Results::ExecutionId).uuid().not_null())
                .col(
                    ColumnDef::new(Results::RequestDigest)
                        .string_len(64)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Results::OutputDigest)
                        .string_len(64)
                        .not_null(),
                )
                .col(ColumnDef::new(Results::KeyId).string_len(64).not_null())
                .col(ColumnDef::new(Results::Nonce).binary().not_null())
                .col(ColumnDef::new(Results::Ciphertext).binary().not_null())
                .col(
                    ColumnDef::new(Results::PlaintextBytes)
                        .big_integer()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Results::ReplayCount)
                        .big_integer()
                        .not_null()
                        .default(0),
                )
                .col(
                    ColumnDef::new(Results::CreatedAt)
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .col(
                    ColumnDef::new(Results::ExpiresAt)
                        .timestamp_with_time_zone()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(Results::LastReplayedAt)
                        .timestamp_with_time_zone()
                        .null(),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_ai_structured_result_execution")
                        .from(Results::Table, Results::ExecutionId)
                        .to(Executions::Table, Executions::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_ai_structured_result_tenant")
                        .from(Results::Table, Results::TenantId)
                        .to(Tenants::Table, Tenants::Id)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .check(Expr::cust("length(request_digest) = 64"))
                .check(Expr::cust("length(output_digest) = 64"))
                .check(Expr::cust("length(key_id) > 0"))
                .check(Expr::cust("length(nonce) = 12"))
                .check(Expr::cust("length(ciphertext) > 16"))
                .check(Expr::cust("plaintext_bytes > 0"))
                .check(Expr::cust("replay_count >= 0"))
                .check(Expr::cust("expires_at > created_at"))
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("uq_ai_structured_result_execution")
                .table(Results::Table)
                .col(Results::ExecutionId)
                .unique()
                .to_owned(),
        )
        .await?;
    manager
        .create_index(
            Index::create()
                .name("idx_ai_structured_result_expiry")
                .table(Results::Table)
                .col(Results::ExpiresAt)
                .to_owned(),
        )
        .await
}

#[derive(Iden)]
enum CancellationIntents {
    #[iden = "ai_structured_cancellation_intents"]
    Table,
    Id,
    TenantId,
    Owner,
    ExecutionIdempotencyKey,
    CancellationIdempotencyKey,
    RequestDigest,
    ActorKind,
    ActorId,
    CreatedAt,
}

#[derive(Iden)]
enum Executions {
    #[iden = "ai_structured_executions"]
    Table,
    Id,
    TenantId,
    Owner,
    TaskSlug,
    IdempotencyKey,
    RequestDigest,
    PromptPolicyDigest,
    InputSchemaDigest,
    InputDigest,
    OutputSchemaDigest,
    Classification,
    EvidenceDigest,
    InputBytes,
    MaxOutputBytes,
    MaxAttempts,
    Status,
    ActorKind,
    ActorId,
    CorrelationId,
    CausationId,
    Traceparent,
    ErrorCode,
    Retryable,
    RetryAfterMs,
    LeaseToken,
    LeaseExpiresAt,
    CancelRequestedAt,
    CancelIdempotencyKey,
    CancelRequestDigest,
    CancelActorKind,
    CancelActorId,
    CreatedAt,
    StartedAt,
    CompletedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Attempts {
    #[iden = "ai_structured_attempts"]
    Table,
    Id,
    TenantId,
    ExecutionId,
    Attempt,
    ProviderProfileId,
    ProviderSlug,
    Model,
    Fallback,
    Status,
    PriceSnapshotDigest,
    CurrencyCode,
    InputCostPerMillionMinor,
    OutputCostPerMillionMinor,
    InputTokens,
    OutputTokens,
    TotalTokens,
    CostMinorUnits,
    ErrorCode,
    Retryable,
    RetryAfterMs,
    CreatedAt,
    StartedAt,
    CompletedAt,
}

#[derive(Iden)]
enum Budgets {
    #[iden = "ai_structured_budgets"]
    Table,
    Id,
    TenantId,
    CurrencyCode,
    LimitMinorUnits,
    ReservedMinorUnits,
    CommittedMinorUnits,
    MaxConcurrent,
    InFlight,
    Revision,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum ProviderPolicies {
    #[iden = "ai_structured_provider_policies"]
    Table,
    Id,
    TenantId,
    ProviderProfileId,
    AllowedClassifications,
    CurrencyCode,
    InputCostPerMillionMinor,
    OutputCostPerMillionMinor,
    MaxConcurrent,
    InFlight,
    IsActive,
    Revision,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Reservations {
    #[iden = "ai_structured_reservations"]
    Table,
    Id,
    TenantId,
    ExecutionId,
    BudgetId,
    CurrencyCode,
    ReservedMinorUnits,
    CommittedMinorUnits,
    State,
    Revision,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Results {
    #[iden = "ai_structured_results"]
    Table,
    Id,
    TenantId,
    ExecutionId,
    RequestDigest,
    OutputDigest,
    KeyId,
    Nonce,
    Ciphertext,
    PlaintextBytes,
    ReplayCount,
    CreatedAt,
    ExpiresAt,
    LastReplayedAt,
}

#[derive(Iden)]
enum Tenants {
    #[iden = "tenants"]
    Table,
    Id,
}

#[derive(Iden)]
enum ProviderProfiles {
    #[iden = "ai_provider_profiles"]
    Table,
    Id,
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement, TryGetable};

    use super::*;

    #[tokio::test]
    async fn creates_content_free_execution_and_accounting_tables() {
        let database = Database::connect("sqlite::memory:").await.unwrap();
        database
            .execute_unprepared(
                "CREATE TABLE tenants (id UUID PRIMARY KEY); \
                 CREATE TABLE ai_provider_profiles (id UUID PRIMARY KEY)",
            )
            .await
            .unwrap();

        Migration.up(&SchemaManager::new(&database)).await.unwrap();

        let execution_columns = table_columns(&database, "ai_structured_executions").await;
        assert!(execution_columns.contains(&"input_digest".to_string()));
        assert!(execution_columns.contains(&"evidence_digest".to_string()));
        assert!(!execution_columns.contains(&"input_payload".to_string()));
        assert!(!execution_columns.contains(&"output_payload".to_string()));
        assert!(!execution_columns.contains(&"raw_response".to_string()));

        for table in [
            "ai_structured_attempts",
            "ai_structured_budgets",
            "ai_structured_provider_policies",
            "ai_structured_reservations",
            "ai_structured_results",
        ] {
            let count = database
                .query_one(Statement::from_sql_and_values(
                    DbBackend::Sqlite,
                    "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = ?"
                        .to_string(),
                    vec![table.into()],
                ))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(i64::try_get(&count, "", "count").unwrap(), 1);
        }

        let result_columns = table_columns(&database, "ai_structured_results").await;
        assert!(result_columns.contains(&"ciphertext".to_string()));
        assert!(result_columns.contains(&"nonce".to_string()));
        assert!(result_columns.contains(&"key_id".to_string()));
        assert!(result_columns.contains(&"expires_at".to_string()));
        assert!(!result_columns.contains(&"output_payload".to_string()));
        assert!(!result_columns.contains(&"plaintext".to_string()));
        assert!(!result_columns.contains(&"provider_response".to_string()));

        let provider_policy_columns =
            table_columns(&database, "ai_structured_provider_policies").await;
        assert!(provider_policy_columns.contains(&"allowed_classifications".to_string()));
    }

    async fn table_columns(database: &sea_orm::DatabaseConnection, table: &str) -> Vec<String> {
        database
            .query_all(Statement::from_string(
                DbBackend::Sqlite,
                format!("PRAGMA table_info({table})"),
            ))
            .await
            .unwrap()
            .into_iter()
            .map(|row| String::try_get(&row, "", "name").unwrap())
            .collect()
    }
}
