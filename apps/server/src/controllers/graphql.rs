use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};

use async_graphql::Data;
use async_graphql::http::{GraphQLPlaygroundConfig, WebSocketProtocols, WsMessage};
use axum::{
    Extension, Json,
    extract::{
        State,
        rejection::JsonRejection,
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use futures_util::{SinkExt, StreamExt};
use rustok_api::{
    Action, AuthPrincipalContext, ChannelContext, OptionalChannel, PLATFORM_FALLBACK_LOCALE,
    Permission, PortActor, PortContext,
};

use rustok_core::i18n::Locale;
use rustok_tenant::{TenantLocalePolicyPort, TenantService};
use tokio_stream::wrappers::ReceiverStream;

use crate::common::RequestContext;
use crate::context::{AuthContext, TenantContext};
use crate::extractors::auth::{
    OptionalCurrentUser, WebSocketAuthLease, resolve_current_user_from_access_token,
};
use crate::graphql::AppSchema;
use crate::graphql::persisted::is_cataloged_admin_hash;
use crate::middleware::tenant;
use crate::services::rbac_request_scope::{RbacRequestScope, with_rbac_request_scope};
use crate::services::server_runtime_context::{ServerAuthRuntime, ServerRuntimeContext};
use rustok_core::ModuleRegistry;

const WS_CLOSE_UNAUTHORIZED: u16 = 4401;
const WS_AUTHORITY_CHANGED_REASON: &str = "authorization changed; reconnect required";
const WS_MAX_MESSAGE_SIZE: usize = 256 * 1024;
const WS_MAX_FRAME_SIZE: usize = 256 * 1024;
const WS_INCOMING_QUEUE_CAPACITY: usize = 32;
const WS_CONNECTION_INIT_TIMEOUT: Duration = Duration::from_secs(10);
const WS_CONNECTION_INIT_VALIDATION_TIMEOUT: Duration = Duration::from_secs(15);
const WS_CONNECTION_INIT_TIMEOUT_CLOSE: u16 = 4408;
const WS_CONNECTION_INIT_TIMEOUT_REASON: &str = "Connection initialisation timeout";
const WS_MAX_ACTIVE_OPERATIONS: usize = 32;
const WS_MAX_OPERATION_ID_LEN: usize = 128;
const WS_AUTH_REVALIDATION_INTERVAL: Duration = Duration::from_secs(30);
const WS_SEND_TIMEOUT: Duration = Duration::from_secs(5);
const WS_CLOSE_DUPLICATE_OPERATION: u16 = 4409;
const WS_CLOSE_TOO_MANY_OPERATIONS: u16 = 4429;
const WS_CLOSE_INVALID_OPERATION: u16 = 4400;
const WS_DUPLICATE_OPERATION_REASON: &str = "Subscriber for this operation id already exists";
const WS_TOO_MANY_OPERATIONS_REASON: &str = "Too many active GraphQL operations";
const WS_INVALID_OPERATION_REASON: &str = "Invalid GraphQL operation id";

/// Normalize canonical RBAC implications for GraphQL policies that inspect an
/// immutable permission vector directly.
///
/// The task-local RBAC lease keeps the original snapshot for drift detection.
/// Only GraphQL request data receives the concrete read/list permissions that
/// are already implied by `manage`, preventing false denials without widening
/// OAuth authority beyond the token's effective snapshot.
fn graphql_permissions(mut permissions: Vec<Permission>) -> Vec<Permission> {
    let managed_resources = permissions
        .iter()
        .filter(|permission| permission.action == Action::Manage)
        .map(|permission| permission.resource)
        .collect::<Vec<_>>();

    for resource in managed_resources {
        for action in [Action::Read, Action::List] {
            let implied = Permission::new(resource, action);
            if !permissions.contains(&implied) {
                permissions.push(implied);
            }
        }
    }

    permissions.sort_by_cached_key(ToString::to_string);
    permissions.dedup();
    permissions
}

#[allow(clippy::too_many_arguments)]
async fn graphql_handler(
    State(runtime_ctx): State<ServerRuntimeContext>,
    Extension(registry): Extension<ModuleRegistry>,
    Extension(schema): Extension<Arc<AppSchema>>,
    tenant_ctx: TenantContext,
    request_context: RequestContext,
    OptionalCurrentUser(current_user): OptionalCurrentUser,
    OptionalChannel(channel): OptionalChannel,
    headers: HeaderMap,
    json_request: Result<Json<async_graphql::Request>, JsonRejection>,
) -> Response {
    let Json(req) = match json_request {
        Ok(request) => request,
        Err(rejection) => return graphql_json_rejection_response(rejection),
    };

    let p_hash = persisted_query_hash(&req);
    tracing::debug!(
        tenant_id = %tenant_ctx.id,
        locale = %request_context.locale,
        user_id = ?current_user.as_ref().map(|u| u.user.id),
        persisted_query_hash = ?p_hash,
        "Handling GraphQL HTTP request"
    );

    let db = runtime_ctx.db_clone();
    let locale = Locale::parse(&request_context.locale).unwrap_or_default();
    if let Some(hash) = p_hash {
        tracing::debug!(
            persisted_query_hash = hash,
            cataloged_admin_hash = is_cataloged_admin_hash(hash),
            "Observed persisted query hash for GraphQL telemetry"
        );
    }

    let mut request = req
        .data(runtime_ctx)
        .data(db)
        .data(tenant_ctx)
        .data(request_context)
        .data(headers)
        .data(registry)
        .data(locale);
    if let Some(channel) = channel {
        request = request.data(channel);
    }

    let rbac_scope = current_user.as_ref().map(|current_user| {
        RbacRequestScope::new(
            current_user.user.tenant_id,
            current_user.user.id,
            current_user.permissions.clone(),
            current_user.inferred_role.clone(),
        )
    });

    if let Some(current_user) = current_user {
        let principal_context = AuthPrincipalContext::new(current_user.principal_kind);
        let auth_ctx = AuthContext {
            user_id: current_user.user.id,
            session_id: current_user.session_id,
            tenant_id: current_user.user.tenant_id,
            permissions: graphql_permissions(current_user.permissions),
            client_id: current_user.client_id,
            scopes: current_user.scopes,
            grant_type: current_user.grant_type,
        };
        request = request.data(auth_ctx).data(principal_context);
        if let Some(scope) = rbac_scope.as_ref() {
            request = request.data(scope.clone());
        }
    }

    let response = with_rbac_request_scope(rbac_scope, schema.execute(request)).await;
    graphql_http_response(response)
}

fn graphql_json_rejection_message(status: StatusCode) -> &'static str {
    if status == StatusCode::PAYLOAD_TOO_LARGE {
        "GraphQL request body is too large"
    } else {
        "Invalid GraphQL JSON request"
    }
}

fn graphql_json_rejection_response(rejection: JsonRejection) -> Response {
    let status = if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
        StatusCode::PAYLOAD_TOO_LARGE
    } else {
        StatusCode::BAD_REQUEST
    };

    (
        status,
        Json(serde_json::json!({
            "errors": [{
                "message": graphql_json_rejection_message(status)
            }]
        })),
    )
        .into_response()
}

fn graphql_http_response(response: async_graphql::Response) -> Response {
    let graphql_headers = response.http_headers.clone();
    let mut response = Json(response).into_response();
    response.headers_mut().extend(graphql_headers);
    response
}

