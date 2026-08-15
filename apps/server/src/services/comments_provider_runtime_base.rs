use std::{
    env,
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use rustok_api::PortActor;
use rustok_comments::{
    CommentsTcpAuthorityResolver, CommentsTcpBearerAuthorityResolver, CommentsTcpBearerToken,
    CommentsTcpChannelProtection, CommentsTcpClientChannelConnector,
    CommentsTcpDelegatingAuthorityResolver, CommentsTcpDelegationSecret,
    CommentsTcpDelegationSigner, CommentsTcpServerChannelAcceptor, CommentsThreadPort,
    CommentsThreadTransport, DEFAULT_COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY,
    DEFAULT_COMMENTS_TCP_DELEGATION_TTL_MS, MAX_COMMENTS_TCP_DELEGATION_TTL_MS,
    PlaintextLoopbackCommentsTcpChannel, TcpJsonCommentsServerAdapter, TcpJsonCommentsTransport,
    in_process_comments_thread_port, remote_comments_thread_port,
};
use rustok_core::ModuleRuntimeExtensions;
use tokio::{
    net::TcpListener,
    sync::Semaphore,
    task::{JoinError, JoinSet},
    time::timeout,
};
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::services::app_lifecycle::StopHandle;
use crate::services::event_bus::transactional_event_bus_from_context;
use crate::services::server_runtime_context::ServerRuntimeContext;

pub const COMMENTS_PROVIDER_MODE_ENV: &str = "RUSTOK_COMMENTS_PROVIDER_MODE";
pub const COMMENTS_TCP_ENDPOINT_ENV: &str = "RUSTOK_COMMENTS_TCP_ENDPOINT";
pub const COMMENTS_TCP_BEARER_TOKEN_ENV: &str = "RUSTOK_COMMENTS_TCP_BEARER_TOKEN";
pub const COMMENTS_TCP_SERVICE_ACTOR_ID_ENV: &str = "RUSTOK_COMMENTS_TCP_SERVICE_ACTOR_ID";
pub const COMMENTS_TCP_DELEGATION_SECRET_ENV: &str = "RUSTOK_COMMENTS_TCP_DELEGATION_SECRET";
pub const COMMENTS_TCP_DELEGATION_TTL_MS_ENV: &str = "RUSTOK_COMMENTS_TCP_DELEGATION_TTL_MS";
pub const COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY_ENV: &str =
    "RUSTOK_COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY";
pub const COMMENTS_TCP_LISTENER_ENABLED_ENV: &str = "RUSTOK_COMMENTS_TCP_LISTENER_ENABLED";
pub const COMMENTS_TCP_BIND_ENV: &str = "RUSTOK_COMMENTS_TCP_BIND";
pub const COMMENTS_TCP_MAX_CONNECTIONS_ENV: &str = "RUSTOK_COMMENTS_TCP_MAX_CONNECTIONS";
pub const COMMENTS_TCP_PRE_REQUEST_TIMEOUT_MS_ENV: &str =
    "RUSTOK_COMMENTS_TCP_PRE_REQUEST_TIMEOUT_MS";
pub const COMMENTS_TCP_SHUTDOWN_GRACE_MS_ENV: &str = "RUSTOK_COMMENTS_TCP_SHUTDOWN_GRACE_MS";
pub const COMMENTS_TCP_MAX_FRAME_BYTES_ENV: &str = "RUSTOK_COMMENTS_TCP_MAX_FRAME_BYTES";

const DEFAULT_COMMENTS_TCP_MAX_CONNECTIONS: usize = 64;
const DEFAULT_COMMENTS_TCP_PRE_REQUEST_TIMEOUT_MS: u64 = 2_000;
const DEFAULT_COMMENTS_TCP_SHUTDOWN_GRACE_MS: u64 = 5_000;
const DEFAULT_COMMENTS_TCP_MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
const COMMENTS_TCP_SERVICE_ROLE: &str = "admin";
const COMMENTS_TCP_SERVICE_PERMISSION: &str = "comments:manage";

static COMMENTS_TCP_LISTENER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommentsProviderProfile {
    InProcessFallback,
    Preconfigured,
    TcpLoopback,
    TcpProtectedLoopback,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommentsProviderRuntimeSelection {
    pub profile: CommentsProviderProfile,
    pub endpoint: Option<SocketAddr>,
}

/// Optional host-provided authority override for the Comments TCP listener.
///
/// When absent, the server composes the concrete bearer read resolver and, when
/// configured, the signed user-delegation write resolver. There is deliberately
/// no allow-all fallback.
#[derive(Clone)]
pub struct SharedCommentsTcpAuthorityResolver(pub Arc<dyn CommentsTcpAuthorityResolver>);

/// Optional host-provided client channel connector.
///
/// The connector is resolved while the consumer transport is published. Missing
/// configuration retains the built-in plaintext loopback connector. A connector
/// classified as authenticated and encrypted does not weaken bearer/delegation
/// authorization and does not yet enable non-loopback publication.
#[derive(Clone)]
pub struct SharedCommentsTcpClientChannelConnector(pub Arc<dyn CommentsTcpClientChannelConnector>);

/// Optional host-provided server channel acceptor.
///
/// Runtime-context registration takes precedence over module extensions. The
/// acceptor must finish its own bounded handshake before returning a byte
/// channel to the typed Comments server adapter.
#[derive(Clone)]
pub struct SharedCommentsTcpServerChannelAcceptor(pub Arc<dyn CommentsTcpServerChannelAcceptor>);

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
                "{COMMENTS_TCP_BIND_ENV} must remain loopback until protected Comments TCP runtime evidence is retained"
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
/// retains its existing database/event-bus fallback. `tcp` requires an explicit
/// loopback endpoint and bearer credential. Signed user delegation is enabled
/// only when a separate delegation secret is configured. A host-injected
/// authenticated encrypted connector is supported, but non-loopback publication
/// remains disabled until retained runtime evidence exists.
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
                format!("{COMMENTS_TCP_ENDPOINT_ENV} must be an explicit IP socket address")
            })?;
            let channel_connector = extensions
                .get::<SharedCommentsTcpClientChannelConnector>()
                .cloned()
                .map(|shared| shared.0)
                .unwrap_or_else(plaintext_client_channel_connector);
            let channel_protection = channel_connector.protection();
            require_loopback_endpoint(endpoint, channel_protection)?;

            let bearer_token = comments_tcp_bearer_token_from_environment()?;
            let transport = match comments_tcp_delegation_signer_from_environment()? {
                Some(signer) => {
                    TcpJsonCommentsTransport::with_channel_connector_bearer_and_delegation(
                        endpoint,
                        channel_connector,
                        bearer_token,
                        signer,
                    )
                }
                None => TcpJsonCommentsTransport::with_channel_connector_and_bearer_token(
                    endpoint,
                    channel_connector,
                    bearer_token,
                ),
            };
            let transport: Arc<dyn CommentsThreadTransport> = Arc::new(transport);
            extensions
                .insert::<Arc<dyn CommentsThreadPort>>(remote_comments_thread_port(transport));
            extensions.insert(CommentsProviderRuntimeSelection {
                profile: match channel_protection {
                    CommentsTcpChannelProtection::PlaintextLoopback => {
                        CommentsProviderProfile::TcpLoopback
                    }
                    CommentsTcpChannelProtection::AuthenticatedEncrypted => {
                        CommentsProviderProfile::TcpProtectedLoopback
                    }
                },
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
/// Binding is fail-closed: a loopback address, authenticated authority resolver,
/// bounded frame size, bounded concurrency, non-zero pre-request timeout, and
/// shutdown grace are all required before the task is spawned. A host-provided
/// channel acceptor is resolved before the accept loop starts.
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
            extensions
                .as_ref()
                .and_then(|values| values.get::<SharedCommentsTcpAuthorityResolver>().cloned())
        })
        .map(|shared| shared.0)
        .map(Ok)
        .unwrap_or_else(comments_tcp_authority_from_environment)
        .map_err(Error::BadRequest)?;
    let channel_acceptor = runtime_ctx
        .shared_get::<SharedCommentsTcpServerChannelAcceptor>()
        .or_else(|| {
            extensions.as_ref().and_then(|values| {
                values
                    .get::<SharedCommentsTcpServerChannelAcceptor>()
                    .cloned()
            })
        })
        .map(|shared| shared.0)
        .unwrap_or_else(plaintext_server_channel_acceptor);
    let channel_protection = channel_acceptor.protection();

    let provider = runtime_ctx
        .shared_get::<SharedCommentsTcpServerProvider>()
        .or_else(|| {
            extensions
                .as_ref()
                .and_then(|values| values.get::<SharedCommentsTcpServerProvider>().cloned())
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
        channel_acceptor,
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
        channel_protection = ?channel_protection,
        authentication = "bearer_or_delegated_hmac_or_host_override",
        max_connections = config.max_connections,
        pre_request_timeout_ms = config.pre_request_timeout.as_millis(),
        shutdown_grace_ms = config.shutdown_grace.as_millis(),
        max_frame_bytes = config.max_frame_bytes,
        "Comments TCP listener started"
    );
    Ok(())
}

