//! WebSocket channel for durable platform build progress snapshots.
//!
//! `GET /ws/builds` upgrades to a WebSocket and sends the current active
//! build snapshots followed by newer snapshots read from the shared database.
//! Local BuildEventHub messages are wake-up hints only.

use std::{collections::HashMap, time::Duration};

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
use rustok_api::{
    HostAuthorityContext, PlatformBuildProgressSnapshot, PlatformBuildStage, PlatformBuildStatus,
};
use rustok_build::{BuildEvent, SharedBuildControl};
use serde::Serialize;
use tokio::sync::{OwnedSemaphorePermit, broadcast::error::RecvError};

use crate::extractors::auth::{CurrentUser, WebSocketAuthLease};
use crate::services::build_event_hub::{BuildEventHub, build_event_hub_from_context};
use crate::services::server_runtime_context::ServerAuthRuntime;

const BUILD_WS_AUTH_REVALIDATION_INTERVAL: Duration = Duration::from_secs(30);
const BUILD_WS_PROGRESS_POLL_INTERVAL: Duration = Duration::from_secs(1);
const BUILD_WS_PROGRESS_PAGE_SIZE: u64 = 100;
const BUILD_WS_MAX_MESSAGE_SIZE: usize = 64 * 1024;
const BUILD_WS_SEND_TIMEOUT: Duration = Duration::from_secs(5);
const BUILD_WS_AUTH_CHANGED_CODE: u16 = 4401;
const BUILD_WS_AUTH_CHANGED_REASON: &str = "authorization changed; reconnect required";
const BUILD_WS_INTERNAL_ERROR_CODE: u16 = 1011;
const BUILD_WS_INTERNAL_ERROR_REASON: &str = "build progress temporarily unavailable";

// ── Wire-format message ───────────────────────────────────────────────────────

/// JSON message sent over the WebSocket for each latest build-state snapshot.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(clippy::enum_variant_names)]
enum WsBuildMessage {
    BuildRequested {
        build_id: String,
        requested_by: String,
        sequence: String,
        is_snapshot: bool,
    },
    BuildStarted {
        build_id: String,
        stage: String,
        progress: i32,
        sequence: String,
        is_snapshot: bool,
    },
    BuildProgress {
        build_id: String,
        stage: String,
        progress: i32,
        sequence: String,
        is_snapshot: bool,
    },
    BuildCompleted {
        build_id: String,
        sequence: String,
        is_snapshot: bool,
    },
    BuildCancelled {
        build_id: String,
        stage: String,
        progress: i32,
        sequence: String,
        is_snapshot: bool,
    },
    BuildFailed {
        build_id: String,
        stage: String,
        progress: i32,
        error: String,
        sequence: String,
        is_snapshot: bool,
    },
}

impl WsBuildMessage {
    fn from_snapshot(
        snapshot: PlatformBuildProgressSnapshot,
        is_snapshot: bool,
    ) -> Self {
        let sequence = snapshot.sequence.to_string();
        let build_id = snapshot.id;
        let stage = format!("{:?}", snapshot.stage).to_lowercase();
        match snapshot.status {
            PlatformBuildStatus::Queued => Self::BuildRequested {
                build_id,
                requested_by: snapshot.requested_by,
                sequence,
                is_snapshot,
            },
            PlatformBuildStatus::Running => Self::BuildProgress {
                build_id,
                stage,
                progress: snapshot.progress,
                sequence,
                is_snapshot,
            },
            PlatformBuildStatus::Success => Self::BuildCompleted {
                build_id,
                sequence,
                is_snapshot,
            },
            PlatformBuildStatus::Cancelled => Self::BuildCancelled {
                build_id,
                stage,
                progress: snapshot.progress,
                sequence,
                is_snapshot,
            },
            PlatformBuildStatus::Failed => Self::BuildFailed {
                build_id,
                stage,
                progress: snapshot.progress,
                error: snapshot
                    .error_message
                    .unwrap_or_else(|| "Build failed".to_string()),
                sequence,
                is_snapshot,
            },
        }
    }
}

impl From<BuildEvent> for WsBuildMessage {
    fn from(event: BuildEvent) -> Self {
        let sequence = "0".to_string();
        let is_snapshot = false;
        match event {
            BuildEvent::BuildRequested {
                build_id,
                requested_by,
            } => Self::BuildRequested {
                build_id: build_id.to_string(),
                requested_by,
                sequence,
                is_snapshot,
            },
            BuildEvent::BuildStarted {
                build_id,
                stage,
                progress,
            } => Self::BuildStarted {
                build_id: build_id.to_string(),
                stage: format!("{stage:?}").to_lowercase(),
                progress,
                sequence,
                is_snapshot,
            },
            BuildEvent::BuildProgress {
                build_id,
                stage,
                progress,
            } => Self::BuildProgress {
                build_id: build_id.to_string(),
                stage: format!("{stage:?}").to_lowercase(),
                progress,
                sequence,
                is_snapshot,
            },
            BuildEvent::BuildCompleted { build_id } => Self::BuildCompleted {
                build_id: build_id.to_string(),
                sequence,
                is_snapshot,
            },
            BuildEvent::BuildCancelled {
                build_id,
                stage,
                progress,
            } => Self::BuildCancelled {
                build_id: build_id.to_string(),
                stage: format!("{stage:?}").to_lowercase(),
                progress,
                sequence,
                is_snapshot,
            },
            BuildEvent::BuildFailed {
                build_id,
                stage,
                progress,
                error,
            } => Self::BuildFailed {
                build_id: build_id.to_string(),
                stage: format!("{stage:?}").to_lowercase(),
                progress,
                error,
                sequence,
                is_snapshot,
            },
        }
    }
}

