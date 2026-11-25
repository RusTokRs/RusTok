/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 * You may not remove or alter this copyright notice or license header.
 */

//! Independent rollback witness. Lock authority first, then witness. Commit
//! the witness first; a split commit can only fence, never grant authority.
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

use hmac::{Hmac, KeyInit, Mac};
use sea_orm::{ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, QueryResult, Statement, TransactionTrait};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use super::{AuthorityBinding, JwtGenerationFence, StoredFence, MAX_LEDGER_BYTES, MAX_GRANT_BYTES, rejected, unavailable};
use crate::{error::Result, jwt_clock::ClockSample};

const MAX_CHECKPOINT_BYTES: usize = 24 * 1024 * 1024;

#[derive(Clone)]
pub(super) struct Witness {
    db: DatabaseConnection,
    pub(super) fault: Arc<AtomicBool>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Checkpoint {
    revision: i64,
    generation: i64,
    payload: String,
    tag: String,
    lease_until_ms: i64,
    drain_until_ms: i64,
    untracked_until_ms: i64,
    replica_grants: String,
}

impl Checkpoint {
    fn capture(stored: &StoredFence) -> Result<Self> {
        if stored.revision < 0 || stored.generation < 0
            || stored.payload.len() > MAX_LEDGER_BYTES || stored.tag.len() > 64
            || stored.replica_grants.len() > MAX_GRANT_BYTES
        {
            return Err(unavailable());
        }
        Ok(Self {
            revision: stored.revision,
            generation: stored.generation,
            payload: stored.payload.clone(),
            tag: stored.tag.clone(),
            lease_until_ms: stored.lease_until_ms,
            drain_until_ms: stored.drain_until_ms,
            untracked_until_ms: stored.untracked_until_ms,
            replica_grants: stored.replica_grants.clone(),
        })
    }

    fn stored(&self, now_ms: i64) -> StoredFence {
        StoredFence {
            revision: self.revision, generation: self.generation,
            payload: self.payload.clone(), tag: self.tag.clone(),
            lease_until_ms: self.lease_until_ms, drain_until_ms: self.drain_until_ms,
            untracked_until_ms: self.untracked_until_ms, replica_grants: self.replica_grants.clone(),
            now_ms,
        }
    }

    fn mac(binding: &AuthorityBinding, payload: &str) -> Result<Hmac<Sha256>> {
        let mut mac = Hmac::<Sha256>::new_from_slice(&binding.witness_key).map_err(|_| unavailable())?;
        mac.update(binding.id.as_bytes());
        mac.update(payload.as_bytes());
        Ok(mac)
    }

    fn encode(&self, binding: &AuthorityBinding) -> Result<(String, String)> {
        let payload = serde_json::to_string(self).map_err(|_| unavailable())?;
        if payload.len() > MAX_CHECKPOINT_BYTES {
            return Err(unavailable());
        }
        let tag = hex::encode(Self::mac(binding, &payload)?.finalize().into_bytes());
        Ok((payload, tag))
    }

    fn decode(binding: &AuthorityBinding, revision: i64, payload: &str, tag: &str) -> Result<Self> {
        if revision < 0 || payload.len() > MAX_CHECKPOINT_BYTES || tag.len() != 64 {
            return Err(unavailable());
        }
        let tag = hex::decode(tag).map_err(|_| unavailable())?;
        Self::mac(binding, payload)?.verify_slice(&tag).map_err(|_| unavailable())?;
        let checkpoint: Self = serde_json::from_str(payload).map_err(|_| unavailable())?;
        if checkpoint.revision != revision || checkpoint.generation < 0
            || checkpoint.payload.len() > MAX_LEDGER_BYTES || checkpoint.tag.len() > 64
            || checkpoint.replica_grants.len() > MAX_GRANT_BYTES
            || checkpoint.lease_until_ms < 0 || checkpoint.drain_until_ms < 0
            || checkpoint.untracked_until_ms < 0
            || checkpoint.untracked_until_ms > checkpoint.lease_until_ms
        {
            return Err(unavailable());
        }
        Ok(checkpoint)
    }
}

// Override session-level asynchronous commit. Acknowledged witness writes
// must reach durable WAL before the authority commit can be attempted.
async fn begin_write(db: &DatabaseConnection) -> Result<DatabaseTransaction> {
    let tx = db.begin().await.map_err(|_| unavailable())?;
    if tx.get_database_backend() == DbBackend::Postgres {
        tx.execute_unprepared("SET LOCAL synchronous_commit = 'on'")
            .await.map_err(|_| unavailable())?;
    }
    Ok(tx)
}

impl Witness {
    pub(super) fn new(db: DatabaseConnection) -> Self {
        Self { db, fault: Arc::new(AtomicBool::new(false)) }
    }

