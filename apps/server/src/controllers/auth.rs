use crate::error::Error;
use crate::error::Result;
use axum::response::Response;
use axum::{
    Json,
    body::{Body, to_bytes},
    extract::{ConnectInfo, FromRequest, Path, Query, Request, State},
    http::{
        StatusCode,
        header::{CACHE_CONTROL, CONTENT_TYPE, USER_AGENT},
    },
    middleware::Next,
    routing::{delete, get, post},
};
use rustok_api::Permission;
use rustok_telemetry::metrics;
use rustok_web::json_response;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use std::net::SocketAddr;

use crate::auth::encode_email_verification_token;
use crate::common::{RequestContext, RustokSettings};
use crate::context::TenantContextExt;
use crate::extractors::{auth::CurrentUser, tenant::CurrentTenant};
use crate::models::{
    sessions,
    users::{self, Entity as Users},
};
use crate::services::auth_lifecycle::{AuthLifecycleError, AuthLifecycleService, AuthTokens};
use crate::services::email::{
    EMAIL_VERIFICATION_TOKEN_TTL_SECS, email_service_from_ctx, spawn_email_verification,
    spawn_email_verification_send,
};
use crate::services::server_runtime_context::ServerAuthRuntime;
pub use rustok_auth::{
    AcceptInviteParams, AuthResponse, ChangePasswordParams, ConfirmResetParams,
    ConfirmVerificationParams, CookieSessionResponse, GenericStatusResponse, InviteAcceptResponse,
    LoginParams, LogoutResponse, RefreshRequest, RegisterParams, RequestResetParams,
    RequestVerificationParams, ResetRequestResponse, SessionItem, SessionListParams,
    SessionsResponse, UpdateProfileParams, UserInfo, UserResponse, VerificationRequestResponse,
};

fn user_info_from_model(
    user: users::Model,
    role: rustok_core::UserRole,
    permissions: Vec<String>,
) -> UserInfo {
    UserInfo {
        id: user.id,
        email: user.email,
        name: user.name,
        role,
        status: user.status,
        permissions,
    }
}

fn user_response_from_model(
    user: users::Model,
    role: rustok_core::UserRole,
    permissions: Vec<String>,
) -> UserResponse {
    UserResponse {
        id: user.id,
        email: user.email,
        name: user.name,
        role: role.to_string(),
        permissions,
    }
}

/// Stable display form of an effective permission snapshot: sorted and
/// deduplicated `"{resource}:{action}"` strings.
fn permission_strings(permissions: &[Permission]) -> Vec<String> {
    let mut values = permissions
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn ensure_registration_enabled(settings: &RustokSettings) -> Result<()> {
    if !settings.features.registration_enabled {
        return Err(Error::Forbidden("Registration is disabled".into()));
    }
    if settings.features.invite_only_registration {
        return Err(Error::Forbidden(
            "Registration is available by invitation only".into(),
        ));
    }
    Ok(())
}

#[utoipa::path(post, path = "/api/auth/register", tag = "auth", request_body = RegisterParams,
    responses((status = 200, description = "Registration successful", body = AuthResponse),(status = 400, description = "Email already exists")))]
async fn register(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    request_context: RequestContext,
    Json(params): Json<RegisterParams>,
) -> Result<Response> {
    let email = params.email.trim();
    tracing::debug!("auth::register entry");
    ensure_registration_enabled(ctx.runtime_ctx().settings())?;

    let runtime_ctx = ctx.runtime_ctx();
    let config = ctx
        .auth_config()
        .cloned()
        .ok_or(Error::InternalServerError)?;
    let (user, mut tokens) = AuthLifecycleService::register_runtime(
        runtime_ctx,
        &config,
        tenant.id,
        email,
        &params.password,
        params.name,
    )
    .await
    .map_err(|e: AuthLifecycleError| Error::from(e))?;

    if runtime_ctx.settings().features.email_verification {
        spawn_email_verification(runtime_ctx, &config, &user, request_context.locale.as_str());
    }

    tokens
        .revalidate_for_publication(&config)
        .map_err(Error::from)?;
    let user_role = tokens.effective_role.clone();
    Ok(json_response(AuthResponse {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        token_type: "Bearer",
        expires_in: tokens.expires_in,
        user: user_info_from_model(user, user_role, tokens.permissions.clone()),
    }))
}

/// Audit the verified client address, not the load balancer, while refusing
/// forwarding headers supplied by peers outside the configured proxy CIDRs.
fn login_client_ip(
    headers: &axum::http::HeaderMap,
    peer_ip: std::net::IpAddr,
    settings: &RustokSettings,
) -> std::net::IpAddr {
    crate::common::extract_effective_client_ip(
        headers,
        Some(peer_ip),
        &settings.runtime.request_trust,
    )
    .unwrap_or(peer_ip)
}

#[utoipa::path(post, path = "/api/auth/login", tag = "auth", request_body = LoginParams,
    responses((status = 200, description = "Login successful", body = AuthResponse),(status = 401, description = "Unauthorized")))]
async fn login(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: axum::http::HeaderMap,
    Json(params): Json<LoginParams>,
) -> Result<Response> {
    let email = params.email.trim();
    tracing::debug!("auth::login entry");
    let user_agent = headers
        .get(USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());

    let runtime_ctx = ctx.runtime_ctx();
    let config = ctx
        .auth_config()
        .cloned()
        .ok_or(Error::InternalServerError)?;
    let (user, mut tokens) = AuthLifecycleService::login_runtime(
        runtime_ctx,
        &config,
        tenant.id,
        email,
        &params.password,
        Some(login_client_ip(&headers, addr.ip(), runtime_ctx.settings()).to_string()),
        user_agent,
        runtime_ctx.settings().features.email_verification,
    )
    .await
    .map_err(|e: AuthLifecycleError| Error::from(e))?;

    tokens
        .revalidate_for_publication(&config)
        .map_err(Error::from)?;
    let user_role = tokens.effective_role.clone();
    Ok(json_response(AuthResponse {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        token_type: "Bearer",
        expires_in: tokens.expires_in,
        user: user_info_from_model(user, user_role, tokens.permissions.clone()),
    }))
}

fn parse_refresh_retry_key(
    headers: &axum::http::HeaderMap,
) -> Result<Option<rustok_auth::AuthRefreshRetryKey>> {
    rustok_auth::AuthRefreshRetryKey::from_header_values(
        headers
            .get_all(rustok_auth::REFRESH_RETRY_KEY_HEADER)
            .iter()
            .map(|value| value.as_bytes()),
    )
    .map_err(AuthLifecycleError::from)
    .map_err(Error::from)
}