fn plaintext_client_channel_connector() -> Arc<dyn CommentsTcpClientChannelConnector> {
    Arc::new(PlaintextLoopbackCommentsTcpChannel)
}

fn plaintext_server_channel_acceptor() -> Arc<dyn CommentsTcpServerChannelAcceptor> {
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
            "{COMMENTS_TCP_ENDPOINT_ENV} must be loopback while the Comments TCP connector is plaintext"
        )),
        CommentsTcpChannelProtection::AuthenticatedEncrypted => Err(format!(
            "{COMMENTS_TCP_ENDPOINT_ENV} must remain loopback until protected Comments TCP runtime evidence is retained"
        )),
    }
}

fn comments_tcp_bearer_token_from_environment()
-> std::result::Result<CommentsTcpBearerToken, String> {
    let secret = match env::var(COMMENTS_TCP_BEARER_TOKEN_ENV) {
        Ok(value) => value,
        Err(env::VarError::NotPresent) => {
            return Err(format!(
                "{COMMENTS_TCP_BEARER_TOKEN_ENV} is required for Comments TCP bearer authentication"
            ));
        }
        Err(env::VarError::NotUnicode(_)) => {
            return Err(format!(
                "{COMMENTS_TCP_BEARER_TOKEN_ENV} must contain valid UTF-8"
            ));
        }
    };

    CommentsTcpBearerToken::new(secret).map_err(|error| {
        format!("{COMMENTS_TCP_BEARER_TOKEN_ENV} is not a valid Comments TCP bearer token: {error}")
    })
}