    pub(super) fn faulted(&self) -> bool {
        self.fault.load(Ordering::Acquire)
    }

    fn trip(&self) {
        if !self.fault.swap(true, Ordering::AcqRel) {
            tracing::error!("JWT rollback witness mismatch or uncertain commit; offline recovery required");
        }
    }

    fn healthy(&self) -> Result<()> {
        if self.faulted() { Err(unavailable()) } else { Ok(()) }
    }

    fn binding(fence: &JwtGenerationFence) -> Result<&AuthorityBinding> {
        fence.binding.as_ref().ok_or_else(unavailable)
    }

    /// A distinct URI is not proof of independent failure domains. At least
    /// reject a witness database containing the authority's relation.
    pub(super) async fn ensure_separate(&self) -> Result<()> {
        let backend = self.db.get_database_backend();
        let sql = match backend {
            DbBackend::Postgres => "SELECT CASE WHEN to_regclass('auth_jwt_generation_fence') IS NULL THEN CAST(0 AS BIGINT) ELSE CAST(1 AS BIGINT) END AS present",
            DbBackend::Sqlite => "SELECT COUNT(*) AS present FROM sqlite_master WHERE name = 'auth_jwt_generation_fence'",
            _ => return Err(unavailable()),
        };
        let row = self.db.query_one_raw(Statement::from_string(backend, sql.to_string()))
            .await.map_err(|_| unavailable())?.ok_or_else(unavailable)?;
        let present: i64 = row.try_get("", "present").map_err(|_| unavailable())?;
        if present != 0 { Err(rejected()) } else { Ok(()) }
    }

    async fn checkpoint(&self, tx: &DatabaseTransaction, binding: &AuthorityBinding) -> Result<Checkpoint> {
        let backend = tx.get_database_backend();
        let sql = match backend {
            DbBackend::Postgres => "SELECT revision, payload, tag FROM auth_jwt_witness WHERE id = 1 AND authority_id = $1 AND proof = $2 FOR UPDATE",
            DbBackend::Sqlite => "SELECT revision, payload, tag FROM auth_jwt_witness WHERE id = 1 AND authority_id = ?1 AND proof = ?2",
            _ => return Err(unavailable()),
        };
        let row = tx.query_one_raw(Statement::from_sql_and_values(backend, sql,
            vec![binding.id.clone().into(), binding.proof.clone().into()]))
            .await.map_err(|_| {
                self.trip();
                unavailable()
            })?;
        let Some(row) = row else {
            self.trip();
            return Err(unavailable());
        };
        let result = (|| {
            let revision: i64 = row.try_get("", "revision").map_err(|_| unavailable())?;
            let payload: String = row.try_get("", "payload").map_err(|_| unavailable())?;
            let tag: String = row.try_get("", "tag").map_err(|_| unavailable())?;
            Checkpoint::decode(binding, revision, &payload, &tag)
        })();
        if result.is_err() { self.trip(); }
        result
    }

    fn compare(&self, stored: &StoredFence, checkpoint: &Checkpoint) -> Result<()> {
        if Checkpoint::capture(stored).is_ok_and(|actual| &actual == checkpoint) {
            Ok(())
        } else {
            self.trip();
            Err(unavailable())
        }
    }

