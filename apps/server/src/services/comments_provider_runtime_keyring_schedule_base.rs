use std::{
    collections::HashSet,
    env,
    fs::File,
    io::Read,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{
        Arc, RwLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rustok_api::{PortActor, PortError};
use rustok_comments::{
    CommentsTcpAuthorityResolver, CommentsTcpBearerToken, CommentsTcpChannelProtection,
    CommentsTcpClientChannelConnector, CommentsTcpDelegationKeyId,
    CommentsTcpDelegationKeyringProvider, CommentsTcpDelegationSchedule,
    CommentsTcpDelegationScheduledKey, CommentsTcpDelegationSecret, CommentsThreadPort,
    CommentsThreadTransport, DEFAULT_COMMENTS_TCP_DELEGATION_CLOCK_SKEW_MS,
    PlaintextLoopbackCommentsTcpChannel, ReloadableCommentsTcpDelegatingAuthorityResolver,
    ReloadableCommentsTcpDelegationSigner, ReloadableTcpJsonCommentsTransport,
    remote_comments_thread_port,
};
use rustok_core::ModuleRuntimeExtensions;
use serde::Deserialize;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::services::server_runtime_context::ServerRuntimeContext;

use super::{base, keyring, keyring_reload, keyring_reload_guard};

pub const COMMENTS_TCP_DELEGATION_SCHEDULE_ENABLED_ENV: &str =
    "RUSTOK_COMMENTS_TCP_DELEGATION_SCHEDULE_ENABLED";

const COMMENTS_TCP_DELEGATION_SCHEDULE_SCHEMA_VERSION: u16 = 2;
const COMMENTS_TCP_SERVICE_ROLE: &str = "admin";
const COMMENTS_TCP_SERVICE_PERMISSION: &str = "comments:manage";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommentsTcpDelegationScheduleRuntimeSelection {
    pub source: keyring::CommentsTcpDelegationKeyringSource,
    pub generation: u64,
    pub scheduled_key_count: usize,
    pub verification_key_count: usize,
    pub propagation_budget_ms: u64,
    pub max_ttl_ms: u64,
    pub clock_skew_ms: u64,
    pub legacy_unkeyed_enabled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommentsTcpDelegationScheduleReloadStatus {
    pub selection: CommentsTcpDelegationScheduleRuntimeSelection,
    pub successful_reloads: u64,
    pub rejected_reloads: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommentsTcpDelegationScheduleReloadOutcome {
    pub previous_generation: u64,
    pub current: CommentsTcpDelegationScheduleRuntimeSelection,
}

#[derive(Clone)]
pub struct SharedCommentsTcpDelegationScheduleHandle(Arc<DelegationScheduleState>);

struct DelegationScheduleState {
    source: DelegationScheduleSource,
    current: RwLock<DelegationScheduleSnapshot>,
    successful_reloads: AtomicU64,
    rejected_reloads: AtomicU64,
}

enum DelegationScheduleSource {
    HostProvided,
    File(PathBuf),
}

#[derive(Clone)]
struct DelegationScheduleSnapshot {
    schedule: CommentsTcpDelegationSchedule,
    source: keyring::CommentsTcpDelegationKeyringSource,
    generation: u64,
}

impl SharedCommentsTcpDelegationScheduleHandle {
    pub fn from_host_schedule(
        schedule: CommentsTcpDelegationSchedule,
        generation: u64,
    ) -> std::result::Result<Self, String> {
        let snapshot = build_schedule_snapshot(
            schedule,
            generation,
            keyring::CommentsTcpDelegationKeyringSource::HostProvided,
        )?;
        Ok(Self::new(DelegationScheduleSource::HostProvided, snapshot))
    }

    pub fn from_file(
        file_path: impl AsRef<Path>,
        max_ttl: Duration,
    ) -> std::result::Result<Self, String> {
        let file_path = file_path.as_ref().to_path_buf();
        let max_ttl_ms = duration_ms(max_ttl)
            .ok_or_else(|| "Comments TCP delegation schedule TTL is invalid".to_string())?;
        let snapshot = load_schedule_snapshot_from_file(&file_path, max_ttl_ms)?;
        Ok(Self::new(
            DelegationScheduleSource::File(file_path),
            snapshot,
        ))
    }

    pub fn current_status(
        &self,
    ) -> std::result::Result<CommentsTcpDelegationScheduleReloadStatus, String> {
        let current = self
            .0
            .current
            .read()
            .map_err(|_| "Comments TCP delegation schedule state is unavailable".to_string())?;
        Ok(CommentsTcpDelegationScheduleReloadStatus {
            selection: schedule_selection(&current)?,
            successful_reloads: self.0.successful_reloads.load(Ordering::Relaxed),
            rejected_reloads: self.0.rejected_reloads.load(Ordering::Relaxed),
        })
    }

    pub fn current_selection(
        &self,
    ) -> std::result::Result<CommentsTcpDelegationScheduleRuntimeSelection, String> {
        self.current_status().map(|status| status.selection)
    }

    pub fn replace_host_schedule(
        &self,
        schedule: CommentsTcpDelegationSchedule,
        generation: u64,
    ) -> std::result::Result<CommentsTcpDelegationScheduleReloadOutcome, String> {
        if !matches!(&self.0.source, DelegationScheduleSource::HostProvided) {
            return self.reject(
                "Comments TCP file-backed delegation schedule must be reloaded from its configured file"
                    .to_string(),
            );
        }
        let candidate = match build_schedule_snapshot(
            schedule,
            generation,
            keyring::CommentsTcpDelegationKeyringSource::HostProvided,
        ) {
            Ok(candidate) => candidate,
            Err(error) => return self.reject(error),
        };
        self.replace_candidate(candidate)
    }

    pub fn reload_file(
        &self,
    ) -> std::result::Result<CommentsTcpDelegationScheduleReloadOutcome, String> {
        let file_path = match &self.0.source {
            DelegationScheduleSource::File(file_path) => file_path.clone(),
            DelegationScheduleSource::HostProvided => {
                return self.reject(
                    "Comments TCP host-provided delegation schedule must be replaced programmatically"
                        .to_string(),
                );
            }
        };
        let max_ttl_ms = match self.0.current.read() {
            Ok(current) => current.schedule.max_ttl_ms(),
            Err(_) => {
                return self
                    .reject("Comments TCP delegation schedule state is unavailable".to_string());
            }
        };
        let candidate = match load_schedule_snapshot_from_file(&file_path, max_ttl_ms) {
            Ok(candidate) => candidate,
            Err(error) => return self.reject(error),
        };
        self.replace_candidate(candidate)
    }

    fn new(source: DelegationScheduleSource, current: DelegationScheduleSnapshot) -> Self {
        Self(Arc::new(DelegationScheduleState {
            source,
            current: RwLock::new(current),
            successful_reloads: AtomicU64::new(0),
            rejected_reloads: AtomicU64::new(0),
        }))
    }

    fn replace_candidate(
        &self,
        candidate: DelegationScheduleSnapshot,
    ) -> std::result::Result<CommentsTcpDelegationScheduleReloadOutcome, String> {
        let now_ms = current_unix_ms()?;
        let mut current = match self.0.current.write() {
            Ok(current) => current,
            Err(_) => {
                return self
                    .reject("Comments TCP delegation schedule state is unavailable".to_string());
            }
        };
        if candidate.source != current.source {
            drop(current);
            return self.reject(
                "Comments TCP delegation schedule reload cannot change source category".to_string(),
            );
        }
        if candidate.generation <= current.generation {
            drop(current);
            return self.reject(
                "Comments TCP delegation schedule generation must be greater than the active generation"
                    .to_string(),
            );
        }
        if let Err(error) = candidate
            .schedule
            .validate_replacement_from(&current.schedule, now_ms)
        {
            drop(current);
            return self.reject(format!(
                "Comments TCP delegation schedule replacement is unsafe: {error}"
            ));
        }
        candidate.schedule.current_keyring_at(now_ms).map_err(|_| {
            "Comments TCP delegation schedule replacement has no active signing key".to_string()
        })?;

        let previous_generation = current.generation;
        let current_selection = schedule_selection_at(&candidate, now_ms)?;
        *current = candidate;
        self.0.successful_reloads.fetch_add(1, Ordering::Relaxed);
        Ok(CommentsTcpDelegationScheduleReloadOutcome {
            previous_generation,
            current: current_selection,
        })
    }

    fn reject<T>(&self, message: String) -> std::result::Result<T, String> {
        self.0.rejected_reloads.fetch_add(1, Ordering::Relaxed);
        Err(message)
    }
}

impl CommentsTcpDelegationKeyringProvider for SharedCommentsTcpDelegationScheduleHandle {
    fn current_keyring(
        &self,
    ) -> std::result::Result<rustok_comments::CommentsTcpDelegationKeyring, PortError> {
        let schedule = self
            .0
            .current
            .read()
            .map(|current| current.schedule.clone())
            .map_err(|_| {
                PortError::unavailable(
                    "comments.tcp_delegation_schedule_unavailable",
                    "Comments TCP delegation schedule is temporarily unavailable",
                )
            })?;
        schedule.current_keyring()
    }
}

impl std::fmt::Debug for SharedCommentsTcpDelegationScheduleHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.current_status() {
            Ok(status) => formatter
                .debug_struct("SharedCommentsTcpDelegationScheduleHandle")
                .field("source", &status.selection.source)
                .field("generation", &status.selection.generation)
                .field("scheduled_key_count", &status.selection.scheduled_key_count)
                .field(
                    "verification_key_count",
                    &status.selection.verification_key_count,
                )
                .field(
                    "propagation_budget_ms",
                    &status.selection.propagation_budget_ms,
                )
                .field("max_ttl_ms", &status.selection.max_ttl_ms)
                .field("clock_skew_ms", &status.selection.clock_skew_ms)
                .field(
                    "legacy_unkeyed_enabled",
                    &status.selection.legacy_unkeyed_enabled,
                )
                .field("successful_reloads", &status.successful_reloads)
                .field("rejected_reloads", &status.rejected_reloads)
                .field("file_path", &"[REDACTED]")
                .field("key_ids", &"[REDACTED]")
                .field("secrets", &"[REDACTED]")
                .finish(),
            Err(_) => formatter
                .debug_struct("SharedCommentsTcpDelegationScheduleHandle")
                .field("state", &"[UNAVAILABLE]")
                .field("file_path", &"[REDACTED]")
                .field("key_ids", &"[REDACTED]")
                .field("secrets", &"[REDACTED]")
                .finish(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DelegationScheduleFileDocument {
    schema_version: u16,
    generation: u64,
    propagation_budget_ms: u64,
    #[serde(default)]
    legacy_unkeyed_key_id: Option<String>,
    keys: Vec<DelegationScheduleFileEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DelegationScheduleFileEntry {
    key_id: String,
    secret: String,
    activates_at_unix_ms: u64,
    #[serde(default)]
    retires_at_unix_ms: Option<u64>,
}

pub(super) fn register_comments_provider_runtime(
    extensions: &mut ModuleRuntimeExtensions,
) -> std::result::Result<(), String> {
    let schedule_enabled =
        match read_optional_environment(COMMENTS_TCP_DELEGATION_SCHEDULE_ENABLED_ENV)? {
            Some(value) => parse_bool_value(COMMENTS_TCP_DELEGATION_SCHEDULE_ENABLED_ENV, &value)?,
            None => false,
        };
    let host_handle = extensions
        .get::<SharedCommentsTcpDelegationScheduleHandle>()
        .cloned();
    if host_handle.is_none() && !schedule_enabled {
        return keyring_reload::register_comments_provider_runtime(extensions);
    }

    if extensions.contains::<keyring::SharedCommentsTcpDelegationKeyringSnapshot>()
        || extensions.contains::<keyring_reload::SharedCommentsTcpDelegationKeyringReloadHandle>()
    {
        return Err(
            "Scheduled, static, and ordinary reloadable Comments TCP delegation keyrings cannot be combined"
                .to_string(),
        );
    }
    if read_optional_environment(keyring_reload::COMMENTS_TCP_DELEGATION_RELOAD_ENABLED_ENV)?
        .is_some_and(|value| {
            parse_bool_value(
                keyring_reload::COMMENTS_TCP_DELEGATION_RELOAD_ENABLED_ENV,
                &value,
            )
            .unwrap_or(false)
        })
    {
        return Err(format!(
            "{COMMENTS_TCP_DELEGATION_SCHEDULE_ENABLED_ENV} cannot be combined with {}",
            keyring_reload::COMMENTS_TCP_DELEGATION_RELOAD_ENABLED_ENV
        ));
    }

    let file_path = read_optional_environment(keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV)?;
    let legacy_secret = read_optional_environment(base::COMMENTS_TCP_DELEGATION_SECRET_ENV)?;
    let schedule_handle = match host_handle {
        Some(handle) => {
            if file_path.is_some() || legacy_secret.is_some() {
                return Err(
                    "Host-provided Comments TCP delegation schedule cannot be combined with file or legacy-secret environment configuration"
                        .to_string(),
                );
            }
            handle
        }
        None => {
            if legacy_secret.is_some() {
                return Err(format!(
                    "{COMMENTS_TCP_DELEGATION_SCHEDULE_ENABLED_ENV} cannot be combined with {}",
                    base::COMMENTS_TCP_DELEGATION_SECRET_ENV
                ));
            }
            let file_path = file_path.ok_or_else(|| {
                format!(
                    "{} is required when {COMMENTS_TCP_DELEGATION_SCHEDULE_ENABLED_ENV}=true",
                    keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
                )
            })?;
            let ttl_ms = comments_tcp_delegation_ttl_ms_from_environment()?;
            SharedCommentsTcpDelegationScheduleHandle::from_file(
                file_path,
                Duration::from_millis(ttl_ms),
            )?
        }
    };

    let listener_configured = base::CommentsTcpListenerConfig::from_environment()?.is_some();
    let preconfigured = extensions.contains::<Arc<dyn CommentsThreadPort>>();
    let mode = if preconfigured {
        None
    } else {
        Some(comments_provider_mode())
    };
    let client_uses_schedule = mode.as_deref() == Some("tcp");
    if !client_uses_schedule && !listener_configured {
        return Err(
            "Comments TCP delegation schedule requires a built-in TCP client or an enabled TCP listener"
                .to_string(),
        );
    }

    let (prepared_port, provider_selection) = if preconfigured {
        (
            None,
            base::CommentsProviderRuntimeSelection {
                profile: base::CommentsProviderProfile::Preconfigured,
                endpoint: None,
            },
        )
    } else {
        match mode.as_deref() {
            Some("in_process") => (
                None,
                base::CommentsProviderRuntimeSelection {
                    profile: base::CommentsProviderProfile::InProcessFallback,
                    endpoint: None,
                },
            ),
            Some("tcp") => {
                let (port, selection) = prepare_scheduled_tcp_client(extensions, &schedule_handle)?;
                (Some(port), selection)
            }
            _ => {
                return Err(format!(
                    "{} must be one of: in_process, tcp",
                    base::COMMENTS_PROVIDER_MODE_ENV
                ));
            }
        }
    };

    let schedule_selection = schedule_handle.current_selection()?;
    if let Some(port) = prepared_port {
        extensions.insert::<Arc<dyn CommentsThreadPort>>(port);
    }
    extensions.insert(provider_selection);
    extensions.insert(schedule_selection);
    extensions.insert(schedule_handle);
    Ok(())
}

pub(super) async fn start_comments_tcp_listener_if_enabled(
    runtime_ctx: &ServerRuntimeContext,
) -> Result<()> {
    let Some(schedule_handle) = schedule_handle_from_context(runtime_ctx) else {
        return keyring_reload_guard::start_comments_tcp_listener_if_enabled(runtime_ctx).await;
    };
    let Some(_) = base::CommentsTcpListenerConfig::from_environment().map_err(Error::BadRequest)?
    else {
        return Ok(());
    };

    let extensions = runtime_ctx.shared_get::<Arc<ModuleRuntimeExtensions>>();
    let authority_already_configured = runtime_ctx
        .shared_get::<base::SharedCommentsTcpAuthorityResolver>()
        .is_some()
        || extensions.as_ref().is_some_and(|values| {
            values
                .get::<base::SharedCommentsTcpAuthorityResolver>()
                .is_some()
        });
    if authority_already_configured && !scheduled_client_is_active(extensions.as_deref()) {
        return Err(Error::BadRequest(
            "Comments TCP delegation schedule is unused because an external listener authority override is configured and no built-in scheduled TCP client is active"
                .to_string(),
        ));
    }
    if !authority_already_configured {
        let authority = comments_tcp_authority_from_schedule_handle(&schedule_handle)
            .map_err(Error::BadRequest)?;
        runtime_ctx.shared_insert(base::SharedCommentsTcpAuthorityResolver(authority));
    }

    base::start_comments_tcp_listener_if_enabled(runtime_ctx).await
}

fn build_schedule_snapshot(
    schedule: CommentsTcpDelegationSchedule,
    generation: u64,
    source: keyring::CommentsTcpDelegationKeyringSource,
) -> std::result::Result<DelegationScheduleSnapshot, String> {
    if generation == 0 {
        return Err(
            "Comments TCP delegation schedule generation must be greater than zero".to_string(),
        );
    }
    schedule.current_keyring().map_err(|_| {
        "Comments TCP delegation schedule must have one active signing key at composition time"
            .to_string()
    })?;
    Ok(DelegationScheduleSnapshot {
        schedule,
        source,
        generation,
    })
}

fn schedule_selection(
    snapshot: &DelegationScheduleSnapshot,
) -> std::result::Result<CommentsTcpDelegationScheduleRuntimeSelection, String> {
    schedule_selection_at(snapshot, current_unix_ms()?)
}

fn schedule_selection_at(
    snapshot: &DelegationScheduleSnapshot,
    now_ms: u64,
) -> std::result::Result<CommentsTcpDelegationScheduleRuntimeSelection, String> {
    let keyring = snapshot.schedule.current_keyring_at(now_ms).map_err(|_| {
        "Comments TCP delegation schedule has no safe keyring for the current time".to_string()
    })?;
    Ok(CommentsTcpDelegationScheduleRuntimeSelection {
        source: snapshot.source,
        generation: snapshot.generation,
        scheduled_key_count: snapshot.schedule.scheduled_key_count(),
        verification_key_count: keyring.key_count(),
        propagation_budget_ms: snapshot.schedule.propagation_budget_ms(),
        max_ttl_ms: snapshot.schedule.max_ttl_ms(),
        clock_skew_ms: snapshot.schedule.clock_skew_ms(),
        legacy_unkeyed_enabled: keyring.accepts_legacy_unkeyed_tokens(),
    })
}

fn load_schedule_snapshot_from_file(
    file_path: &Path,
    max_ttl_ms: u64,
) -> std::result::Result<DelegationScheduleSnapshot, String> {
    let bytes = read_bounded_schedule_file(file_path)?;
    parse_schedule_document(&bytes, max_ttl_ms)
}

fn parse_schedule_document(
    bytes: &[u8],
    max_ttl_ms: u64,
) -> std::result::Result<DelegationScheduleSnapshot, String> {
    let document =
        serde_json::from_slice::<DelegationScheduleFileDocument>(bytes).map_err(|_| {
            format!(
                "{} must contain one valid version-2 Comments TCP delegation schedule JSON object",
                keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
            )
        })?;
    if document.schema_version != COMMENTS_TCP_DELEGATION_SCHEDULE_SCHEMA_VERSION {
        return Err(format!(
            "{} schema_version must equal {COMMENTS_TCP_DELEGATION_SCHEDULE_SCHEMA_VERSION} in schedule mode",
            keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
        ));
    }
    if document.generation == 0 {
        return Err(format!(
            "{} generation must be greater than zero",
            keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
        ));
    }

    let mut retained_ids = HashSet::with_capacity(document.keys.len());
    let mut scheduled_keys = Vec::with_capacity(document.keys.len());
    for entry in document.keys {
        let key_id = CommentsTcpDelegationKeyId::new(&entry.key_id).map_err(|error| {
            format!(
                "{} contains an invalid scheduled key ID: {error}",
                keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
            )
        })?;
        if !retained_ids.insert(entry.key_id) {
            return Err(format!(
                "{} scheduled key IDs must be unique",
                keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
            ));
        }
        let secret = CommentsTcpDelegationSecret::new(entry.secret).map_err(|error| {
            format!(
                "{} contains an invalid delegation secret: {error}",
                keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
            )
        })?;
        let scheduled_key = CommentsTcpDelegationScheduledKey::new(
            key_id,
            secret,
            entry.activates_at_unix_ms,
            entry.retires_at_unix_ms,
        )
        .map_err(|error| {
            format!(
                "{} contains an invalid key lifecycle: {error}",
                keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
            )
        })?;
        scheduled_keys.push(scheduled_key);
    }

    let mut schedule = CommentsTcpDelegationSchedule::new(
        scheduled_keys,
        Duration::from_millis(document.propagation_budget_ms),
        Duration::from_millis(max_ttl_ms),
        Duration::from_millis(DEFAULT_COMMENTS_TCP_DELEGATION_CLOCK_SKEW_MS),
    )
    .map_err(|error| {
        format!(
            "{} schedule is invalid: {error}",
            keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
        )
    })?;
    if let Some(raw_legacy_key_id) = document.legacy_unkeyed_key_id {
        let legacy_key_id =
            CommentsTcpDelegationKeyId::new(raw_legacy_key_id).map_err(|error| {
                format!(
                    "{} legacy_unkeyed_key_id is invalid: {error}",
                    keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
                )
            })?;
        schedule = schedule
            .with_legacy_unkeyed_key_id(legacy_key_id)
            .map_err(|error| {
                format!(
                    "{} legacy key selection is invalid: {error}",
                    keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
                )
            })?;
    }

    build_schedule_snapshot(
        schedule,
        document.generation,
        keyring::CommentsTcpDelegationKeyringSource::File,
    )
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

fn prepare_scheduled_tcp_client(
    extensions: &ModuleRuntimeExtensions,
    schedule_handle: &SharedCommentsTcpDelegationScheduleHandle,
) -> std::result::Result<
    (
        Arc<dyn CommentsThreadPort>,
        base::CommentsProviderRuntimeSelection,
    ),
    String,
> {
    let raw_endpoint = read_required_environment(
        base::COMMENTS_TCP_ENDPOINT_ENV,
        format!(
            "{} is required when {}=tcp",
            base::COMMENTS_TCP_ENDPOINT_ENV,
            base::COMMENTS_PROVIDER_MODE_ENV
        ),
    )?;
    let endpoint = raw_endpoint.trim().parse::<SocketAddr>().map_err(|_| {
        format!(
            "{} must be an explicit IP socket address",
            base::COMMENTS_TCP_ENDPOINT_ENV
        )
    })?;
    let channel_connector = extensions
        .get::<base::SharedCommentsTcpClientChannelConnector>()
        .cloned()
        .map(|shared| shared.0)
        .unwrap_or_else(plaintext_client_channel_connector);
    let channel_protection = channel_connector.protection();
    require_loopback_endpoint(endpoint, channel_protection)?;

    let bearer_token = comments_tcp_bearer_token_from_environment()?;
    let ttl_ms = comments_tcp_delegation_ttl_ms_from_environment()?;
    let signer = ReloadableCommentsTcpDelegationSigner::with_ttl(
        Arc::new(schedule_handle.clone()),
        Duration::from_millis(ttl_ms),
    )
    .map_err(|error| format!("Comments TCP delegation signer configuration failed: {error}"))?;
    let transport =
        ReloadableTcpJsonCommentsTransport::with_channel_connector_bearer_and_delegation(
            endpoint,
            channel_connector,
            bearer_token,
            signer,
        );
    let transport: Arc<dyn CommentsThreadTransport> = Arc::new(transport);
    let port = remote_comments_thread_port(transport);
    let selection = base::CommentsProviderRuntimeSelection {
        profile: match channel_protection {
            CommentsTcpChannelProtection::PlaintextLoopback => {
                base::CommentsProviderProfile::TcpLoopback
            }
            CommentsTcpChannelProtection::AuthenticatedEncrypted => {
                base::CommentsProviderProfile::TcpProtectedLoopback
            }
        },
        endpoint: Some(endpoint),
    };
    Ok((port, selection))
}

fn comments_tcp_authority_from_schedule_handle(
    schedule_handle: &SharedCommentsTcpDelegationScheduleHandle,
) -> std::result::Result<Arc<dyn CommentsTcpAuthorityResolver>, String> {
    let token = comments_tcp_bearer_token_from_environment()?;
    let actor = comments_tcp_service_actor_from_environment()?;
    let ttl_ms = comments_tcp_delegation_ttl_ms_from_environment()?;
    let replay_capacity = parse_optional_positive_usize(
        base::COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY_ENV,
        rustok_comments::DEFAULT_COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY,
    )?;
    let resolver = ReloadableCommentsTcpDelegatingAuthorityResolver::new(
        token,
        actor,
        Arc::new(schedule_handle.clone()),
    )
    .with_service_claim(COMMENTS_TCP_SERVICE_PERMISSION)
    .with_service_role(COMMENTS_TCP_SERVICE_ROLE)
    .with_max_ttl(Duration::from_millis(ttl_ms))
    .map_err(|error| format!("Comments TCP delegation TTL configuration failed: {error}"))?
    .with_replay_capacity(replay_capacity)
    .map_err(|error| format!("Comments TCP replay configuration failed: {error}"))?;
    Ok(Arc::new(resolver))
}

fn schedule_handle_from_context(
    runtime_ctx: &ServerRuntimeContext,
) -> Option<SharedCommentsTcpDelegationScheduleHandle> {
    runtime_ctx
        .shared_get::<SharedCommentsTcpDelegationScheduleHandle>()
        .or_else(|| {
            runtime_ctx
                .shared_get::<Arc<ModuleRuntimeExtensions>>()
                .and_then(|extensions| {
                    extensions
                        .get::<SharedCommentsTcpDelegationScheduleHandle>()
                        .cloned()
                })
        })
}

fn scheduled_client_is_active(extensions: Option<&ModuleRuntimeExtensions>) -> bool {
    extensions
        .and_then(|values| values.get::<base::CommentsProviderRuntimeSelection>())
        .is_some_and(|selection| {
            matches!(
                selection.profile,
                base::CommentsProviderProfile::TcpLoopback
                    | base::CommentsProviderProfile::TcpProtectedLoopback
            )
        })
}

fn comments_provider_mode() -> String {
    env::var(base::COMMENTS_PROVIDER_MODE_ENV)
        .unwrap_or_else(|_| "in_process".to_string())
        .trim()
        .to_ascii_lowercase()
}

fn plaintext_client_channel_connector() -> Arc<dyn CommentsTcpClientChannelConnector> {
    Arc::new(PlaintextLoopbackCommentsTcpChannel)
}

fn require_loopback_endpoint(
    endpoint: SocketAddr,
    protection: CommentsTcpChannelProtection,
) -> std::result::Result<(), String> {
    if endpoint.ip().is_loopback() {
        return Ok(());
    }
    match protection {
        CommentsTcpChannelProtection::PlaintextLoopback => Err(format!(
            "{} must be loopback while the Comments TCP connector is plaintext",
            base::COMMENTS_TCP_ENDPOINT_ENV
        )),
        CommentsTcpChannelProtection::AuthenticatedEncrypted => Err(format!(
            "{} must remain loopback until protected Comments TCP runtime evidence is retained",
            base::COMMENTS_TCP_ENDPOINT_ENV
        )),
    }
}

fn comments_tcp_bearer_token_from_environment()
-> std::result::Result<CommentsTcpBearerToken, String> {
    let secret = read_required_environment(
        base::COMMENTS_TCP_BEARER_TOKEN_ENV,
        format!(
            "{} is required for Comments TCP bearer authentication",
            base::COMMENTS_TCP_BEARER_TOKEN_ENV
        ),
    )?;
    CommentsTcpBearerToken::new(secret).map_err(|error| {
        format!(
            "{} is not a valid Comments TCP bearer token: {error}",
            base::COMMENTS_TCP_BEARER_TOKEN_ENV
        )
    })
}

fn comments_tcp_delegation_ttl_ms_from_environment() -> std::result::Result<u64, String> {
    let ttl_ms = parse_optional_positive_u64(
        base::COMMENTS_TCP_DELEGATION_TTL_MS_ENV,
        rustok_comments::DEFAULT_COMMENTS_TCP_DELEGATION_TTL_MS,
    )?;
    if ttl_ms > rustok_comments::MAX_COMMENTS_TCP_DELEGATION_TTL_MS {
        return Err(format!(
            "{} must be within 1..={}",
            base::COMMENTS_TCP_DELEGATION_TTL_MS_ENV,
            rustok_comments::MAX_COMMENTS_TCP_DELEGATION_TTL_MS
        ));
    }
    Ok(ttl_ms)
}

fn comments_tcp_service_actor_from_environment() -> std::result::Result<PortActor, String> {
    let raw_actor_id = read_required_environment(
        base::COMMENTS_TCP_SERVICE_ACTOR_ID_ENV,
        format!(
            "{} is required when the built-in Comments TCP authority resolver is used",
            base::COMMENTS_TCP_SERVICE_ACTOR_ID_ENV
        ),
    )?;
    let actor_id = raw_actor_id.trim();
    let parsed = Uuid::parse_str(actor_id).map_err(|_| {
        format!(
            "{} must be a canonical UUID without surrounding whitespace",
            base::COMMENTS_TCP_SERVICE_ACTOR_ID_ENV
        )
    })?;
    if actor_id != raw_actor_id || parsed.to_string() != actor_id {
        return Err(format!(
            "{} must be a canonical UUID without surrounding whitespace",
            base::COMMENTS_TCP_SERVICE_ACTOR_ID_ENV
        ));
    }
    Ok(PortActor::service(actor_id.to_string()))
}

fn read_required_environment(
    key: &'static str,
    missing_message: String,
) -> std::result::Result<String, String> {
    match env::var(key) {
        Ok(value) => Ok(value),
        Err(env::VarError::NotPresent) => Err(missing_message),
        Err(env::VarError::NotUnicode(_)) => Err(format!("{key} must contain valid UTF-8")),
    }
}

fn read_optional_environment(key: &'static str) -> std::result::Result<Option<String>, String> {
    match env::var(key) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(format!("{key} must contain valid UTF-8")),
    }
}

fn parse_bool_value(key: &'static str, value: &str) -> std::result::Result<bool, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(format!(
            "{key} must be one of: true, false, 1, 0, yes, no, on, off"
        )),
    }
}

fn parse_optional_positive_usize(
    key: &'static str,
    default: usize,
) -> std::result::Result<usize, String> {
    match read_optional_environment(key)? {
        Some(value) => parse_positive_usize_value(key, &value),
        None => Ok(default),
    }
}

fn parse_optional_positive_u64(
    key: &'static str,
    default: u64,
) -> std::result::Result<u64, String> {
    match read_optional_environment(key)? {
        Some(value) => parse_positive_u64_value(key, &value),
        None => Ok(default),
    }
}

fn parse_positive_usize_value(
    key: &'static str,
    value: &str,
) -> std::result::Result<usize, String> {
    let parsed = value
        .trim()
        .parse::<usize>()
        .map_err(|_| format!("{key} must be a positive integer"))?;
    if parsed == 0 {
        return Err(format!("{key} must be greater than zero"));
    }
    Ok(parsed)
}

fn parse_positive_u64_value(key: &'static str, value: &str) -> std::result::Result<u64, String> {
    let parsed = value
        .trim()
        .parse::<u64>()
        .map_err(|_| format!("{key} must be a positive integer"))?;
    if parsed == 0 {
        return Err(format!("{key} must be greater than zero"));
    }
    Ok(parsed)
}

fn duration_ms(duration: Duration) -> Option<u64> {
    u64::try_from(duration.as_millis()).ok()
}

fn current_unix_ms() -> std::result::Result<u64, String> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "Comments TCP delegation schedule clock is not available".to_string())?;
    u64::try_from(elapsed.as_millis())
        .map_err(|_| "Comments TCP delegation schedule clock is not available".to_string())
}
