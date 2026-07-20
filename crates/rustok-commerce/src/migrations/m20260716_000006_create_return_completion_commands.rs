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
                    .table(ReturnCompletionCommands::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ReturnCompletionCommands::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ReturnCompletionCommands::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ReturnCompletionCommands::ReturnId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ReturnCompletionCommands::RequestHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ReturnCompletionCommands::RequestPayload)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ReturnCompletionCommands::RequestedByActorId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ReturnCompletionCommands::RetryCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(ReturnCompletionCommands::LastRetryActorId).uuid())
                    .col(
                        ColumnDef::new(ReturnCompletionCommands::LastRetryAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(
                        ColumnDef::new(ReturnCompletionCommands::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ReturnCompletionCommands::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_return_completion_command_return")
                            .from(
                                ReturnCompletionCommands::Table,
                                ReturnCompletionCommands::ReturnId,
                            )
                            .to(OrderReturns::Table, OrderReturns::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("ux_return_completion_commands_return")
                    .table(ReturnCompletionCommands::Table)
                    .col(ReturnCompletionCommands::TenantId)
                    .col(ReturnCompletionCommands::ReturnId)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_return_completion_commands_retry")
                    .table(ReturnCompletionCommands::Table)
                    .col(ReturnCompletionCommands::TenantId)
                    .col(ReturnCompletionCommands::LastRetryAt)
                    .col(ReturnCompletionCommands::UpdatedAt)
                    .to_owned(),
            )
            .await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres_guards(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite_guards(manager).await?,
            _ => {}
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == DatabaseBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared(
                    "DROP FUNCTION IF EXISTS enforce_return_completion_command_integrity() CASCADE;",
                )
                .await?;
        }
        manager
            .drop_table(
                Table::drop()
                    .table(ReturnCompletionCommands::Table)
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
            ALTER TABLE return_completion_commands
                ADD CONSTRAINT ck_return_completion_commands_request_hash
                CHECK (request_hash ~ '^[0-9a-f]{64}$'),
                ADD CONSTRAINT ck_return_completion_commands_request_payload
                CHECK (
                    jsonb_typeof(request_payload) = 'object'
                    AND request_payload ->> 'version' = '1'
                ),
                ADD CONSTRAINT ck_return_completion_commands_retry_count
                CHECK (retry_count >= 0),
                ADD CONSTRAINT ck_return_completion_commands_retry_audit
                CHECK (
                    (last_retry_actor_id IS NULL AND last_retry_at IS NULL)
                    OR
                    (last_retry_actor_id IS NOT NULL AND last_retry_at IS NOT NULL)
                );

            CREATE OR REPLACE FUNCTION enforce_return_completion_command_integrity()
            RETURNS trigger AS $$
            DECLARE
                referenced_tenant UUID;
            BEGIN
                SELECT tenant_id INTO referenced_tenant
                FROM order_returns WHERE id = NEW.return_id;
                IF referenced_tenant IS NULL OR referenced_tenant <> NEW.tenant_id THEN
                    RAISE EXCEPTION 'return completion command return tenant mismatch'
                        USING ERRCODE = '23514';
                END IF;

                IF TG_OP = 'UPDATE' THEN
                    IF NEW.id IS DISTINCT FROM OLD.id
                        OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
                        OR NEW.return_id IS DISTINCT FROM OLD.return_id
                        OR NEW.request_hash IS DISTINCT FROM OLD.request_hash
                        OR NEW.request_payload IS DISTINCT FROM OLD.request_payload
                        OR NEW.requested_by_actor_id IS DISTINCT FROM OLD.requested_by_actor_id
                        OR NEW.created_at IS DISTINCT FROM OLD.created_at
                    THEN
                        RAISE EXCEPTION 'return completion command identity and payload are immutable'
                            USING ERRCODE = '23514';
                    END IF;
                    IF NEW.retry_count < OLD.retry_count THEN
                        RAISE EXCEPTION 'return completion command retry_count cannot decrease'
                            USING ERRCODE = '23514';
                    END IF;
                    IF OLD.last_retry_at IS NOT NULL
                        AND NEW.last_retry_at IS NOT NULL
                        AND NEW.last_retry_at < OLD.last_retry_at
                    THEN
                        RAISE EXCEPTION 'return completion command retry timestamp cannot move backwards'
                            USING ERRCODE = '23514';
                    END IF;
                END IF;

                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER return_completion_commands_integrity_guard
            BEFORE INSERT OR UPDATE ON return_completion_commands
            FOR EACH ROW
            EXECUTE FUNCTION enforce_return_completion_command_integrity();
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
            CREATE TRIGGER return_completion_commands_guard_insert
            BEFORE INSERT ON return_completion_commands
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN length(NEW.request_hash) <> 64
                    OR NEW.request_hash GLOB '*[^0-9a-f]*'
                    THEN RAISE(ABORT, 'invalid return completion command request hash') END;
                SELECT CASE WHEN json_valid(NEW.request_payload) = 0
                    OR json_type(NEW.request_payload) <> 'object'
                    OR json_extract(NEW.request_payload, '$.version') <> 1
                    THEN RAISE(ABORT, 'invalid return completion command request payload') END;
                SELECT CASE WHEN NEW.retry_count < 0
                    THEN RAISE(ABORT, 'invalid return completion command retry count') END;
                SELECT CASE WHEN NOT (
                    (NEW.last_retry_actor_id IS NULL AND NEW.last_retry_at IS NULL)
                    OR
                    (NEW.last_retry_actor_id IS NOT NULL AND NEW.last_retry_at IS NOT NULL)
                ) THEN RAISE(ABORT, 'invalid return completion command retry audit') END;
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1 FROM order_returns
                    WHERE id = NEW.return_id AND tenant_id = NEW.tenant_id
                ) THEN RAISE(ABORT, 'return completion command return tenant mismatch') END;
            END;

            CREATE TRIGGER return_completion_commands_guard_update
            BEFORE UPDATE ON return_completion_commands
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NEW.id IS NOT OLD.id
                    OR NEW.tenant_id IS NOT OLD.tenant_id
                    OR NEW.return_id IS NOT OLD.return_id
                    OR NEW.request_hash IS NOT OLD.request_hash
                    OR NEW.request_payload IS NOT OLD.request_payload
                    OR NEW.requested_by_actor_id IS NOT OLD.requested_by_actor_id
                    OR NEW.created_at IS NOT OLD.created_at
                    THEN RAISE(ABORT, 'return completion command identity and payload are immutable') END;
                SELECT CASE WHEN NEW.retry_count < OLD.retry_count
                    THEN RAISE(ABORT, 'return completion command retry count cannot decrease') END;
                SELECT CASE WHEN OLD.last_retry_at IS NOT NULL
                    AND NEW.last_retry_at IS NOT NULL
                    AND NEW.last_retry_at < OLD.last_retry_at
                    THEN RAISE(ABORT, 'return completion command retry timestamp cannot move backwards') END;
                SELECT CASE WHEN NEW.retry_count < 0
                    THEN RAISE(ABORT, 'invalid return completion command retry count') END;
                SELECT CASE WHEN NOT (
                    (NEW.last_retry_actor_id IS NULL AND NEW.last_retry_at IS NULL)
                    OR
                    (NEW.last_retry_actor_id IS NOT NULL AND NEW.last_retry_at IS NOT NULL)
                ) THEN RAISE(ABORT, 'invalid return completion command retry audit') END;
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1 FROM order_returns
                    WHERE id = NEW.return_id AND tenant_id = NEW.tenant_id
                ) THEN RAISE(ABORT, 'return completion command return tenant mismatch') END;
            END;
            "#,
        )
        .await?;
    Ok(())
}

#[derive(DeriveIden)]
enum ReturnCompletionCommands {
    Table,
    Id,
    TenantId,
    ReturnId,
    RequestHash,
    RequestPayload,
    RequestedByActorId,
    RetryCount,
    LastRetryActorId,
    LastRetryAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum OrderReturns {
    Table,
    Id,
}
