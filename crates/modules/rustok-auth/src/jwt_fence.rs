//! Durable admission of complete JWT generations. No key material is persisted.
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use std::sync::Arc;
use crate::jwt_clock::{ClockGuard, ClockSample, LeaseDeadline, KEY_CLOCK_MARGIN_MS};

use hkdf::Hkdf;
use hmac::{Hmac, KeyInit, Mac};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::{AuthConfig, AuthError, JwtAlgorithm, JwtKeyringPublisher};
use crate::error::Result;

mod observability;
pub use observability::{JwtClusterObservation, JwtOperationalMetrics, JwtReloadOutcome};
use observability::OperationMetrics;

const MAX_ENROLLED_KEYS: usize = 8192;
const MAX_LEDGER_BYTES: usize = 4 * 1024 * 1024;
const MAX_CAS_ATTEMPTS: usize = 8;
const LOCAL_LEASE: Duration = Duration::from_secs(10);
const DB_LEASE_MS: i64 = 15_000;
const DRAIN_MS: i64 = 60_000;
const MAX_GRANTS: usize = 256;
const MAX_GRANT_BYTES: usize = 4 * 1024 * 1024;
const OVERLAP_MARGIN_MS: i64 = 5_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JwtAdmission {
    Admitted,
    Draining,
    AwaitingReplicas,
}

/// An irreversible, database-wide incident command receipt. The horizon is
/// conservative, not confirmation that every in-flight operation has stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct JwtRevocationReceipt {
    pub generation: u64,
    pub revoked_at_ms: i64,
    pub existing_lease_until_ms: i64,
    pub newly_revoked: bool,
}

fn revocation_purpose(purpose: &str) -> Result<&'static str> {
    use crate::jwt_keyring::JwtPurpose;
    let purpose = match purpose {
        "access" => JwtPurpose::Access,
        "password-reset" => JwtPurpose::PasswordReset,
        "email-verification" => JwtPurpose::EmailVerification,
        "invite" => JwtPurpose::Invite,
        _ => return Err(rejected()),
    };
    Ok(purpose.typ())
}

fn rejected() -> AuthError {
    AuthError::Validation("JWT generation rejected by durable fence".into())
}

