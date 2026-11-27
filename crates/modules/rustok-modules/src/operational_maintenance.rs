//! Owner-issued serving-maintenance state and transaction-bound continuity.
use std::collections::BTreeSet;

use async_trait::async_trait;
use rustok_api::is_valid_module_slug;
use sea_orm::{ConnectionTrait, DatabaseConnection, DatabaseTransaction, Statement};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{ModuleCommandContext, ModuleEffectivePolicyMaintenanceInput};
use crate::backend::{SupportedBackend, begin_fenced_transaction, set_platform_scope};

const MAX_SCOPES: usize = 1024;
const MAX_STATE_BYTES: usize = 256 * 1024;
const READ_STATE: &str =
    "SELECT revision, scopes FROM module_operational_maintenance_state WHERE id = 'current'";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationalMaintenanceScope {
    /// None means every tenant.
    pub tenant_id: Option<Uuid>,
    /// None means every module in the selected tenant scope.
    pub module_slug: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationalMaintenanceState {
    revision: u64,
    scopes: Vec<OperationalMaintenanceScope>,
}

impl OperationalMaintenanceState {
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn scopes(&self) -> &[OperationalMaintenanceScope] {
        &self.scopes
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplaceOperationalMaintenanceCommand {
    pub expected_revision: u64,
    pub scopes: Vec<OperationalMaintenanceScope>,
    pub reason: String,
    pub context: ModuleCommandContext,
}

#[derive(Debug, Error)]
pub enum OperationalMaintenanceError {
    #[error("Invalid operational maintenance command")]
    InvalidCommand,
    #[error("Operational maintenance authorization denied")]
    Denied,
    #[error("Operational maintenance revision conflict")]
    RevisionConflict,
    #[error("Operational maintenance authority exhausted")]
    Exhausted,
    #[error("Operational maintenance storage: {0}")]
    Store(String),
}

fn store_error(error: impl std::fmt::Display) -> OperationalMaintenanceError {
    OperationalMaintenanceError::Store(error.to_string())
}

/// Host implementations must acquire current credential and RBAC authority on
/// this transaction before the maintenance state lock. No guest or caller
/// supplied permission flag constitutes authorization.
#[async_trait]
pub trait OperationalMaintenanceAuthorizer: Send + Sync {
    async fn authorize_replace(
        &self,
        tx: &DatabaseTransaction,
        command: &ReplaceOperationalMaintenanceCommand,
    ) -> Result<(), OperationalMaintenanceError>;
}

pub struct OperationalMaintenanceService<A> {
    db: DatabaseConnection,
    authorizer: A,
}

impl<A: OperationalMaintenanceAuthorizer> OperationalMaintenanceService<A> {
    pub(crate) fn new(db: DatabaseConnection, authorizer: A) -> Self {
        Self { db, authorizer }
    }

    /// Host-only state read. Do not expose the platform-wide scope list through
    /// tenant transports.
    pub async fn state(&self) -> Result<OperationalMaintenanceState, OperationalMaintenanceError> {
        load_state(&self.db, "").await
    }

    /// Replace the full active scope set using predecessor CAS. Recovery can
    /// clear a gate without needing a serving lease. The authorization and
    /// history are committed atomically with the new state.
    pub async fn replace(
        &self,
        mut command: ReplaceOperationalMaintenanceCommand,
    ) -> Result<OperationalMaintenanceState, OperationalMaintenanceError> {
        if command.expected_revision == 0
            || command.context.tenant_id.is_some()
            || command.context.validate().is_err()
            || command.reason.is_empty()
            || command.reason.len() > 1024
            || command.reason.trim() != command.reason
            || command.reason.chars().any(char::is_control)
        {
            return Err(OperationalMaintenanceError::InvalidCommand);
        }
        command.scopes = normalize_scopes(command.scopes)?;
        let tx = begin_fenced_transaction(&self.db).await.map_err(store_error)?;
        set_platform_scope(&tx).await.map_err(store_error)?;
        self.authorizer.authorize_replace(&tx, &command).await?;
        let backend = SupportedBackend::new(tx.get_database_backend()).map_err(store_error)?;
        reserve_sqlite_writer(&tx, backend).await?;
        let current = load_state(&tx, match backend {
            SupportedBackend::Postgres => " FOR UPDATE",
            SupportedBackend::Sqlite => "",
        }).await?;
        if current.revision != command.expected_revision {
            return Err(OperationalMaintenanceError::RevisionConflict);
        }
        if current.scopes == command.scopes {
            tx.commit().await.map_err(store_error)?;
            return Ok(current);
        }
        let revision = i64::try_from(current.revision).ok()
            .and_then(|revision| revision.checked_add(1))
            .ok_or(OperationalMaintenanceError::Exhausted)?;
        let scopes = serde_json::to_string(&command.scopes).map_err(store_error)?;
        let context = serde_json::to_string(&command.context).map_err(store_error)?;
        let result = tx.execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!("UPDATE module_operational_maintenance_state SET revision = {}, scopes = {} WHERE id = 'current' AND revision = {}",
                backend.placeholder(1), backend.placeholder(2), backend.placeholder(3)),
            vec![revision.into(), scopes.clone().into(), (revision - 1).into()],
        )).await.map_err(store_error)?;
        if result.rows_affected() != 1 {
            return Err(OperationalMaintenanceError::RevisionConflict);
        }
        tx.execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!("INSERT INTO module_operational_maintenance_history (revision, scopes, command_context, reason) VALUES ({}, {}, {}, {})",
                backend.placeholder(1), backend.placeholder(2), backend.placeholder(3), backend.placeholder(4)),
            vec![revision.into(), scopes.into(), context.into(), command.reason.into()],
        )).await.map_err(store_error)?;
        tx.commit().await.map_err(store_error)?;
        Ok(OperationalMaintenanceState { revision: revision as u64, scopes: command.scopes })
    }
}

