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
    time::Duration,
};

use rustok_api::{PortActor, PortError};
use rustok_comments::{
    CommentsTcpAuthorityResolver, CommentsTcpBearerToken, CommentsTcpChannelProtection,
    CommentsTcpClientChannelConnector, CommentsTcpDelegationKeyId, CommentsTcpDelegationKeyring,
    CommentsTcpDelegationKeyringProvider, CommentsTcpDelegationSecret, CommentsThreadPort,
    CommentsThreadTransport, MAX_COMMENTS_TCP_DELEGATION_KEYS, PlaintextLoopbackCommentsTcpChannel,
    ReloadableCommentsTcpDelegatingAuthorityResolver, ReloadableCommentsTcpDelegationSigner,
    ReloadableTcpJsonCommentsTransport, remote_comments_thread_port,
};
use rustok_core::ModuleRuntimeExtensions;
use serde::Deserialize;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::services::server_runtime_context::ServerRuntimeContext;

use super::{base, keyring};

pub const COMMENTS_TCP_DELEGATION_RELOAD_ENABLED_ENV: &str =
    "RUSTOK_COMMENTS_TCP_DELEGATION_RELOAD_ENABLED";

const COMMENTS_TCP_DELEGATION_KEYRING_SCHEMA_VERSION: u16 = 1;
const COMMENTS_TCP_SERVICE_ROLE: &str = "admin";
const COMMENTS_TCP_SERVICE_PERMISSION: &str = "comments:manage";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommentsTcpDelegationKeyringReloadStatus {
    pub selection: keyring::CommentsTcpDelegationKeyringRuntimeSelection,
    pub successful_reloads: u64,
    pub rejected_reloads: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommentsTcpDelegationKeyringReloadOutcome {
    pub previous_generation: u64,
    pub current: keyring::CommentsTcpDelegationKeyringRuntimeSelection,
}

#[derive(Clone)]
pub struct SharedCommentsTcpDelegationKeyringReloadHandle(Arc<DelegationReloadState>);

struct DelegationReloadState {
    source: DelegationReloadSource,
    current: RwLock<DelegationReloadSnapshot>,
    successful_reloads: AtomicU64,
    rejected_reloads: AtomicU64,
}

enum DelegationReloadSource {
    HostProvided,
    File(PathBuf),
}

#[derive(Clone)]
struct DelegationReloadSnapshot {
    keyring: CommentsTcpDelegationKeyring,
    selection: keyring::CommentsTcpDelegationKeyringRuntimeSelection,
}

impl SharedCommentsTcpDelegationKeyringReloadHandle {
    pub fn from_host_keyring(
        delegation_keyring: CommentsTcpDelegationKeyring,
        generation: u64,
        revoked_key_count: usize,
    ) -> std::result::Result<Self, String> {
        let snapshot = build_reload_snapshot(
            delegation_keyring,
            generation,
            revoked_key_count,
            keyring::CommentsTcpDelegationKeyringSource::HostProvided,
        )?;
        Ok(Self::new(DelegationReloadSource::HostProvided, snapshot))
    }

    pub fn from_file(file_path: impl AsRef<Path>) -> std::result::Result<Self, String> {
        let file_path = file_path.as_ref().to_path_buf();
        let snapshot = load_reload_snapshot_from_file(&file_path)?;
        Ok(Self::new(DelegationReloadSource::File(file_path), snapshot))
    }

    pub fn current_status(
        &self,
    ) -> std::result::Result<CommentsTcpDelegationKeyringReloadStatus, String> {
        let current = self.0.current.read().map_err(|_| {
            "Comments TCP delegation keyring reload state is unavailable".to_string()
        })?;
        Ok(CommentsTcpDelegationKeyringReloadStatus {
            selection: current.selection,
            successful_reloads: self.0.successful_reloads.load(Ordering::Relaxed),
            rejected_reloads: self.0.rejected_reloads.load(Ordering::Relaxed),
        })
    }

    pub fn current_selection(
        &self,
    ) -> std::result::Result<keyring::CommentsTcpDelegationKeyringRuntimeSelection, String> {
        self.current_status().map(|status| status.selection)
    }

    pub fn replace_host_keyring(
        &self,
        delegation_keyring: CommentsTcpDelegationKeyring,
        generation: u64,
        revoked_key_count: usize,
    ) -> std::result::Result<CommentsTcpDelegationKeyringReloadOutcome, String> {
        if !matches!(&self.0.source, DelegationReloadSource::HostProvided) {
            return self.reject(
                "Comments TCP file-backed delegation keyring must be reloaded from its configured file"
                    .to_string(),
            );
        }
        let candidate = match build_reload_snapshot(
            delegation_keyring,
            generation,
            revoked_key_count,
            keyring::CommentsTcpDelegationKeyringSource::HostProvided,
        ) {
            Ok(candidate) => candidate,
            Err(error) => return self.reject(error),
        };
        self.replace_candidate(candidate)
    }

    pub fn reload_file(
        &self,
    ) -> std::result::Result<CommentsTcpDelegationKeyringReloadOutcome, String> {
        let file_path = match &self.0.source {
            DelegationReloadSource::File(file_path) => file_path.clone(),
            DelegationReloadSource::HostProvided => {
                return self.reject(
                    "Comments TCP host-provided delegation keyring must be replaced programmatically"
                        .to_string(),
                );
            }
        };
        let candidate = match load_reload_snapshot_from_file(&file_path) {
            Ok(candidate) => candidate,
            Err(error) => return self.reject(error),
        };
        self.replace_candidate(candidate)
    }

    fn new(source: DelegationReloadSource, current: DelegationReloadSnapshot) -> Self {
        Self(Arc::new(DelegationReloadState {
            source,
            current: RwLock::new(current),
            successful_reloads: AtomicU64::new(0),
            rejected_reloads: AtomicU64::new(0),
        }))
    }

    fn replace_candidate(
        &self,
        candidate: DelegationReloadSnapshot,
    ) -> std::result::Result<CommentsTcpDelegationKeyringReloadOutcome, String> {
        let mut current = match self.0.current.write() {
            Ok(current) => current,
            Err(_) => {
                return self.reject(
                    "Comments TCP delegation keyring reload state is unavailable".to_string(),
                );
            }
        };
        if candidate.selection.source != current.selection.source {
            drop(current);
            return self.reject(
                "Comments TCP delegation keyring reload cannot change source category".to_string(),
            );
        }
        if candidate.selection.generation <= current.selection.generation {
            drop(current);
            return self.reject(
                "Comments TCP delegation keyring reload generation must be greater than the active generation"
                    .to_string(),
            );
        }
        let previous_generation = current.selection.generation;
        let current_selection = candidate.selection;
        *current = candidate;
        self.0.successful_reloads.fetch_add(1, Ordering::Relaxed);
        Ok(CommentsTcpDelegationKeyringReloadOutcome {
            previous_generation,
            current: current_selection,
        })
    }

    fn reject<T>(&self, message: String) -> std::result::Result<T, String> {
        self.0.rejected_reloads.fetch_add(1, Ordering::Relaxed);
        Err(message)
    }
}

impl CommentsTcpDelegationKeyringProvider for SharedCommentsTcpDelegationKeyringReloadHandle {
    fn current_keyring(&self) -> std::result::Result<CommentsTcpDelegationKeyring, PortError> {
        self.0
            .current
            .read()
            .map(|current| current.keyring.clone())
            .map_err(|_| {
                PortError::unavailable(
                    "comments.tcp_delegation_keyring_unavailable",
                    "Comments TCP delegation keyring is temporarily unavailable",
                )
            })
    }
}

impl std::fmt::Debug for SharedCommentsTcpDelegationKeyringReloadHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.current_status() {
            Ok(status) => formatter
                .debug_struct("SharedCommentsTcpDelegationKeyringReloadHandle")
                .field("source", &status.selection.source)
                .field("generation", &status.selection.generation)
                .field("retained_key_count", &status.selection.retained_key_count)
                .field("revoked_key_count", &status.selection.revoked_key_count)
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
                .debug_struct("SharedCommentsTcpDelegationKeyringReloadHandle")
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
struct DelegationReloadFileDocument {
    schema_version: u16,
    generation: u64,
    active_key_id: String,
    #[serde(default)]
    legacy_unkeyed_key_id: Option<String>,
    #[serde(default)]
    revoked_key_ids: Vec<String>,
    keys: Vec<DelegationReloadFileEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DelegationReloadFileEntry {
    key_id: String,
    secret: String,
}

pub(super) fn register_comments_provider_runtime(
    extensions: &mut ModuleRuntimeExtensions,
) -> std::result::Result<(), String> {
    let reload_enabled =
        match read_optional_environment(COMMENTS_TCP_DELEGATION_RELOAD_ENABLED_ENV)? {
            Some(value) => parse_bool_value(COMMENTS_TCP_DELEGATION_RELOAD_ENABLED_ENV, &value)?,
            None => false,
        };
    let host_handle = extensions
        .get::<SharedCommentsTcpDelegationKeyringReloadHandle>()
        .cloned();
    if host_handle.is_none() && !reload_enabled {
        return keyring::register_comments_provider_runtime(extensions);
    }

    if extensions.contains::<keyring::SharedCommentsTcpDelegationKeyringSnapshot>() {
        return Err(
            "Static and reloadable Comments TCP delegation keyring snapshots cannot be combined"
                .to_string(),
        );
    }
    let file_path = read_optional_environment(keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV)?;
    let legacy_secret = read_optional_environment(base::COMMENTS_TCP_DELEGATION_SECRET_ENV)?;
    let reload_handle = match host_handle {
        Some(handle) => {
            if file_path.is_some() || legacy_secret.is_some() {
                return Err(
                    "Host-provided Comments TCP delegation reload handle cannot be combined with file or legacy-secret environment configuration"
                        .to_string(),
                );
            }
            handle
        }
        None => {
            if legacy_secret.is_some() {
                return Err(format!(
                    "{COMMENTS_TCP_DELEGATION_RELOAD_ENABLED_ENV} cannot be combined with {}",
                    base::COMMENTS_TCP_DELEGATION_SECRET_ENV
                ));
            }
            let file_path = file_path.ok_or_else(|| {
                format!(
                    "{} is required when {COMMENTS_TCP_DELEGATION_RELOAD_ENABLED_ENV}=true",
                    keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
                )
            })?;
            SharedCommentsTcpDelegationKeyringReloadHandle::from_file(file_path)?
        }
    };

    let listener_configured = base::CommentsTcpListenerConfig::from_environment()?.is_some();
    let preconfigured = extensions.contains::<Arc<dyn CommentsThreadPort>>();
    let mode = if preconfigured {
        None
    } else {
        Some(comments_provider_mode())
    };
    let client_uses_reload = mode.as_deref() == Some("tcp");
    if !client_uses_reload && !listener_configured {
        return Err(
            "Comments TCP delegation reload handle requires a built-in TCP client or an enabled TCP listener"
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
                let (port, selection) = prepare_reloadable_tcp_client(extensions, &reload_handle)?;
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

    let keyring_selection = reload_handle.current_selection()?;
    if let Some(port) = prepared_port {
        extensions.insert::<Arc<dyn CommentsThreadPort>>(port);
    }
    extensions.insert(provider_selection);
    extensions.insert(keyring_selection);
    extensions.insert(reload_handle);
    Ok(())
}

pub(super) async fn start_comments_tcp_listener_if_enabled(
    runtime_ctx: &ServerRuntimeContext,
) -> Result<()> {
    let Some(reload_handle) = reload_handle_from_context(runtime_ctx) else {
        return keyring::start_comments_tcp_listener_if_enabled(runtime_ctx).await;
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
    if !authority_already_configured {
        let authority =
            comments_tcp_authority_from_reload_handle(&reload_handle).map_err(Error::BadRequest)?;
        runtime_ctx.shared_insert(base::SharedCommentsTcpAuthorityResolver(authority));
    }

    base::start_comments_tcp_listener_if_enabled(runtime_ctx).await
}

fn build_reload_snapshot(
    delegation_keyring: CommentsTcpDelegationKeyring,
    generation: u64,
    revoked_key_count: usize,
    source: keyring::CommentsTcpDelegationKeyringSource,
) -> std::result::Result<DelegationReloadSnapshot, String> {
    if generation == 0 {
        return Err(
            "Comments TCP delegation keyring generation must be greater than zero".to_string(),
        );
    }
    if revoked_key_count > MAX_COMMENTS_TCP_DELEGATION_KEYS {
        return Err(format!(
            "Comments TCP delegation revoked-key metadata must contain at most {MAX_COMMENTS_TCP_DELEGATION_KEYS} entries"
        ));
    }
    Ok(DelegationReloadSnapshot {
        selection: keyring::CommentsTcpDelegationKeyringRuntimeSelection {
            source,
            generation,
            retained_key_count: delegation_keyring.key_count(),
            revoked_key_count,
            legacy_unkeyed_enabled: delegation_keyring.accepts_legacy_unkeyed_tokens(),
        },
        keyring: delegation_keyring,
    })
}

fn load_reload_snapshot_from_file(
    file_path: &Path,
) -> std::result::Result<DelegationReloadSnapshot, String> {
    let bytes = read_bounded_reload_file(file_path)?;
    parse_reload_document(&bytes)
}

fn parse_reload_document(bytes: &[u8]) -> std::result::Result<DelegationReloadSnapshot, String> {
    let document = serde_json::from_slice::<DelegationReloadFileDocument>(bytes).map_err(|_| {
        format!(
            "{} must contain one valid version-1 Comments TCP delegation keyring JSON object",
            keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
        )
    })?;
    if document.schema_version != COMMENTS_TCP_DELEGATION_KEYRING_SCHEMA_VERSION {
        return Err(format!(
            "{} schema_version must equal {COMMENTS_TCP_DELEGATION_KEYRING_SCHEMA_VERSION}",
            keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
        ));
    }
    if document.generation == 0 {
        return Err(format!(
            "{} generation must be greater than zero",
            keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
        ));
    }
    if document.revoked_key_ids.len() > MAX_COMMENTS_TCP_DELEGATION_KEYS {
        return Err(format!(
            "{} revoked_key_ids must contain at most {MAX_COMMENTS_TCP_DELEGATION_KEYS} entries",
            keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
        ));
    }

    let mut retained_ids = HashSet::with_capacity(document.keys.len());
    let mut keys = Vec::with_capacity(document.keys.len());
    for entry in document.keys {
        let key_id = CommentsTcpDelegationKeyId::new(&entry.key_id).map_err(|error| {
            format!(
                "{} contains an invalid retained key ID: {error}",
                keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
            )
        })?;
        if !retained_ids.insert(entry.key_id) {
            return Err(format!(
                "{} retained key IDs must be unique",
                keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
            ));
        }
        let secret = CommentsTcpDelegationSecret::new(entry.secret).map_err(|error| {
            format!(
                "{} contains an invalid delegation secret: {error}",
                keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
            )
        })?;
        keys.push((key_id, secret));
    }

    let mut revoked_ids = HashSet::with_capacity(document.revoked_key_ids.len());
    for raw_key_id in &document.revoked_key_ids {
        CommentsTcpDelegationKeyId::new(raw_key_id).map_err(|error| {
            format!(
                "{} contains an invalid revoked key ID: {error}",
                keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
            )
        })?;
        if !revoked_ids.insert(raw_key_id.clone()) {
            return Err(format!(
                "{} revoked key IDs must be unique",
                keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
            ));
        }
        if retained_ids.contains(raw_key_id) {
            return Err(format!(
                "{} cannot retain and revoke the same key ID",
                keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
            ));
        }
    }

    let active_key_id =
        CommentsTcpDelegationKeyId::new(document.active_key_id).map_err(|error| {
            format!(
                "{} active_key_id is invalid: {error}",
                keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
            )
        })?;
    let mut delegation_keyring =
        CommentsTcpDelegationKeyring::new(active_key_id, keys).map_err(|error| {
            format!(
                "{} keyring is invalid: {error}",
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
        delegation_keyring = delegation_keyring
            .with_legacy_unkeyed_key_id(legacy_key_id)
            .map_err(|error| {
                format!(
                    "{} legacy key selection is invalid: {error}",
                    keyring::COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV
                )
            })?;
    }

    build_reload_snapshot(
        delegation_keyring,
        document.generation,
        revoked_ids.len(),
        keyring::CommentsTcpDelegationKeyringSource::File,
    )
}

fn read_bounded_reload_file(file_path: &Path) -> std::result::Result<Vec<u8>, String> {
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

fn prepare_reloadable_tcp_client(
    extensions: &ModuleRuntimeExtensions,
    reload_handle: &SharedCommentsTcpDelegationKeyringReloadHandle,
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
        Arc::new(reload_handle.clone()),
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

fn comments_tcp_authority_from_reload_handle(
    reload_handle: &SharedCommentsTcpDelegationKeyringReloadHandle,
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
        Arc::new(reload_handle.clone()),
    )
    .with_service_claim(COMMENTS_TCP_SERVICE_PERMISSION)
    .with_service_role(COMMENTS_TCP_SERVICE_ROLE)
    .with_max_ttl(Duration::from_millis(ttl_ms))
    .map_err(|error| format!("Comments TCP delegation TTL configuration failed: {error}"))?
    .with_replay_capacity(replay_capacity)
    .map_err(|error| format!("Comments TCP replay configuration failed: {error}"))?;
    Ok(Arc::new(resolver))
}

fn reload_handle_from_context(
    runtime_ctx: &ServerRuntimeContext,
) -> Option<SharedCommentsTcpDelegationKeyringReloadHandle> {
    runtime_ctx
        .shared_get::<SharedCommentsTcpDelegationKeyringReloadHandle>()
        .or_else(|| {
            runtime_ctx
                .shared_get::<Arc<ModuleRuntimeExtensions>>()
                .and_then(|extensions| {
                    extensions
                        .get::<SharedCommentsTcpDelegationKeyringReloadHandle>()
                        .cloned()
                })
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