fn persisted_query_hash(req: &async_graphql::Request) -> Option<&str> {
    use async_graphql::Value;

    let value = req.extensions.get("persistedQuery")?;
    let Value::Object(obj) = value else {
        return None;
    };
    let Value::String(hash) = obj.get("sha256Hash")? else {
        return None;
    };

    let hash = hash.as_str();
    (hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())).then_some(hash)
}

async fn graphql_playground() -> impl axum::response::IntoResponse {
    tracing::debug!("Handling GraphQL playground HTML request");
    axum::response::Html(async_graphql::http::playground_source(
        GraphQLPlaygroundConfig::new("/api/graphql").subscription_endpoint("/api/graphql/ws"),
    ))
}

async fn graphql_schema_sdl(Extension(schema): Extension<Arc<AppSchema>>) -> impl IntoResponse {
    tracing::debug!("Handling GraphQL schema SDL request");
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        schema.sdl(),
    )
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
struct GraphqlWsInitPayload {
    token: Option<String>,
    #[serde(rename = "tenantSlug", alias = "tenant_slug")]
    tenant_slug: Option<String>,
    locale: Option<String>,
}

async fn graphql_ws_handler(
    ws: WebSocketUpgrade,
    State(runtime_ctx): State<ServerRuntimeContext>,
    State(auth_runtime): State<ServerAuthRuntime>,
    Extension(registry): Extension<ModuleRegistry>,
    Extension(schema): Extension<Arc<AppSchema>>,
    OptionalChannel(channel): OptionalChannel,
) -> impl IntoResponse {
    let ws = ws
        .protocols(async_graphql::http::ALL_WEBSOCKET_PROTOCOLS)
        .max_message_size(WS_MAX_MESSAGE_SIZE)
        .max_frame_size(WS_MAX_FRAME_SIZE);
    let selected_proto = ws
        .selected_protocol()
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<WebSocketProtocols>().ok());
    tracing::debug!(
        selected_protocol = ?selected_proto,
        "Handling GraphQL WebSocket upgrade request"
    );
    let Some(protocol) = selected_proto else {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            "GraphQL WebSocket subprotocol negotiation is required",
        )
            .into_response();
    };

    ws.on_upgrade(move |socket| {
        handle_graphql_ws(
            socket,
            schema,
            runtime_ctx,
            auth_runtime,
            registry,
            protocol,
            channel,
        )
    })
}

async fn handle_graphql_ws(
    socket: WebSocket,
    schema: Arc<AppSchema>,
    runtime_ctx: ServerRuntimeContext,
    auth_runtime: ServerAuthRuntime,
    registry: ModuleRegistry,
    protocol: WebSocketProtocols,
    channel: Option<ChannelContext>,
) {
    let (mut sink, mut source) = socket.split();
    let (incoming_tx, incoming_rx) =
        tokio::sync::mpsc::channel::<String>(WS_INCOMING_QUEUE_CAPACITY);
    let auth_lease = Arc::new(OnceLock::<WebSocketAuthLease>::new());
    let connection_init_received = Arc::new(OnceLock::<()>::new());
    let operations = Arc::new(std::sync::Mutex::new(WsOperationTracker::default()));
    let (close_tx, mut close_rx) = tokio::sync::mpsc::channel::<WsCloseReason>(1);
    let _close_tx_keepalive = close_tx.clone();

    let schema_for_stream = schema.as_ref().clone();
    let runtime_ctx_for_init = runtime_ctx.clone();
    let auth_runtime_for_init = auth_runtime.clone();
    let registry_for_init = registry.clone();
    let channel_for_init = channel.clone();
    let auth_lease_for_init = Arc::clone(&auth_lease);
    let connection_init_received_for_init = Arc::clone(&connection_init_received);
    let mut graphql_stream = async_graphql::http::WebSocket::new(
        schema_for_stream,
        WsAdmissionStream::new(incoming_rx, Arc::clone(&operations), close_tx),
        protocol,
    )
    .on_connection_init(move |payload| {
        let _ = connection_init_received_for_init.set(());
        bounded_ws_connection_init(
            build_ws_connection_data(
                runtime_ctx_for_init.clone(),
                auth_runtime_for_init.clone(),
                registry_for_init.clone(),
                channel_for_init.clone(),
                Arc::clone(&auth_lease_for_init),
                payload,
            ),
            WS_CONNECTION_INIT_VALIDATION_TIMEOUT,
        )
    });

    let forward_incoming = tokio::spawn(async move {
        while let Some(message) = source.next().await {
            let text = match message {
                Ok(Message::Text(text)) => text.to_string(),
                Ok(Message::Binary(bytes)) => {
                    let Ok(text) = String::from_utf8(bytes.to_vec()) else {
                        continue;
                    };
                    text
                }
                Ok(Message::Close(_)) => break,
                Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => continue,
                Err(_) => break,
            };

            if incoming_tx.send(text).await.is_err() {
                break;
            }
        }
    });

    // The timeout covers receipt of the init message, not tenant/auth/locale validation after
    // the callback starts. `connection_init_received` is set at callback entry so a slow but
    // legitimately received initialization is not cancelled by the transport deadline.
    let mut connection_init_deadline = Box::pin(tokio::time::sleep(WS_CONNECTION_INIT_TIMEOUT));
    let mut auth_revalidation = tokio::time::interval(WS_AUTH_REVALIDATION_INTERVAL);
    auth_revalidation.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        let scope_before_poll = match auth_lease.get() {
            Some(lease) => match lease.revalidate(&auth_runtime).await {
                Ok(scope) => Some(scope),
                Err(_) => {
                    let _ = close_ws_for_auth_change(&mut sink).await;
                    break;
                }
            },
            None => None,
        };

        let next_message = tokio::select! {
            timed_out = connection_init_timed_out(
                connection_init_deadline.as_mut(), &connection_init_received,
            ), if auth_lease.get().is_none() && connection_init_received.get().is_none() =>
            {
                // The init callback may have started while this select was pending.
                // Guards are evaluated only when select starts, so check again.
                if !timed_out {
                    continue;
                }
                let _ = close_ws(
                    &mut sink,
                    WS_CONNECTION_INIT_TIMEOUT_CLOSE,
                    WS_CONNECTION_INIT_TIMEOUT_REASON,
                )
                .await;
                break;
            }
            _ = async {
                if let Some(lease) = auth_lease.get() {
                    tokio::time::sleep(lease.remaining()).await;
                }
            }, if auth_lease.get().is_some() => {
                let _ = close_ws_for_auth_change(&mut sink).await;
                break;
            }
            reason = close_rx.recv() => {
                if let Some(reason) = reason {
                    let _ = close_ws(&mut sink, reason.code, reason.message).await;
                }
                break;
            }
            _ = auth_revalidation.tick(), if auth_lease.get().is_some() => {
                if let Some(lease) = auth_lease.get()
                    && lease.revalidate(&auth_runtime).await.is_err()
                {
                    let _ = close_ws_for_auth_change(&mut sink).await;
                    break;
                }
                continue;
            }
            message = with_rbac_request_scope(scope_before_poll, graphql_stream.next()) => message,
        };
        // Admission may reject an input while the protocol engine also has a
        // ready output. Prioritize the close over that output.
        if let Ok(reason) = close_rx.try_recv() {
            let _ = close_ws(&mut sink, reason.code, reason.message).await;
            break;
        }
        let Some(message) = next_message else {
            break;
        };

        if let Some(lease) = auth_lease.get()
            && lease.revalidate(&auth_runtime).await.is_err()
        {
            let _ = close_ws_for_auth_change(&mut sink).await;
            break;
        }

        let message = match message {
            WsMessage::Text(text) => {
                if !ws_should_forward_text(
                    protocol,
                    &text,
                    &mut operations
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner),
                ) {
                    continue;
                }
                Message::Text(text.into())
            }
            WsMessage::Close(code, reason) => Message::Close(Some(CloseFrame {
                code,
                reason: reason.into(),
            })),
        };

        let remaining = auth_lease.get().map(WebSocketAuthLease::remaining);
        if !send_ws_message(&mut sink, message, remaining).await {
            break;
        }
    }

    forward_incoming.abort();
}

