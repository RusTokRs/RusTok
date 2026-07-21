use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MarketplaceReversalAdaptationFailure::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MarketplaceReversalAdaptationFailure::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalAdaptationFailure::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalAdaptationFailure::ProviderEventId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalAdaptationFailure::EventSource)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalAdaptationFailure::EventId)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalAdaptationFailure::EventType)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalAdaptationFailure::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalAdaptationFailure::Retryable)
                            .boolean()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalAdaptationFailure::AttemptCount)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalAdaptationFailure::LastErrorCode)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalAdaptationFailure::LastErrorMessage)
                            .string_len(2000)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalAdaptationFailure::NextRetryAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalAdaptationFailure::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalAdaptationFailure::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalAdaptationFailure::ResolvedAt)
                            .timestamp_with_time_zone(),
                    )
                    .to_owned(),
            )
            .await?;

        for index in [
            Index::create()
                .name("ux_marketplace_reversal_adaptation_provider_event")
                .table(MarketplaceReversalAdaptationFailure::Table)
                .col(MarketplaceReversalAdaptationFailure::TenantId)
                .col(MarketplaceReversalAdaptationFailure::ProviderEventId)
                .unique()
                .to_owned(),
            Index::create()
                .name("idx_marketplace_reversal_adaptation_recovery")
                .table(MarketplaceReversalAdaptationFailure::Table)
                .col(MarketplaceReversalAdaptationFailure::Status)
                .col(MarketplaceReversalAdaptationFailure::NextRetryAt)
                .col(MarketplaceReversalAdaptationFailure::UpdatedAt)
                .to_owned(),
            Index::create()
                .name("idx_marketplace_reversal_adaptation_operator")
                .table(MarketplaceReversalAdaptationFailure::Table)
                .col(MarketplaceReversalAdaptationFailure::TenantId)
                .col(MarketplaceReversalAdaptationFailure::Status)
                .col(MarketplaceReversalAdaptationFailure::UpdatedAt)
                .to_owned(),
        ] {
            manager.create_index(index).await?;
        }

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres_guards(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite_guards(manager).await?,
            DatabaseBackend::MySql => install_mysql_guards(manager).await?,
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        "DROP TRIGGER IF EXISTS marketplace_reversal_adaptation_failure_guard ON marketplace_reversal_adaptation_failures; DROP FUNCTION IF EXISTS enforce_marketplace_reversal_adaptation_failure_integrity();",
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite | DatabaseBackend::MySql => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        "DROP TRIGGER IF EXISTS marketplace_reversal_adaptation_failure_guard_insert;",
                    )
                    .await?;
                manager
                    .get_connection()
                    .execute_unprepared(
                        "DROP TRIGGER IF EXISTS marketplace_reversal_adaptation_failure_guard_update;",
                    )
                    .await?;
            }
        }
        manager
            .drop_table(
                Table::drop()
                    .table(MarketplaceReversalAdaptationFailure::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

async fn install_postgres_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            ALTER TABLE marketplace_reversal_adaptation_failures
                ADD CONSTRAINT ck_marketplace_reversal_adaptation_identity
                CHECK (
                    btrim(event_source) <> ''
                    AND btrim(event_id) <> ''
                    AND btrim(event_type) <> ''
                    AND attempt_count > 0
                    AND btrim(last_error_code) <> ''
                    AND btrim(last_error_message) <> ''
                ),
                ADD CONSTRAINT ck_marketplace_reversal_adaptation_state
                CHECK (
                    (status = 'retryable_error' AND retryable AND next_retry_at IS NOT NULL AND resolved_at IS NULL)
                    OR
                    (status = 'operator_review' AND NOT retryable AND next_retry_at IS NULL AND resolved_at IS NULL)
                    OR
                    (status = 'resolved' AND next_retry_at IS NULL AND resolved_at IS NOT NULL)
                );

            CREATE OR REPLACE FUNCTION enforce_marketplace_reversal_adaptation_failure_integrity()
            RETURNS trigger AS $$
            BEGIN
                IF NEW.id IS DISTINCT FROM OLD.id
                    OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
                    OR NEW.provider_event_id IS DISTINCT FROM OLD.provider_event_id
                    OR NEW.event_source IS DISTINCT FROM OLD.event_source
                    OR NEW.event_id IS DISTINCT FROM OLD.event_id
                    OR NEW.event_type IS DISTINCT FROM OLD.event_type
                    OR NEW.created_at IS DISTINCT FROM OLD.created_at
                THEN
                    RAISE EXCEPTION 'marketplace reversal adaptation identity is immutable'
                        USING ERRCODE = '23514';
                END IF;
                IF OLD.status = 'resolved' AND NEW IS DISTINCT FROM OLD THEN
                    RAISE EXCEPTION 'resolved marketplace reversal adaptation failure is immutable'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER marketplace_reversal_adaptation_failure_guard
            BEFORE UPDATE ON marketplace_reversal_adaptation_failures
            FOR EACH ROW EXECUTE FUNCTION enforce_marketplace_reversal_adaptation_failure_integrity();
            "#,
        )
        .await?;
    Ok(())
}

