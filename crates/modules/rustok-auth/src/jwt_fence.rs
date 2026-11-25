//! Durable admission of complete JWT generations. No key material is persisted.
use std::collections::BTreeMap;

use hkdf::Hkdf;
use hmac::{Hmac, KeyInit, Mac};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::{AuthConfig, AuthError, JwtAlgorithm};
use crate::error::Result;

const MAX_ENROLLED_KEYS: usize = 8192;
const MAX_LEDGER_BYTES: usize = 4 * 1024 * 1024;
const MAX_CAS_ATTEMPTS: usize = 8;

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
struct SeenKey {
    identity: String,
    ever_active: bool,
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
    fn enroll(generation: u64, current: Snapshot) -> Self {
        let seen = current.iter().map(|(purpose, ring)| {
            let keys = ring.keys.iter().map(|(kid, key)| {
                // Pre-enrollment history is not evidence of an unused signer.
                (kid.clone(), SeenKey { identity: key.identity.clone(), ever_active: true })
            }).collect();
            (purpose.clone(), keys)
        }).collect();
        Self { generation, current, seen }
    }

    fn advance(mut self, generation: u64, next: Snapshot, now: i64) -> Result<Self> {
        if generation <= self.generation || next.len() != self.current.len() {
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
                        if key.identity != binding.identity || key.identity != before.identity {
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
                        identity: key.identity.clone(), ever_active: false,
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
    fn new(config: &AuthConfig) -> Result<Self> {
        let hkdf = Hkdf::<Sha256>::new(
            Some(b"rustok/auth/jwt-generation-fence/v1"), config.secret().as_bytes(),
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
pub struct JwtGenerationFence {
    db: DatabaseConnection,
}

struct StoredFence {
    generation: i64,
    payload: String,
    tag: String,
}

impl StoredFence {
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
        Self { db }
    }

    async fn load(&self) -> Result<StoredFence> {
        let backend = self.db.get_database_backend();
        if !matches!(backend, DbBackend::Postgres | DbBackend::Sqlite) {
            return Err(unavailable());
        }
        let row = self.db.query_one_raw(Statement::from_string(
            backend,
            "SELECT generation, payload, tag FROM auth_jwt_generation_fence WHERE id = 1".to_string(),
        )).await.map_err(|_| unavailable())?.ok_or_else(unavailable)?;
        Ok(StoredFence {
            generation: row.try_get("", "generation").map_err(|_| unavailable())?,
            payload: row.try_get("", "payload").map_err(|_| unavailable())?,
            tag: row.try_get("", "tag").map_err(|_| unavailable())?,
        })
    }

    /// Refuse configuration-source downgrade after the first file enrollment.
    pub async fn ensure_static_allowed(&self) -> Result<()> {
        if self.load().await?.is_empty() { Ok(()) } else { Err(rejected()) }
    }

    /// Admit before local publication. Same generation is idempotent only for
    /// the same canonical snapshot, including cutoffs and legacy mapping.
    pub async fn admit(&self, generation: u64, candidate: &AuthConfig) -> Result<()> {
        let generation_sql = i64::try_from(generation).map_err(|_| rejected())?;
        if generation_sql <= 0 {
            return Err(rejected());
        }
        crate::validate_auth_config(candidate)?;
        let mac = FenceMac::new(candidate)?;
        let snapshot = candidate.fence_snapshot(|identity| mac.identity(identity))?;
        for _ in 0..MAX_CAS_ATTEMPTS {
            let stored = self.load().await?;
            let next = match stored.ledger(&mac)? {
                None => Ledger::enroll(generation, snapshot.clone()),
                Some(ledger) if generation == ledger.generation => {
                    return if snapshot == ledger.current { Ok(()) } else { Err(rejected()) };
                }
                Some(ledger) => ledger.advance(generation, snapshot.clone(), chrono::Utc::now().timestamp())?,
            };
            let payload = serde_json::to_string(&next).map_err(|_| unavailable())?;
            if payload.len() > MAX_LEDGER_BYTES {
                return Err(rejected());
            }
            let tag = hex::encode(mac.state_mac(&payload)?.finalize().into_bytes());
            let backend = self.db.get_database_backend();
            let sql = match backend {
                DbBackend::Postgres => "UPDATE auth_jwt_generation_fence SET generation = $1, payload = $2, tag = $3 WHERE id = 1 AND generation = $4",
                DbBackend::Sqlite => "UPDATE auth_jwt_generation_fence SET generation = ?1, payload = ?2, tag = ?3 WHERE id = 1 AND generation = ?4",
                _ => return Err(unavailable()),
            };
            let updated = self.db.execute_raw(Statement::from_sql_and_values(
                backend, sql, vec![generation_sql.into(), payload.into(), tag.into(), stored.generation.into()],
            )).await.map_err(|_| unavailable())?;
            if updated.rows_affected() == 1 {
                return Ok(());
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

    const ROOT: &str = "stable-host-root-for-fence-tests-only-32bytes";
    const OLD: &str = "first-independent-signing-key-at-least-32-bytes";
    const NEW: &str = "second-independent-signing-key-at-least-32-bytes";
    const THIRD: &str = "third-independent-signing-key-at-least-32-bytes";

    fn config(root: &str, kid: &str, material: &str, history: Option<(&str, &str, i64)>,
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

    async fn database() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let migration = crate::migrations::migrations().into_iter()
            .find(|migration| migration.name() == "m20261125_000002_create_auth_jwt_generation_fence")
            .unwrap();
        migration.up(&SchemaManager::new(&db)).await.unwrap();
        db
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