fn unavailable() -> AuthError {
    AuthError::Internal("JWT generation fence unavailable or inconsistent".into())
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct KeyMetadata {
    pub(crate) identity: String,
    pub(crate) verification_identity: String,
    pub(crate) verify_until: Option<i64>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LegacyMetadata {
    pub(crate) kid: String,
    pub(crate) verify_until: i64,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RingMetadata {
    pub(crate) algorithm: JwtAlgorithm,
    pub(crate) keys: BTreeMap<String, KeyMetadata>,
    pub(crate) legacy: Option<LegacyMetadata>,
}

pub(crate) type Snapshot = BTreeMap<String, RingMetadata>;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplicaGrant {
    generation: u64,
    snapshot: Snapshot,
    until_ms: i64,
}

type ReplicaGrants = BTreeMap<String, ReplicaGrant>;

/// Directional transition: preserve old verification and permit both signers
/// throughout the overlap. The operator separately budgets outgoing JWT TTL.
fn rolling_compatible(old: &Snapshot, next: &Snapshot, now_ms: i64, horizon_ms: i64) -> bool {
    let now = now_ms.saturating_sub(KEY_CLOCK_MARGIN_MS) / 1000;
    let until = horizon_ms.saturating_add(OVERLAP_MARGIN_MS).saturating_add(999) / 1000;
    if old.len() != next.len() {
        return false;
    }
    for (purpose, before) in old {
        let Some(after) = next.get(purpose) else { return false; };
        if before.algorithm != after.algorithm || before.legacy != after.legacy {
            return false;
        }
        let matches = |a: &KeyMetadata, b: &KeyMetadata| {
            a.identity == b.identity && a.verification_identity == b.verification_identity
        };
        // Every currently accepted old verifier remains accepted, with no
        // shortened history deadline. The outgoing signer covers overlap.
        for (kid, key) in &before.keys {
            if key.verify_until.is_some_and(|cutoff| cutoff <= now) {
                continue;
            }
            let Some(other) = after.keys.get(kid) else { return false; };
            if !matches(key, other) {
                return false;
            }
            match (key.verify_until, other.verify_until) {
                (None, Some(cutoff)) if cutoff <= until => return false,
                (Some(cutoff), Some(next_cutoff)) if next_cutoff < cutoff => return false,
                _ => {}
            }
        }
        // Every new signer was already a verifier on the older holder.
        for (kid, signer) in after.keys.iter().filter(|(_, key)| key.verify_until.is_none()) {
            let Some(other) = before.keys.get(kid) else { return false; };
            if !matches(signer, other) || other.verify_until.is_some_and(|cutoff| cutoff <= until) {
                return false;
            }
        }
    }
    true
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SeenKey {
    identity: String,
    ever_active: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    revoked_at_ms: Option<i64>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ledger {
    generation: u64,
    current: Snapshot,
    // Never delete a binding, even after its verification window ends.
    seen: BTreeMap<String, BTreeMap<String, SeenKey>>,
}

impl Ledger {
    fn contains_revoked(&self, snapshot: &Snapshot) -> bool {
        let revoked: std::collections::HashSet<&str> = self.seen.values()
            .flat_map(|keys| keys.values())
            .filter(|binding| binding.revoked_at_ms.is_some())
            .map(|binding| binding.identity.as_str())
            .collect();
        snapshot.values().flat_map(|ring| ring.keys.values())
            .any(|key| revoked.contains(key.identity.as_str()))
    }

    fn enroll(generation: u64, current: Snapshot) -> Self {
        let seen = current.iter().map(|(purpose, ring)| {
            let keys = ring.keys.iter().map(|(kid, key)| {
                // Pre-enrollment history is not evidence of an unused signer.
                (kid.clone(), SeenKey { identity: key.identity.clone(), ever_active: true, revoked_at_ms: None })
            }).collect();
            (purpose.clone(), keys)
        }).collect();
        Self { generation, current, seen }
    }

    fn advance(mut self, generation: u64, next: Snapshot, now: i64) -> Result<Self> {
        if generation <= self.generation || next.len() != self.current.len() || self.contains_revoked(&next) {
            return Err(rejected());
        }

        for (purpose, ring) in &next {
            let old = self.current.get(purpose).ok_or_else(rejected)?;
            if old.algorithm != ring.algorithm {
                return Err(rejected());
            }
            match (&old.legacy, &ring.legacy) {
                (_, None) => {}
                (Some(before), Some(after))
                    if before.kid == after.kid && after.verify_until <= before.verify_until => {}
                _ => return Err(rejected()),
            }
            for (kid, key) in &ring.keys {
                let previous = old.keys.get(kid);
                let seen = self.seen.get(purpose).and_then(|keys| keys.get(kid));
                match (previous, seen) {
                    (Some(before), Some(binding)) => {
                        if key.identity != binding.identity || key.identity != before.identity
                            || key.verification_identity != before.verification_identity
                        {
                            return Err(rejected());
                        }
                        match (before.verify_until, key.verify_until) {
                            (Some(cutoff), None) if binding.ever_active || cutoff <= now => {
                                return Err(rejected());
                            }
                            (Some(cutoff), Some(next_cutoff)) if next_cutoff > cutoff => {
                                return Err(rejected());
                            }
                            _ => {}
                        }
                    }
                    // A known but absent ID is a permanent tombstone.
                    (None, Some(_)) => return Err(rejected()),
                    (Some(_), None) => return Err(unavailable()),
                    (None, None) => {
                        if self.seen.values().flat_map(|keys| keys.values())
                            .any(|binding| binding.identity == key.identity)
                        {
                            return Err(rejected());
                        }
                    }
                }
                let binding = self.seen.entry(purpose.clone()).or_default()
                    .entry(kid.clone()).or_insert_with(|| SeenKey {
                        identity: key.identity.clone(), ever_active: false, revoked_at_ms: None,
                    });
                binding.ever_active |= key.verify_until.is_none();
            }
        }
        if self.seen.values().map(BTreeMap::len).sum::<usize>() > MAX_ENROLLED_KEYS {
            return Err(rejected());
        }
        self.generation = generation;
        self.current = next;
        Ok(self)
    }
}

struct FenceMac {
    identity: [u8; 32],
    state: [u8; 32],
}

impl FenceMac {
    fn new(root: &str) -> Result<Self> {
        let hkdf = Hkdf::<Sha256>::new(
            Some(b"rustok/auth/jwt-generation-fence/v1"), root.as_bytes(),
        );
        let mut identity = [0_u8; 32];
        let mut state = [0_u8; 32];
        hkdf.expand(b"key-identity", &mut identity).map_err(|_| unavailable())?;
        hkdf.expand(b"state-envelope", &mut state).map_err(|_| unavailable())?;
        Ok(Self { identity, state })
    }

    fn identity(&self, digest: &[u8; 32]) -> Result<String> {
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.identity).map_err(|_| unavailable())?;
        mac.update(digest);
        Ok(hex::encode(mac.finalize().into_bytes()))
    }

    fn state_mac(&self, payload: &str) -> Result<Hmac<Sha256>> {
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.state).map_err(|_| unavailable())?;
        mac.update(payload.as_bytes());
        Ok(mac)
    }
}

/// Auth-owned database authority. Call only from trusted deployment composition.
/// The store admits a snapshot; it does not distribute secrets or publish keys
/// into a process. No database access is added to the JWT request path.
#[derive(Clone)]
pub struct JwtGenerationFence {
    db: DatabaseConnection,
    metrics: OperationMetrics,
    clock: Arc<ClockGuard>,
}

struct StoredFence {
    generation: i64,
    payload: String,
    tag: String,
    lease_until_ms: i64,
    drain_until_ms: i64,
    untracked_until_ms: i64,
    replica_grants: String,
    now_ms: i64,
}

impl StoredFence {
    fn live_grants(&self) -> Result<ReplicaGrants> {
        if self.replica_grants.len() > MAX_GRANT_BYTES {
            return Err(unavailable());
        }
        let mut grants: ReplicaGrants = serde_json::from_str(&self.replica_grants)
            .map_err(|_| unavailable())?;
        if grants.len() > MAX_GRANTS || grants.values().any(|grant| {
            grant.until_ms <= 0 || grant.until_ms > self.lease_until_ms
                || i64::try_from(grant.generation).ok().is_none_or(|g| g > self.generation)
        }) {
            return Err(unavailable());
        }
        grants.retain(|_, grant| grant.until_ms > self.now_ms);
        let covered = grants.values().map(|g| g.until_ms)
            .chain(std::iter::once(self.untracked_until_ms)).max().unwrap_or(0);
        if self.lease_until_ms > self.now_ms && covered < self.lease_until_ms {
            return Err(unavailable());
        }
        Ok(grants)
    }

    fn is_empty(&self) -> bool {
        self.generation == 0 && self.payload.is_empty() && self.tag.is_empty()
    }

    fn ledger(&self, mac: &FenceMac) -> Result<Option<Ledger>> {
        if self.is_empty() {
            return Ok(None);
        }
        if self.generation <= 0 || self.payload.len() > MAX_LEDGER_BYTES || self.tag.len() != 64 {
            return Err(unavailable());
        }
        let tag = hex::decode(&self.tag).map_err(|_| unavailable())?;
        mac.state_mac(&self.payload)?.verify_slice(&tag).map_err(|_| unavailable())?;
        let ledger: Ledger = serde_json::from_str(&self.payload).map_err(|_| unavailable())?;
        if i64::try_from(ledger.generation).ok() != Some(self.generation)
            || ledger.seen.values().map(BTreeMap::len).sum::<usize>() > MAX_ENROLLED_KEYS
        {
            return Err(unavailable());
        }
        Ok(Some(ledger))
    }
}

impl JwtGenerationFence {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db, metrics: OperationMetrics::default(), clock: Arc::new(ClockGuard::new()) }
    }

    async fn load(&self) -> Result<StoredFence> {
        self.load_at(ClockSample::now()).await
    }

    async fn load_at(&self, before: ClockSample) -> Result<StoredFence> {
        if !self.clock.healthy_at(before) {
            return Err(unavailable());
        }
        let stored = self.load_raw().await?;
        if !self.clock.validate_db(before, ClockSample::now(), stored.now_ms) {
            return Err(unavailable());
        }
        Ok(stored)
    }

    // Only emergency revocation may read without using the clock as an
    // expiry oracle: it never grants, removes or extends authority.
    async fn load_raw(&self) -> Result<StoredFence> {
        let backend = self.db.get_database_backend();
        let clock = match backend {
            DbBackend::Postgres => "CAST(EXTRACT(EPOCH FROM clock_timestamp()) * 1000 AS BIGINT)",
            DbBackend::Sqlite => "CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)",
            _ => return Err(unavailable()),
        };
        let row = self.db.query_one_raw(Statement::from_string(
            backend,
            format!("SELECT generation, payload, tag, lease_until_ms, drain_until_ms, untracked_until_ms, replica_grants, {clock} AS now_ms FROM auth_jwt_generation_fence WHERE id = 1"),
        )).await.map_err(|_| unavailable())?.ok_or_else(unavailable)?;
        let stored = StoredFence {
            generation: row.try_get("", "generation").map_err(|_| unavailable())?,
            payload: row.try_get("", "payload").map_err(|_| unavailable())?,
            tag: row.try_get("", "tag").map_err(|_| unavailable())?,
            lease_until_ms: row.try_get("", "lease_until_ms").map_err(|_| unavailable())?,
            drain_until_ms: row.try_get("", "drain_until_ms").map_err(|_| unavailable())?,
            untracked_until_ms: row.try_get("", "untracked_until_ms").map_err(|_| unavailable())?,
            replica_grants: row.try_get("", "replica_grants").map_err(|_| unavailable())?,
            now_ms: row.try_get("", "now_ms").map_err(|_| unavailable())?,
        };
        if stored.now_ms <= 0 || stored.lease_until_ms < 0 || stored.drain_until_ms < 0
            || stored.untracked_until_ms < 0 || stored.untracked_until_ms > stored.lease_until_ms
        {
            return Err(unavailable());
        }
        Ok(stored)
    }

    /// Refuse configuration-source downgrade after the first file enrollment.
    pub async fn ensure_static_allowed(&self) -> Result<()> {
        if self.load().await?.is_empty() { Ok(()) } else { Err(rejected()) }
    }

    /// Trusted operator command, not a request/tenant capability.
    /// The supplied stable root must authenticate the enrolled authority.
    /// The CAS updates only the envelope and tag, preserving all leases.
    pub async fn revoke_key(
        &self, root: &str, purpose: &str, kid: &str,
    ) -> Result<JwtRevocationReceipt> {
        let purpose = revocation_purpose(purpose)?;
        if !crate::config::hs256_secret_has_min_length(root)
            || !crate::jwt_keyring::valid_kid(kid)
        {
            return Err(rejected());
        }
        let mac = FenceMac::new(root)?;
        for _ in 0..MAX_CAS_ATTEMPTS {
            let stored = self.load_raw().await?;
            let mut ledger = stored.ledger(&mac)?.ok_or_else(rejected)?;
            let binding = ledger.seen.get_mut(purpose)
                .and_then(|keys| keys.get_mut(kid)).ok_or_else(rejected)?;
            if let Some(revoked_at_ms) = binding.revoked_at_ms {
                return Ok(JwtRevocationReceipt {
                    generation: ledger.generation, revoked_at_ms,
                    existing_lease_until_ms: stored.lease_until_ms,
                    newly_revoked: false,
                });
            }
            binding.revoked_at_ms = Some(stored.now_ms);
            let payload = serde_json::to_string(&ledger).map_err(|_| unavailable())?;
            // Reserve enough envelope headroom for every possible revocation
            // in admission; an emergency command must not prune identities.
            if payload.len() > MAX_LEDGER_BYTES {
                return Err(unavailable());
            }
            let tag = hex::encode(mac.state_mac(&payload)?.finalize().into_bytes());
            let backend = self.db.get_database_backend();
            // Lease-only writes do not modify the authenticated envelope.
            // Compare its tag, not the lease registry, and return the horizon
            // from the updated row. This serializes with concurrent grants
            // without repeatedly losing to ordinary renewal traffic.
            let sql = match backend {
                DbBackend::Postgres => "UPDATE auth_jwt_generation_fence SET payload = $1, tag = $2 WHERE id = 1 AND generation = $3 AND tag = $4 RETURNING lease_until_ms",
                DbBackend::Sqlite => "UPDATE auth_jwt_generation_fence SET payload = ?1, tag = ?2 WHERE id = 1 AND generation = ?3 AND tag = ?4 RETURNING lease_until_ms",
                _ => return Err(unavailable()),
            };
            let updated = self.db.query_one_raw(Statement::from_sql_and_values(
                backend, sql, vec![payload.into(), tag.into(), stored.generation.into(),
                    stored.tag.into()],
            )).await.map_err(|_| unavailable())?;
            if let Some(row) = updated {
                let existing_lease_until_ms: i64 = row.try_get("", "lease_until_ms")
                    .map_err(|_| unavailable())?;
                tracing::warn!(generation = ledger.generation, "JWT emergency key revocation committed");
                return Ok(JwtRevocationReceipt {
                    generation: ledger.generation, revoked_at_ms: stored.now_ms,
                    existing_lease_until_ms,
                    newly_revoked: true,
                });
            }
        }
        Err(unavailable())
    }

    /// Renew only the exact local generation. A grant is bounded from before
    /// DB IO; an expired/delayed grant cannot revive a local deadline.
    pub async fn renew(&self, publisher: &JwtKeyringPublisher) -> Result<bool> {
        let result = self.renew_inner(publisher).await;
        if self.clock.faulted() {
            publisher.fence_clock()?;
        }
        self.metrics.record("renew", match &result {
            Ok(true) => "granted", Ok(false) => "denied", Err(_) => "error",
        });
        result
    }

    async fn renew_inner(&self, publisher: &JwtKeyringPublisher) -> Result<bool> {
        if !publisher.clock_healthy()? {
            return Err(unavailable());
        }
        let mac = FenceMac::new(publisher.lease_root())?;
        let (generation, snapshot) = publisher.lease_metadata(|identity| mac.identity(identity))?;
        let Some(deadline) = self.grant(publisher, generation, snapshot, &mac, true).await? else {
            return Ok(false);
        };
        publisher.install_lease(generation, deadline)
    }

    /// Obtain a grant for the exact admitted candidate before exchanging
    /// local keys and lease in a single write-fenced operation.
    pub async fn activate(
        &self, publisher: &JwtKeyringPublisher, generation: u64, candidate: AuthConfig,
    ) -> Result<bool> {
        let result = self.activate_inner(publisher, generation, candidate).await;
        if self.clock.faulted() {
            publisher.fence_clock()?;
        }
        self.metrics.record("activate", match &result {
            Ok(true) => "applied", Ok(false) => "denied", Err(_) => "error",
        });
        result
    }

    async fn activate_inner(
        &self, publisher: &JwtKeyringPublisher, generation: u64, candidate: AuthConfig,
    ) -> Result<bool> {
        if !publisher.clock_healthy()? {
            return Err(unavailable());
        }
        if generation <= publisher.generation()? || candidate.secret() != publisher.lease_root() {
            return Err(rejected());
        }
        crate::validate_auth_config(&candidate)?;
        let mac = FenceMac::new(candidate.secret())?;
        let snapshot = candidate.fence_snapshot(|identity| mac.identity(identity))?;
        let Some(deadline) = self.grant(publisher, generation, snapshot, &mac, false).await? else {
            return Ok(false);
        };
        if !publisher.publish_granted(generation, candidate, deadline)? {
            return Ok(false);
        }
        // Only after the local write fence has drained old crypto and
        // installed the new generation may this incarnation ACK replacement.
        let started = Instant::now();
        let ack = self.acknowledge(publisher, generation, &mac).await;
        self.metrics.record_ack(started.elapsed(), match &ack {
            Ok(true) => "complete", Ok(false) => "deferred", Err(_) => "error",
        });
        ack?;
        Ok(true)
    }

    /// Remove only grants of this incarnation that can no longer be used.
    /// Generation is monotonic under the local write fence; even a delayed
    /// renewal cannot reinstall an older grant after this ACK.
    async fn acknowledge(
        &self, publisher: &JwtKeyringPublisher, applied: u64, mac: &FenceMac,
    ) -> Result<bool> {
        let prefix = format!("{}:", publisher.incarnation());
        for _ in 0..MAX_CAS_ATTEMPTS {
            let stored = self.load().await?;
            // Validate the durable envelope before modifying its grant registry.
            let _ = stored.ledger(mac)?;
            let mut grants = stored.live_grants()?;
            let before = grants.len();
            grants.retain(|key, grant| !key.starts_with(&prefix) || grant.generation >= applied);
            if grants.len() == before {
                return Ok(true);
            }
            let horizon = grants.values().map(|grant| grant.until_ms)
                .chain(std::iter::once(stored.untracked_until_ms)).max().unwrap_or(0);
            let encoded = serde_json::to_string(&grants).map_err(|_| unavailable())?;
            let backend = self.db.get_database_backend();
            let sql = match backend {
                DbBackend::Postgres => "UPDATE auth_jwt_generation_fence SET lease_until_ms = $1, replica_grants = $2 WHERE id = 1 AND generation = $3 AND tag = $4 AND drain_until_ms = $5 AND lease_until_ms = $6 AND replica_grants = $7",
                DbBackend::Sqlite => "UPDATE auth_jwt_generation_fence SET lease_until_ms = ?1, replica_grants = ?2 WHERE id = 1 AND generation = ?3 AND tag = ?4 AND drain_until_ms = ?5 AND lease_until_ms = ?6 AND replica_grants = ?7",
                _ => return Err(unavailable()),
            };
            let updated = self.db.execute_raw(Statement::from_sql_and_values(
                backend, sql, vec![horizon.into(), encoded.into(), stored.generation.into(),
                    stored.tag.into(), stored.drain_until_ms.into(), stored.lease_until_ms.into(),
                    stored.replica_grants.into()],
            )).await.map_err(|_| unavailable())?;
            if updated.rows_affected() == 1 {
                return Ok(true);
            }
        }
        // Contention does not invalidate the installed lease. Renewal will
        // retry the ACK; until then the old grant conservatively remains.
        Ok(false)
    }

    async fn grant(
        &self, publisher: &JwtKeyringPublisher, generation: u64, snapshot: Snapshot, mac: &FenceMac,
        already_applied: bool,
    ) -> Result<Option<LeaseDeadline>> {
        let generation_sql = i64::try_from(generation).map_err(|_| rejected())?;
        let key = format!("{}:{generation}", publisher.incarnation());
        for _ in 0..MAX_CAS_ATTEMPTS {
            let started = ClockSample::now();
            let stored = self.load_at(started).await?;
            let ledger = stored.ledger(mac)?;
            if ledger.as_ref().is_some_and(|ledger| ledger.contains_revoked(&snapshot)) {
                if already_applied {
                    publisher.fence_revoked_generation(generation)?;
                }
                self.metrics.record("grant", "revoked");
                return Ok(None);
            }
            let admitted = match ledger {
                None => generation == 0,
                Some(ledger) => generation == ledger.generation && snapshot == ledger.current,
            };
            if !admitted {
                self.metrics.record("grant", "not_admitted");
                return Ok(None);
            }
            if stored.drain_until_ms > stored.now_ms {
                self.metrics.record("grant", "draining");
                return Ok(None);
            }
            let mut grants = stored.live_grants()?;
            if already_applied {
                // Renewal observes the locally installed generation, so it
                // can retry an ACK lost to CAS contention during activation.
                let prefix = format!("{}:", publisher.incarnation());
                grants.retain(|key, grant| !key.starts_with(&prefix) || grant.generation >= generation);
            }
            if !grants.contains_key(&key) && grants.len() >= MAX_GRANTS {
                self.metrics.record("grant", "capacity");
                return Ok(None);
            }
            let until = stored.now_ms.checked_add(DB_LEASE_MS).ok_or_else(unavailable)?;
            let until = if let Some(previous) = grants.get(&key) {
                if previous.snapshot != snapshot || previous.generation != generation {
                    return Err(unavailable());
                }
                until.max(previous.until_ms)
            } else { until };
            grants.insert(key.clone(), ReplicaGrant {
                generation, snapshot: snapshot.clone(), until_ms: until,
            });
            let encoded = serde_json::to_string(&grants).map_err(|_| unavailable())?;
            if encoded.len() > MAX_GRANT_BYTES {
                self.metrics.record("grant", "capacity");
                return Ok(None);
            }
            let horizon = grants.values().map(|grant| grant.until_ms)
                .chain(std::iter::once(stored.untracked_until_ms)).max().unwrap_or(0);
            let backend = self.db.get_database_backend();
            let sql = match backend {
                DbBackend::Postgres => "UPDATE auth_jwt_generation_fence SET lease_until_ms = $1, replica_grants = $2 WHERE id = 1 AND generation = $3 AND tag = $4 AND drain_until_ms = $5 AND lease_until_ms = $6 AND replica_grants = $7",
                DbBackend::Sqlite => "UPDATE auth_jwt_generation_fence SET lease_until_ms = ?1, replica_grants = ?2 WHERE id = 1 AND generation = ?3 AND tag = ?4 AND drain_until_ms = ?5 AND lease_until_ms = ?6 AND replica_grants = ?7",
                _ => return Err(unavailable()),
            };
            let updated = self.db.execute_raw(Statement::from_sql_and_values(
                backend, sql, vec![horizon.into(), encoded.into(),
                    generation_sql.into(), stored.tag.into(), stored.drain_until_ms.into(),
                    stored.lease_until_ms.into(), stored.replica_grants.into()],
            )).await.map_err(|_| unavailable())?;
            if updated.rows_affected() == 1 {
                if !self.clock.healthy_at(ClockSample::now()) {
                    return Err(unavailable());
                }
                return Ok(Some(LeaseDeadline::new(started, LOCAL_LEASE, self.clock.clone())
                    .ok_or_else(unavailable)?));
            }
        }
        self.metrics.record("grant", "contention");
        Ok(None)
    }

    /// Admit before local publication. Same generation is idempotent only for
    /// the same canonical snapshot, including cutoffs and legacy mapping.
    pub async fn admit(&self, generation: u64, candidate: &AuthConfig) -> Result<JwtAdmission> {
        let result = self.admit_inner(generation, candidate).await;
        self.metrics.record("admit", match &result {
            Ok(JwtAdmission::Admitted) => "admitted",
            Ok(JwtAdmission::Draining) => "draining",
            Ok(JwtAdmission::AwaitingReplicas) => "awaiting_replicas",
            Err(AuthError::Validation(_)) => "rejected",
            Err(_) => "error",
        });
        result
    }

    async fn admit_inner(&self, generation: u64, candidate: &AuthConfig) -> Result<JwtAdmission> {
        let generation_sql = i64::try_from(generation).map_err(|_| rejected())?;
        if generation_sql <= 0 {
            return Err(rejected());
        }
        crate::validate_auth_config(candidate)?;
        let mac = FenceMac::new(candidate.secret())?;
        let snapshot = candidate.fence_snapshot(|identity| mac.identity(identity))?;
        for _ in 0..MAX_CAS_ATTEMPTS {
            let stored = self.load().await?;
            let previous = stored.ledger(&mac)?;
            let grants = stored.live_grants()?;
            let horizon = stored.lease_until_ms.max(stored.now_ms.saturating_add(DB_LEASE_MS));
            if previous.as_ref().is_some_and(|ledger| ledger.contains_revoked(&snapshot)) {
                return Err(rejected());
            }
            let rolling = stored.drain_until_ms <= stored.now_ms
                && previous.as_ref().is_some_and(|ledger| {
                    !ledger.contains_revoked(&ledger.current)
                        && !grants.values().any(|grant| ledger.contains_revoked(&grant.snapshot))
                        && rolling_compatible(&ledger.current, &snapshot, stored.now_ms, horizon)
                });
            let next = match previous {
                None => Ledger::enroll(generation, snapshot.clone()),
                Some(ledger) if generation == ledger.generation => {
                    return if snapshot == ledger.current { Ok(JwtAdmission::Admitted) } else { Err(rejected()) };
                }
                Some(ledger) => ledger.advance(generation, snapshot.clone(),
                    stored.now_ms.saturating_add(KEY_CLOCK_MARGIN_MS) / 1000)?,
            };
            // Reject an oversized ledger before closing renewal. A candidate
            // that cannot be persisted must not start an avoidable drain.
            let payload = serde_json::to_string(&next).map_err(|_| unavailable())?;
            // Each missing timestamp could grow the envelope by at most
            // 40 bytes (field name, separator and signed i64 value).
            let remaining_revocations = next.seen.values().flat_map(|keys| keys.values())
                .filter(|binding| binding.revoked_at_ms.is_none()).count();
            if payload.len().saturating_add(remaining_revocations.saturating_mul(40)) > MAX_LEDGER_BYTES {
                return Err(rejected());
            }
            if rolling {
                // Every outstanding grant is an acknowledgement of the actual
                // verifier set. Older incompatible grants must expire before
                // promotion; do not drain the compatible current generation.
                if stored.untracked_until_ms > stored.now_ms || grants.values().any(|grant| {
                    !rolling_compatible(&grant.snapshot, &snapshot, stored.now_ms, grant.until_ms)
                }) {
                    return Ok(JwtAdmission::AwaitingReplicas);
                }
            } else if stored.lease_until_ms > stored.now_ms {
                if stored.drain_until_ms > stored.now_ms {
                    return Ok(JwtAdmission::Draining);
                }
                let until = stored.now_ms.checked_add(DRAIN_MS).ok_or_else(unavailable)?;
                let backend = self.db.get_database_backend();
                let sql = match backend {
                    DbBackend::Postgres => "UPDATE auth_jwt_generation_fence SET drain_until_ms = $1 WHERE id = 1 AND generation = $2 AND tag = $3 AND drain_until_ms = $4",
                    DbBackend::Sqlite => "UPDATE auth_jwt_generation_fence SET drain_until_ms = ?1 WHERE id = 1 AND generation = ?2 AND tag = ?3 AND drain_until_ms = ?4",
                    _ => return Err(unavailable()),
                };
                let updated = self.db.execute_raw(Statement::from_sql_and_values(
                    backend, sql, vec![until.into(), stored.generation.into(), stored.tag.into(), stored.drain_until_ms.into()],
                )).await.map_err(|_| unavailable())?;
                if updated.rows_affected() == 1 {
                    return Ok(JwtAdmission::Draining);
                }
                continue;
            }
            let tag = hex::encode(mac.state_mac(&payload)?.finalize().into_bytes());
            let encoded = serde_json::to_string(&grants).map_err(|_| unavailable())?;
            let retained_horizon = if rolling { stored.lease_until_ms } else { 0 };
            let backend = self.db.get_database_backend();
            let sql = match backend {
                DbBackend::Postgres => "UPDATE auth_jwt_generation_fence SET generation = $1, payload = $2, tag = $3, lease_until_ms = $4, drain_until_ms = 0, untracked_until_ms = 0, replica_grants = $5 WHERE id = 1 AND generation = $6 AND tag = $7 AND lease_until_ms = $8 AND drain_until_ms = $9 AND replica_grants = $10",
                DbBackend::Sqlite => "UPDATE auth_jwt_generation_fence SET generation = ?1, payload = ?2, tag = ?3, lease_until_ms = ?4, drain_until_ms = 0, untracked_until_ms = 0, replica_grants = ?5 WHERE id = 1 AND generation = ?6 AND tag = ?7 AND lease_until_ms = ?8 AND drain_until_ms = ?9 AND replica_grants = ?10",
                _ => return Err(unavailable()),
            };
            let updated = self.db.execute_raw(Statement::from_sql_and_values(
                backend, sql, vec![generation_sql.into(), payload.into(), tag.into(), retained_horizon.into(),
                    encoded.into(), stored.generation.into(), stored.tag.into(), stored.lease_until_ms.into(),
                    stored.drain_until_ms.into(), stored.replica_grants.into()],
            )).await.map_err(|_| unavailable())?;
            if updated.rows_affected() == 1 {
                return Ok(JwtAdmission::Admitted);
            }
        }
        Err(unavailable())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Database;
    use sea_orm_migration::SchemaManager;

    pub(super) const ROOT: &str = "stable-host-root-for-fence-tests-only-32bytes";
    pub(super) const OLD: &str = "first-independent-signing-key-at-least-32-bytes";
    pub(super) const NEW: &str = "second-independent-signing-key-at-least-32-bytes";
    const THIRD: &str = "third-independent-signing-key-at-least-32-bytes";

    pub(super) fn config(root: &str, kid: &str, material: &str, history: Option<(&str, &str, i64)>,
        legacy: Option<(&str, i64)>) -> AuthConfig {
        let mut access = serde_json::json!({"active_kid": kid, "active_hs256_secret": material});
        if let Some((kid, material, until)) = history {
            access["verification_keys"] = serde_json::json!([{
                "kid": kid, "material": material, "verify_until": until,
            }]);
        }
        if let Some((kid, until)) = legacy {
            access["legacy_without_kid"] = serde_json::json!({"kid": kid, "verify_until": until});
        }
        let doc = serde_json::json!({
            "schema_version": 1, "generation": 1,
            "access": {"keyring": access},
            "password_reset": {"keyring": {"active_kid": "reset"}},
            "email_verification": {"keyring": {"active_kid": "email"}},
            "invite": {"keyring": {"active_kid": "invite"}},
        });
        AuthConfig::new(root.to_string()).prepare_jwt_document(
            &serde_json::to_vec(&doc).unwrap(),
        ).unwrap().1
    }

    pub(super) async fn database() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        for name in [
            "m20261125_000002_create_auth_jwt_generation_fence",
            "m20261125_000003_add_auth_jwt_cluster_lease",
            "m20261125_000004_add_auth_jwt_replica_grants",
        ] {
            let migration = crate::migrations::migrations().into_iter()
                .find(|migration| migration.name() == name).unwrap();
            migration.up(&SchemaManager::new(&db)).await.unwrap();
        }
        db
    }

    #[tokio::test]
    async fn emergency_revocation_fences_current_and_stale_replicas_without_file_change() {
        use crate::{decode_access_token, encode_access_token};
        use rustok_core::UserRole;
        use uuid::Uuid;
        let db = database().await;
        let fence = JwtGenerationFence::new(db.clone());
        let until = chrono::Utc::now().timestamp() + 3600;
        let first = config(ROOT, "old", OLD, None, None);
        let staged = config(ROOT, "old", OLD, Some(("new", NEW, until)), None);
        let clean = config(ROOT, "new", NEW, None, None);
        assert!(fence.revoke_key(ROOT, "access", "old").await.is_err());
        assert_eq!(fence.admit(1, &first).await.unwrap(), JwtAdmission::Admitted);
        let (live_a, a) = first.clone().into_cluster_jwt(1).unwrap();
        let (live_b, b) = first.into_cluster_jwt(1).unwrap();
        assert!(fence.renew(&a).await.unwrap());
        assert!(fence.renew(&b).await.unwrap());
        assert_eq!(fence.admit(2, &staged).await.unwrap(), JwtAdmission::Admitted);
        assert!(fence.activate(&a, 2, staged.clone()).await.unwrap());
        let issue = |cfg: &AuthConfig| encode_access_token(
            cfg, Uuid::new_v4(), Uuid::new_v4(), UserRole::Customer, Uuid::new_v4(),
        );
        let token = issue(&live_a).unwrap();
        assert!(decode_access_token(&live_b, &token).is_ok());
        assert!(fence.revoke_key(ROOT, "access", "missing").await.is_err());
        assert!(fence.revoke_key(ROOT, "unknown", "old").await.is_err());
        assert!(fence.revoke_key("wrong-host-root-for-emergency-tests-32bytes", "access", "old").await.is_err());
        let receipt = fence.revoke_key(ROOT, "access", "old").await.unwrap();
        assert!(receipt.newly_revoked);
        assert_eq!(receipt.generation, 2);
        let reopened = JwtGenerationFence::new(db.clone());
        let repeat = reopened.revoke_key(ROOT, "access", "old").await.unwrap();
        assert!(!repeat.newly_revoked);
        assert_eq!(receipt.revoked_at_ms, repeat.revoked_at_ms);
        assert!(reopened.admit(2, &staged).await.is_err());
        assert!(!reopened.renew(&a).await.unwrap());
        assert!(!reopened.renew(&b).await.unwrap());
        assert!(issue(&live_a).is_err());
        assert!(decode_access_token(&live_b, &token).is_err());
        assert!(!a.lease_readiness().unwrap().is_ready());
        let deadline = LeaseDeadline::new(ClockSample::now(), LOCAL_LEASE, fence.clock.clone()).unwrap();
        assert!(!a.install_lease(2, deadline).unwrap());
        assert_eq!(reopened.admit(3, &clean).await.unwrap(), JwtAdmission::Draining);
        db.execute_unprepared("UPDATE auth_jwt_generation_fence SET lease_until_ms = 0, replica_grants = '{}' WHERE id = 1")
            .await.unwrap();
        assert_eq!(reopened.admit(3, &clean).await.unwrap(), JwtAdmission::Admitted);
        assert!(reopened.activate(&a, 3, clean.clone()).await.unwrap());
        assert!(issue(&live_a).is_ok());
        assert!(decode_access_token(&live_a, &token).is_err());
        assert!(!reopened.renew(&b).await.unwrap());
        assert!(reopened.admit(4, &config(ROOT, "renamed", OLD, None, None)).await.is_err());
        assert!(reopened.admit(4, &staged).await.is_err());
    }

    #[tokio::test]
    async fn emergency_revocation_serializes_with_renewal_and_reports_horizon() {
        let db = database().await;
        let fence = JwtGenerationFence::new(db.clone());
        let other = JwtGenerationFence::new(db);
        let first = config(ROOT, "old", OLD, None, None);
        assert_eq!(fence.admit(1, &first).await.unwrap(), JwtAdmission::Admitted);
        let (_, publisher) = first.into_cluster_jwt(1).unwrap();
        let (renewal, revoked) = tokio::join!(
            fence.renew(&publisher),
            other.revoke_key(ROOT, "access", "old"),
        );
        let _ = renewal.unwrap();
        let receipt = revoked.unwrap();
        assert!(receipt.newly_revoked);
        assert_eq!(receipt.existing_lease_until_ms, fence.load().await.unwrap().lease_until_ms);
        assert!(!fence.renew(&publisher).await.unwrap());
        assert!(!publisher.lease_readiness().unwrap().is_ready());
    }

    #[tokio::test]
    async fn emergency_revocation_of_history_requires_removal_before_renewal() {
        let db = database().await;
        let fence = JwtGenerationFence::new(db.clone());
        let until = chrono::Utc::now().timestamp() + 3600;
        let first = config(ROOT, "new", NEW, Some(("old", OLD, until)), None);
        assert_eq!(fence.admit(1, &first).await.unwrap(), JwtAdmission::Admitted);
        let (_, publisher) = first.clone().into_cluster_jwt(1).unwrap();
        assert!(fence.renew(&publisher).await.unwrap());
        assert!(fence.revoke_key(ROOT, "access", "old").await.unwrap().newly_revoked);
        assert!(!fence.renew(&publisher).await.unwrap());
        assert!(!publisher.lease_readiness().unwrap().is_ready());
        assert!(fence.admit(2, &first).await.is_err());
        let clean = config(ROOT, "new", NEW, None, None);
        assert_eq!(fence.admit(2, &clean).await.unwrap(), JwtAdmission::Draining);
        db.execute_unprepared("UPDATE auth_jwt_generation_fence SET lease_until_ms = 0, replica_grants = '{}' WHERE id = 1")
            .await.unwrap();
        assert_eq!(fence.admit(2, &clean).await.unwrap(), JwtAdmission::Admitted);
        assert!(fence.activate(&publisher, 2, clean).await.unwrap());
        assert!(publisher.lease_readiness().unwrap().is_ready());
        assert!(fence.admit(3, &first).await.is_err());
    }

    #[tokio::test]
    async fn rolling_stage_and_promotion_wait_for_old_replica_grants() {
        use crate::{decode_access_token, encode_access_token};
        use rustok_core::UserRole;
        use uuid::Uuid;
        let db = database().await;
        let fence = JwtGenerationFence::new(db.clone());
        let until = chrono::Utc::now().timestamp() + 3600;
        let first = config(ROOT, "old", OLD, None, None);
        let staged = config(ROOT, "old", OLD, Some(("new", NEW, until)), None);
        let promoted = config(ROOT, "new", NEW, Some(("old", OLD, until)), None);
        assert_eq!(fence.admit(1, &first).await.unwrap(), JwtAdmission::Admitted);
        let (live_a, a) = first.clone().into_cluster_jwt(1).unwrap();
        let (live_b, b) = first.into_cluster_jwt(1).unwrap();
        assert_ne!(a.incarnation(), b.incarnation());
        assert!(fence.renew(&a).await.unwrap());
        assert!(fence.renew(&b).await.unwrap());
        let issue = |cfg: &AuthConfig| encode_access_token(
            cfg, Uuid::new_v4(), Uuid::new_v4(), UserRole::Customer, Uuid::new_v4(),
        ).unwrap();
        let old = issue(&live_a);
        assert_eq!(fence.admit(2, &staged).await.unwrap(), JwtAdmission::Admitted);
        assert!(fence.activate(&a, 2, staged.clone()).await.unwrap());
        assert!(decode_access_token(&live_b, &issue(&live_a)).is_ok());
        assert!(!fence.renew(&b).await.unwrap());
        assert_eq!(fence.admit(3, &promoted).await.unwrap(), JwtAdmission::AwaitingReplicas);
        assert!(fence.renew(&a).await.unwrap());
        assert!(fence.activate(&b, 2, staged).await.unwrap());
        // Both incarnations have now applied the staged verifier set.
        // The post-publication ACK removed only their own previous grants.
        let stored = fence.load().await.unwrap();
        let grants = stored.live_grants().unwrap();
        assert_eq!(grants.len(), 2);
        assert!(grants.values().all(|grant| grant.generation == 2));
        assert_eq!(fence.admit(3, &promoted).await.unwrap(), JwtAdmission::Admitted);
        assert!(fence.activate(&a, 3, promoted.clone()).await.unwrap());
        let fresh = issue(&live_a);
        assert_eq!(jsonwebtoken::decode_header(&fresh).unwrap().kid.as_deref(), Some("new"));
        assert!(decode_access_token(&live_b, &fresh).is_ok());
        assert!(decode_access_token(&live_a, &issue(&live_b)).is_ok());
        assert!(decode_access_token(&live_a, &old).is_ok());
        assert!(fence.activate(&b, 3, promoted).await.unwrap());
        assert!(decode_access_token(&live_b, &fresh).is_ok());
    }

    #[tokio::test]
    async fn candidate_grant_is_not_an_ack_until_local_publication() {
        let db = database().await;
        let fence = JwtGenerationFence::new(db.clone());
        let until = chrono::Utc::now().timestamp() + 3600;
        let first = config(ROOT, "old", OLD, None, None);
        let staged = config(ROOT, "old", OLD, Some(("new", NEW, until)), None);
        let promoted = config(ROOT, "new", NEW, Some(("old", OLD, until)), None);
        assert_eq!(fence.admit(1, &first).await.unwrap(), JwtAdmission::Admitted);
        let (_, a) = first.clone().into_cluster_jwt(1).unwrap();
        let (_, b) = first.into_cluster_jwt(1).unwrap();
        assert!(fence.renew(&a).await.unwrap());
        assert!(fence.renew(&b).await.unwrap());
        assert_eq!(fence.admit(2, &staged).await.unwrap(), JwtAdmission::Admitted);

        // Reserve the next lease but do not install it: this cannot remove
        // either previous grant or authorize the promotion.
        let mac = FenceMac::new(staged.secret()).unwrap();
        let snapshot = staged.fence_snapshot(|identity| mac.identity(identity)).unwrap();
        assert!(fence.grant(&a, 2, snapshot, &mac, false).await.unwrap().is_some());
        assert_eq!(a.generation().unwrap(), 1);
        assert_eq!(fence.admit(3, &promoted).await.unwrap(), JwtAdmission::AwaitingReplicas);
        assert!(fence.activate(&a, 2, staged).await.unwrap());
        assert_eq!(fence.admit(3, &promoted).await.unwrap(), JwtAdmission::AwaitingReplicas);
        let stored = fence.load().await.unwrap();
        let grants = stored.live_grants().unwrap();
        assert!(grants.values().any(|grant| grant.generation == 1));
        // A disconnected B cannot ACK; it stops locally and its old grant
        // remains a barrier until the conservative database deadline.
        b.revoke_lease();
        let mut remaining = grants;
        remaining.retain(|_, grant| grant.generation != 1);
        let horizon = remaining.values().map(|grant| grant.until_ms).max().unwrap_or(0);
        db.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "UPDATE auth_jwt_generation_fence SET replica_grants = ?1, lease_until_ms = ?2 WHERE id = 1",
            vec![serde_json::to_string(&remaining).unwrap().into(), horizon.into()],
        )).await.unwrap();
        assert_eq!(fence.admit(3, &promoted).await.unwrap(), JwtAdmission::Admitted);
    }

    #[tokio::test]
    async fn cluster_drain_fences_old_holders_before_advancing() {
        use crate::{decode_access_token, encode_access_token};
        use rustok_core::UserRole;
        use uuid::Uuid;
        let db = database().await;
        let fence = JwtGenerationFence::new(db.clone());
        let first = config(ROOT, "old", OLD, None, None);
        let second = config(ROOT, "new", NEW, None, None);
        assert_eq!(fence.admit(1, &first).await.unwrap(), JwtAdmission::Admitted);
        let (live_a, a) = first.clone().into_cluster_jwt(1).unwrap();
        let (live_b, b) = first.into_cluster_jwt(1).unwrap();
        let issue = |cfg: &AuthConfig| encode_access_token(
            cfg, Uuid::new_v4(), Uuid::new_v4(), UserRole::Customer, Uuid::new_v4(),
        );
        assert!(issue(&live_a).is_err());
        assert!(fence.renew(&a).await.unwrap());
        assert!(fence.renew(&b).await.unwrap());
        let old = issue(&live_a).unwrap();
        assert!(decode_access_token(&live_b, &old).is_ok());
        assert_eq!(fence.admit(2, &second).await.unwrap(), JwtAdmission::Draining);
        assert!(!fence.renew(&a).await.unwrap());
        assert!(!fence.renew(&b).await.unwrap());
        // Simulate all local leases and the conservative DB deadline expiring.
        a.revoke_lease();
        b.revoke_lease();
        db.execute_unprepared("UPDATE auth_jwt_generation_fence SET lease_until_ms = 0, replica_grants = '{}' WHERE id = 1")
            .await.unwrap();
        assert!(issue(&live_a).is_err());
        assert!(decode_access_token(&live_b, &old).is_err());
        assert_eq!(fence.admit(2, &second).await.unwrap(), JwtAdmission::Admitted);
        assert!(!fence.renew(&a).await.unwrap());
        a.publish(2, second.clone()).unwrap();
        assert!(issue(&live_a).is_err());
        assert!(fence.renew(&a).await.unwrap());
        assert!(issue(&live_a).is_ok());
        assert!(decode_access_token(&live_a, &old).is_err());
        assert!(issue(&live_b).is_err());
        b.publish(2, second).unwrap();
        assert!(fence.renew(&b).await.unwrap());
        assert!(decode_access_token(&live_b, &issue(&live_a).unwrap()).is_ok());
    }

    #[tokio::test]
    async fn static_lease_blocks_first_enrollment_and_expired_drain_recovers() {
        let db = database().await;
        let fence = JwtGenerationFence::new(db.clone());
        let first = config(ROOT, "old", OLD, None, None);
        let (_, static_runtime) = AuthConfig::new(ROOT.into()).into_cluster_jwt(0).unwrap();
        assert!(fence.renew(&static_runtime).await.unwrap());
        assert_eq!(fence.admit(1, &first).await.unwrap(), JwtAdmission::Draining);
        assert!(!fence.renew(&static_runtime).await.unwrap());
        db.execute_unprepared("UPDATE auth_jwt_generation_fence SET drain_until_ms = 0 WHERE id = 1")
            .await.unwrap();
        assert!(fence.renew(&static_runtime).await.unwrap());
        assert_eq!(fence.admit(1, &first).await.unwrap(), JwtAdmission::Draining);
        static_runtime.revoke_lease();
        db.execute_unprepared("UPDATE auth_jwt_generation_fence SET lease_until_ms = 0, replica_grants = '{}' WHERE id = 1")
            .await.unwrap();
        assert_eq!(fence.admit(1, &first).await.unwrap(), JwtAdmission::Admitted);
        assert!(!fence.renew(&static_runtime).await.unwrap());
        assert!(fence.ensure_static_allowed().await.is_err());
    }

    #[tokio::test]
    async fn persists_floor_and_tombstones_across_store_recreation() {
        let db = database().await;
        let first = config(ROOT, "old", OLD, None, None);
        let second = config(ROOT, "new", NEW, None, None);
        let fence = JwtGenerationFence::new(db.clone());
        assert!(fence.ensure_static_allowed().await.is_ok());
        fence.admit(1, &first).await.unwrap();
        fence.admit(2, &second).await.unwrap();
        drop(fence);
        let restarted = JwtGenerationFence::new(db);
        restarted.admit(2, &second).await.unwrap();
        assert!(restarted.admit(1, &first).await.is_err());
        assert!(restarted.admit(2, &first).await.is_err());
        assert!(restarted.ensure_static_allowed().await.is_err());
        assert!(restarted.admit(3, &first).await.is_err());
        assert!(restarted.admit(3, &config(ROOT, "renamed", OLD, None, None)).await.is_err());
        assert!(restarted.admit(3, &config(ROOT, "old", THIRD, None, None)).await.is_err());
        assert!(restarted.admit(3, &config("different-host-root-for-fence-tests-32bytes", "new", NEW, None, None)).await.is_err());
        restarted.admit(3, &config(ROOT, "third", THIRD, None, None)).await.unwrap();
        let row = restarted.load().await.unwrap();
        assert!(!row.payload.contains(OLD) && !row.payload.contains(NEW) && !row.payload.contains(ROOT));
    }

    #[tokio::test]
    async fn staged_keys_promote_once_and_retirement_is_terminal() {
        let fence = JwtGenerationFence::new(database().await);
        let until = chrono::Utc::now().timestamp() + 3600;
        let first = config(ROOT, "old", OLD, None, Some(("old", until)));
        fence.admit(1, &first).await.unwrap();
        let staged = config(ROOT, "old", OLD, Some(("new", NEW, until)), Some(("old", until)));
        fence.admit(2, &staged).await.unwrap();
        let extended = config(ROOT, "old", OLD, Some(("new", NEW, until + 1)), Some(("old", until)));
        assert!(fence.admit(3, &extended).await.is_err());
        let promoted = config(ROOT, "new", NEW, Some(("old", OLD, until)), None);
        fence.admit(3, &promoted).await.unwrap();
        assert!(fence.admit(4, &staged).await.is_err());
        let reenabled = config(ROOT, "new", NEW, Some(("old", OLD, until)), Some(("old", until)));
        assert!(fence.admit(4, &reenabled).await.is_err());
        let demoted = config(ROOT, "old", OLD, Some(("new", NEW, until)), None);
        assert!(fence.admit(4, &demoted).await.is_err());
        fence.admit(4, &config(ROOT, "new", NEW, None, None)).await.unwrap();
        assert!(fence.admit(5, &promoted).await.is_err());
    }

    #[test]
    fn same_reuse_identity_cannot_change_exact_verifier() {
        let config = config(ROOT, "old", OLD, None, None);
        let mac = FenceMac::new(config.secret()).unwrap();
        let current = config.fence_snapshot(|identity| mac.identity(identity)).unwrap();
        let mut next = current.clone();
        next.get_mut("JWT").unwrap().keys.get_mut("old").unwrap()
            .verification_identity = "changed-exact-verifier".into();
        assert!(current != next);
        assert!(Ledger::enroll(1, current).advance(2, next, 1).is_err());
    }

    #[tokio::test]
    async fn missing_or_tampered_authority_is_not_reenrolled() {
        let db = database().await;
        let fence = JwtGenerationFence::new(db.clone());
        let first = config(ROOT, "old", OLD, None, None);
        fence.admit(1, &first).await.unwrap();
        db.execute_unprepared(
            "UPDATE auth_jwt_generation_fence SET tag = '0000000000000000000000000000000000000000000000000000000000000000' WHERE id = 1",
        ).await.unwrap();
        assert!(matches!(fence.admit(1, &first).await, Err(AuthError::Internal(_))));
        assert!(fence.ensure_static_allowed().await.is_err());
        db.execute_unprepared("DELETE FROM auth_jwt_generation_fence WHERE id = 1")
            .await.unwrap();
        assert!(fence.ensure_static_allowed().await.is_err());
        assert!(matches!(fence.admit(2, &first).await, Err(AuthError::Internal(_))));
    }

    #[tokio::test]
    async fn pre_enrollment_history_cannot_become_a_signer() {
        let fence = JwtGenerationFence::new(database().await);
        let until = chrono::Utc::now().timestamp() + 3600;
        let first = config(ROOT, "old", OLD, Some(("new", NEW, until)), None);
        fence.admit(1, &first).await.unwrap();
        let promoted = config(ROOT, "new", NEW, Some(("old", OLD, until)), None);
        assert!(fence.admit(2, &promoted).await.is_err());
    }

    #[tokio::test]
    async fn concurrent_same_generation_conflict_has_one_authority() {
        let db = database().await;
        let a = JwtGenerationFence::new(db.clone());
        let b = JwtGenerationFence::new(db);
        let first = config(ROOT, "one", OLD, None, None);
        let second = config(ROOT, "two", NEW, None, None);
        let (left, right) = tokio::join!(a.admit(1, &first), b.admit(1, &second));
        assert_ne!(left.is_ok(), right.is_ok());
        if left.is_ok() {
            a.admit(1, &first).await.unwrap();
            assert!(b.admit(1, &second).await.is_err());
        } else {
            b.admit(1, &second).await.unwrap();
            assert!(a.admit(1, &first).await.is_err());
        }
    }
}