fn parse_expected_session_id(headers: &axum::http::HeaderMap) -> Result<Option<uuid::Uuid>> {
    rustok_auth::parse_expected_session_id(
        headers
            .get_all(rustok_auth::EXPECTED_SESSION_ID_HEADER)
            .iter()
            .map(|value| value.as_bytes()),
    )
    .map_err(AuthLifecycleError::from)
    .map_err(Error::from)
}

#[utoipa::path(post, path = "/api/auth/refresh", tag = "auth", request_body = RefreshRequest,
    responses((status = 200, description = "Token refreshed", body = AuthResponse),(status = 401, description = "Invalid or expired refresh token"),(status = 409, description = "Refresh token was already rotated by another concurrent request")))]
async fn refresh(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    headers: axum::http::HeaderMap,
    Json(params): Json<RefreshRequest>,
) -> Result<Response> {
    tracing::debug!("auth::refresh entry");
    let retry_key = parse_refresh_retry_key(&headers)?;
    let expected_session_id = parse_expected_session_id(&headers)?;
    let runtime_ctx = ctx.runtime_ctx();
    let config = ctx
        .auth_config()
        .cloned()
        .ok_or(Error::InternalServerError)?;
    let (user, mut tokens) = AuthLifecycleService::refresh_runtime(
        runtime_ctx,
        &config,
        tenant.id,
        &params.refresh_token,
        runtime_ctx.settings().features.email_verification,
        retry_key.as_ref(),
        expected_session_id,
    )
    .await
    .map_err(|e: AuthLifecycleError| Error::from(e))?;

    tokens
        .revalidate_for_publication(&config)
        .map_err(Error::from)?;
    let user_role = tokens.effective_role.clone();
    Ok(json_response(AuthResponse {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        token_type: "Bearer",
        expires_in: tokens.expires_in,
        user: user_info_from_model(user, user_role, tokens.permissions.clone()),
    }))
}

#[utoipa::path(post, path = "/api/auth/logout", tag = "auth", request_body = RefreshRequest,
    responses((status = 200, description = "Logout successful", body = LogoutResponse),(status = 401, description = "Invalid or expired refresh token")))]
async fn logout(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    headers: axum::http::HeaderMap,
    Json(params): Json<RefreshRequest>,
) -> Result<Response> {
    tracing::debug!("auth::logout entry");
    let expected_session_id = parse_expected_session_id(&headers)?;
    AuthLifecycleService::logout_by_refresh_token_runtime(
        ctx.runtime_ctx(),
        tenant.id,
        &params.refresh_token,
        expected_session_id,
    )
    .await
    .map_err(|error: AuthLifecycleError| Error::from(error))?;

    Ok(json_response(LogoutResponse { status: "ok" }))
}

// ─── HttpOnly cookie sessions for first-party browsers ─────────────────────
//
// Browser clients (admin/storefront) must never hold the refresh token
// in JavaScript-readable storage: any XSS would become full session
// theft with a long-lived lineage. These endpoints keep the refresh
// token in an HttpOnly, path-scoped, SameSite=Lax cookie and return
// only the short-lived access token in the JSON body, which the
// client holds in memory. The cookie-authenticated state changers
// (refresh/logout) additionally require a custom request header as a
// CSRF barrier: cross-site forms cannot set it, and SameSite=Lax
// already withholds the cookie on cross-site POSTs.

const SESSION_COOKIE_NAME: &str = "rustok-session";
const SESSION_COOKIE_PATH: &str = "/api/auth/session";
const SESSION_REQUEST_HEADER: &str = "x-rustok-session-request";

fn session_cookie_is_secure(
    headers: &axum::http::HeaderMap,
    peer_ip: std::net::IpAddr,
    settings: &RustokSettings,
) -> bool {
    crate::common::secure_cookie_required(headers, Some(peer_ip), &settings.runtime.request_trust)
}

#[cfg(test)]
fn session_cookie_is_secure_for_environment(
    headers: &axum::http::HeaderMap,
    peer_ip: std::net::IpAddr,
    settings: &RustokSettings,
    production_mode: bool,
) -> bool {
    crate::common::secure_cookie_required_for_environment(
        headers,
        Some(peer_ip),
        &settings.runtime.request_trust,
        production_mode,
    )
}

fn build_session_cookie(refresh_token: &str, max_age_secs: u64, secure: bool) -> String {
    let mut cookie = format!(
        "{SESSION_COOKIE_NAME}={refresh_token}; Path={SESSION_COOKIE_PATH}; HttpOnly; SameSite=Lax; Max-Age={max_age_secs}"
    );
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

fn parse_session_cookie(
    headers: &axum::http::HeaderMap,
) -> Result<Option<rustok_auth::AuthRefreshCookie>> {
    let mut credential = None;
    for header in headers.get_all(axum::http::header::COOKIE).iter() {
        let value = header
            .to_str()
            .map_err(|_| Error::BadRequest("Invalid session Cookie header".into()))?;
        for part in value.split(';') {
            let Some((name, value)) = part.trim().split_once('=') else {
                if part.trim() == SESSION_COOKIE_NAME {
                    return Err(Error::BadRequest("Invalid session cookie".into()));
                }
                continue;
            };
            if name.trim() != SESSION_COOKIE_NAME {
                continue;
            }
            // Multiple cookies with the same name can originate from
            // different paths. Picking the first is order-dependent and
            // would let logout report success without revoking the real token.
            if credential.is_some() {
                return Err(Error::BadRequest("Ambiguous session cookie".into()));
            }
            credential = Some(
                rustok_auth::AuthRefreshCookie::parse(value)
                    .map_err(|_| Error::BadRequest("Invalid session cookie".into()))?,
            );
        }
    }
    Ok(credential)
}

fn reject_explicit_cookie_retry_key(headers: &axum::http::HeaderMap) -> Result<()> {
    if headers.contains_key(rustok_auth::REFRESH_RETRY_KEY_HEADER) {
        return Err(Error::BadRequest(
            "Cookie refresh owns its retry proof".into(),
        ));
    }
    Ok(())
}

fn require_session_request_header(headers: &axum::http::HeaderMap) -> Result<()> {
    let mut values = headers.get_all(SESSION_REQUEST_HEADER).iter();
    let first = values.next();
    if values.next().is_some() {
        return Err(Error::BadRequest("Ambiguous session request header".into()));
    }
    match first.and_then(|value| value.to_str().ok()) {
        Some("1") => Ok(()),
        _ => Err(Error::BadRequest("Missing session request header".into())),
    }
}

/// A successful cookie-session response must carry its host-generated
/// Set-Cookie header. Encoding failure is an internal error, not a successful
/// login/rotation with a missing refresh credential.
fn with_session_cookie(mut response: Response, cookie: &str) -> Result<Response> {
    let mut value = axum::http::HeaderValue::from_str(cookie).map_err(|_error| {
        tracing::error!(
            failure_code = "session_cookie_header_encoding_failed",
            "Session cookie header could not be encoded"
        );
        Error::InternalServerError
    })?;
    value.set_sensitive(true);
    response.headers_mut().insert(
        CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
        .headers_mut()
        .append(axum::http::header::SET_COOKIE, value);
    Ok(response)
}

fn cookie_session_response(
    user: users::Model,
    tokens: AuthTokens,
    cookie: String,
) -> Result<Response> {
    let user_info = user_info_from_model(
        user,
        tokens.effective_role.clone(),
        tokens.permissions.clone(),
    );
    let response = json_response(CookieSessionResponse {
        access_token: tokens.access_token,
        token_type: "Bearer",
        expires_in: tokens.expires_in,
        user: user_info,
    });
    with_session_cookie(response, &cookie)
}

#[utoipa::path(post, path = "/api/auth/session/login", tag = "auth", request_body = LoginParams,
    responses((status = 200, description = "Login successful; refresh token set as HttpOnly cookie", body = CookieSessionResponse),(status = 401, description = "Unauthorized")))]