async fn bounded_ws_connection_init<F>(
    validation: F,
    timeout: Duration,
) -> async_graphql::Result<Data>
where
    F: std::future::Future<Output = async_graphql::Result<Data>>,
{
    match tokio::time::timeout(timeout, validation).await {
        Ok(result) => result,
        Err(_) => {
            tracing::warn!("GraphQL WebSocket initialization validation timed out");
            Err(async_graphql::Error::new(
                "Connection initialization timed out",
            ))
        }
    }
}

async fn connection_init_timed_out(
    deadline: std::pin::Pin<&mut tokio::time::Sleep>,
    connection_init_received: &OnceLock<()>,
) -> bool {
    deadline.await;
    connection_init_received.get().is_none()
}

// Bound writes by both a fixed timeout and the remaining credential lifetime;
// a blocked peer cannot keep the connection task alive indefinitely.
async fn send_ws_message<S>(sink: &mut S, message: Message, remaining: Option<Duration>) -> bool
where
    S: futures_util::Sink<Message> + Unpin,
{
    let timeout = remaining.unwrap_or(WS_SEND_TIMEOUT).min(WS_SEND_TIMEOUT);
    if timeout.is_zero() {
        return false;
    }
    match tokio::time::timeout(timeout, sink.send(message)).await {
        Ok(Ok(())) => true,
        Ok(Err(_)) => {
            tracing::debug!("GraphQL WebSocket send failed");
            false
        }
        Err(_) => {
            tracing::debug!("GraphQL WebSocket send timed out");
            false
        }
    }
}

async fn close_ws<S>(sink: &mut S, code: u16, reason: &'static str) -> bool
where
    S: futures_util::Sink<Message> + Unpin,
{
    send_ws_message(
        sink,
        Message::Close(Some(CloseFrame {
            code,
            reason: reason.into(),
        })),
        None,
    )
    .await
}

async fn close_ws_for_auth_change<S>(sink: &mut S) -> bool
where
    S: futures_util::Sink<Message> + Unpin,
{
    close_ws(sink, WS_CLOSE_UNAUTHORIZED, WS_AUTHORITY_CHANGED_REASON).await
}

#[derive(Clone, Copy, Debug)]
struct WsCloseReason {
    code: u16,
    message: &'static str,
}

/// Admits operations only as the protocol engine consumes the bounded input
/// queue. A server completion is handled before the engine is polled again,
/// so cancellation and id reuse cannot race ahead of that completion.
struct WsAdmissionStream {
    incoming: ReceiverStream<String>,
    operations: Arc<std::sync::Mutex<WsOperationTracker>>,
    close_tx: tokio::sync::mpsc::Sender<WsCloseReason>,
    rejected: bool,
}

impl WsAdmissionStream {
    fn new(
        incoming: tokio::sync::mpsc::Receiver<String>,
        operations: Arc<std::sync::Mutex<WsOperationTracker>>,
        close_tx: tokio::sync::mpsc::Sender<WsCloseReason>,
    ) -> Self {
        Self {
            incoming: ReceiverStream::new(incoming),
            operations,
            close_tx,
            rejected: false,
        }
    }
}

impl futures_util::Stream for WsAdmissionStream {
    type Item = String;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if this.rejected {
            return std::task::Poll::Pending;
        }
        let text = match futures_util::Stream::poll_next(std::pin::Pin::new(&mut this.incoming), cx)
        {
            std::task::Poll::Ready(Some(text)) => text,
            std::task::Poll::Ready(None) => return std::task::Poll::Ready(None),
            std::task::Poll::Pending => return std::task::Poll::Pending,
        };
        let result = this
            .operations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .observe_client_message(&text);
        if let Err(reason) = result {
            this.rejected = true;
            // The one-slot channel has a single sender and this stream stops
            // after its first rejection. Waking the receiver lets the host send
            // the intended close frame instead of handing invalid input to the engine.
            let _ = this.close_tx.try_send(reason);
            return std::task::Poll::Pending;
        }
        std::task::Poll::Ready(Some(text))
    }
}

#[derive(Default)]
struct WsOperationTracker {
    active: std::collections::HashSet<String>,
    cancelling: std::collections::HashSet<String>,
}

impl WsOperationTracker {
    fn observe_client_message(&mut self, text: &str) -> Result<(), WsCloseReason> {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
            return Ok(());
        };
        let message_type = value.get("type").and_then(serde_json::Value::as_str);
        if matches!(message_type, Some("stop" | "complete")) {
            if let Some(id) = value.get("id").and_then(serde_json::Value::as_str)
                && self.active.contains(id)
            {
                self.cancelling.insert(id.to_string());
            }
            return Ok(());
        }
        if !matches!(message_type, Some("start" | "subscribe")) {
            return Ok(());
        }
        let id = value
            .get("id")
            .and_then(serde_json::Value::as_str)
            .filter(|id| !id.is_empty() && id.len() <= WS_MAX_OPERATION_ID_LEN)
            .ok_or(WsCloseReason {
                code: WS_CLOSE_INVALID_OPERATION,
                message: WS_INVALID_OPERATION_REASON,
            })?;
        if self.active.contains(id) {
            return Err(WsCloseReason {
                code: WS_CLOSE_DUPLICATE_OPERATION,
                message: WS_DUPLICATE_OPERATION_REASON,
            });
        }
        if self.active.len() >= WS_MAX_ACTIVE_OPERATIONS {
            return Err(WsCloseReason {
                code: WS_CLOSE_TOO_MANY_OPERATIONS,
                message: WS_TOO_MANY_OPERATIONS_REASON,
            });
        }
        self.active.insert(id.to_string());
        Ok(())
    }

    /// Release the engine's completed operation and report whether the
    /// completion was caused by a client cancellation.
    fn release(&mut self, id: &str) -> bool {
        self.active.remove(id);
        self.cancelling.remove(id)
    }
}

fn ws_should_forward_text(
    protocol: WebSocketProtocols,
    text: &str,
    operations: &mut WsOperationTracker,
) -> bool {
    let Some(id) = ws_server_complete_id(text) else {
        return true;
    };
    let cancelled_by_client = operations.release(&id);
    // graphql-transport-ws treats client completion as final; echoing the
    // engine's completion may be attributed to a new operation reusing the id.
    // The legacy subscriptions-transport-ws protocol expects this response.
    !(cancelled_by_client && protocol == WebSocketProtocols::GraphQLWS)
}