async fn install_sqlite_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE TRIGGER marketplace_reversal_adaptation_failure_guard_insert
            BEFORE INSERT ON marketplace_reversal_adaptation_failures
            FOR EACH ROW BEGIN
                SELECT CASE WHEN trim(NEW.event_source) = '' OR trim(NEW.event_id) = ''
                    OR trim(NEW.event_type) = '' OR NEW.attempt_count <= 0
                    OR trim(NEW.last_error_code) = '' OR trim(NEW.last_error_message) = ''
                    THEN RAISE(ABORT, 'invalid marketplace reversal adaptation identity') END;
                SELECT CASE WHEN NOT (
                    (NEW.status = 'retryable_error' AND NEW.retryable = 1 AND NEW.next_retry_at IS NOT NULL AND NEW.resolved_at IS NULL)
                    OR
                    (NEW.status = 'operator_review' AND NEW.retryable = 0 AND NEW.next_retry_at IS NULL AND NEW.resolved_at IS NULL)
                    OR
                    (NEW.status = 'resolved' AND NEW.next_retry_at IS NULL AND NEW.resolved_at IS NOT NULL)
                ) THEN RAISE(ABORT, 'invalid marketplace reversal adaptation state') END;
            END;

            CREATE TRIGGER marketplace_reversal_adaptation_failure_guard_update
            BEFORE UPDATE ON marketplace_reversal_adaptation_failures
            FOR EACH ROW BEGIN
                SELECT CASE WHEN NEW.id IS NOT OLD.id
                    OR NEW.tenant_id IS NOT OLD.tenant_id
                    OR NEW.provider_event_id IS NOT OLD.provider_event_id
                    OR NEW.event_source IS NOT OLD.event_source
                    OR NEW.event_id IS NOT OLD.event_id
                    OR NEW.event_type IS NOT OLD.event_type
                    OR NEW.created_at IS NOT OLD.created_at
                    THEN RAISE(ABORT, 'marketplace reversal adaptation identity is immutable') END;
                SELECT CASE WHEN OLD.status = 'resolved'
                    THEN RAISE(ABORT, 'resolved marketplace reversal adaptation failure is immutable') END;
                SELECT CASE WHEN NOT (
                    (NEW.status = 'retryable_error' AND NEW.retryable = 1 AND NEW.next_retry_at IS NOT NULL AND NEW.resolved_at IS NULL)
                    OR
                    (NEW.status = 'operator_review' AND NEW.retryable = 0 AND NEW.next_retry_at IS NULL AND NEW.resolved_at IS NULL)
                    OR
                    (NEW.status = 'resolved' AND NEW.next_retry_at IS NULL AND NEW.resolved_at IS NOT NULL)
                ) THEN RAISE(ABORT, 'invalid marketplace reversal adaptation state') END;
            END;
            "#,
        )
        .await?;
    Ok(())
}

async fn install_mysql_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    for trigger in [
        "marketplace_reversal_adaptation_failure_guard_insert",
        "marketplace_reversal_adaptation_failure_guard_update",
    ] {
        manager
            .get_connection()
            .execute_unprepared(format!("DROP TRIGGER IF EXISTS {trigger};").as_str())
            .await?;
    }
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE TRIGGER marketplace_reversal_adaptation_failure_guard_insert
            BEFORE INSERT ON marketplace_reversal_adaptation_failures
            FOR EACH ROW
            BEGIN
                IF TRIM(NEW.event_source) = '' OR TRIM(NEW.event_id) = ''
                    OR TRIM(NEW.event_type) = '' OR NEW.attempt_count <= 0
                    OR TRIM(NEW.last_error_code) = '' OR TRIM(NEW.last_error_message) = ''
                THEN
                    SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'invalid marketplace reversal adaptation identity';
                END IF;
                IF NOT (
                    (NEW.status = 'retryable_error' AND NEW.retryable = TRUE AND NEW.next_retry_at IS NOT NULL AND NEW.resolved_at IS NULL)
                    OR
                    (NEW.status = 'operator_review' AND NEW.retryable = FALSE AND NEW.next_retry_at IS NULL AND NEW.resolved_at IS NULL)
                    OR
                    (NEW.status = 'resolved' AND NEW.next_retry_at IS NULL AND NEW.resolved_at IS NOT NULL)
                ) THEN
                    SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'invalid marketplace reversal adaptation state';
                END IF;
            END
            "#,
        )
        .await?;
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE TRIGGER marketplace_reversal_adaptation_failure_guard_update
            BEFORE UPDATE ON marketplace_reversal_adaptation_failures
            FOR EACH ROW
            BEGIN
                IF NOT (NEW.id <=> OLD.id)
                    OR NOT (NEW.tenant_id <=> OLD.tenant_id)
                    OR NOT (NEW.provider_event_id <=> OLD.provider_event_id)
                    OR NOT (NEW.event_source <=> OLD.event_source)
                    OR NOT (NEW.event_id <=> OLD.event_id)
                    OR NOT (NEW.event_type <=> OLD.event_type)
                    OR NOT (NEW.created_at <=> OLD.created_at)
                THEN
                    SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'marketplace reversal adaptation identity is immutable';
                END IF;
                IF OLD.status = 'resolved' THEN
                    SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'resolved marketplace reversal adaptation failure is immutable';
                END IF;
                IF NOT (
                    (NEW.status = 'retryable_error' AND NEW.retryable = TRUE AND NEW.next_retry_at IS NOT NULL AND NEW.resolved_at IS NULL)
                    OR
                    (NEW.status = 'operator_review' AND NEW.retryable = FALSE AND NEW.next_retry_at IS NULL AND NEW.resolved_at IS NULL)
                    OR
                    (NEW.status = 'resolved' AND NEW.next_retry_at IS NULL AND NEW.resolved_at IS NOT NULL)
                ) THEN
                    SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'invalid marketplace reversal adaptation state';
                END IF;
            END
            "#,
        )
        .await?;
    Ok(())
}

#[derive(Iden)]
enum MarketplaceReversalAdaptationFailure {
    Table,
    Id,
    TenantId,
    ProviderEventId,
    EventSource,
    EventId,
    EventType,
    Status,
    Retryable,
    AttemptCount,
    LastErrorCode,
    LastErrorMessage,
    NextRetryAt,
    CreatedAt,
    UpdatedAt,
    ResolvedAt,
}
