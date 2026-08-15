use std::{collections::HashSet, fmt, fs::File, io::Read, path::Path, sync::Arc, time::Duration};

use rustok_comments::{
    CommentsTcpDelegationKeyId, CommentsTcpDelegationKeyringProvider,
    CommentsTcpDelegationSchedule, CommentsTcpDelegationScheduledKey, CommentsTcpDelegationSecret,
    DEFAULT_COMMENTS_TCP_DELEGATION_CLOCK_SKEW_MS, MAX_COMMENTS_TCP_DELEGATION_KEYS,
    MAX_COMMENTS_TCP_DELEGATION_PROPAGATION_BUDGET_MS, MAX_COMMENTS_TCP_DELEGATION_TTL_MS,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::keyring;

pub const COMMENTS_TCP_DELEGATION_SCHEDULE_PERSISTENCE_SCHEMA_VERSION: u16 = 1;

const COMMENTS_TCP_DELEGATION_SCHEDULE_FILE_SCHEMA_VERSION: u16 = 2;
const COMMENTS_TCP_DELEGATION_SCHEDULE_DIGEST_DOMAIN: &[u8] =
    b"rustok-comments-tcp-delegation-schedule-state-v1\0";

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct CommentsTcpDelegationScheduleDigest([u8; 32]);

impl CommentsTcpDelegationScheduleDigest {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }
}

impl fmt::Debug for CommentsTcpDelegationScheduleDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CommentsTcpDelegationScheduleDigest([CONFIGURED])")
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct CommentsTcpDelegationSchedulePersistenceRecord {
    schema_version: u16,
    source: keyring::CommentsTcpDelegationKeyringSource,
    generation: u64,
    schedule_digest: CommentsTcpDelegationScheduleDigest,
}

impl CommentsTcpDelegationSchedulePersistenceRecord {
    pub fn schema_version(&self) -> u16 {
        self.schema_version
    }

    pub fn source(&self) -> keyring::CommentsTcpDelegationKeyringSource {
        self.source
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn schedule_digest(&self) -> CommentsTcpDelegationScheduleDigest {
        self.schedule_digest
    }

    pub(super) fn from_prepared(candidate: &PreparedScheduleCandidate) -> Self {
        Self {
            schema_version: COMMENTS_TCP_DELEGATION_SCHEDULE_PERSISTENCE_SCHEMA_VERSION,
            source: candidate.source,
            generation: candidate.generation,
            schedule_digest: candidate.digest,
        }
    }
}

impl fmt::Debug for CommentsTcpDelegationSchedulePersistenceRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommentsTcpDelegationSchedulePersistenceRecord")
            .field("schema_version", &self.schema_version)
            .field("source", &self.source)
            .field("generation", &self.generation)
            .field("schedule_digest", &"[CONFIGURED]")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommentsTcpDelegationSchedulePersistenceStoreError {
    Conflict,
    Unavailable,
}

/// Host-owned durable state boundary for one Comments delegation schedule.
///
/// A conforming implementation must provide linearizable process-visible operations.
/// `compare_and_store` must durably commit the candidate before returning `Ok(())`.
/// Any returned error must guarantee that the durable state remains exactly unchanged.
pub trait CommentsTcpDelegationSchedulePersistenceStore: Send + Sync {
    fn verify_current(
        &self,
        expected: &CommentsTcpDelegationSchedulePersistenceRecord,
    ) -> std::result::Result<(), CommentsTcpDelegationSchedulePersistenceStoreError>;