fn ws_server_complete_id(text: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(text).ok()?;
    if value.get("type").and_then(serde_json::Value::as_str) != Some("complete") {
        return None;
    }
    value
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

async fn build_ws_connection_data(
    runtime_ctx: ServerRuntimeContext,
    auth_runtime: ServerAuthRuntime,
    registry: ModuleRegistry,
    channel: Option<ChannelContext>,
    auth_lease: Arc<OnceLock<WebSocketAuthLease>>,
    payload: serde_json::Value,
) -> async_graphql::Result<Data> {
    let payload: GraphqlWsInitPayload = serde_json::from_value(payload)
        .map_err(|_| async_graphql::Error::new("Invalid connection_init payload"))?;
    let tenant_slug = payload
        .tenant_slug
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| async_graphql::Error::new("connection_init.tenantSlug is required"))?;
    let token = payload
        .token
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| async_graphql::Error::new("connection_init.token is required"))?;

    let tenant_ctx = tenant::resolve_tenant_context_by_slug(&runtime_ctx, &tenant_slug)
        .await
        .map_err(|error| {
            tracing::warn!(error = %error, "GraphQL WebSocket tenant resolution failed");
            async_graphql::Error::new(error.client_message())
        })?;
    ensure_ws_channel_tenant(channel.as_ref(), tenant_ctx.id)?;
    let access_token = token
        .trim()
        .strip_prefix("Bearer ")
        .or_else(|| token.trim().strip_prefix("bearer "))
        .unwrap_or(token.trim())
        .to_string();
    let current_user =
        resolve_current_user_from_access_token(&auth_runtime, tenant_ctx.id, &access_token)
            .await
            .map_err(|(_, message)| async_graphql::Error::new(message))?;

    let request_scope = RbacRequestScope::new(
        current_user.user.tenant_id,
        current_user.user.id,
        current_user.permissions.clone(),
        current_user.inferred_role.clone(),
    );

    let locale = resolve_ws_locale(&runtime_ctx, &tenant_ctx, payload.locale.as_deref()).await?;

    auth_lease
        .set(WebSocketAuthLease::new(access_token, &current_user))
        .map_err(|_| async_graphql::Error::new("RBAC connection scope was already initialized"))?;
    let principal_context = AuthPrincipalContext::new(current_user.principal_kind);
    let auth_ctx = AuthContext {
        user_id: current_user.user.id,
        session_id: current_user.session_id,
        tenant_id: current_user.user.tenant_id,
        permissions: graphql_permissions(current_user.permissions),
        client_id: current_user.client_id,
        scopes: current_user.scopes,
        grant_type: current_user.grant_type,
    };

    let request_context = RequestContext {
        tenant_id: tenant_ctx.id,
        user_id: Some(current_user.user.id),
        channel_id: channel.as_ref().map(|channel| channel.id),
        channel_slug: channel.as_ref().map(|channel| channel.slug.clone()),
        channel_resolution_source: channel
            .as_ref()
            .map(|channel| channel.resolution_source.clone()),
        locale: locale.to_string(),
        correlation_id: uuid::Uuid::new_v4().to_string(),
    };
    let mut data = Data::default();
    data.insert(runtime_ctx.db_clone());
    data.insert(runtime_ctx);
    data.insert(registry);
    data.insert(locale);
    data.insert(request_context);
    data.insert(tenant_ctx);
    data.insert(auth_ctx);
    data.insert(principal_context);
    data.insert(request_scope);
    if let Some(channel) = channel {
        data.insert(channel);
    }
    Ok(data)
}

fn ensure_ws_channel_tenant(
    channel: Option<&ChannelContext>,
    tenant_id: uuid::Uuid,
) -> async_graphql::Result<()> {
    if let Some(channel) = channel
        && channel.tenant_id != tenant_id
    {
        return Err(async_graphql::Error::new(
            "connection_init channel does not belong to the resolved tenant",
        ));
    }
    Ok(())
}

async fn resolve_ws_locale(
    runtime_ctx: &ServerRuntimeContext,
    tenant_ctx: &TenantContext,
    requested_locale: Option<&str>,
) -> Result<Locale, async_graphql::Error> {
    let service = TenantService::new(runtime_ctx.db_clone());
    let context = PortContext::new(
        tenant_ctx.id.to_string(),
        PortActor::service("rustok-server.graphql-ws-locale"),
        PLATFORM_FALLBACK_LOCALE,
        format!("graphql-ws-locale:{}", tenant_ctx.id),
    )
    .with_deadline(std::time::Duration::from_secs(2));

    let policy = service
        .read_locale_policy(context)
        .await
        .map_err(|_| async_graphql::Error::new("Tenant locale policy unavailable"))?;

    let requested = requested_locale.and_then(rustok_api::normalize_locale_tag);
    let locale = requested
        .as_deref()
        .and_then(|value| {
            policy
                .locales
                .iter()
                .find(|entry| entry.is_enabled && entry.locale.as_str() == value)
                .map(|entry| entry.locale.as_str().to_string())
                .or_else(|| {
                    policy
                        .locales
                        .iter()
                        .find(|entry| entry.locale.as_str() == value)
                        .and_then(|entry| entry.fallback_locale.as_ref())
                        .and_then(|fallback| {
                            policy
                                .locales
                                .iter()
                                .find(|entry| {
                                    entry.is_enabled && entry.locale.as_str() == fallback.as_str()
                                })
                                .map(|entry| entry.locale.as_str().to_string())
                        })
                })
        })
        .or_else(|| {
            policy
                .locales
                .iter()
                .find(|entry| entry.is_default && entry.is_enabled)
                .map(|entry| entry.locale.as_str().to_string())
        })
        .or_else(|| {
            policy
                .locales
                .iter()
                .find(|entry| {
                    entry.is_enabled && entry.locale.as_str() == tenant_ctx.default_locale.as_str()
                })
                .map(|entry| entry.locale.as_str().to_string())
        })
        .or_else(|| {
            policy
                .locales
                .iter()
                .find(|entry| entry.is_enabled)
                .map(|entry| entry.locale.as_str().to_string())
        })
        .unwrap_or_else(|| {
            rustok_api::normalize_locale_tag(tenant_ctx.default_locale.as_str())
                .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string())
        });

    Locale::parse(&locale).ok_or_else(|| async_graphql::Error::new("Invalid tenant locale policy"))
}

const GRAPHQL_HTTP_PATH: &str = "/api/graphql";

pub fn router() -> crate::routes::ServerRouter {
    axum::Router::new()
        .route(
            GRAPHQL_HTTP_PATH,
            get(graphql_playground).post(graphql_handler),
        )
        .route("/api/graphql/schema.graphql", get(graphql_schema_sdl))
        .route("/api/graphql/ws", get(graphql_ws_handler))
}

#[cfg(test)]
mod tests {
    use super::{
        GRAPHQL_HTTP_PATH, WS_CONNECTION_INIT_TIMEOUT, WS_CONNECTION_INIT_TIMEOUT_CLOSE,
        WS_CONNECTION_INIT_TIMEOUT_REASON, WS_INCOMING_QUEUE_CAPACITY, WS_MAX_FRAME_SIZE,
        WS_MAX_MESSAGE_SIZE, graphql_http_response, graphql_permissions,
    };
    use crate::{
        common::settings::RustokSettings, middleware::tenant,
        services::server_runtime_context::ServerRuntimeContext,
    };
    use async_graphql::http::WebSocketProtocols;
    use axum::http::{HeaderMap, StatusCode, header};
    use rustok_api::{Permission, Resource};
    use rustok_cache::CacheService;
    use rustok_migrations::SqliteTestMigrator as Migrator;
    use sea_orm::{ActiveModelTrait, Set};
    use serial_test::serial;

