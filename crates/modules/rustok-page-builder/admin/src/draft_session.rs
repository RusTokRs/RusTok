use crate::AdminCanvasController;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeSet, HashMap};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
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

    fn commit(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError>;

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

    fn prune(&self) -> Result<usize, SsrDraftSessionError>;
}

#[derive(Clone)]
pub struct InMemorySsrDraftSessionStore {
    entries: Arc<RwLock<HashMap<String, DraftEntry>>>,
    ttl: Duration,
    maximum_entries: usize,
}

#[derive(Debug, Clone)]
struct DraftEntry {
    controller: AdminCanvasController,
    runtime_context: Value,
    generation: u64,
    owner: Option<DraftSessionOwner>,
    expires_at: Instant,
}

impl InMemorySsrDraftSessionStore {
    pub fn new(ttl: Duration, maximum_entries: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            ttl: ttl.max(Duration::from_secs(30)),
            maximum_entries: maximum_entries.max(1),
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

    pub fn insert_snapshot(
        &self,
        snapshot: SsrDraftSessionSnapshot,
    ) -> Result<(), SsrDraftSessionError> {
        let mut entries = self
            .entries
            .write()
            .map_err(|_| SsrDraftSessionError::Poisoned)?;
        let now = Instant::now();
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
                expires_at: now + self.ttl,
            },
        );
        Ok(())
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

    fn prune_locked(entries: &mut HashMap<String, DraftEntry>, now: Instant) -> usize {
        let before = entries.len();
        entries.retain(|_, entry| entry.expires_at > now);
        before.saturating_sub(entries.len())
    }

    fn evict_oldest(entries: &mut HashMap<String, DraftEntry>, maximum_entries: usize) {
        while entries.len() >= maximum_entries {
            let Some(oldest) = entries
                .iter()
                .min_by_key(|(_, entry)| entry.expires_at)
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
        let mut owned: Vec<(String, Instant)> = entries
            .iter()
            .filter(|(_, entry)| entry.owner.as_ref() == Some(owner))
            .map(|(token, entry)| (token.clone(), entry.expires_at))
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
        let now = Instant::now();
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
                entry.expires_at = now + self.ttl;
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
                expires_at: now + self.ttl,
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
        let mut entries = self
            .entries
            .write()
            .map_err(|_| SsrDraftSessionError::Poisoned)?;
        let now = Instant::now();
        Self::prune_locked(&mut entries, now);
        let Some(entry) = entries.get_mut(token) else {
            return Ok(None);
        };
        if entry.controller.page_id() != page_id {
            return Err(SsrDraftSessionError::PageMismatch {
                expected: entry.controller.page_id().to_string(),
                actual: page_id.to_string(),
            });
        }
        ensure_session_owner(entry.owner.as_ref(), owner)?;
        entry.expires_at = now + self.ttl;
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
        Ok(Self::prune_locked(&mut entries, Instant::now()))
    }
}

/// Pluggable persistent storage backend for SSR draft sessions.
///
/// Backends may be local filesystem directories (`FileSsrDraftSessionStore`),
/// distributed key-value caches (`ExternalKvDraftSessionBackend` for Redis/KV),
/// or database tables.
pub trait PersistentDraftSessionBackend: Send + Sync {
    /// Loads the persistent record for the given token.
    fn load_record(
        &self,
        token: &str,
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

    /// Deletes the record associated with the token.
    fn delete_record(&self, token: &str) -> Result<(), SsrDraftSessionError>;

    /// Prunes expired records and enforces capacity limits.
    fn prune_records(
        &self,
        now_unix_secs: u64,
        max_entries: usize,
    ) -> Result<usize, SsrDraftSessionError>;
}

#[derive(Clone)]
pub struct FileSsrDraftSessionStore {
    dir: PathBuf,
    ttl: Duration,
    maximum_entries: usize,
}

impl FileSsrDraftSessionStore {
    pub fn new(
        dir: impl Into<PathBuf>,
        ttl: Duration,
        maximum_entries: usize,
    ) -> Result<Self, SsrDraftSessionError> {
        let dir = dir.into();
        fs::create_dir_all(&dir).map_err(|e| SsrDraftSessionError::Io(e.to_string()))?;
        Ok(Self {
            dir,
            ttl: ttl.max(Duration::from_secs(30)),
            maximum_entries: maximum_entries.max(1),
        })
    }

    pub fn default_dir() -> PathBuf {
        std::env::temp_dir().join("rustok_draft_sessions")
    }

