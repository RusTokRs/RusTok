//! WebSocket channel for streaming build events.
//!
//! `GET /ws/builds` — upgrades to a WebSocket connection and streams
//! `BuildEvent` payloads as newline-delimited JSON until the build hub
//! is dropped or the client disconnects.
//!
//! **Authentication**: a valid user Bearer token plus explicit host-global
//! read authority (`x-rustok-host-token`), configured by the host operator.

use axum::{
    extract::{
        State,
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_extra::{
    TypedHeader,
    headers::{Authorization, authorization::Bearer},
};
use serde::Serialize;
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;
use uuid::Uuid;

use crate::extractors::auth::{CurrentUser, WebSocketAuthLease};
use crate::services::build_event_hub::build_event_hub_from_context;
use crate::services::server_runtime_context::ServerAuthRuntime;
use rustok_build::BuildEvent;

const BUILD_WS_AUTH_REVALIDATION_INTERVAL: Duration = Duration::from_secs(30);
const BUILD_WS_MAX_MESSAGE_SIZE: usize = 64 * 1024;
const BUILD_WS_SEND_TIMEOUT: Duration = Duration::from_secs(5);
const BUILD_WS_AUTH_CHANGED_CODE: u16 = 4401;
const BUILD_WS_AUTH_CHANGED_REASON: &str = "authorization changed; reconnect required";

// ── Wire-format message ───────────────────────────────────────────────────────

/// JSON message sent over the WebSocket for every build event.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(clippy::enum_variant_names)]
enum WsBuildMessage {
    BuildRequested {
        build_id: Uuid,
        requested_by: String,
    },
    BuildStarted {
        build_id: Uuid,
        stage: String,
        progress: i32,
    },
    BuildProgress {
        build_id: Uuid,
        stage: String,
        progress: i32,
    },
    BuildCompleted {
        build_id: Uuid,
    },
    BuildCancelled {
        build_id: Uuid,
        stage: String,
        progress: i32,
    },
    BuildFailed {
        build_id: Uuid,
        stage: String,
        progress: i32,
        error: String,
    },
}

impl From<BuildEvent> for WsBuildMessage {
    fn from(ev: BuildEvent) -> Self {
        match ev {
            BuildEvent::BuildRequested {
                build_id,
                requested_by,
            } => Self::BuildRequested {
                build_id,
                requested_by,
            },
            BuildEvent::BuildStarted {
                build_id,
                stage,
                progress,
            } => Self::BuildStarted {
                build_id,
                stage: format!("{stage:?}").to_lowercase(),
                progress,
            },
            BuildEvent::BuildProgress {
                build_id,
                stage,
                progress,
            } => Self::BuildProgress {
                build_id,
                stage: format!("{stage:?}").to_lowercase(),
                progress,
            },
            BuildEvent::BuildCompleted { build_id } => Self::BuildCompleted { build_id },
            BuildEvent::BuildCancelled {
                build_id,
                stage,
                progress,
            } => Self::BuildCancelled {
                build_id,
                stage: format!("{stage:?}").to_lowercase(),
                progress,
            },
            BuildEvent::BuildFailed {
                build_id,
                stage,
                progress,
                error,
            } => Self::BuildFailed {
                build_id,
                stage: format!("{stage:?}").to_lowercase(),
                progress,
                error,
            },
        }
    }
}

// ── Handler ───────────────────────────────────────────────────────────────────

/// Upgrade an HTTP request to a WebSocket connection that streams build events.
pub async fn ws_builds(
    ws: WebSocketUpgrade,
    State(auth_runtime): State<ServerAuthRuntime>,
    current_user: CurrentUser,
    TypedHeader(Authorization(bearer)): TypedHeader<Authorization<Bearer>>,
) -> Response {
    let host_authority = crate::host_authority::current_host_authority();
    if !crate::services::platform_build_access::is_platform_build_operator(host_authority) {
        return (
            StatusCode::FORBIDDEN,
            "Host-global read authority is required for the platform build stream",
        )
            .into_response();
    }

    let lease = WebSocketAuthLease::new(bearer.token().to_owned(), &current_user);
    let hub = build_event_hub_from_context(auth_runtime.runtime_ctx());
    ws.max_message_size(BUILD_WS_MAX_MESSAGE_SIZE)
        .max_frame_size(BUILD_WS_MAX_MESSAGE_SIZE)
        .on_upgrade(move |socket| handle_socket(socket, hub, auth_runtime, lease, host_authority))
        .into_response()
}

async fn check_build_ws_auth(
    auth_runtime: &ServerAuthRuntime,
    lease: &WebSocketAuthLease,
    host_authority: Option<rustok_api::HostAuthorityContext>,
) -> bool {
    crate::services::platform_build_access::is_platform_build_operator(host_authority)
        && lease.revalidate(auth_runtime).await.is_ok()
}

async fn close_for_auth_change(socket: &mut WebSocket) {
    let result = tokio::time::timeout(
        BUILD_WS_SEND_TIMEOUT,
        socket.send(Message::Close(Some(CloseFrame {
            code: BUILD_WS_AUTH_CHANGED_CODE,
            reason: BUILD_WS_AUTH_CHANGED_REASON.into(),
        }))),
    )
    .await;
    if !matches!(result, Ok(Ok(()))) {
        tracing::debug!("Failed to send build WebSocket authorization close");
    }
}

async fn handle_socket(
    mut socket: WebSocket,
    hub: std::sync::Arc<crate::services::build_event_hub::BuildEventHub>,
    auth_runtime: ServerAuthRuntime,
    lease: WebSocketAuthLease,
    host_authority: Option<rustok_api::HostAuthorityContext>,
) {
    let mut rx = hub.subscribe();
    let mut revalidation = tokio::time::interval(BUILD_WS_AUTH_REVALIDATION_INTERVAL);
    revalidation.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let expiry = tokio::time::sleep(lease.remaining());
    tokio::pin!(expiry);

    loop {
        tokio::select! {
            _ = &mut expiry => {
                close_for_auth_change(&mut socket).await;
                break;
            }
            _ = revalidation.tick() => {
                if !check_build_ws_auth(&auth_runtime, &lease, host_authority).await {
                    close_for_auth_change(&mut socket).await;
                    break;
                }
            }
            result = rx.recv() => {
                match result {
                    Ok(event) => {
                        // Revalidate the authority before sending each event,
                        // even when the periodic validation tick has not fired yet.
                        if !check_build_ws_auth(&auth_runtime, &lease, host_authority).await {
                            close_for_auth_change(&mut socket).await;
                            break;
                        }
                        let msg: WsBuildMessage = event.into();
                        match serde_json::to_string(&msg) {
                            Ok(json) => {
                                let send_timeout = BUILD_WS_SEND_TIMEOUT.min(lease.remaining());
                                if send_timeout.is_zero() {
                                    close_for_auth_change(&mut socket).await;
                                    break;
                                }
                                if !matches!(
                                    tokio::time::timeout(
                                        send_timeout,
                                        socket.send(Message::Text(json.into())),
                                    ).await,
                                    Ok(Ok(()))
                                ) {
                                    break;
                                }
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, "Failed to serialise build event");
                            }
                        }
                    }
                    Err(RecvError::Lagged(n)) => {
                        tracing::warn!(skipped = n, "WebSocket build channel lagged");
                    }
                    Err(RecvError::Closed) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                    Some(Ok(Message::Ping(payload))) => {
                        let send_timeout = BUILD_WS_SEND_TIMEOUT.min(lease.remaining());
                        if send_timeout.is_zero() {
                            close_for_auth_change(&mut socket).await;
                            break;
                        }
                        if !matches!(
                            tokio::time::timeout(
                                send_timeout,
                                socket.send(Message::Pong(payload)),
                            ).await,
                            Ok(Ok(()))
                        ) {
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

// ── Route registration ────────────────────────────────────────────────────────

pub fn router() -> crate::routes::ServerRouter {
    use axum::routing::get;
    axum::Router::new().route("/ws/builds", get(ws_builds))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn with_test_host_authority(
        request: axum::extract::Request,
        next: axum::middleware::Next,
    ) -> Response {
        let authority = rustok_api::HostAuthorityContext::manage(Uuid::new_v4())
            .expect("non-nil test host actor");
        rustok_api::with_host_authority_scope(Some(authority), next.run(request)).await
    }

    #[tokio::test]
    async fn build_socket_closes_before_forwarding_after_session_revocation() {
        use super::*;
        use crate::auth::{AuthSettingsOverrides, encode_access_token};
        use crate::common::settings::RustokSettings;
        use crate::context::{TenantContext, TenantContextExtension};
        use crate::models::{sessions, tenants, users};
        use crate::services::server_runtime_context::ServerRuntimeContext;
        use chrono::{Duration as ChronoDuration, Utc};
        use rustok_core::UserRole;
        use sea_orm::{ActiveModelTrait, EntityTrait, Set};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        async fn frame(stream: &mut tokio::net::TcpStream) -> (u8, Vec<u8>) {
            let mut header = [0u8; 2];
            stream.read_exact(&mut header).await.expect("frame header");
            let len = match header[1] & 0x7f {
                126 => stream.read_u16().await.expect("frame length") as usize,
                127 => stream.read_u64().await.expect("frame length") as usize,
                value => value as usize,
            };
            assert_eq!(header[1] & 0x80, 0, "server frames must not be masked");
            assert!(len <= 4096, "test frame exceeds bound");
            let mut payload = vec![0; len];
            stream
                .read_exact(&mut payload)
                .await
                .expect("frame payload");
            (header[0] & 0x0f, payload)
        }

        let db = rustok_test_utils::db::setup_test_db_with_migrations::<
            rustok_migrations::SqliteTestMigrator,
        >()
        .await;
        let tenant = tenants::ActiveModel::new("WS build test", &format!("ws-{}", Uuid::new_v4()))
            .insert(&db)
            .await
            .expect("tenant");
        let user = users::ActiveModel::new(tenant.id, "ws-build@example.com", "hash")
            .insert(&db)
            .await
            .expect("user");
        rustok_rbac::RbacRoleAssignmentDbWriter::assign_role_on(
            &db,
            tenant.id,
            user.id,
            UserRole::SuperAdmin,
        )
        .await
        .expect("superadmin role");
        let mut session = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "hash".to_string(),
            Utc::now() + ChronoDuration::hours(1),
            None,
            None,
        );
        let session_id = Uuid::new_v4();
        session.id = Set(session_id);
        session.insert(&db).await.expect("session");
        let config = rustok_auth::build_auth_config(
            "test-secret-key-for-auth-extractor-32b".to_string(),
            900,
            AuthSettingsOverrides::default(),
        )
        .expect("auth configuration");
        let token =
            encode_access_token(&config, user.id, tenant.id, UserRole::SuperAdmin, session_id)
                .expect("access token");
        let runtime = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
        let hub = build_event_hub_from_context(&runtime);
        let auth_runtime = ServerAuthRuntime::new(runtime, config);
        let tenant_ctx = TenantContext {
            id: tenant.id,
            name: tenant.name,
            slug: tenant.slug,
            domain: tenant.domain,
            settings: tenant.settings,
            default_locale: tenant.default_locale,
            is_active: true,
        };
        let app = router()
            .layer(axum::Extension(TenantContextExtension(tenant_ctx)))
            .layer(axum::middleware::from_fn(with_test_host_authority))
            .with_state(auth_runtime);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("server address");
        let server = tokio::spawn(async move { axum::serve(listener, app).await });
        let mut stream = tokio::net::TcpStream::connect(addr).await.expect("connect");
        let request = format!(
            "GET /ws/builds HTTP/1.1\r\nHost: {addr}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nAuthorization: Bearer {token}\r\n\r\n"
        );
        stream
            .write_all(request.as_bytes())
            .await
            .expect("upgrade request");
        tokio::time::timeout(Duration::from_secs(10), async {
            let mut response = Vec::new();
            while !response.ends_with(b"\r\n\r\n") {
                response.push(stream.read_u8().await.expect("upgrade response"));
                assert!(response.len() < 16 * 1024);
            }
            assert!(response.starts_with(b"HTTP/1.1 101"));
            while hub.receiver_count() == 0 {
                tokio::task::yield_now().await;
            }
            hub.publish(BuildEvent::BuildCompleted {
                build_id: Uuid::new_v4(),
            });
            assert_eq!(frame(&mut stream).await.0, 0x01);
            sessions::Entity::delete_by_id(session_id)
                .exec(&db)
                .await
                .expect("revoke session");
            hub.publish(BuildEvent::BuildCompleted {
                build_id: Uuid::new_v4(),
            });
            let (opcode, payload) = frame(&mut stream).await;
            assert_eq!(opcode, 0x08, "revoked session must not receive the event");
            assert!(payload.len() >= 2);
            assert_eq!(
                u16::from_be_bytes([payload[0], payload[1]]),
                BUILD_WS_AUTH_CHANGED_CODE
            );
        })
        .await
        .expect("WebSocket test deadline");
        server.abort();
    }
}