    #[test]
    fn persisted_query_telemetry_accepts_only_sha256_hex_identifiers() {
        let valid = serde_json::json!({
            "persistedQuery": {
                "version": 1,
                "sha256Hash": "a".repeat(64),
            }
        });
        let request = async_graphql::Request::new("{ __typename }");
        let mut request = request;
        request.extensions = async_graphql::Extensions::default();
        request.extensions.insert(
            "persistedQuery".to_string(),
            async_graphql::Value::from_json(valid["persistedQuery"].clone()).expect("json value"),
        );

        assert_eq!(
            super::persisted_query_hash(&request),
            Some("a".repeat(64).as_str())
        );

        let mut request = async_graphql::Request::new("{ __typename }");
        request.extensions = async_graphql::Extensions::default();
        request.extensions.insert(
            "persistedQuery".to_string(),
            async_graphql::Value::from_json(serde_json::json!({
                "version": 1,
                "sha256Hash": "b".repeat(65)
            }))
            .expect("json value"),
        );
        assert!(super::persisted_query_hash(&request).is_none());
    }

    #[test]
    fn graphql_ws_protocols_are_not_defaulted_without_negotiation() {
        let protocols = async_graphql::http::ALL_WEBSOCKET_PROTOCOLS;
        assert!(
            protocols
                .iter()
                .all(|protocol| protocol.parse::<WebSocketProtocols>().is_ok()),
            "all advertised GraphQL websocket protocols must be parseable"
        );
    }

    #[test]
    fn graphql_ws_transport_limits_are_bounded() {
        assert_eq!(WS_MAX_MESSAGE_SIZE, 256 * 1024);
        assert_eq!(WS_MAX_FRAME_SIZE, 256 * 1024);
        assert_eq!(WS_INCOMING_QUEUE_CAPACITY, 32);
        assert_eq!(super::WS_MAX_ACTIVE_OPERATIONS, 32);
        assert_eq!(super::WS_MAX_OPERATION_ID_LEN, 128);
        assert_eq!(
            super::WS_AUTH_REVALIDATION_INTERVAL,
            std::time::Duration::from_secs(30)
        );
    }

    #[test]
    fn graphql_ws_operation_tracker_bounds_active_ids_and_rejects_duplicates() {
        let mut tracker = super::WsOperationTracker::default();
        for index in 0..super::WS_MAX_ACTIVE_OPERATIONS {
            let message = serde_json::json!({ "type": "subscribe", "id": index.to_string() });
            tracker
                .observe_client_message(&message.to_string())
                .expect("operation within cap");
        }

        let duplicate = serde_json::json!({ "type": "subscribe", "id": "0" }).to_string();
        let error = tracker
            .observe_client_message(&duplicate)
            .expect_err("duplicate operation id must be rejected");
        assert_eq!(error.code, super::WS_CLOSE_DUPLICATE_OPERATION);

        let overflow = serde_json::json!({ "type": "start", "id": "overflow" }).to_string();
        let error = tracker
            .observe_client_message(&overflow)
            .expect_err("operation cap must be enforced");
        assert_eq!(error.code, super::WS_CLOSE_TOO_MANY_OPERATIONS);

        tracker.release("0");
        tracker
            .observe_client_message(&overflow)
            .expect("released slot is reusable");
    }

    #[test]
    fn graphql_ws_operation_tracker_validates_ids_and_ignores_non_start_messages() {
        let mut tracker = super::WsOperationTracker::default();
        for message in [
            serde_json::json!({ "type": "connection_init" }).to_string(),
            serde_json::json!({ "type": "ping" }).to_string(),
            "not json".to_string(),
        ] {
            tracker
                .observe_client_message(&message)
                .expect("non-start messages are forwarded to the protocol engine");
        }

        let missing_id = serde_json::json!({ "type": "subscribe" }).to_string();
        assert_eq!(
            tracker
                .observe_client_message(&missing_id)
                .expect_err("missing id")
                .code,
            super::WS_CLOSE_INVALID_OPERATION
        );
        let long_id = serde_json::json!({
            "type": "start",
            "id": "a".repeat(super::WS_MAX_OPERATION_ID_LEN + 1),
        })
        .to_string();
        assert_eq!(
            tracker.observe_client_message(&long_id).expect_err("long id").code,
            super::WS_CLOSE_INVALID_OPERATION
        );
    }

    #[test]
    fn graphql_ws_server_complete_releases_operation_slot() {
        let complete = serde_json::json!({ "type": "complete", "id": "op-1" }).to_string();
        assert_eq!(super::ws_server_complete_id(&complete), Some("op-1".to_string()));
        let next = serde_json::json!({ "type": "next", "id": "op-1" }).to_string();
        assert_eq!(super::ws_server_complete_id(&next), None);

        let mut tracker = super::WsOperationTracker::default();
        let start = serde_json::json!({ "type": "subscribe", "id": "op-1" }).to_string();
        tracker.observe_client_message(&start).expect("start");
        if let Some(id) = super::ws_server_complete_id(&complete) {
            tracker.release(&id);
        }
        tracker.observe_client_message(&start).expect("id is reusable after complete");
    }

    #[test]
    fn graphql_ws_client_cancellation_completion_is_protocol_specific() {
        for (protocol, start, stop, forward_cancelled) in [
            (
                WebSocketProtocols::GraphQLWS,
                "subscribe",
                "complete",
                false,
            ),
            (
                WebSocketProtocols::SubscriptionsTransportWS,
                "start",
                "stop",
                true,
            ),
        ] {
            let mut tracker = super::WsOperationTracker::default();
            let start = serde_json::json!({"type": start, "id": "op"}).to_string();
            let stop = serde_json::json!({"type": stop, "id": "op"}).to_string();
            let complete = serde_json::json!({"type": "complete", "id": "op"}).to_string();
            let next = serde_json::json!({"type": "next", "id": "op"}).to_string();
            // Stopping an unknown ID must not mark a future operation as cancelled.
            tracker.observe_client_message(&stop).unwrap();
            tracker.observe_client_message(&start).unwrap();
            assert!(super::ws_should_forward_text(protocol, &next, &mut tracker));
            assert!(super::ws_should_forward_text(
                protocol,
                &complete,
                &mut tracker
            ));
            tracker.observe_client_message(&start).unwrap();
            tracker.observe_client_message(&stop).unwrap();
            assert_eq!(
                super::ws_should_forward_text(protocol, &complete, &mut tracker),
                forward_cancelled
            );
            tracker.observe_client_message(&start).unwrap();
            assert!(tracker.active.contains("op"));
            assert!(tracker.cancelling.is_empty());
        }
    }