    pub fn editor_default() -> Self {
        Self::new(
            Self::default_dir(),
            Duration::from_secs(60 * 60 * 8),
            10_000,
        )
        .unwrap_or_else(|_| Self {
            dir: Self::default_dir(),
            ttl: Duration::from_secs(60 * 60 * 8),
            maximum_entries: 10_000,
        })
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

    pub fn validate_token(token: &str) -> Result<(), SsrDraftSessionError> {
        if Self::is_safe_token(token) {
            Ok(())
        } else {
            Err(SsrDraftSessionError::InvalidToken(token.to_string()))
        }
    }

    pub fn save_record(
        &self,
        record: &PersistentDraftSessionRecord,
    ) -> Result<(), SsrDraftSessionError> {
        Self::validate_token(&record.token)?;
        let json_bytes = serde_json::to_vec_pretty(record)
            .map_err(|e| SsrDraftSessionError::Serialization(e.to_string()))?;
        ensure_record_size(json_bytes.len())?;
        let target_path = self.dir.join(format!("{}.json", record.token));
        if !target_path.is_file() {
            self.enforce_owner_cap(&record.owner, &record.token)?;
        }
        atomic_write_file(&target_path, &json_bytes)
            .map_err(|e| SsrDraftSessionError::Io(e.to_string()))?;
        Ok(())
    }

    /// Deletes the oldest same-owner records beyond
    /// [`MAX_DRAFT_SESSIONS_PER_OWNER`] (audit §2.5). Only runs when this
    /// record is new — updating an existing session does not change counts.
    fn enforce_owner_cap(
        &self,
        owner: &Option<DraftSessionOwner>,
        keep_token: &str,
    ) -> Result<(), SsrDraftSessionError> {
        let Some(owner) = owner.as_ref() else {
            return Ok(());
        };
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(SsrDraftSessionError::Io(err.to_string())),
        };
        let mut owned: Vec<(PathBuf, u64)> = Vec::new();
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
                owned.push((path, record.expires_at_unix_secs));
            }
        }
        if owned.len() >= MAX_DRAFT_SESSIONS_PER_OWNER {
            owned.sort_by_key(|(_, expires_at)| *expires_at);
            let excess = owned.len() + 1 - MAX_DRAFT_SESSIONS_PER_OWNER;
            for (path, _) in owned.into_iter().take(excess) {
                let _ = fs::remove_file(path);
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

    /// Commits an update to an existing record while holding the token lock,
    /// closing the read-check-write TOCTOU window between pods and processes
    /// sharing this directory (audit §4.1).
    ///
    /// The caller's inputs are bounced back in [`TokenLockCommit::Missing`]
    /// when the record is absent or expired, so a fresh session can be
    /// created from them afterwards.
    fn commit_existing_under_lock(
        &self,
        token: &str,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
        runtime_context: Value,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<TokenLockCommit, SsrDraftSessionError> {
        with_token_lock(&self.dir, token, move || {
            let Some(mut record) = self.load_record(token)? else {
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
            record.runtime_context = runtime_context;
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
            self.save_snapshot(&snapshot)?;
            Ok(TokenLockCommit::Committed(snapshot))
        })
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
    ) -> Result<Option<PersistentDraftSessionRecord>, SsrDraftSessionError> {
        let token = token.trim();
        if token.is_empty() || !Self::is_safe_token(token) {
            return Ok(None);
        }
        let file_path = self.dir.join(format!("{token}.json"));
        if !file_path.is_file() {
            return Ok(None);
        }
        let content = match fs::read_to_string(&file_path) {
            Ok(content) => content,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(SsrDraftSessionError::Io(err.to_string())),
        };
        let record: PersistentDraftSessionRecord = serde_json::from_str(&content)
            .map_err(|e| SsrDraftSessionError::Serialization(e.to_string()))?;

        let now = current_unix_secs();
        if record.expires_at_unix_secs <= now {
            let _ = fs::remove_file(&file_path);
            return Ok(None);
        }
        Ok(Some(record))
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
        with_token_lock(&self.dir, &new_record.token, || {
            let current = self.load_record(&new_record.token)?;
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

    fn delete_record(&self, token: &str) -> Result<(), SsrDraftSessionError> {
        let token = token.trim();
        if !Self::is_safe_token(token) {
            return Ok(());
        }
        // Backend primitive: the caller (the durable store) has already
        // enforced ownership against this record, so unlike the
        // session-store `remove` this deletes unconditionally — under the
        // same token lock, so it cannot interleave with a commit.
        with_token_lock(&self.dir, token, || {
            let file_path = self.dir.join(format!("{token}.json"));
            if file_path.is_file() {
                let _ = fs::remove_file(file_path);
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
        let Some(record) = self.load_record(token)? else {
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
        // Refresh the TTL without regressing a concurrent commit: the
        // compare-and-set only lands while the record is still exactly the
        // one that was read (audit §4.1).
        let mut refreshed = record;
        refreshed.expires_at_unix_secs = current_unix_secs().saturating_add(self.ttl.as_secs());
        let _ = self.compare_and_save_record(Some(&refreshed), &refreshed);
        Ok(Some(snapshot))
    }

    fn commit(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError> {
        self.commit_with_context(
            token,
            expected_generation,
            controller,
            Value::Object(Map::new()),
            owner,
        )
    }

    fn commit_with_context(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
        runtime_context: Value,
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
            runtime_context,
            owner: owner.cloned(),
        };
        self.save_snapshot(&snapshot)?;
        Ok(snapshot)
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
        with_token_lock(&self.dir, token, || {
            if let Some(record) = self.load_record(token)? {
                ensure_session_owner(record.owner.as_ref(), owner)?;
            }
            let file_path = self.dir.join(format!("{token}.json"));
            if file_path.is_file() {
                let _ = fs::remove_file(file_path);
            }
            Ok(())
        })
    }

    fn prune(&self) -> Result<usize, SsrDraftSessionError> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(err) => return Err(SsrDraftSessionError::Io(err.to_string())),
        };
        let mut pruned = 0;
        let now = current_unix_secs();
        let mut surviving: Vec<(PathBuf, u64, Option<DraftSessionOwner>)> = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                if file_name.contains(".tmp.") {
                    let _ = fs::remove_file(&path);
                    pruned += 1;
                    continue;
                }
                if file_name.ends_with(".json") {
                    let record = fs::read_to_string(&path).ok().and_then(|c| {
                        serde_json::from_str::<PersistentDraftSessionRecord>(&c).ok()
                    });
                    if let Some(record) = record {
                        if record.expires_at_unix_secs <= now {
                            let _ = fs::remove_file(&path);
                            pruned += 1;
                        } else {
                            surviving.push((path, record.expires_at_unix_secs, record.owner));
                        }
                    }
                }
            }
        }

        // Global cap first (soonest expiring evicted), then the per-owner
        // cap on the survivors (audit §2.5) — mirroring the KV backend.
        if surviving.len() > self.maximum_entries {
            surviving.sort_by_key(|(_, expires_at, _)| *expires_at);
            let excess = surviving.len() - self.maximum_entries;
            let evicted: Vec<(PathBuf, u64, Option<DraftSessionOwner>)> =
                surviving.drain(..excess).collect();
            for (path, _, _) in evicted {
                let _ = fs::remove_file(&path);
                pruned += 1;
            }
        }

        let mut per_owner: HashMap<String, Vec<(PathBuf, u64)>> = HashMap::new();
        for (path, expires_at, owner) in &surviving {
            if let Some(owner) = owner {
                per_owner
                    .entry(format!("{}:{}", owner.tenant_id, owner.user_id))
                    .or_default()
                    .push((path.clone(), *expires_at));
            }
        }
        for (_, mut owned) in per_owner {
            if owned.len() > MAX_DRAFT_SESSIONS_PER_OWNER {
                owned.sort_by_key(|(_, expires_at)| *expires_at);
                let excess = owned.len() - MAX_DRAFT_SESSIONS_PER_OWNER;
                for (path, _) in owned.into_iter().take(excess) {
                    let _ = fs::remove_file(&path);
                    pruned += 1;
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
    ) -> Result<Option<PersistentDraftSessionRecord>, SsrDraftSessionError> {
        let token = token.trim();
        if token.is_empty() || !FileSsrDraftSessionStore::is_safe_token(token) {
            return Ok(None);
        }
        let key = self.key_for_token(token);
        let Some(bytes) = self.driver.get(&key)? else {
            return Ok(None);
        };
        let record: PersistentDraftSessionRecord = serde_json::from_slice(&bytes)
            .map_err(|e| SsrDraftSessionError::Serialization(e.to_string()))?;
        let now = current_unix_secs();
        if record.expires_at_unix_secs <= now {
            let _ = self.driver.delete(&key);
            return Ok(None);
        }
        Ok(Some(record))
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

    fn delete_record(&self, token: &str) -> Result<(), SsrDraftSessionError> {
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

        for key in keys {
            let record = self.driver.get(&key).ok().flatten().and_then(|bytes| {
                serde_json::from_slice::<PersistentDraftSessionRecord>(&bytes).ok()
            });
            if let Some(record) = record {
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

        for key in &deletions {
            let _ = self.driver.delete(key);
        }
        Ok(deletions.len())
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
        if let Some((bytes, _, _)) = entries
            .get(key)
            .filter(|(_, _, expires)| *expires > now)
        {
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
            if let Some(record) = self.backend.load_record(token)? {
                if record.controller.page_id() != page_id {
                    let _ = self.memory.remove(token, owner);
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
                let _ = self.memory.remove(token, owner);
                return Ok(None);
            }
        }
        // Fallback to persistent backend (cross-process / cross-pod restart)
        if let Some(record) = self.backend.load_record(token)? {
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
        self.commit_with_context(
            token,
            expected_generation,
            controller,
            Value::Object(Map::new()),
            owner,
        )
    }

    fn commit_with_context(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
        runtime_context: Value,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError> {
        let persistent_record = if let Some(tok) = token.map(str::trim).filter(|t| !t.is_empty()) {
            self.backend.load_record(tok)?
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
            let is_stale = self
                .memory
                .load(&persistent_record.token, controller.page_id(), owner)?
                .map(|s| s.generation < persistent_record.generation)
                .unwrap_or(true);
            if is_stale {
                self.memory.insert_snapshot(SsrDraftSessionSnapshot {
                    token: persistent_record.token.clone(),
                    generation: persistent_record.generation,
                    controller: persistent_record.controller.clone(),
                    runtime_context: persistent_record.runtime_context.clone(),
                    owner: persistent_record.owner.clone(),
                })?;
            }
        }
        let committed = self.memory.commit_with_context(
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
        let stored = self
            .backend
            .compare_and_save_record(persistent_record.as_ref(), &record)?;
        if !stored {
            let current = self.backend.load_record(&committed.token)?;
            let current_generation = current
                .as_ref()
                .map(|record| record.generation)
                .unwrap_or_default();
            if let Some(current) = current {
                self.memory.insert_snapshot(SsrDraftSessionSnapshot {
                    token: current.token,
                    generation: current.generation,
                    controller: current.controller,
                    runtime_context: current.runtime_context,
                    owner: current.owner,
                })?;
            }
            return Err(SsrDraftSessionError::GenerationConflict {
                expected: current_generation,
                actual: expected_generation.unwrap_or_default(),
            });
        }
        Ok(committed)
    }

    fn remove(
        &self,
        token: &str,
        owner: Option<&DraftSessionOwner>,
    ) -> Result<(), SsrDraftSessionError> {
        // The backend record is authoritative for ownership: the memory cache
        // may be empty after a restart while the record lives on.
        if let Some(record) = self.backend.load_record(token)? {
            ensure_session_owner(record.owner.as_ref(), owner)?;
        }
        self.memory.remove(token, owner)?;
        self.backend.delete_record(token)?;
        Ok(())
    }

    fn prune(&self) -> Result<usize, SsrDraftSessionError> {
        let mem = self.memory.prune()?;
        let persistent = self
            .backend
            .prune_records(current_unix_secs(), self.memory.maximum_entries())?;
        Ok(mem.max(persistent))
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
        match OpenOptions::new().write(true).create_new(true).open(&lock_path) {
            Ok(mut lock_file) => {
                // Identify the holder inside the lock so release only ever
                // removes a lock we still own. A failed identity write
                // leaves an unverifiable lock: drop it and surface the I/O
                // error instead of running the action unattributed.
                if let Err(err) = lock_file.write_all(holder.as_bytes()) {
                    let _ = fs::remove_file(&lock_path);
                    return Err(SsrDraftSessionError::Io(err.to_string()));
                }
                drop(lock_file);
                let result = action();
                let still_ours = fs::read_to_string(&lock_path)
                    .ok()
                    .is_some_and(|content| content == holder);
                if still_ours {
                    let _ = fs::remove_file(&lock_path);
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
                    let _ = fs::remove_file(&lock_path);
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

/// Outcome of committing an existing session under its token lock: either the
/// commit landed, or the record was absent/expired and the caller's inputs are
/// handed back so a fresh session can be created from them.
enum TokenLockCommit {
    Committed(SsrDraftSessionSnapshot),
    Missing(AdminCanvasController, Value),
}

fn atomic_write_file(target: &Path, content: &[u8]) -> std::io::Result<()> {
    let tmp = target.with_extension(format!("tmp.{}", Uuid::new_v4().simple()));
    fs::write(&tmp, content)?;
    if target.exists() {
        let _ = fs::remove_file(target);
    }
    if let Err(_err) = fs::rename(&tmp, target) {
        let _ = fs::remove_file(target);
        if let Err(retry_err) = fs::rename(&tmp, target) {
            let _ = fs::remove_file(&tmp);
            return Err(retry_err);
        }
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum SsrDraftSessionError {
    #[error("SSR draft session storage lock is poisoned")]
    Poisoned,
    #[error("SSR draft session belongs to page `{expected}`, not `{actual}`")]
    PageMismatch { expected: String, actual: String },
    #[error("SSR draft session generation conflict: expected `{expected}`, received `{actual}`")]
    GenerationConflict { expected: u64, actual: u64 },
    #[error("SSR draft session token `{0}` is invalid")]
    InvalidToken(String),
    #[error("SSR draft session belongs to another owner")]
    OwnerMismatch,
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
        assert_eq!(store.load("token with spaces!", "home", None).unwrap(), None);
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
        restarted_durable.remove(&committed.token, None).expect("remove");
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
        assert!(pod2_store
            .load(&committed.token, "home", None)
            .unwrap()
            .is_none());
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
        assert!(store
            .load(&created.token, "home", Some(&alice))
            .expect("owner load")
            .is_some());
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
        assert!(pod2
            .load(&created.token, "home", Some(&alice))
            .expect("owner load")
            .is_some());
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
        assert!(driver
            .compare_and_set("k", Some(1), b"v2", 2, 3600)
            .expect("cas"));
        assert_eq!(driver.get("k").unwrap().as_deref(), Some(b"v2".as_slice()));

        // A stale CAS token loses without overwriting.
        assert!(!driver
            .compare_and_set("k", Some(1), b"stale", 3, 3600)
            .expect("cas"));
        assert_eq!(driver.get("k").unwrap().as_deref(), Some(b"v2".as_slice()));

        // expected None requires the key to be absent (SETNX semantics).
        assert!(!driver
            .compare_and_set("k", None, b"create", 1, 3600)
            .expect("cas"));
        driver.delete("k").expect("delete");
        assert!(driver
            .compare_and_set("k", None, b"created", 1, 3600)
            .expect("cas"));
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
        assert!(pod2
            .load(&created.token, "home", None)
            .expect("load")
            .is_some());

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
                    self.inner.set(key, &winner_bytes, record.generation, ttl_secs)?;
                }
                self.inner
                    .compare_and_set(key, expected_cas, value, cas, ttl_secs)
            }

            fn delete(&self, key: &str) -> Result<(), SsrDraftSessionError> {
                self.inner.delete(key)
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
        assert!(backend
            .compare_and_save_record(None, &base(1))
            .expect("create cas"));
        // Create against an existing key loses.
        assert!(!backend
            .compare_and_save_record(None, &base(1))
            .expect("create cas loses"));
        // Update with the matching expected generation wins.
        assert!(backend
            .compare_and_save_record(Some(&base(1)), &base(2))
            .expect("update cas"));
        // Update with a stale expected generation loses.
        assert!(!backend
            .compare_and_save_record(Some(&base(1)), &base(3))
            .expect("stale cas loses"));
        // The stored record is the winner's.
        let stored = backend
            .load_record("tokCAS01")
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
        let base = |generation: u64, owner: Option<DraftSessionOwner>| {
            PersistentDraftSessionRecord {
                token: "tokCAS01".to_string(),
                generation,
                controller: controller("home"),
                runtime_context: json!({ "generation": generation }),
                owner,
                expires_at_unix_secs: current_unix_secs() + 3600,
            }
        };

        // Create: expected None against an absent record wins.
        assert!(backend
            .compare_and_save_record(None, &base(1, None))
            .expect("create cas"));
        // Create against an existing record loses.
        assert!(!backend
            .compare_and_save_record(None, &base(1, None))
            .expect("create cas loses"));
        // Update with the matching expected record wins.
        assert!(backend
            .compare_and_save_record(Some(&base(1, None)), &base(2, None))
            .expect("update cas"));
        // A stale expected generation loses.
        assert!(!backend
            .compare_and_save_record(Some(&base(1, None)), &base(3, None))
            .expect("stale cas loses"));
        // A different expected owner loses even with a fresh generation.
        let alice = owner("acme", "user-alice");
        assert!(!backend
            .compare_and_save_record(Some(&base(2, Some(alice.clone()))), &base(3, None))
            .expect("owner cas loses"));
        // The stored record is the winner's.
        let stored = backend
            .load_record("tokCAS01")
            .expect("load")
            .expect("record");
        assert_eq!(stored.generation, 2);
        assert!(stored.owner.is_none());
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