async fn session_login(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: axum::http::HeaderMap,
    Json(params): Json<LoginParams>,
) -> Result<Response> {
    let email = params.email.trim();
    tracing::debug!("auth::session_login entry");
    let seed = rustok_auth::AuthRefreshCookie::generate_seed().map_err(AuthLifecycleError::from)?;
    let user_agent = headers
        .get(USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());

    let runtime_ctx = ctx.runtime_ctx();
    let config = ctx
        .auth_config()
        .cloned()
        .ok_or(Error::InternalServerError)?;
    let (user, mut tokens) = AuthLifecycleService::login_runtime(
        runtime_ctx,
        &config,
        tenant.id,
        email,
        &params.password,
        Some(login_client_ip(&headers, addr.ip(), runtime_ctx.settings()).to_string()),
        user_agent,
        runtime_ctx.settings().features.email_verification,
    )
    .await
    .map_err(|e: AuthLifecycleError| Error::from(e))?;

    let credential = rustok_auth::AuthRefreshCookie::new(&tokens.refresh_token, seed)
        .map_err(|_| Error::InternalServerError)?;
    tokens
        .revalidate_for_publication(&config)
        .map_err(Error::from)?;
    let cookie = build_session_cookie(
        &credential.cookie_value(),
        tokens.refresh_expires_in,
        session_cookie_is_secure(&headers, addr.ip(), runtime_ctx.settings()),
    );
    cookie_session_response(user, tokens, cookie)
}

#[utoipa::path(post, path = "/api/auth/session/register", tag = "auth", request_body = RegisterParams,
    responses((status = 200, description = "Registration successful; refresh token set as HttpOnly cookie", body = CookieSessionResponse),(status = 400, description = "Email already exists")))]
async fn session_register(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: axum::http::HeaderMap,
    request_context: RequestContext,
    Json(params): Json<RegisterParams>,
) -> Result<Response> {
    let email = params.email.trim();
    tracing::debug!("auth::session_register entry");
    ensure_registration_enabled(ctx.runtime_ctx().settings())?;
    let seed = rustok_auth::AuthRefreshCookie::generate_seed().map_err(AuthLifecycleError::from)?;

    let runtime_ctx = ctx.runtime_ctx();
    let config = ctx
        .auth_config()
        .cloned()
        .ok_or(Error::InternalServerError)?;
    let (user, mut tokens) = AuthLifecycleService::register_runtime(
        runtime_ctx,
        &config,
        tenant.id,
        email,
        &params.password,
        params.name,
    )
    .await
    .map_err(|e: AuthLifecycleError| Error::from(e))?;

    if runtime_ctx.settings().features.email_verification {
        spawn_email_verification(runtime_ctx, &config, &user, request_context.locale.as_str());
    }

    let credential = rustok_auth::AuthRefreshCookie::new(&tokens.refresh_token, seed)
        .map_err(|_| Error::InternalServerError)?;
    tokens
        .revalidate_for_publication(&config)
        .map_err(Error::from)?;
    let cookie = build_session_cookie(
        &credential.cookie_value(),
        tokens.refresh_expires_in,
        session_cookie_is_secure(&headers, addr.ip(), runtime_ctx.settings()),
    );
    cookie_session_response(user, tokens, cookie)
}

#[utoipa::path(post, path = "/api/auth/session/refresh", tag = "auth",
    responses((status = 200, description = "Session refreshed; cookie rotated", body = CookieSessionResponse),(status = 401, description = "Invalid or expired session cookie"),(status = 409, description = "Refresh token was already rotated by another concurrent request")))]
async fn session_refresh(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: axum::http::HeaderMap,
) -> Result<Response> {
    tracing::debug!("auth::session_refresh entry");
    require_session_request_header(&headers)?;
    reject_explicit_cookie_retry_key(&headers)?;
    let expected_session_id = parse_expected_session_id(&headers)?;
    let credential = parse_session_cookie(&headers)?
        .ok_or_else(|| Error::Unauthorized("Invalid refresh token".into()))?;
    let retry_key = credential
        .retry_key(tenant.id)
        .map_err(AuthLifecycleError::from)?;

    let runtime_ctx = ctx.runtime_ctx();
    let config = ctx
        .auth_config()
        .cloned()
        .ok_or(Error::InternalServerError)?;
    let (user, mut tokens) = AuthLifecycleService::refresh_runtime(
        runtime_ctx,
        &config,
        tenant.id,
        credential.token(),
        runtime_ctx.settings().features.email_verification,
        Some(&retry_key),
        expected_session_id,
    )
    .await
    .map_err(|e: AuthLifecycleError| Error::from(e))?;

    let rotated = credential
        .rotated(&tokens.refresh_token)
        .map_err(|_| Error::InternalServerError)?;
    tokens
        .revalidate_for_publication(&config)
        .map_err(Error::from)?;
    let cookie = build_session_cookie(
        &rotated.cookie_value(),
        tokens.refresh_expires_in,
        session_cookie_is_secure(&headers, addr.ip(), runtime_ctx.settings()),
    );
    cookie_session_response(user, tokens, cookie)
}

fn cookie_logout_revocation_result(
    result: std::result::Result<(), AuthLifecycleError>,
) -> Result<()> {
    match result {
        Ok(())
        | Err(AuthLifecycleError::InvalidRefreshToken | AuthLifecycleError::SessionExpired) => {
            Ok(())
        }
        Err(error) => Err(Error::from(error)),
    }
}