    /// All reads serialize with the two-store write protocol. PostgreSQL row
    /// locks are held until the comparison has finished.
    pub(super) async fn read(&self, fence: &JwtGenerationFence) -> Result<StoredFence> {
        self.healthy()?;
        let binding = Self::binding(fence)?;
        let authority = fence.db.begin().await.map_err(|_| unavailable())?;
        let stored = fence.read_on(&authority, true, true).await.map_err(|error| {
            self.trip();
            error
        })?;
        let witness = self.db.begin().await.map_err(|_| unavailable())?;
        let checkpoint = self.checkpoint(&witness, binding).await?;
        self.compare(&stored, &checkpoint)?;
        witness.commit().await.map_err(|_| unavailable())?;
        authority.commit().await.map_err(|_| unavailable())?;
        self.healthy()?;
        Ok(stored)
    }

    /// Existing CAS predicates remain mandatory. Revision equality also
    /// prevents a stale read from crossing an ABA state transition.
    pub(super) async fn apply(&self, fence: &JwtGenerationFence, statement: Statement,
        expected_revision: i64, returning: bool) -> Result<(u64, Option<QueryResult>)> {
        self.healthy()?;
        let binding = Self::binding(fence)?;
        let authority = begin_write(&fence.db).await?;
        let stored = fence.read_on(&authority, true, true).await.map_err(|error| {
            self.trip();
            error
        })?;
        let witness = begin_write(&self.db).await?;
        let checkpoint = self.checkpoint(&witness, binding).await?;
        self.compare(&stored, &checkpoint)?;
        if stored.revision != expected_revision {
            return Ok((0, None));
        }
        let (affected, row) = if returning {
            let row = authority.query_one_raw(statement).await.map_err(|_| unavailable())?;
            (u64::from(row.is_some()), row)
        } else {
            (authority.execute_raw(statement).await.map_err(|_| unavailable())?.rows_affected(), None)
        };
        if affected == 0 {
            return Ok((0, None));
        }
        if affected != 1 {
            return Err(unavailable());
        }

        let next_revision = stored.revision.checked_add(1).ok_or_else(unavailable)?;
        let backend = authority.get_database_backend();
        let sql = match backend {
            DbBackend::Postgres => "UPDATE auth_jwt_generation_fence SET witness_revision = $1 WHERE id = 1 AND witness_revision = $2",
            DbBackend::Sqlite => "UPDATE auth_jwt_generation_fence SET witness_revision = ?1 WHERE id = 1 AND witness_revision = ?2",
            _ => return Err(unavailable()),
        };
        let changed = authority.execute_raw(fence.statement(backend, sql,
            vec![next_revision.into(), stored.revision.into()])?).await.map_err(|_| unavailable())?;
        if changed.rows_affected() != 1 {
            return Err(unavailable());
        }
        let next = Checkpoint::capture(&fence.read_on(&authority, true, false).await?)?;
        let (payload, tag) = next.encode(binding)?;
        let backend = witness.get_database_backend();
        let sql = match backend {
            DbBackend::Postgres => "UPDATE auth_jwt_witness SET revision = $1, payload = $2, tag = $3 WHERE id = 1 AND revision = $4 AND authority_id = $5 AND proof = $6",
            DbBackend::Sqlite => "UPDATE auth_jwt_witness SET revision = ?1, payload = ?2, tag = ?3 WHERE id = 1 AND revision = ?4 AND authority_id = ?5 AND proof = ?6",
            _ => return Err(unavailable()),
        };
        let changed = witness.execute_raw(Statement::from_sql_and_values(backend, sql,
            vec![next.revision.into(), payload.into(), tag.into(), stored.revision.into(),
                binding.id.clone().into(), binding.proof.clone().into()]))
            .await.map_err(|_| unavailable())?;
        if changed.rows_affected() != 1 {
            return Err(unavailable());
        }
        self.healthy()?;
        // Witness ahead is a recoverable fenced state. Authority ahead must
        // never be produced by this protocol. No local grant before both commits.
        if witness.commit().await.is_err() {
            self.trip();
            return Err(unavailable());
        }
        if authority.commit().await.is_err() {
            self.trip();
            return Err(unavailable());
        }
        self.healthy()?;
        Ok((affected, row))
    }

