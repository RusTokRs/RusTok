//! Atomic bridge between settings receipts and the static lifecycle aggregate.
//!
//! Receipt rows are locked before aggregate rows in every transaction. The
//! durable fence records which receipt and lease took the aggregate, so only
//! an exact receipt reclaim can transfer its own claim; unknown or journal
//! claims require their owner's reconciliation instead of being stolen.

use rustok_api::PortError;
use rustok_outbox::idempotency::{self, Lease};
use sea_orm::{ConnectionTrait, DatabaseConnection, DatabaseTransaction, Statement, TransactionTrait};
use thiserror::Error;
use uuid::Uuid;

use crate::backend::SupportedBackend;
use crate::data::configure_tenant_scope;
use crate::{StaticTenantLifecycleStore, StaticTenantLifecycleStoreError};

#[derive(Debug, Error)]
pub(crate) enum SettingsClaimError {
    #[error("{0}")]
    Receipt(PortError),
    #[error("{0}")]
    Owner(StaticTenantLifecycleStoreError),
    #[error("settings claim storage failed: {0}")]
    Database(String),
}

fn db_error(error: impl std::fmt::Display) -> SettingsClaimError {
    SettingsClaimError::Database(error.to_string())
}

pub(crate) async fn lock_receipt(
    transaction: &DatabaseTransaction,
    lease: Lease,
) -> Result<(), SettingsClaimError> {
    idempotency::lock_current(transaction, lease).await.map_err(SettingsClaimError::Receipt)
}

pub(crate) async fn claim(
    db: &DatabaseConnection,
    tenant: Uuid,
    slug: &str,
    revision: u64,
    lease: Lease,
) -> Result<(), SettingsClaimError> {
    let tx = db.begin().await.map_err(db_error)?;
    let result = async {
        configure_tenant_scope(&tx, tenant).await.map_err(db_error)?;
        lock_receipt(&tx, lease).await?;
        let backend = SupportedBackend::new(tx.get_database_backend()).map_err(db_error)?;
        let snapshot = StaticTenantLifecycleStore::snapshot(&tx, tenant, slug)
            .await.map_err(SettingsClaimError::Owner)?;
        let marker = tx.query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!("SELECT tenant_id, module_slug, claim_token FROM module_static_settings_claim_fences WHERE receipt_id = {}", backend.placeholder(1)),
            vec![backend.uuid_value(lease.operation_id)],
        )).await.map_err(db_error)?;
        let marker_token = if let Some(row) = marker {
            let marker_tenant = backend.uuid_from_row(&row, "tenant_id").map_err(db_error)?;
            let marker_slug: String = row.try_get("", "module_slug").map_err(db_error)?;
            if marker_tenant != tenant || marker_slug != slug {
                return Err(SettingsClaimError::Database("settings claim fence identity is inconsistent".to_string()));
            }
            Some(backend.uuid_from_row(&row, "claim_token").map_err(db_error)?)
        } else {
            None
        };
        let mut transferred = false;
        if let Some(active) = snapshot.active_idempotency_key {
            if active == lease.token {
                // A copied lease is not a second execution permit. Re-entry
                // must wait for a fresh receipt reclaim rather than overlap.
                return Err(SettingsClaimError::Owner(StaticTenantLifecycleStoreError::OperationInProgress { module_slug: slug.to_string() }));
            }
            if marker_token != Some(active) {
                return Err(SettingsClaimError::Owner(StaticTenantLifecycleStoreError::OperationInProgress { module_slug: slug.to_string() }));
            }
            if snapshot.revision != revision {
                // A matching fence with a different revision is not an
                // ordinary conflict: terminalizing the receipt would strand
                // an owned aggregate. Preserve it for investigation.
                return Err(SettingsClaimError::Database(format!(
                    "settings claim revision changed from {revision} to {} while still owned", snapshot.revision
                )));
            }
            // The receipt row is locked. A late old worker cannot lock it,
            // mutate or clean up after the transfer. The aggregate CAS also
            // prevents transferring a claim stolen by another owner.
            StaticTenantLifecycleStore::release_settings(&tx, tenant, slug, active)
                .await.map_err(|error| db_error(format!("settings claim transfer could not release its recorded owner: {error}")))?;
            transferred = true;
        }
        StaticTenantLifecycleStore::claim_without_reclaim(&tx, tenant, slug, revision, lease.token)
            .await.map_err(|error| {
                if transferred {
                    // Rollback restores the old owner and fence. Do not
                    // terminalize their receipt on an uncertain transfer.
                    db_error(format!("settings claim transfer could not acquire its replacement: {error}"))
                } else {
                    SettingsClaimError::Owner(error)
                }
            })?;
        tx.execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!("INSERT INTO module_static_settings_claim_fences (receipt_id, tenant_id, module_slug, claim_token) VALUES ({}, {}, {}, {}) ON CONFLICT (receipt_id) DO UPDATE SET tenant_id = EXCLUDED.tenant_id, module_slug = EXCLUDED.module_slug, claim_token = EXCLUDED.claim_token",
                backend.placeholder(1), backend.placeholder(2), backend.placeholder(3), backend.placeholder(4)),
            vec![backend.uuid_value(lease.operation_id), backend.uuid_value(tenant), slug.to_string().into(), backend.uuid_value(lease.token)],
        )).await.map_err(db_error)?;
        Ok(())
    }.await;
    match result {
        Ok(()) => tx.commit().await.map_err(db_error),
        Err(error) => {
            tx.rollback().await.map_err(|rollback| db_error(format!("{error}; additionally, settings claim rollback failed: {rollback}")))?;
            Err(error)
        }
    }
}

