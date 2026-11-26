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
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement, TransactionTrait, Value};
use sea_orm_migration::SchemaManager;
use sha2::Sha256;
use uuid::Uuid;

use super::{FenceMac, JwtGenerationFence, JwtRevocationReceipt, rejected, unavailable};
use crate::{config::AuthConfig, error::Result};

const DATABASE_URL: &str = "RUSTOK_JWT_AUTHORITY_DATABASE_URL";
const AUTHORITY_ID: &str = "RUSTOK_JWT_AUTHORITY_ID";
const WITNESS_URL: &str = "RUSTOK_JWT_WITNESS_DATABASE_URL";

/// Deployment-pinned identity and independently provisioned PostgreSQL URI.
/// Deliberately not Debug/Serialize: the URI can contain credentials.
pub struct JwtAuthorityConfig {
    uri: String,
    id: Uuid,
    witness_uri: String,
}

#[derive(Clone)]
pub(super) struct AuthorityBinding {
    pub(super) id: String,
    pub(super) proof: String,
    pub(super) witness_key: [u8; 32],
}

impl AuthorityBinding {
    fn new(id: Uuid, root: &str) -> Result<Self> {
        if id.is_nil() {
            return Err(rejected());
        }
        let mac = FenceMac::new(root)?;
        let mut proof = Hmac::<Sha256>::new_from_slice(&mac.authority).map_err(|_| unavailable())?;
        proof.update(id.as_bytes());
        Ok(Self { id: id.to_string(), proof: hex::encode(proof.finalize().into_bytes()), witness_key: mac.witness })
    }

    /// Authenticate the caller even when the ledger is empty.
    pub(super) fn verify_mac(&self, mac: &FenceMac) -> Result<()> {
        let id = Uuid::parse_str(&self.id).map_err(|_| unavailable())?;
        let expected = hex::decode(&self.proof).map_err(|_| unavailable())?;
        let mut proof = Hmac::<Sha256>::new_from_slice(&mac.authority).map_err(|_| unavailable())?;
        proof.update(id.as_bytes());
        proof.verify_slice(&expected).map_err(|_| rejected())
    }

    /// Check the identity atomically in reads and conditional mutations,
    /// including after a reconnect between a read and its CAS.
    pub(super) fn bind_statement(&self, backend: DbBackend, sql: &str, mut values: Vec<Value>) -> Result<Statement> {
        let prefix = match backend {
            DbBackend::Postgres => "$",
            DbBackend::Sqlite => "?",
            _ => return Err(unavailable()),
        };
        let (first, second) = (values.len() + 1, values.len() + 2);
        let condition = format!(" AND EXISTS (SELECT 1 FROM auth_jwt_authority_identity WHERE id = 1 AND authority_id = {prefix}{first} AND proof = {prefix}{second})");
        let sql = match sql.split_once(" RETURNING ") {
            Some((before, returning)) => format!("{before}{condition} RETURNING {returning}"),
            None => match sql.strip_suffix(" FOR UPDATE") {
                Some(before) => format!("{before}{condition} FOR UPDATE"),
                None => format!("{sql}{condition}"),
            },
        };
        values.push(self.id.clone().into());
        values.push(self.proof.clone().into());
        Ok(Statement::from_sql_and_values(backend, sql, values))
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
        Self::from_values(optional(DATABASE_URL)?, optional(AUTHORITY_ID)?, optional(WITNESS_URL)?)
    }

    fn from_values(uri: Option<String>, id: Option<String>, witness_uri: Option<String>) -> Result<Option<Self>> {
        match (uri, id, witness_uri) {
            (None, None, None) => Ok(None),
            (Some(uri), Some(id), Some(witness_uri)) => {
                let id = Uuid::parse_str(&id).map_err(|_| rejected())?;
                if id.is_nil() || uri == witness_uri || !verified_tls_uri(&uri) || !verified_tls_uri(&witness_uri) {
                    return Err(rejected());
                }
                Ok(Some(Self { uri, id, witness_uri }))
            }
            _ => Err(rejected()),
        }
    }

    async fn connect(&self) -> Result<DatabaseConnection> {
        connect_verified(&self.uri).await
    }