    /// Offline one-time initialization, never a startup fallback. Both stores
    /// must be trusted at cutover, with all older writers stopped.
    pub(super) async fn initialize(&self, fence: &JwtGenerationFence, root: &str) -> Result<()> {
        self.ensure_separate().await?;
        let binding = Self::binding(fence)?;
        let mac = fence.checked_mac(root)?;
        let before = ClockSample::now();
        let authority = begin_write(&fence.db).await?;
        authority.execute_unprepared(
            "ALTER TABLE auth_jwt_generation_fence ADD COLUMN witness_revision BIGINT NOT NULL DEFAULT 0 CHECK (witness_revision >= 0)",
        ).await.map_err(|_| unavailable())?;
        let stored = fence.read_on(&authority, true, true).await?;
        if !fence.clock.validate_db(before, ClockSample::now(), stored.now_ms) {
            return Err(unavailable());
        }
        stored.ledger(&mac)?;
        if stored.lease_until_ms > stored.now_ms || stored.drain_until_ms > stored.now_ms
            || !stored.live_grants()?.is_empty()
        {
            return Err(rejected());
        }
        let checkpoint = Checkpoint::capture(&stored)?;
        let (payload, tag) = checkpoint.encode(binding)?;
        let witness = begin_write(&self.db).await?;
        witness.execute_unprepared(
            "CREATE TABLE auth_jwt_witness (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                authority_id TEXT NOT NULL,
                proof TEXT NOT NULL CHECK (length(proof) = 64),
                revision BIGINT NOT NULL CHECK (revision >= 0),
                payload TEXT NOT NULL,
                tag TEXT NOT NULL CHECK (length(tag) = 64)
            )",
        ).await.map_err(|_| unavailable())?;
        let backend = witness.get_database_backend();
        let sql = match backend {
            DbBackend::Postgres => "INSERT INTO auth_jwt_witness (id, authority_id, proof, revision, payload, tag) VALUES (1, $1, $2, $3, $4, $5)",
            DbBackend::Sqlite => "INSERT INTO auth_jwt_witness (id, authority_id, proof, revision, payload, tag) VALUES (1, ?1, ?2, ?3, ?4, ?5)",
            _ => return Err(unavailable()),
        };
        witness.execute_raw(Statement::from_sql_and_values(backend, sql,
            vec![binding.id.clone().into(), binding.proof.clone().into(), checkpoint.revision.into(), payload.into(), tag.into()]))
            .await.map_err(|_| unavailable())?;
        witness.commit().await.map_err(|_| unavailable())?;
        authority.commit().await.map_err(|_| unavailable())?;
        Ok(())
    }