    #[tokio::test]
    async fn graphql_ws_admission_follows_protocol_cancellation_order() {
        use async_graphql::{EmptyMutation, Object, Schema, Subscription};
        use futures_util::StreamExt;
        use std::sync::{Arc, Mutex};
        use std::time::Duration;

        struct Query;
        #[Object]
        impl Query {
            async fn ping(&self) -> bool {
                true
            }
        }
        struct Subscriptions;
        #[Subscription]
        impl Subscriptions {
            async fn pending_value(&self) -> impl futures_util::Stream<Item = i32> {
                futures_util::stream::pending()
            }
        }

        for (protocol, start, stop) in [
            (WebSocketProtocols::GraphQLWS, "subscribe", "complete"),
            (
                WebSocketProtocols::SubscriptionsTransportWS,
                "start",
                "stop",
            ),
        ] {
            let schema = Schema::build(Query, EmptyMutation, Subscriptions).finish();
            let (incoming_tx, incoming_rx) = tokio::sync::mpsc::channel(128);
            let (close_tx, mut close_rx) = tokio::sync::mpsc::channel(1);
            let operations = Arc::new(Mutex::new(super::WsOperationTracker::default()));
            let incoming =
                super::WsAdmissionStream::new(incoming_rx, Arc::clone(&operations), close_tx);
            let mut stream = async_graphql::http::WebSocket::new(schema, incoming, protocol);
            incoming_tx
                .send(serde_json::json!({"type": "connection_init"}).to_string())
                .await
                .unwrap();
            let ack = tokio::time::timeout(Duration::from_secs(2), stream.next())
                .await
                .unwrap();
            assert!(
                matches!(ack, Some(async_graphql::http::WsMessage::Text(text)) if text.contains("connection_ack"))
            );

            // All input is queued before the protocol stream is polled. Every
            // cancellation must complete before the next id is admitted.
            for index in 0..40 {
                let id = format!("op-{index}");
                incoming_tx.send(serde_json::json!({"type": start, "id": id, "payload": {"query": "subscription { pendingValue }"}}).to_string()).await.unwrap();
                incoming_tx
                    .send(serde_json::json!({"type": stop, "id": id}).to_string())
                    .await
                    .unwrap();
            }
            for index in 0..40 {
                let message = tokio::time::timeout(Duration::from_secs(2), stream.next())
                    .await
                    .unwrap();
                let Some(async_graphql::http::WsMessage::Text(text)) = message else {
                    panic!("expected completion")
                };
                let id = super::ws_server_complete_id(&text).expect("complete message");
                assert_eq!(id, format!("op-{index}"));
                assert_eq!(
                    super::ws_should_forward_text(protocol, &text, &mut operations.lock().unwrap()),
                    protocol == WebSocketProtocols::SubscriptionsTransportWS
                );
            }
            assert!(close_rx.try_recv().is_err());

            // A natural completion may race an already queued cancellation.
            // Once its completion has been observed, a redundant stop must not
            // prevent another operation from reusing that id.
            incoming_tx.send(serde_json::json!({"type": start, "id": "natural", "payload": {"query": "{ ping }"}}).to_string()).await.unwrap();
            let response = tokio::time::timeout(Duration::from_secs(2), stream.next())
                .await
                .unwrap();
            assert!(
                matches!(response, Some(async_graphql::http::WsMessage::Text(text)) if !text.contains("complete"))
            );
            let completion = tokio::time::timeout(Duration::from_secs(2), stream.next())
                .await
                .unwrap();
            let Some(async_graphql::http::WsMessage::Text(text)) = completion else {
                panic!("expected natural completion")
            };
            let id = super::ws_server_complete_id(&text).unwrap();
            assert_eq!(id, "natural");
            assert!(super::ws_should_forward_text(
                protocol,
                &text,
                &mut operations.lock().unwrap()
            ));
            incoming_tx
                .send(serde_json::json!({"type": stop, "id": "natural"}).to_string())
                .await
                .unwrap();
            incoming_tx.send(serde_json::json!({"type": start, "id": "natural", "payload": {"query": "{ ping }"}}).to_string()).await.unwrap();
            let response = tokio::time::timeout(Duration::from_secs(2), stream.next())
                .await
                .unwrap();
            assert!(
                matches!(response, Some(async_graphql::http::WsMessage::Text(text)) if !text.contains("complete"))
            );
            let completion = tokio::time::timeout(Duration::from_secs(2), stream.next())
                .await
                .unwrap();
            let Some(async_graphql::http::WsMessage::Text(text)) = completion else {
                panic!("expected reused natural completion")
            };
            let id = super::ws_server_complete_id(&text).unwrap();
            assert_eq!(id, "natural");
            assert!(super::ws_should_forward_text(
                protocol,
                &text,
                &mut operations.lock().unwrap()
            ));

            // Cancel between the query's response and its natural completion,
            // then reuse the id before the engine processes that cancellation.
            let race_query =
                serde_json::json!({"type": start, "id": "race", "payload": {"query": "{ ping }"}})
                    .to_string();
            incoming_tx.send(race_query.clone()).await.unwrap();
            let response = tokio::time::timeout(Duration::from_secs(2), stream.next())
                .await
                .unwrap();
            assert!(
                matches!(response, Some(async_graphql::http::WsMessage::Text(text)) if !text.contains("complete"))
            );
            incoming_tx
                .send(serde_json::json!({"type": stop, "id": "race"}).to_string())
                .await
                .unwrap();
            incoming_tx.send(race_query).await.unwrap();
            let completion = tokio::time::timeout(Duration::from_secs(2), stream.next())
                .await
                .unwrap();
            let Some(async_graphql::http::WsMessage::Text(text)) = completion else {
                panic!("expected cancelled completion")
            };
            let id = super::ws_server_complete_id(&text).unwrap();
            assert_eq!(id, "race");
            assert_eq!(
                super::ws_should_forward_text(protocol, &text, &mut operations.lock().unwrap()),
                protocol == WebSocketProtocols::SubscriptionsTransportWS
            );
            let response = tokio::time::timeout(Duration::from_secs(2), stream.next())
                .await
                .unwrap();
            assert!(
                matches!(response, Some(async_graphql::http::WsMessage::Text(text)) if !text.contains("complete"))
            );
            let completion = tokio::time::timeout(Duration::from_secs(2), stream.next())
                .await
                .unwrap();
            let Some(async_graphql::http::WsMessage::Text(text)) = completion else {
                panic!("expected reused race completion")
            };
            let id = super::ws_server_complete_id(&text).unwrap();
            assert_eq!(id, "race");
            assert!(super::ws_should_forward_text(
                protocol,
                &text,
                &mut operations.lock().unwrap()
            ));

            let start_message = serde_json::json!({"type": start, "id": "reused", "payload": {"query": "subscription { pendingValue }"}}).to_string();
            let stop_message = serde_json::json!({"type": stop, "id": "reused"}).to_string();
            incoming_tx.send(start_message.clone()).await.unwrap();
            incoming_tx.send(stop_message.clone()).await.unwrap();
            incoming_tx.send(start_message.clone()).await.unwrap();
            let old = tokio::time::timeout(Duration::from_secs(2), stream.next())
                .await
                .unwrap();
            let Some(async_graphql::http::WsMessage::Text(text)) = old else {
                panic!("expected old completion")
            };
            let id = super::ws_server_complete_id(&text).unwrap();
            assert_eq!(id, "reused");
            assert_eq!(
                super::ws_should_forward_text(protocol, &text, &mut operations.lock().unwrap()),
                protocol == WebSocketProtocols::SubscriptionsTransportWS
            );
            // Polling again admits the replacement subscription; the old
            // completion has already been accounted for, so it remains active.
            assert!(
                tokio::time::timeout(Duration::from_millis(20), stream.next())
                    .await
                    .is_err()
            );
            assert!(operations.lock().unwrap().active.contains("reused"));
            assert!(close_rx.try_recv().is_err());
            incoming_tx.send(start_message).await.unwrap();
            let reason = tokio::time::timeout(Duration::from_secs(2), async {
                tokio::select! {
                    reason = close_rx.recv() => reason.expect("duplicate close reason"),
                    message = stream.next() => panic!("unexpected protocol output: {message:?}"),
                }
            })
            .await
            .unwrap();
            assert_eq!(reason.code, super::WS_CLOSE_DUPLICATE_OPERATION);
        }
    }

