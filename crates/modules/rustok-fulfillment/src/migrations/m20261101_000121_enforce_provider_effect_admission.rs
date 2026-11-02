//! Forward-only provider admission correction. No rows, keys, payloads or historical
//! migrations are rewritten. Pending means locally proved or operator-confirmed no effect;
//! a retained provider_error is never automatically permission to execute again.
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Kept inside this immutable migration, not imported from mutable runtime state logic.
const ALLOWED_TRANSITIONS: &str = r#"
    OLD.status = NEW.status
    OR (OLD.status = 'pending' AND NEW.status = 'executing'
        AND OLD.provider_result IS NULL AND OLD.provider_reference IS NULL
        AND OLD.provider_completed_at IS NULL AND OLD.committed_at IS NULL)
    OR (OLD.status = 'executing' AND NEW.status = 'pending'
        AND OLD.provider_result IS NULL AND OLD.provider_reference IS NULL
        AND OLD.provider_completed_at IS NULL AND OLD.committed_at IS NULL)
    OR (OLD.status = 'executing'
        AND NEW.status IN ('provider_succeeded', 'provider_error', 'reconciliation_required'))
    OR (OLD.status = 'provider_succeeded'
        AND NEW.status IN ('reconciliation_required', 'committed'))
    OR (OLD.status = 'reconciliation_required' AND NEW.status = 'committed')
    OR (OLD.status IN ('reconciliation_required', 'provider_error')
        AND OLD.provider_result IS NULL AND OLD.provider_reference IS NULL
        AND OLD.committed_at IS NULL AND NEW.status = 'pending')
    OR (OLD.status IN ('reconciliation_required', 'provider_error')
        AND OLD.committed_at IS NULL AND NEW.status = 'provider_succeeded')
"#;

const CONSISTENT_STATE: &str = r#"
    (NEW.status IN ('pending', 'executing')
        AND NEW.provider_result IS NULL AND NEW.provider_reference IS NULL
        AND NEW.provider_completed_at IS NULL AND NEW.committed_at IS NULL)
    OR (NEW.status = 'provider_error'
        AND NEW.error_message IS NOT NULL AND NEW.committed_at IS NULL)
    OR (NEW.status = 'provider_succeeded'
        AND NEW.provider_result IS NOT NULL AND NEW.provider_completed_at IS NOT NULL
        AND NEW.committed_at IS NULL)
    OR (NEW.status = 'reconciliation_required'
        AND NEW.provider_completed_at IS NOT NULL AND NEW.committed_at IS NULL)
    OR (NEW.status = 'committed'
        AND NEW.provider_result IS NOT NULL AND NEW.provider_completed_at IS NOT NULL
        AND NEW.committed_at IS NOT NULL)
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = match manager.get_database_backend() {
            DatabaseBackend::Postgres => postgres_sql(),
            DatabaseBackend::Sqlite => sqlite_sql(),
            _ => return Err(DbErr::Custom("provider effect admission requires PostgreSQL or SQLite; unsupported backends must not install a permissive guard".into())),
        };
        manager.get_connection().execute_unprepared(&sql).await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Custom("provider effect admission is forward-only; rollback must not reopen legacy ambiguous provider_error execution".into()))
    }
}

fn identity_changed(comparison: &str) -> String {
    [
        "id",
        "tenant_id",
        "fulfillment_id",
        "operation",
        "provider_id",
        "idempotency_key",
        "request_payload",
    ]
    .into_iter()
    .map(|field| format!("NEW.{field} {comparison} OLD.{field}"))
    .collect::<Vec<_>>()
    .join(" OR ")
}

fn postgres_sql() -> String {
    let identity = identity_changed("IS DISTINCT FROM");
    format!(
        r#"
        CREATE OR REPLACE FUNCTION enforce_fulfillment_provider_operation_lifecycle()
        RETURNS trigger AS $$
        BEGIN
            IF {identity} THEN
                RAISE EXCEPTION 'fulfillment provider operation identity is immutable' USING ERRCODE = '23514';
            END IF;
            IF NOT ({ALLOWED_TRANSITIONS}) THEN
                RAISE EXCEPTION 'invalid fulfillment provider effect admission transition' USING ERRCODE = '23514';
            END IF;
            IF NOT ({CONSISTENT_STATE}) THEN
                RAISE EXCEPTION 'provider effect admission cannot erase or invent outcome evidence' USING ERRCODE = '23514';
            END IF;
            IF OLD.provider_reference IS NOT NULL
               AND NEW.provider_reference IS DISTINCT FROM OLD.provider_reference THEN
                RAISE EXCEPTION 'retained provider reference cannot be replaced or cleared' USING ERRCODE = '23514';
            END IF;
            IF OLD.provider_result IS NOT NULL
               AND NEW.provider_result IS DISTINCT FROM OLD.provider_result THEN
                RAISE EXCEPTION 'retained provider result cannot be replaced or cleared' USING ERRCODE = '23514';
            END IF;
            RETURN NEW;
        END;
        $$ LANGUAGE plpgsql;

        CREATE OR REPLACE FUNCTION enforce_fulfillment_provider_effect_insert()
        RETURNS trigger AS $$
        BEGIN
            IF NOT ({CONSISTENT_STATE}) THEN
                RAISE EXCEPTION 'provider effect admission cannot erase or invent outcome evidence' USING ERRCODE = '23514';
            END IF;
            RETURN NEW;
        END;
        $$ LANGUAGE plpgsql;
        DROP TRIGGER IF EXISTS fulfillment_provider_operations_effect_insert ON fulfillment_provider_operations;
        CREATE TRIGGER fulfillment_provider_operations_effect_insert
            BEFORE INSERT ON fulfillment_provider_operations
            FOR EACH ROW EXECUTE FUNCTION enforce_fulfillment_provider_effect_insert();
    "#
    )
}

fn sqlite_sql() -> String {
    let identity = identity_changed("IS NOT");
    format!(
        r#"
        DROP TRIGGER IF EXISTS fulfillment_provider_operations_state_guard_update;
        CREATE TRIGGER fulfillment_provider_operations_state_guard_update
        BEFORE UPDATE ON fulfillment_provider_operations
        FOR EACH ROW
        BEGIN
            SELECT CASE WHEN {identity}
                THEN RAISE(ABORT, 'fulfillment provider operation identity is immutable') END;
            SELECT CASE WHEN NOT ({ALLOWED_TRANSITIONS})
                THEN RAISE(ABORT, 'invalid fulfillment provider effect admission transition') END;
            SELECT CASE WHEN NOT ({CONSISTENT_STATE})
                THEN RAISE(ABORT, 'provider effect admission cannot erase or invent outcome evidence') END;
            SELECT CASE WHEN OLD.provider_reference IS NOT NULL
                AND NEW.provider_reference IS NOT OLD.provider_reference
                THEN RAISE(ABORT, 'retained provider reference cannot be replaced or cleared') END;
            SELECT CASE WHEN OLD.provider_result IS NOT NULL
                AND NEW.provider_result IS NOT OLD.provider_result
                THEN RAISE(ABORT, 'retained provider result cannot be replaced or cleared') END;
        END;
        DROP TRIGGER IF EXISTS fulfillment_provider_operations_effect_insert;
        CREATE TRIGGER fulfillment_provider_operations_effect_insert
        BEFORE INSERT ON fulfillment_provider_operations
        FOR EACH ROW
        BEGIN
            SELECT CASE WHEN NOT ({CONSISTENT_STATE})
                THEN RAISE(ABORT, 'provider effect admission cannot erase or invent outcome evidence') END;
        END;
    "#
    )
}
