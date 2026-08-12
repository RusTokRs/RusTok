use sea_orm_migration::{
    prelude::*,
    sea_orm::{ConnectionTrait, DbBackend, Statement},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

const TABLE_NAME: &str = "index_consistency_findings";
const TEMP_TABLE_NAME: &str = "index_consistency_findings_locale_scope_v2";
const RELAXED_CONSTRAINT_NAME: &str = "ck_index_consistency_findings_scope_v2";
const STRICT_CONSTRAINT_NAME: &str = "ck_index_consistency_findings_scope_v1";

const RELAXED_SCOPE_CHECK: &str = "(scope_kind = 'global' AND module_name IS NULL AND entity_name IS NULL AND schema_version IS NULL AND entity_id IS NULL AND locale_key IS NULL) OR (scope_kind = 'schema' AND module_name IS NOT NULL AND entity_name IS NOT NULL AND schema_version IS NOT NULL AND entity_id IS NULL AND locale_key IS NULL) OR (scope_kind = 'entity' AND module_name IS NOT NULL AND entity_name IS NOT NULL AND schema_version IS NOT NULL AND entity_id IS NOT NULL)";
const STRICT_SCOPE_CHECK: &str = "(scope_kind = 'global' AND module_name IS NULL AND entity_name IS NULL AND schema_version IS NULL AND entity_id IS NULL AND locale_key IS NULL) OR (scope_kind = 'schema' AND module_name IS NOT NULL AND entity_name IS NOT NULL AND schema_version IS NOT NULL AND entity_id IS NULL AND locale_key IS NULL) OR (scope_kind = 'entity' AND module_name IS NOT NULL AND entity_name IS NOT NULL AND schema_version IS NOT NULL AND entity_id IS NOT NULL AND locale_key IS NOT NULL)";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_connection().get_database_backend() {
            DbBackend::Postgres => {
                replace_postgres_scope_constraint(
                    manager,
                    RELAXED_CONSTRAINT_NAME,
                    RELAXED_SCOPE_CHECK,
                )
                .await
            }
            DbBackend::Sqlite => rebuild_sqlite_table(manager, RELAXED_SCOPE_CHECK).await,
            DbBackend::MySql => Err(DbErr::Custom(
                "rustok-index locale-optional finding migration supports PostgreSQL and SQLite"
                    .to_owned(),
            )),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_connection().get_database_backend() {
            DbBackend::Postgres => {
                replace_postgres_scope_constraint(
                    manager,
                    STRICT_CONSTRAINT_NAME,
                    STRICT_SCOPE_CHECK,
                )
                .await
            }
            DbBackend::Sqlite => rebuild_sqlite_table(manager, STRICT_SCOPE_CHECK).await,
            DbBackend::MySql => Err(DbErr::Custom(
                "rustok-index locale-optional finding migration supports PostgreSQL and SQLite"
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
        .query_all(Statement::from_string(
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

fn quote_postgres_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

async fn rebuild_sqlite_table(manager: &SchemaManager<'_>, scope_check: &str) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    connection
        .execute_unprepared(&format!(
            "CREATE TABLE {TEMP_TABLE_NAME} (tenant_id TEXT NOT NULL, finding_id TEXT NOT NULL, finding_key VARCHAR(64) NOT NULL, check_name VARCHAR(128) NOT NULL, severity VARCHAR(16) NOT NULL, state VARCHAR(16) NOT NULL DEFAULT 'open', scope_kind VARCHAR(16) NOT NULL, module_name VARCHAR(128), entity_name VARCHAR(128), schema_version INTEGER, entity_id TEXT, locale_key VARCHAR(32), expected_digest VARCHAR(64), actual_digest VARCHAR(64), details JSON NOT NULL, first_detected_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, last_detected_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, closed_at TEXT, PRIMARY KEY (tenant_id, finding_id), FOREIGN KEY (tenant_id) REFERENCES tenants(id) ON UPDATE CASCADE ON DELETE CASCADE, CHECK (length(finding_key) = 64 AND finding_key = lower(finding_key)), CHECK (length(check_name) BETWEEN 1 AND 128 AND check_name = trim(check_name)), CHECK (severity IN ('info', 'warning', 'error')), CHECK (state IN ('open', 'resolved', 'ignored')), CHECK (schema_version IS NULL OR schema_version > 0), CHECK (locale_key IS NULL OR (length(locale_key) <= 32 AND locale_key = trim(locale_key))), CHECK (expected_digest IS NULL OR (length(expected_digest) = 64 AND expected_digest = lower(expected_digest))), CHECK (actual_digest IS NULL OR (length(actual_digest) = 64 AND actual_digest = lower(actual_digest))), CHECK ({scope_check}), CHECK ((state = 'open' AND closed_at IS NULL) OR (state IN ('resolved', 'ignored') AND closed_at IS NOT NULL)))"
        ))
        .await?;
    connection
        .execute_unprepared(&format!(
            "INSERT INTO {TEMP_TABLE_NAME} (tenant_id, finding_id, finding_key, check_name, severity, state, scope_kind, module_name, entity_name, schema_version, entity_id, locale_key, expected_digest, actual_digest, details, first_detected_at, last_detected_at, closed_at) SELECT tenant_id, finding_id, finding_key, check_name, severity, state, scope_kind, module_name, entity_name, schema_version, entity_id, locale_key, expected_digest, actual_digest, details, first_detected_at, last_detected_at, closed_at FROM {TABLE_NAME}"
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
            "CREATE UNIQUE INDEX uq_index_consistency_finding_key ON index_consistency_findings (tenant_id, finding_key)",
        )
        .await?;
    connection
        .execute_unprepared(
            "CREATE INDEX idx_index_consistency_open ON index_consistency_findings (tenant_id, state, severity, last_detected_at)",
        )
        .await?;
    Ok(())
}
