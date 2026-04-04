use std::sync::Arc;

use async_graphql::http::{GraphQLPlaygroundConfig, WebSocketProtocols, WsMessage};
use async_graphql::Data;
use axum::{
    extract::{
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::HeaderMap,
    response::IntoResponse,
    routing::get,
    Extension, Json,
};
use futures_util::{SinkExt, StreamExt};
use loco_rs::app::AppContext;
use loco_rs::controller::Routes;
use rustok_core::i18n::Locale;
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::common::RequestContext;
use crate::context::{AuthContext, TenantContext};
use crate::extractors::auth::{
    resolve_current_user_from_access_token, CurrentUser, OptionalCurrentUser,
};
use crate::graphql::persisted::is_cataloged_admin_hash;
use crate::graphql::AppSchema;
use rustok_core::ModuleRegistry;

async fn graphql_handler(
    State(ctx): State<AppContext>,
    Extension(registry): Extension<ModuleRegistry>,
    Extension(schema): Extension<Arc<AppSchema>>,
    tenant_ctx: TenantContext,
    request_context: RequestContext,
    OptionalCurrentUser(current_user): OptionalCurrentUser,
    headers: HeaderMap,
    Json(req): Json<async_graphql::Request>,
) -> Json<async_graphql::Response> {
    let locale = Locale::parse(&request_context.locale).unwrap_or_default();
    if let Some(hash) = persisted_query_hash(&req) {
        tracing::debug!(
            persisted_query_hash = hash,
            cataloged_admin_hash = is_cataloged_admin_hash(hash),
            "Observed persisted query hash for GraphQL telemetry"
        );
    }

    let mut request = req
        .data(ctx)
        .data(tenant_ctx)
        .data(request_context)
        .data(headers)
        .data(registry)
        .data(locale);

    if let Some(current_user) = current_user {
        let auth_ctx = AuthContext {
            user_id: current_user.user.id,
            session_id: current_user.session_id,
            tenant_id: current_user.user.tenant_id,
            permissions: current_user.permissions,
            client_id: current_user.client_id,
            scopes: current_user.scopes.clone(),
            grant_type: current_user.grant_type.clone(),
        };
        request = request.data(auth_ctx);
    }

    Json(schema.execute(request).await)
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
    State(ctx): State<AppContext>,
    Extension(registry): Extension<ModuleRegistry>,
    Extension(schema): Extension<Arc<AppSchema>>,
) -> impl IntoResponse {
    let ws = ws.protocols(async_graphql::http::ALL_WEBSOCKET_PROTOCOLS);
    let protocol = ws
        .selected_protocol()
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<WebSocketProtocols>().ok())
        .unwrap_or(WebSocketProtocols::GraphQLWS);

    ws.on_upgrade(move |socket| handle_graphql_ws(socket, schema, ctx, registry, protocol))
}

async fn handle_graphql_ws(
    socket: WebSocket,
    schema: Arc<AppSchema>,
    app_ctx: AppContext,
    registry: ModuleRegistry,
    protocol: WebSocketProtocols,
) {
    let (mut sink, mut source) = socket.split();
    let (incoming_tx, incoming_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    let schema_for_stream = schema.as_ref().clone();
    let app_ctx_for_init = app_ctx.clone();
    let registry_for_init = registry.clone();
    let mut graphql_stream = async_graphql::http::WebSocket::new(
        schema_for_stream,
        UnboundedReceiverStream::new(incoming_rx),
        protocol,
    )
    .on_connection_init(move |payload| {
        build_ws_connection_data(app_ctx_for_init.clone(), registry_for_init.clone(), payload)
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

    while let Some(message) = graphql_stream.next().await {
        let result = match message {
            WsMessage::Text(text) => sink.send(Message::Text(text.into())).await,
            WsMessage::Close(code, reason) => {
                sink.send(Message::Close(Some(CloseFrame {
                    code: code.into(),
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

async fn build_ws_connection_data(
    app_ctx: AppContext,
    registry: ModuleRegistry,
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

    let tenant = crate::models::tenants::Entity::find_by_slug(&app_ctx.db, &tenant_slug)
        .await
        .map_err(|_| async_graphql::Error::new("Failed to resolve tenant"))?
        .ok_or_else(|| async_graphql::Error::new("Tenant not found"))?;

    if !tenant.is_enabled() {
        return Err(async_graphql::Error::new("Tenant is disabled"));
    }

    let access_token = token
        .trim()
        .strip_prefix("Bearer ")
        .or_else(|| token.trim().strip_prefix("bearer "))
        .unwrap_or(token.trim());
    let current_user = resolve_current_user_from_access_token(&app_ctx, tenant.id, access_token)
        .await
        .map_err(|(_, message)| async_graphql::Error::new(message))?;

    let locale = payload
        .locale
        .as_deref()
        .and_then(Locale::parse)
        .or_else(|| Locale::parse(&tenant.default_locale))
        .unwrap_or_default();
    let tenant_ctx = TenantContext {
        id: tenant.id,
        name: tenant.name,
        slug: tenant.slug,
        domain: tenant.domain,
        settings: tenant.settings,
        default_locale: tenant.default_locale,
        is_active: tenant.is_active,
    };
    let auth_ctx = AuthContext {
        user_id: current_user.user.id,
        session_id: current_user.session_id,
        tenant_id: current_user.user.tenant_id,
        permissions: current_user.permissions,
        client_id: current_user.client_id,
        scopes: current_user.scopes,
        grant_type: current_user.grant_type,
    };

    let mut data = Data::default();
    data.insert(app_ctx);
    data.insert(registry);
    data.insert(locale);
    data.insert(tenant_ctx);
    data.insert(auth_ctx);
    Ok(data)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/graphql")
        .add("/", get(graphql_playground).post(graphql_handler))
        .add("/ws", get(graphql_ws_handler))
}