#[utoipa::path(post, path = "/api/auth/session/logout", tag = "auth",
    responses((status = 200, description = "Logout successful; session cookie cleared", body = LogoutResponse)))]
async fn session_logout(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: axum::http::HeaderMap,
) -> Result<Response> {
    tracing::debug!("auth::session_logout entry");
    require_session_request_header(&headers)?;

    // An absent or already-revoked cookie is an idempotent logout.
    // A persistence or event-publication failure is not: the refresh token
    // might still be live, so never report a successful server-side logout.
    let expected_session_id = parse_expected_session_id(&headers)?;
    if let Some(credential) = parse_session_cookie(&headers)? {
        let revocation = AuthLifecycleService::logout_by_refresh_token_runtime(
            ctx.runtime_ctx(),
            tenant.id,
            credential.token(),
            expected_session_id,
        )
        .await;
        cookie_logout_revocation_result(revocation)?;
    } else if expected_session_id.is_some() {
        // Absence cannot confirm revocation of an explicitly expected lineage.
        return Err(Error::from(AuthLifecycleError::SessionLineageMismatch));
    }

    let cookie = build_session_cookie(
        "",
        0,
        session_cookie_is_secure(&headers, addr.ip(), ctx.runtime_ctx().settings()),
    );
    with_session_cookie(json_response(LogoutResponse { status: "ok" }), &cookie)
}

#[utoipa::path(get, path = "/api/auth/me", tag = "auth", security(("bearer_auth" = [])),
    responses((status = 200, description = "Current user info", body = UserResponse),(status = 401, description = "Unauthorized")))]
async fn me(
    CurrentUser {
        user,
        inferred_role,
        permissions,
        ..
    }: CurrentUser,
) -> Result<Response> {
    tracing::debug!(user_id = %user.id, "auth::me entry");
    Ok(json_response(user_response_from_model(
        user,
        inferred_role,
        permission_strings(&permissions),
    )))
}

pub(crate) const INVITE_ACCEPT_PATH: &str = "/api/auth/invite/accept";
const MAX_INVITE_REQUEST_BYTES: usize = 64 * 1024;

fn invite_request_error(status: StatusCode, code: &'static str, message: &'static str) -> Error {
    rustok_web::HttpError::new(status, code, message).into()
}

async fn decode_invite_request(request: Request) -> Result<AcceptInviteParams> {
    let unsupported_media = || {
        invite_request_error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
            "Invite requests require a single JSON Content-Type",
        )
    };
    let invalid_json = || {
        invite_request_error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Invite request must be a valid JSON object",
        )
    };

    if request.headers().get_all(CONTENT_TYPE).iter().count() != 1 {
        return Err(unsupported_media());
    }

    // Preserve the smaller legacy invite limit, independent of Axum's
    // general JSON body limit and any enclosing router configuration.
    let (parts, body) = request.into_parts();
    let bytes = to_bytes(body, MAX_INVITE_REQUEST_BYTES)
        .await
        .map_err(|_| {
            invite_request_error(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "Invite request body is invalid or too large",
            )
        })?;
    let is_object = bytes
        .iter()
        .copied()
        .find(|byte| !matches!(*byte, b' ' | b'\n' | b'\r' | b'\t'))
        == Some(b'{');

    // Use the same media-type and JSON decoder as the other Auth routes.
    // Do not expose serde diagnostics containing caller-supplied credentials.
    let Json(params) = Json::<AcceptInviteParams>::from_request(
        Request::from_parts(parts, Body::from(bytes)),
        &(),
    )
    .await
    .map_err(|rejection| {
        if rejection.status() == StatusCode::UNSUPPORTED_MEDIA_TYPE {
            unsupported_media()
        } else {
            invalid_json()
        }
    })?;
    // Serde structs may deserialize positional arrays; the REST DTO is an object.
    if !is_object {
        return Err(invalid_json());
    }
    Ok(params)
}

#[utoipa::path(post, path = "/api/auth/invite/accept", tag = "auth", request_body = AcceptInviteParams,
    responses((status = 200, description = "Invite accepted", body = InviteAcceptResponse),
    (status = 400, description = "Invalid or oversized invite request"),
    (status = 401, description = "Invalid or expired invite token"),
    (status = 415, description = "Missing, ambiguous or unsupported JSON media type")))]
pub(crate) async fn accept_invite(
    State(ctx): State<ServerAuthRuntime>,
    request: Request,
) -> Result<Response> {
    tracing::debug!("auth::accept_invite entry");
    let tenant_id = request
        .extensions()
        .tenant_context()
        .map(|tenant| tenant.id)
        .ok_or_else(|| {
            invite_request_error(
                StatusCode::NOT_FOUND,
                "tenant_not_found",
                "Tenant context is required",
            )
        })?;
    let params = decode_invite_request(request).await?;
    let config = ctx
        .auth_config()
        .cloned()
        .ok_or(Error::InternalServerError)?;
    let accepted = AuthLifecycleService::accept_invite_once_runtime(
        ctx.runtime_ctx(),
        &config,
        tenant_id,
        &params.token,
        &params.password,
        params.name,
    )
    .await
    .map_err(Error::from)?;

    Ok(json_response(InviteAcceptResponse {
        status: "ok",
        email: accepted.email,
        role: accepted.role,
    }))
}

#[utoipa::path(post, path = "/api/auth/reset/request", tag = "auth", request_body = RequestResetParams,
    responses((status = 200, description = "Reset request accepted", body = ResetRequestResponse)))]
async fn request_reset(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    request_context: RequestContext,
    Json(params): Json<RequestResetParams>,
) -> Result<Response> {
    tracing::debug!("auth::request_reset entry");
    let config = ctx
        .auth_config()
        .cloned()
        .ok_or(Error::InternalServerError)?;
    AuthLifecycleService::request_password_reset_runtime(
        ctx.runtime_ctx(),
        &config,
        tenant.id,
        &params.email,
        request_context.locale.as_str(),
    )
    .await
    .map_err(Error::from)?;

    Ok(json_response(ResetRequestResponse { status: "ok" }))
}

#[utoipa::path(post, path = "/api/auth/reset/confirm", tag = "auth", request_body = ConfirmResetParams,
    responses((status = 200, description = "Password updated", body = GenericStatusResponse),(status = 401, description = "Invalid token")))]
async fn confirm_reset(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    Json(params): Json<ConfirmResetParams>,
) -> Result<Response> {
    tracing::debug!("auth::confirm_reset entry");
    let runtime_ctx = ctx.runtime_ctx();
    let config = ctx
        .auth_config()
        .cloned()
        .ok_or(Error::InternalServerError)?;
    AuthLifecycleService::confirm_bound_password_reset_runtime(
        runtime_ctx,
        &config,
        tenant.id,
        &params.token,
        &params.password,
    )
    .await
    .map_err(|e: AuthLifecycleError| Error::from(e))?;

    Ok(json_response(GenericStatusResponse { status: "ok" }))
}