    #[tokio::test]
    async fn graphql_ws_admission_rejects_overflow_before_protocol_consumes_it() {
        use async_graphql::{EmptyMutation, Object, Schema, Subscription};
        use futures_util::StreamExt;
        use std::sync::{Arc, Mutex};
        use std::time::Duration;

        struct Query;
        #[Object]
        impl Query {
            async fn ping(&self) -> bool {
                true
            }
        }
        struct Subscriptions;
        #[Subscription]
        impl Subscriptions {
            async fn pending_value(&self) -> impl futures_util::Stream<Item = i32> {
                futures_util::stream::pending()
            }
        }
        for (protocol, start) in [
            (WebSocketProtocols::GraphQLWS, "subscribe"),
            (WebSocketProtocols::SubscriptionsTransportWS, "start"),
        ] {
            let schema = Schema::build(Query, EmptyMutation, Subscriptions).finish();
            let (incoming_tx, incoming_rx) = tokio::sync::mpsc::channel(64);
            let (close_tx, mut close_rx) = tokio::sync::mpsc::channel(1);
            let operations = Arc::new(Mutex::new(super::WsOperationTracker::default()));
            let incoming =
                super::WsAdmissionStream::new(incoming_rx, Arc::clone(&operations), close_tx);
            let mut stream = async_graphql::http::WebSocket::new(schema, incoming, protocol);
            incoming_tx
                .send(serde_json::json!({"type": "connection_init"}).to_string())
                .await
                .unwrap();
            tokio::time::timeout(Duration::from_secs(2), stream.next())
                .await
                .unwrap()
                .expect("ack");
            for index in 0..=super::WS_MAX_ACTIVE_OPERATIONS {
                incoming_tx.send(serde_json::json!({"type": start, "id": index.to_string(), "payload": {"query": "subscription { pendingValue }"}}).to_string()).await.unwrap();
            }
            let reason = tokio::time::timeout(Duration::from_secs(2), async {
                tokio::select! {
                    reason = close_rx.recv() => reason.expect("overflow close reason"),
                    message = stream.next() => panic!("unexpected protocol output: {message:?}"),
                }
            })
            .await
            .unwrap();
            assert_eq!(reason.code, super::WS_CLOSE_TOO_MANY_OPERATIONS);
            let tracker = operations.lock().unwrap();
            assert_eq!(tracker.active.len(), super::WS_MAX_ACTIVE_OPERATIONS);
            assert!(
                !tracker
                    .active
                    .contains(&super::WS_MAX_ACTIVE_OPERATIONS.to_string())
            );
        }
    }

    #[tokio::test]
    async fn graphql_ws_slow_received_init_does_not_timeout_and_can_finish() {
        use async_graphql::{Data, EmptyMutation, EmptySubscription, Object, Schema};
        use futures_util::StreamExt;
        use std::sync::{Arc, OnceLock};
        use std::time::Duration;
        use tokio_stream::wrappers::ReceiverStream;

        struct Query;
        #[Object]
        impl Query {
            async fn ping(&self) -> bool {
                true
            }
        }

        let schema = Schema::build(Query, EmptyMutation, EmptySubscription).finish();
        let (incoming_tx, incoming_rx) = tokio::sync::mpsc::channel::<String>(1);
        incoming_tx
            .send(r#"{"type":"connection_init"}"#.to_string())
            .await
            .unwrap();
        let received = Arc::new(OnceLock::new());
        let received_for_init = Arc::clone(&received);
        let (finish_init, wait_for_init) = tokio::sync::oneshot::channel::<()>();
        let mut stream = async_graphql::http::WebSocket::new(
            schema,
            ReceiverStream::new(incoming_rx),
            WebSocketProtocols::GraphQLWS,
        )
        .on_connection_init(move |_| {
            received_for_init
                .set(())
                .expect("init callback called once");
            async move {
                wait_for_init.await.expect("initializer released");
                Ok(Data::default())
            }
        });
        let mut deadline = Box::pin(tokio::time::sleep(Duration::from_millis(20)));
        let timed_out = tokio::select! {
            biased;
            message = stream.next() => panic!("initializer must remain pending: {message:?}"),
            timed_out = super::connection_init_timed_out(deadline.as_mut(), &received) => timed_out,
        };
        assert!(
            !timed_out,
            "an accepted init must not time out during validation"
        );
        finish_init.send(()).unwrap();
        let message = tokio::time::timeout(Duration::from_secs(2), stream.next())
            .await
            .expect("initializer must resume after the deadline");
        assert!(
            matches!(message, Some(async_graphql::http::WsMessage::Text(text)) if text.contains("connection_ack"))
        );

        let absent = OnceLock::new();
        let mut deadline = Box::pin(tokio::time::sleep(Duration::from_millis(1)));
        assert!(super::connection_init_timed_out(deadline.as_mut(), &absent).await);
    }

    #[tokio::test]
    async fn graphql_ws_init_validation_is_bounded() {
        use std::time::Duration;

        let result = super::bounded_ws_connection_init(
            std::future::pending::<async_graphql::Result<async_graphql::Data>>(),
            Duration::from_millis(20),
        )
        .await;
        assert!(
            matches!(result, Err(error) if error.message == "Connection initialization timed out")
        );
        assert!(
            super::bounded_ws_connection_init(
                std::future::ready(Ok(async_graphql::Data::default())),
                Duration::from_secs(1),
            )
            .await
            .is_ok()
        );
    }

    #[tokio::test]
    async fn graphql_ws_send_is_bounded_by_expiry_and_backpressure() {
        use axum::extract::ws::Message;
        use std::convert::Infallible;
        use std::pin::Pin;
        use std::task::{Context, Poll};
        use std::time::Duration;

        struct BlockedSink;
        impl futures_util::Sink<Message> for BlockedSink {
            type Error = Infallible;

            fn poll_ready(
                self: Pin<&mut Self>,
                _: &mut Context<'_>,
            ) -> Poll<Result<(), Self::Error>> {
                Poll::Pending
            }
            fn start_send(self: Pin<&mut Self>, _: Message) -> Result<(), Self::Error> {
                panic!("a blocked sink cannot accept a message")
            }
            fn poll_flush(
                self: Pin<&mut Self>,
                _: &mut Context<'_>,
            ) -> Poll<Result<(), Self::Error>> {
                Poll::Pending
            }
            fn poll_close(
                self: Pin<&mut Self>,
                _: &mut Context<'_>,
            ) -> Poll<Result<(), Self::Error>> {
                Poll::Pending
            }
        }

        let mut ready = futures_util::sink::drain::<Message>();
        assert!(
            !super::send_ws_message(
                &mut ready,
                Message::Text("expired".into()),
                Some(Duration::ZERO),
            )
            .await
        );
        assert!(
            super::send_ws_message(
                &mut ready,
                Message::Text("valid".into()),
                Some(Duration::from_secs(1)),
            )
            .await
        );
        let mut blocked = BlockedSink;
        let bounded = tokio::time::timeout(
            Duration::from_secs(1),
            super::send_ws_message(
                &mut blocked,
                Message::Text("blocked".into()),
                Some(Duration::from_millis(20)),
            ),
        )
        .await
        .expect("send must not block indefinitely");
        assert!(!bounded);
    }

    #[test]
    fn graphql_ws_connection_init_wait_is_bounded_and_protocol_compliant() {
        assert_eq!(
            WS_CONNECTION_INIT_TIMEOUT,
            std::time::Duration::from_secs(10)
        );
        assert_eq!(WS_CONNECTION_INIT_TIMEOUT_CLOSE, 4408);
        assert_eq!(
            WS_CONNECTION_INIT_TIMEOUT_REASON,
            "Connection initialisation timeout"
        );
    }

    #[test]
    fn websocket_tenant_resolution_does_not_log_raw_slug_payload() {
        let source = include_str!("graphql.rs");
        let production_source = source.split("#[cfg(test)]").next().unwrap_or(source);
        let warn_pattern = format!("{}(tenant_slug", "tracing::warn!");
        let error_pattern = format!("{}(tenant_slug", "tracing::error!");
        let info_pattern = format!("{}(tenant_slug", "tracing::info!");
        let debug_pattern = format!("{}(tenant_slug", "tracing::debug!");
        assert!(!production_source.contains(&warn_pattern));
        assert!(!production_source.contains(&error_pattern));
        assert!(!production_source.contains(&info_pattern));
        assert!(!production_source.contains(&debug_pattern));
    }

    #[test]
    fn graphql_router_uses_the_canonical_http_path() {
        assert_eq!(GRAPHQL_HTTP_PATH, "/api/graphql");
    }

    #[test]
    fn graphql_json_rejection_messages_are_stable_and_non_internal() {
        assert_eq!(
            super::graphql_json_rejection_message(StatusCode::PAYLOAD_TOO_LARGE),
            "GraphQL request body is too large"
        );
        assert_eq!(
            super::graphql_json_rejection_message(StatusCode::BAD_REQUEST),
            "Invalid GraphQL JSON request"
        );
        assert_eq!(
            super::graphql_json_rejection_message(StatusCode::UNPROCESSABLE_ENTITY),
            "Invalid GraphQL JSON request"
        );
    }

    #[test]
    fn graphql_http_response_preserves_extension_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(header::RETRY_AFTER, "23".parse().expect("valid header"));
        let graphql_response =
            async_graphql::Response::new(async_graphql::Value::Null).http_headers(headers);

        let http_response = graphql_http_response(graphql_response);

        assert_eq!(
            http_response
                .headers()
                .get(header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok()),
            Some("23")
        );
        assert_eq!(
            http_response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/json")
        );
    }

