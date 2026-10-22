use crate::AdminCanvasController;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SsrDraftSessionSnapshot {
    pub token: String,
    pub generation: u64,
    pub controller: AdminCanvasController,
    pub runtime_context: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersistentDraftSessionRecord {
    pub token: String,
    pub generation: u64,
    pub controller: AdminCanvasController,
    pub runtime_context: Value,
    pub expires_at_unix_secs: u64,
}

pub trait SsrDraftSessionStore: Send + Sync {
    fn load(
        &self,
        token: &str,
        page_id: &str,
    ) -> Result<Option<SsrDraftSessionSnapshot>, SsrDraftSessionError>;

    fn commit(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
    ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError>;

    fn commit_with_context(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
        runtime_context: Value,
    ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError> {
        let _ = runtime_context;
        self.commit(token, expected_generation, controller)
    }

    fn remove(&self, token: &str) -> Result<(), SsrDraftSessionError>;

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
        entries.insert(
            snapshot.token,
            DraftEntry {
                controller: snapshot.controller,
                runtime_context: snapshot.runtime_context,
                generation: snapshot.generation,
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

    fn commit_internal(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
        runtime_context: Option<Value>,
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
                entry.generation = entry.generation.saturating_add(1);
                entry.expires_at = now + self.ttl;
                return Ok(SsrDraftSessionSnapshot {
                    token: token.to_string(),
                    generation: entry.generation,
                    controller: entry.controller.clone(),
                    runtime_context: entry.runtime_context.clone(),
                });
            }
        }

        Self::evict_oldest(&mut entries, self.maximum_entries);
        let token = Self::fresh_token(&entries);
        let generation = 1;
        let runtime_context = runtime_context.unwrap_or_else(Self::empty_context);
        entries.insert(
            token.clone(),
            DraftEntry {
                controller: controller.clone(),
                runtime_context: runtime_context.clone(),
                generation,
                expires_at: now + self.ttl,
            },
        );
        Ok(SsrDraftSessionSnapshot {
            token,
            generation,
            controller,
            runtime_context,
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
        entry.expires_at = now + self.ttl;
        Ok(Some(SsrDraftSessionSnapshot {
            token: token.to_string(),
            generation: entry.generation,
            controller: entry.controller.clone(),
            runtime_context: entry.runtime_context.clone(),
        }))
    }

    fn commit(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
    ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError> {
        self.commit_internal(token, expected_generation, controller, None)
    }

    fn commit_with_context(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
        runtime_context: Value,
    ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError> {
        self.commit_internal(
            token,
            expected_generation,
            controller,
            Some(runtime_context),
        )
    }

    fn remove(&self, token: &str) -> Result<(), SsrDraftSessionError> {
        self.entries
            .write()
            .map_err(|_| SsrDraftSessionError::Poisoned)?
            .remove(token.trim());
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
        let target_path = self.dir.join(format!("{}.json", record.token));
        atomic_write_file(&target_path, &json_bytes)
            .map_err(|e| SsrDraftSessionError::Io(e.to_string()))?;
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
            expires_at_unix_secs: current_unix_secs().saturating_add(self.ttl.as_secs()),
        };
        self.save_record(&record)
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

    fn delete_record(&self, token: &str) -> Result<(), SsrDraftSessionError> {
        self.remove(token)
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
        let snapshot = SsrDraftSessionSnapshot {
            token: record.token,
            generation: record.generation,
            controller: record.controller,
            runtime_context: record.runtime_context,
        };
        let _ = self.save_snapshot(&snapshot);
        Ok(Some(snapshot))
    }

    fn commit(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
    ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError> {
        self.commit_with_context(
            token,
            expected_generation,
            controller,
            Value::Object(Map::new()),
        )
    }

    fn commit_with_context(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
        runtime_context: Value,
    ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError> {
        let trimmed_token = token.map(str::trim).filter(|t| !t.is_empty());
        if let Some(token) = trimmed_token {
            if Self::is_safe_token(token) {
                let file_path = self.dir.join(format!("{token}.json"));
                if file_path.is_file() {
                    let content = fs::read_to_string(&file_path)
                        .map_err(|e| SsrDraftSessionError::Io(e.to_string()))?;
                    let mut record: PersistentDraftSessionRecord = serde_json::from_str(&content)
                        .map_err(|e| SsrDraftSessionError::Serialization(e.to_string()))?;
                    if record.controller.page_id() != controller.page_id() {
                        return Err(SsrDraftSessionError::PageMismatch {
                            expected: record.controller.page_id().to_string(),
                            actual: controller.page_id().to_string(),
                        });
                    }
                    if expected_generation.is_some_and(|expected| expected != record.generation) {
                        return Err(SsrDraftSessionError::GenerationConflict {
                            expected: record.generation,
                            actual: expected_generation.unwrap_or_default(),
                        });
                    }
                    record.generation = record.generation.saturating_add(1);
                    record.controller = controller;
                    record.runtime_context = runtime_context;
                    record.expires_at_unix_secs =
                        current_unix_secs().saturating_add(self.ttl.as_secs());
                    let snapshot = SsrDraftSessionSnapshot {
                        token: record.token.clone(),
                        generation: record.generation,
                        controller: record.controller.clone(),
                        runtime_context: record.runtime_context.clone(),
                    };
                    self.save_snapshot(&snapshot)?;
                    return Ok(snapshot);
                }
            }
        }

        let token = Uuid::new_v4().simple().to_string();
        let snapshot = SsrDraftSessionSnapshot {
            token,
            generation: 1,
            controller,
            runtime_context,
        };
        self.save_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    fn remove(&self, token: &str) -> Result<(), SsrDraftSessionError> {
        let token = token.trim();
        if !Self::is_safe_token(token) {
            return Ok(());
        }
        let file_path = self.dir.join(format!("{token}.json"));
        if file_path.is_file() {
            let _ = fs::remove_file(file_path);
        }
        Ok(())
    }

    fn prune(&self) -> Result<usize, SsrDraftSessionError> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(err) => return Err(SsrDraftSessionError::Io(err.to_string())),
        };
        let mut pruned = 0;
        let now = current_unix_secs();
        let mut surviving = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                if file_name.contains(".tmp.") {
                    let _ = fs::remove_file(&path);
                    pruned += 1;
                    continue;
                }
                if file_name.ends_with(".json") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(record) =
                            serde_json::from_str::<PersistentDraftSessionRecord>(&content)
                        {
                            if record.expires_at_unix_secs <= now {
                                let _ = fs::remove_file(&path);
                                pruned += 1;
                            } else {
                                surviving.push((path, record.expires_at_unix_secs));
                            }
                        }
                    }
                }
            }
        }

        if surviving.len() > self.maximum_entries {
            surviving.sort_by_key(|(_, expires_at)| *expires_at);
            let excess = surviving.len() - self.maximum_entries;
            for (path, _) in surviving.into_iter().take(excess) {
                let _ = fs::remove_file(path);
                pruned += 1;
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
    fn set(&self, key: &str, value: &[u8], ttl_secs: u64) -> Result<(), SsrDraftSessionError>;

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
        let now = current_unix_secs();
        let remaining_ttl = record.expires_at_unix_secs.saturating_sub(now).max(1);
        self.driver.set(&key, &json_bytes, remaining_ttl)
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
        let mut pruned = 0;
        let mut surviving = Vec::new();

        for key in keys {
            if let Ok(Some(bytes)) = self.driver.get(&key) {
                if let Ok(record) = serde_json::from_slice::<PersistentDraftSessionRecord>(&bytes) {
                    if record.expires_at_unix_secs <= now_unix_secs {
                        let _ = self.driver.delete(&key);
                        pruned += 1;
                    } else {
                        surviving.push((key, record.expires_at_unix_secs));
                    }
                }
            }
        }

        if surviving.len() > max_entries {
            surviving.sort_by_key(|(_, expires)| *expires);
            let excess = surviving.len() - max_entries;
            for (key, _) in surviving.into_iter().take(excess) {
                let _ = self.driver.delete(&key);
                pruned += 1;
            }
        }

        Ok(pruned)
    }
}

/// In-memory reference implementation of `DraftSessionKvDriver` for testing and simulations.
#[derive(Clone, Default)]
pub struct MemoryKvDriver {
    entries: Arc<RwLock<HashMap<String, (Vec<u8>, u64)>>>,
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
        let entries = self.entries.read().map_err(|_| SsrDraftSessionError::Poisoned)?;
        let now = current_unix_secs();
        if let Some((bytes, expires)) = entries.get(key) {
            if *expires > now {
                return Ok(Some(bytes.clone()));
            }
        }
        Ok(None)
    }

    fn set(&self, key: &str, value: &[u8], ttl_secs: u64) -> Result<(), SsrDraftSessionError> {
        let mut entries = self.entries.write().map_err(|_| SsrDraftSessionError::Poisoned)?;
        let expires = current_unix_secs().saturating_add(ttl_secs);
        entries.insert(key.to_string(), (value.to_vec(), expires));
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<(), SsrDraftSessionError> {
        let mut entries = self.entries.write().map_err(|_| SsrDraftSessionError::Poisoned)?;
        entries.remove(key);
        Ok(())
    }

    fn scan_keys(&self, prefix: &str) -> Result<Vec<String>, SsrDraftSessionError> {
        let entries = self.entries.read().map_err(|_| SsrDraftSessionError::Poisoned)?;
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
    ) -> Result<Option<SsrDraftSessionSnapshot>, SsrDraftSessionError> {
        if let Some(snapshot) = self.memory.load(token, page_id)? {
            return Ok(Some(snapshot));
        }
        // Fallback to persistent backend (cross-process / cross-pod restart)
        if let Some(record) = self.backend.load_record(token)? {
            if record.controller.page_id() != page_id {
                return Err(SsrDraftSessionError::PageMismatch {
                    expected: record.controller.page_id().to_string(),
                    actual: page_id.to_string(),
                });
            }
            let snapshot = SsrDraftSessionSnapshot {
                token: record.token,
                generation: record.generation,
                controller: record.controller,
                runtime_context: record.runtime_context,
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
    ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError> {
        self.commit_with_context(
            token,
            expected_generation,
            controller,
            Value::Object(Map::new()),
        )
    }

    fn commit_with_context(
        &self,
        token: Option<&str>,
        expected_generation: Option<u64>,
        controller: AdminCanvasController,
        runtime_context: Value,
    ) -> Result<SsrDraftSessionSnapshot, SsrDraftSessionError> {
        if let Some(tok) = token.map(str::trim).filter(|t| !t.is_empty()) {
            if self.memory.load(tok, controller.page_id())?.is_none() {
                if let Some(record) = self.backend.load_record(tok)? {
                    if record.controller.page_id() == controller.page_id() {
                        self.memory.insert_snapshot(SsrDraftSessionSnapshot {
                            token: record.token,
                            generation: record.generation,
                            controller: record.controller,
                            runtime_context: record.runtime_context,
                        })?;
                    }
                }
            }
        }
        let committed = self.memory.commit_with_context(
            token,
            expected_generation,
            controller,
            runtime_context,
        )?;
        let record = PersistentDraftSessionRecord {
            token: committed.token.clone(),
            generation: committed.generation,
            controller: committed.controller.clone(),
            runtime_context: committed.runtime_context.clone(),
            expires_at_unix_secs: current_unix_secs().saturating_add(self.memory.ttl().as_secs()),
        };
        self.backend.save_record(&record)?;
        Ok(committed)
    }

    fn remove(&self, token: &str) -> Result<(), SsrDraftSessionError> {
        self.memory.remove(token)?;
        self.backend.delete_record(token)?;
        Ok(())
    }

    fn prune(&self) -> Result<usize, SsrDraftSessionError> {
        let mem = self.memory.prune()?;
        let persistent = self.backend.prune_records(current_unix_secs(), self.memory.maximum_entries())?;
        Ok(mem.max(persistent))
    }
}

fn current_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
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
            )
            .expect("create");
        assert!(first.controller.has_clipboard());

        let loaded = store
            .load(&first.token, "home")
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
            .commit_with_context(None, None, controller("home"), json!({ "value": 42 }))
            .expect("create");
        let second = store
            .commit(Some(&first.token), Some(first.generation), first.controller)
            .expect("update controller");
        assert_eq!(second.runtime_context["value"], 42);
    }

    #[test]
    fn optimistic_generation_rejects_parallel_commit() {
        let store = InMemorySsrDraftSessionStore::default();
        let first = store
            .commit(None, None, controller("home"))
            .expect("create");
        let second = store
            .commit(
                Some(&first.token),
                Some(first.generation),
                first.controller.clone(),
            )
            .expect("update");
        assert_eq!(second.generation, 2);
        assert!(matches!(
            store.commit(Some(&first.token), Some(first.generation), first.controller,),
            Err(SsrDraftSessionError::GenerationConflict { .. })
        ));
    }

    #[test]
    fn session_token_cannot_cross_page_boundary() {
        let store = InMemorySsrDraftSessionStore::default();
        let first = store
            .commit(None, None, controller("home"))
            .expect("create");
        assert!(matches!(
            store.load(&first.token, "about"),
            Err(SsrDraftSessionError::PageMismatch { .. })
        ));
    }

    #[test]
    fn file_store_persists_across_restart_and_validates_page() {
        let temp_dir = std::env::temp_dir().join(format!("rustok_draft_test_{}", Uuid::new_v4().simple()));
        let store = FileSsrDraftSessionStore::new(&temp_dir, Duration::from_secs(3600), 100).expect("store");
        let mut ctrl = controller("home");
        ctrl.dispatch(UiIntent::Select(Some("hero".to_string()))).unwrap();
        ctrl.dispatch(UiIntent::CopySelection).unwrap();

        let committed = store.commit_with_context(
            None,
            None,
            ctrl,
            json!({ "theme": "dark" }),
        ).expect("commit");

        // Simulate server restart by dropping store and re-instantiating against same dir
        drop(store);
        let restarted_store = FileSsrDraftSessionStore::new(&temp_dir, Duration::from_secs(3600), 100).expect("restarted store");
        let loaded = restarted_store.load(&committed.token, "home").expect("load").expect("snapshot exists");

        assert_eq!(loaded.generation, 1);
        assert!(loaded.controller.has_clipboard());
        assert_eq!(loaded.runtime_context["theme"], "dark");

        // Page mismatch on disk
        assert!(matches!(
            restarted_store.load(&committed.token, "other_page"),
            Err(SsrDraftSessionError::PageMismatch { .. })
        ));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn file_store_rejects_unsafe_tokens() {
        let temp_dir = std::env::temp_dir().join(format!("rustok_draft_test_{}", Uuid::new_v4().simple()));
        let store = FileSsrDraftSessionStore::new(&temp_dir, Duration::from_secs(3600), 100).expect("store");
        assert_eq!(store.load("../traversal", "home").unwrap(), None);
        assert_eq!(store.load("token with spaces!", "home").unwrap(), None);
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn file_store_optimistic_generation_conflict() {
        let temp_dir = std::env::temp_dir().join(format!("rustok_draft_test_{}", Uuid::new_v4().simple()));
        let store = FileSsrDraftSessionStore::new(&temp_dir, Duration::from_secs(3600), 100).expect("store");
        let ctrl = controller("home");
        let first = store.commit(None, None, ctrl.clone()).expect("commit 1");
        assert_eq!(first.generation, 1);

        let second = store.commit(Some(&first.token), Some(first.generation), ctrl.clone()).expect("commit 2");
        assert_eq!(second.generation, 2);

        // Conflict: expected generation 1 when generation is now 2
        assert!(matches!(
            store.commit(Some(&first.token), Some(1), ctrl),
            Err(SsrDraftSessionError::GenerationConflict { expected: 2, actual: 1 })
        ));
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn durable_store_serves_from_memory_and_survives_cold_restart() {
        let temp_dir = std::env::temp_dir().join(format!("rustok_durable_test_{}", Uuid::new_v4().simple()));
        let file_store = FileSsrDraftSessionStore::new(&temp_dir, Duration::from_secs(3600), 100).expect("file store");
        let mem_store = InMemorySsrDraftSessionStore::new(Duration::from_secs(3600), 100);
        let durable = DurableSsrDraftSessionStore::new(mem_store, file_store);

        let mut ctrl = controller("landing");
        ctrl.dispatch(UiIntent::Select(Some("hero".to_string()))).unwrap();
        ctrl.dispatch(UiIntent::CopySelection).unwrap();

        let committed = durable.commit_with_context(
            None,
            None,
            ctrl,
            json!({ "user": "admin" }),
        ).expect("commit");

        // Immediately available in memory
        assert!(durable.memory().load(&committed.token, "landing").unwrap().is_some());

        // Simulate server process restart: new empty memory store, re-attached to same disk dir
        let cold_memory = InMemorySsrDraftSessionStore::new(Duration::from_secs(3600), 100);
        let reattached_file = FileSsrDraftSessionStore::new(&temp_dir, Duration::from_secs(3600), 100).expect("file");
        let restarted_durable = DurableSsrDraftSessionStore::new(cold_memory, reattached_file);

        // Cold memory does not have the token initially
        assert!(restarted_durable.memory().load(&committed.token, "landing").unwrap().is_none());

        // Loading via durable store seamlessly retrieves from disk and warms memory
        let loaded = restarted_durable.load(&committed.token, "landing").expect("load").expect("session");
        assert_eq!(loaded.generation, 1);
        assert_eq!(loaded.runtime_context["user"], "admin");
        assert!(loaded.controller.has_clipboard());

        // Now memory is warmed!
        assert!(restarted_durable.memory().load(&committed.token, "landing").unwrap().is_some());

        // Further commit increments generation across both layers
        let updated = restarted_durable.commit(
            Some(&committed.token),
            Some(1),
            loaded.controller,
        ).expect("second commit");
        assert_eq!(updated.generation, 2);

        // Remove cleans up both
        restarted_durable.remove(&committed.token).expect("remove");
        assert!(restarted_durable.load(&committed.token, "landing").unwrap().is_none());

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
        ctrl.dispatch(UiIntent::Select(Some("hero".to_string()))).unwrap();
        ctrl.dispatch(UiIntent::CopySelection).unwrap();

        let committed = pod1_store
            .commit_with_context(None, None, ctrl, json!({ "pod": "pod-1" }))
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
            .load(&committed.token, "home")
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
            )
            .expect("pod 2 update");
        assert_eq!(pod2_updated.generation, 2);

        // Pod 1 attempts commit with stale generation 1 -> must fail with GenerationConflict!
        let stale_ctrl = controller("home");
        let conflict_result = pod1_store.commit(Some(&committed.token), Some(1), stale_ctrl);
        assert!(matches!(
            conflict_result,
            Err(SsrDraftSessionError::GenerationConflict { .. })
        ));

        // Pod 1 can delete the session across the cluster
        pod1_store.remove(&committed.token).expect("cluster remove");
        assert!(pod2_store.load(&committed.token, "home").unwrap().is_none());
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
            .commit(None, None, controller("catalog"))
            .expect("commit");

        // Attempting to load for "checkout" page must yield PageMismatch
        let mismatch = store.load(&first.token, "checkout");
        assert!(matches!(
            mismatch,
            Err(SsrDraftSessionError::PageMismatch { expected, actual }) if expected == "catalog" && actual == "checkout"
        ));

        // Pruning works without errors
        let pruned = store.prune().expect("prune");
        assert_eq!(pruned, 0); // Not expired yet
    }
}
