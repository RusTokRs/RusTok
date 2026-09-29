use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};

use async_graphql::Data;
use async_graphql::http::{GraphQLPlaygroundConfig, WebSocketProtocols, WsMessage};
use axum::{
    Extension, Json,
    extract::{
        rejection::JsonRejection,
        State,
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use futures_util::{SinkExt, StreamExt};
use rustok_api::{
    Action, AuthPrincipalContext, Permission, PLATFORM_FALLBACK_LOCALE, PortActor, PortContext,
};

use rustok_core::i18n::Locale;
use rustok_tenant::{TenantLocalePolicyPort, TenantService};
use tokio_stream::wrappers::ReceiverStream;

use crate::common::RequestContext;
use crate::context::{AuthContext, TenantContext};
use crate::extractors::auth::{OptionalCurrentUser, resolve_current_user_from_access_token};
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
const WS_CONNECTION_INIT_TIMEOUT_CLOSE: u16 = 4408;
const WS_CONNECTION_INIT_TIMEOUT_REASON: &str = "Connection initialisation timeout";

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

#[derive(Clone)]
struct GraphqlWsAuthLease {
    tenant_id: uuid::Uuid,
    access_token: String,
    initial_scope: RbacRequestScope,
}

#[allow(clippy::too_many_arguments)]
async fn graphql_handler(
    State(runtime_ctx): State<ServerRuntimeContext>,
    Extension(registry): Extension<ModuleRegistry>,
    Extension(schema): Extension<Arc<AppSchema>>,
    tenant_ctx: TenantContext,
    request_context: RequestContext,
    OptionalCurrentUser(current_user): OptionalCurrentUser,
    headers: HeaderMap,
    json_request: Result<Json<async_graphql::Request>, JsonRejection>,
) -> Response {
    let Json(req) = match json_request {
        Ok(request) => request,
        Err(rejection) => return graphql_json_rejection_response(rejection),
    };

    let db = runtime_ctx.db_clone();
    let locale = Locale::parse(&request_context.locale).unwrap_or_default();
    if let Some(hash) = persisted_query_hash(&req) {
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
    axum::response::Html(async_graphql::http::playground_source(
        GraphQLPlaygroundConfig::new("/api/graphql").subscription_endpoint("/api/graphql/ws"),
    ))
}

async fn graphql_schema_sdl(Extension(schema): Extension<Arc<AppSchema>>) -> impl IntoResponse {
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
) -> impl IntoResponse {
    let ws = ws
        .protocols(async_graphql::http::ALL_WEBSOCKET_PROTOCOLS)
        .max_message_size(WS_MAX_MESSAGE_SIZE)
        .max_frame_size(WS_MAX_FRAME_SIZE);
    let Some(protocol) = ws
        .selected_protocol()
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<WebSocketProtocols>().ok())
    else {
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
) {
    let (mut sink, mut source) = socket.split();
    let (incoming_tx, incoming_rx) =
        tokio::sync::mpsc::channel::<String>(WS_INCOMING_QUEUE_CAPACITY);
    let auth_lease = Arc::new(OnceLock::<GraphqlWsAuthLease>::new());
    let connection_init_received = Arc::new(OnceLock::<()>::new());

    let schema_for_stream = schema.as_ref().clone();
    let runtime_ctx_for_init = runtime_ctx.clone();
    let auth_runtime_for_init = auth_runtime.clone();
    let registry_for_init = registry.clone();
    let auth_lease_for_init = Arc::clone(&auth_lease);
    let connection_init_received_for_init = Arc::clone(&connection_init_received);
    let mut graphql_stream = async_graphql::http::WebSocket::new(
        schema_for_stream,
        ReceiverStream::new(incoming_rx),
        protocol,
    )
    .on_connection_init(move |payload| {
        let _ = connection_init_received_for_init.set(());
        build_ws_connection_data(
            runtime_ctx_for_init.clone(),
            auth_runtime_for_init.clone(),
            registry_for_init.clone(),
            Arc::clone(&auth_lease_for_init),
            payload,
        )
    });

    let forward_incoming = tokio::spawn(async move {
        while let Some(message) = source.next().await {
            match message {
                Ok(Message::Text(text)) => {
                    if incoming_tx.send(text.to_string()).await.is_err() {
                        break;
                    }
                }
                Ok(Message::Binary(bytes)) => {
                    let Ok(text) = String::from_utf8(bytes.to_vec()) else {
                        continue;
                    };
                    if incoming_tx.send(text).await.is_err() {
                        break;
                    }
                }
                Ok(Message::Close(_)) => break,
                Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {}
                Err(_) => break,
            }
        }
    });

    let mut connection_init_deadline = Box::pin(tokio::time::sleep(WS_CONNECTION_INIT_TIMEOUT));

    loop {
        let scope_before_poll = match auth_lease.get() {
            Some(lease) => match revalidate_ws_auth(&auth_runtime, lease).await {
                Ok(scope) => Some(scope),
                Err(()) => {
                    let _ = close_ws_for_auth_change(&mut sink).await;
                    break;
                }
            },
            None => None,
        };

        let next_message = if auth_lease.get().is_none() {
            tokio::select! {
                _ = &mut connection_init_deadline,
                    if connection_init_received.get().is_none() =>
                {
                    let _ = close_ws(
                        &mut sink,
                        WS_CONNECTION_INIT_TIMEOUT_CLOSE,
                        WS_CONNECTION_INIT_TIMEOUT_REASON,
                    )
                    .await;
                    break;
                }
                message = with_rbac_request_scope(None, graphql_stream.next()) => message,
            }
        } else {
            with_rbac_request_scope(scope_before_poll, graphql_stream.next()).await
        };
        let Some(message) = next_message else {
            break;
        };

        if let Some(lease) = auth_lease.get()
            && revalidate_ws_auth(&auth_runtime, lease).await.is_err()
        {
            let _ = close_ws_for_auth_change(&mut sink).await;
            break;
        }

        let result = match message {
            WsMessage::Text(text) => sink.send(Message::Text(text.into())).await,
            WsMessage::Close(code, reason) => {
                sink.send(Message::Close(Some(CloseFrame {
                    code,
                    reason: reason.into(),
                })))
                .await
            }
        };

        if result.is_err() {
            break;
        }
    }

    forward_incoming.abort();
}

async fn revalidate_ws_auth(
    auth_runtime: &ServerAuthRuntime,
    lease: &GraphqlWsAuthLease,
) -> Result<RbacRequestScope, ()> {
    let current_user =
        resolve_current_user_from_access_token(auth_runtime, lease.tenant_id, &lease.access_token)
            .await
            .map_err(|_| ())?;
    let current_scope = RbacRequestScope::new(
        current_user.user.tenant_id,
        current_user.user.id,
        current_user.permissions,
        current_user.inferred_role,
    );

    if current_scope != lease.initial_scope {
        return Err(());
    }

    Ok(current_scope)
}

async fn close_ws<S>(sink: &mut S, code: u16, reason: &'static str) -> Result<(), S::Error>
where
    S: futures_util::Sink<Message> + Unpin,
{
    sink.send(Message::Close(Some(CloseFrame {
        code,
        reason: reason.into(),
    })))
    .await
}

async fn close_ws_for_auth_change<S>(sink: &mut S) -> Result<(), S::Error>
where
    S: futures_util::Sink<Message> + Unpin,
{
    close_ws(sink, WS_CLOSE_UNAUTHORIZED, WS_AUTHORITY_CHANGED_REASON).await
}

async fn build_ws_connection_data(
    runtime_ctx: ServerRuntimeContext,
    auth_runtime: ServerAuthRuntime,
    registry: ModuleRegistry,
    auth_lease: Arc<OnceLock<GraphqlWsAuthLease>>,
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

    let locale = resolve_ws_locale(&runtime_ctx, &tenant_ctx, payload.locale.as_deref())
        .await?;

    auth_lease
        .set(GraphqlWsAuthLease {
            tenant_id: tenant_ctx.id,
            access_token,
            initial_scope: request_scope.clone(),
        })
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
        channel_id: None,
        channel_slug: None,
        channel_resolution_source: None,
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
    Ok(data)
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
                    entry.is_enabled
                        && entry.locale.as_str() == tenant_ctx.default_locale.as_str()
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

    Locale::parse(&locale)
        .ok_or_else(|| async_graphql::Error::new("Invalid tenant locale policy"))
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
        request
            .extensions
            .insert("persistedQuery".to_string(), async_graphql::Value::from_json(valid["persistedQuery"].clone()).expect("json value"));

        assert_eq!(super::persisted_query_hash(&request), Some("a".repeat(64).as_str()));

        let mut request = async_graphql::Request::new("{ __typename }");
        request.extensions = async_graphql::Extensions::default();
        request.extensions.insert(
            "persistedQuery".to_string(),
            async_graphql::Value::from_json(serde_json::json!({
                "version": 1,
                "sha256Hash": "b".repeat(65)
            })).expect("json value"),
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
    }

    #[test]
    fn graphql_ws_connection_init_wait_is_bounded_and_protocol_compliant() {
        assert_eq!(WS_CONNECTION_INIT_TIMEOUT, std::time::Duration::from_secs(10));
        assert_eq!(WS_CONNECTION_INIT_TIMEOUT_CLOSE, 4408);
        assert_eq!(
            WS_CONNECTION_INIT_TIMEOUT_REASON,
            "Connection initialisation timeout"
        );
    }

    #[test]
    fn websocket_tenant_resolution_does_not_log_raw_slug_payload() {
        let source = include_str!("graphql.rs");
        assert!(!source.contains("tracing::warn!(tenant_slug"));
        assert!(!source.contains("tracing::error!(tenant_slug"));
        assert!(!source.contains("tracing::info!(tenant_slug"));
        assert!(!source.contains("tracing::debug!(tenant_slug"));
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
        crate::models::_entities::tenants::ActiveModel {
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
            .find("auth_lease
        .set(GraphqlWsAuthLease")
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