/// Release only the exact receipt's aggregate and remove its fence atomically.
/// The caller must already have locked this receipt in the same transaction.
pub(crate) async fn release_in_transaction(
    tx: &DatabaseTransaction,
    tenant: Uuid,
    slug: &str,
    lease: Lease,
) -> Result<(), SettingsClaimError> {
    let backend = SupportedBackend::new(tx.get_database_backend()).map_err(db_error)?;
    StaticTenantLifecycleStore::release_settings(tx, tenant, slug, lease.token)
        .await.map_err(SettingsClaimError::Owner)?;
    let removed = tx.execute_raw(Statement::from_sql_and_values(
        backend.db_backend(),
        format!("DELETE FROM module_static_settings_claim_fences WHERE receipt_id = {} AND tenant_id = {} AND module_slug = {} AND claim_token = {}",
            backend.placeholder(1), backend.placeholder(2), backend.placeholder(3), backend.placeholder(4)),
        vec![backend.uuid_value(lease.operation_id), backend.uuid_value(tenant), slug.to_string().into(), backend.uuid_value(lease.token)],
    )).await.map_err(db_error)?;
    if removed.rows_affected() != 1 {
        return Err(SettingsClaimError::Database("settings claim fence is missing or changed".to_string()));
    }
    Ok(())
}