// Like password recovery, account-dependent token preparation must not
// change the public response. Keep diagnostics fixed and never log the
// recipient, token, or signing error payload.
fn prepare_verification_token(
    config: &crate::auth::AuthConfig,
    tenant_id: uuid::Uuid,
    user: Option<users::Model>,
) -> Option<(String, String)> {
    let user = user.filter(|record| {
        record.tenant_id == tenant_id && record.is_active() && record.email_verified_at.is_none()
    })?;
    match encode_email_verification_token(config, &user, EMAIL_VERIFICATION_TOKEN_TTL_SECS) {
        Ok(token) => Some((user.email, token)),
        Err(_) => {
            tracing::warn!(
                "Email verification token could not be prepared; keeping the public response generic"
            );
            None
        }
    }
}

#[utoipa::path(post, path = "/api/auth/verify/request", tag = "auth", request_body = RequestVerificationParams,
    responses((status = 200, description = "Verification request accepted", body = VerificationRequestResponse)))]
async fn request_verification(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    request_context: RequestContext,
    Json(params): Json<RequestVerificationParams>,
) -> Result<Response> {
    let email = params.email.trim();
    tracing::debug!("auth::request_verification entry");
    let settings = ctx.runtime_ctx().settings();
    if !settings.features.email_verification {
        return Ok(json_response(VerificationRequestResponse { status: "ok" }));
    }

    let config = ctx
        .auth_config()
        .cloned()
        .ok_or(Error::InternalServerError)?;
    rustok_auth::application_clock::admit_now(ctx.runtime_ctx().db()).await?;
    let user = Users::find_by_email(ctx.runtime_ctx().db(), tenant.id, email).await?;
    crate::auth::mask_auth_lookup_timing().await?;

    if let Some((recipient, verification_token_value)) =
        prepare_verification_token(&config, tenant.id, user)
    {
        let runtime_ctx = ctx.runtime_ctx();
        let email_service = match email_service_from_ctx(
            runtime_ctx,
            request_context.locale.as_str(),
        ) {
            Ok(service) => service,
            Err(_) => {
                tracing::warn!(
                    "Email verification could not be prepared; keeping the public response generic"
                );
                return Ok(json_response(VerificationRequestResponse { status: "ok" }));
            }
        };
        let verification_token = verification_token_value;

        let _email_task = spawn_email_verification_send(
            runtime_ctx,
            email_service,
            recipient,
            verification_token,
            &config,
        );
    }

    Ok(json_response(VerificationRequestResponse { status: "ok" }))
}

#[utoipa::path(post, path = "/api/auth/verify/confirm", tag = "auth", request_body = ConfirmVerificationParams,
    responses((status = 200, description = "Email verified", body = GenericStatusResponse),(status = 401, description = "Invalid token")))]
async fn confirm_verification(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    Json(params): Json<ConfirmVerificationParams>,
) -> Result<Response> {
    tracing::debug!("auth::confirm_verification entry");
    let config = ctx
        .auth_config()
        .cloned()
        .ok_or(Error::InternalServerError)?;
    AuthLifecycleService::confirm_email_verification_runtime(
        ctx.runtime_ctx(),
        &config,
        tenant.id,
        &params.token,
    )
    .await
    .map_err(|error: AuthLifecycleError| Error::from(error))?;

    Ok(json_response(GenericStatusResponse { status: "ok" }))
}

#[utoipa::path(get, path = "/api/auth/sessions", tag = "auth", security(("bearer_auth" = [])),
    params(
        ("limit" = Option<u64>, Query, description = "Maximum number of sessions to return (1-100)")
    ),
    responses((status = 200, description = "Active sessions", body = SessionsResponse)))]
async fn list_sessions(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Query(params): Query<SessionListParams>,
) -> Result<Response> {
    tracing::debug!(user_id = %current.user.id, "auth::list_sessions entry");
    let requested_limit = params.limit;
    let limit = clamp_session_limit(params.limit);

    let rows = AuthLifecycleService::list_sessions_runtime(
        ctx.runtime_ctx(),
        tenant.id,
        current.user.id,
        limit,
    )
    .await
    .map_err(|error: AuthLifecycleError| Error::from(error))?;

    metrics::record_read_path_budget(
        "http",
        "auth.list_sessions",
        requested_limit,
        limit,
        rows.len(),
    );

    let data = rows
        .into_iter()
        .map(|item| SessionItem {
            id: item.id,
            ip_address: item.ip_address,
            user_agent: item.user_agent,
            last_used_at: item.last_used_at.map(|value| value.into()),
            expires_at: item.expires_at.into(),
            created_at: item.created_at.into(),
            current: item.id == current.session_id,
        })
        .collect();

    Ok(json_response(SessionsResponse { sessions: data }))
}
#[utoipa::path(post, path = "/api/auth/sessions/revoke-all", tag = "auth", security(("bearer_auth" = [])),
    responses((status = 200, description = "Sessions revoked", body = GenericStatusResponse)))]
async fn revoke_all_sessions(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
) -> Result<Response> {
    tracing::debug!(user_id = %current.user.id, "auth::revoke_all_sessions entry");
    AuthLifecycleService::revoke_all_other_sessions_runtime(
        ctx.runtime_ctx(),
        tenant.id,
        current.user.id,
        current.session_id,
    )
    .await
    .map_err(|error: AuthLifecycleError| Error::from(error))?;

    Ok(json_response(GenericStatusResponse { status: "ok" }))
}
#[utoipa::path(post, path = "/api/auth/change-password", tag = "auth", security(("bearer_auth" = [])), request_body = ChangePasswordParams,
    responses((status = 200, description = "Password changed", body = GenericStatusResponse),(status = 401, description = "Invalid credentials")))]
async fn change_password(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Json(params): Json<ChangePasswordParams>,
) -> Result<Response> {
    tracing::debug!(user_id = %current.user.id, "auth::change_password entry");
    let runtime_ctx = ctx.runtime_ctx();
    let config = ctx
        .auth_config()
        .cloned()
        .ok_or(Error::InternalServerError)?;
    AuthLifecycleService::change_password_runtime(
        runtime_ctx,
        tenant.id,
        current.user.id,
        current.session_id,
        &params.current_password,
        &params.new_password,
        config.min_password_length,
    )
    .await
    .map_err(|e: AuthLifecycleError| Error::from(e))?;

    Ok(json_response(GenericStatusResponse { status: "ok" }))
}

