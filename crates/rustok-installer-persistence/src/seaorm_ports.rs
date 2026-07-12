use rustok_installer::{
    DatabaseEngine, InstallApplyOptions, InstallDatabasePort, InstallDatabaseReady,
    InstallExecutionError, InstallPersistencePort, InstallPlan, InstallReceipt,
    InstallReceiptRecord, InstallSchemaPort, InstallSessionRecord, InstallState,
};
use rustok_migrations::Migrator;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::MigratorTrait;
use url::Url;
use uuid::Uuid;

use crate::{entities::install_session, InstallerPersistenceService};

const DEFAULT_PG_ADMIN_URL: &str = "postgres://postgres:postgres@localhost:5432/postgres";

/// SeaORM implementation of database, schema and durable installer-state ports.
pub struct SeaOrmInstallerPorts;

#[async_trait::async_trait]
impl InstallDatabasePort for SeaOrmInstallerPorts {
    type Runtime = DatabaseConnection;

    async fn prepare_database(
        &self,
        plan: &InstallPlan,
        database_url: &str,
        options: &InstallApplyOptions,
    ) -> Result<InstallDatabaseReady<Self::Runtime>, InstallExecutionError> {
        let target = parse_database_target(&plan.database.engine, database_url)?;
        let mut created_database = false;
        if plan.database.create_if_missing {
            if plan.database.engine != DatabaseEngine::Postgres {
                return Err(InstallExecutionError::new(
                    "--create-database is only supported for postgres install plans",
                ));
            }
            created_database = ensure_postgres_database(
                options
                    .pg_admin_url
                    .as_deref()
                    .unwrap_or(DEFAULT_PG_ADMIN_URL),
                &target,
            )
            .await?;
        }
        let runtime = Database::connect(database_url)
            .await
            .map_err(database_error)?;
        runtime
            .query_one(Statement::from_string(
                runtime.get_database_backend(),
                "SELECT 1".to_string(),
            ))
            .await
            .map_err(database_error)?;
        Ok(InstallDatabaseReady {
            runtime,
            database_name: target.database_name,
            created_database,
        })
    }
}

#[async_trait::async_trait]
impl InstallSchemaPort<DatabaseConnection> for SeaOrmInstallerPorts {
    async fn apply_schema(
        &self,
        runtime: &DatabaseConnection,
    ) -> Result<(), InstallExecutionError> {
        Migrator::up(runtime, None).await.map_err(database_error)
    }
}