pub(crate) async fn abandon(
    db: &DatabaseConnection,
    tenant: Uuid,
    slug: &str,
    lease: Lease,
) -> Result<(), SettingsClaimError> {
    let tx = db.begin().await.map_err(db_error)?;
    let result = async {
        configure_tenant_scope(&tx, tenant).await.map_err(db_error)?;
        lock_receipt(&tx, lease).await?;
        release_in_transaction(&tx, tenant, slug, lease).await
    }.await;
    match result {
        Ok(()) => tx.commit().await.map_err(db_error),
        Err(error) => {
            tx.rollback().await.map_err(|rollback| db_error(format!("{error}; additionally, settings claim cleanup rollback failed: {rollback}")))?;
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_outbox::idempotency::{Admission, OwnerOperationScope};
    use sea_orm::{Database, DbBackend};
    use sea_orm_migration::{MigrationTrait, SchemaManager};

    async fn fixture() -> (DatabaseConnection, Uuid) {
        let mut options = sea_orm::ConnectOptions::new("sqlite::memory:");
        options.max_connections(1).min_connections(1);
        let db = Database::connect(options).await.unwrap();
        db.execute_unprepared("CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL); CREATE VIEW tenant_identity_keys AS SELECT id FROM tenants").await.unwrap();
        db.execute_unprepared("CREATE TABLE module_static_tenant_lifecycle (tenant_id TEXT NOT NULL, module_slug TEXT NOT NULL, revision INTEGER NOT NULL, active_idempotency_key TEXT, active_claimed_at TEXT, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY (tenant_id, module_slug))").await.unwrap();
        let manager = SchemaManager::new(&db);
        rustok_outbox::migration::create_owner_operation_receipts_table(&manager).await.unwrap();
        crate::migrations::m20261117_000093_settings_claim_fences::Migration.up(&manager).await.unwrap();
        let tenant = Uuid::new_v4();
        db.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "INSERT INTO tenants VALUES (?1)", vec![tenant.to_string().into()])).await.unwrap();
        (db, tenant)
    }

    async fn admit(db: &DatabaseConnection, tenant: Uuid, owner: &str, key: &str) -> Lease {
        match idempotency::admit(db, OwnerOperationScope::Tenant(tenant), owner, key, "update", &serde_json::json!({"value": 1})).await.unwrap() {
            Admission::Run(lease) => lease,
            _ => panic!("expected processing receipt"),
        }
    }

    async fn age_receipts(db: &DatabaseConnection) {
        db.execute_unprepared("UPDATE owner_operation_receipts SET updated_at = '2000-01-01T00:00:00+00:00' WHERE status = 'processing'").await.unwrap();
    }

    #[tokio::test]
    async fn reclaimed_receipt_transfers_only_its_claim_and_fences_old_cleanup() {
        let (db, tenant) = fixture().await;
        let old = admit(&db, tenant, "settings", "same").await;
        claim(&db, tenant, "forum", 0, old).await.unwrap();
        age_receipts(&db).await;
        let current = admit(&db, tenant, "settings", "same").await;
        assert_eq!(old.operation_id, current.operation_id);
        assert_ne!(old.token, current.token);
        claim(&db, tenant, "forum", 0, current).await.unwrap();
        assert!(matches!(abandon(&db, tenant, "forum", old).await, Err(SettingsClaimError::Receipt(_))));
        assert_eq!(StaticTenantLifecycleStore::snapshot(&db, tenant, "forum").await.unwrap().active_idempotency_key, Some(current.token));
        let tx = db.begin().await.unwrap();
        assert!(matches!(lock_receipt(&tx, old).await, Err(SettingsClaimError::Receipt(_))));
        tx.rollback().await.unwrap();
        let tx = db.begin().await.unwrap();
        lock_receipt(&tx, current).await.unwrap();
        assert_eq!(StaticTenantLifecycleStore::advance_settings(&tx, tenant, "forum", 0, current.token).await.unwrap(), 1);
        release_in_transaction(&tx, tenant, "forum", current).await.unwrap();
        idempotency::complete(&tx, current, &serde_json::json!({"ok": true})).await.unwrap();
        tx.commit().await.unwrap();
        assert_eq!(StaticTenantLifecycleStore::snapshot(&db, tenant, "forum").await.unwrap().revision, 1);
    }

    #[tokio::test]
    async fn late_old_claim_cannot_take_aggregate_after_receipt_reclaim() {
        let (db, tenant) = fixture().await;
        let old = admit(&db, tenant, "settings", "same").await;
        age_receipts(&db).await;
        let current = admit(&db, tenant, "settings", "same").await;
        claim(&db, tenant, "forum", 0, current).await.unwrap();
        assert!(matches!(claim(&db, tenant, "forum", 0, old).await, Err(SettingsClaimError::Receipt(_))));
        assert_eq!(StaticTenantLifecycleStore::snapshot(&db, tenant, "forum").await.unwrap().active_idempotency_key, Some(current.token));
    }

    #[tokio::test]
    async fn distinct_receipt_and_unrecognized_journal_claim_are_never_stolen() {
        let (db, tenant) = fixture().await;
        let first = admit(&db, tenant, "localization", "same-caller-key").await;
        let second = admit(&db, tenant, "base-settings", "same-caller-key").await;
        claim(&db, tenant, "forum", 0, first).await.unwrap();
        assert!(matches!(claim(&db, tenant, "forum", 0, first).await,
            Err(SettingsClaimError::Owner(StaticTenantLifecycleStoreError::OperationInProgress { .. }))));
        assert!(matches!(claim(&db, tenant, "forum", 0, second).await,
            Err(SettingsClaimError::Owner(StaticTenantLifecycleStoreError::OperationInProgress { .. }))));
        assert!(matches!(abandon(&db, tenant, "forum", second).await, Err(SettingsClaimError::Owner(_))));
        abandon(&db, tenant, "forum", first).await.unwrap();
        let journal_key = Uuid::new_v4();
        StaticTenantLifecycleStore::claim(&db, tenant, "forum", 0, journal_key).await.unwrap();
        db.execute_unprepared("UPDATE module_static_tenant_lifecycle SET active_claimed_at = '2000-01-01T00:00:00+00:00'").await.unwrap();
        assert!(matches!(claim(&db, tenant, "forum", 0, second).await,
            Err(SettingsClaimError::Owner(StaticTenantLifecycleStoreError::OperationInProgress { .. }))));
        assert_eq!(StaticTenantLifecycleStore::snapshot(&db, tenant, "forum").await.unwrap().active_idempotency_key, Some(journal_key));
    }

    #[tokio::test]
    async fn journal_cannot_use_a_settings_token_even_if_keys_match() {
        let (db, tenant) = fixture().await;
        let settings = admit(&db, tenant, "settings", "same").await;
        claim(&db, tenant, "forum", 0, settings).await.unwrap();
        assert!(matches!(StaticTenantLifecycleStore::claim(&db, tenant, "forum", 0, settings.token).await,
            Err(StaticTenantLifecycleStoreError::OperationInProgress { .. })));
        assert!(!StaticTenantLifecycleStore::heartbeat(&db, tenant, "forum", 0, settings.token).await.unwrap());
        assert!(StaticTenantLifecycleStore::advance(&db, tenant, "forum", 0, settings.token).await.is_err());
        assert!(StaticTenantLifecycleStore::release(&db, tenant, "forum", settings.token).await.is_err());
        assert_eq!(StaticTenantLifecycleStore::snapshot(&db, tenant, "forum").await.unwrap().active_idempotency_key, Some(settings.token));
        abandon(&db, tenant, "forum", settings).await.unwrap();
    }

    #[tokio::test]
    async fn inconsistent_marker_cannot_transfer_a_journal_owned_claim() {
        let (db, tenant) = fixture().await;
        let old = admit(&db, tenant, "settings", "same").await;
        claim(&db, tenant, "forum", 0, old).await.unwrap();
        // Simulate corrupt ownership while retaining the old settings marker.
        db.execute_unprepared("UPDATE module_static_tenant_lifecycle SET claim_owner = 'journal'").await.unwrap();
        age_receipts(&db).await;
        let current = admit(&db, tenant, "settings", "same").await;
        assert!(matches!(claim(&db, tenant, "forum", 0, current).await, Err(SettingsClaimError::Database(_))));
        assert_eq!(StaticTenantLifecycleStore::snapshot(&db, tenant, "forum").await.unwrap().active_idempotency_key, Some(old.token));
        let row = db.query_one_raw(Statement::from_string(DbBackend::Sqlite,
            "SELECT claim_token FROM module_static_settings_claim_fences".to_string())).await.unwrap().unwrap();
        assert_eq!(row.try_get::<String>("", "claim_token").unwrap(), old.token.to_string());
    }

    #[tokio::test]
    async fn journal_must_not_reclaim_an_expired_settings_claim() {
        let (db, tenant) = fixture().await;
        let settings = admit(&db, tenant, "settings", "same").await;
        claim(&db, tenant, "forum", 0, settings).await.unwrap();
        db.execute_unprepared("UPDATE module_static_tenant_lifecycle SET active_claimed_at = '2000-01-01T00:00:00+00:00'").await.unwrap();
        let journal = Uuid::new_v4();
        assert!(matches!(StaticTenantLifecycleStore::claim(&db, tenant, "forum", 0, journal).await,
            Err(StaticTenantLifecycleStoreError::OperationInProgress { .. })));
        assert_eq!(StaticTenantLifecycleStore::snapshot(&db, tenant, "forum").await.unwrap().active_idempotency_key, Some(settings.token));
    }

    #[tokio::test]
    async fn migration_rejects_unreconciled_legacy_claims() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("CREATE TABLE module_static_tenant_lifecycle (active_idempotency_key TEXT)").await.unwrap();
        db.execute_unprepared("INSERT INTO module_static_tenant_lifecycle VALUES ('legacy')").await.unwrap();
        assert!(crate::migrations::m20261117_000093_settings_claim_fences::Migration.up(&SchemaManager::new(&db)).await.is_err());
        db.execute_unprepared("DELETE FROM module_static_tenant_lifecycle").await.unwrap();
        crate::migrations::m20261117_000093_settings_claim_fences::Migration.up(&SchemaManager::new(&db)).await.unwrap();
        db.execute_unprepared("INSERT INTO module_static_tenant_lifecycle (active_idempotency_key) VALUES (NULL)").await.unwrap();
        let row = db.query_one_raw(Statement::from_string(DbBackend::Sqlite,
            "SELECT claim_owner FROM module_static_tenant_lifecycle".to_string())).await.unwrap().unwrap();
        assert_eq!(row.try_get::<String>("", "claim_owner").unwrap(), "journal");
        assert!(db.execute_unprepared("UPDATE module_static_tenant_lifecycle SET claim_owner = 'unknown'").await.is_err());
    }
}