    #[tokio::test]
    #[serial]
    async fn graphql_ws_tenant_handshake_fails_closed() {
        let db = rustok_test_utils::db::setup_test_db_with_migrations::<Migrator>().await;
        let runtime = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
        tenant::init_tenant_cache_infrastructure(&runtime, &CacheService::from_url(None))
            .await
            .expect("initialize tenant cache");

        let malformed = match tenant::resolve_tenant_context_by_slug(&runtime, "../../other").await
        {
            Ok(_) => panic!("malformed WebSocket tenant slug must be rejected"),
            Err(error) => error,
        };
        assert_eq!(malformed.client_message(), "Invalid tenant identifier");

        let unknown =
            match tenant::resolve_tenant_context_by_slug(&runtime, "missing-ws-tenant").await {
                Ok(_) => panic!("unknown WebSocket tenant must be rejected"),
                Err(error) => error,
            };
        assert_eq!(unknown.client_message(), "Tenant not found");

        let now = chrono::Utc::now();
        rustok_tenant::entities::tenant::ActiveModel {
            id: Set(uuid::Uuid::new_v4()),
            name: Set("Disabled WS tenant".to_string()),
            slug: Set("disabled-ws-tenant".to_string()),
            domain: Set(None),
            settings: Set(serde_json::json!({})),
            default_locale: Set("en".to_string()),
            is_active: Set(false),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&db)
        .await
        .expect("disabled tenant should insert");

        let disabled =
            match tenant::resolve_tenant_context_by_slug(&runtime, "disabled-ws-tenant").await {
                Ok(_) => panic!("disabled WebSocket tenant must be rejected"),
                Err(error) => error,
            };
        assert_eq!(disabled.client_message(), "Tenant is disabled");
    }

    #[test]
    fn websocket_auth_lease_is_published_after_fallible_locale_resolution() {
        let source = include_str!("graphql.rs");
        let locale_call = source
            .find("let locale = resolve_ws_locale")
            .expect("locale resolution call should exist");
        let lease_set = source
            .find(
                "auth_lease
        .set(WebSocketAuthLease::new",
            )
            .expect("auth lease publication should exist");
        assert!(
            locale_call < lease_set,
            "auth lease must not be published before locale policy resolution"
        );
    }

    #[test]
    fn websocket_locale_selection_prefers_enabled_requested_locale() {
        let policy = rustok_tenant::TenantLocalePolicyProjection {
            tenant_id: uuid::Uuid::new_v4(),
            revision: 1,
            default_locale: rustok_api::TenantLocale::new("en").expect("valid locale"),
            locales: vec![
                rustok_tenant::TenantLocalePolicyEntry {
                    locale: rustok_api::TenantLocale::new("en").expect("valid locale"),
                    name: "English".to_string(),
                    native_name: "English".to_string(),
                    is_default: true,
                    is_enabled: true,
                    fallback_locale: None,
                },
                rustok_tenant::TenantLocalePolicyEntry {
                    locale: rustok_api::TenantLocale::new("de-DE").expect("valid locale"),
                    name: "German".to_string(),
                    native_name: "Deutsch".to_string(),
                    is_default: false,
                    is_enabled: false,
                    fallback_locale: Some(
                        rustok_api::TenantLocale::new("en").expect("valid locale"),
                    ),
                },
            ],
        };

        let select = |requested: Option<&str>| {
            let requested = requested.and_then(rustok_api::normalize_locale_tag);
            requested
                .as_deref()
                .and_then(|value| {
                    policy
                        .locales
                        .iter()
                        .find(|entry| entry.is_enabled && entry.locale.as_str() == value)
                        .map(|entry| entry.locale.as_str().to_string())
                })
                .unwrap_or_else(|| policy.default_locale.as_str().to_string())
        };

        assert_eq!(select(Some("en")), "en");
        assert_eq!(select(Some("de-DE")), "en");
        assert_eq!(select(Some("fr")), "en");
    }

    #[test]
    fn websocket_channel_must_belong_to_the_resolved_tenant() {
        let tenant_id = uuid::Uuid::new_v4();
        let mut channel = rustok_api::ChannelContext {
            id: uuid::Uuid::new_v4(),
            tenant_id,
            slug: "web".to_string(),
            name: "Web".to_string(),
            is_active: true,
            status: "active".to_string(),
            target_type: None,
            target_value: None,
            settings: serde_json::json!({}),
            resolution_source: rustok_api::ChannelResolutionSource::Default,
            resolution_trace: Vec::new(),
        };
        assert!(super::ensure_ws_channel_tenant(Some(&channel), tenant_id).is_ok());
        assert!(super::ensure_ws_channel_tenant(None, tenant_id).is_ok());
        channel.tenant_id = uuid::Uuid::new_v4();
        assert!(super::ensure_ws_channel_tenant(Some(&channel), tenant_id).is_err());
    }

    #[test]
    fn manage_implies_graphql_read_and_list_without_widening_other_resources() {
        let permissions = graphql_permissions(vec![Permission::USERS_MANAGE]);

        assert!(permissions.contains(&Permission::USERS_MANAGE));
        assert!(permissions.contains(&Permission::USERS_READ));
        assert!(permissions.contains(&Permission::USERS_LIST));
        assert!(!permissions.contains(&Permission::new(
            Resource::Settings,
            rustok_api::Action::Read,
        )));
    }

    #[test]
    fn concrete_permissions_remain_stable_and_deduplicated() {
        let permissions = graphql_permissions(vec![
            Permission::USERS_READ,
            Permission::USERS_READ,
            Permission::USERS_LIST,
        ]);

        assert_eq!(
            permissions
                .iter()
                .filter(|permission| **permission == Permission::USERS_READ)
                .count(),
            1
        );
        assert_eq!(
            permissions
                .iter()
                .filter(|permission| **permission == Permission::USERS_LIST)
                .count(),
            1
        );
    }
}