fn comments_tcp_delegation_secret_from_environment()
-> std::result::Result<Option<CommentsTcpDelegationSecret>, String> {
    let Some(secret) = read_optional_environment(COMMENTS_TCP_DELEGATION_SECRET_ENV)? else {
        return Ok(None);
    };
    CommentsTcpDelegationSecret::new(secret).map(Some).map_err(|error| {
        format!(
            "{COMMENTS_TCP_DELEGATION_SECRET_ENV} is not a valid Comments TCP delegation secret: {error}"
        )
    })
}

fn comments_tcp_delegation_ttl_ms_from_environment() -> std::result::Result<u64, String> {
    let ttl_ms = parse_optional_positive_u64(
        COMMENTS_TCP_DELEGATION_TTL_MS_ENV,
        DEFAULT_COMMENTS_TCP_DELEGATION_TTL_MS,
    )?;
    if ttl_ms > MAX_COMMENTS_TCP_DELEGATION_TTL_MS {
        return Err(format!(
            "{COMMENTS_TCP_DELEGATION_TTL_MS_ENV} must be within 1..={MAX_COMMENTS_TCP_DELEGATION_TTL_MS}"
        ));
    }
    Ok(ttl_ms)
}

fn comments_tcp_delegation_signer_from_environment()
-> std::result::Result<Option<CommentsTcpDelegationSigner>, String> {
    let Some(secret) = comments_tcp_delegation_secret_from_environment()? else {
        return Ok(None);
    };
    let ttl_ms = comments_tcp_delegation_ttl_ms_from_environment()?;
    CommentsTcpDelegationSigner::with_ttl(secret, Duration::from_millis(ttl_ms))
        .map(Some)
        .map_err(|error| format!("Comments TCP delegation signer configuration failed: {error}"))
}

fn comments_tcp_authority_from_environment()
-> std::result::Result<Arc<dyn CommentsTcpAuthorityResolver>, String> {
    let token = comments_tcp_bearer_token_from_environment()?;
    let actor = comments_tcp_service_actor_from_environment()?;
    let Some(delegation_secret) = comments_tcp_delegation_secret_from_environment()? else {
        return Ok(Arc::new(
            CommentsTcpBearerAuthorityResolver::from_token(token, actor)
                .with_claim(COMMENTS_TCP_SERVICE_PERMISSION)
                .with_role(COMMENTS_TCP_SERVICE_ROLE),
        ));
    };

    let ttl_ms = comments_tcp_delegation_ttl_ms_from_environment()?;
    let replay_capacity = parse_optional_positive_usize(
        COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY_ENV,
        DEFAULT_COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY,
    )?;
    let resolver = CommentsTcpDelegatingAuthorityResolver::new(token, actor, delegation_secret)
        .with_service_claim(COMMENTS_TCP_SERVICE_PERMISSION)
        .with_service_role(COMMENTS_TCP_SERVICE_ROLE)
        .with_max_ttl(Duration::from_millis(ttl_ms))
        .map_err(|error| format!("Comments TCP delegation TTL configuration failed: {error}"))?
        .with_replay_capacity(replay_capacity)
        .map_err(|error| format!("Comments TCP replay configuration failed: {error}"))?;
    Ok(Arc::new(resolver))
}

fn comments_tcp_service_actor_from_environment() -> std::result::Result<PortActor, String> {
    let raw_actor_id = read_optional_environment(COMMENTS_TCP_SERVICE_ACTOR_ID_ENV)?.ok_or_else(|| {
        format!(
            "{COMMENTS_TCP_SERVICE_ACTOR_ID_ENV} is required when the built-in Comments TCP authority resolver is used"
        )
    })?;
    parse_comments_tcp_service_actor_id(&raw_actor_id)
}