fn normalize_scopes(
    mut scopes: Vec<OperationalMaintenanceScope>,
) -> Result<Vec<OperationalMaintenanceScope>, OperationalMaintenanceError> {
    if scopes.len() > MAX_SCOPES || scopes.iter().any(|scope|
        scope.tenant_id.is_some_and(|id| id.is_nil()) ||
        scope.module_slug.as_ref().is_some_and(|slug| !is_valid_module_slug(slug))
    ) {
        return Err(OperationalMaintenanceError::InvalidCommand);
    }
    scopes.sort();
    scopes.dedup();
    if serde_json::to_string(&scopes).map_err(store_error)?.len() > MAX_STATE_BYTES {
        return Err(OperationalMaintenanceError::InvalidCommand);
    }
    Ok(scopes)
}

async fn load_state(
    db: &impl ConnectionTrait,
    lock: &str,
) -> Result<OperationalMaintenanceState, OperationalMaintenanceError> {
    let row = db.query_one_raw(Statement::from_string(
        db.get_database_backend(), format!("{READ_STATE}{lock}"),
    )).await.map_err(store_error)?
        .ok_or_else(|| store_error("maintenance state missing"))?;
    let revision: i64 = row.try_get("", "revision").map_err(store_error)?;
    let raw: String = row.try_get("", "scopes").map_err(store_error)?;
    if revision <= 0 || raw.len() > MAX_STATE_BYTES {
        return Err(store_error("maintenance state invalid"));
    }
    let scopes: Vec<OperationalMaintenanceScope> = serde_json::from_str(&raw).map_err(store_error)?;
    if normalize_scopes(scopes.clone())? != scopes {
        return Err(store_error("maintenance scopes are not canonical"));
    }
    Ok(OperationalMaintenanceState { revision: revision as u64, scopes })
}

async fn reserve_sqlite_writer(
    tx: &DatabaseTransaction,
    backend: SupportedBackend,
) -> Result<(), OperationalMaintenanceError> {
    if backend == SupportedBackend::Sqlite {
        let result = tx.execute_unprepared(
            "UPDATE module_operational_maintenance_state SET revision = revision WHERE id = 'current'",
        ).await.map_err(store_error)?;
        if result.rows_affected() != 1 {
            return Err(store_error("maintenance state missing"));
        }
    }
    Ok(())
}

/// Host-only proof of an exact global revision and its tenant projection.
/// Even an inactive projection retains the revision, preventing on/off ABA.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationalMaintenanceAuthority {
    tenant_id: Uuid,
    revision: u64,
    input: ModuleEffectivePolicyMaintenanceInput,
}

