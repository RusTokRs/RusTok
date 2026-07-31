use std::{
    env,
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use rustok_comments::{
    CommentsTcpAuthorityResolver, CommentsThreadPort, CommentsThreadTransport,
    TcpJsonCommentsServerAdapter, TcpJsonCommentsTransport, in_process_comments_thread_port,
    remote_comments_thread_port,
};
use rustok_core::ModuleRuntimeExtensions;
use tokio::{
    net::TcpListener,
    sync::Semaphore,
    task::{JoinError, JoinSet},
    time::timeout,
};

use crate::error::{Error, Result};
use crate::services::app_lifecycle::StopHandle;
use crate::services::event_bus::transactional_event_bus_from_context;
use crate::services::server_runtime_context::ServerRuntimeContext;

pub const COMMENTS_PROVIDER_MODE_ENV: &str = "RUSTOK_COMMENTS_PROVIDER_MODE";
pub const COMMENTS_TCP_ENDPOINT_ENV: &str = "RUSTOK_COMMENTS_TCP_ENDPOINT";
pub const COMMENTS_TCP_LISTENER_ENABLED_ENV: &str = "RUSTOK_COMMENTS_TCP_LISTENER_ENABLED";
pub const COMMENTS_TCP_BIND_ENV: &str = "RUSTOK_COMMENTS_TCP_BIND";
pub const COMMENTS_TCP_MAX_CONNECTIONS_ENV: &str = "RUSTOK_COMMENTS_TCP_MAX_CONNECTIONS";
pub const COMMENTS_TCP_PRE_REQUEST_TIMEOUT_MS_ENV: &str =
    "RUSTOK_COMMENTS_TCP_PRE_REQUEST_TIMEOUT_MS";
pub const COMMENTS_TCP_SHUTDOWN_GRACE_MS_ENV: &str =
    "RUSTOK_COMMENTS_TCP_SHUTDOWN_GRACE_MS";
pub const COMMENTS_TCP_MAX_FRAME_BYTES_ENV: &str = "RUSTOK_COMMENTS_TCP_MAX_FRAME_BYTES";

const DEFAULT_COMMENTS_TCP_MAX_CONNECTIONS: usize = 64;
const DEFAULT_COMMENTS_TCP_PRE_REQUEST_TIMEOUT_MS: u64 = 2_000;
const DEFAULT_COMMENTS_TCP_SHUTDOWN_GRACE_MS: u64 = 5_000;
const DEFAULT_COMMENTS_TCP_MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

static COMMENTS_TCP_LISTENER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommentsProviderProfile {
    InProcessFallback,
    Preconfigured,
    TcpLoopback,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommentsProviderRuntimeSelection {
    pub profile: CommentsProviderProfile,
    pub endpoint: Option<SocketAddr>,
}

/// Host-provided authority implementation required before a Comments TCP
/// listener may bind. There is deliberately no allow-all fallback.
#[derive(Clone)]
pub struct SharedCommentsTcpAuthorityResolver(pub Arc<dyn CommentsTcpAuthorityResolver>);

/// Optional host override for the provider served by the TCP listener.
///
/// This wrapper is intentionally distinct from the consumer-selected
/// `Arc<dyn CommentsThreadPort>` so a TCP client cannot accidentally be served
/// back through itself. When absent, the listener uses the owner-managed
/// in-process Comments provider.
#[derive(Clone)]
pub struct SharedCommentsTcpServerProvider(pub Arc<dyn CommentsThreadPort>);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommentsTcpListenerConfig {
    pub bind_addr: SocketAddr,
    pub max_connections: usize,
    pub pre_request_timeout: Duration,
    pub shutdown_grace: Duration,
    pub max_frame_bytes: usize,
}

impl CommentsTcpListenerConfig {
    pub fn from_environment() -> std::result::Result<Option<Self>, String> {
        let enabled = match read_optional_environment(COMMENTS_TCP_LISTENER_ENABLED_ENV)? {
            Some(value) => parse_bool_value(COMMENTS_TCP_LISTENER_ENABLED_ENV, &value)?,
            None => false,
        };
        if !enabled {
            return Ok(None);
        }

        let raw_bind = read_optional_environment(COMMENTS_TCP_BIND_ENV)?.ok_or_else(|| {
            format!(
                "{COMMENTS_TCP_BIND_ENV} is required when {COMMENTS_TCP_LISTENER_ENABLED_ENV}=true"
            )
        })?;
        let bind_addr = raw_bind.trim().parse::<SocketAddr>().map_err(|_| {
            format!("{COMMENTS_TCP_BIND_ENV} must be an explicit IP socket address")
        })?;
        if !bind_addr.ip().is_loopback() {
            return Err(format!(
                "{COMMENTS_TCP_BIND_ENV} must be loopback while Comments TCP transport is unencrypted"
            ));
        }
        if bind_addr.port() == 0 {
            return Err(format!(
                "{COMMENTS_TCP_BIND_ENV} must use an explicit non-zero port"
            ));
        }

        let max_connections = parse_optional_positive_usize(
            COMMENTS_TCP_MAX_CONNECTIONS_ENV,
            DEFAULT_COMMENTS_TCP_MAX_CONNECTIONS,
        )?;
        let pre_request_timeout_ms = parse_optional_positive_u64(
            COMMENTS_TCP_PRE_REQUEST_TIMEOUT_MS_ENV,
            DEFAULT_COMMENTS_TCP_PRE_REQUEST_TIMEOUT_MS,
        )?;
        let shutdown_grace_ms = parse_optional_positive_u64(
            COMMENTS_TCP_SHUTDOWN_GRACE_MS_ENV,
            DEFAULT_COMMENTS_TCP_SHUTDOWN_GRACE_MS,
        )?;
        let max_frame_bytes = parse_optional_positive_usize(
            COMMENTS_TCP_MAX_FRAME_BYTES_ENV,
            DEFAULT_COMMENTS_TCP_MAX_FRAME_BYTES,
        )?;
        if max_frame_bytes > u32::MAX as usize {
            return Err(format!(
                "{COMMENTS_TCP_MAX_FRAME_BYTES_ENV} must be within 1..=u32::MAX"
            ));
        }

        Ok(Some(Self {
            bind_addr,
            max_connections,
            pre_request_timeout: Duration::from_millis(pre_request_timeout_ms),
            shutdown_grace: Duration::from_millis(shutdown_grace_ms),
            max_frame_bytes,
        }))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommentsTcpListenerHandle {
    pub instance_id: u64,
    pub local_addr: SocketAddr,
}

struct CommentsTcpListenerLifecycleReservation;

/// Publishes the host-selected Comments provider through `ModuleRuntimeExtensions`.
///
/// The default `in_process` mode intentionally inserts no port. Blog therefore
/// retains its existing database/event-bus fallback. `tcp` publishes the typed
/// remote adapter only for an explicit loopback sidecar endpoint; plaintext TCP
/// is never enabled for a non-loopback address.
pub fn register_comments_provider_runtime(
    extensions: &mut ModuleRuntimeExtensions,
) -> std::result::Result<(), String> {
    if extensions.contains::<Arc<dyn CommentsThreadPort>>() {
        extensions.insert(CommentsProviderRuntimeSelection {
            profile: CommentsProviderProfile::Preconfigured,
            endpoint: None,
        });
        return Ok(());
    }

    let mode = env::var(COMMENTS_PROVIDER_MODE_ENV)
        .unwrap_or_else(|_| "in_process".to_string())
        .trim()
        .to_ascii_lowercase();

    match mode.as_str() {
        "in_process" => {
            extensions.insert(CommentsProviderRuntimeSelection {
                profile: CommentsProviderProfile::InProcessFallback,
                endpoint: None,
            });
            Ok(())
        }
        "tcp" => {
            let raw_endpoint = env::var(COMMENTS_TCP_ENDPOINT_ENV).map_err(|_| {
                format!(
                    "{COMMENTS_TCP_ENDPOINT_ENV} is required when {COMMENTS_PROVIDER_MODE_ENV}=tcp"
                )
            })?;
            let endpoint = raw_endpoint.trim().parse::<SocketAddr>().map_err(|_| {
                format!(
                    "{COMMENTS_TCP_ENDPOINT_ENV} must be an explicit IP socket address"
                )
            })?;
            if !endpoint.ip().is_loopback() {
                return Err(format!(
                    "{COMMENTS_TCP_ENDPOINT_ENV} must be loopback while Comments TCP transport is unencrypted"
                ));
            }

            let transport: Arc<dyn CommentsThreadTransport> =
                Arc::new(TcpJsonCommentsTransport::new(endpoint));
            extensions.insert::<Arc<dyn CommentsThreadPort>>(remote_comments_thread_port(transport));
            extensions.insert(CommentsProviderRuntimeSelection {
                profile: CommentsProviderProfile::TcpLoopback,
                endpoint: Some(endpoint),
            });
            Ok(())
        }
        _ => Err(format!(
            "{COMMENTS_PROVIDER_MODE_ENV} must be one of: in_process, tcp"
        )),
    }
}

/// Starts the opt-in host-owned Comments TCP listener exactly once.
///
/// Binding is fail-closed: a loopback address, explicit authority resolver,
/// bounded frame size, bounded concurrency, non-zero pre-request timeout, and
/// shutdown grace are all required before the task is spawned.
pub async fn start_comments_tcp_listener_if_enabled(
    runtime_ctx: &ServerRuntimeContext,
) -> Result<()> {
    let Some(config) = CommentsTcpListenerConfig::from_environment().map_err(Error::BadRequest)?
    else {
        return Ok(());
    };

    if runtime_ctx.settings().runtime.is_registry_only()
        || runtime_ctx.settings().runtime.is_worker_only()
    {
        return Err(Error::BadRequest(
            "Comments TCP listener requires an HTTP-serving runtime host mode".to_string(),
        ));
    }

    if !runtime_ctx.shared_insert_if_absent(CommentsTcpListenerLifecycleReservation) {
        return Ok(());
    }

    let result = start_comments_tcp_listener(runtime_ctx, config).await;
    if result.is_err() {
        let _ = runtime_ctx.shared_take::<CommentsTcpListenerLifecycleReservation>();
    }
    result
}

async fn start_comments_tcp_listener(
    runtime_ctx: &ServerRuntimeContext,
    config: CommentsTcpListenerConfig,
) -> Result<()> {
    let extensions = runtime_ctx.shared_get::<Arc<ModuleRuntimeExtensions>>();
    let authority = runtime_ctx
        .shared_get::<SharedCommentsTcpAuthorityResolver>()
        .or_else(|| {
            extensions.as_ref().and_then(|values| {
                values
                    .get::<SharedCommentsTcpAuthorityResolver>()
                    .cloned()
            })
        })
        .ok_or_else(|| {
            Error::BadRequest(
                "Comments TCP listener requires a host-provided SharedCommentsTcpAuthorityResolver"
                    .to_string(),
            )
        })?
        .0;

    let provider = runtime_ctx
        .shared_get::<SharedCommentsTcpServerProvider>()
        .or_else(|| {
            extensions.as_ref().and_then(|values| {
                values
                    .get::<SharedCommentsTcpServerProvider>()
                    .cloned()
            })
        })
        .map(|shared| shared.0)
        .unwrap_or_else(|| {
            in_process_comments_thread_port(
                runtime_ctx.db_clone(),
                transactional_event_bus_from_context(runtime_ctx),
            )
        });

    let adapter = TcpJsonCommentsServerAdapter::with_max_frame_bytes(
        provider,
        authority,
        config.max_frame_bytes,
    )
    .map_err(|error| {
        Error::BadRequest(format!(
            "Comments TCP server adapter configuration failed with {} ({:?})",
            error.code, error.kind
        ))
    })?;

    let listener = TcpListener::bind(config.bind_addr).await.map_err(|error| {
        Error::Message(format!(
            "failed to bind Comments TCP listener at {}: {error}",
            config.bind_addr
        ))
    })?;
    let local_addr = listener.local_addr().map_err(|error| {
        Error::Message(format!(
            "failed to read Comments TCP listener address after bind: {error}"
        ))
    })?;

    let stop_handle = ensure_stop_handle(runtime_ctx);
    let stop_rx = stop_handle.subscribe();
    let instance_id = COMMENTS_TCP_LISTENER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    tokio::spawn(run_comments_tcp_listener(
        listener,
        adapter,
        config,
        stop_rx,
        instance_id,
    ));
    runtime_ctx.shared_insert(CommentsTcpListenerHandle {
        instance_id,
        local_addr,
    });

    tracing::info!(
        instance_id,
        bind_addr = %local_addr,
        max_connections = config.max_connections,
        pre_request_timeout_ms = config.pre_request_timeout.as_millis(),
        shutdown_grace_ms = config.shutdown_grace.as_millis(),
        max_frame_bytes = config.max_frame_bytes,
        "Comments TCP listener started"
    );
    Ok(())
}

fn ensure_stop_handle(runtime_ctx: &ServerRuntimeContext) -> StopHandle {
    if let Some(handle) = runtime_ctx.shared_get::<StopHandle>() {
        return handle;
    }

    let (candidate, _receiver) = StopHandle::new();
    let _ = runtime_ctx.shared_insert_if_absent(candidate.clone());
    runtime_ctx
        .shared_get::<StopHandle>()
        .expect("StopHandle must exist after Comments TCP listener initialization")
}

async fn run_comments_tcp_listener(
    listener: TcpListener,
    adapter: TcpJsonCommentsServerAdapter,
    config: CommentsTcpListenerConfig,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
    instance_id: u64,
) {
    let permits = Arc::new(Semaphore::new(config.max_connections));
    let mut connections = JoinSet::new();

    loop {
        if *stop_rx.borrow() {
            break;
        }

        tokio::select! {
            changed = stop_rx.changed() => {
                if changed.is_err() || *stop_rx.borrow() {
                    break;
                }
            }
            accepted = listener.accept() => {
                let (stream, peer_addr) = match accepted {
                    Ok(value) => value,
                    Err(error) => {
                        tracing::warn!(instance_id, error = %error, "Comments TCP accept failed");
                        continue;
                    }
                };
                if !peer_addr.ip().is_loopback() {
                    tracing::warn!(instance_id, peer_addr = %peer_addr, "Rejected non-loopback Comments TCP peer");
                    drop(stream);
                    continue;
                }
                let permit = match permits.clone().try_acquire_owned() {
                    Ok(permit) => permit,
                    Err(_) => {
                        tracing::warn!(instance_id, peer_addr = %peer_addr, max_connections = config.max_connections, "Rejected Comments TCP connection at concurrency limit");
                        drop(stream);
                        continue;
                    }
                };
                let adapter = adapter.clone();
                let pre_request_timeout = config.pre_request_timeout;
                connections.spawn(async move {
                    let _permit = permit;
                    if let Err(error) = adapter
                        .handle_connection_with_pre_request_timeout(
                            stream,
                            peer_addr,
                            pre_request_timeout,
                        )
                        .await
                    {
                        tracing::warn!(
                            peer_addr = %peer_addr,
                            code = %error.code,
                            kind = ?error.kind,
                            retryable = error.retryable,
                            "Comments TCP connection failed closed"
                        );
                    }
                });
            }
            Some(result) = connections.join_next(), if !connections.is_empty() => {
                log_connection_join(instance_id, result);
            }
        }
    }

    drop(listener);
    tracing::info!(
        instance_id,
        active_connections = connections.len(),
        "Comments TCP listener stopped accepting connections"
    );

    let drain_result = timeout(config.shutdown_grace, async {
        while let Some(result) = connections.join_next().await {
            log_connection_join(instance_id, result);
        }
    })
    .await;

    if drain_result.is_err() {
        let aborted_connections = connections.len();
        connections.abort_all();
        while let Some(result) = connections.join_next().await {
            log_connection_join(instance_id, result);
        }
        tracing::warn!(
            instance_id,
            aborted_connections,
            "Comments TCP listener shutdown grace elapsed; remaining connections were aborted"
        );
    } else {
        tracing::info!(instance_id, "Comments TCP listener drained all connections");
    }
}

fn log_connection_join(instance_id: u64, result: std::result::Result<(), JoinError>) {
    if let Err(error) = result {
        tracing::warn!(
            instance_id,
            cancelled = error.is_cancelled(),
            panicked = error.is_panic(),
            "Comments TCP connection task ended unexpectedly"
        );
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

fn parse_positive_u64_value(
    key: &'static str,
    value: &str,
) -> std::result::Result<u64, String> {
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

    #[test]
    fn selector_contract_exposes_profiles_and_environment_keys() {
        let selector: fn(&mut ModuleRuntimeExtensions) -> std::result::Result<(), String> =
            register_comments_provider_runtime;
        let _ = selector;
        assert_eq!(COMMENTS_PROVIDER_MODE_ENV, "RUSTOK_COMMENTS_PROVIDER_MODE");
        assert_eq!(COMMENTS_TCP_ENDPOINT_ENV, "RUSTOK_COMMENTS_TCP_ENDPOINT");
    }

    #[test]
    fn listener_contract_exposes_bounded_defaults() {
        let starter = start_comments_tcp_listener_if_enabled;
        let _ = starter;
        assert_eq!(DEFAULT_COMMENTS_TCP_MAX_CONNECTIONS, 64);
        assert_eq!(DEFAULT_COMMENTS_TCP_PRE_REQUEST_TIMEOUT_MS, 2_000);
        assert_eq!(DEFAULT_COMMENTS_TCP_SHUTDOWN_GRACE_MS, 5_000);
        assert_eq!(DEFAULT_COMMENTS_TCP_MAX_FRAME_BYTES, 8 * 1024 * 1024);
        assert!(parse_bool_value("enabled", "true").unwrap());
        assert!(!parse_bool_value("enabled", "off").unwrap());
        assert!(parse_positive_usize_value("limit", "0").is_err());
        assert!(parse_positive_u64_value("timeout", "0").is_err());
    }
}