// ── Handler ───────────────────────────────────────────────────────────────────

/// Upgrade an HTTP request to a WebSocket connection that streams build state.
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
    let Some(stream_permit) = hub.try_acquire_stream() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "Platform build progress stream capacity reached",
        )
            .into_response();
    };

    ws.max_message_size(BUILD_WS_MAX_MESSAGE_SIZE)
        .max_frame_size(BUILD_WS_MAX_MESSAGE_SIZE)
        .on_upgrade(move |socket| {
            handle_socket(
                socket,
                hub,
                auth_runtime,
                lease,
                host_authority,
                stream_permit,
            )
        })
        .into_response()
}

async fn check_build_ws_auth(
    auth_runtime: &ServerAuthRuntime,
    lease: &WebSocketAuthLease,
    host_authority: Option<HostAuthorityContext>,
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

async fn close_for_internal_error(socket: &mut WebSocket) {
    let result = tokio::time::timeout(
        BUILD_WS_SEND_TIMEOUT,
        socket.send(Message::Close(Some(CloseFrame {
            code: BUILD_WS_INTERNAL_ERROR_CODE,
            reason: BUILD_WS_INTERNAL_ERROR_REASON.into(),
        }))),
    )
    .await;
    if !matches!(result, Ok(Ok(()))) {
        tracing::debug!("Failed to send build WebSocket internal-error close");
    }
}

async fn send_ws_build_message(
    socket: &mut WebSocket,
    lease: &WebSocketAuthLease,
    message: WsBuildMessage,
) -> bool {
    let json = match serde_json::to_string(&message) {
        Ok(json) => json,
        Err(error) => {
            tracing::warn!(
                %error,
                failure_code = "build_event_serialization_failed",
                "Failed to serialize build progress snapshot"
            );
            return false;
        }
    };
    let send_timeout = BUILD_WS_SEND_TIMEOUT.min(lease.remaining());
    if send_timeout.is_zero() {
        return false;
    }
    matches!(
        tokio::time::timeout(send_timeout, socket.send(Message::Text(json.into()))).await,
        Ok(Ok(()))
    )
}

async fn send_build_updates(
    socket: &mut WebSocket,
    control: &SharedBuildControl,
    cursor: &mut i64,
    last_sequences: &mut HashMap<String, i64>,
    auth_runtime: &ServerAuthRuntime,
    lease: &WebSocketAuthLease,
    host_authority: Option<HostAuthorityContext>,
) -> bool {
    let updates = match control
        .0
        .build_progress_after(*cursor, BUILD_WS_PROGRESS_PAGE_SIZE)
        .await
    {
        Ok(updates) => updates,
        Err(error) => {
            tracing::error!(%error, "Durable WebSocket build progress read failed");
            close_for_internal_error(socket).await;
            return false;
        }
    };

    for snapshot in updates {
        *cursor = (*cursor).max(snapshot.sequence);
        if last_sequences
            .get(&snapshot.id)
            .is_some_and(|seen| *seen >= snapshot.sequence)
        {
            continue;
        }
        last_sequences.insert(snapshot.id.clone(), snapshot.sequence);

        if !check_build_ws_auth(auth_runtime, lease, host_authority).await {
            close_for_auth_change(socket).await;
            return false;
        }

        if !send_ws_build_message(
            socket,
            lease,
            WsBuildMessage::from_snapshot(snapshot, false),
        )
        .await
        {
            return false;
        }
    }

    last_sequences.retain(|_, sequence| *sequence > *cursor);
    true
}

async fn handle_socket(
    mut socket: WebSocket,
    hub: std::sync::Arc<BuildEventHub>,
    auth_runtime: ServerAuthRuntime,
    lease: WebSocketAuthLease,
    host_authority: Option<HostAuthorityContext>,
    _stream_permit: OwnedSemaphorePermit,
) {
    let control = hub.control();

    // Read the cursor before subscribing; every later commit is then either
    // reflected in the initial snapshot or returned by the cursor query.
    let mut cursor = match control.0.current_progress_sequence().await {
        Ok(cursor) => cursor,
        Err(error) => {
            tracing::error!(%error, "Unable to initialize WebSocket build progress cursor");
            close_for_internal_error(&mut socket).await;
            return;
        }
    };
    let mut rx = hub.subscribe();

    let initial = match control.0.active_build_progress().await {
        Ok(initial) => initial,
        Err(error) => {
            tracing::error!(%error, "Unable to read initial WebSocket build snapshots");
            close_for_internal_error(&mut socket).await;
            return;
        }
    };

    let mut last_sequences = HashMap::new();
    for snapshot in initial {
        last_sequences.insert(snapshot.id.clone(), snapshot.sequence);
        if !check_build_ws_auth(&auth_runtime, &lease, host_authority).await {
            close_for_auth_change(&mut socket).await;
            return;
        }
        if !send_ws_build_message(
            &mut socket,
            &lease,
            WsBuildMessage::from_snapshot(snapshot, true),
        )
        .await
        {
            return;
        }
    }

    let mut revalidation = tokio::time::interval(BUILD_WS_AUTH_REVALIDATION_INTERVAL);
    revalidation.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut progress_poll = tokio::time::interval(BUILD_WS_PROGRESS_POLL_INTERVAL);
    progress_poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
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
            _ = progress_poll.tick() => {
                if !check_build_ws_auth(&auth_runtime, &lease, host_authority).await {
                    close_for_auth_change(&mut socket).await;
                    break;
                }
                if !send_build_updates(
                    &mut socket,
                    &control,
                    &mut cursor,
                    &mut last_sequences,
                    &auth_runtime,
                    &lease,
                    host_authority,
                ).await {
                    break;
                }
            }
            result = rx.recv() => {
                match result {
                    Ok(()) => {
                        if !check_build_ws_auth(&auth_runtime, &lease, host_authority).await {
                            close_for_auth_change(&mut socket).await;
                            break;
                        }
                        if !send_build_updates(
                            &mut socket,
                            &control,
                            &mut cursor,
                            &mut last_sequences,
                            &auth_runtime,
                            &lease,
                            host_authority,
                        ).await {
                            break;
                        }
                    }
                    Err(RecvError::Lagged(skipped)) => {
                        tracing::warn!(
                            skipped,
                            "WebSocket build wake-up channel lagged; reconciling durable state"
                        );
                        if !check_build_ws_auth(&auth_runtime, &lease, host_authority).await {
                            close_for_auth_change(&mut socket).await;
                            break;
                        }
                        if !send_build_updates(
                            &mut socket,
                            &control,
                            &mut cursor,
                            &mut last_sequences,
                            &auth_runtime,
                            &lease,
                            host_authority,
                        ).await {
                            break;
                        }
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
    use uuid::Uuid;

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
        use crate::auth::encode_access_token;
        use crate::common::settings::RustokSettings;
        use crate::context::{TenantContext, TenantContextExtension};
        use crate::models::{sessions, tenants, users};
        use crate::services::server_runtime_context::ServerRuntimeContext;
        use chrono::{Duration as ChronoDuration, Utc};
        use rustok_core::UserRole;
        use rustok_build::{
            BuildExecutionPlan, BuildRequest, BuildRuntimeMode, BuildService, BuildStage,
            BuildStatus, DeploymentProfile,
        };
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
        let build = BuildService::new(db.clone())
            .request_build(BuildRequest {
                manifest_ref: "main".to_string(),
                manifest_revision: 1,
                manifest_snapshot: serde_json::json!({"modules": []}),
                artifact_identity: "websocket-test-artifact".to_string(),
                requested_by: user.id.to_string(),
                reason: Some("WebSocket progress test".to_string()),
                modules_delta: "{}".to_string(),
                modules: std::collections::HashMap::new(),
                profile: DeploymentProfile::Monolith,
                execution_plan: BuildExecutionPlan {
                    runtime_mode: BuildRuntimeMode::Api,
                    cargo_package: "rustok-server".to_string(),
                    cargo_profile: "release".to_string(),
                    cargo_target: None,
                    cargo_features: Vec::new(),
                    cargo_command: "cargo build -p rustok-server --release".to_string(),
                    admin_build: None,
                    storefront_build: None,
                },
            })
            .await
            .expect("persist queued build");
        let config =
            crate::auth::test_auth_config("test-secret-key-for-auth-extractor-32b".to_string());
        let token = encode_access_token(
            &config,
            user.id,
            tenant.id,
            UserRole::SuperAdmin,
            session_id,
        )
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
            let (snapshot_opcode, snapshot_payload) = frame(&mut stream).await;
            assert_eq!(snapshot_opcode, 0x01);
            let snapshot: serde_json::Value =
                serde_json::from_slice(&snapshot_payload).expect("snapshot JSON");
            assert_eq!(snapshot["type"], "build_requested");
            assert_eq!(snapshot["is_snapshot"], true);
            assert_eq!(snapshot["build_id"], build.id.to_string());

            sessions::Entity::delete_by_id(session_id)
                .exec(&db)
                .await
                .expect("revoke session");
            BuildService::new(db.clone())
                .update_build_status(
                    build.id,
                    BuildStatus::Running,
                    Some(BuildStage::Checkout),
                    Some(10),
                )
                .await
                .expect("persist next build state");
            hub.publish(BuildEvent::BuildProgress {
                build_id: build.id,
                stage: BuildStage::Checkout,
                progress: 10,
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