#[utoipa::path(post, path = "/api/auth/profile", tag = "auth", security(("bearer_auth" = [])), request_body = UpdateProfileParams,
    responses((status = 200, description = "Profile updated", body = UserResponse)))]
async fn update_profile(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Json(params): Json<UpdateProfileParams>,
) -> Result<Response> {
    tracing::debug!(user_id = %current.user.id, "auth::update_profile entry");
    let runtime_ctx = ctx.runtime_ctx();
    let user = AuthLifecycleService::update_profile_runtime(
        runtime_ctx,
        tenant.id,
        current.user.id,
        params.name,
    )
    .await
    .map_err(|e: AuthLifecycleError| Error::from(e))?;

    Ok(json_response(user_response_from_model(
        user,
        current.inferred_role,
        permission_strings(&current.permissions),
    )))
}

#[utoipa::path(get, path = "/api/auth/history", tag = "auth", security(("bearer_auth" = [])),
    params(
        ("limit" = Option<u64>, Query, description = "Maximum number of history entries to return (1-100)")
    ),
    responses((status = 200, description = "Login history", body = SessionsResponse)))]
async fn login_history(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Query(params): Query<SessionListParams>,
) -> Result<Response> {
    tracing::debug!(user_id = %current.user.id, "auth::login_history entry");
    let requested_limit = params.limit;
    let limit = clamp_session_limit(params.limit);

    let rows = sessions::Entity::find()
        .filter(sessions::Column::TenantId.eq(tenant.id))
        .filter(sessions::Column::UserId.eq(current.user.id))
        .order_by_desc(sessions::Column::CreatedAt)
        .limit(limit)
        .all(ctx.runtime_ctx().db())
        .await?;

    metrics::record_read_path_budget(
        "http",
        "auth.login_history",
        requested_limit,
        limit,
        rows.len(),
    );

    let data = rows
        .into_iter()
        .map(|item| SessionItem {
            id: item.id,
            ip_address: item.ip_address,
            user_agent: item.user_agent,
            last_used_at: item.last_used_at.map(|value| value.into()),
            expires_at: item.expires_at.into(),
            created_at: item.created_at.into(),
            current: item.id == current.session_id,
        })
        .collect();

    Ok(json_response(SessionsResponse { sessions: data }))
}

#[utoipa::path(delete, path = "/api/auth/sessions/{id}", tag = "auth", security(("bearer_auth" = [])),
    params(("id" = uuid::Uuid, Path, description = "Session ID to revoke")),
    responses(
        (status = 200, description = "Session revoked", body = GenericStatusResponse),
        (status = 404, description = "Session not found or already revoked")
    ))]
async fn revoke_session(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path(session_id): Path<uuid::Uuid>,
) -> Result<Response> {
    tracing::debug!(user_id = %current.user.id, %session_id, "auth::revoke_session entry");
    let runtime_ctx = ctx.runtime_ctx();
    let revoked = AuthLifecycleService::revoke_session_runtime(
        runtime_ctx,
        tenant.id,
        current.user.id,
        session_id,
    )
    .await
    .map_err(|e: AuthLifecycleError| Error::from(e))?;

    if revoked {
        Ok(json_response(GenericStatusResponse { status: "ok" }))
    } else {
        Err(Error::NotFound)
    }
}

/// Credential-bearing responses must never be stored by browsers or shared
/// intermediaries, including on error paths.
///
/// `/api/auth/login`, `/refresh`, `/session/*` and `/reset/confirm` return access
/// and refresh tokens in the body (and rotate them into a cookie), so a shared
/// cache that stored one of those responses could replay another user's
/// credentials. The OAuth endpoints already enforce this via their own
/// `disable_oauth_caching` layer; the auth surface needs the same guarantee.
async fn disable_auth_response_caching(request: axum::extract::Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response.headers_mut().insert(
        axum::http::header::PRAGMA,
        axum::http::HeaderValue::from_static("no-cache"),
    );
    response
}

pub fn router() -> crate::routes::ServerRouter {
    axum::Router::new()
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/auth/refresh", post(refresh))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/session/login", post(session_login))
        .route("/api/auth/session/register", post(session_register))
        .route("/api/auth/session/refresh", post(session_refresh))
        .route("/api/auth/session/logout", post(session_logout))
        .route("/api/auth/me", get(me))
        .route(INVITE_ACCEPT_PATH, post(accept_invite))
        .route("/api/auth/reset/request", post(request_reset))
        .route("/api/auth/reset/confirm", post(confirm_reset))
        .route("/api/auth/verify/request", post(request_verification))
        .route("/api/auth/verify/confirm", post(confirm_verification))
        .route("/api/auth/sessions", get(list_sessions))
        .route("/api/auth/sessions/revoke-all", post(revoke_all_sessions))
        .route("/api/auth/sessions/{id}", delete(revoke_session))
        .route("/api/auth/change-password", post(change_password))
        .route("/api/auth/profile", post(update_profile))
        .route("/api/auth/history", get(login_history))
        .layer(axum::middleware::from_fn(disable_auth_response_caching))
}

fn clamp_session_limit(limit: Option<u64>) -> u64 {
    limit.unwrap_or(50).clamp(1, 100)
}
#[cfg(test)]
mod tests {
    use super::{disable_auth_response_caching, ensure_registration_enabled};
    use crate::common::RustokSettings;

    #[tokio::test]
    async fn invite_decoder_rejects_non_json_ambiguous_and_oversized_requests() {
        use axum::{
            body::Body,
            http::{Request, StatusCode, header::CONTENT_TYPE},
            response::IntoResponse,
        };

        let valid = r#"{"token":"test-token","password":"test-password","name":null}"#;
        let request = |content_type: Option<&str>, body: String| {
            let mut builder = Request::builder();
            if let Some(content_type) = content_type {
                builder = builder.header(CONTENT_TYPE, content_type);
            }
            builder.body(Body::from(body)).unwrap()
        };
        let params = super::decode_invite_request(request(
            Some("application/json; charset=utf-8"),
            valid.to_string(),
        ))
        .await
        .expect("valid invite object");
        assert_eq!(params.token, "test-token");

        for content_type in [None, Some("text/plain")] {
            let error = super::decode_invite_request(request(content_type, valid.to_string()))
                .await
                .err()
                .expect("unsupported media type");
            assert_eq!(
                error.into_response().status(),
                StatusCode::UNSUPPORTED_MEDIA_TYPE
            );
        }

        for second in ["application/json", "text/plain"] {
            let mut duplicate = request(Some("application/json"), valid.to_string());
            duplicate
                .headers_mut()
                .append(CONTENT_TYPE, second.parse().unwrap());
            let error = super::decode_invite_request(duplicate)
                .await
                .err()
                .expect("ambiguous media type");
            assert_eq!(
                error.into_response().status(),
                StatusCode::UNSUPPORTED_MEDIA_TYPE
            );
        }

        for body in [
            r#"["test-token","test-password",null]"#.to_string(),
            r#"{"token":"first","token":"second","password":"test-password"}"#.to_string(),
            "{".to_string(),
            format!("{valid}{}", " ".repeat(super::MAX_INVITE_REQUEST_BYTES)),
        ] {
            let error = super::decode_invite_request(request(Some("application/json"), body))
                .await
                .err()
                .expect("invalid invite body");
            assert_eq!(error.into_response().status(), StatusCode::BAD_REQUEST);
        }
    }

