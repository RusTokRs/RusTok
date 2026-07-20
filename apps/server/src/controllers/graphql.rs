use std::sync::{Arc, OnceLock};

use async_graphql::Data;
use async_graphql::http::{GraphQLPlaygroundConfig, WebSocketProtocols, WsMessage};
use axum::{
    Extension, Json,
    extract::{
        State,
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, header},
    response::IntoResponse,
    routing::get,
};
use futures_util::{SinkExt, StreamExt};
use rustok_api::{Action, Permission};
use rustok_core::i18n::Locale;
use tokio_stream::wrappers::UnboundedReceiverStream;

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
    Json(req): Json<async_graphql::Request>,
) -> Json<async_graphql::Response> {
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
        let auth_ctx = AuthContext {
            user_id: current_user.user.id,
            session_id: current_user.session_id,
            tenant_id: current_user.user.tenant_id,
            permissions: graphql_permissions(current_user.permissions),
            client_id: current_user.client_id,
            scopes: current_user.scopes,
            grant_type: current_user.grant_type,
        };
        request = request.data(auth_ctx);
        if let Some(scope) = rbac_scope.as_ref() {
            request = request.data(scope.clone());
        }
    }

    let response = with_rbac_request_scope(rbac_scope, schema.execute(request)).await;
    Json(response)
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
    Some(hash.as_ref())
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
    let ws = ws.protocols(async_graphql::http::ALL_WEBSOCKET_PROTOCOLS);
    let protocol = ws
        .selected_protocol()
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<WebSocketProtocols>().ok())
        .unwrap_or(WebSocketProtocols::GraphQLWS);

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
    let (incoming_tx, incoming_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let auth_lease = Arc::new(OnceLock::<GraphqlWsAuthLease>::new());

    let schema_for_stream = schema.as_ref().clone();
    let runtime_ctx_for_init = runtime_ctx.clone();
    let auth_runtime_for_init = auth_runtime.clone();
    let registry_for_init = registry.clone();
    let auth_lease_for_init = Arc::clone(&auth_lease);
    let mut graphql_stream = async_graphql::http::WebSocket::new(
        schema_for_stream,
        UnboundedReceiverStream::new(incoming_rx),
        protocol,
    )
    .on_connection_init(move |payload| {
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
                    if incoming_tx.send(text.to_string()).is_err() {
                        break;
                    }
                }
                Ok(Message::Binary(bytes)) => {
                    if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                        if incoming_tx.send(text).is_err() {
                            break;
                        }
                    }
                }
                Ok(Message::Close(_)) => break,
                Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {}
                Err(_) => break,
            }
        }
    });

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

        let next_message = with_rbac_request_scope(scope_before_poll, graphql_stream.next()).await;
        let Some(message) = next_message else {
            break;
        };

        if let Some(lease) = auth_lease.get() {
            if revalidate_ws_auth(&auth_runtime, lease).await.is_err() {
                let _ = close_ws_for_auth_change(&mut sink).await;
                break;
            }
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

async fn close_ws_for_auth_change<S>(sink: &mut S) -> Result<(), S::Error>
where
    S: futures_util::Sink<Message> + Unpin,
{
    sink.send(Message::Close(Some(CloseFrame {
        code: WS_CLOSE_UNAUTHORIZED,
        reason: WS_AUTHORITY_CHANGED_REASON.into(),
    })))
    .await
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
            tracing::warn!(tenant_slug, error = %error, "GraphQL WebSocket tenant resolution failed");
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
    auth_lease
        .set(GraphqlWsAuthLease {
            tenant_id: tenant_ctx.id,
            access_token,
            initial_scope: request_scope.clone(),
        })
        .map_err(|_| async_graphql::Error::new("RBAC connection scope was already initialized"))?;

    let locale = payload
        .locale
        .as_deref()
        .and_then(Locale::parse)
        .or_else(|| Locale::parse(&tenant_ctx.default_locale))
        .unwrap_or_default();
    let auth_ctx = AuthContext {
        user_id: current_user.user.id,
        session_id: current_user.session_id,
        tenant_id: current_user.user.tenant_id,
        permissions: graphql_permissions(current_user.permissions),
        client_id: current_user.client_id,
        scopes: current_user.scopes,
        grant_type: current_user.grant_type,
    };

    let mut data = Data::default();
    data.insert(runtime_ctx.db_clone());
    data.insert(runtime_ctx);
    data.insert(registry);
    data.insert(locale);
    data.insert(tenant_ctx);
    data.insert(auth_ctx);
    data.insert(request_scope);
    Ok(data)
}

pub fn router() -> crate::routes::ServerRouter {
    axum::Router::new()
        .route(
            "/api/graphql/",
            get(graphql_playground).post(graphql_handler),
        )
        .route("/api/graphql/schema.graphql", get(graphql_schema_sdl))
        .route("/api/graphql/ws", get(graphql_ws_handler))
}

#[cfg(test)]
mod tests {
    use super::graphql_permissions;
    use crate::{
        common::settings::RustokSettings, middleware::tenant,
        services::server_runtime_context::ServerRuntimeContext,
    };
    use rustok_api::{Permission, Resource};
    use rustok_cache::CacheService;
    use rustok_migrations::Migrator;
    use sea_orm::{ActiveModelTrait, Set};
    use serial_test::serial;

    #[tokio::test]
    #[serial]
    async fn graphql_ws_tenant_handshake_fails_closed() {
        let db = rustok_test_utils::db::setup_test_db_with_migrations::<Migrator>().await;
        let runtime = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
        tenant::init_tenant_cache_infrastructure(&runtime, &CacheService::from_url(None)).await;

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
