use std::{
    collections::HashSet, env, fs::File, io::Read, net::SocketAddr, path::Path, sync::Arc,
    time::Duration,
};

use rustok_api::PortActor;
use rustok_comments::{
    CommentsTcpAuthorityResolver, CommentsTcpBearerToken, CommentsTcpChannelProtection,
    CommentsTcpClientChannelConnector, CommentsTcpDelegatingAuthorityResolver,
    CommentsTcpDelegationKeyId, CommentsTcpDelegationKeyring, CommentsTcpDelegationSecret,
    CommentsTcpDelegationSigner, CommentsThreadPort, CommentsThreadTransport,
    MAX_COMMENTS_TCP_DELEGATION_KEYS, PlaintextLoopbackCommentsTcpChannel,
    TcpJsonCommentsTransport, remote_comments_thread_port,
};
use rustok_core::ModuleRuntimeExtensions;
use serde::Deserialize;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::services::server_runtime_context::ServerRuntimeContext;

use super::base;

pub const COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV: &str =
    "RUSTOK_COMMENTS_TCP_DELEGATION_KEYRING_FILE";
pub const MAX_COMMENTS_TCP_DELEGATION_KEYRING_FILE_BYTES: usize = 64 * 1024;

const COMMENTS_TCP_DELEGATION_KEYRING_SCHEMA_VERSION: u16 = 1;
const COMMENTS_TCP_SERVICE_ROLE: &str = "admin";
const COMMENTS_TCP_SERVICE_PERMISSION: &str = "comments:manage";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommentsTcpDelegationKeyringSource {
    HostProvided,
    File,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommentsTcpDelegationKeyringRuntimeSelection {
    pub source: CommentsTcpDelegationKeyringSource,
    pub generation: u64,
    pub retained_key_count: usize,
    pub revoked_key_count: usize,
    pub legacy_unkeyed_enabled: bool,
}

#[derive(Clone)]
pub struct SharedCommentsTcpDelegationKeyringSnapshot(Arc<DelegationKeyringSnapshot>);

#[derive(Clone)]
struct DelegationKeyringSnapshot {
    keyring: CommentsTcpDelegationKeyring,
    selection: CommentsTcpDelegationKeyringRuntimeSelection,
}

impl SharedCommentsTcpDelegationKeyringSnapshot {
    pub fn from_host_keyring(
        keyring: CommentsTcpDelegationKeyring,
        generation: u64,
        revoked_key_count: usize,
    ) -> std::result::Result<Self, String> {
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
        Ok(Self::new(
            keyring,
            CommentsTcpDelegationKeyringRuntimeSelection {
                source: CommentsTcpDelegationKeyringSource::HostProvided,
                generation,
                retained_key_count: 0,
                revoked_key_count,
                legacy_unkeyed_enabled: false,
            },
        ))
    }

    pub fn selection(&self) -> CommentsTcpDelegationKeyringRuntimeSelection {
        self.0.selection
    }

    fn new(
        keyring: CommentsTcpDelegationKeyring,
        mut selection: CommentsTcpDelegationKeyringRuntimeSelection,
    ) -> Self {
        selection.retained_key_count = keyring.key_count();
        selection.legacy_unkeyed_enabled = keyring.accepts_legacy_unkeyed_tokens();
        Self(Arc::new(DelegationKeyringSnapshot { keyring, selection }))
    }

    fn keyring(&self) -> CommentsTcpDelegationKeyring {
        self.0.keyring.clone()
    }
}

impl std::fmt::Debug for SharedCommentsTcpDelegationKeyringSnapshot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SharedCommentsTcpDelegationKeyringSnapshot")
            .field("source", &self.0.selection.source)
            .field("generation", &self.0.selection.generation)
            .field("retained_key_count", &self.0.selection.retained_key_count)
            .field("revoked_key_count", &self.0.selection.revoked_key_count)
            .field(
                "legacy_unkeyed_enabled",
                &self.0.selection.legacy_unkeyed_enabled,
            )
            .field("key_ids", &"[REDACTED]")
            .field("secrets", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DelegationKeyringFileDocument {
    schema_version: u16,
    generation: u64,
    active_key_id: String,
    #[serde(default)]
    legacy_unkeyed_key_id: Option<String>,
    #[serde(default)]
    revoked_key_ids: Vec<String>,
    keys: Vec<DelegationKeyFileEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DelegationKeyFileEntry {
    key_id: String,
    secret: String,
}

pub(super) fn register_comments_provider_runtime(
    extensions: &mut ModuleRuntimeExtensions,
) -> std::result::Result<(), String> {
    let Some(snapshot) = resolve_keyring_snapshot(extensions)? else {
        return base::register_comments_provider_runtime(extensions);
    };

    let listener_configured = base::CommentsTcpListenerConfig::from_environment()?.is_some();
    let preconfigured = extensions.contains::<Arc<dyn CommentsThreadPort>>();
    let mode = if preconfigured {
        None
    } else {
        Some(comments_provider_mode())
    };
    let client_uses_snapshot = mode.as_deref() == Some("tcp");
    if !client_uses_snapshot && !listener_configured {
        return Err(format!(
            "{COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV} or a host-provided Comments TCP delegation keyring requires a built-in TCP client or an enabled TCP listener"
        ));
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
                let (port, selection) = prepare_tcp_client(extensions, &snapshot)?;
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

    let keyring_selection = snapshot.selection();
    if let Some(port) = prepared_port {
        extensions.insert::<Arc<dyn CommentsThreadPort>>(port);
    }
    extensions.insert(provider_selection);
    extensions.insert(keyring_selection);
    extensions.insert(snapshot);
    Ok(())
}

pub(super) async fn start_comments_tcp_listener_if_enabled(
    runtime_ctx: &ServerRuntimeContext,
) -> Result<()> {
    let Some(snapshot) = keyring_snapshot_from_context(runtime_ctx) else {
        return base::start_comments_tcp_listener_if_enabled(runtime_ctx).await;
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
            comments_tcp_authority_from_snapshot(&snapshot).map_err(Error::BadRequest)?;
        runtime_ctx.shared_insert(base::SharedCommentsTcpAuthorityResolver(authority));
    }

    base::start_comments_tcp_listener_if_enabled(runtime_ctx).await
}

fn resolve_keyring_snapshot(
    extensions: &ModuleRuntimeExtensions,
) -> std::result::Result<Option<SharedCommentsTcpDelegationKeyringSnapshot>, String> {
    let host_snapshot = extensions
        .get::<SharedCommentsTcpDelegationKeyringSnapshot>()
        .cloned();
    let file_path = read_optional_environment(COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV)?;
    let legacy_secret = read_optional_environment(base::COMMENTS_TCP_DELEGATION_SECRET_ENV)?;

    if host_snapshot.is_some() && (file_path.is_some() || legacy_secret.is_some()) {
        return Err(
            "Host-provided Comments TCP delegation keyring cannot be combined with file or legacy-secret environment configuration"
                .to_string(),
        );
    }
    if file_path.is_some() && legacy_secret.is_some() {
        return Err(format!(
            "{COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV} cannot be combined with {}",
            base::COMMENTS_TCP_DELEGATION_SECRET_ENV
        ));
    }
    if let Some(snapshot) = host_snapshot {
        return Ok(Some(snapshot));
    }
    let Some(file_path) = file_path else {
        return Ok(None);
    };
    load_keyring_snapshot_from_file(&file_path).map(Some)
}

fn load_keyring_snapshot_from_file(
    file_path: &str,
) -> std::result::Result<SharedCommentsTcpDelegationKeyringSnapshot, String> {
    let bytes = read_bounded_keyring_file(file_path)?;
    parse_keyring_document(&bytes)
}

fn parse_keyring_document(
    bytes: &[u8],
) -> std::result::Result<SharedCommentsTcpDelegationKeyringSnapshot, String> {
    let document = serde_json::from_slice::<DelegationKeyringFileDocument>(bytes).map_err(|_| {
        format!(
            "{COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV} must contain one valid version-1 Comments TCP delegation keyring JSON object"
        )
    })?;
    if document.schema_version != COMMENTS_TCP_DELEGATION_KEYRING_SCHEMA_VERSION {
        return Err(format!(
            "{COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV} schema_version must equal {COMMENTS_TCP_DELEGATION_KEYRING_SCHEMA_VERSION}"
        ));
    }
    if document.generation == 0 {
        return Err(format!(
            "{COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV} generation must be greater than zero"
        ));
    }
    if document.revoked_key_ids.len() > MAX_COMMENTS_TCP_DELEGATION_KEYS {
        return Err(format!(
            "{COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV} revoked_key_ids must contain at most {MAX_COMMENTS_TCP_DELEGATION_KEYS} entries"
        ));
    }

    let mut retained_ids = HashSet::with_capacity(document.keys.len());
    let mut keys = Vec::with_capacity(document.keys.len());
    for entry in document.keys {
        let key_id = CommentsTcpDelegationKeyId::new(&entry.key_id).map_err(|error| {
            format!(
                "{COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV} contains an invalid retained key ID: {error}"
            )
        })?;
        if !retained_ids.insert(entry.key_id) {
            return Err(format!(
                "{COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV} retained key IDs must be unique"
            ));
        }
        let secret = CommentsTcpDelegationSecret::new(entry.secret).map_err(|error| {
            format!(
                "{COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV} contains an invalid delegation secret: {error}"
            )
        })?;
        keys.push((key_id, secret));
    }

    let mut revoked_ids = HashSet::with_capacity(document.revoked_key_ids.len());
    for raw_key_id in &document.revoked_key_ids {
        CommentsTcpDelegationKeyId::new(raw_key_id).map_err(|error| {
            format!(
                "{COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV} contains an invalid revoked key ID: {error}"
            )
        })?;
        if !revoked_ids.insert(raw_key_id.clone()) {
            return Err(format!(
                "{COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV} revoked key IDs must be unique"
            ));
        }
        if retained_ids.contains(raw_key_id) {
            return Err(format!(
                "{COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV} cannot retain and revoke the same key ID"
            ));
        }
    }

    let active_key_id =
        CommentsTcpDelegationKeyId::new(document.active_key_id).map_err(|error| {
            format!("{COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV} active_key_id is invalid: {error}")
        })?;
    let mut keyring = CommentsTcpDelegationKeyring::new(active_key_id, keys).map_err(|error| {
        format!("{COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV} keyring is invalid: {error}")
    })?;
    if let Some(raw_legacy_key_id) = document.legacy_unkeyed_key_id {
        let legacy_key_id = CommentsTcpDelegationKeyId::new(raw_legacy_key_id).map_err(|error| {
            format!(
                "{COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV} legacy_unkeyed_key_id is invalid: {error}"
            )
        })?;
        keyring = keyring
            .with_legacy_unkeyed_key_id(legacy_key_id)
            .map_err(|error| {
                format!(
                    "{COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV} legacy key selection is invalid: {error}"
                )
            })?;
    }

    Ok(SharedCommentsTcpDelegationKeyringSnapshot::new(
        keyring,
        CommentsTcpDelegationKeyringRuntimeSelection {
            source: CommentsTcpDelegationKeyringSource::File,
            generation: document.generation,
            retained_key_count: 0,
            revoked_key_count: revoked_ids.len(),
            legacy_unkeyed_enabled: false,
        },
    ))
}

fn read_bounded_keyring_file(file_path: &str) -> std::result::Result<Vec<u8>, String> {
    if file_path.is_empty() {
        return Err(format!(
            "{COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV} must reference a non-empty file path"
        ));
    }
    let mut file = File::open(Path::new(file_path))
        .map_err(|_| format!("{COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV} could not be opened"))?;
    let metadata = file.metadata().map_err(|_| {
        format!("{COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV} metadata could not be read")
    })?;
    if !metadata.is_file() {
        return Err(format!(
            "{COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV} must reference a regular file"
        ));
    }
    if metadata.len() == 0 || metadata.len() > MAX_COMMENTS_TCP_DELEGATION_KEYRING_FILE_BYTES as u64
    {
        return Err(format!(
            "{COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV} file size must be within 1..={MAX_COMMENTS_TCP_DELEGATION_KEYRING_FILE_BYTES} bytes"
        ));
    }

    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.by_ref()
        .take((MAX_COMMENTS_TCP_DELEGATION_KEYRING_FILE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| format!("{COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV} could not be read"))?;
    if bytes.is_empty() || bytes.len() > MAX_COMMENTS_TCP_DELEGATION_KEYRING_FILE_BYTES {
        return Err(format!(
            "{COMMENTS_TCP_DELEGATION_KEYRING_FILE_ENV} file size must be within 1..={MAX_COMMENTS_TCP_DELEGATION_KEYRING_FILE_BYTES} bytes"
        ));
    }
    Ok(bytes)
}

fn prepare_tcp_client(
    extensions: &ModuleRuntimeExtensions,
    snapshot: &SharedCommentsTcpDelegationKeyringSnapshot,
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
    let signer = CommentsTcpDelegationSigner::with_keyring_and_ttl(
        snapshot.keyring(),
        Duration::from_millis(ttl_ms),
    )
    .map_err(|error| format!("Comments TCP delegation signer configuration failed: {error}"))?;
    let transport = TcpJsonCommentsTransport::with_channel_connector_bearer_and_delegation(
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

fn comments_tcp_authority_from_snapshot(
    snapshot: &SharedCommentsTcpDelegationKeyringSnapshot,
) -> std::result::Result<Arc<dyn CommentsTcpAuthorityResolver>, String> {
    let token = comments_tcp_bearer_token_from_environment()?;
    let actor = comments_tcp_service_actor_from_environment()?;
    let ttl_ms = comments_tcp_delegation_ttl_ms_from_environment()?;
    let replay_capacity = parse_optional_positive_usize(
        base::COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY_ENV,
        rustok_comments::DEFAULT_COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY,
    )?;
    let resolver =
        CommentsTcpDelegatingAuthorityResolver::with_keyring(token, actor, snapshot.keyring())
            .with_service_claim(COMMENTS_TCP_SERVICE_PERMISSION)
            .with_service_role(COMMENTS_TCP_SERVICE_ROLE)
            .with_max_ttl(Duration::from_millis(ttl_ms))
            .map_err(|error| format!("Comments TCP delegation TTL configuration failed: {error}"))?
            .with_replay_capacity(replay_capacity)
            .map_err(|error| format!("Comments TCP replay configuration failed: {error}"))?;
    Ok(Arc::new(resolver))
}

fn keyring_snapshot_from_context(
    runtime_ctx: &ServerRuntimeContext,
) -> Option<SharedCommentsTcpDelegationKeyringSnapshot> {
    runtime_ctx
        .shared_get::<SharedCommentsTcpDelegationKeyringSnapshot>()
        .or_else(|| {
            runtime_ctx
                .shared_get::<Arc<ModuleRuntimeExtensions>>()
                .and_then(|extensions| {
                    extensions
                        .get::<SharedCommentsTcpDelegationKeyringSnapshot>()
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

#[cfg(test)]
mod tests {
    use super::*;

    const OLD_SECRET: &str = "0123456789abcdef0123456789abcdef";
    const NEW_SECRET: &str = "abcdef0123456789abcdef0123456789";

    fn valid_document() -> Vec<u8> {
        format!(
            r#"{{
                "schema_version": 1,
                "generation": 7,
                "active_key_id": "new-2026-08",
                "legacy_unkeyed_key_id": "old-2026-07",
                "revoked_key_ids": ["retired-2026-06"],
                "keys": [
                    {{"key_id": "old-2026-07", "secret": "{OLD_SECRET}"}},
                    {{"key_id": "new-2026-08", "secret": "{NEW_SECRET}"}}
                ]
            }}"#,
        )
        .into_bytes()
    }

    #[test]
    fn file_snapshot_retains_overlap_and_redacts_debug() {
        let snapshot = parse_keyring_document(&valid_document()).unwrap();
        assert_eq!(
            snapshot.selection(),
            CommentsTcpDelegationKeyringRuntimeSelection {
                source: CommentsTcpDelegationKeyringSource::File,
                generation: 7,
                retained_key_count: 2,
                revoked_key_count: 1,
                legacy_unkeyed_enabled: true,
            }
        );
        let debug = format!("{snapshot:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains(OLD_SECRET));
        assert!(!debug.contains(NEW_SECRET));
        assert!(!debug.contains("new-2026-08"));
    }

    #[test]
    fn file_snapshot_rejects_retained_revoked_overlap() {
        let document = format!(
            r#"{{
                "schema_version": 1,
                "generation": 1,
                "active_key_id": "active",
                "revoked_key_ids": ["active"],
                "keys": [{{"key_id": "active", "secret": "{OLD_SECRET}"}}]
            }}"#,
        );
        assert!(parse_keyring_document(document.as_bytes()).is_err());
    }

    #[test]
    fn file_snapshot_rejects_unknown_fields_and_zero_generation() {
        let unknown = format!(
            r#"{{
                "schema_version": 1,
                "generation": 1,
                "active_key_id": "active",
                "unexpected": true,
                "keys": [{{"key_id": "active", "secret": "{OLD_SECRET}"}}]
            }}"#,
        );
        assert!(parse_keyring_document(unknown.as_bytes()).is_err());

        let zero = format!(
            r#"{{
                "schema_version": 1,
                "generation": 0,
                "active_key_id": "active",
                "keys": [{{"key_id": "active", "secret": "{OLD_SECRET}"}}]
            }}"#,
        );
        assert!(parse_keyring_document(zero.as_bytes()).is_err());
    }

    #[test]
    fn host_snapshot_records_only_bounded_metadata() {
        let active = CommentsTcpDelegationKeyId::new("active").unwrap();
        let keyring = CommentsTcpDelegationKeyring::new(
            active.clone(),
            vec![(
                active,
                CommentsTcpDelegationSecret::new(OLD_SECRET).unwrap(),
            )],
        )
        .unwrap();
        let snapshot =
            SharedCommentsTcpDelegationKeyringSnapshot::from_host_keyring(keyring, 3, 1).unwrap();
        assert_eq!(
            snapshot.selection().source,
            CommentsTcpDelegationKeyringSource::HostProvided
        );
        assert_eq!(snapshot.selection().generation, 3);
        assert_eq!(snapshot.selection().retained_key_count, 1);
        assert_eq!(snapshot.selection().revoked_key_count, 1);
        assert!(!snapshot.selection().legacy_unkeyed_enabled);
    }
}