impl OperationalMaintenanceAuthority {
    pub async fn capture(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<Self, OperationalMaintenanceError> {
        if tenant_id.is_nil() {
            return Err(OperationalMaintenanceError::InvalidCommand);
        }
        let state = load_state(db, "").await?;
        Ok(Self::from_state(tenant_id, &state))
    }

    fn from_state(tenant_id: Uuid, state: &OperationalMaintenanceState) -> Self {
        let mut all = false;
        let mut affected = BTreeSet::new();
        for scope in &state.scopes {
            if scope.tenant_id.is_some_and(|id| id != tenant_id) {
                continue;
            }
            match &scope.module_slug {
                Some(slug) => { affected.insert(slug.clone()); }
                None => all = true,
            }
        }
        let mut hash = Sha256::new();
        hash.update(b"rustok:operational-maintenance:v1");
        hash.update(tenant_id.as_bytes());
        hash.update(state.revision.to_be_bytes());
        let input = ModuleEffectivePolicyMaintenanceInput {
            maintenance_revision: format!("sha256:{}", hex::encode(hash.finalize())),
            active: all || !affected.is_empty(),
            reason_code: "operational_maintenance".into(),
            affected_modules: if all { None } else { Some(affected.into_iter().collect()) },
        };
        Self { tenant_id, revision: state.revision, input }
    }

    pub fn input(&self) -> &ModuleEffectivePolicyMaintenanceInput {
        &self.input
    }

    pub async fn is_current(&self, db: &DatabaseConnection) -> Result<bool, OperationalMaintenanceError> {
        Ok(Self::capture(db, self.tenant_id).await? == *self)
    }

    /// Retain owner authority until the protected SQL effect commits. The
    /// writer takes this same row exclusively; no second pool checkout.
    pub async fn require_current_on(
        &self,
        tx: &DatabaseTransaction,
    ) -> Result<(), OperationalMaintenanceError> {
        let backend = SupportedBackend::new(tx.get_database_backend()).map_err(store_error)?;
        reserve_sqlite_writer(tx, backend).await?;
        let current = load_state(tx, match backend {
            SupportedBackend::Postgres => " FOR SHARE",
            SupportedBackend::Sqlite => "",
        }).await?;
        if Self::from_state(self.tenant_id, &current) != *self {
            return Err(OperationalMaintenanceError::RevisionConflict);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Database;
    use sea_orm_migration::prelude::{MigrationTrait, SchemaManager};

    async fn fixture() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        crate::migrations::m20261127_000097_operational_maintenance_authority::Migration
            .up(&SchemaManager::new(&db)).await.unwrap();
        db
    }

    #[tokio::test]
    async fn operational_maintenance_authority_rejects_aba_and_missing_state() {
        let db = fixture().await;
        let tenant = Uuid::new_v4();
        let original = OperationalMaintenanceAuthority::capture(&db, tenant).await.unwrap();
        assert!(!original.input().active);
        db.execute_unprepared("UPDATE module_operational_maintenance_state SET revision = 2, scopes = '[{\"tenant_id\":null,\"module_slug\":null}]'").await.unwrap();
        let blocked = OperationalMaintenanceAuthority::capture(&db, tenant).await.unwrap();
        assert!(blocked.input().active);
        assert!(blocked.input().affected_modules.is_none());
        db.execute_unprepared("UPDATE module_operational_maintenance_state SET revision = 3, scopes = '[]'").await.unwrap();
        assert!(!original.is_current(&db).await.unwrap());
        let tx = begin_fenced_transaction(&db).await.unwrap();
        assert!(original.require_current_on(&tx).await.is_err());
        tx.rollback().await.unwrap();
        let fresh = OperationalMaintenanceAuthority::capture(&db, tenant).await.unwrap();
        let tx = begin_fenced_transaction(&db).await.unwrap();
        fresh.require_current_on(&tx).await.unwrap();
        tx.commit().await.unwrap();
        assert!(db.execute_unprepared("UPDATE module_operational_maintenance_state SET revision = 1").await.is_err());
        assert!(db.execute_unprepared("DELETE FROM module_operational_maintenance_state").await.is_err());
        assert!(db.execute_unprepared("INSERT OR REPLACE INTO module_operational_maintenance_state VALUES ('current', 1, '[]')").await.is_err());
        db.execute_unprepared("DROP TABLE module_operational_maintenance_state").await.unwrap();
        assert!(OperationalMaintenanceAuthority::capture(&db, tenant).await.is_err());
    }

    struct TestAuthorizer(bool);

    #[async_trait]
    impl OperationalMaintenanceAuthorizer for TestAuthorizer {
        async fn authorize_replace(
            &self,
            _: &DatabaseTransaction,
            _: &ReplaceOperationalMaintenanceCommand,
        ) -> Result<(), OperationalMaintenanceError> {
            if self.0 { Ok(()) } else { Err(OperationalMaintenanceError::Denied) }
        }
    }

    fn command(expected_revision: u64, scopes: Vec<OperationalMaintenanceScope>) -> ReplaceOperationalMaintenanceCommand {
        let key = Uuid::new_v4();
        ReplaceOperationalMaintenanceCommand {
            expected_revision,
            scopes,
            reason: "operator maintenance".into(),
            context: ModuleCommandContext {
                actor_id: Uuid::new_v4(),
                tenant_id: None,
                trace_id: "test:operational-maintenance".into(),
                correlation_id: key,
                idempotency_key: key,
            },
        }
    }

    #[tokio::test]
    async fn operational_maintenance_authorizes_before_cas_and_history_is_atomic() {
        let db = fixture().await;
        let scopes = vec![OperationalMaintenanceScope { tenant_id: None, module_slug: None }];
        let denied = OperationalMaintenanceService::new(db.clone(), TestAuthorizer(false));
        assert!(matches!(denied.replace(command(1, scopes.clone())).await, Err(OperationalMaintenanceError::Denied)));
        let owner = OperationalMaintenanceService::new(db.clone(), TestAuthorizer(true));
        assert_eq!(owner.state().await.unwrap().revision(), 1);
        assert_eq!(owner.replace(command(1, scopes.clone())).await.unwrap().revision(), 2);
        assert!(matches!(owner.replace(command(1, Vec::new())).await, Err(OperationalMaintenanceError::RevisionConflict)));
        assert_eq!(owner.replace(command(2, scopes)).await.unwrap().revision(), 2);
        assert_eq!(owner.replace(command(2, Vec::new())).await.unwrap().revision(), 3);
        let row = db.query_one_raw(Statement::from_string(
            db.get_database_backend(), "SELECT COUNT(*) AS count FROM module_operational_maintenance_history".to_string(),
        )).await.unwrap().unwrap();
        assert_eq!(row.try_get::<i64>("", "count").unwrap(), 2);
    }

    async fn postgres_transaction(db: &DatabaseConnection, schema: &str) -> DatabaseTransaction {
        let tx = begin_fenced_transaction(db).await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        tx
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_MODULES_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn operational_maintenance_postgres_writer_and_effect_serialize() {
        use std::time::Duration;
        use sea_orm::DbBackend;
        let db = Database::connect(std::env::var("RUSTOK_MODULES_TEST_DATABASE_URL").unwrap()).await.unwrap();
        assert_eq!(db.get_database_backend(), DbBackend::Postgres);
        let schema = format!("maintenance_authority_test_{}", Uuid::new_v4().simple());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}")).await.unwrap();
        let setup = postgres_transaction(&db, &schema).await;
        crate::migrations::m20261127_000097_operational_maintenance_authority::Migration
            .up(&SchemaManager::new(&setup)).await.unwrap();
        let tenant = Uuid::new_v4();
        let proof = OperationalMaintenanceAuthority::from_state(tenant, &load_state(&setup, "").await.unwrap());
        setup.commit().await.unwrap();

        let writer = postgres_transaction(&db, &schema).await;
        writer.execute_unprepared("UPDATE module_operational_maintenance_state SET revision = 2, scopes = '[{\"tenant_id\":null,\"module_slug\":null}]'").await.unwrap();
        let worker_db = db.clone();
        let worker_schema = schema.clone();
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let mut worker = tokio::spawn(async move {
            let effect = postgres_transaction(&worker_db, &worker_schema).await;
            ready_tx.send(()).unwrap();
            let result = proof.require_current_on(&effect).await;
            effect.rollback().await.unwrap();
            result
        });
        ready_rx.await.unwrap();
        assert!(tokio::time::timeout(Duration::from_millis(150), &mut worker).await.is_err());
        writer.commit().await.unwrap();
        assert!(worker.await.unwrap().is_err());

        let effect = postgres_transaction(&db, &schema).await;
        let proof = OperationalMaintenanceAuthority::from_state(tenant, &load_state(&effect, "").await.unwrap());
        proof.require_current_on(&effect).await.unwrap();
        let worker_db = db.clone();
        let worker_schema = schema.clone();
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let mut writer = tokio::spawn(async move {
            let tx = postgres_transaction(&worker_db, &worker_schema).await;
            ready_tx.send(()).unwrap();
            tx.execute_unprepared("UPDATE module_operational_maintenance_state SET revision = 3, scopes = '[]'").await.unwrap();
            tx.commit().await.unwrap();
        });
        ready_rx.await.unwrap();
        assert!(tokio::time::timeout(Duration::from_millis(150), &mut writer).await.is_err());
        effect.commit().await.unwrap();
        writer.await.unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }

    #[test]
    fn operational_maintenance_scope_union_is_tenant_safe() {
        let tenant = Uuid::new_v4();
        let other = Uuid::new_v4();
        let state = OperationalMaintenanceState {
            revision: 2,
            scopes: normalize_scopes(vec![
                OperationalMaintenanceScope { tenant_id: None, module_slug: Some("blog".into()) },
                OperationalMaintenanceScope { tenant_id: Some(tenant), module_slug: Some("forum".into()) },
                OperationalMaintenanceScope { tenant_id: Some(other), module_slug: None },
            ]).unwrap(),
        };
        let a = OperationalMaintenanceAuthority::from_state(tenant, &state);
        assert_eq!(a.input().affected_modules, Some(vec!["blog".into(), "forum".into()]));
        let b = OperationalMaintenanceAuthority::from_state(other, &state);
        assert!(b.input().active && b.input().affected_modules.is_none());
        let c = OperationalMaintenanceAuthority::from_state(Uuid::new_v4(), &state);
        assert_eq!(c.input().affected_modules, Some(vec!["blog".into()]));
    }
}
