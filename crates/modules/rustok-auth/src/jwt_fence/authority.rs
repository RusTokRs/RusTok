/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 * You may not remove or alter this copyright notice or license header.
 */

//! Independent JWT authority composition and one-time offline import.
use std::time::Duration;
use hmac::{Hmac, KeyInit, Mac};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement, TransactionTrait};
use sea_orm_migration::SchemaManager;
use sha2::Sha256;
use uuid::Uuid;

use super::{FenceMac, JwtGenerationFence, JwtRevocationReceipt, rejected, unavailable};
use crate::error::Result;

const DATABASE_URL: &str = "RUSTOK_JWT_AUTHORITY_DATABASE_URL";
const AUTHORITY_ID: &str = "RUSTOK_JWT_AUTHORITY_ID";

/// Deployment-pinned identity and independently provisioned PostgreSQL URI.
/// Deliberately not Debug/Serialize: the URI can contain credentials.
pub struct JwtAuthorityConfig {
    uri: String,
    id: Uuid,
}

#[derive(Clone)]
pub(super) struct AuthorityBinding {
    id: String,
    proof: String,
}

impl AuthorityBinding {
    fn new(id: Uuid, root: &str) -> Result<Self> {
        if id.is_nil() || !crate::config::hs256_secret_has_min_length(root) {
            return Err(rejected());
        }
        let mac = FenceMac::new(root)?;
        let mut proof = Hmac::<Sha256>::new_from_slice(&mac.authority).map_err(|_| unavailable())?;
        proof.update(id.as_bytes());
        Ok(Self { id: id.to_string(), proof: hex::encode(proof.finalize().into_bytes()) })
    }

    /// Check identity in the same read as the ledger, even after pool reconnect.
    pub(super) fn bind_read(&self, backend: DbBackend, sql: String) -> Result<Statement> {
        let suffix = match backend {
            DbBackend::Postgres => " AND EXISTS (SELECT 1 FROM auth_jwt_authority_identity WHERE id = 1 AND authority_id = $1 AND proof = $2)",
            DbBackend::Sqlite => " AND EXISTS (SELECT 1 FROM auth_jwt_authority_identity WHERE id = 1 AND authority_id = ?1 AND proof = ?2)",
            _ => return Err(unavailable()),
        };
        Ok(Statement::from_sql_and_values(backend, sql + suffix,
            vec![self.id.clone().into(), self.proof.clone().into()]))
    }
}

impl JwtAuthorityConfig {
    /// Partial or malformed settings never select a local fallback.
    pub fn from_env() -> Result<Option<Self>> {
        fn optional(name: &str) -> Result<Option<String>> {
            match std::env::var(name) {
                Ok(value) => Ok(Some(value)),
                Err(std::env::VarError::NotPresent) => Ok(None),
                Err(_) => Err(rejected()),
            }
        }
        Self::from_values(optional(DATABASE_URL)?, optional(AUTHORITY_ID)?)
    }

    fn from_values(uri: Option<String>, id: Option<String>) -> Result<Option<Self>> {
        match (uri, id) {
            (None, None) => Ok(None),
            (Some(uri), Some(id)) => {
                let id = Uuid::parse_str(&id).map_err(|_| rejected())?;
                if id.is_nil() || uri.trim() != uri
                    || !(uri.starts_with("postgres://") || uri.starts_with("postgresql://"))
                {
                    return Err(rejected());
                }
                Ok(Some(Self { uri, id }))
            }
            _ => Err(rejected()),
        }
    }

    async fn connect(&self) -> Result<DatabaseConnection> {
        let mut options = ConnectOptions::new(self.uri.clone());
        options.sqlx_logging(false)
            .max_connections(4)
            .connect_timeout(Duration::from_secs(5))
            .acquire_timeout(Duration::from_secs(1));
        let db = Database::connect(options).await.map_err(|_| unavailable())?;
        if db.get_database_backend() != DbBackend::Postgres {
            return Err(unavailable());
        }
        Ok(db)
    }