    /// Offline recovery copies the authenticated witness state, not an
    /// operator-provided generation. Never lower the authority's revision.
    pub(super) async fn recover(&self, fence: &JwtGenerationFence, root: &str) -> Result<()> {
        self.ensure_separate().await?;
        let binding = Self::binding(fence)?;
        let mac = fence.checked_mac(root)?;
        let authority = begin_write(&fence.db).await?;
        if authority.get_database_backend() == DbBackend::Postgres {
            // Supports a restore taken before witness initialization, or an
            // interrupted initial DDL commit. The pinned identity must exist.
            authority.execute_unprepared(
                "ALTER TABLE auth_jwt_generation_fence ADD COLUMN IF NOT EXISTS witness_revision BIGINT NOT NULL DEFAULT 0 CHECK (witness_revision >= 0)",
            ).await.map_err(|_| unavailable())?;
        }
        let stored = fence.read_on(&authority, true, true).await?;
        let witness = begin_write(&self.db).await?;
        let checkpoint = self.checkpoint(&witness, binding).await?;
        if stored.revision > checkpoint.revision {
            return Err(rejected());
        }
        let restored = checkpoint.stored(stored.now_ms);
        restored.ledger(&mac)?;
        restored.live_grants()?;
        let backend = authority.get_database_backend();
        let sql = match backend {
            DbBackend::Postgres => "UPDATE auth_jwt_generation_fence SET generation = $1, payload = $2, tag = $3, lease_until_ms = $4, drain_until_ms = $5, untracked_until_ms = $6, replica_grants = $7, witness_revision = $8 WHERE id = 1 AND witness_revision = $9",
            DbBackend::Sqlite => "UPDATE auth_jwt_generation_fence SET generation = ?1, payload = ?2, tag = ?3, lease_until_ms = ?4, drain_until_ms = ?5, untracked_until_ms = ?6, replica_grants = ?7, witness_revision = ?8 WHERE id = 1 AND witness_revision = ?9",
            _ => return Err(unavailable()),
        };
        let changed = authority.execute_raw(fence.statement(backend, sql,
            vec![restored.generation.into(), restored.payload.into(), restored.tag.into(),
                restored.lease_until_ms.into(), restored.drain_until_ms.into(), restored.untracked_until_ms.into(),
                restored.replica_grants.into(), restored.revision.into(), stored.revision.into()])?)
            .await.map_err(|_| unavailable())?;
        if changed.rows_affected() != 1 {
            return Err(unavailable());
        }
        self.compare(&fence.read_on(&authority, true, false).await?, &checkpoint)?;
        // Keep the witness locked until the exact copy is committed.
        authority.commit().await.map_err(|_| unavailable())?;
        witness.commit().await.map_err(|_| unavailable())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Database;
    use uuid::Uuid;
    use super::super::{JwtAdmission, authority::{bound, import_bound}, tests::{ROOT, OLD, NEW, config, database}};

    async fn deployment() -> (DatabaseConnection, DatabaseConnection, Uuid, JwtGenerationFence) {
        let source = database().await;
        let authority = Database::connect("sqlite::memory:").await.unwrap();
        let witness_db = Database::connect("sqlite::memory:").await.unwrap();
        let id = Uuid::new_v4();
        import_bound(&source, authority.clone(), id, ROOT).await.unwrap();
        let mut fence = bound(authority.clone(), id, ROOT).unwrap();
        fence.witness = Some(Witness::new(witness_db.clone()));
        fence.witness.as_ref().unwrap().initialize(&fence, ROOT).await.unwrap();
        (authority, witness_db, id, fence)
    }

    #[tokio::test]
    async fn rollback_fences_active_lease_and_recovery_preserves_revocation() {
        let (authority, witness_db, id, fence) = deployment().await;
        let first = config(ROOT, "old", OLD, None, None);
        assert_eq!(fence.admit(1, &first).await.unwrap(), JwtAdmission::Admitted);
        let (_, publisher) = first.clone().into_cluster_jwt(1).unwrap();
        assert!(fence.renew(&publisher).await.unwrap());
        let before = fence.load().await.unwrap();
        assert!(fence.revoke_key(ROOT, "access", "old").await.unwrap().newly_revoked);
        let after = fence.load().await.unwrap();
        assert!(after.revision > before.revision);
        // Roll back only the authority, including the valid old ledger MAC.
        authority.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "UPDATE auth_jwt_generation_fence SET payload = ?1, tag = ?2, witness_revision = ?3 WHERE id = 1",
            vec![before.payload.into(), before.tag.into(), before.revision.into()])).await.unwrap();
        assert!(fence.load().await.is_err());
        assert!(fence.witness.as_ref().unwrap().faulted());
        assert!(!publisher.lease_readiness().unwrap().is_ready());
        let mut restarted = bound(authority, id, ROOT).unwrap();
        restarted.witness = Some(Witness::new(witness_db));
        assert!(restarted.load().await.is_err());
        // Recovery requires a fresh offline operator handle, never clearing a
        // running host's fault latch.
        let witness = restarted.witness.as_ref().unwrap();
        witness.recover(&restarted, ROOT).await.unwrap();
        let mut recovered = restarted.clone();
        recovered.witness = Some(Witness::new(witness.db.clone()));
        assert_eq!(recovered.load().await.unwrap().revision, after.revision);
        assert!(!recovered.revoke_key(ROOT, "access", "old").await.unwrap().newly_revoked);
        assert!(recovered.admit(2, &first).await.is_err());
        // A recovery must retain the old holder's grant until its deadline.
        assert_eq!(recovered.load().await.unwrap().replica_grants, after.replica_grants);
        let clean = config(ROOT, "new", NEW, None, None);
        assert_eq!(recovered.admit(2, &clean).await.unwrap(), JwtAdmission::Draining);
    }

    #[tokio::test]
    async fn same_revision_grant_corruption_is_detected_and_reinit_refused() {
        let (authority, _, _, fence) = deployment().await;
        let first = config(ROOT, "old", OLD, None, None);
        assert_eq!(fence.admit(1, &first).await.unwrap(), JwtAdmission::Admitted);
        let (_, publisher) = first.into_cluster_jwt(1).unwrap();
        assert!(fence.renew(&publisher).await.unwrap());
        authority.execute_unprepared("UPDATE auth_jwt_generation_fence SET replica_grants = '{}', lease_until_ms = 0 WHERE id = 1").await.unwrap();
        assert!(fence.load().await.is_err());
        assert!(!publisher.lease_readiness().unwrap().is_ready());
        assert!(fence.witness.as_ref().unwrap().initialize(&fence, ROOT).await.is_err());
    }

    #[tokio::test]
    async fn stale_revision_cannot_mutate_even_if_original_cas_predicate_matches() {
        let (_, _, _, fence) = deployment().await;
        let before = fence.load().await.unwrap();
        let first = config(ROOT, "old", OLD, None, None);
        assert_eq!(fence.admit(1, &first).await.unwrap(), JwtAdmission::Admitted);
        let after = fence.load().await.unwrap();
        let statement = fence.statement(DbBackend::Sqlite,
            "UPDATE auth_jwt_generation_fence SET drain_until_ms = ?1 WHERE id = 1",
            vec![42_i64.into()]).unwrap();
        assert_eq!(fence.execute_checked(statement, before.revision).await.unwrap(), 0);
        let current = fence.load().await.unwrap();
        assert_eq!(current.revision, after.revision);
        assert_eq!(current.drain_until_ms, after.drain_until_ms);
    }

    #[tokio::test]
    async fn mixed_algorithm_checkpoint_persists_and_revocation_cannot_be_bypassed() {
        use super::super::tests::{algorithm_config, rsa_config};
        use serde_json::json;
        let (authority, witness_db, id, fence) = deployment().await;
        let until = chrono::Utc::now().timestamp() + 3600;
        let first = config(ROOT, "old", OLD, None, None);
        let staged = algorithm_config(json!({"keyring":{
            "active_kid":"old", "active_hs256_secret":OLD,
            "verification_keys":[{"kid":"rsa","algorithm":"RS256",
                "material":crate::jwt::TEST_RSA_PUBLIC_KEY,"verify_until":until}]
        }}));
        let promoted = rsa_config(json!([{"kid":"old","algorithm":"HS256",
            "material":OLD,"verify_until":until}]));
        assert_eq!(fence.admit(1, &first).await.unwrap(), JwtAdmission::Admitted);
        assert_eq!(fence.admit(2, &staged).await.unwrap(), JwtAdmission::Admitted);
        assert_eq!(fence.admit(3, &promoted).await.unwrap(), JwtAdmission::Admitted);
        let after = fence.load().await.unwrap();
        let mut reopened = bound(authority, id, ROOT).unwrap();
        reopened.witness = Some(Witness::new(witness_db));
        assert_eq!(reopened.load().await.unwrap().revision, after.revision);
        assert_eq!(reopened.admit(3, &promoted).await.unwrap(), JwtAdmission::Admitted);
        assert!(reopened.revoke_key(ROOT, "access", "old").await.unwrap().newly_revoked);
        assert!(reopened.admit(4, &staged).await.is_err());
        assert!(reopened.admit(4, &promoted).await.is_err());
        assert_eq!(reopened.admit(4, &rsa_config(json!([]))).await.unwrap(), JwtAdmission::Admitted);
        assert!(reopened.admit(5, &promoted).await.is_err());
    }

    #[tokio::test]
    async fn recovery_refuses_authority_ahead_of_witness() {
        let (authority, _, _, fence) = deployment().await;
        authority.execute_unprepared("UPDATE auth_jwt_generation_fence SET witness_revision = 1 WHERE id = 1").await.unwrap();
        assert!(fence.witness.as_ref().unwrap().recover(&fence, ROOT).await.is_err());
        assert!(fence.load().await.is_err());
    }
}
