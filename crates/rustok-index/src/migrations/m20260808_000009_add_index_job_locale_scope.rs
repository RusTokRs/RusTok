use sea_orm_migration::{
    prelude::*,
    sea_orm::{ConnectionTrait, DbBackend, Statement},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

const TABLE_NAME: &str = "index_jobs";
const TEMP_TABLE_NAME: &str = "index_jobs_locale_scope_v2";
const LOCALE_SCOPE_CONSTRAINT_NAME: &str = "ck_index_jobs_scope_v2";
const STRICT_SCOPE_CONSTRAINT_NAME: &str = "ck_index_jobs_scope_v1";

const LOCALE_SCOPE_CHECK: &str = "(scope_kind = 'global' AND module_name IS NULL AND entity_name IS NULL AND schema_version IS NULL AND entity_id IS NULL AND locale_key IS NULL) OR (scope_kind = 'schema' AND module_name IS NOT NULL AND entity_name IS NOT NULL AND schema_version IS NOT NULL AND entity_id IS NULL AND locale_key IS NULL) OR (scope_kind = 'locale' AND module_name IS NOT NULL AND entity_name IS NOT NULL AND schema_version IS NOT NULL AND entity_id IS NULL AND locale_key IS NOT NULL AND length(locale_key) BETWEEN 1 AND 32) OR (scope_kind = 'entity' AND module_name IS NOT NULL AND entity_name IS NOT NULL AND schema_version IS NOT NULL AND entity_id IS NOT NULL AND locale_key IS NOT NULL)";
const STRICT_SCOPE_CHECK: &str = "(scope_kind = 'global' AND module_name IS NULL AND entity_name IS NULL AND schema_version IS NULL AND entity_id IS NULL AND locale_key IS NULL) OR (scope_kind = 'schema' AND module_name IS NOT NULL AND entity_name IS NOT NULL AND schema_version IS NOT NULL AND entity_id IS NULL AND locale_key IS NULL) OR (scope_kind = 'entity' AND module_name IS NOT NULL AND entity_name IS NOT NULL AND schema_version IS NOT NULL AND entity_id IS NOT NULL AND locale_key IS NOT NULL)";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_connection().get_database_backend() {
            DbBackend::Postgres => {
                replace_postgres_scope_constraint(
                    manager,
                    LOCALE_SCOPE_CONSTRAINT_NAME,
                    LOCALE_SCOPE_CHECK,
                )
                .await?;
                replace_postgres_scope_index(manager, true).await
            }
            DbBackend::Sqlite => rebuild_sqlite_table(manager, LOCALE_SCOPE_CHECK, true).await,
            _ => Err(DbErr::Custom(
                "rustok-index replay locale job scope migration supports PostgreSQL and SQLite"
                    .to_owned(),
            )),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_connection().get_database_backend() {
            DbBackend::Postgres => {
                replace_postgres_scope_constraint(
                    manager,
                    STRICT_SCOPE_CONSTRAINT_NAME,
                    STRICT_SCOPE_CHECK,
                )
                .await?;
                replace_postgres_scope_index(manager, false).await
            }
            DbBackend::Sqlite => rebuild_sqlite_table(manager, STRICT_SCOPE_CHECK, false).await,
            _ => Err(DbErr::Custom(
                "rustok-index replay locale job scope migration supports PostgreSQL and SQLite"
                    .to_owned(),
            )),
        }
    }
}

async fn replace_postgres_scope_constraint(
    manager: &SchemaManager<'_>,
    replacement_name: &str,
    replacement_check: &str,
) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    let rows = connection
        .query_all_raw(Statement::from_string(
            DbBackend::Postgres,
            format!(
                "SELECT c.conname FROM pg_constraint c JOIN pg_class t ON t.oid = c.conrelid JOIN pg_namespace n ON n.oid = t.relnamespace WHERE c.contype = 'c' AND n.nspname = current_schema() AND t.relname = '{TABLE_NAME}' AND pg_get_constraintdef(c.oid) LIKE '%scope_kind%' AND pg_get_constraintdef(c.oid) LIKE '%entity_id%' AND pg_get_constraintdef(c.oid) LIKE '%locale_key%' ORDER BY c.conname"
            ),
        ))
        .await?;
    if rows.len() != 1 {
        return Err(DbErr::Custom(format!(
            "expected exactly one {TABLE_NAME} scope constraint, found {}",
            rows.len()
        )));
    }
    let constraint_name: String = rows[0].try_get("", "conname")?;
    let quoted_constraint = quote_postgres_identifier(&constraint_name);
    let quoted_replacement = quote_postgres_identifier(replacement_name);
    connection
        .execute_unprepared(&format!(
            "ALTER TABLE {TABLE_NAME} DROP CONSTRAINT {quoted_constraint}"
        ))
        .await?;
    connection
        .execute_unprepared(&format!(
            "ALTER TABLE {TABLE_NAME} ADD CONSTRAINT {quoted_replacement} CHECK ({replacement_check})"
        ))
        .await?;
    Ok(())
}