    /// Open only a previously provisioned independent authority.
    pub async fn open(&self, root: &str) -> Result<JwtGenerationFence> {
        open_bound(self.connect().await?, self.id, root).await
    }

    /// Revocation is security-reducing; it must remain available during a
    /// clock incident. The bound identity and authenticated ledger are still
    /// checked by revoke_key, but no lease expiry oracle is consulted.
    pub async fn revoke_key(&self, root: &str, purpose: &str, kid: &str) -> Result<JwtRevocationReceipt> {
        let fence = bound(self.connect().await?, self.id, root)?;
        fence.revoke_key(root, purpose, kid).await
    }

    /// Offline cutover only. Stop all old writers and let their grants expire.
    pub async fn import_offline(&self, source: &DatabaseConnection, root: &str) -> Result<()> {
        import_bound(source, self.connect().await?, self.id, root).await
    }
}

fn bound(db: DatabaseConnection, id: Uuid, root: &str) -> Result<JwtGenerationFence> {
    let mut fence = JwtGenerationFence::new(db);
    fence.binding = Some(AuthorityBinding::new(id, root)?);
    Ok(fence)
}

async fn open_bound(db: DatabaseConnection, id: Uuid, root: &str) -> Result<JwtGenerationFence> {
    let fence = bound(db, id, root)?;
    let stored = fence.load().await?;
    stored.ledger(&FenceMac::new(root)?)?;
    stored.live_grants()?;
    Ok(fence)
}