    #[tokio::test]
    async fn verification_request_masks_account_dependent_signing_failure() {
        use crate::models::{tenants, users};
        use rustok_migrations::SqliteTestMigrator;
        use rustok_test_utils::db::setup_test_db_with_migrations;
        use sea_orm::ActiveModelTrait;

        let db = setup_test_db_with_migrations::<SqliteTestMigrator>().await;
        let tenant = tenants::ActiveModel::new("Verification tenant", "verify-signing")
            .insert(&db)
            .await
            .unwrap();
        let user = users::ActiveModel::new(tenant.id, "verify@example.com", "hash")
            .insert(&db)
            .await
            .unwrap();
        let config = crate::auth::test_auth_config(
            "verification-request-signing-failure-test-secret".to_string(),
        );
        let (recipient, token) =
            super::prepare_verification_token(&config, tenant.id, Some(user.clone()))
                .expect("healthy signer prepares a verification token");
        assert_eq!(recipient, user.email);
        assert!(!token.is_empty());

        let (unleased, _publisher) = config
            .into_cluster_jwt(1)
            .expect("prepare runtime without granting a signing lease");
        assert!(
            crate::auth::encode_email_verification_token(
                &unleased,
                &user,
                super::EMAIL_VERIFICATION_TOKEN_TTL_SECS,
            )
            .is_err()
        );
        assert!(super::prepare_verification_token(&unleased, tenant.id, Some(user)).is_none());
        assert!(super::prepare_verification_token(&unleased, tenant.id, None).is_none());
    }

    #[test]
    fn registration_policy_respects_feature_flag() {
        let enabled = RustokSettings::default();
        assert!(ensure_registration_enabled(&enabled).is_ok());

        let mut disabled = RustokSettings::default();
        disabled.features.registration_enabled = false;
        assert!(matches!(
            ensure_registration_enabled(&disabled),
            Err(crate::error::Error::Forbidden(message)) if message == "Registration is disabled"
        ));
    }