    async fn witnessed(&self, root: &str) -> Result<JwtGenerationFence> {
        let mut fence = bound(self.connect().await?, self.id, root)?;
        let witness = super::witness::Witness::new(connect_verified(&self.witness_uri).await?);
        witness.ensure_separate().await?;
        fence.witness = Some(witness);
        Ok(fence)
    }

    /// Open using the Auth-owned MAC root without exporting it to host code.
    pub async fn open_for(&self, config: &AuthConfig) -> Result<JwtGenerationFence> {
        self.open(config.secret()).await
    }

    /// Open only a previously provisioned independent authority and witness.
    /// Use `open_for` from host composition; raw-root access is for trusted
    /// operator and offline tooling only.
    pub async fn open(&self, root: &str) -> Result<JwtGenerationFence> {
        let fence = self.witnessed(root).await?;
        let stored = fence.load().await?;
        stored.ledger(&fence.checked_mac(root)?)?;
        stored.live_grants()?;
        Ok(fence)
    }

    /// Revocation uses the witness but does not consult lease expiry time.
    pub async fn revoke_key(&self, root: &str, purpose: &str, kid: &str) -> Result<JwtRevocationReceipt> {
        self.witnessed(root).await?.revoke_key(root, purpose, kid).await
    }

    /// Explicit one-time offline witness initialization. Never invoked by a host.
    pub async fn init_witness(&self, root: &str) -> Result<()> {
        let fence = self.witnessed(root).await?;
        fence.witness.as_ref().ok_or_else(unavailable)?.initialize(&fence, root).await
    }

    /// Restore only the exact authenticated witness snapshot, offline.
    pub async fn recover_witness(&self, root: &str) -> Result<()> {
        let fence = self.witnessed(root).await?;
        fence.witness.as_ref().ok_or_else(unavailable)?.recover(&fence, root).await
    }

    /// Offline cutover only. Stop all old writers and let their grants expire.
    pub async fn import_offline(&self, source: &DatabaseConnection, root: &str) -> Result<()> {
        import_bound(source, self.connect().await?, self.id, root).await
    }
}

// Restrict the URI grammar before handing it to SQLx: exactly one literal
// sslmode=verify-full, no alternate spellings/overrides or duplicate options.
fn verified_tls_uri(uri: &str) -> bool {
    if uri.trim() != uri || uri.contains('#') || uri.chars().any(char::is_control) {
        return false;
    }
    let Some((base, query)) = uri.split_once('?') else { return false; };
    let Some(target) = base.strip_prefix("postgres://").or_else(|| base.strip_prefix("postgresql://")) else {
        return false;
    };
    let Some((address, database)) = target.split_once('/') else { return false; };
    let endpoint = address.rsplit_once('@').map_or(address, |(_, host)| host);
    // Require an explicit TCP endpoint. Encoded Unix socket paths and empty
    // hosts must not select an implicit non-TLS connection.
    if database.is_empty() || database.contains('/') || endpoint.is_empty()
        || !endpoint.bytes().all(|b| b.is_ascii_alphanumeric() || b"-.:[]".contains(&b))
        || !endpoint.bytes().any(|b| b.is_ascii_alphanumeric())
    {
        return false;
    }
    let mut keys = std::collections::BTreeSet::new();
    for option in query.split('&') {
        let Some((key, value)) = option.split_once('=') else { return false; };
        if value.is_empty() || !keys.insert(key) {
            return false;
        }
        match key {
            "sslmode" if value == "verify-full" => {}
            "sslrootcert" | "sslcert" | "sslkey" => {}
            _ => return false,
        }
    }
    keys.contains("sslmode")
}