    fn compare_and_store(
        &self,
        expected: Option<&CommentsTcpDelegationSchedulePersistenceRecord>,
        candidate: &CommentsTcpDelegationSchedulePersistenceRecord,
    ) -> std::result::Result<(), CommentsTcpDelegationSchedulePersistenceStoreError>;
}

pub type SharedCommentsTcpDelegationSchedulePersistenceStore =
    Arc<dyn CommentsTcpDelegationSchedulePersistenceStore>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommentsTcpDelegationSchedulePersistenceStartupMode {
    BootstrapEmpty,
    ResumeExact,
}

#[derive(Clone)]
pub struct CommentsTcpDelegationSchedulePersistenceKey {
    key_id: String,
    secret: String,
    activates_at_unix_ms: u64,
    retires_at_unix_ms: Option<u64>,
}

impl CommentsTcpDelegationSchedulePersistenceKey {
    pub fn new(
        key_id: impl Into<String>,
        secret: impl Into<String>,
        activates_at_unix_ms: u64,
        retires_at_unix_ms: Option<u64>,
    ) -> std::result::Result<Self, String> {
        let key_id = key_id.into();
        let secret = secret.into();
        let typed_key_id = CommentsTcpDelegationKeyId::new(&key_id).map_err(|error| {
            format!("Comments TCP persisted schedule key ID is invalid: {error}")
        })?;
        let typed_secret = CommentsTcpDelegationSecret::new(&secret).map_err(|error| {
            format!("Comments TCP persisted schedule secret is invalid: {error}")
        })?;
        CommentsTcpDelegationScheduledKey::new(
            typed_key_id,
            typed_secret,
            activates_at_unix_ms,
            retires_at_unix_ms,
        )
        .map_err(|error| {
            format!("Comments TCP persisted schedule key lifecycle is invalid: {error}")
        })?;
        Ok(Self {
            key_id,
            secret,
            activates_at_unix_ms,
            retires_at_unix_ms,
        })
    }
}

impl fmt::Debug for CommentsTcpDelegationSchedulePersistenceKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommentsTcpDelegationSchedulePersistenceKey")
            .field("key_id", &"[CONFIGURED]")
            .field("secret", &"[REDACTED]")
            .field("activates_at_unix_ms", &self.activates_at_unix_ms)
            .field("retires_at_unix_ms", &self.retires_at_unix_ms)
            .finish()
    }
}

/// Canonical source document; a programmatic precomputed digest is intentionally not accepted.
#[derive(Clone)]
pub struct CommentsTcpDelegationSchedulePersistenceDocument {
    generation: u64,
    propagation_budget_ms: u64,
    legacy_unkeyed_key_id: Option<String>,
    keys: Vec<CommentsTcpDelegationSchedulePersistenceKey>,
}