fn parse_comments_tcp_service_actor_id(value: &str) -> std::result::Result<PortActor, String> {
    let actor_id = value.trim();
    if actor_id != value || Uuid::parse_str(actor_id).is_err() {
        return Err(format!(
            "{COMMENTS_TCP_SERVICE_ACTOR_ID_ENV} must be a canonical UUID without surrounding whitespace"
        ));
    }
    Ok(PortActor::service(actor_id.to_string()))
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
    channel_acceptor: Arc<dyn CommentsTcpServerChannelAcceptor>,
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
                let channel_acceptor = channel_acceptor.clone();
                let pre_request_timeout = config.pre_request_timeout;
                connections.spawn(async move {
                    let _permit = permit;
                    if let Err(error) = adapter
                        .handle_connection_with_acceptor_and_pre_request_timeout(
                            stream,
                            peer_addr,
                            channel_acceptor.as_ref(),
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

    #[test]
    fn selector_contract_exposes_profiles_and_environment_keys() {
        let selector: fn(&mut ModuleRuntimeExtensions) -> std::result::Result<(), String> =
            register_comments_provider_runtime;
        let _ = selector;
        assert_eq!(COMMENTS_PROVIDER_MODE_ENV, "RUSTOK_COMMENTS_PROVIDER_MODE");
        assert_eq!(COMMENTS_TCP_ENDPOINT_ENV, "RUSTOK_COMMENTS_TCP_ENDPOINT");
        assert_eq!(
            COMMENTS_TCP_BEARER_TOKEN_ENV,
            "RUSTOK_COMMENTS_TCP_BEARER_TOKEN"
        );
        assert_eq!(
            COMMENTS_TCP_SERVICE_ACTOR_ID_ENV,
            "RUSTOK_COMMENTS_TCP_SERVICE_ACTOR_ID"
        );
        assert_eq!(
            COMMENTS_TCP_DELEGATION_SECRET_ENV,
            "RUSTOK_COMMENTS_TCP_DELEGATION_SECRET"
        );
        assert_eq!(
            COMMENTS_TCP_DELEGATION_TTL_MS_ENV,
            "RUSTOK_COMMENTS_TCP_DELEGATION_TTL_MS"
        );
        assert_eq!(
            COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY_ENV,
            "RUSTOK_COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY"
        );
        assert_ne!(
            CommentsProviderProfile::TcpLoopback,
            CommentsProviderProfile::TcpProtectedLoopback
        );
    }

    #[test]
    fn listener_contract_exposes_bounded_defaults() {
        let starter = start_comments_tcp_listener_if_enabled;
        let _ = starter;
        assert_eq!(DEFAULT_COMMENTS_TCP_MAX_CONNECTIONS, 64);
        assert_eq!(DEFAULT_COMMENTS_TCP_PRE_REQUEST_TIMEOUT_MS, 2_000);
        assert_eq!(DEFAULT_COMMENTS_TCP_SHUTDOWN_GRACE_MS, 5_000);
        assert_eq!(DEFAULT_COMMENTS_TCP_MAX_FRAME_BYTES, 8 * 1024 * 1024);
        assert_eq!(DEFAULT_COMMENTS_TCP_DELEGATION_TTL_MS, 5_000);
        assert_eq!(MAX_COMMENTS_TCP_DELEGATION_TTL_MS, 30_000);
        assert_eq!(DEFAULT_COMMENTS_TCP_DELEGATION_REPLAY_CAPACITY, 4_096);
        assert!(parse_bool_value("enabled", "true").unwrap());
        assert!(!parse_bool_value("enabled", "off").unwrap());
        assert!(parse_positive_usize_value("limit", "0").is_err());
        assert!(parse_positive_u64_value("timeout", "0").is_err());
    }

    #[test]
    fn protected_connector_does_not_enable_non_loopback() {
        let endpoint: SocketAddr = "192.0.2.10:9000".parse().unwrap();
        assert!(
            require_loopback_endpoint(
                endpoint,
                CommentsTcpChannelProtection::AuthenticatedEncrypted,
            )
            .is_err()
        );
    }

    #[test]
    fn built_in_authority_actor_requires_canonical_uuid() {
        let actor_id = Uuid::new_v4().to_string();
        assert_eq!(
            parse_comments_tcp_service_actor_id(&actor_id).unwrap(),
            PortActor::service(actor_id.clone())
        );
        assert!(parse_comments_tcp_service_actor_id("not-a-uuid").is_err());
        assert!(parse_comments_tcp_service_actor_id(&format!(" {actor_id}")).is_err());
    }
}