#[async_trait::async_trait]
impl InstallPersistencePort<DatabaseConnection> for SeaOrmInstallerPorts {
    async fn create_session(
        &self,
        runtime: &DatabaseConnection,
        plan: &InstallPlan,
    ) -> Result<InstallSessionRecord, InstallExecutionError> {
        InstallerPersistenceService::new(runtime.clone())
            .create_session(plan, None, None)
            .await
            .map(session_record)
            .map_err(database_error)
    }
    async fn acquire_lock(
        &self,
        runtime: &DatabaseConnection,
        session: InstallSessionRecord,
        owner: &str,
        ttl_secs: i64,
    ) -> Result<InstallSessionRecord, InstallExecutionError> {
        let persistence = InstallerPersistenceService::new(runtime.clone());
        let model = persistence
            .get_session(session.id)
            .await
            .map_err(database_error)?
            .ok_or_else(|| {
                InstallExecutionError::new(format!("install session {} not found", session.id))
            })?;
        persistence
            .acquire_lock(model, owner, chrono::Duration::seconds(ttl_secs.max(1)))
            .await
            .map(session_record)
            .map_err(database_error)
    }
    async fn record_receipt(
        &self,
        runtime: &DatabaseConnection,
        receipt: &InstallReceipt,
    ) -> Result<InstallReceiptRecord, InstallExecutionError> {
        InstallerPersistenceService::new(runtime.clone())
            .record_receipt(receipt)
            .await
            .map(|model| InstallReceiptRecord {
                id: model.id,
                input_checksum: model.input_checksum,
            })
            .map_err(database_error)
    }
    async fn set_state(
        &self,
        runtime: &DatabaseConnection,
        session_id: Uuid,
        state: InstallState,
    ) -> Result<InstallSessionRecord, InstallExecutionError> {
        InstallerPersistenceService::new(runtime.clone())
            .set_state(session_id, state)
            .await
            .map(session_record)
            .map_err(database_error)
    }
    async fn set_tenant_id(
        &self,
        runtime: &DatabaseConnection,
        session_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<InstallSessionRecord, InstallExecutionError> {
        InstallerPersistenceService::new(runtime.clone())
            .set_tenant_id(session_id, tenant_id)
            .await
            .map(session_record)
            .map_err(database_error)
    }
}

fn session_record(model: install_session::Model) -> InstallSessionRecord {
    InstallSessionRecord {
        id: model.id,
        tenant_id: model.tenant_id,
        lock_owner: model.lock_owner,
        lock_expires_at: model.lock_expires_at,
    }
}
fn database_error(error: impl std::fmt::Display) -> InstallExecutionError {
    InstallExecutionError::new(error.to_string())
}

struct DatabaseTarget {
    database_name: Option<String>,
    username: Option<String>,
    password: Option<String>,
}
fn parse_database_target(
    engine: &DatabaseEngine,
    database_url: &str,
) -> Result<DatabaseTarget, InstallExecutionError> {
    if *engine == DatabaseEngine::Sqlite {
        return Ok(DatabaseTarget {
            database_name: None,
            username: None,
            password: None,
        });
    }
    let parsed = Url::parse(database_url).map_err(database_error)?;
    if !matches!(parsed.scheme(), "postgres" | "postgresql") {
        return Err(InstallExecutionError::new(format!(
            "postgres install plan requires postgres URL, got `{}`",
            parsed.scheme()
        )));
    }
    let database_name = parsed
        .path_segments()
        .and_then(|mut s| s.next_back())
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| {
            InstallExecutionError::new("postgres database URL must include a database name")
        })?
        .to_string();
    if parsed.username().trim().is_empty() {
        return Err(InstallExecutionError::new(
            "postgres database URL must include a username",
        ));
    }
    Ok(DatabaseTarget {
        database_name: Some(database_name),
        username: Some(parsed.username().to_string()),
        password: parsed.password().map(ToString::to_string),
    })
}
async fn ensure_postgres_database(
    admin_url: &str,
    target: &DatabaseTarget,
) -> Result<bool, InstallExecutionError> {
    let database_name = target
        .database_name
        .as_deref()
        .ok_or_else(|| InstallExecutionError::new("postgres database name is required"))?;
    let username = target
        .username
        .as_deref()
        .ok_or_else(|| InstallExecutionError::new("postgres username is required"))?;
    let password = target.password.as_deref().unwrap_or_default();
    let admin = Database::connect(admin_url).await.map_err(database_error)?;
    let role_exists = admin
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            format!(
                "SELECT 1 FROM pg_roles WHERE rolname = {}",
                quote_literal(username)
            ),
        ))
        .await
        .map_err(database_error)?
        .is_some();
    if !role_exists {
        admin
            .execute(Statement::from_string(
                DbBackend::Postgres,
                format!(
                    "CREATE ROLE {} LOGIN PASSWORD {}",
                    quote_ident(username),
                    quote_literal(password)
                ),
            ))
            .await
            .map_err(database_error)?;
    }
    let database_exists = admin
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            format!(
                "SELECT 1 FROM pg_database WHERE datname = {}",
                quote_literal(database_name)
            ),
        ))
        .await
        .map_err(database_error)?
        .is_some();
    if database_exists {
        return Ok(false);
    }
    admin
        .execute(Statement::from_string(
            DbBackend::Postgres,
            format!(
                "CREATE DATABASE {} OWNER {}",
                quote_ident(database_name),
                quote_ident(username)
            ),
        ))
        .await
        .map_err(database_error)?;
    Ok(true)
}
fn quote_ident(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
fn quote_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