async fn connect_verified(uri: &str) -> Result<DatabaseConnection> {
    if !verified_tls_uri(uri) {
        return Err(rejected());
    }
    let mut options = ConnectOptions::new(uri.to_string());
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

pub(super) fn bound(db: DatabaseConnection, id: Uuid, root: &str) -> Result<JwtGenerationFence> {
    let mut fence = JwtGenerationFence::new(db);
    fence.binding = Some(AuthorityBinding::new(id, root)?);
    Ok(fence)
}

#[cfg(test)]
async fn open_bound(db: DatabaseConnection, id: Uuid, root: &str) -> Result<JwtGenerationFence> {
    let fence = bound(db, id, root)?;
    let stored = fence.load().await?;
    stored.ledger(&FenceMac::new(root)?)?;
    stored.live_grants()?;
    Ok(fence)
}

pub(super) async fn import_bound(source: &DatabaseConnection, destination: DatabaseConnection, id: Uuid, root: &str) -> Result<()> {
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
    use super::super::{JwtAdmission, tests::{ROOT, FenceKeys, database}};

    #[test]
    fn external_settings_and_verified_tls_are_mandatory() {
        let uri = "postgres://example/authority?sslmode=verify-full".to_string();
        let witness = "postgres://other/witness?sslmode=verify-full".to_string();
        let id = Uuid::new_v4().to_string();
        assert!(JwtAuthorityConfig::from_values(None, None, None).unwrap().is_none());
        assert!(JwtAuthorityConfig::from_values(Some(uri.clone()), Some(id.clone()), None).is_err());
        assert!(JwtAuthorityConfig::from_values(None, None, Some(witness.clone())).is_err());
        assert!(JwtAuthorityConfig::from_values(Some(uri.clone()), Some(id.clone()), Some(uri.clone())).is_err());
        assert!(JwtAuthorityConfig::from_values(Some(uri), Some(id), Some(witness)).unwrap().is_some());
        for bad in [
            "postgres://example/db",
            "postgres://example/db?sslmode=require",
            "postgres://example/db?sslmode=verify-ca",
            "postgres://example/db?sslmode=verify-full&sslmode=disable",
            "postgres://example/db?sslmode=verify-full&%73slmode=disable",
            "postgres://example/db?sslmode=verify-full&options=anything",
            "postgres://example/db?sslmode=verify-full#fragment",
            "postgres:///db?sslmode=verify-full",
            "postgres://%2Fvar%2Frun%2Fpostgresql/db?sslmode=verify-full",
            "postgres://example?sslmode=verify-full",
        ] {
            assert!(!verified_tls_uri(bad), "{bad}");
        }
        assert!(verified_tls_uri("postgresql://example/db?sslrootcert=%2Fca.pem&sslmode=verify-full"));
    }

    #[tokio::test]
    async fn independent_ledger_survives_application_restore_and_refuses_reimport() {
        let keys = FenceKeys::generate();
        let source = database().await;
        let original = JwtGenerationFence::new(source.clone());
        let first = keys.config(ROOT, "old", keys.old(), None);
        let clean = keys.config(ROOT, "new", &keys.next, None);
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
    async fn empty_bound_authority_rejects_wrong_root_and_cas_rechecks_pin() {
        let keys = FenceKeys::generate();
        let source = database().await;
        let destination = Database::connect("sqlite::memory:").await.unwrap();
        let id = Uuid::new_v4();
        import_bound(&source, destination.clone(), id, ROOT).await.unwrap();
        let independent = open_bound(destination.clone(), id, ROOT).await.unwrap();
        let candidate = keys.config("incorrect-root-at-least-thirty-two-bytes", "new", &keys.next, None);
        assert!(independent.admit(1, &candidate).await.is_err());
        let (_, publisher) = candidate.into_cluster_jwt(1).unwrap();
        assert!(independent.renew(&publisher).await.is_err());
        assert!(independent.load().await.unwrap().is_empty());
        // Simulate the marker disappearing between the read and the CAS.
        let update = independent.statement(DbBackend::Sqlite,
            "UPDATE auth_jwt_generation_fence SET drain_until_ms = ?1 WHERE id = 1 AND generation = ?2 RETURNING drain_until_ms",
            vec![42_i64.into(), 0_i64.into()]).unwrap();
        destination.execute_unprepared("DELETE FROM auth_jwt_authority_identity").await.unwrap();
        assert!(destination.query_one_raw(update).await.unwrap().is_none());
        assert_eq!(JwtGenerationFence::new(destination).load().await.unwrap().drain_until_ms, 0);
    }

    #[tokio::test]
    async fn import_refuses_live_source_grants_and_same_database() {
        let keys = FenceKeys::generate();
        let source = database().await;
        let original = JwtGenerationFence::new(source.clone());
        let first = keys.config(ROOT, "old", keys.old(), None);
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