impl CommentsTcpDelegationSchedulePersistenceDocument {
    pub fn new(
        generation: u64,
        propagation_budget: Duration,
        keys: Vec<CommentsTcpDelegationSchedulePersistenceKey>,
        legacy_unkeyed_key_id: Option<String>,
    ) -> std::result::Result<Self, String> {
        if generation == 0 {
            return Err(
                "Comments TCP persisted schedule generation must be greater than zero".to_string(),
            );
        }
        if keys.is_empty() || keys.len() > MAX_COMMENTS_TCP_DELEGATION_KEYS {
            return Err(format!(
                "Comments TCP persisted schedule must contain 1..={MAX_COMMENTS_TCP_DELEGATION_KEYS} keys"
            ));
        }
        let propagation_budget_ms =
            u64::try_from(propagation_budget.as_millis()).map_err(|_| {
                "Comments TCP persisted schedule propagation budget is invalid".to_string()
            })?;
        if propagation_budget_ms == 0
            || propagation_budget_ms > MAX_COMMENTS_TCP_DELEGATION_PROPAGATION_BUDGET_MS
        {
            return Err(format!(
                "Comments TCP persisted schedule propagation budget must be within 1..={MAX_COMMENTS_TCP_DELEGATION_PROPAGATION_BUDGET_MS} milliseconds"
            ));
        }

        let mut ids = HashSet::with_capacity(keys.len());
        let mut activations = HashSet::with_capacity(keys.len());
        for key in &keys {
            if !ids.insert(key.key_id.as_str()) {
                return Err("Comments TCP persisted schedule key IDs must be unique".to_string());
            }
            if !activations.insert(key.activates_at_unix_ms) {
                return Err(
                    "Comments TCP persisted schedule activation timestamps must be unique"
                        .to_string(),
                );
            }
        }
        if let Some(legacy_key_id) = legacy_unkeyed_key_id.as_deref() {
            CommentsTcpDelegationKeyId::new(legacy_key_id).map_err(|error| {
                format!("Comments TCP persisted legacy key ID is invalid: {error}")
            })?;
            if !ids.contains(legacy_key_id) {
                return Err(
                    "Comments TCP persisted legacy key ID must exist in the schedule".to_string(),
                );
            }
        }

        Ok(Self {
            generation,
            propagation_budget_ms,
            legacy_unkeyed_key_id,
            keys,
        })
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn scheduled_key_count(&self) -> usize {
        self.keys.len()
    }

    pub(super) fn prepare(
        mut self,
        source: keyring::CommentsTcpDelegationKeyringSource,
        max_ttl: Duration,
    ) -> std::result::Result<PreparedScheduleCandidate, String> {
        let max_ttl_ms = u64::try_from(max_ttl.as_millis())
            .map_err(|_| "Comments TCP persisted schedule TTL is invalid".to_string())?;
        if max_ttl_ms == 0 || max_ttl_ms > MAX_COMMENTS_TCP_DELEGATION_TTL_MS {
            return Err(format!(
                "Comments TCP persisted schedule TTL must be within 1..={MAX_COMMENTS_TCP_DELEGATION_TTL_MS} milliseconds"
            ));
        }

        self.keys.sort_by(|left, right| {
            left.activates_at_unix_ms
                .cmp(&right.activates_at_unix_ms)
                .then_with(|| left.key_id.cmp(&right.key_id))
        });

        let digest = canonical_schedule_digest(
            self.propagation_budget_ms,
            max_ttl_ms,
            DEFAULT_COMMENTS_TCP_DELEGATION_CLOCK_SKEW_MS,
            self.legacy_unkeyed_key_id.as_deref(),
            &self.keys,
        )?;

        let mut scheduled_keys = Vec::with_capacity(self.keys.len());
        for key in self.keys {
            let key_id = CommentsTcpDelegationKeyId::new(&key.key_id).map_err(|error| {
                format!("Comments TCP persisted schedule key ID is invalid: {error}")
            })?;
            let secret = CommentsTcpDelegationSecret::new(&key.secret).map_err(|error| {
                format!("Comments TCP persisted schedule secret is invalid: {error}")
            })?;
            scheduled_keys.push(
                CommentsTcpDelegationScheduledKey::new(
                    key_id,
                    secret,
                    key.activates_at_unix_ms,
                    key.retires_at_unix_ms,
                )
                .map_err(|error| {
                    format!("Comments TCP persisted schedule key lifecycle is invalid: {error}")
                })?,
            );
        }

        let mut schedule = CommentsTcpDelegationSchedule::new(
            scheduled_keys,
            Duration::from_millis(self.propagation_budget_ms),
            Duration::from_millis(max_ttl_ms),
            Duration::from_millis(DEFAULT_COMMENTS_TCP_DELEGATION_CLOCK_SKEW_MS),
        )
        .map_err(|error| {
            format!("Comments TCP persisted delegation schedule is invalid: {error}")
        })?;
        if let Some(legacy_key_id) = self.legacy_unkeyed_key_id {
            schedule = schedule
                .with_legacy_unkeyed_key_id(
                    CommentsTcpDelegationKeyId::new(legacy_key_id).map_err(|error| {
                        format!("Comments TCP persisted legacy key ID is invalid: {error}")
                    })?,
                )
                .map_err(|error| {
                    format!("Comments TCP persisted legacy key selection is invalid: {error}")
                })?;
        }
        schedule.current_keyring().map_err(|_| {
            "Comments TCP persisted schedule must have one active signing key at composition time"
                .to_string()
        })?;

        Ok(PreparedScheduleCandidate {
            schedule,
            source,
            generation: self.generation,
            digest,
        })
    }
}

impl fmt::Debug for CommentsTcpDelegationSchedulePersistenceDocument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommentsTcpDelegationSchedulePersistenceDocument")
            .field("generation", &self.generation)
            .field("propagation_budget_ms", &self.propagation_budget_ms)
            .field("scheduled_key_count", &self.keys.len())
            .field(
                "legacy_unkeyed_enabled",
                &self.legacy_unkeyed_key_id.is_some(),
            )
            .field("key_ids", &"[REDACTED]")
            .field("secrets", &"[REDACTED]")
            .finish()
    }
}

