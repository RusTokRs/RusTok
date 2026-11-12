use crate::AdminCanvasController;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeSet, HashMap};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Identity a draft session is bound to (audit §2.5).
///
/// Records carry the owner that created them; `load`, `commit`, and `remove`
/// verify the caller against it, so a leaked draft token cannot be used by
/// another editor to read, overwrite, or delete someone else's unsaved work.
/// `None` means "unattributed": records written before the field existed and
/// hosts without a user identity. Unattributed records stay loadable, and the
/// next attributed commit adopts that caller as the owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftSessionOwner {
    pub tenant_id: String,
    pub user_id: String,
}

/// Hard cap on one serialized draft session record (controller + runtime
/// context). Page Builder bounds a project at 16 MiB; this leaves headroom and
/// stops a single session from exhausting memory or disk (audit §2.5).
pub const MAX_DRAFT_SESSION_RECORD_BYTES: usize = 20 * 1024 * 1024;

/// How many draft sessions one owner may keep simultaneously; the oldest
/// same-owner session is evicted first (audit §2.5).
pub const MAX_DRAFT_SESSIONS_PER_OWNER: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SsrDraftSessionSnapshot {
    pub token: String,
    pub generation: u64,
    pub controller: AdminCanvasController,
    pub runtime_context: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<DraftSessionOwner>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersistentDraftSessionRecord {
    pub token: String,
    pub generation: u64,
    pub controller: AdminCanvasController,
    pub runtime_context: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<DraftSessionOwner>,
    pub expires_at_unix_secs: u64,
}

pub trait SsrDraftSessionStore: Send + Sync {
    /// Loads the session for `token` if it belongs to `page_id` and `owner`.
    ///
    /// Returns `OwnerMismatch` when the session is bound to a different
    /// owner, and `None` only when no live session exists for the token.
    fn load(
        &self,
        token: &str,
        page_id: &str,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<Option<SsrDraftSessionSnapshot>, SsrDraftSessionError>;

    /// Commits `controller`, keeping the runtime context already stored for
    /// the session; a new session starts with an empty context (audit §6.5).
    /// Every store in this crate honours this — use
    /// [`Self::commit_with_context`] to replace the context.
    fn commit(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError>;

    /// Commits `controller` and replaces the stored runtime context.
    ///
    /// The default implementation is for stores that keep no runtime
    /// context: it drops `runtime_context` and delegates to [`Self::commit`].
    fn commit_with_context(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
        runtime_context: Value,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError> {
        let _ = runtime_context;
        self.commit(token, expected_generation, controller, owner)
    }

    /// Deletes the session, but only when the caller owns it.
    fn remove(
        &self,
        token: &str,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<(), SsrDraftSessionError>;

    /// Removes expired sessions and enforces the capacity limits; returns
    /// how many stored entries were deleted.
    fn prune(&self) -> Result<usize, SsrDraftSessionError>;
}

#[derive(Clone)]
pub struct InMemorySsrDraftSessionStore {
    entries: Arc<RwLock<HashMap<String, DraftEntry>>>,
    ttl: Duration,
    maximum_entries: usize,
    /// Reference point for [`EntryDeadline`] values.
    epoch: Instant,
}

/// Expiry deadline of one entry, in nanoseconds since the store epoch.
///
/// Atomic so that `load` can extend a session's TTL while holding only the
/// shared map lock (audit §5.3): concurrent readers of different — or the
/// same — sessions no longer serialize on an exclusive lock.
#[derive(Debug)]
struct EntryDeadline(AtomicU64);

impl EntryDeadline {
    fn new(deadline: u64) -> Self {
        Self(AtomicU64::new(deadline))
    }

    fn get(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }

    /// Moves the deadline forward; never backwards, so a slower concurrent
    /// reader cannot shorten a TTL that a faster one already extended.
    fn extend_to(&self, deadline: u64) {
        self.0.fetch_max(deadline, Ordering::Relaxed);
    }
}

#[derive(Debug)]
struct DraftEntry {
    controller: AdminCanvasController,
    runtime_context: Value,
    generation: u64,
    owner: Option<DraftSessionOwner>,
    expires_at: EntryDeadline,
}

impl InMemorySsrDraftSessionStore {
    pub fn new(ttl: Duration, maximum_entries: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            ttl: ttl.max(Duration::from_secs(30)),
            maximum_entries: maximum_entries.max(1),
            epoch: Instant::now(),
        }
    }

    pub fn editor_default() -> Self {
        Self::new(Duration::from_secs(60 * 60 * 8), 10_000)
    }

    pub fn len(&self) -> Result<usize, SsrDraftSessionError> {
        self.entries
            .read()
            .map(|entries| entries.len())
            .map_err(|_| SsrDraftSessionError::Poisoned)
    }

    pub fn is_empty(&self) -> Result<bool, SsrDraftSessionError> {
        self.len().map(|len| len == 0)
    }

    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    pub fn maximum_entries(&self) -> usize {
        self.maximum_entries
    }

    /// Generation of the live session stored for `token`, if any.
    ///
    /// A read-only probe: unlike [`SsrDraftSessionStore::load`] it neither
    /// extends the TTL nor checks page and owner, so callers can compare
    /// cache freshness without side effects (audit §5.3).
    pub fn generation(&self, token: &str) -> Result<Option<u64>, SsrDraftSessionError> {
        let entries = self
            .entries
            .read()
            .map_err(|_| SsrDraftSessionError::Poisoned)?;
        let now = self.now();
        Ok(entries
            .get(token.trim())
            .filter(|entry| entry.expires_at.get() > now)
            .map(|entry| entry.generation))
    }

    pub fn insert_snapshot(
        &self,
        snapshot: SsrDraftSessionSnapshot,
    ) -> Result<(), SsrDraftSessionError> {
        let mut entries = self
            .entries
            .write()
            .map_err(|_| SsrDraftSessionError::Poisoned)?;
        let now = self.now();
        Self::prune_locked(&mut entries, now);
        Self::evict_oldest(&mut entries, self.maximum_entries);
        // Re-seeding a token that already exists must not evict other
        // sessions of the same owner: the per-owner count does not grow.
        if !entries.contains_key(&snapshot.token)
            && let Some(owner) = snapshot.owner.as_ref()
        {
            Self::evict_oldest_for_owner(
                &mut entries,
                owner,
                MAX_DRAFT_SESSIONS_PER_OWNER.saturating_sub(1),
            );
        }
        entries.insert(
            snapshot.token,
            DraftEntry {
                controller: snapshot.controller,
                runtime_context: snapshot.runtime_context,
                generation: snapshot.generation,
                owner: snapshot.owner,
                expires_at: EntryDeadline::new(self.deadline_from(now)),
            },
        );
        Ok(())
    }

    /// Drops the entry cached for `token` without an ownership check.
    ///
    /// For [`DurableSsrDraftSessionStore`] only. Its memory tier is a cache
    /// of the backend, so an entry the backend did not accept must not
    /// outlive the failed write; dropping a cache entry never loses a stored
    /// session.
    fn evict(&self, token: &str) -> Result<(), SsrDraftSessionError> {
        self.entries
            .write()
            .map_err(|_| SsrDraftSessionError::Poisoned)?
            .remove(token.trim());
        Ok(())
    }

    /// Nanoseconds elapsed since the store epoch (saturating after ~584 years).
    fn now(&self) -> u64 {
        u64::try_from(self.epoch.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }

    fn deadline_from(&self, now: u64) -> u64 {
        now.saturating_add(u64::try_from(self.ttl.as_nanos()).unwrap_or(u64::MAX))
    }

    fn fresh_token(entries: &HashMap<String, DraftEntry>) -> String {
        loop {
            let token = Uuid::new_v4().simple().to_string();
            if !entries.contains_key(&token) {
                return token;
            }
        }
    }

    fn empty_context() -> Value {
        Value::Object(Map::new())
    }

    fn prune_locked(entries: &mut HashMap<String, DraftEntry>, now: u64) -> usize {
        let before = entries.len();
        entries.retain(|_, entry| entry.expires_at.get() > now);
        before.saturating_sub(entries.len())
    }

    fn evict_oldest(entries: &mut HashMap<String, DraftEntry>, maximum_entries: usize) {
        while entries.len() >= maximum_entries {
            let Some(oldest) = entries
                .iter()
                .min_by_key(|(_, entry)| entry.expires_at.get())
                .map(|(token, _)| token.clone())
            else {
                break;
            };
            entries.remove(&oldest);
        }
    }

    /// Enforces the per-owner session cap: keeps at most `keep` live entries
    /// of one owner, evicting the ones expiring soonest first.
    fn evict_oldest_for_owner(
        entries: &mut HashMap<String, DraftEntry>,
        owner: &DraftSessionOwner,
        keep: usize,
    ) {
        let mut owned: Vec<(String, u64)> = entries
            .iter()
            .filter(|(_, entry)| entry.owner.as_ref() == Some(owner))
            .map(|(token, entry)| (token.clone(), entry.expires_at.get()))
            .collect();
        while owned.len() > keep {
            let Some(index) = owned
                .iter()
                .enumerate()
                .min_by_key(|(_, (_, expires_at))| *expires_at)
                .map(|(index, _)| index)
            else {
                break;
            };
            let (token, _) = owned.swap_remove(index);
            entries.remove(&token);
        }
    }

    /// Shared commit path. `runtime_context: None` keeps the context already
    /// stored for the session (an empty object for a new one); `Some`
    /// replaces it.
    fn commit_internal(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
        runtime_context: Option<Value>,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError> {
        let mut entries = self
            .entries
            .write()
            .map_err(|_| SsrDraftSessionError::Poisoned)?;
        let now = self.now();
        Self::prune_locked(&mut entries, now);

        let trimmed_token = token.map(str::trim).filter(|token| !token.is_empty());
        if let Some(token) = trimmed_token {
            let entry = entries.get_mut(token);
            if let Some(entry) = entry {
                if entry.controller.page_id() != controller.page_id() {
                    return Err(SsrDraftSessionError::PageMismatch {
                        expected: entry.controller.page_id().to_string(),
                        actual: controller.page_id().to_string(),
                    });
                }
                ensure_session_owner(entry.owner.as_ref(), owner)?;
                if expected_generation.is_some_and(|generation| generation != entry.generation) {
                    return Err(SsrDraftSessionError::GenerationConflict {
                        expected: entry.generation,
                        actual: expected_generation.unwrap_or_default(),
                    });
                }
                entry.controller = controller;
                if let Some(runtime_context) = runtime_context {
                    entry.runtime_context = runtime_context;
                }
                // An attributed commit adopts the caller as the owner
                // (records written before owner binding had none).
                if owner.is_some() {
                    entry.owner = owner.cloned();
                }
                entry.generation = entry.generation.saturating_add(1);
                entry.expires_at = EntryDeadline::new(self.deadline_from(now));
                return Ok(SsrDraftSessionSnapshot {
                    token: token.to_string(),
                    generation: entry.generation,
                    controller: entry.controller.clone(),
                    runtime_context: entry.runtime_context.clone(),
                    owner: entry.owner.clone(),
                });
            }
        }

        Self::evict_oldest(&mut entries, self.maximum_entries);
        if let Some(owner) = owner {
            Self::evict_oldest_for_owner(
                &mut entries,
                owner,
                MAX_DRAFT_SESSIONS_PER_OWNER.saturating_sub(1),
            );
        }
        let token = Self::fresh_token(&entries);
        let generation = 1;
        let runtime_context = runtime_context.unwrap_or_else(Self::empty_context);
        entries.insert(
            token.clone(),
            DraftEntry {
                controller: controller.clone(),
                runtime_context: runtime_context.clone(),
                generation,
                owner: owner.cloned(),
                expires_at: EntryDeadline::new(self.deadline_from(now)),
            },
        );
        Ok(SsrDraftSessionSnapshot {
            token,
            generation,
            controller,
            runtime_context,
            owner: owner.cloned(),
        })
    }
}

impl Default for InMemorySsrDraftSessionStore {
    fn default() -> Self {
        Self::editor_default()
    }
}

impl SsrDraftSessionStore for InMemorySsrDraftSessionStore {
    fn load(
        &self,
        token: &str,
        page_id: &str,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<Option<SsrDraftSessionSnapshot>, SsrDraftSessionError> {
        let token = token.trim();
        if token.is_empty() {
            return Ok(None);
        }
        // Shared lock only (audit §5.3): the TTL refresh goes through the
        // entry's atomic deadline. Expired entries are skipped here and
        // reclaimed by the next commit or `prune`, which hold the map
        // exclusively anyway.
        let entries = self
            .entries
            .read()
            .map_err(|_| SsrDraftSessionError::Poisoned)?;
        let now = self.now();
        let Some(entry) = entries
            .get(token)
            .filter(|entry| entry.expires_at.get() > now)
        else {
            return Ok(None);
        };
        if entry.controller.page_id() != page_id {
            return Err(SsrDraftSessionError::PageMismatch {
                expected: entry.controller.page_id().to_string(),
                actual: page_id.to_string(),
            });
        }
        ensure_session_owner(entry.owner.as_ref(), owner)?;
        entry.expires_at.extend_to(self.deadline_from(now));
        Ok(Some(SsrDraftSessionSnapshot {
            token: token.to_string(),
            generation: entry.generation,
            controller: entry.controller.clone(),
            runtime_context: entry.runtime_context.clone(),
            owner: entry.owner.clone(),
        }))
    }

    fn commit(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError> {
        self.commit_internal(token, expected_generation, controller, None, owner)
    }

    fn commit_with_context(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
        runtime_context: Value,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError> {
        self.commit_internal(
            token,
            expected_generation,
            controller,
            Some(runtime_context),
            owner,
        )
    }

    fn remove(
        &self,
        token: &str,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<(), SsrDraftSessionError> {
        let token = token.trim();
        let mut entries = self
            .entries
            .write()
            .map_err(|_| SsrDraftSessionError::Poisoned)?;
        if let Some(entry) = entries.get(token) {
            ensure_session_owner(entry.owner.as_ref(), owner)?;
            entries.remove(token);
        }
        Ok(())
    }

    fn prune(&self) -> Result<usize, SsrDraftSessionError> {
        let mut entries = self
            .entries
            .write()
            .map_err(|_| SsrDraftSessionError::Poisoned)?;
        let now = self.now();
        Ok(Self::prune_locked(&mut entries, now))
    }
}

/// Pluggable persistent storage backend for SSR draft sessions.
///
/// Backends may be local filesystem directories (`FileSsrDraftSessionStore`),
/// distributed key-value caches (`ExternalKvDraftSessionBackend` for Redis/KV),
/// or database tables.
pub trait PersistentDraftSessionBackend: Send + Sync {
    /// Loads the persistent record for the given token.
    ///
    /// `owner` is the caller's identity. Backends that partition storage by
    /// tenant (the file store, audit §2.6) use it to route the lookup and
    /// report a token held by another tenant as `OwnerMismatch`; flat
    /// backends may ignore it. Either way the caller still verifies the
    /// returned record's owner.
    fn load_record(
        &self,
        token: &str,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<Option<PersistentDraftSessionRecord>, SsrDraftSessionError>;

    /// Persists or updates the draft session record.
    fn save_record(
        &self,
        record: &PersistentDraftSessionRecord,
    ) -> Result<(), SsrDraftSessionError>;

    /// Atomically persists `new_record` only while the stored record still
    /// matches `expected` (audit §4.1).
    ///
    /// `expected: None` means "no record for the token may exist" (create);
    /// `Some(record)` means the stored record must still be that exact record
    /// (same token, generation, page, and owner). Returns `Ok(false)` — not an
    /// error — when the comparison fails: the caller lost the race and must
    /// resolve the conflict. This is the compare-and-set that makes the
    /// generation check race-free across processes and pods.
    fn compare_and_save_record(
        &self,
        expected: Option<&PersistentDraftSessionRecord>,
        new_record: &PersistentDraftSessionRecord,
    ) -> Result<bool, SsrDraftSessionError>;

    /// Deletes the record associated with the token. `owner` routes the
    /// lookup exactly as in [`Self::load_record`]; ownership itself has
    /// already been enforced by the caller.
    fn delete_record(
        &self,
        token: &str,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<(), SsrDraftSessionError>;

    /// Prunes expired records and enforces capacity limits.
    fn prune_records(
        &self,
        now_unix_secs: u64,
        max_entries: usize,
    ) -> Result<usize, SsrDraftSessionError>;
}

/// Environment variable naming the directory that holds file-backed draft
/// sessions. Unset or empty falls back to a private directory under the OS
/// temp dir (audit §2.6).
pub const DRAFT_SESSION_DIR_ENV: &str = "RUSTOK_DRAFT_SESSION_DIR";

/// Partition for sessions without an owner (hosts without a user identity).
const UNATTRIBUTED_PARTITION: &str = "_unattributed";
/// Prefix of per-tenant partitions; the encoded tenant id follows.
const TENANT_PARTITION_PREFIX: &str = "t-";
/// Directory of the per-token advisory lock files, shared by all partitions
/// because a token is unique across tenants.
const LOCKS_DIR: &str = "_locks";
/// Longest partition directory name accepted (file systems cap one path
/// component at 255 bytes).
const MAX_PARTITION_NAME_BYTES: usize = 200;
/// Upper bound on how much of the TTL a read may consume before the record is
/// rewritten to extend it.
const MAX_TTL_REFRESH_SLACK_SECS: u64 = 5 * 60;

/// Filesystem-backed draft session store.
///
/// Layout (audit §2.6): `<dir>/t-<tenant>/<token>.json` per tenant,
/// `<dir>/_unattributed/<token>.json` for sessions without an owner, and
/// `<dir>/_locks/<token>.lock`. On Unix every directory is created `0700` and
/// every file `0600`; a storage directory that is a symbolic link, or that the
/// process cannot restrict to its own user, is refused.
#[derive(Clone)]
pub struct FileSsrDraftSessionStore {
    dir: PathBuf,
    ttl: Duration,
    maximum_entries: usize,
    /// Why the storage directory could not be secured. A store in this state
    /// fails closed: every operation returns this as an I/O error instead of
    /// touching a directory it does not control.
    unavailable: Option<String>,
}

impl FileSsrDraftSessionStore {
    pub fn new(
        dir: impl Into<PathBuf>,
        ttl: Duration,
        maximum_entries: usize,
    ) -> Result<Self, SsrDraftSessionError> {
        let dir = dir.into();
        let unusable = |path: &Path, error: std::io::Error| {
            SsrDraftSessionError::Io(format!(
                "draft session directory `{}` is not usable: {error}",
                path.display()
            ))
        };
        ensure_private_dir(&dir)
            .and_then(|()| ensure_owned_by_process(&dir))
            .map_err(|error| unusable(&dir, error))?;
        let locks = dir.join(LOCKS_DIR);
        ensure_private_dir(&locks).map_err(|error| unusable(&locks, error))?;
        Ok(Self {
            dir,
            ttl: ttl.max(Duration::from_secs(30)),
            maximum_entries: maximum_entries.max(1),
            unavailable: None,
        })
    }

    /// Storage directory used by [`Self::editor_default`]: the value of
    /// [`DRAFT_SESSION_DIR_ENV`], or `<temp dir>/rustok_draft_sessions`.
    pub fn default_dir() -> PathBuf {
        Self::resolve_dir(std::env::var_os(DRAFT_SESSION_DIR_ENV))
    }

    fn resolve_dir(configured: Option<std::ffi::OsString>) -> PathBuf {
        match configured.filter(|value| !value.is_empty()) {
            Some(value) => PathBuf::from(value),
            None => std::env::temp_dir().join("rustok_draft_sessions"),
        }
    }

    /// Store rooted at [`Self::default_dir`]. When that directory cannot be
    /// secured the returned store fails closed — every operation reports the
    /// reason — rather than falling back to an unprotected location.
    pub fn editor_default() -> Self {
        let dir = Self::default_dir();
        let ttl = Duration::from_secs(60 * 60 * 8);
        let maximum_entries = 10_000;
        match Self::new(dir.clone(), ttl, maximum_entries) {
            Ok(store) => store,
            Err(error) => Self {
                dir,
                ttl,
                maximum_entries,
                unavailable: Some(match error {
                    SsrDraftSessionError::Io(reason) => reason,
                    other => other.to_string(),
                }),
            },
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn maximum_entries(&self) -> usize {
        self.maximum_entries
    }

    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    pub fn is_safe_token(token: &str) -> bool {
        !token.is_empty()
            && token.len() <= 64
            && token
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    }

    /// Whether `name` is a file the pre-partition layout kept directly in the
    /// storage root: `<token>.json`, `<token>.lock`, or a temporary file of
    /// either. Anything else in the root is not this store's to delete.
    fn is_legacy_flat_file(name: &str) -> bool {
        let Some((token, suffix)) = name.split_once('.') else {
            return false;
        };
        Self::is_safe_token(token)
            && (suffix == "json" || suffix == "lock" || suffix.starts_with("tmp."))
    }

    pub fn validate_token(token: &str) -> Result<(), SsrDraftSessionError> {
        if Self::is_safe_token(token) {
            Ok(())
        } else {
            Err(SsrDraftSessionError::InvalidToken(token.to_string()))
        }
    }

    fn ensure_available(&self) -> Result<(), SsrDraftSessionError> {
        match &self.unavailable {
            Some(reason) => Err(SsrDraftSessionError::Io(reason.clone())),
            None => Ok(()),
        }
    }

    fn with_lock<T>(
        &self,
        token: &str,
        action: impl FnOnce() -> Result<T, SsrDraftSessionError>,
    ) -> Result<T, SsrDraftSessionError> {
        self.ensure_available()?;
        with_token_lock(&self.dir.join(LOCKS_DIR), token, action)
    }

    /// Directory name of the partition holding `owner`'s sessions.
    ///
    /// The tenant id is encoded injectively into `[a-z0-9-_]`, so distinct
    /// tenants never share a directory — also on case-insensitive file
    /// systems — and no tenant id can escape the storage directory.
    fn partition_name(owner: Option<&DraftSessionOwner>) -> Result<String, SsrDraftSessionError> {
        let Some(owner) = owner else {
            return Ok(UNATTRIBUTED_PARTITION.to_string());
        };
        let mut name = String::from(TENANT_PARTITION_PREFIX);
        for byte in owner.tenant_id.bytes() {
            match byte {
                b'a'..=b'z' | b'0'..=b'9' | b'-' => name.push(char::from(byte)),
                _ => name.push_str(&format!("_{byte:02x}")),
            }
        }
        if name.len() > MAX_PARTITION_NAME_BYTES {
            return Err(SsrDraftSessionError::InvalidOwner(format!(
                "tenant id of {} bytes is too long for file-backed draft storage",
                owner.tenant_id.len()
            )));
        }
        Ok(name)
    }

    fn record_file_name(token: &str) -> String {
        format!("{token}.json")
    }

    /// Finds the record file for `token` as seen by `owner`.
    ///
    /// Looks in the caller's own tenant partition, then — for attributed
    /// callers — in the unattributed partition (such records stay loadable
    /// and are adopted by the next attributed commit). A token that exists
    /// only under another tenant yields `OwnerMismatch`, matching the other
    /// stores, and that tenant's file is never opened.
    fn locate(
        &self,
        token: &str,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<Option<PathBuf>, SsrDraftSessionError> {
        self.ensure_available()?;
        let file_name = Self::record_file_name(token);
        let own_partition = Self::partition_name(owner)?;
        let own = self.dir.join(&own_partition).join(&file_name);
        if own.is_file() {
            return Ok(Some(own));
        }
        if owner.is_some() {
            let unattributed = self.dir.join(UNATTRIBUTED_PARTITION).join(&file_name);
            if unattributed.is_file() {
                return Ok(Some(unattributed));
            }
        }
        let partitions = match fs::read_dir(&self.dir) {
            Ok(partitions) => partitions,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(SsrDraftSessionError::Io(err.to_string())),
        };
        for partition in partitions.flatten() {
            let name = partition.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if name.starts_with(TENANT_PARTITION_PREFIX)
                && name != own_partition
                && partition.path().join(&file_name).is_file()
            {
                return Err(SsrDraftSessionError::OwnerMismatch);
            }
        }
        Ok(None)
    }

    /// Reads the live record for `token` together with the file it came from.
    ///
    /// A session that is over is reported as absent: a record past its
    /// expiry, and a record that cannot be parsed in a file nobody has
    /// written for a whole TTL. Records are replaced atomically, so an
    /// unparsable one is not a write in flight; it was written by a build
    /// with another record shape, or it is damaged. While it may still be a
    /// live session of that other build it is reported as an error, not
    /// discarded. Every write stamps a fresh TTL, so after a whole TTL
    /// without a write the session has expired whatever the file contains,
    /// and the editor that still holds the token starts a new session
    /// instead of failing on every request.
    ///
    /// The file of such a session is deleted under the token lock only. A
    /// reader without it may have read the file just before a commit
    /// replaced it and would delete the new record, so for
    /// [`TokenLock::NotHeld`] the lock is taken and the file looked at again
    /// ([`Self::remove_dead_record`]). When that look finds the record
    /// replaced, the read starts over and returns the record that is there.
    fn read_located(
        &self,
        token: &str,
        owner: Option<&DraftSessionOwner>,
        lock: TokenLock,
    ) -> Result<Option<(PathBuf, PersistentDraftSessionRecord)>, SsrDraftSessionError> {
        let token = token.trim();
        if token.is_empty() || !Self::is_safe_token(token) {
            return Ok(None);
        }
        // The second pass is for a reader without the lock whose dead record
        // turned out, under the lock, to have been replaced by a commit: it
        // reads the file again and answers with what is there now.
        for _ in 0..2 {
            let Some(file_path) = self.locate(token, owner)? else {
                return Ok(None);
            };
            let content = match fs::read_to_string(&file_path) {
                Ok(content) => content,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(err) => return Err(SsrDraftSessionError::Io(err.to_string())),
            };
            let record = match serde_json::from_str::<PersistentDraftSessionRecord>(&content) {
                Ok(record) => record,
                Err(error) => {
                    if !file_is_older_than(&file_path, self.ttl) {
                        return Err(SsrDraftSessionError::Serialization(error.to_string()));
                    }
                    tracing::warn!(
                        error = %error,
                        "unreadable draft session record outlived its TTL and is treated as expired"
                    );
                    if self.collect_dead_record(&file_path, token, lock) == DeadRecord::Replaced {
                        continue;
                    }
                    return Ok(None);
                }
            };

            if record.expires_at_unix_secs <= current_unix_secs() {
                if self.collect_dead_record(&file_path, token, lock) == DeadRecord::Replaced {
                    continue;
                }
                return Ok(None);
            }
            return Ok(Some((file_path, record)));
        }
        Ok(None)
    }

    /// Deletes the file of a session [`Self::read_located`] found to be over.
    /// A leftover is retried by the next read of the token or by `prune`.
    fn collect_dead_record(&self, path: &Path, token: &str, lock: TokenLock) -> DeadRecord {
        match lock {
            TokenLock::Held => {
                if discard_file(path, "expired record") {
                    DeadRecord::Removed
                } else {
                    DeadRecord::Left
                }
            }
            TokenLock::NotHeld => self.remove_dead_record(path, token),
        }
    }

    /// Deletes the record file of `token` when the session in it is over
    /// (see [`Self::read_located`]) and reports what became of the file.
    ///
    /// For callers that read the file without the token lock (`prune`, a
    /// plain load): the file is looked at again under the lock, so a commit
    /// that replaced the record after it was read is not deleted.
    fn remove_dead_record(&self, path: &Path, token: &str) -> DeadRecord {
        if !Self::is_safe_token(token) {
            // Not a record this store wrote.
            return DeadRecord::Left;
        }
        let outcome = self.with_lock(token, || {
            let dead = match fs::read_to_string(path) {
                Ok(content) => {
                    match serde_json::from_str::<PersistentDraftSessionRecord>(&content) {
                        Ok(record) => record.expires_at_unix_secs <= current_unix_secs(),
                        Err(_) => file_is_older_than(path, self.ttl),
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(DeadRecord::Left);
                }
                Err(_) => file_is_older_than(path, self.ttl),
            };
            Ok(if !dead {
                DeadRecord::Replaced
            } else if discard_file(path, "expired record") {
                DeadRecord::Removed
            } else {
                DeadRecord::Left
            })
        });
        match outcome {
            Ok(outcome) => outcome,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "expired draft session record was not collected"
                );
                DeadRecord::Left
            }
        }
    }

    pub fn save_record(
        &self,
        record: &PersistentDraftSessionRecord,
    ) -> Result<(), SsrDraftSessionError> {
        self.ensure_available()?;
        Self::validate_token(&record.token)?;
        let json_bytes = serde_json::to_vec_pretty(record)
            .map_err(|e| SsrDraftSessionError::Serialization(e.to_string()))?;
        ensure_record_size(json_bytes.len())?;
        let partition = self.dir.join(Self::partition_name(record.owner.as_ref())?);
        ensure_private_dir(&partition).map_err(|e| SsrDraftSessionError::Io(e.to_string()))?;
        let file_name = Self::record_file_name(&record.token);
        let target_path = partition.join(&file_name);
        if !target_path.is_file() {
            self.enforce_owner_cap(&partition, &record.owner, &record.token)?;
        }
        atomic_write_file(&target_path, &json_bytes)
            .map_err(|e| SsrDraftSessionError::Io(e.to_string()))?;
        if record.owner.is_some() {
            // An attributed commit adopts an unattributed session: it now
            // lives in the tenant partition only.
            remove_file_if_present(&self.dir.join(UNATTRIBUTED_PARTITION).join(&file_name))
                .map_err(|e| SsrDraftSessionError::Io(e.to_string()))?;
        }
        Ok(())
    }

    /// Deletes the oldest same-owner records beyond
    /// [`MAX_DRAFT_SESSIONS_PER_OWNER`] (audit §2.5). Only runs when this
    /// record is new — updating an existing session does not change counts.
    /// An owner's records all live in that owner's tenant `partition`.
    /// Fails, before anything is written, when an excess record cannot be
    /// deleted.
    fn enforce_owner_cap(
        &self,
        partition: &Path,
        owner: &Option<DraftSessionOwner>,
        keep_token: &str,
    ) -> Result<(), SsrDraftSessionError> {
        let Some(owner) = owner.as_ref() else {
            return Ok(());
        };
        let entries = match fs::read_dir(partition) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(SsrDraftSessionError::Io(err.to_string())),
        };
        let mut owned: Vec<(PathBuf, RecordAge)> = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !file_name.ends_with(".json") {
                continue;
            }
            let Some(record) = fs::read_to_string(&path).ok().and_then(|content| {
                serde_json::from_str::<PersistentDraftSessionRecord>(&content).ok()
            }) else {
                continue;
            };
            if record.owner.as_ref() == Some(owner) && record.token != keep_token {
                let age = record_age(&path, record.expires_at_unix_secs);
                owned.push((path, age));
            }
        }
        if owned.len() >= MAX_DRAFT_SESSIONS_PER_OWNER {
            owned.sort_by_key(|(_, age)| *age);
            let excess = owned.len() + 1 - MAX_DRAFT_SESSIONS_PER_OWNER;
            for (path, _) in owned.into_iter().take(excess) {
                // The cap is a bound, not a hint: a session that cannot be
                // evicted must not be joined by one more.
                remove_file_if_present(&path)
                    .map_err(|err| SsrDraftSessionError::Io(err.to_string()))?;
            }
        }
        Ok(())
    }

    pub fn save_snapshot(
        &self,
        snapshot: &SsrDraftSessionSnapshot,
    ) -> Result<(), SsrDraftSessionError> {
        Self::validate_token(&snapshot.token)?;
        let record = PersistentDraftSessionRecord {
            token: snapshot.token.clone(),
            generation: snapshot.generation,
            controller: snapshot.controller.clone(),
            runtime_context: snapshot.runtime_context.clone(),
            owner: snapshot.owner.clone(),
            expires_at_unix_secs: current_unix_secs().saturating_add(self.ttl.as_secs()),
        };
        self.save_record(&record)
    }

    /// Extends the TTL of a record that was just read.
    ///
    /// A record whose TTL is still nearly full is left alone, so reads do not
    /// rewrite (and fsync) the whole record every time; the slack is a
    /// sixteenth of the TTL, at most [`MAX_TTL_REFRESH_SLACK_SECS`]. Otherwise
    /// the refresh is a compare-and-set, so it cannot regress a concurrent
    /// commit (audit §4.1) — losing that race is fine, the winning commit
    /// wrote a fresh TTL itself. A refresh that fails is reported
    /// (audit §6.7): swallowing it would let the session expire silently.
    fn refresh_ttl(
        &self,
        record: PersistentDraftSessionRecord,
    ) -> Result<(), SsrDraftSessionError> {
        let now = current_unix_secs();
        let ttl_secs = self.ttl.as_secs();
        let slack = (ttl_secs / 16).min(MAX_TTL_REFRESH_SLACK_SECS);
        let remaining = record.expires_at_unix_secs.saturating_sub(now);
        if remaining.saturating_add(slack) >= ttl_secs {
            return Ok(());
        }
        let mut refreshed = record;
        refreshed.expires_at_unix_secs = now.saturating_add(ttl_secs);
        self.compare_and_save_record(Some(&refreshed), &refreshed)
            .map(|_stored| ())
    }

    /// Commits an update to an existing record while holding the token lock,
    /// closing the read-check-write TOCTOU window between pods and processes
    /// sharing this directory (audit §4.1).
    ///
    /// `runtime_context: None` keeps the stored context (audit §6.5). The
    /// caller's inputs are bounced back in [`TokenLockCommit::Missing`] when
    /// the record is absent or expired, so a fresh session can be created
    /// from them afterwards.
    fn commit_existing_under_lock(
        &self,
        token: &str,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
        runtime_context: Option<Value>,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<TokenLockCommit, SsrDraftSessionError> {
        self.with_lock(token, move || {
            let Some((_, mut record)) = self.read_located(token, owner, TokenLock::Held)? else {
                return Ok(TokenLockCommit::Missing(controller, runtime_context));
            };
            if record.controller.page_id() != controller.page_id() {
                return Err(SsrDraftSessionError::PageMismatch {
                    expected: record.controller.page_id().to_string(),
                    actual: controller.page_id().to_string(),
                });
            }
            ensure_session_owner(record.owner.as_ref(), owner)?;
            if expected_generation.is_some_and(|expected| expected != record.generation) {
                return Err(SsrDraftSessionError::GenerationConflict {
                    expected: record.generation,
                    actual: expected_generation.unwrap_or_default(),
                });
            }
            record.generation = record.generation.saturating_add(1);
            record.controller = controller;
            if let Some(runtime_context) = runtime_context {
                record.runtime_context = runtime_context;
            }
            if owner.is_some() {
                record.owner = owner.cloned();
            }
            record.expires_at_unix_secs = current_unix_secs().saturating_add(self.ttl.as_secs());
            let snapshot = SsrDraftSessionSnapshot {
                token: record.token.clone(),
                generation: record.generation,
                controller: record.controller.clone(),
                runtime_context: record.runtime_context.clone(),
                owner: record.owner.clone(),
            };
            self.save_record(&record)?;
            Ok(TokenLockCommit::Committed(snapshot))
        })
    }

    /// Shared commit path; `runtime_context: None` keeps the stored context
    /// (an empty object for a new session), `Some` replaces it.
    fn commit_internal(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
        runtime_context: Option<Value>,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError> {
        let trimmed_token = token
            .map(str::trim)
            .filter(|t| !t.is_empty() && Self::is_safe_token(t));
        let (controller, runtime_context) = if let Some(token) = trimmed_token {
            match self.commit_existing_under_lock(
                token,
                expected_generation,
                controller,
                runtime_context,
                owner,
            )? {
                TokenLockCommit::Committed(snapshot) => return Ok(snapshot),
                // The record expired while we waited for the lock; create a
                // fresh session from the bounced-back inputs below.
                TokenLockCommit::Missing(controller, runtime_context) => {
                    (controller, runtime_context)
                }
            }
        } else {
            (controller, runtime_context)
        };

        let token = Uuid::new_v4().simple().to_string();
        let snapshot = SsrDraftSessionSnapshot {
            token,
            generation: 1,
            controller,
            runtime_context: runtime_context.unwrap_or_else(|| Value::Object(Map::new())),
            owner: owner.cloned(),
        };
        self.save_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    /// Scans one partition for `prune`: deletes the records of sessions that
    /// are over and abandoned temporary files, and collects the live records.
    fn prune_partition(
        &self,
        partition: &Path,
        now: u64,
        surviving: &mut Vec<(PathBuf, RecordAge, Option<DraftSessionOwner>)>,
    ) -> usize {
        let Ok(entries) = fs::read_dir(partition) else {
            return 0;
        };
        let mut pruned = 0;
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if file_name.contains(".tmp.") {
                // A fresh temporary file belongs to a write in flight.
                if file_is_older_than(&path, TOKEN_LOCK_STALE_AFTER)
                    && fs::remove_file(&path).is_ok()
                {
                    pruned += 1;
                }
                continue;
            }
            let Some(token) = file_name.strip_suffix(".json") else {
                continue;
            };
            let record = fs::read_to_string(&path)
                .ok()
                .and_then(|c| serde_json::from_str::<PersistentDraftSessionRecord>(&c).ok());
            match record {
                Some(record) if record.expires_at_unix_secs > now => {
                    let age = record_age(&path, record.expires_at_unix_secs);
                    surviving.push((path, age, record.owner));
                }
                // Expired or unreadable: decided again under the token lock.
                _ => {
                    if self.remove_dead_record(&path, token) == DeadRecord::Removed {
                        pruned += 1;
                    }
                }
            }
        }
        pruned
    }
}

impl Default for FileSsrDraftSessionStore {
    fn default() -> Self {
        Self::editor_default()
    }
}

impl PersistentDraftSessionBackend for FileSsrDraftSessionStore {
    fn load_record(
        &self,
        token: &str,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<Option<PersistentDraftSessionRecord>, SsrDraftSessionError> {
        Ok(self
            .read_located(token, owner, TokenLock::NotHeld)?
            .map(|(_, record)| record))
    }

    fn save_record(
        &self,
        record: &PersistentDraftSessionRecord,
    ) -> Result<(), SsrDraftSessionError> {
        self.save_record(record)
    }

    fn compare_and_save_record(
        &self,
        expected: Option<&PersistentDraftSessionRecord>,
        new_record: &PersistentDraftSessionRecord,
    ) -> Result<bool, SsrDraftSessionError> {
        Self::validate_token(&new_record.token)?;
        self.with_lock(&new_record.token, || {
            let current = self
                .read_located(
                    &new_record.token,
                    new_record.owner.as_ref(),
                    TokenLock::Held,
                )?
                .map(|(_, record)| record);
            let matches = match (expected, current) {
                (None, None) => true,
                (Some(expected), Some(current)) => {
                    expected.token == current.token
                        && expected.generation == current.generation
                        && expected.controller.page_id() == current.controller.page_id()
                        && expected.owner == current.owner
                }
                _ => false,
            };
            if !matches {
                return Ok(false);
            }
            self.save_record(new_record)?;
            Ok(true)
        })
    }

    fn delete_record(
        &self,
        token: &str,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<(), SsrDraftSessionError> {
        let token = token.trim();
        if !Self::is_safe_token(token) {
            return Ok(());
        }
        // Backend primitive: the caller (the durable store) has already
        // enforced ownership against this record; `owner` only selects the
        // tenant partition. Runs under the token lock, so it cannot
        // interleave with a commit.
        self.with_lock(token, || {
            if let Some(file_path) = self.locate(token, owner)? {
                remove_file_if_present(&file_path)
                    .map_err(|e| SsrDraftSessionError::Io(e.to_string()))?;
            }
            Ok(())
        })
    }

    fn prune_records(
        &self,
        _now_unix_secs: u64,
        _max_entries: usize,
    ) -> Result<usize, SsrDraftSessionError> {
        self.prune()
    }
}

impl SsrDraftSessionStore for FileSsrDraftSessionStore {
    fn load(
        &self,
        token: &str,
        page_id: &str,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<Option<SsrDraftSessionSnapshot>, SsrDraftSessionError> {
        let Some(record) = self.load_record(token, owner)? else {
            return Ok(None);
        };
        if record.controller.page_id() != page_id {
            return Err(SsrDraftSessionError::PageMismatch {
                expected: record.controller.page_id().to_string(),
                actual: page_id.to_string(),
            });
        }
        ensure_session_owner(record.owner.as_ref(), owner)?;
        let snapshot = SsrDraftSessionSnapshot {
            token: record.token.clone(),
            generation: record.generation,
            controller: record.controller.clone(),
            runtime_context: record.runtime_context.clone(),
            owner: record.owner.clone(),
        };
        self.refresh_ttl(record)?;
        Ok(Some(snapshot))
    }

    fn commit(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError> {
        self.commit_internal(token, expected_generation, controller, None, owner)
    }

    fn commit_with_context(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
        runtime_context: Value,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError> {
        self.commit_internal(
            token,
            expected_generation,
            controller,
            Some(runtime_context),
            owner,
        )
    }

    fn remove(
        &self,
        token: &str,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<(), SsrDraftSessionError> {
        let token = token.trim();
        if !Self::is_safe_token(token) {
            return Ok(());
        }
        // Deleting under the token lock keeps a concurrent commit from
        // resurrecting the record (or this remove from deleting a freshly
        // committed one) mid-flight (audit §4.1).
        self.with_lock(token, || {
            if let Some((file_path, record)) = self.read_located(token, owner, TokenLock::Held)? {
                ensure_session_owner(record.owner.as_ref(), owner)?;
                remove_file_if_present(&file_path)
                    .map_err(|e| SsrDraftSessionError::Io(e.to_string()))?;
            }
            Ok(())
        })
    }

    fn prune(&self) -> Result<usize, SsrDraftSessionError> {
        self.ensure_available()?;
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(err) => return Err(SsrDraftSessionError::Io(err.to_string())),
        };
        let mut pruned = 0;
        let now = current_unix_secs();
        let mut surviving: Vec<(PathBuf, RecordAge, Option<DraftSessionOwner>)> = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let is_dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
            if !is_dir {
                // Files of the flat layout this store used before tenant
                // partitions. Nothing reads them any more, so they would
                // keep tenant content in the storage root forever. A whole
                // TTL without a write means the session has expired even for
                // a process still running the old layout: every write there
                // stamped the record with a fresh TTL, so its expiry is never
                // later than the file's modification time plus one TTL.
                if Self::is_legacy_flat_file(name)
                    && file_is_older_than(&path, self.ttl)
                    && fs::remove_file(&path).is_ok()
                    && name.ends_with(".json")
                {
                    pruned += 1;
                }
                continue;
            }
            if name == LOCKS_DIR {
                // Tokens are never reused, so the lock of a crashed holder
                // would otherwise stay behind forever.
                remove_stale_files(&path, TOKEN_LOCK_STALE_AFTER);
            } else if name == UNATTRIBUTED_PARTITION || name.starts_with(TENANT_PARTITION_PREFIX) {
                pruned += self.prune_partition(&path, now, &mut surviving);
            }
        }

        // Global cap first (oldest evicted), then the per-owner cap on the
        // survivors (audit §2.5) — mirroring the KV backend.
        if surviving.len() > self.maximum_entries {
            surviving.sort_by_key(|(_, age, _)| *age);
            let excess = surviving.len() - self.maximum_entries;
            let evicted: Vec<(PathBuf, RecordAge, Option<DraftSessionOwner>)> =
                surviving.drain(..excess).collect();
            for (path, _, _) in evicted {
                if discard_file(&path, "evicted record") {
                    pruned += 1;
                }
            }
        }

        let mut per_owner: HashMap<String, Vec<(PathBuf, RecordAge)>> = HashMap::new();
        for (path, age, owner) in &surviving {
            if let Some(owner) = owner {
                per_owner
                    .entry(format!("{}:{}", owner.tenant_id, owner.user_id))
                    .or_default()
                    .push((path.clone(), *age));
            }
        }
        for (_, mut owned) in per_owner {
            if owned.len() > MAX_DRAFT_SESSIONS_PER_OWNER {
                owned.sort_by_key(|(_, age)| *age);
                let excess = owned.len() - MAX_DRAFT_SESSIONS_PER_OWNER;
                for (path, _) in owned.into_iter().take(excess) {
                    if discard_file(&path, "evicted record") {
                        pruned += 1;
                    }
                }
            }
        }

        Ok(pruned)
    }
}

/// Driver interface for distributed Key-Value stores (Redis, Memcached, KV database table).
pub trait DraftSessionKvDriver: Send + Sync {
    /// Gets raw serialized bytes by key. Returns `None` if key does not exist.
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, SsrDraftSessionError>;

    /// Stores raw serialized bytes with an explicit TTL in seconds.
    ///
    /// `cas` is the record's generation; it becomes the key's compare-and-set
    /// token, so a later [`Self::compare_and_set`] against the same generation
    /// succeeds.
    fn set(
        &self,
        key: &str,
        value: &[u8],
        cas: u64,
        ttl_secs: u64,
    ) -> Result<(), SsrDraftSessionError>;

    /// Atomically stores `value` (with CAS token `cas`) only while the key's
    /// current CAS token equals `expected_cas` (audit §4.1).
    ///
    /// `expected_cas: None` means the key must not exist (SETNX-style
    /// create). Returns `Ok(false)` — not an error — when the comparison
    /// fails: the caller lost the race. Redis-style drivers implement this
    /// with WATCH/MULTI or a Lua script; the token is the record generation,
    /// not part of the payload bytes.
    fn compare_and_set(
        &self,
        key: &str,
        expected_cas: Option<u64>,
        value: &[u8],
        cas: u64,
        ttl_secs: u64,
    ) -> Result<bool, SsrDraftSessionError>;

    /// Deletes a key.
    fn delete(&self, key: &str) -> Result<(), SsrDraftSessionError>;

    /// Atomically deletes the key only while its current CAS token equals
    /// `expected_cas`.
    ///
    /// Returns `Ok(false)` — not an error — when the key is absent or holds
    /// another token: the record was replaced or removed after the caller
    /// read it. This is how the backend drops a record it found expired
    /// without removing a commit that landed in between; an unconditional
    /// [`Self::delete`] would discard that commit after it was acknowledged.
    /// Redis-style drivers implement it the same way as
    /// [`Self::compare_and_set`].
    fn compare_and_delete(
        &self,
        key: &str,
        expected_cas: u64,
    ) -> Result<bool, SsrDraftSessionError>;

    /// Scans or lists all keys starting with the given prefix.
    fn scan_keys(&self, prefix: &str) -> Result<Vec<String>, SsrDraftSessionError>;
}

/// Distributed SSR draft session storage backend backed by a Key-Value driver (e.g. Redis or database KV).
#[derive(Clone)]
pub struct ExternalKvDraftSessionBackend {
    driver: Arc<dyn DraftSessionKvDriver>,
    prefix: String,
    ttl: Duration,
    maximum_entries: usize,
}

impl ExternalKvDraftSessionBackend {
    pub const DEFAULT_PREFIX: &'static str = "rustok:draft_sessions:";

    pub fn new(
        driver: Arc<dyn DraftSessionKvDriver>,
        ttl: Duration,
        maximum_entries: usize,
    ) -> Self {
        Self::with_prefix(driver, Self::DEFAULT_PREFIX, ttl, maximum_entries)
    }

    pub fn with_prefix(
        driver: Arc<dyn DraftSessionKvDriver>,
        prefix: impl Into<String>,
        ttl: Duration,
        maximum_entries: usize,
    ) -> Self {
        Self {
            driver,
            prefix: prefix.into(),
            ttl: ttl.max(Duration::from_secs(30)),
            maximum_entries: maximum_entries.max(1),
        }
    }

    pub fn driver(&self) -> &Arc<dyn DraftSessionKvDriver> {
        &self.driver
    }

    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    pub fn maximum_entries(&self) -> usize {
        self.maximum_entries
    }

    fn key_for_token(&self, token: &str) -> String {
        format!("{}{}", self.prefix, token.trim())
    }
}

impl PersistentDraftSessionBackend for ExternalKvDraftSessionBackend {
    fn load_record(
        &self,
        token: &str,
        _owner: Option<&DraftSessionOwner>,
    ) -> Result<Option<PersistentDraftSessionRecord>, SsrDraftSessionError> {
        let token = token.trim();
        if token.is_empty() || !FileSsrDraftSessionStore::is_safe_token(token) {
            return Ok(None);
        }
        let key = self.key_for_token(token);
        // An expired record is deleted only while it is still the record
        // that was read. A commit can land between the read and the delete;
        // the key then holds a live record under a newer generation, and the
        // read looks once more instead of deleting it.
        for _ in 0..2 {
            let Some(bytes) = self.driver.get(&key)? else {
                return Ok(None);
            };
            // A record that cannot be parsed is reported, not deleted: the
            // driver removes it when the TTL it was stored with runs out.
            let record: PersistentDraftSessionRecord = serde_json::from_slice(&bytes)
                .map_err(|e| SsrDraftSessionError::Serialization(e.to_string()))?;
            if record.expires_at_unix_secs > current_unix_secs() {
                return Ok(Some(record));
            }
            match self.driver.compare_and_delete(&key, record.generation) {
                Ok(true) => return Ok(None),
                Ok(false) => continue,
                // The record is expired whether or not the delete lands; the
                // driver TTL and `prune` collect a leftover.
                Err(err) => {
                    tracing::warn!(
                        error = %err,
                        "expired draft session record could not be deleted"
                    );
                    return Ok(None);
                }
            }
        }
        Ok(None)
    }

    fn save_record(
        &self,
        record: &PersistentDraftSessionRecord,
    ) -> Result<(), SsrDraftSessionError> {
        FileSsrDraftSessionStore::validate_token(&record.token)?;
        let key = self.key_for_token(&record.token);
        let json_bytes = serde_json::to_vec(record)
            .map_err(|e| SsrDraftSessionError::Serialization(e.to_string()))?;
        ensure_record_size(json_bytes.len())?;
        let now = current_unix_secs();
        let remaining_ttl = record.expires_at_unix_secs.saturating_sub(now).max(1);
        self.driver
            .set(&key, &json_bytes, record.generation, remaining_ttl)
    }

    fn compare_and_save_record(
        &self,
        expected: Option<&PersistentDraftSessionRecord>,
        new_record: &PersistentDraftSessionRecord,
    ) -> Result<bool, SsrDraftSessionError> {
        FileSsrDraftSessionStore::validate_token(&new_record.token)?;
        let key = self.key_for_token(&new_record.token);
        let json_bytes = serde_json::to_vec(new_record)
            .map_err(|e| SsrDraftSessionError::Serialization(e.to_string()))?;
        ensure_record_size(json_bytes.len())?;
        let now = current_unix_secs();
        let remaining_ttl = new_record.expires_at_unix_secs.saturating_sub(now).max(1);
        self.driver.compare_and_set(
            &key,
            expected.map(|record| record.generation),
            &json_bytes,
            new_record.generation,
            remaining_ttl,
        )
    }

    fn delete_record(
        &self,
        token: &str,
        _owner: Option<&DraftSessionOwner>,
    ) -> Result<(), SsrDraftSessionError> {
        let token = token.trim();
        if !FileSsrDraftSessionStore::is_safe_token(token) {
            return Ok(());
        }
        let key = self.key_for_token(token);
        self.driver.delete(&key)
    }

    fn prune_records(
        &self,
        now_unix_secs: u64,
        max_entries: usize,
    ) -> Result<usize, SsrDraftSessionError> {
        let keys = self.driver.scan_keys(&self.prefix)?;
        let mut surviving = Vec::new();
        let mut per_owner: HashMap<String, Vec<(String, u64)>> = HashMap::new();
        let mut deletions: BTreeSet<String> = BTreeSet::new();
        // Generation each key held when it was scanned. A key is deleted
        // only while it still holds that generation, so a session committed
        // during the prune keeps its record.
        let mut scanned: HashMap<String, u64> = HashMap::new();

        for key in keys {
            let record = self.driver.get(&key).ok().flatten().and_then(|bytes| {
                serde_json::from_slice::<PersistentDraftSessionRecord>(&bytes).ok()
            });
            if let Some(record) = record {
                scanned.insert(key.clone(), record.generation);
                if record.expires_at_unix_secs <= now_unix_secs {
                    deletions.insert(key);
                } else {
                    if let Some(owner) = record.owner.as_ref() {
                        per_owner
                            .entry(format!("{}:{}", owner.tenant_id, owner.user_id))
                            .or_default()
                            .push((key.clone(), record.expires_at_unix_secs));
                    }
                    surviving.push((key, record.expires_at_unix_secs));
                }
            }
        }

        if surviving.len() > max_entries {
            surviving.sort_by_key(|(_, expires)| *expires);
            let excess = surviving.len() - max_entries;
            for (key, _) in surviving.into_iter().take(excess) {
                deletions.insert(key);
            }
        }

        // Per-owner session cap (audit §2.5): evict the same-owner sessions
        // expiring soonest beyond the cap. Keys already picked by the global
        // cap are deduped by `deletions`, so each prune is counted once.
        for (_, mut owned) in per_owner {
            if owned.len() > MAX_DRAFT_SESSIONS_PER_OWNER {
                owned.sort_by_key(|(_, expires)| *expires);
                let excess = owned.len() - MAX_DRAFT_SESSIONS_PER_OWNER;
                for (key, _) in owned.into_iter().take(excess) {
                    deletions.insert(key);
                }
            }
        }

        let mut pruned = 0;
        for key in &deletions {
            let Some(generation) = scanned.get(key).copied() else {
                continue;
            };
            match self.driver.compare_and_delete(key, generation) {
                Ok(true) => pruned += 1,
                // Committed or removed since the scan: not this prune's to
                // delete. The next prune judges the record that is there.
                Ok(false) => {}
                Err(err) => {
                    tracing::warn!(error = %err, "draft session record could not be pruned");
                }
            }
        }
        Ok(pruned)
    }
}

/// In-memory reference implementation of `DraftSessionKvDriver` for testing and simulations.
#[derive(Clone, Default)]
pub struct MemoryKvDriver {
    entries: Arc<RwLock<HashMap<String, (Vec<u8>, u64, u64)>>>,
}

impl MemoryKvDriver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.read().map(|e| e.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl DraftSessionKvDriver for MemoryKvDriver {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>, SsrDraftSessionError> {
        let entries = self
            .entries
            .read()
            .map_err(|_| SsrDraftSessionError::Poisoned)?;
        let now = current_unix_secs();
        if let Some((bytes, _, _)) = entries.get(key).filter(|(_, _, expires)| *expires > now) {
            return Ok(Some(bytes.clone()));
        }
        Ok(None)
    }

    fn set(
        &self,
        key: &str,
        value: &[u8],
        cas: u64,
        ttl_secs: u64,
    ) -> Result<(), SsrDraftSessionError> {
        let mut entries = self
            .entries
            .write()
            .map_err(|_| SsrDraftSessionError::Poisoned)?;
        let expires = current_unix_secs().saturating_add(ttl_secs);
        entries.insert(key.to_string(), (value.to_vec(), cas, expires));
        Ok(())
    }

    fn compare_and_set(
        &self,
        key: &str,
        expected_cas: Option<u64>,
        value: &[u8],
        cas: u64,
        ttl_secs: u64,
    ) -> Result<bool, SsrDraftSessionError> {
        let mut entries = self
            .entries
            .write()
            .map_err(|_| SsrDraftSessionError::Poisoned)?;
        let now = current_unix_secs();
        let current = entries
            .get(key)
            .filter(|(_, _, expires)| *expires > now)
            .map(|(_, current_cas, _)| *current_cas);
        if current != expected_cas {
            return Ok(false);
        }
        let expires = now.saturating_add(ttl_secs);
        entries.insert(key.to_string(), (value.to_vec(), cas, expires));
        Ok(true)
    }

    fn delete(&self, key: &str) -> Result<(), SsrDraftSessionError> {
        let mut entries = self
            .entries
            .write()
            .map_err(|_| SsrDraftSessionError::Poisoned)?;
        entries.remove(key);
        Ok(())
    }

    fn compare_and_delete(
        &self,
        key: &str,
        expected_cas: u64,
    ) -> Result<bool, SsrDraftSessionError> {
        let mut entries = self
            .entries
            .write()
            .map_err(|_| SsrDraftSessionError::Poisoned)?;
        let now = current_unix_secs();
        let matches = entries.get(key).is_some_and(|(_, current_cas, expires)| {
            *expires > now && *current_cas == expected_cas
        });
        if matches {
            entries.remove(key);
        }
        Ok(matches)
    }

    fn scan_keys(&self, prefix: &str) -> Result<Vec<String>, SsrDraftSessionError> {
        let entries = self
            .entries
            .read()
            .map_err(|_| SsrDraftSessionError::Poisoned)?;
        Ok(entries
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect())
    }
}

#[derive(Clone)]
pub struct DurableSsrDraftSessionStore {
    memory: InMemorySsrDraftSessionStore,
    backend: Arc<dyn PersistentDraftSessionBackend>,
    file: Option<FileSsrDraftSessionStore>,
}

impl DurableSsrDraftSessionStore {
    pub fn new(memory: InMemorySsrDraftSessionStore, file: FileSsrDraftSessionStore) -> Self {
        Self {
            memory,
            backend: Arc::new(file.clone()),
            file: Some(file),
        }
    }

    pub fn with_backend(
        memory: InMemorySsrDraftSessionStore,
        backend: Arc<dyn PersistentDraftSessionBackend>,
    ) -> Self {
        Self {
            memory,
            backend,
            file: None,
        }
    }

    pub fn distributed(
        memory: InMemorySsrDraftSessionStore,
        driver: Arc<dyn DraftSessionKvDriver>,
        ttl: Duration,
        maximum_entries: usize,
    ) -> Self {
        Self::with_backend(
            memory,
            Arc::new(ExternalKvDraftSessionBackend::new(
                driver,
                ttl,
                maximum_entries,
            )),
        )
    }

    pub fn editor_default() -> Self {
        Self::new(
            InMemorySsrDraftSessionStore::editor_default(),
            FileSsrDraftSessionStore::editor_default(),
        )
    }

    pub fn memory(&self) -> &InMemorySsrDraftSessionStore {
        &self.memory
    }

    pub fn backend(&self) -> &Arc<dyn PersistentDraftSessionBackend> {
        &self.backend
    }

    pub fn file(&self) -> Option<&FileSsrDraftSessionStore> {
        self.file.as_ref()
    }

    /// Shared commit path; `runtime_context: None` keeps the stored context
    /// (audit §6.5), `Some` replaces it.
    fn commit_internal(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
        runtime_context: Option<Value>,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError> {
        let persistent_record = if let Some(tok) = token.map(str::trim).filter(|t| !t.is_empty()) {
            self.backend.load_record(tok, owner)?
        } else {
            None
        };
        if let Some(persistent_record) = persistent_record.as_ref() {
            if persistent_record.controller.page_id() != controller.page_id() {
                return Err(SsrDraftSessionError::PageMismatch {
                    expected: persistent_record.controller.page_id().to_string(),
                    actual: controller.page_id().to_string(),
                });
            }
            ensure_session_owner(persistent_record.owner.as_ref(), owner)?;
            if expected_generation.is_some_and(|expected| expected != persistent_record.generation)
            {
                return Err(SsrDraftSessionError::GenerationConflict {
                    expected: persistent_record.generation,
                    actual: expected_generation.unwrap_or_default(),
                });
            }
            // Read-only freshness probe: no TTL refresh, no exclusive lock
            // (audit §5.3).
            let is_stale = self
                .memory
                .generation(&persistent_record.token)?
                .is_none_or(|generation| generation < persistent_record.generation);
            if is_stale {
                // The loaded record stays borrowed: it is the compare-and-set
                // expectation for the backend write below.
                self.memory.insert_snapshot(SsrDraftSessionSnapshot {
                    token: persistent_record.token.clone(),
                    generation: persistent_record.generation,
                    controller: persistent_record.controller.clone(),
                    runtime_context: persistent_record.runtime_context.clone(),
                    owner: persistent_record.owner.clone(),
                })?;
            }
        }
        // `None` keeps the context held by the memory tier, which was just
        // re-seeded from the persistent record when it was stale.
        let committed = self.memory.commit_internal(
            token,
            expected_generation,
            controller,
            runtime_context,
            owner,
        )?;
        let record = PersistentDraftSessionRecord {
            token: committed.token.clone(),
            generation: committed.generation,
            controller: committed.controller.clone(),
            runtime_context: committed.runtime_context.clone(),
            owner: committed.owner.clone(),
            expires_at_unix_secs: current_unix_secs().saturating_add(self.memory.ttl().as_secs()),
        };
        // Compare-and-set closes the TOCTOU window: another pod may have
        // committed the same generation between our load and this write
        // (audit §4.1). On a lost race, restore the authoritative record into
        // the memory cache and report the conflict instead of overwriting.
        let stored = match self
            .backend
            .compare_and_save_record(persistent_record.as_ref(), &record)
        {
            Ok(stored) => stored,
            Err(error) => {
                // The memory tier already holds the new generation, the
                // backend does not. Left in place, that entry would fail
                // every later commit of this session: a load answers with
                // the cached generation, and the commit then compares it
                // with the stored record, which is one behind. The backend
                // is authoritative, so the entry is dropped and the next
                // read loads the stored record again.
                self.forget_unpersisted(&committed.token);
                return Err(error);
            }
        };
        if !stored {
            let current = match self.backend.load_record(&committed.token, owner) {
                Ok(current) => current,
                Err(error) => {
                    self.forget_unpersisted(&committed.token);
                    return Err(error);
                }
            };
            let current_generation = current
                .as_ref()
                .map(|record| record.generation)
                .unwrap_or_default();
            match current {
                Some(current) => self.memory.insert_snapshot(SsrDraftSessionSnapshot {
                    token: current.token,
                    generation: current.generation,
                    controller: current.controller,
                    runtime_context: current.runtime_context,
                    owner: current.owner,
                })?,
                // The record disappeared between the two backend calls.
                None => self.forget_unpersisted(&committed.token),
            }
            return Err(SsrDraftSessionError::GenerationConflict {
                expected: current_generation,
                actual: expected_generation.unwrap_or_default(),
            });
        }
        Ok(committed)
    }

    /// Drops the memory entry of a commit the backend did not store. A
    /// failure here is logged, not returned: the caller is already reporting
    /// the backend error that caused it.
    fn forget_unpersisted(&self, token: &str) {
        if let Err(error) = self.memory.evict(token) {
            tracing::warn!(
                error = %error,
                "draft session that was not persisted stayed in the memory cache"
            );
        }
    }
}

impl Default for DurableSsrDraftSessionStore {
    fn default() -> Self {
        Self::editor_default()
    }
}

impl SsrDraftSessionStore for DurableSsrDraftSessionStore {
    fn load(
        &self,
        token: &str,
        page_id: &str,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<Option<SsrDraftSessionSnapshot>, SsrDraftSessionError> {
        if let Some(snapshot) = self.memory.load(token, page_id, owner)? {
            if let Some(record) = self.backend.load_record(token, owner)? {
                if record.controller.page_id() != page_id {
                    // The mismatch is the error to report; a cache entry
                    // that stays behind is rejected the same way next time.
                    if let Err(err) = self.memory.remove(token, owner) {
                        tracing::warn!(
                            error = %err,
                            "mismatched draft session stayed in the memory cache"
                        );
                    }
                    return Err(SsrDraftSessionError::PageMismatch {
                        expected: record.controller.page_id().to_string(),
                        actual: page_id.to_string(),
                    });
                }
                ensure_session_owner(record.owner.as_ref(), owner)?;
                if record.generation > snapshot.generation {
                    let fresh_snapshot = SsrDraftSessionSnapshot {
                        token: record.token,
                        generation: record.generation,
                        controller: record.controller,
                        runtime_context: record.runtime_context,
                        owner: record.owner,
                    };
                    self.memory.insert_snapshot(fresh_snapshot.clone())?;
                    return Ok(Some(fresh_snapshot));
                }
                return Ok(Some(snapshot));
            } else {
                self.memory.remove(token, owner)?;
                return Ok(None);
            }
        }
        // Fallback to persistent backend (cross-process / cross-pod restart)
        if let Some(record) = self.backend.load_record(token, owner)? {
            if record.controller.page_id() != page_id {
                return Err(SsrDraftSessionError::PageMismatch {
                    expected: record.controller.page_id().to_string(),
                    actual: page_id.to_string(),
                });
            }
            ensure_session_owner(record.owner.as_ref(), owner)?;
            let snapshot = SsrDraftSessionSnapshot {
                token: record.token,
                generation: record.generation,
                controller: record.controller,
                runtime_context: record.runtime_context,
                owner: record.owner,
            };
            self.memory.insert_snapshot(snapshot.clone())?;
            return Ok(Some(snapshot));
        }
        Ok(None)
    }

    fn commit(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError> {
        self.commit_internal(token, expected_generation, controller, None, owner)
    }

    fn commit_with_context(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
        runtime_context: Value,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError> {
        self.commit_internal(
            token,
            expected_generation,
            controller,
            Some(runtime_context),
            owner,
        )
    }

    fn remove(
        &self,
        token: &str,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<(), SsrDraftSessionError> {
        // The backend record is authoritative for ownership: the memory cache
        // may be empty after a restart while the record lives on.
        if let Some(record) = self.backend.load_record(token, owner)? {
            ensure_session_owner(record.owner.as_ref(), owner)?;
        }
        self.memory.remove(token, owner)?;
        self.backend.delete_record(token, owner)?;
        Ok(())
    }

    /// Entries removed across both tiers (audit §6.6). The memory tier is a
    /// cache of the backend, so a session dropped from both counts twice:
    /// the number reports deleted entries, not distinct sessions.
    fn prune(&self) -> Result<usize, SsrDraftSessionError> {
        let mem = self.memory.prune()?;
        let persistent = self
            .backend
            .prune_records(current_unix_secs(), self.memory.maximum_entries())?;
        Ok(mem.saturating_add(persistent))
    }
}

fn current_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Owner check shared by every store implementation (audit §2.5).
///
/// `None` on the record side is "unattributed" (predates owner binding) and
/// always passes; an attributed record requires the exact same caller.
fn ensure_session_owner(
    record_owner: Option<&DraftSessionOwner>,
    caller: Option<&DraftSessionOwner>,
) -> Result<(), SsrDraftSessionError> {
    match (record_owner, caller) {
        (Some(record), Some(caller)) if record == caller => Ok(()),
        (Some(_), _) => Err(SsrDraftSessionError::OwnerMismatch),
        (None, _) => Ok(()),
    }
}

/// Rejects oversized serialized records (audit §2.5). Callers pass the exact
/// byte length they are about to persist, so the limit matches what lands in
/// storage — the file backend writes pretty-printed JSON, the KV backend
/// compact JSON, and only those actual bytes are capped.
fn ensure_record_size(bytes: usize) -> Result<(), SsrDraftSessionError> {
    if bytes > MAX_DRAFT_SESSION_RECORD_BYTES {
        return Err(SsrDraftSessionError::RecordTooLarge {
            bytes,
            maximum: MAX_DRAFT_SESSION_RECORD_BYTES,
        });
    }
    Ok(())
}

/// How long a crashed holder's token lock may survive before waiters break it.
const TOKEN_LOCK_STALE_AFTER: Duration = Duration::from_secs(30);

/// How long a waiter polls for the token lock before giving up.
const TOKEN_LOCK_WAIT: Duration = Duration::from_secs(2);

/// Runs `action` while holding an advisory lock file for `token` in `dir`,
/// making read-check-write sequences on one record atomic across processes
/// and pods that share the directory (audit §4.1).
///
/// The lock is a `O_EXCL`-created companion file; a holder that crashed leaves
/// it behind, so waiters break locks older than [`TOKEN_LOCK_STALE_AFTER`].
/// Two waiters can never both acquire from a clean state: the create itself is
/// atomic, and after a stale break only one `create_new` succeeds. Each holder
/// writes its identity into the lock and only removes a lock it still owns, so
/// a broken-then-recreated lock is never deleted by its predecessor.
///
/// Residual race, accepted and bounded: a waiter that has already judged a
/// lock stale can delete a freshly re-created lock file in the microseconds
/// between its `metadata` and `remove_file` calls, briefly yielding two
/// holders. It requires a crashed holder plus two concurrent waiters hitting
/// that exact interleave; the holder identity keeps the aftermath consistent.
/// When a filesystem dependency is permissible, `flock`/`fcntl` locks remove
/// the window entirely — they die with the process.
fn with_token_lock<T>(
    dir: &Path,
    token: &str,
    action: impl FnOnce() -> Result<T, SsrDraftSessionError>,
) -> Result<T, SsrDraftSessionError> {
    FileSsrDraftSessionStore::validate_token(token)?;
    let lock_path = dir.join(format!("{token}.lock"));
    let holder = Uuid::new_v4().simple().to_string();
    let deadline = Instant::now() + TOKEN_LOCK_WAIT;
    loop {
        match private_file_options().open(&lock_path) {
            Ok(mut lock_file) => {
                // Identify the holder inside the lock so release only ever
                // removes a lock we still own. A failed identity write
                // leaves an unverifiable lock: drop it and surface the I/O
                // error instead of running the action unattributed.
                if let Err(err) = lock_file.write_all(holder.as_bytes()) {
                    discard_file(&lock_path, "token lock");
                    return Err(SsrDraftSessionError::Io(err.to_string()));
                }
                drop(lock_file);
                let result = action();
                let still_ours = fs::read_to_string(&lock_path)
                    .ok()
                    .is_some_and(|content| content == holder);
                if still_ours {
                    // A lock that stays behind blocks this token until
                    // waiters break it after `TOKEN_LOCK_STALE_AFTER`.
                    discard_file(&lock_path, "token lock");
                }
                return result;
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                let stale = fs::metadata(&lock_path)
                    .ok()
                    .and_then(|meta| meta.modified().ok())
                    .and_then(|modified| modified.elapsed().ok())
                    .is_some_and(|age| age > TOKEN_LOCK_STALE_AFTER);
                if stale {
                    // Retrying is only sound once the stale lock is gone. A
                    // lock that cannot be removed would be found stale again
                    // on every pass, and this branch skips the deadline.
                    remove_file_if_present(&lock_path)
                        .map_err(|err| SsrDraftSessionError::Io(err.to_string()))?;
                    continue;
                }
                if Instant::now() >= deadline {
                    return Err(SsrDraftSessionError::LockTimeout(token.to_string()));
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(err) => return Err(SsrDraftSessionError::Io(err.to_string())),
        }
    }
}

/// Whether the caller of [`FileSsrDraftSessionStore::read_located`] holds the
/// lock of the token it reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenLock {
    Held,
    NotHeld,
}

/// What became of a record file that looked dead to a reader without the
/// token lock, once it was looked at again under that lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeadRecord {
    /// The session in it was over and this call deleted the file.
    Removed,
    /// The file no longer holds a session that is over: a commit replaced
    /// the record after it was read.
    Replaced,
    /// Nothing was deleted: the lock stayed busy, the delete failed, or the
    /// file was already gone.
    Left,
}

/// Outcome of committing an existing session under its token lock: either the
/// commit landed, or the record was absent/expired and the caller's inputs are
/// handed back so a fresh session can be created from them.
enum TokenLockCommit {
    Committed(SsrDraftSessionSnapshot),
    Missing(AdminCanvasController, Option<Value>),
}

/// Eviction order of a file-backed record: expiry first, then the file's
/// modification time. Expiry has one-second resolution, so sessions written
/// within the same second are ordered by the nanosecond timestamp that
/// [`atomic_write_file`] stamps on every record.
type RecordAge = (u64, SystemTime);

fn record_age(path: &Path, expires_at_unix_secs: u64) -> RecordAge {
    let modified = fs::metadata(path)
        .and_then(|meta| meta.modified())
        .unwrap_or(UNIX_EPOCH);
    (expires_at_unix_secs, modified)
}

fn file_is_older_than(path: &Path, age: Duration) -> bool {
    fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|elapsed| elapsed > age)
}

/// Removes the files in `dir` not modified for longer than `age`. A failure
/// does not fail the caller: the files are leftovers, retried by the next run.
fn remove_stale_files(dir: &Path, age: Duration) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
        Err(err) => {
            tracing::warn!(error = %err, "draft session lock directory could not be read");
            return;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if file_is_older_than(&path, age) {
            discard_file(&path, "stale token lock");
        }
    }
}

/// Removes a file that is no longer wanted, where a failure must not fail the
/// caller's operation; returns whether this call removed it. A leftover is
/// collected later (`prune`, or a waiter breaking a stale lock), and the
/// failure is logged: storage that stops accepting deletions fills up.
///
/// The path is deliberately not logged — record and lock file names are
/// session tokens.
fn discard_file(path: &Path, kind: &'static str) -> bool {
    match fs::remove_file(path) {
        Ok(()) => true,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
        Err(err) => {
            tracing::warn!(error = %err, kind = %kind, "draft session file could not be removed");
            false
        }
    }
}

/// Deletes `path`, treating "already gone" as success.
fn remove_file_if_present(path: &Path) -> std::io::Result<()> {
    match fs::remove_file(path) {
        Err(err) if err.kind() != std::io::ErrorKind::NotFound => Err(err),
        _ => Ok(()),
    }
}

/// Makes sure `path` is a directory only this process's user can enter
/// (audit §2.6): created `0700` when missing, tightened to `0700` when it
/// exists with wider permissions.
///
/// A symbolic link is refused — the storage root may live in a shared
/// location such as the OS temp dir, where another local user could plant
/// one. For an unprivileged process the mode is also an ownership check:
/// `chmod` fails for a directory owned by someone else, and a foreign
/// directory that already is `0700` cannot be entered. Neither holds for a
/// privileged process, so the storage root is additionally checked with
/// [`ensure_owned_by_process`].
fn ensure_private_dir(path: &Path) -> std::io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => restrict_dir_to_owner(path, &meta),
        Ok(_) => Err(not_a_private_dir()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            create_private_dir(path)?;
            // Re-check: the mode is subject to the umask, and another
            // process may have created the path first.
            let meta = fs::symlink_metadata(path)?;
            if !meta.is_dir() {
                return Err(not_a_private_dir());
            }
            restrict_dir_to_owner(path, &meta)
        }
        Err(err) => Err(err),
    }
}

/// Refuses a storage root that belongs to another user (audit §2.6).
///
/// Mode bits do not prove ownership to a privileged process: root can enter
/// and `chmod` anyone's directory, so a directory another local user created
/// ahead of the server — and can therefore still rename, empty, or refill —
/// would pass [`ensure_private_dir`]. The standard library has no `geteuid`,
/// so the process's identity is read off a file it creates here: on the same
/// file system a new file and a directory this process owns carry one uid.
///
/// Only the root needs this. Partitions and the lock directory are created
/// inside it, where nobody else can create anything once it is `0700` and
/// owned by this process.
#[cfg(unix)]
fn ensure_owned_by_process(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    // Named like a temporary record so `prune` collects a probe left behind
    // by a crash between the two calls below.
    let probe = dir.join(format!("probe.tmp.{}", Uuid::new_v4().simple()));
    let probe_uid = private_file_options()
        .open(&probe)
        .and_then(|file| file.metadata())
        .map(|meta| meta.uid());
    // Removed before either result is inspected, so no outcome leaves the
    // probe behind. A root this process cannot delete from is not usable
    // storage either: the creation error, if any, is reported first.
    let removed = remove_file_if_present(&probe);
    let probe_uid = probe_uid?;
    removed?;
    let dir_uid = fs::symlink_metadata(dir)?.uid();
    if probe_uid == dir_uid {
        Ok(())
    } else {
        Err(std::io::Error::other("directory is owned by another user"))
    }
}

#[cfg(not(unix))]
fn ensure_owned_by_process(_dir: &Path) -> std::io::Result<()> {
    // No POSIX owner: access control is inherited from the parent directory,
    // which the operator configures.
    Ok(())
}

fn not_a_private_dir() -> std::io::Error {
    std::io::Error::other("path exists and is not a directory (symbolic links are refused)")
}

#[cfg(unix)]
fn create_private_dir(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
}

#[cfg(not(unix))]
fn create_private_dir(path: &Path) -> std::io::Result<()> {
    fs::create_dir_all(path)
}

#[cfg(unix)]
fn restrict_dir_to_owner(path: &Path, meta: &fs::Metadata) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if meta.permissions().mode() & 0o7777 != 0o700 {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn restrict_dir_to_owner(_path: &Path, _meta: &fs::Metadata) -> std::io::Result<()> {
    // No POSIX mode bits: access control is inherited from the parent
    // directory, which the operator configures.
    Ok(())
}

/// Options for creating a new file readable and writable by its owner only.
fn private_file_options() -> OpenOptions {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
}

/// Replaces `target` with `content` atomically and durably (audit §2.6).
///
/// The content goes to a private (`0600`) temporary file in the same
/// directory, is flushed to disk, and is then renamed over the target, so a
/// crash leaves either the old record or the new one — never a partial or
/// missing file.
fn atomic_write_file(target: &Path, content: &[u8]) -> std::io::Result<()> {
    let tmp = target.with_extension(format!("tmp.{}", Uuid::new_v4().simple()));
    let written = write_private_file(&tmp, content).and_then(|()| replace_file(&tmp, target));
    if let Err(err) = written {
        // `prune` collects a temporary file that stays behind.
        discard_file(&tmp, "temporary record");
        return Err(err);
    }
    // The record is in place; flushing the directory entry only hardens the
    // rename against power loss. Some file systems cannot sync a directory,
    // and failing the commit now would misreport a write that did land.
    if let Err(err) = sync_parent_dir(target) {
        tracing::warn!(error = %err, "draft session directory entry could not be flushed");
    }
    Ok(())
}

fn write_private_file(path: &Path, content: &[u8]) -> std::io::Result<()> {
    let mut file = private_file_options().open(path)?;
    file.write_all(content)?;
    // Nanosecond modification time for `RecordAge`: the kernel may stamp
    // writes with a coarse (tick-resolution) clock, which would tie records
    // written in quick succession. Best effort — ordering falls back to the
    // file system's own timestamp.
    if let Err(err) = file.set_modified(SystemTime::now()) {
        tracing::debug!(error = %err, "draft session record keeps the file system timestamp");
    }
    file.sync_all()
}

#[cfg(unix)]
fn replace_file(tmp: &Path, target: &Path) -> std::io::Result<()> {
    // `rename(2)` replaces the target atomically; removing it first would
    // open a window with no record at all.
    fs::rename(tmp, target)
}

#[cfg(not(unix))]
fn replace_file(tmp: &Path, target: &Path) -> std::io::Result<()> {
    if fs::rename(tmp, target).is_ok() {
        return Ok(());
    }
    // Platforms where rename does not replace an existing file.
    remove_file_if_present(target)?;
    fs::rename(tmp, target)
}

#[cfg(unix)]
fn sync_parent_dir(target: &Path) -> std::io::Result<()> {
    match target.parent() {
        Some(parent) => fs::File::open(parent)?.sync_all(),
        None => Ok(()),
    }
}

#[cfg(not(unix))]
fn sync_parent_dir(_target: &Path) -> std::io::Result<()> {
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum SsrDraftSessionError {
    #[error("SSR draft session storage lock is poisoned")]
    Poisoned,
    #[error("SSR draft session belongs to page `{expected}`, not `{actual}`")]
    PageMismatch { expected: String, actual: String },
    /// A commit carried a generation that is not the one the session holds.
    ///
    /// The field names read as a validation pair, so read them with the message: `expected` is
    /// the generation the **stored record** holds (what the store expected the caller to send)
    /// and `actual` is the generation the **caller** sent. Every constructor in this crate and in
    /// its consumers uses that order; the values are diagnostics only (the browser receives
    /// `DRAFT_GENERATION_CONFLICT`), so a swap would not mis-behave, it would only mislead an
    /// operator reading the log line.
    #[error("SSR draft session generation conflict: stored `{expected}`, caller sent `{actual}`")]
    GenerationConflict { expected: u64, actual: u64 },
    #[error("SSR draft session token `{0}` is invalid")]
    InvalidToken(String),
    #[error("SSR draft session belongs to another owner")]
    OwnerMismatch,
    #[error("SSR draft session owner is invalid: {0}")]
    InvalidOwner(String),
    #[error("SSR draft session record is {bytes} bytes, above the {maximum} byte limit")]
    RecordTooLarge { bytes: usize, maximum: usize },
    #[error("SSR draft session storage lock for token `{0}` timed out")]
    LockTimeout(String),
    #[error("SSR draft session I/O error: {0}")]
    Io(String),
    #[error("SSR draft session serialization error: {0}")]
    Serialization(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly_ui::UiIntent;
    use serde_json::json;

    fn controller(page_id: &str) -> AdminCanvasController {
        AdminCanvasController::new(
            page_id,
            "rev-1",
            json!({
                "pages": [{
                    "id": page_id,
                    "component": {
                        "id": "root",
                        "type": "wrapper",
                        "components": [{ "id": "hero", "type": "section" }]
                    }
                }]
            }),
        )
        .expect("controller")
    }

    #[test]
    fn session_preserves_editor_history_clipboard_and_runtime_context() {
        let store = InMemorySsrDraftSessionStore::default();
        let mut controller = controller("home");
        controller
            .dispatch(UiIntent::Select(Some("hero".to_string())))
            .unwrap();
        controller.dispatch(UiIntent::CopySelection).unwrap();
        let first = store
            .commit_with_context(
                None,
                None,
                controller,
                json!({ "customer": { "name": "Ada" } }),
                None,
            )
            .expect("create");
        assert!(first.controller.has_clipboard());

        let loaded = store
            .load(&first.token, "home", None)
            .expect("load")
            .expect("session");
        assert_eq!(loaded.generation, 1);
        assert!(loaded.controller.has_clipboard());
        assert_eq!(loaded.runtime_context["customer"]["name"], "Ada");
    }

    #[test]
    fn ordinary_commit_preserves_existing_runtime_context() {
        let store = InMemorySsrDraftSessionStore::default();
        let first = store
            .commit_with_context(None, None, controller("home"), json!({ "value": 42 }), None)
            .expect("create");
        let second = store
            .commit(
                Some(&first.token),
                Some(first.generation),
                first.controller,
                None,
            )
            .expect("update controller");
        assert_eq!(second.runtime_context["value"], 42);
    }

    #[test]
    fn optimistic_generation_rejects_parallel_commit() {
        let store = InMemorySsrDraftSessionStore::default();
        let first = store
            .commit(None, None, controller("home"), None)
            .expect("create");
        let second = store
            .commit(
                Some(&first.token),
                Some(first.generation),
                first.controller.clone(),
                None,
            )
            .expect("update");
        assert_eq!(second.generation, 2);
        assert!(matches!(
            store.commit(
                Some(&first.token),
                Some(first.generation),
                first.controller,
                None,
            ),
            Err(SsrDraftSessionError::GenerationConflict { .. })
        ));
    }

    #[test]
    fn session_token_cannot_cross_page_boundary() {
        let store = InMemorySsrDraftSessionStore::default();
        let first = store
            .commit(None, None, controller("home"), None)
            .expect("create");
        assert!(matches!(
            store.load(&first.token, "about", None),
            Err(SsrDraftSessionError::PageMismatch { .. })
        ));
    }

    #[test]
    fn file_store_persists_across_restart_and_validates_page() {
        let temp_dir =
            std::env::temp_dir().join(format!("rustok_draft_test_{}", Uuid::new_v4().simple()));
        let store = FileSsrDraftSessionStore::new(&temp_dir, Duration::from_secs(3600), 100)
            .expect("store");
        let mut ctrl = controller("home");
        ctrl.dispatch(UiIntent::Select(Some("hero".to_string())))
            .unwrap();
        ctrl.dispatch(UiIntent::CopySelection).unwrap();

        let committed = store
            .commit_with_context(None, None, ctrl, json!({ "theme": "dark" }), None)
            .expect("commit");

        // Simulate server restart by dropping store and re-instantiating against same dir
        drop(store);
        let restarted_store =
            FileSsrDraftSessionStore::new(&temp_dir, Duration::from_secs(3600), 100)
                .expect("restarted store");
        let loaded = restarted_store
            .load(&committed.token, "home", None)
            .expect("load")
            .expect("snapshot exists");

        assert_eq!(loaded.generation, 1);
        assert!(loaded.controller.has_clipboard());
        assert_eq!(loaded.runtime_context["theme"], "dark");

        // Page mismatch on disk
        assert!(matches!(
            restarted_store.load(&committed.token, "other_page", None),
            Err(SsrDraftSessionError::PageMismatch { .. })
        ));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn file_store_rejects_unsafe_tokens() {
        let temp_dir =
            std::env::temp_dir().join(format!("rustok_draft_test_{}", Uuid::new_v4().simple()));
        let store = FileSsrDraftSessionStore::new(&temp_dir, Duration::from_secs(3600), 100)
            .expect("store");
        assert_eq!(store.load("../traversal", "home", None).unwrap(), None);
        assert_eq!(
            store.load("token with spaces!", "home", None).unwrap(),
            None
        );
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn file_store_optimistic_generation_conflict() {
        let temp_dir =
            std::env::temp_dir().join(format!("rustok_draft_test_{}", Uuid::new_v4().simple()));
        let store = FileSsrDraftSessionStore::new(&temp_dir, Duration::from_secs(3600), 100)
            .expect("store");
        let ctrl = controller("home");
        let first = store
            .commit(None, None, ctrl.clone(), None)
            .expect("commit 1");
        assert_eq!(first.generation, 1);

        let second = store
            .commit(
                Some(&first.token),
                Some(first.generation),
                ctrl.clone(),
                None,
            )
            .expect("commit 2");
        assert_eq!(second.generation, 2);

        // Conflict: expected generation 1 when generation is now 2
        assert!(matches!(
            store.commit(Some(&first.token), Some(1), ctrl, None),
            Err(SsrDraftSessionError::GenerationConflict {
                expected: 2,
                actual: 1
            })
        ));
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn durable_store_serves_from_memory_and_survives_cold_restart() {
        let temp_dir =
            std::env::temp_dir().join(format!("rustok_durable_test_{}", Uuid::new_v4().simple()));
        let file_store = FileSsrDraftSessionStore::new(&temp_dir, Duration::from_secs(3600), 100)
            .expect("file store");
        let mem_store = InMemorySsrDraftSessionStore::new(Duration::from_secs(3600), 100);
        let durable = DurableSsrDraftSessionStore::new(mem_store, file_store);

        let mut ctrl = controller("landing");
        ctrl.dispatch(UiIntent::Select(Some("hero".to_string())))
            .unwrap();
        ctrl.dispatch(UiIntent::CopySelection).unwrap();

        let committed = durable
            .commit_with_context(None, None, ctrl, json!({ "user": "admin" }), None)
            .expect("commit");

        // Immediately available in memory
        assert!(
            durable
                .memory()
                .load(&committed.token, "landing", None)
                .unwrap()
                .is_some()
        );

        // Simulate server process restart: new empty memory store, re-attached to same disk dir
        let cold_memory = InMemorySsrDraftSessionStore::new(Duration::from_secs(3600), 100);
        let reattached_file =
            FileSsrDraftSessionStore::new(&temp_dir, Duration::from_secs(3600), 100).expect("file");
        let restarted_durable = DurableSsrDraftSessionStore::new(cold_memory, reattached_file);

        // Cold memory does not have the token initially
        assert!(
            restarted_durable
                .memory()
                .load(&committed.token, "landing", None)
                .unwrap()
                .is_none()
        );

        // Loading via durable store seamlessly retrieves from disk and warms memory
        let loaded = restarted_durable
            .load(&committed.token, "landing", None)
            .expect("load")
            .expect("session");
        assert_eq!(loaded.generation, 1);
        assert_eq!(loaded.runtime_context["user"], "admin");
        assert!(loaded.controller.has_clipboard());

        // Now memory is warmed!
        assert!(
            restarted_durable
                .memory()
                .load(&committed.token, "landing", None)
                .unwrap()
                .is_some()
        );

        // Further commit increments generation across both layers
        let updated = restarted_durable
            .commit(Some(&committed.token), Some(1), loaded.controller, None)
            .expect("second commit");
        assert_eq!(updated.generation, 2);

        // Remove cleans up both
        restarted_durable
            .remove(&committed.token, None)
            .expect("remove");
        assert!(
            restarted_durable
                .load(&committed.token, "landing", None)
                .unwrap()
                .is_none()
        );

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn distributed_kv_store_multi_pod_simulation() {
        let shared_driver = Arc::new(MemoryKvDriver::new());

        // Pod 1 creates durable store
        let pod1_memory = InMemorySsrDraftSessionStore::new(Duration::from_secs(3600), 100);
        let pod1_store = DurableSsrDraftSessionStore::distributed(
            pod1_memory,
            shared_driver.clone(),
            Duration::from_secs(3600),
            100,
        );

        let mut ctrl = controller("home");
        ctrl.dispatch(UiIntent::Select(Some("hero".to_string())))
            .unwrap();
        ctrl.dispatch(UiIntent::CopySelection).unwrap();

        let committed = pod1_store
            .commit_with_context(None, None, ctrl, json!({ "pod": "pod-1" }), None)
            .expect("pod 1 commit");
        assert_eq!(committed.generation, 1);

        // Pod 2 (with separate in-memory cache) connects to the same shared KV driver
        let pod2_memory = InMemorySsrDraftSessionStore::new(Duration::from_secs(3600), 100);
        let pod2_store = DurableSsrDraftSessionStore::distributed(
            pod2_memory,
            shared_driver.clone(),
            Duration::from_secs(3600),
            100,
        );

        // Pod 2 loads session from distributed KV driver
        let pod2_loaded = pod2_store
            .load(&committed.token, "home", None)
            .expect("pod 2 load")
            .expect("session");
        assert_eq!(pod2_loaded.generation, 1);
        assert_eq!(pod2_loaded.runtime_context["pod"], "pod-1");
        assert!(pod2_loaded.controller.has_clipboard());

        // Pod 2 commits update (generation increments to 2)
        let pod2_updated = pod2_store
            .commit_with_context(
                Some(&committed.token),
                Some(1),
                pod2_loaded.controller,
                json!({ "pod": "pod-2" }),
                None,
            )
            .expect("pod 2 update");
        assert_eq!(pod2_updated.generation, 2);

        // Pod 1 attempts commit with stale generation 1 -> must fail with GenerationConflict!
        let stale_ctrl = controller("home");
        let conflict_result = pod1_store.commit(Some(&committed.token), Some(1), stale_ctrl, None);
        assert!(matches!(
            conflict_result,
            Err(SsrDraftSessionError::GenerationConflict { .. })
        ));

        // Pod 1 can delete the session across the cluster
        pod1_store
            .remove(&committed.token, None)
            .expect("cluster remove");
        assert!(
            pod2_store
                .load(&committed.token, "home", None)
                .unwrap()
                .is_none()
        );
        assert_eq!(shared_driver.len(), 0);
    }

    #[test]
    fn distributed_kv_store_page_isolation_and_pruning() {
        let shared_driver = Arc::new(MemoryKvDriver::new());
        let store = DurableSsrDraftSessionStore::distributed(
            InMemorySsrDraftSessionStore::new(Duration::from_secs(3600), 10),
            shared_driver.clone(),
            Duration::from_secs(3600),
            10,
        );

        let first = store
            .commit(None, None, controller("catalog"), None)
            .expect("commit");

        // Attempting to load for "checkout" page must yield PageMismatch
        let mismatch = store.load(&first.token, "checkout", None);
        assert!(matches!(
            mismatch,
            Err(SsrDraftSessionError::PageMismatch { expected, actual }) if expected == "catalog" && actual == "checkout"
        ));

        // Pruning works without errors
        let pruned = store.prune().expect("prune");
        assert_eq!(pruned, 0); // Not expired yet
    }

    fn owner(tenant: &str, user: &str) -> DraftSessionOwner {
        DraftSessionOwner {
            tenant_id: tenant.to_string(),
            user_id: user.to_string(),
        }
    }

    #[test]
    fn owner_binding_rejects_foreign_callers() {
        let store = InMemorySsrDraftSessionStore::default();
        let alice = owner("acme", "user-alice");
        let created = store
            .commit(None, None, controller("home"), Some(&alice))
            .expect("create as alice");
        assert_eq!(created.owner.as_ref(), Some(&alice));

        // A different user cannot load, commit, or remove the session —
        // even with the correct token, page, and generation.
        let bob = owner("acme", "user-bob");
        assert!(matches!(
            store.load(&created.token, "home", Some(&bob)),
            Err(SsrDraftSessionError::OwnerMismatch)
        ));
        assert!(matches!(
            store.commit(
                Some(&created.token),
                Some(created.generation),
                controller("home"),
                Some(&bob)
            ),
            Err(SsrDraftSessionError::OwnerMismatch)
        ));
        assert!(matches!(
            store.remove(&created.token, Some(&bob)),
            Err(SsrDraftSessionError::OwnerMismatch)
        ));

        // Cross-tenant is equally rejected, while the owner succeeds.
        let mallory = owner("evil", "user-alice");
        assert!(matches!(
            store.load(&created.token, "home", Some(&mallory)),
            Err(SsrDraftSessionError::OwnerMismatch)
        ));
        assert!(
            store
                .load(&created.token, "home", Some(&alice))
                .expect("owner load")
                .is_some()
        );
    }

    #[test]
    fn owner_binding_allows_legacy_records_and_adopts_owner() {
        let store = InMemorySsrDraftSessionStore::default();
        // A record written before owner binding (no owner) stays loadable
        // by attributed callers.
        let legacy = store
            .commit(None, None, controller("home"), None)
            .expect("legacy create");
        let alice = owner("acme", "user-alice");
        let loaded = store
            .load(&legacy.token, "home", Some(&alice))
            .expect("legacy load")
            .expect("session");
        assert!(loaded.owner.is_none());

        // An attributed commit adopts the owner; anonymous callers are then
        // locked out.
        let adopted = store
            .commit(
                Some(&legacy.token),
                Some(loaded.generation),
                loaded.controller,
                Some(&alice),
            )
            .expect("adopt");
        assert_eq!(adopted.owner.as_ref(), Some(&alice));
        assert!(matches!(
            store.load(&legacy.token, "home", None),
            Err(SsrDraftSessionError::OwnerMismatch)
        ));
    }

    #[test]
    fn durable_store_enforces_owner_on_backend_record() {
        let shared_driver = Arc::new(MemoryKvDriver::new());
        let pod1 = DurableSsrDraftSessionStore::distributed(
            InMemorySsrDraftSessionStore::new(Duration::from_secs(3600), 100),
            shared_driver.clone(),
            Duration::from_secs(3600),
            100,
        );
        let alice = owner("acme", "user-alice");
        let created = pod1
            .commit(None, None, controller("home"), Some(&alice))
            .expect("create");

        // Pod 2 has an empty memory cache: the ownership check must come
        // from the backend record, not the local cache.
        let pod2 = DurableSsrDraftSessionStore::distributed(
            InMemorySsrDraftSessionStore::new(Duration::from_secs(3600), 100),
            shared_driver.clone(),
            Duration::from_secs(3600),
            100,
        );
        let bob = owner("acme", "user-bob");
        assert!(matches!(
            pod2.load(&created.token, "home", Some(&bob)),
            Err(SsrDraftSessionError::OwnerMismatch)
        ));
        assert!(matches!(
            pod2.remove(&created.token, Some(&bob)),
            Err(SsrDraftSessionError::OwnerMismatch)
        ));
        assert!(
            pod2.load(&created.token, "home", Some(&alice))
                .expect("owner load")
                .is_some()
        );
    }

    #[test]
    fn in_memory_store_caps_sessions_per_owner() {
        let store = InMemorySsrDraftSessionStore::new(Duration::from_secs(3600), 10_000);
        let alice = owner("acme", "user-alice");
        let mut tokens = Vec::new();
        for _ in 0..(MAX_DRAFT_SESSIONS_PER_OWNER + 2) {
            let created = store
                .commit(None, None, controller("home"), Some(&alice))
                .expect("commit");
            tokens.push(created.token);
        }

        // The oldest sessions of this owner were evicted; only the newest
        // MAX_DRAFT_SESSIONS_PER_OWNER survive.
        let expired = &tokens[..tokens.len() - MAX_DRAFT_SESSIONS_PER_OWNER];
        assert!(!expired.is_empty());
        for token in expired {
            assert!(store.load(token, "home", Some(&alice)).unwrap().is_none());
        }
        for token in &tokens[tokens.len() - MAX_DRAFT_SESSIONS_PER_OWNER..] {
            assert!(store.load(token, "home", Some(&alice)).unwrap().is_some());
        }
    }

    #[test]
    fn file_store_caps_sessions_per_owner() {
        let temp_dir =
            std::env::temp_dir().join(format!("rustok_draft_test_{}", Uuid::new_v4().simple()));
        let store = FileSsrDraftSessionStore::new(&temp_dir, Duration::from_secs(3600), 10_000)
            .expect("store");
        let alice = owner("acme", "user-alice");
        let mut tokens = Vec::new();
        for _ in 0..(MAX_DRAFT_SESSIONS_PER_OWNER + 2) {
            let created = store
                .commit(None, None, controller("home"), Some(&alice))
                .expect("commit");
            tokens.push(created.token);
        }
        let expired = &tokens[..tokens.len() - MAX_DRAFT_SESSIONS_PER_OWNER];
        assert!(!expired.is_empty());
        for token in expired {
            assert!(store.load(token, "home", Some(&alice)).unwrap().is_none());
        }
        for token in &tokens[tokens.len() - MAX_DRAFT_SESSIONS_PER_OWNER..] {
            assert!(store.load(token, "home", Some(&alice)).unwrap().is_some());
        }
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn file_store_rejects_oversized_records() {
        let temp_dir =
            std::env::temp_dir().join(format!("rustok_draft_test_{}", Uuid::new_v4().simple()));
        let store = FileSsrDraftSessionStore::new(&temp_dir, Duration::from_secs(3600), 100)
            .expect("store");
        // A runtime context far above MAX_DRAFT_SESSION_RECORD_BYTES must be
        // rejected before it reaches the disk (audit §2.5).
        let oversized = json!({ "blob": "x".repeat(MAX_DRAFT_SESSION_RECORD_BYTES + 1) });
        let created = store.commit_with_context(None, None, controller("home"), oversized, None);
        assert!(matches!(
            created,
            Err(SsrDraftSessionError::RecordTooLarge { .. })
        ));
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn kv_driver_compare_and_set_is_atomic() {
        let driver = MemoryKvDriver::new();
        driver.set("k", b"v1", 1, 3600).expect("set");

        // Matching CAS token wins and bumps the stored token.
        assert!(
            driver
                .compare_and_set("k", Some(1), b"v2", 2, 3600)
                .expect("cas")
        );
        assert_eq!(driver.get("k").unwrap().as_deref(), Some(b"v2".as_slice()));

        // A stale CAS token loses without overwriting.
        assert!(
            !driver
                .compare_and_set("k", Some(1), b"stale", 3, 3600)
                .expect("cas")
        );
        assert_eq!(driver.get("k").unwrap().as_deref(), Some(b"v2".as_slice()));

        // expected None requires the key to be absent (SETNX semantics).
        assert!(
            !driver
                .compare_and_set("k", None, b"create", 1, 3600)
                .expect("cas")
        );
        driver.delete("k").expect("delete");
        assert!(
            driver
                .compare_and_set("k", None, b"created", 1, 3600)
                .expect("cas")
        );
        assert_eq!(
            driver.get("k").unwrap().as_deref(),
            Some(b"created".as_slice())
        );
    }

    #[test]
    fn file_store_commit_race_one_winner() {
        // Two processes share one directory and race on the same token and
        // generation: exactly one commit survives, the loser gets a
        // conflict instead of silently clobbering the winner (audit §4.1).
        let temp_dir =
            std::env::temp_dir().join(format!("rustok_draft_test_{}", Uuid::new_v4().simple()));
        let pod1 = FileSsrDraftSessionStore::new(&temp_dir, Duration::from_secs(3600), 100)
            .expect("pod 1 store");
        let pod2 = FileSsrDraftSessionStore::new(&temp_dir, Duration::from_secs(3600), 100)
            .expect("pod 2 store");

        let created = pod1
            .commit(None, None, controller("home"), None)
            .expect("create");
        assert!(
            pod2.load(&created.token, "home", None)
                .expect("load")
                .is_some()
        );

        let first = pod1
            .commit(
                Some(&created.token),
                Some(created.generation),
                controller("home"),
                None,
            )
            .expect("pod 1 wins the race");
        assert_eq!(first.generation, 2);

        let second = pod2.commit(
            Some(&created.token),
            Some(created.generation),
            controller("home"),
            None,
        );
        assert!(matches!(
            second,
            Err(SsrDraftSessionError::GenerationConflict { expected, actual })
                if expected == 2 && actual == 1
        ));

        // The token lock files are cleaned up behind both pods.
        let lock_files = fs::read_dir(&temp_dir)
            .expect("read dir")
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.ends_with(".lock"))
            })
            .count();
        assert_eq!(lock_files, 0);
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn distributed_kv_commit_race_one_winner() {
        // Two pods share one KV driver and race on the same token and
        // generation: the compare-and-set lets exactly one commit through
        // and the loser observes the winner's state (audit §4.1).
        let shared_driver = Arc::new(MemoryKvDriver::new());
        let pod1 = DurableSsrDraftSessionStore::distributed(
            InMemorySsrDraftSessionStore::new(Duration::from_secs(3600), 100),
            shared_driver.clone(),
            Duration::from_secs(3600),
            100,
        );
        let pod2 = DurableSsrDraftSessionStore::distributed(
            InMemorySsrDraftSessionStore::new(Duration::from_secs(3600), 100),
            shared_driver.clone(),
            Duration::from_secs(3600),
            100,
        );

        let created = pod1
            .commit(None, None, controller("home"), None)
            .expect("create");
        let loaded1 = pod1
            .load(&created.token, "home", None)
            .expect("pod 1 load")
            .expect("session");
        let loaded2 = pod2
            .load(&created.token, "home", None)
            .expect("pod 2 load")
            .expect("session");
        assert_eq!(loaded1.generation, loaded2.generation);

        // Both pods commit from the same observed generation. The first
        // compare-and-set wins; the second must fail with a conflict and
        // reseed its memory cache from the authoritative record.
        let winner = pod1
            .commit_with_context(
                Some(&created.token),
                Some(loaded1.generation),
                loaded1.controller,
                json!({ "pod": "pod-1" }),
                None,
            )
            .expect("pod 1 wins the race");
        assert_eq!(winner.generation, 2);

        let loser = pod2.commit_with_context(
            Some(&created.token),
            Some(loaded2.generation),
            loaded2.controller,
            json!({ "pod": "pod-2" }),
            None,
        );
        assert!(matches!(
            loser,
            Err(SsrDraftSessionError::GenerationConflict { expected, actual })
                if expected == 2 && actual == 1
        ));

        // The loser's cache now reflects the winner's committed state.
        let reseeded = pod2
            .load(&created.token, "home", None)
            .expect("pod 2 reload")
            .expect("session");
        assert_eq!(reseeded.generation, 2);
        assert_eq!(reseeded.runtime_context["pod"], "pod-1");
    }

    #[test]
    fn durable_commit_rolls_back_memory_when_backend_rejects() {
        // Even the losing pod's in-memory state must never advance past the
        // authoritative backend record.
        let shared_driver = Arc::new(MemoryKvDriver::new());
        let pod1 = DurableSsrDraftSessionStore::distributed(
            InMemorySsrDraftSessionStore::new(Duration::from_secs(3600), 100),
            shared_driver.clone(),
            Duration::from_secs(3600),
            100,
        );
        let pod2 = DurableSsrDraftSessionStore::distributed(
            InMemorySsrDraftSessionStore::new(Duration::from_secs(3600), 100),
            shared_driver,
            Duration::from_secs(3600),
            100,
        );

        let created = pod1
            .commit(None, None, controller("home"), None)
            .expect("create");
        let loaded2 = pod2
            .load(&created.token, "home", None)
            .expect("pod 2 load")
            .expect("session");

        // Pod 1 advances the backend to generation 2 behind pod 2's back.
        pod1.commit(Some(&created.token), Some(1), controller("home"), None)
            .expect("pod 1 commit");

        // Pod 2 commits from its stale view: the fail-fast check on the
        // reloaded backend record must reject before anything is persisted.
        let loser = pod2.commit(
            Some(&created.token),
            Some(loaded2.generation),
            loaded2.controller,
            None,
        );
        assert!(matches!(
            loser,
            Err(SsrDraftSessionError::GenerationConflict { expected, actual })
                if expected == 2 && actual == 1
        ));

        // Pod 2's next load reflects pod 1's authoritative state.
        let reloaded = pod2
            .load(&created.token, "home", None)
            .expect("pod 2 reload")
            .expect("session");
        assert_eq!(reloaded.generation, 2);
    }

    /// Backend whose writes can be switched off, standing in for a full
    /// disk, a lock timeout or an unreachable KV service.
    struct FlakyBackend {
        inner: ExternalKvDraftSessionBackend,
        fail_writes: std::sync::atomic::AtomicBool,
    }

    impl FlakyBackend {
        fn new() -> Self {
            Self {
                inner: ExternalKvDraftSessionBackend::new(
                    Arc::new(MemoryKvDriver::new()),
                    Duration::from_secs(3600),
                    100,
                ),
                fail_writes: std::sync::atomic::AtomicBool::new(false),
            }
        }

        fn fail_writes(&self, fail: bool) {
            self.fail_writes
                .store(fail, std::sync::atomic::Ordering::SeqCst);
        }

        fn write_gate(&self) -> Result<(), SsrDraftSessionError> {
            if self.fail_writes.load(std::sync::atomic::Ordering::SeqCst) {
                Err(SsrDraftSessionError::Io("backend unavailable".to_string()))
            } else {
                Ok(())
            }
        }
    }

    impl PersistentDraftSessionBackend for FlakyBackend {
        fn load_record(
            &self,
            token: &str,
            owner: Option<&DraftSessionOwner>,
        ) -> Result<Option<PersistentDraftSessionRecord>, SsrDraftSessionError> {
            self.inner.load_record(token, owner)
        }

        fn save_record(
            &self,
            record: &PersistentDraftSessionRecord,
        ) -> Result<(), SsrDraftSessionError> {
            self.write_gate()?;
            self.inner.save_record(record)
        }

        fn compare_and_save_record(
            &self,
            expected: Option<&PersistentDraftSessionRecord>,
            new_record: &PersistentDraftSessionRecord,
        ) -> Result<bool, SsrDraftSessionError> {
            self.write_gate()?;
            self.inner.compare_and_save_record(expected, new_record)
        }

        fn delete_record(
            &self,
            token: &str,
            owner: Option<&DraftSessionOwner>,
        ) -> Result<(), SsrDraftSessionError> {
            self.inner.delete_record(token, owner)
        }

        fn prune_records(
            &self,
            now_unix_secs: u64,
            max_entries: usize,
        ) -> Result<usize, SsrDraftSessionError> {
            self.inner.prune_records(now_unix_secs, max_entries)
        }
    }

    #[test]
    fn durable_commit_that_fails_to_persist_leaves_the_session_usable() {
        // A commit whose backend write fails must not leave the memory tier
        // one generation ahead of the stored record: every later commit of
        // the session would then fail its generation check.
        let backend = Arc::new(FlakyBackend::new());
        let store = DurableSsrDraftSessionStore::with_backend(
            InMemorySsrDraftSessionStore::new(Duration::from_secs(3600), 100),
            backend.clone(),
        );
        let created = store
            .commit(None, None, controller("home"), None)
            .expect("create");
        assert_eq!(created.generation, 1);

        backend.fail_writes(true);
        let failed = store.commit_with_context(
            Some(&created.token),
            Some(1),
            controller("home"),
            json!({ "attempt": "lost" }),
            None,
        );
        assert!(matches!(failed, Err(SsrDraftSessionError::Io(_))));
        assert_eq!(
            store
                .memory()
                .generation(&created.token)
                .expect("memory probe"),
            None,
            "the unpersisted generation must not stay cached"
        );

        // The session is still the stored one, for a reader...
        backend.fail_writes(false);
        let reloaded = store
            .load(&created.token, "home", None)
            .expect("reload")
            .expect("session");
        assert_eq!(reloaded.generation, 1);
        assert!(reloaded.runtime_context.get("attempt").is_none());

        // ...and for the retry of the same commit.
        let retried = store
            .commit_with_context(
                Some(&created.token),
                Some(reloaded.generation),
                controller("home"),
                json!({ "attempt": "kept" }),
                None,
            )
            .expect("retry");
        assert_eq!(retried.generation, 2);
        let stored = store
            .load(&created.token, "home", None)
            .expect("load after retry")
            .expect("session");
        assert_eq!(stored.generation, 2);
        assert_eq!(stored.runtime_context["attempt"], "kept");
    }

    #[test]
    fn durable_create_that_fails_to_persist_leaves_no_session_behind() {
        let backend = Arc::new(FlakyBackend::new());
        let store = DurableSsrDraftSessionStore::with_backend(
            InMemorySsrDraftSessionStore::new(Duration::from_secs(3600), 100),
            backend.clone(),
        );
        backend.fail_writes(true);
        let failed = store.commit(None, None, controller("home"), None);
        assert!(matches!(failed, Err(SsrDraftSessionError::Io(_))));
        assert!(store.memory().is_empty().expect("memory probe"));
    }

    #[test]
    fn durable_commit_cas_loss_reseeds_memory_from_winner() {
        // Deterministic two-pod write race (audit §4.1): a competing pod
        // commits between this pod's read and its compare-and-set. The CAS
        // must fail, the loser must surface GenerationConflict, and its
        // memory cache must be reseeded from the winner's record.
        use std::sync::atomic::{AtomicBool, Ordering};

        struct RacingKvDriver {
            inner: MemoryKvDriver,
            raced: AtomicBool,
        }

        impl DraftSessionKvDriver for RacingKvDriver {
            fn get(&self, key: &str) -> Result<Option<Vec<u8>>, SsrDraftSessionError> {
                self.inner.get(key)
            }

            fn set(
                &self,
                key: &str,
                value: &[u8],
                cas: u64,
                ttl_secs: u64,
            ) -> Result<(), SsrDraftSessionError> {
                self.inner.set(key, value, cas, ttl_secs)
            }

            fn compare_and_set(
                &self,
                key: &str,
                expected_cas: Option<u64>,
                value: &[u8],
                cas: u64,
                ttl_secs: u64,
            ) -> Result<bool, SsrDraftSessionError> {
                // Right before the first update CAS lands, a competing pod
                // slips its own commit into the store: it commits the same
                // target generation from the same observed state.
                if expected_cas.is_some() && !self.raced.swap(true, Ordering::SeqCst) {
                    let mut record: PersistentDraftSessionRecord =
                        serde_json::from_slice(value).expect("loser record parses");
                    record.runtime_context = json!({ "pod": "pod-1" });
                    let winner_bytes = serde_json::to_vec(&record).expect("winner serializes");
                    self.inner
                        .set(key, &winner_bytes, record.generation, ttl_secs)?;
                }
                self.inner
                    .compare_and_set(key, expected_cas, value, cas, ttl_secs)
            }

            fn delete(&self, key: &str) -> Result<(), SsrDraftSessionError> {
                self.inner.delete(key)
            }

            fn compare_and_delete(
                &self,
                key: &str,
                expected_cas: u64,
            ) -> Result<bool, SsrDraftSessionError> {
                self.inner.compare_and_delete(key, expected_cas)
            }

            fn scan_keys(&self, prefix: &str) -> Result<Vec<String>, SsrDraftSessionError> {
                self.inner.scan_keys(prefix)
            }
        }

        let driver = Arc::new(RacingKvDriver {
            inner: MemoryKvDriver::new(),
            raced: AtomicBool::new(false),
        });
        let store = DurableSsrDraftSessionStore::distributed(
            InMemorySsrDraftSessionStore::new(Duration::from_secs(3600), 100),
            driver,
            Duration::from_secs(3600),
            100,
        );

        let created = store
            .commit(None, None, controller("home"), None)
            .expect("create");

        // The competing pod wins the race between our read and our CAS:
        // the store must report the conflict, not overwrite the winner.
        let loser = store.commit_with_context(
            Some(&created.token),
            Some(created.generation),
            controller("home"),
            json!({ "pod": "pod-2" }),
            None,
        );
        assert!(matches!(
            loser,
            Err(SsrDraftSessionError::GenerationConflict { expected, actual })
                if expected == 2 && actual == 1
        ));

        // The loser's memory cache was reseeded from the winner's record.
        let reseeded = store
            .load(&created.token, "home", None)
            .expect("reload")
            .expect("session");
        assert_eq!(reseeded.generation, 2);
        assert_eq!(reseeded.runtime_context["pod"], "pod-1");
    }

    #[test]
    fn kv_backend_compare_and_save_record_enforces_expected() {
        let driver = Arc::new(MemoryKvDriver::new());
        let backend = ExternalKvDraftSessionBackend::new(driver, Duration::from_secs(3600), 100);
        let base = |generation: u64| PersistentDraftSessionRecord {
            token: "tokCAS01".to_string(),
            generation,
            controller: controller("home"),
            runtime_context: json!({ "generation": generation }),
            owner: None,
            expires_at_unix_secs: current_unix_secs() + 3600,
        };

        // Create: expected None against an absent key wins.
        assert!(
            backend
                .compare_and_save_record(None, &base(1))
                .expect("create cas")
        );
        // Create against an existing key loses.
        assert!(
            !backend
                .compare_and_save_record(None, &base(1))
                .expect("create cas loses")
        );
        // Update with the matching expected generation wins.
        assert!(
            backend
                .compare_and_save_record(Some(&base(1)), &base(2))
                .expect("update cas")
        );
        // Update with a stale expected generation loses.
        assert!(
            !backend
                .compare_and_save_record(Some(&base(1)), &base(3))
                .expect("stale cas loses")
        );
        // The stored record is the winner's.
        let stored = backend
            .load_record("tokCAS01", None)
            .expect("load")
            .expect("record");
        assert_eq!(stored.generation, 2);
    }

    #[test]
    fn file_backend_compare_and_save_record_enforces_expected() {
        let temp_dir =
            std::env::temp_dir().join(format!("rustok_draft_test_{}", Uuid::new_v4().simple()));
        let backend = FileSsrDraftSessionStore::new(&temp_dir, Duration::from_secs(3600), 100)
            .expect("store");
        let base =
            |generation: u64, owner: Option<DraftSessionOwner>| PersistentDraftSessionRecord {
                token: "tokCAS01".to_string(),
                generation,
                controller: controller("home"),
                runtime_context: json!({ "generation": generation }),
                owner,
                expires_at_unix_secs: current_unix_secs() + 3600,
            };

        // Create: expected None against an absent record wins.
        assert!(
            backend
                .compare_and_save_record(None, &base(1, None))
                .expect("create cas")
        );
        // Create against an existing record loses.
        assert!(
            !backend
                .compare_and_save_record(None, &base(1, None))
                .expect("create cas loses")
        );
        // Update with the matching expected record wins.
        assert!(
            backend
                .compare_and_save_record(Some(&base(1, None)), &base(2, None))
                .expect("update cas")
        );
        // A stale expected generation loses.
        assert!(
            !backend
                .compare_and_save_record(Some(&base(1, None)), &base(3, None))
                .expect("stale cas loses")
        );
        // A different expected owner loses even with a fresh generation.
        let alice = owner("acme", "user-alice");
        assert!(
            !backend
                .compare_and_save_record(Some(&base(2, Some(alice.clone()))), &base(3, None))
                .expect("owner cas loses")
        );
        // The stored record is the winner's.
        let stored = backend
            .load_record("tokCAS01", None)
            .expect("load")
            .expect("record");
        assert_eq!(stored.generation, 2);
        assert!(stored.owner.is_none());
        let _ = fs::remove_dir_all(&temp_dir);
    }

    fn temp_store_dir() -> PathBuf {
        std::env::temp_dir().join(format!("rustok_draft_test_{}", Uuid::new_v4().simple()))
    }

    fn file_store(dir: &Path) -> FileSsrDraftSessionStore {
        FileSsrDraftSessionStore::new(dir, Duration::from_secs(3600), 100).expect("store")
    }

    /// `commit` must keep the stored runtime context and only
    /// `commit_with_context` may replace it — on every store (audit §6.5).
    fn assert_commit_preserves_runtime_context(store: &dyn SsrDraftSessionStore) {
        let alice = owner("acme", "user-alice");
        let created = store
            .commit(None, None, controller("home"), Some(&alice))
            .expect("create");
        assert_eq!(created.runtime_context, json!({}));

        let with_context = store
            .commit_with_context(
                Some(&created.token),
                Some(created.generation),
                controller("home"),
                json!({ "customer": { "name": "Ada" } }),
                Some(&alice),
            )
            .expect("set context");
        let plain = store
            .commit(
                Some(&with_context.token),
                Some(with_context.generation),
                controller("home"),
                Some(&alice),
            )
            .expect("plain commit");
        assert_eq!(plain.generation, 3);
        assert_eq!(plain.runtime_context["customer"]["name"], "Ada");

        let loaded = store
            .load(&plain.token, "home", Some(&alice))
            .expect("load")
            .expect("session");
        assert_eq!(loaded.runtime_context["customer"]["name"], "Ada");

        let replaced = store
            .commit_with_context(
                Some(&plain.token),
                Some(plain.generation),
                controller("home"),
                json!({ "customer": null }),
                Some(&alice),
            )
            .expect("replace context");
        assert_eq!(replaced.runtime_context, json!({ "customer": null }));
    }

    #[test]
    fn commit_preserves_runtime_context_on_every_store() {
        assert_commit_preserves_runtime_context(&InMemorySsrDraftSessionStore::default());

        let file_dir = temp_store_dir();
        assert_commit_preserves_runtime_context(&file_store(&file_dir));
        let _ = fs::remove_dir_all(&file_dir);

        let durable_dir = temp_store_dir();
        assert_commit_preserves_runtime_context(&DurableSsrDraftSessionStore::new(
            InMemorySsrDraftSessionStore::new(Duration::from_secs(3600), 100),
            file_store(&durable_dir),
        ));
        let _ = fs::remove_dir_all(&durable_dir);

        assert_commit_preserves_runtime_context(&DurableSsrDraftSessionStore::distributed(
            InMemorySsrDraftSessionStore::new(Duration::from_secs(3600), 100),
            Arc::new(MemoryKvDriver::new()),
            Duration::from_secs(3600),
            100,
        ));
    }

    #[test]
    fn durable_commit_preserves_context_held_only_by_the_backend() {
        // Pod 2 has never seen the session: the context to preserve exists
        // only in the shared backend, not in its memory tier.
        let shared_driver = Arc::new(MemoryKvDriver::new());
        let pod = |driver: Arc<MemoryKvDriver>| {
            DurableSsrDraftSessionStore::distributed(
                InMemorySsrDraftSessionStore::new(Duration::from_secs(3600), 100),
                driver,
                Duration::from_secs(3600),
                100,
            )
        };
        let pod1 = pod(shared_driver.clone());
        let pod2 = pod(shared_driver);
        let created = pod1
            .commit_with_context(None, None, controller("home"), json!({ "cart": 3 }), None)
            .expect("create");
        let committed = pod2
            .commit(
                Some(&created.token),
                Some(created.generation),
                controller("home"),
                None,
            )
            .expect("cold commit");
        assert_eq!(committed.generation, 2);
        assert_eq!(committed.runtime_context["cart"], 3);
    }

    #[test]
    fn in_memory_load_needs_only_the_shared_lock() {
        let store = InMemorySsrDraftSessionStore::default();
        let created = store
            .commit(None, None, controller("home"), None)
            .expect("create");
        // While a reader holds the map, `load` — including its TTL refresh —
        // must still complete (audit §5.3). With an exclusive lock it blocks.
        let guard = store.entries.read().expect("read guard");
        let (sender, receiver) = std::sync::mpsc::channel();
        let worker_store = store.clone();
        let token = created.token.clone();
        let worker = std::thread::spawn(move || {
            let loaded = worker_store.load(&token, "home", None);
            let _sent = sender.send(loaded.map(|snapshot| snapshot.map(|s| s.generation)));
        });
        let loaded = receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("load must not wait for an exclusive lock");
        drop(guard);
        worker.join().expect("worker");
        assert_eq!(loaded.expect("load"), Some(1));
    }

    #[test]
    fn in_memory_load_extends_ttl_and_generation_probe_does_not() {
        let store = InMemorySsrDraftSessionStore::new(Duration::from_secs(3600), 100);
        let created = store
            .commit(None, None, controller("home"), None)
            .expect("create");
        let deadline = |store: &InMemorySsrDraftSessionStore| {
            store.entries.read().expect("entries")[&created.token]
                .expires_at
                .get()
        };
        let initial = deadline(&store);

        std::thread::sleep(Duration::from_millis(2));
        assert_eq!(store.generation(&created.token).expect("probe"), Some(1));
        assert_eq!(
            deadline(&store),
            initial,
            "the probe must not touch the TTL"
        );
        assert_eq!(store.generation("unknown").expect("probe"), None);

        store
            .load(&created.token, "home", None)
            .expect("load")
            .expect("session");
        assert!(deadline(&store) > initial, "load must extend the TTL");
    }

    #[test]
    fn in_memory_expired_entries_are_invisible_and_reclaimed() {
        let store = InMemorySsrDraftSessionStore::new(Duration::from_secs(3600), 100);
        let created = store
            .commit(None, None, controller("home"), None)
            .expect("create");
        // Force the deadline into the past.
        store
            .entries
            .write()
            .expect("entries")
            .get_mut(&created.token)
            .expect("entry")
            .expires_at = EntryDeadline::new(0);
        assert_eq!(
            store.load(&created.token, "home", None).expect("load"),
            None
        );
        assert_eq!(store.generation(&created.token).expect("probe"), None);
        // `load` leaves the reclaim to the exclusive paths.
        assert_eq!(store.len().expect("len"), 1);
        assert_eq!(store.prune().expect("prune"), 1);
        assert!(store.is_empty().expect("empty"));
    }

    #[test]
    fn durable_prune_reports_entries_removed_from_both_tiers() {
        let driver = Arc::new(MemoryKvDriver::new());
        let store = DurableSsrDraftSessionStore::distributed(
            InMemorySsrDraftSessionStore::new(Duration::from_secs(3600), 100),
            driver.clone(),
            Duration::from_secs(3600),
            100,
        );
        let first = store
            .commit(None, None, controller("home"), None)
            .expect("first");
        let second = store
            .commit(None, None, controller("home"), None)
            .expect("second");
        assert_eq!(store.prune().expect("nothing expired"), 0);

        // Expire both sessions in the memory tier and one of them in the
        // backend: three entries go, and the count must say so (audit §6.6).
        {
            let mut entries = store.memory().entries.write().expect("entries");
            for entry in entries.values_mut() {
                entry.expires_at = EntryDeadline::new(0);
            }
        }
        let backend = store.backend();
        let mut expired = backend
            .load_record(&first.token, None)
            .expect("load")
            .expect("record");
        expired.expires_at_unix_secs = current_unix_secs().saturating_sub(1);
        let key = format!(
            "{}{}",
            ExternalKvDraftSessionBackend::DEFAULT_PREFIX,
            first.token
        );
        driver
            .set(&key, &serde_json::to_vec(&expired).expect("json"), 1, 3600)
            .expect("overwrite");

        assert_eq!(store.prune().expect("prune"), 3);
        assert!(
            backend
                .load_record(&second.token, None)
                .expect("load")
                .is_some()
        );
    }

    #[test]
    fn file_store_partitions_records_by_tenant() {
        let dir = temp_store_dir();
        let store = file_store(&dir);
        let alice = owner("acme", "user-alice");
        let carol = owner("Other/Tenant", "user-carol");
        let acme = store
            .commit(None, None, controller("home"), Some(&alice))
            .expect("acme session");
        let other = store
            .commit(None, None, controller("home"), Some(&carol))
            .expect("other session");
        let anonymous = store
            .commit(None, None, controller("home"), None)
            .expect("anonymous session");

        // One directory per tenant; ids are encoded, never used as paths.
        assert!(
            dir.join("t-acme")
                .join(format!("{}.json", acme.token))
                .is_file()
        );
        assert!(
            dir.join("t-_4fther_2f_54enant")
                .join(format!("{}.json", other.token))
                .is_file()
        );
        assert!(
            dir.join("_unattributed")
                .join(format!("{}.json", anonymous.token))
                .is_file()
        );
        // Nothing but directories at the top level: no record, lock, or
        // temporary file of one tenant sits next to another tenant's.
        for entry in fs::read_dir(&dir).expect("read dir").flatten() {
            assert!(
                entry.file_type().expect("file type").is_dir(),
                "{:?}",
                entry.path()
            );
        }

        // A token of another tenant is rejected exactly like in the other
        // stores, for every operation, and stays intact.
        assert!(matches!(
            store.load(&acme.token, "home", Some(&carol)),
            Err(SsrDraftSessionError::OwnerMismatch)
        ));
        assert!(matches!(
            store.commit(Some(&acme.token), Some(1), controller("home"), Some(&carol)),
            Err(SsrDraftSessionError::OwnerMismatch)
        ));
        assert!(matches!(
            store.remove(&acme.token, Some(&carol)),
            Err(SsrDraftSessionError::OwnerMismatch)
        ));
        assert!(matches!(
            store.load(&acme.token, "home", None),
            Err(SsrDraftSessionError::OwnerMismatch)
        ));
        // Same tenant, different user: also rejected.
        assert!(matches!(
            store.load(&acme.token, "home", Some(&owner("acme", "user-bob"))),
            Err(SsrDraftSessionError::OwnerMismatch)
        ));
        let loaded = store
            .load(&acme.token, "home", Some(&alice))
            .expect("owner load")
            .expect("session");
        assert_eq!(loaded.generation, 1);

        store
            .remove(&acme.token, Some(&alice))
            .expect("owner remove");
        assert_eq!(
            store.load(&acme.token, "home", Some(&alice)).expect("load"),
            None
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_store_partition_names_are_injective_and_bounded() {
        let name =
            |tenant: &str| FileSsrDraftSessionStore::partition_name(Some(&owner(tenant, "user")));
        assert_eq!(name("acme-1").expect("slug"), "t-acme-1");
        // Case, separators and the escape character itself are all encoded,
        // so no two tenant ids can meet in one directory.
        let distinct = [
            "acme", "Acme", "ACME", "ac_6de", "ac/me", "../acme", "acme.", "",
        ];
        let names: BTreeSet<String> = distinct
            .iter()
            .map(|tenant| name(tenant).expect("name"))
            .collect();
        assert_eq!(names.len(), distinct.len());
        for encoded in &names {
            assert!(
                encoded.bytes().all(|b| b.is_ascii_lowercase()
                    || b.is_ascii_digit()
                    || b == b'-'
                    || b == b'_')
            );
            assert_ne!(encoded, UNATTRIBUTED_PARTITION);
            assert_ne!(encoded, LOCKS_DIR);
        }
        assert!(matches!(
            name(&"x".repeat(MAX_PARTITION_NAME_BYTES)),
            Err(SsrDraftSessionError::InvalidOwner(_))
        ));
        assert_eq!(
            FileSsrDraftSessionStore::partition_name(None).expect("unattributed"),
            UNATTRIBUTED_PARTITION
        );
    }

    #[test]
    fn file_store_adoption_moves_the_record_into_the_tenant_partition() {
        let dir = temp_store_dir();
        let store = file_store(&dir);
        let legacy = store
            .commit_with_context(
                None,
                None,
                controller("home"),
                json!({ "kept": true }),
                None,
            )
            .expect("unattributed create");
        let file_name = format!("{}.json", legacy.token);
        let alice = owner("acme", "user-alice");

        // Loadable by an attributed caller while still unattributed.
        let loaded = store
            .load(&legacy.token, "home", Some(&alice))
            .expect("load")
            .expect("session");
        assert!(loaded.owner.is_none());

        let adopted = store
            .commit(
                Some(&legacy.token),
                Some(1),
                controller("home"),
                Some(&alice),
            )
            .expect("adopt");
        assert_eq!(adopted.owner.as_ref(), Some(&alice));
        assert_eq!(adopted.runtime_context["kept"], true);
        assert!(dir.join("t-acme").join(&file_name).is_file());
        assert!(!dir.join(UNATTRIBUTED_PARTITION).join(&file_name).exists());

        // Anonymous and foreign callers are locked out from now on.
        assert!(matches!(
            store.load(&legacy.token, "home", None),
            Err(SsrDraftSessionError::OwnerMismatch)
        ));
        assert!(matches!(
            store.load(&legacy.token, "home", Some(&owner("evil", "user-alice"))),
            Err(SsrDraftSessionError::OwnerMismatch)
        ));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn durable_file_store_routes_by_tenant_after_cold_restart() {
        let dir = temp_store_dir();
        let alice = owner("acme", "user-alice");
        let durable = |dir: &Path| {
            DurableSsrDraftSessionStore::new(
                InMemorySsrDraftSessionStore::new(Duration::from_secs(3600), 100),
                file_store(dir),
            )
        };
        let created = durable(&dir)
            .commit_with_context(
                None,
                None,
                controller("home"),
                json!({ "n": 1 }),
                Some(&alice),
            )
            .expect("create");

        // A fresh process: nothing in memory, everything from the partition.
        let restarted = durable(&dir);
        assert!(matches!(
            restarted.load(&created.token, "home", Some(&owner("evil", "user-alice"))),
            Err(SsrDraftSessionError::OwnerMismatch)
        ));
        let loaded = restarted
            .load(&created.token, "home", Some(&alice))
            .expect("load")
            .expect("session");
        assert_eq!(loaded.runtime_context["n"], 1);
        let committed = restarted
            .commit(
                Some(&created.token),
                Some(1),
                controller("home"),
                Some(&alice),
            )
            .expect("commit");
        assert_eq!(committed.generation, 2);
        restarted
            .remove(&created.token, Some(&alice))
            .expect("remove");
        assert!(
            !dir.join("t-acme")
                .join(format!("{}.json", created.token))
                .exists()
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn file_store_keeps_directories_and_files_private() {
        use std::os::unix::fs::PermissionsExt;
        let mode = |path: &Path| {
            fs::symlink_metadata(path)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o7777
        };

        // A pre-existing world-readable directory is tightened, not trusted.
        let dir = temp_store_dir();
        fs::create_dir_all(&dir).expect("precreate");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).expect("chmod");
        let store = file_store(&dir);
        assert_eq!(mode(&dir), 0o700);
        assert_eq!(mode(&dir.join(LOCKS_DIR)), 0o700);

        let alice = owner("acme", "user-alice");
        let created = store
            .commit(None, None, controller("home"), Some(&alice))
            .expect("create");
        store
            .commit(
                Some(&created.token),
                Some(1),
                controller("home"),
                Some(&alice),
            )
            .expect("update");
        let partition = dir.join("t-acme");
        assert_eq!(mode(&partition), 0o700);
        assert_eq!(
            mode(&partition.join(format!("{}.json", created.token))),
            0o600
        );

        // Lock files are private too, while they exist.
        with_token_lock(&dir.join(LOCKS_DIR), "tokLOCK01", || {
            assert_eq!(mode(&dir.join(LOCKS_DIR).join("tokLOCK01.lock")), 0o600);
            Ok(())
        })
        .expect("lock");
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn file_store_checks_the_storage_root_owner_and_leaves_no_probe() {
        // The refusal itself needs a directory owned by another user, which a
        // test cannot create without privileges; this covers the accepting
        // path and that the ownership probe does not stay in the root.
        let dir = temp_store_dir();
        let _store = file_store(&dir);
        ensure_owned_by_process(&dir).expect("own directory is accepted");
        let entries: Vec<String> = fs::read_dir(&dir)
            .expect("read root")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries, vec![LOCKS_DIR.to_string()]);

        // A root the process cannot write to is not usable, hence refused.
        let missing = dir.join("absent");
        assert!(ensure_owned_by_process(&missing).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn stale_token_lock_that_cannot_be_removed_fails_instead_of_retrying_forever() {
        // A directory stands in for a lock file that cannot be deleted:
        // `create_new` reports it as existing and `remove_file` refuses it.
        // The waiter used to treat the failed removal as done and retry in a
        // loop that never reached its deadline.
        let dir = temp_store_dir();
        let lock = dir.join("tokSTALE01.lock");
        fs::create_dir_all(&lock).expect("undeletable lock");
        let long_ago = SystemTime::now() - TOKEN_LOCK_STALE_AFTER - Duration::from_secs(60);
        fs::File::open(&lock)
            .expect("open lock")
            .set_modified(long_ago)
            .expect("age lock");

        let outcome = with_token_lock(&dir, "tokSTALE01", || Ok(()));
        assert!(matches!(outcome, Err(SsrDraftSessionError::Io(_))));
        fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn file_store_refuses_a_symlinked_directory() {
        let real = temp_store_dir();
        fs::create_dir_all(&real).expect("real dir");
        let link = temp_store_dir();
        std::os::unix::fs::symlink(&real, &link).expect("symlink");
        assert!(matches!(
            FileSsrDraftSessionStore::new(&link, Duration::from_secs(3600), 100),
            Err(SsrDraftSessionError::Io(_))
        ));
        // Nothing was created behind the link.
        assert_eq!(fs::read_dir(&real).expect("read dir").count(), 0);

        // A planted symlink in place of a tenant partition is refused as well.
        let dir = temp_store_dir();
        let store = file_store(&dir);
        std::os::unix::fs::symlink(&real, dir.join("t-acme")).expect("partition symlink");
        assert!(matches!(
            store.commit(None, None, controller("home"), Some(&owner("acme", "user"))),
            Err(SsrDraftSessionError::Io(_))
        ));
        assert_eq!(fs::read_dir(&real).expect("read dir").count(), 0);

        let _ = fs::remove_file(&link);
        let _ = fs::remove_dir_all(&real);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_store_unavailable_directory_fails_closed() {
        // What `editor_default` builds when the directory cannot be secured.
        let dir = temp_store_dir();
        let store = FileSsrDraftSessionStore {
            dir: dir.clone(),
            ttl: Duration::from_secs(3600),
            maximum_entries: 100,
            unavailable: Some("draft session directory is not usable".to_string()),
        };
        let unusable = |result: Result<(), SsrDraftSessionError>| matches!(result, Err(SsrDraftSessionError::Io(reason)) if reason.contains("not usable"));
        assert!(unusable(
            store
                .commit(None, None, controller("home"), None)
                .map(|_| ())
        ));
        assert!(unusable(store.load("tokA", "home", None).map(|_| ())));
        assert!(unusable(store.remove("tokA", None)));
        assert!(unusable(store.prune().map(|_| ())));
        assert!(
            !dir.exists(),
            "a store that failed closed must not write anything"
        );
    }

    #[test]
    fn file_store_directory_is_configurable() {
        let configured =
            FileSsrDraftSessionStore::resolve_dir(Some("/var/lib/rustok/drafts".into()));
        assert_eq!(configured, PathBuf::from("/var/lib/rustok/drafts"));
        let fallback = std::env::temp_dir().join("rustok_draft_sessions");
        assert_eq!(FileSsrDraftSessionStore::resolve_dir(None), fallback);
        assert_eq!(
            FileSsrDraftSessionStore::resolve_dir(Some("".into())),
            fallback
        );
    }

    #[test]
    fn atomic_write_replaces_in_place_and_leaves_no_temporary_file() {
        let dir = temp_store_dir();
        fs::create_dir_all(&dir).expect("dir");
        let target = dir.join("record.json");
        atomic_write_file(&target, b"first").expect("create");
        atomic_write_file(&target, b"second").expect("replace");
        assert_eq!(fs::read(&target).expect("read"), b"second");
        let names: Vec<String> = fs::read_dir(&dir)
            .expect("read dir")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["record.json".to_string()]);

        // A failed write (the target's directory is gone) reports the error.
        assert!(atomic_write_file(&dir.join("missing").join("record.json"), b"x").is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_store_load_skips_rewrite_while_ttl_is_fresh_and_extends_it_later() {
        let dir = temp_store_dir();
        let store = file_store(&dir);
        let created = store
            .commit(None, None, controller("home"), None)
            .expect("create");
        let path = dir
            .join(UNATTRIBUTED_PARTITION)
            .join(format!("{}.json", created.token));
        let modified = |path: &Path| {
            fs::metadata(path)
                .expect("metadata")
                .modified()
                .expect("mtime")
        };
        let expires = |store: &FileSsrDraftSessionStore| {
            store
                .load_record(&created.token, None)
                .expect("load record")
                .expect("record")
                .expires_at_unix_secs
        };

        // A full TTL: reading must not rewrite the record.
        let written = modified(&path);
        store
            .load(&created.token, "home", None)
            .expect("load")
            .expect("session");
        assert_eq!(modified(&path), written);

        // Half of the TTL consumed: the read extends it, generation intact.
        let mut aged = store
            .load_record(&created.token, None)
            .expect("load")
            .expect("record");
        aged.expires_at_unix_secs = current_unix_secs() + 1800;
        store.save_record(&aged).expect("age record");
        let loaded = store
            .load(&created.token, "home", None)
            .expect("load")
            .expect("session");
        assert_eq!(loaded.generation, 1);
        assert!(expires(&store) >= current_unix_secs() + 3590);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_store_load_reports_a_failed_ttl_refresh() {
        let dir = temp_store_dir();
        let store = file_store(&dir);
        let created = store
            .commit(None, None, controller("home"), None)
            .expect("create");
        let mut aged = store
            .load_record(&created.token, None)
            .expect("load")
            .expect("record");
        aged.expires_at_unix_secs = current_unix_secs() + 1800;
        store.save_record(&aged).expect("age record");

        // Another process holds the token lock for longer than the wait
        // deadline, so the TTL cannot be extended. The caller must hear about
        // it instead of losing the draft silently later (audit §6.7).
        let lock_path = dir.join(LOCKS_DIR).join(format!("{}.lock", created.token));
        fs::write(&lock_path, "another-holder").expect("foreign lock");
        assert!(matches!(
            store.load(&created.token, "home", None),
            Err(SsrDraftSessionError::LockTimeout(_))
        ));

        // Once the lock is released the same read succeeds and extends.
        fs::remove_file(&lock_path).expect("release");
        store
            .load(&created.token, "home", None)
            .expect("load")
            .expect("session");
        let refreshed = store
            .load_record(&created.token, None)
            .expect("load")
            .expect("record");
        assert!(refreshed.expires_at_unix_secs >= current_unix_secs() + 3590);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_store_prune_collects_expired_files_of_the_flat_layout() {
        let dir = temp_store_dir();
        let store = file_store(&dir);
        let legacy = [
            "tokOLD01.json",
            "tokOLD01.lock",
            "tokOLD02.tmp.0123456789abcdef",
        ];
        let foreign = ["README.txt", "notes", "bad token.json", ".hidden.json"];
        for name in legacy.iter().chain(&foreign) {
            fs::write(dir.join(name), "{}").expect("write");
        }

        // Fresh files may belong to a process still on the old layout.
        assert_eq!(store.prune().expect("prune fresh"), 0);
        for name in legacy.iter().chain(&foreign) {
            assert!(dir.join(name).is_file(), "{name} must survive");
        }

        let long_ago = SystemTime::now() - store.ttl() - Duration::from_secs(5);
        for name in legacy.iter().chain(&foreign) {
            fs::File::options()
                .write(true)
                .open(dir.join(name))
                .and_then(|file| file.set_modified(long_ago))
                .expect("backdate");
        }
        // Only the record counts as a pruned session; files the store never
        // wrote are left alone however old they are.
        assert_eq!(store.prune().expect("prune expired"), 1);
        for name in legacy {
            assert!(!dir.join(name).exists(), "{name} must be collected");
        }
        for name in foreign {
            assert!(dir.join(name).is_file(), "{name} is not ours to delete");
        }
    }

    #[test]
    fn file_store_prune_spans_partitions_and_spares_writes_in_flight() {
        let dir = temp_store_dir();
        let store = file_store(&dir);
        let alice = owner("acme", "user-alice");
        let live = store
            .commit(None, None, controller("home"), Some(&alice))
            .expect("live");
        let stale = store
            .commit(None, None, controller("home"), None)
            .expect("stale");
        let mut expired = store
            .load_record(&stale.token, None)
            .expect("load")
            .expect("record");
        expired.expires_at_unix_secs = current_unix_secs().saturating_sub(1);
        store.save_record(&expired).expect("expire record");

        // A temporary file and a lock that were just created belong to an
        // operation in flight and must survive the prune.
        let in_flight = dir.join("t-acme").join("tokNEW01.tmp.0123456789abcdef");
        fs::write(&in_flight, "partial").expect("tmp");
        let lock = dir.join(LOCKS_DIR).join("tokNEW01.lock");
        fs::write(&lock, "holder").expect("lock");

        assert_eq!(store.prune().expect("prune"), 1);
        assert!(in_flight.is_file());
        assert!(lock.is_file());
        assert_eq!(store.load(&stale.token, "home", None).expect("load"), None);
        assert!(
            store
                .load(&live.token, "home", Some(&alice))
                .expect("load")
                .is_some()
        );

        // Abandoned ones (older than the stale threshold) are collected.
        let long_ago = SystemTime::now() - TOKEN_LOCK_STALE_AFTER - Duration::from_secs(5);
        for path in [&in_flight, &lock] {
            fs::File::options()
                .write(true)
                .open(path)
                .and_then(|file| file.set_modified(long_ago))
                .expect("backdate");
        }
        assert_eq!(store.prune().expect("prune abandoned"), 1);
        assert!(!in_flight.exists());
        assert!(!lock.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    fn backdate(path: &Path, age: Duration) {
        fs::File::options()
            .write(true)
            .open(path)
            .and_then(|file| file.set_modified(SystemTime::now() - age))
            .expect("backdate");
    }

    #[test]
    fn file_store_deletes_an_expired_record_only_under_the_token_lock() {
        let dir = temp_store_dir();
        let store = file_store(&dir);
        let created = store
            .commit(None, None, controller("home"), None)
            .expect("create");
        let mut expired = store
            .load_record(&created.token, None)
            .expect("load")
            .expect("record");
        expired.expires_at_unix_secs = current_unix_secs().saturating_sub(1);
        store.save_record(&expired).expect("expire record");
        let file = dir
            .join(UNATTRIBUTED_PARTITION)
            .join(format!("{}.json", created.token));

        // A plain read holds no lock, and it could have read the file just
        // before a commit replaced it. While a commit holds the lock the read
        // cannot delete anything: it reports the session as gone and leaves
        // the file alone.
        let lock = dir.join(LOCKS_DIR).join(format!("{}.lock", created.token));
        fs::write(&lock, "holder").expect("hold the lock");
        assert_eq!(
            store.load(&created.token, "home", None).expect("load"),
            None
        );
        assert!(file.is_file());

        // With the lock free the same read collects the file.
        fs::remove_file(&lock).expect("release the lock");
        assert_eq!(
            store.load(&created.token, "home", None).expect("load"),
            None
        );
        assert!(!file.exists());

        // A commit that names the token starts a new session.
        let fresh = store
            .commit(Some(&created.token), Some(1), controller("home"), None)
            .expect("commit after expiry");
        assert_ne!(fresh.token, created.token);
        assert_eq!(fresh.generation, 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_store_read_returns_the_record_a_commit_wrote_while_the_read_waited() {
        let dir = temp_store_dir();
        let store = file_store(&dir);
        let created = store
            .commit(None, None, controller("home"), None)
            .expect("create");
        let mut record = store
            .load_record(&created.token, None)
            .expect("load")
            .expect("record");
        record.expires_at_unix_secs = current_unix_secs().saturating_sub(1);
        store.save_record(&record).expect("expire record");
        let file = dir
            .join(UNATTRIBUTED_PARTITION)
            .join(format!("{}.json", created.token));

        // A commit holds the token lock. It read the record before the
        // deadline and writes generation 2 after the plain read below has
        // already seen generation 1 expired.
        let lock = dir.join(LOCKS_DIR).join(format!("{}.lock", created.token));
        fs::write(&lock, "committing").expect("hold the lock");
        record.generation = 2;
        record.expires_at_unix_secs = current_unix_secs() + 3600;
        let committed = serde_json::to_vec(&record).expect("json");
        let commit = {
            let (file, lock) = (file.clone(), lock.clone());
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(300));
                fs::write(&file, committed).expect("commit");
                fs::remove_file(&lock).expect("release the lock");
            })
        };

        let loaded = store
            .load_record(&created.token, None)
            .expect("load")
            .expect("the record the commit wrote");
        commit.join().expect("commit thread");
        assert_eq!(loaded.generation, 2);
        assert!(file.is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_store_treats_an_unreadable_record_as_expired_after_one_ttl() {
        let dir = temp_store_dir();
        let store = file_store(&dir);
        let alice = owner("acme", "user-alice");
        let created = store
            .commit(None, None, controller("home"), Some(&alice))
            .expect("create");
        let file = dir.join("t-acme").join(format!("{}.json", created.token));
        // A record of another build's shape, or a damaged one.
        fs::write(&file, "{ \"token\": 1 }").expect("damage");

        // Written within the TTL it may be another build's live session:
        // reported, never discarded.
        assert!(matches!(
            store.load(&created.token, "home", Some(&alice)),
            Err(SsrDraftSessionError::Serialization(_))
        ));
        assert!(matches!(
            store.commit(
                Some(&created.token),
                Some(1),
                controller("home"),
                Some(&alice)
            ),
            Err(SsrDraftSessionError::Serialization(_))
        ));
        assert_eq!(store.prune().expect("prune fresh"), 0);
        assert!(file.is_file());

        // A whole TTL without a write: the session has expired whatever the
        // file contains, and the token stops failing every request.
        backdate(&file, store.ttl() + Duration::from_secs(5));
        assert_eq!(
            store
                .load(&created.token, "home", Some(&alice))
                .expect("load after the TTL"),
            None
        );
        assert!(!file.exists());
        let fresh = store
            .commit(
                Some(&created.token),
                Some(1),
                controller("home"),
                Some(&alice),
            )
            .expect("commit after the TTL");
        assert_ne!(fresh.token, created.token);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_store_prune_collects_unreadable_records_after_one_ttl() {
        let dir = temp_store_dir();
        let store = file_store(&dir);
        let alice = owner("acme", "user-alice");
        let live = store
            .commit(None, None, controller("home"), Some(&alice))
            .expect("live");
        let damaged = dir.join("t-acme").join("tokDAMAGED01.json");
        fs::write(&damaged, "not a record").expect("damage");
        let foreign = dir.join("t-acme").join("bad token.json");
        fs::write(&foreign, "not a record").expect("foreign");

        assert_eq!(store.prune().expect("prune fresh"), 0);
        assert!(damaged.is_file());

        let long_ago = store.ttl() + Duration::from_secs(5);
        backdate(&damaged, long_ago);
        backdate(&foreign, long_ago);
        assert_eq!(store.prune().expect("prune after the TTL"), 1);
        assert!(!damaged.exists());
        assert!(foreign.is_file(), "not a file this store wrote");
        assert!(
            store
                .load(&live.token, "home", Some(&alice))
                .expect("load")
                .is_some()
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// Key-value driver that runs a hook once, right before the first
    /// conditional delete reaches the store. That is where a commit of
    /// another pod can land after the backend has read an expired record.
    struct CommitBeforeDeleteDriver {
        inner: MemoryKvDriver,
        before_delete: std::sync::Mutex<Option<Box<dyn FnOnce(&MemoryKvDriver) + Send>>>,
    }

    impl CommitBeforeDeleteDriver {
        fn new(before_delete: impl FnOnce(&MemoryKvDriver) + Send + 'static) -> Self {
            let hook: Box<dyn FnOnce(&MemoryKvDriver) + Send> = Box::new(before_delete);
            Self {
                inner: MemoryKvDriver::new(),
                before_delete: std::sync::Mutex::new(Some(hook)),
            }
        }
    }

    impl DraftSessionKvDriver for CommitBeforeDeleteDriver {
        fn get(&self, key: &str) -> Result<Option<Vec<u8>>, SsrDraftSessionError> {
            self.inner.get(key)
        }

        fn set(
            &self,
            key: &str,
            value: &[u8],
            cas: u64,
            ttl_secs: u64,
        ) -> Result<(), SsrDraftSessionError> {
            self.inner.set(key, value, cas, ttl_secs)
        }

        fn compare_and_set(
            &self,
            key: &str,
            expected_cas: Option<u64>,
            value: &[u8],
            cas: u64,
            ttl_secs: u64,
        ) -> Result<bool, SsrDraftSessionError> {
            self.inner
                .compare_and_set(key, expected_cas, value, cas, ttl_secs)
        }

        fn delete(&self, key: &str) -> Result<(), SsrDraftSessionError> {
            self.inner.delete(key)
        }

        fn compare_and_delete(
            &self,
            key: &str,
            expected_cas: u64,
        ) -> Result<bool, SsrDraftSessionError> {
            let hook = self.before_delete.lock().expect("hook").take();
            if let Some(hook) = hook {
                hook(&self.inner);
            }
            self.inner.compare_and_delete(key, expected_cas)
        }

        fn scan_keys(&self, prefix: &str) -> Result<Vec<String>, SsrDraftSessionError> {
            self.inner.scan_keys(prefix)
        }
    }

    fn kv_key(token: &str) -> String {
        format!("{}{token}", ExternalKvDraftSessionBackend::DEFAULT_PREFIX)
    }

    /// Stores a record of `token` under a driver TTL of an hour, whatever
    /// its own expiry says: the state of a driver whose TTL has not fired
    /// yet for a record that is already past its deadline.
    fn kv_put(driver: &MemoryKvDriver, token: &str, generation: u64, expires_at_unix_secs: u64) {
        let record = PersistentDraftSessionRecord {
            token: token.to_string(),
            generation,
            controller: controller("home"),
            runtime_context: json!({ "generation": generation }),
            owner: None,
            expires_at_unix_secs,
        };
        driver
            .set(
                &kv_key(token),
                &serde_json::to_vec(&record).expect("json"),
                generation,
                3600,
            )
            .expect("set");
    }

    #[test]
    fn kv_driver_compare_and_delete_requires_the_stored_token() {
        let driver = MemoryKvDriver::new();
        driver.set("k", b"v1", 1, 3600).expect("set");

        // Another token leaves the key in place.
        assert!(!driver.compare_and_delete("k", 2).expect("stale token"));
        assert_eq!(driver.get("k").unwrap().as_deref(), Some(b"v1".as_slice()));

        // The stored token deletes it, once.
        assert!(driver.compare_and_delete("k", 1).expect("stored token"));
        assert!(driver.get("k").unwrap().is_none());
        assert!(!driver.compare_and_delete("k", 1).expect("absent key"));
    }

    #[test]
    fn kv_backend_drops_an_expired_record_on_read() {
        let driver = Arc::new(MemoryKvDriver::new());
        let backend =
            ExternalKvDraftSessionBackend::new(driver.clone(), Duration::from_secs(3600), 100);
        kv_put(
            &driver,
            "tokEXP01",
            4,
            current_unix_secs().saturating_sub(1),
        );

        assert!(
            backend
                .load_record("tokEXP01", None)
                .expect("load")
                .is_none()
        );
        assert!(driver.is_empty());
    }

    #[test]
    fn kv_backend_read_keeps_a_commit_that_replaced_an_expired_record() {
        // The read finds generation 4 expired. Before its delete reaches the
        // store, another pod commits generation 5 with a fresh deadline.
        let driver = Arc::new(CommitBeforeDeleteDriver::new(|store| {
            kv_put(store, "tokEXP02", 5, current_unix_secs() + 3600);
        }));
        kv_put(
            &driver.inner,
            "tokEXP02",
            4,
            current_unix_secs().saturating_sub(1),
        );
        let backend =
            ExternalKvDraftSessionBackend::new(driver.clone(), Duration::from_secs(3600), 100);

        let loaded = backend
            .load_record("tokEXP02", None)
            .expect("load")
            .expect("the commit that landed during the read");
        assert_eq!(loaded.generation, 5);
        assert!(
            driver
                .inner
                .get(&kv_key("tokEXP02"))
                .expect("get")
                .is_some()
        );
    }

    #[test]
    fn kv_backend_reports_an_unparsable_record_without_deleting_it() {
        let driver = Arc::new(MemoryKvDriver::new());
        let backend =
            ExternalKvDraftSessionBackend::new(driver.clone(), Duration::from_secs(3600), 100);
        driver
            .set(&kv_key("tokBAD01"), b"not a record", 1, 3600)
            .expect("set");

        assert!(matches!(
            backend.load_record("tokBAD01", None),
            Err(SsrDraftSessionError::Serialization(_))
        ));
        // Neither the read nor `prune` deletes it: the TTL the driver stored
        // it with does.
        assert_eq!(
            backend
                .prune_records(current_unix_secs(), 100)
                .expect("prune"),
            0
        );
        assert_eq!(driver.len(), 1);
    }

    #[test]
    fn kv_prune_keeps_a_session_committed_during_the_scan() {
        let driver = Arc::new(CommitBeforeDeleteDriver::new(|store| {
            kv_put(store, "tokEXP03", 5, current_unix_secs() + 3600);
        }));
        kv_put(
            &driver.inner,
            "tokEXP03",
            4,
            current_unix_secs().saturating_sub(1),
        );
        let backend =
            ExternalKvDraftSessionBackend::new(driver.clone(), Duration::from_secs(3600), 100);

        // The scan saw generation 4 expired; the record there now is not it.
        assert_eq!(
            backend
                .prune_records(current_unix_secs(), 100)
                .expect("prune"),
            0
        );
        let kept = backend
            .load_record("tokEXP03", None)
            .expect("load")
            .expect("committed session");
        assert_eq!(kept.generation, 5);

        // An expired record nobody touched is collected and counted.
        kv_put(
            &driver.inner,
            "tokEXP04",
            2,
            current_unix_secs().saturating_sub(1),
        );
        assert_eq!(
            backend
                .prune_records(current_unix_secs(), 100)
                .expect("prune"),
            1
        );
        assert!(
            driver
                .inner
                .get(&kv_key("tokEXP04"))
                .expect("get")
                .is_none()
        );
    }
}