    #[tokio::test]
    async fn auth_responses_are_never_cacheable() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode, header::CACHE_CONTROL};
        use tower::ServiceExt;

        // Success, redirect and error paths all carry credentials or credential
        // material; none of them may be stored by a browser or shared cache.
        let router = axum::Router::new()
            .route("/ok", axum::routing::get(|| async { StatusCode::OK }))
            .route(
                "/redirect",
                axum::routing::get(|| async { StatusCode::FOUND }),
            )
            .route(
                "/error",
                axum::routing::get(|| async { StatusCode::BAD_REQUEST }),
            )
            .layer(axum::middleware::from_fn(disable_auth_response_caching));

        for path in ["/ok", "/redirect", "/error"] {
            let response = router
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .expect("auth response");
            assert_eq!(
                response.headers().get(CACHE_CONTROL).unwrap(),
                "no-store",
                "{path} must not be cacheable"
            );
            assert_eq!(
                response.headers().get(axum::http::header::PRAGMA).unwrap(),
                "no-cache",
                "{path} must not be cacheable by HTTP/1.0 intermediaries"
            );
        }
    }

    #[test]
    fn production_security_mode_forces_secure_refresh_cookies_without_forwarded_headers() {
        let settings = RustokSettings::default();
        let headers = axum::http::HeaderMap::new();
        let peer = std::net::IpAddr::from([192, 0, 2, 4]);

        assert!(crate::common::secure_cookie_required_for_environment(
            &headers,
            Some(peer),
            &settings.runtime.request_trust,
            true,
        ));
        assert!(!crate::common::secure_cookie_required_for_environment(
            &headers,
            Some(peer),
            &settings.runtime.request_trust,
            false,
        ));
    }

    #[test]
    fn production_refresh_cookie_is_always_secure_without_forwarded_headers() {
        let settings = RustokSettings::default();
        let headers = axum::http::HeaderMap::new();
        let peer = std::net::IpAddr::from([192, 0, 2, 4]);

        assert!(super::session_cookie_is_secure_for_environment(
            &headers, peer, &settings, true,
        ));
        assert!(!super::session_cookie_is_secure_for_environment(
            &headers, peer, &settings, false,
        ));
        assert!(super::build_session_cookie("opaque-token", 60, true).contains("; Secure"));
        assert!(!super::build_session_cookie("opaque-token", 60, false).contains("; Secure"));
    }

    #[test]
    fn trusted_https_proxy_keeps_refresh_cookie_secure_in_development() {
        use crate::common::{ForwardedHeadersMode, RequestTrustSettings};

        let mut settings = RustokSettings::default();
        settings.runtime.request_trust = RequestTrustSettings {
            forwarded_headers_mode: ForwardedHeadersMode::TrustedOnly,
            trusted_proxy_cidrs: vec!["10.0.0.0/8".to_string()],
        };
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("x-forwarded-proto", "http, https".parse().unwrap());
        let trusted_peer = std::net::IpAddr::from([10, 0, 0, 4]);
        let untrusted_peer = std::net::IpAddr::from([192, 0, 2, 4]);

        assert!(super::session_cookie_is_secure_for_environment(
            &headers,
            trusted_peer,
            &settings,
            false,
        ));
        assert!(!super::session_cookie_is_secure_for_environment(
            &headers,
            untrusted_peer,
            &settings,
            false,
        ));
    }

    #[test]
    fn login_audit_ip_uses_only_a_verified_forwarded_client() {
        use crate::common::{ForwardedHeadersMode, RequestTrustSettings};

        let mut settings = RustokSettings::default();
        settings.runtime.request_trust = RequestTrustSettings {
            forwarded_headers_mode: ForwardedHeadersMode::TrustedOnly,
            trusted_proxy_cidrs: vec!["10.0.0.0/8".to_owned()],
        };
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            "198.51.100.44, 203.0.113.9".parse().unwrap(),
        );
        let trusted_peer = std::net::IpAddr::from([10, 0, 0, 4]);
        let untrusted_peer = std::net::IpAddr::from([192, 0, 2, 4]);

        assert_eq!(
            super::login_client_ip(&headers, trusted_peer, &settings),
            std::net::IpAddr::from([203, 0, 113, 9]),
        );
        assert_eq!(
            super::login_client_ip(&headers, untrusted_peer, &settings),
            untrusted_peer,
        );

        settings.runtime.request_trust = RequestTrustSettings::default();
        assert_eq!(
            super::login_client_ip(&headers, trusted_peer, &settings),
            trusted_peer,
        );
    }

    #[test]
    fn session_cookie_encoding_failure_cannot_report_success() {
        use axum::http::{StatusCode, header::SET_COOKIE};
        use axum::response::IntoResponse;

        let invalid = super::build_session_cookie("bad\r\ntoken", 60, true);
        let result = super::with_session_cookie(StatusCode::OK.into_response(), &invalid);
        assert!(matches!(
            result,
            Err(crate::error::Error::InternalServerError)
        ));

        for (token, max_age) in [("valid-token", 60), ("", 0)] {
            let cookie = super::build_session_cookie(token, max_age, true);
            let response = super::with_session_cookie(StatusCode::OK.into_response(), &cookie)
                .expect("valid cookie must be returned");
            let header = response.headers().get(SET_COOKIE).expect("Set-Cookie");
            assert_eq!(header.to_str().unwrap(), cookie.as_str());
            assert!(header.is_sensitive());
        }
    }

    #[test]
    fn session_cookie_uses_exact_server_reported_refresh_lifetime() {
        let cookie = super::build_session_cookie("token", 10 * 24 * 3600, true);
        assert!(cookie.contains("Max-Age=864000"));
        assert!(cookie.contains("; Secure"));
        assert!(!cookie.contains("Max-Age=7776000"));
    }

    #[test]
    fn session_request_header_requires_one_exact_marker() {
        use axum::http::{HeaderMap, HeaderValue};

        let mut headers = HeaderMap::new();
        assert!(super::require_session_request_header(&headers).is_err());

        for marker in ["0", "1, 1", ""] {
            headers.insert(
                super::SESSION_REQUEST_HEADER,
                HeaderValue::from_static(marker),
            );
            assert!(super::require_session_request_header(&headers).is_err());
        }

        headers.insert(super::SESSION_REQUEST_HEADER, HeaderValue::from_static("1"));
        assert!(super::require_session_request_header(&headers).is_ok());
        headers.append(super::SESSION_REQUEST_HEADER, HeaderValue::from_static("1"));
        assert!(super::require_session_request_header(&headers).is_err());

        for (first, second) in [("1", "0"), ("0", "1")] {
            headers.insert(
                super::SESSION_REQUEST_HEADER,
                HeaderValue::from_static(first),
            );
            headers.append(
                super::SESSION_REQUEST_HEADER,
                HeaderValue::from_static(second),
            );
            assert!(super::require_session_request_header(&headers).is_err());
        }
    }

    #[test]
    fn session_cookie_parser_rejects_duplicate_names_instead_of_guessing() {
        use axum::http::{HeaderMap, HeaderValue, header};
        let token = "a1".repeat(32);
        let wire = format!("rustok-session={token}.{}", "b2".repeat(32));
        let mut headers = HeaderMap::new();
        headers.append(
            header::COOKIE,
            HeaderValue::from_str(&format!("other=1; {wire}")).unwrap(),
        );
        assert_eq!(
            super::parse_session_cookie(&headers)
                .unwrap()
                .unwrap()
                .token(),
            token
        );
        headers.append(header::COOKIE, HeaderValue::from_str(&wire).unwrap());
        assert!(matches!(
            super::parse_session_cookie(&headers),
            Err(super::Error::BadRequest(_))
        ));
        headers.clear();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("{wire}; {wire}")).unwrap(),
        );
        assert!(matches!(
            super::parse_session_cookie(&headers),
            Err(super::Error::BadRequest(_))
        ));
    }

    #[test]
    fn only_absent_cookie_is_idempotent_absence() {
        use axum::http::{HeaderMap, HeaderValue, header};
        let mut headers = HeaderMap::new();
        assert_eq!(super::parse_session_cookie(&headers).unwrap(), None);
        for malformed in [
            "rustok-session".to_string(),
            "rustok-session=".to_string(),
            format!("rustok-session={}", "a1".repeat(32)),
            format!("rustok-session={}.{}", "a1".repeat(32), "0".repeat(64)),
        ] {
            headers.insert(header::COOKIE, HeaderValue::from_str(&malformed).unwrap());
            assert!(super::parse_session_cookie(&headers).is_err());
        }
    }

    #[test]
    fn cookie_refresh_refuses_caller_supplied_proof() {
        use axum::http::{HeaderMap, HeaderValue};
        let mut headers = HeaderMap::new();
        assert!(super::reject_explicit_cookie_retry_key(&headers).is_ok());
        headers.insert(
            rustok_auth::REFRESH_RETRY_KEY_HEADER,
            HeaderValue::from_str(&"a1".repeat(32)).unwrap(),
        );
        assert!(super::reject_explicit_cookie_retry_key(&headers).is_err());
    }

    #[test]
    fn cookie_logout_never_confirms_a_lineage_mismatch() {
        assert!(matches!(
            super::cookie_logout_revocation_result(Err(
                super::AuthLifecycleError::SessionLineageMismatch
            )),
            Err(super::Error::Conflict(_))
        ));
    }

    #[test]
    fn cookie_logout_is_idempotent_but_fails_closed_on_revocation_outage() {
        assert!(super::cookie_logout_revocation_result(Ok(())).is_ok());
        assert!(
            super::cookie_logout_revocation_result(Err(
                super::AuthLifecycleError::InvalidRefreshToken
            ))
            .is_ok()
        );
        assert!(
            super::cookie_logout_revocation_result(Err(super::AuthLifecycleError::SessionExpired))
                .is_ok()
        );

        let result = super::cookie_logout_revocation_result(Err(
            super::AuthLifecycleError::Internal(crate::error::Error::InternalServerError),
        ));
        assert!(matches!(
            result,
            Err(crate::error::Error::InternalServerError)
        ));
    }

    #[test]
    fn clamp_session_limit_bounds() {
        assert_eq!(super::clamp_session_limit(None), 50);
        assert_eq!(super::clamp_session_limit(Some(0)), 1);
        assert_eq!(super::clamp_session_limit(Some(25)), 25);
        assert_eq!(super::clamp_session_limit(Some(100)), 100);
        assert_eq!(super::clamp_session_limit(Some(1000)), 100);
    }
}