async fn replace_postgres_scope_index(
    manager: &SchemaManager<'_>,
    include_locale: bool,
) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    connection
        .execute_unprepared("DROP INDEX IF EXISTS idx_index_jobs_scope")
        .await?;
    let locale = if include_locale { ", locale_key" } else { "" };
    connection
        .execute_unprepared(&format!(
            "CREATE INDEX idx_index_jobs_scope ON index_jobs (tenant_id, scope_kind, module_name, entity_name, schema_version{locale}, state)"
        ))
        .await?;
    Ok(())
}

fn quote_postgres_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

async fn rebuild_sqlite_table(
    manager: &SchemaManager<'_>,
    scope_check: &str,
    include_locale_in_scope_index: bool,
) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    connection
        .execute_unprepared(&format!(
            "CREATE TABLE {TEMP_TABLE_NAME} (tenant_id TEXT NOT NULL, job_id TEXT NOT NULL, kind VARCHAR(32) NOT NULL, state VARCHAR(16) NOT NULL DEFAULT 'pending', scope_kind VARCHAR(16) NOT NULL, module_name VARCHAR(128), entity_name VARCHAR(128), schema_version INTEGER, entity_id TEXT, locale_key VARCHAR(32), request JSON NOT NULL, cursor JSON, attempt_count INTEGER NOT NULL DEFAULT 0, retry_epoch INTEGER NOT NULL DEFAULT 0 CHECK (retry_epoch >= 0), available_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, lease_owner VARCHAR(191), lease_expires_at TEXT, heartbeat_at TEXT, cancel_requested BOOLEAN NOT NULL DEFAULT FALSE, last_error_code VARCHAR(128), last_error_details JSON, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, completed_at TEXT, PRIMARY KEY (tenant_id, job_id), FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON UPDATE CASCADE ON DELETE CASCADE, CHECK (kind IN ('schema_apply', 'secondary_index', 'rebuild', 'reconcile', 'consistency_check')), CHECK (state IN ('pending', 'running', 'succeeded', 'failed', 'cancelled')), CHECK (attempt_count >= 0), CHECK (schema_version IS NULL OR schema_version > 0), CHECK (locale_key IS NULL OR (length(locale_key) <= 32 AND locale_key = trim(locale_key))), CHECK ({scope_check}), CHECK ((state = 'running' AND lease_owner IS NOT NULL AND lease_expires_at IS NOT NULL) OR (state <> 'running' AND lease_owner IS NULL AND lease_expires_at IS NULL)), CHECK ((state IN ('succeeded', 'failed', 'cancelled') AND completed_at IS NOT NULL) OR (state IN ('pending', 'running') AND completed_at IS NULL)))"
        ))
        .await?;
    connection
        .execute_unprepared(&format!(
            "INSERT INTO {TEMP_TABLE_NAME} (tenant_id, job_id, kind, state, scope_kind, module_name, entity_name, schema_version, entity_id, locale_key, request, cursor, attempt_count, retry_epoch, available_at, lease_owner, lease_expires_at, heartbeat_at, cancel_requested, last_error_code, last_error_details, created_at, updated_at, completed_at) SELECT tenant_id, job_id, kind, state, scope_kind, module_name, entity_name, schema_version, entity_id, locale_key, request, cursor, attempt_count, retry_epoch, available_at, lease_owner, lease_expires_at, heartbeat_at, cancel_requested, last_error_code, last_error_details, created_at, updated_at, completed_at FROM {TABLE_NAME}"
        ))
        .await?;
    connection
        .execute_unprepared(&format!("DROP TABLE {TABLE_NAME}"))
        .await?;
    connection
        .execute_unprepared(&format!(
            "ALTER TABLE {TEMP_TABLE_NAME} RENAME TO {TABLE_NAME}"
        ))
        .await?;
    connection
        .execute_unprepared(
            "CREATE INDEX idx_index_jobs_claim ON index_jobs (tenant_id, state, available_at, lease_expires_at)",
        )
        .await?;
    let locale = if include_locale_in_scope_index {
        ", locale_key"
    } else {
        ""
    };
    connection
        .execute_unprepared(&format!(
            "CREATE INDEX idx_index_jobs_scope ON index_jobs (tenant_id, scope_kind, module_name, entity_name, schema_version{locale}, state)"
        ))
        .await?;
    Ok(())
}