pub(super) struct PreparedScheduleCandidate {
    pub(super) schedule: CommentsTcpDelegationSchedule,
    pub(super) source: keyring::CommentsTcpDelegationKeyringSource,
    pub(super) generation: u64,
    pub(super) digest: CommentsTcpDelegationScheduleDigest,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PersistedScheduleFileDocument {
    schema_version: u16,
    generation: u64,
    propagation_budget_ms: u64,
    #[serde(default)]
    legacy_unkeyed_key_id: Option<String>,
    keys: Vec<PersistedScheduleFileKey>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PersistedScheduleFileKey {
    key_id: String,
    secret: String,
    activates_at_unix_ms: u64,
    #[serde(default)]
    retires_at_unix_ms: Option<u64>,
}

pub(super) fn load_prepared_schedule_from_file(
    file_path: &Path,
    max_ttl: Duration,
) -> std::result::Result<PreparedScheduleCandidate, String> {
    let bytes = read_bounded_schedule_file(file_path)?;
    let document =
        serde_json::from_slice::<PersistedScheduleFileDocument>(&bytes).map_err(|_| {
            format!(
                "{} must contain one valid version-2 Comments TCP delegation schedule JSON object",
                keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
            )
        })?;
    if document.schema_version != COMMENTS_TCP_DELEGATION_SCHEDULE_FILE_SCHEMA_VERSION {
        return Err(format!(
            "{} schema_version must equal {COMMENTS_TCP_DELEGATION_SCHEDULE_FILE_SCHEMA_VERSION} in persisted schedule mode",
            keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
        ));
    }

    let mut keys = Vec::with_capacity(document.keys.len());
    for key in document.keys {
        keys.push(CommentsTcpDelegationSchedulePersistenceKey::new(
            key.key_id,
            key.secret,
            key.activates_at_unix_ms,
            key.retires_at_unix_ms,
        )?);
    }
    CommentsTcpDelegationSchedulePersistenceDocument::new(
        document.generation,
        Duration::from_millis(document.propagation_budget_ms),
        keys,
        document.legacy_unkeyed_key_id,
    )?
    .prepare(keyring::CommentsTcpDelegationKeyringSource::File, max_ttl)
}

fn read_bounded_schedule_file(file_path: &Path) -> std::result::Result<Vec<u8>, String> {
    if file_path.as_os_str().is_empty() {
        return Err(format!(
            "{} must reference a non-empty file path",
            keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
        ));
    }
    let mut file = File::open(file_path).map_err(|_| {
        format!(
            "{} could not be opened",
            keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
        )
    })?;
    let metadata = file.metadata().map_err(|_| {
        format!(
            "{} metadata could not be read",
            keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
        )
    })?;
    if !metadata.is_file() {
        return Err(format!(
            "{} must reference a regular file",
            keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
        ));
    }
    if metadata.len() == 0
        || metadata.len() > keyring::MAX_COMMENTS_TCP_DELEGATION_KEYRING_FILE_BYTES as u64
    {
        return Err(format!(
            "{} file size must be within 1..={} bytes",
            keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV,
            keyring::MAX_COMMENTS_TCP_DELEGATION_KEYRING_FILE_BYTES
        ));
    }

    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.by_ref()
        .take((keyring::MAX_COMMENTS_TCP_DELEGATION_KEYRING_FILE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| {
            format!(
                "{} could not be read",
                keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
            )
        })?;
    if bytes.is_empty() || bytes.len() > keyring::MAX_COMMENTS_TCP_DELEGATION_KEYRING_FILE_BYTES {
        return Err(format!(
            "{} file size must be within 1..={} bytes",
            keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV,
            keyring::MAX_COMMENTS_TCP_DELEGATION_KEYRING_FILE_BYTES
        ));
    }
    Ok(bytes)
}

fn canonical_schedule_digest(
    propagation_budget_ms: u64,
    max_ttl_ms: u64,
    clock_skew_ms: u64,
    legacy_unkeyed_key_id: Option<&str>,
    keys: &[CommentsTcpDelegationSchedulePersistenceKey],
) -> std::result::Result<CommentsTcpDelegationScheduleDigest, String> {
    let mut hasher = Sha256::new();
    hasher.update(COMMENTS_TCP_DELEGATION_SCHEDULE_DIGEST_DOMAIN);
    hasher.update(propagation_budget_ms.to_be_bytes());
    hasher.update(max_ttl_ms.to_be_bytes());
    hasher.update(clock_skew_ms.to_be_bytes());
    update_optional_text(&mut hasher, legacy_unkeyed_key_id)?;
    let key_count = u16::try_from(keys.len())
        .map_err(|_| "Comments TCP persisted schedule key count is invalid".to_string())?;
    hasher.update(key_count.to_be_bytes());
    for key in keys {
        update_text(&mut hasher, &key.key_id)?;
        update_text(&mut hasher, &key.secret)?;
        hasher.update(key.activates_at_unix_ms.to_be_bytes());
        match key.retires_at_unix_ms {
            Some(retirement) => {
                hasher.update([1u8]);
                hasher.update(retirement.to_be_bytes());
            }
            None => hasher.update([0u8]),
        }
    }
    Ok(CommentsTcpDelegationScheduleDigest(
        hasher.finalize().into(),
    ))
}

fn update_optional_text(
    hasher: &mut Sha256,
    value: Option<&str>,
) -> std::result::Result<(), String> {
    match value {
        Some(value) => {
            hasher.update([1u8]);
            update_text(hasher, value)
        }
        None => {
            hasher.update([0u8]);
            Ok(())
        }
    }
}

fn update_text(hasher: &mut Sha256, value: &str) -> std::result::Result<(), String> {
    let bytes = value.as_bytes();
    let length = u32::try_from(bytes.len())
        .map_err(|_| "Comments TCP persisted schedule canonical field is too large".to_string())?;
    hasher.update(length.to_be_bytes());
    hasher.update(bytes);
    Ok(())
}