async fn import_bound(source: &DatabaseConnection, destination: DatabaseConnection, id: Uuid, root: &str) -> Result<()> {
    let binding = AuthorityBinding::new(id, root)?;
    let source = JwtGenerationFence::new(source.clone());
    let stored = source.load().await?;
    stored.ledger(&FenceMac::new(root)?)?;
    let grants = stored.live_grants()?;
    if stored.lease_until_ms > stored.now_ms || !grants.is_empty()
        || stored.drain_until_ms > stored.now_ms
    {
        return Err(rejected());
    }

    // PostgreSQL DDL and the full import are atomic on the destination.
    // CREATE without IF NOT EXISTS refuses an existing authority or source.
    let txn = destination.begin().await.map_err(|_| unavailable())?;
    let manager = SchemaManager::new(&txn);
    for migration in crate::migrations::jwt_fence_migrations() {
        migration.up(&manager).await.map_err(|_| unavailable())?;
    }
    txn.execute_unprepared(
        "CREATE TABLE auth_jwt_authority_identity (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            authority_id TEXT NOT NULL,
            proof TEXT NOT NULL CHECK (length(proof) = 64)
        )",
    ).await.map_err(|_| unavailable())?;
    let backend = txn.get_database_backend();
    let (import_sql, identity_sql) = match backend {
        DbBackend::Postgres => (
            "UPDATE auth_jwt_generation_fence SET generation = $1, payload = $2, tag = $3, lease_until_ms = $4, drain_until_ms = $5, untracked_until_ms = $6, replica_grants = $7 WHERE id = 1",
            "INSERT INTO auth_jwt_authority_identity (id, authority_id, proof) VALUES (1, $1, $2)",
        ),
        DbBackend::Sqlite => (
            "UPDATE auth_jwt_generation_fence SET generation = ?1, payload = ?2, tag = ?3, lease_until_ms = ?4, drain_until_ms = ?5, untracked_until_ms = ?6, replica_grants = ?7 WHERE id = 1",
            "INSERT INTO auth_jwt_authority_identity (id, authority_id, proof) VALUES (1, ?1, ?2)",
        ),
        _ => return Err(unavailable()),
    };
    let result = txn.execute_raw(Statement::from_sql_and_values(backend, import_sql,
        vec![stored.generation.into(), stored.payload.into(), stored.tag.into(),
            stored.lease_until_ms.into(), stored.drain_until_ms.into(),
            stored.untracked_until_ms.into(), stored.replica_grants.into()],
    )).await.map_err(|_| unavailable())?;
    if result.rows_affected() != 1 {
        return Err(unavailable());
    }
    txn.execute_raw(Statement::from_sql_and_values(backend, identity_sql,
        vec![binding.id.into(), binding.proof.into()],
    )).await.map_err(|_| unavailable())?;
    txn.commit().await.map_err(|_| unavailable())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{JwtAdmission, tests::{ROOT, OLD, NEW, config, database}};

    #[test]
    fn external_settings_are_all_or_nothing() {
        let uri = "postgres://example/authority".to_string();
        let id = Uuid::new_v4().to_string();
        assert!(JwtAuthorityConfig::from_values(None, None).unwrap().is_none());
        assert!(JwtAuthorityConfig::from_values(Some(uri.clone()), None).is_err());
        assert!(JwtAuthorityConfig::from_values(None, Some(id.clone())).is_err());
        assert!(JwtAuthorityConfig::from_values(Some("sqlite::memory:".into()), Some(id.clone())).is_err());
        assert!(JwtAuthorityConfig::from_values(Some(uri.clone()), Some(Uuid::nil().to_string())).is_err());
        assert!(JwtAuthorityConfig::from_values(Some(uri), Some(id)).unwrap().is_some());
    }

    #[tokio::test]
    async fn independent_ledger_survives_application_restore_and_refuses_reimport() {
        let source = database().await;
        let original = JwtGenerationFence::new(source.clone());
        let first = config(ROOT, "old", OLD, None, None);
        let clean = config(ROOT, "new", NEW, None, None);
        assert_eq!(original.admit(1, &first).await.unwrap(), JwtAdmission::Admitted);
        let destination = Database::connect("sqlite::memory:").await.unwrap();
        let id = Uuid::new_v4();
        assert!(open_bound(destination.clone(), id, ROOT).await.is_err());
        import_bound(&source, destination.clone(), id, ROOT).await.unwrap();
        let independent = open_bound(destination.clone(), id, ROOT).await.unwrap();
        assert!(independent.revoke_key(ROOT, "access", "old").await.unwrap().newly_revoked);
        assert_eq!(independent.admit(2, &clean).await.unwrap(), JwtAdmission::Admitted);
        // Application database is still the pre-revocation snapshot.
        assert_eq!(original.load().await.unwrap().generation, 1);
        let reopened = open_bound(destination.clone(), id, ROOT).await.unwrap();
        assert!(reopened.admit(1, &first).await.is_err());
        assert!(reopened.admit(3, &first).await.is_err());
        assert!(!reopened.revoke_key(ROOT, "access", "old").await.unwrap().newly_revoked);
        let (_, stale) = first.into_cluster_jwt(1).unwrap();
        assert!(!reopened.renew(&stale).await.unwrap());
        assert!(import_bound(&source, destination.clone(), id, ROOT).await.is_err());
        assert!(open_bound(destination.clone(), Uuid::new_v4(), ROOT).await.is_err());
        assert!(open_bound(destination.clone(), id, "incorrect-root-at-least-thirty-two-bytes").await.is_err());
        assert_eq!(reopened.load().await.unwrap().generation, 2);
        destination.execute_unprepared("DELETE FROM auth_jwt_authority_identity").await.unwrap();
        assert!(reopened.load().await.is_err());
    }

    #[tokio::test]
    async fn import_refuses_live_source_grants_and_same_database() {
        let source = database().await;
        let original = JwtGenerationFence::new(source.clone());
        let first = config(ROOT, "old", OLD, None, None);
        assert_eq!(original.admit(1, &first).await.unwrap(), JwtAdmission::Admitted);
        let id = Uuid::new_v4();
        assert!(import_bound(&source, source.clone(), id, ROOT).await.is_err());
        let (_, publisher) = first.into_cluster_jwt(1).unwrap();
        assert!(original.renew(&publisher).await.unwrap());
        let destination = Database::connect("sqlite::memory:").await.unwrap();
        assert!(import_bound(&source, destination.clone(), id, ROOT).await.is_err());
        assert!(open_bound(destination, id, ROOT).await.is_err());
    }
}
