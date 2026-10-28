use crate::error::Error;
use crate::error::Result;
use axum::response::Response;
use axum::{
    Json,
    extract::State,
    extract::{ConnectInfo, Path, Query},
    http::header::USER_AGENT,
    routing::{delete, get, post},
};
use rustok_telemetry::metrics;
use rustok_web::json_response;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use std::net::SocketAddr;

use crate::auth::{encode_email_verification_token, encode_password_reset_token};
use crate::common::{RequestContext, RustokSettings};
use crate::extractors::{auth::CurrentUser, tenant::CurrentTenant};
use crate::models::{
    sessions,
    users::{self, Entity as Users},
};
use crate::services::auth_lifecycle::{AuthLifecycleError, AuthLifecycleService, AuthTokens};
use crate::services::email::{
    EMAIL_VERIFICATION_TOKEN_TTL_SECS, EmailVerificationEmail, PASSWORD_RESET_TOKEN_TTL_SECS,
    PasswordResetEmail, email_service_from_ctx, password_reset_url, spawn_email_verification,
};
use crate::services::server_runtime_context::ServerAuthRuntime;
pub use rustok_auth::{
    AcceptInviteParams, AuthResponse, ChangePasswordParams, ConfirmResetParams,
    ConfirmVerificationParams, GenericStatusResponse, InviteAcceptResponse, LoginParams,
    LogoutResponse, RefreshRequest, RegisterParams, RequestResetParams, RequestVerificationParams,
    ResetRequestResponse, SessionItem, SessionListParams, SessionsResponse, UpdateProfileParams,
    UserInfo, UserResponse, VerificationRequestResponse,
};

fn user_info_from_model(user: users::Model, role: rustok_core::UserRole) -> UserInfo {
    UserInfo {
        id: user.id,
        email: user.email,
        name: user.name,
        role,
        status: user.status,
    }
}

fn user_response_from_model(user: users::Model, role: rustok_core::UserRole) -> UserResponse {
    UserResponse {
        id: user.id,
        email: user.email,
        name: user.name,
        role: role.to_string(),
    }
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
    tracing::debug!(email, "auth::register entry");
    ensure_registration_enabled(ctx.runtime_ctx().settings())?;

    let runtime_ctx = ctx.runtime_ctx();
    let config = ctx
        .auth_config()
        .cloned()
        .ok_or(Error::InternalServerError)?;
    let (user, tokens) = AuthLifecycleService::register_runtime(
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
        spawn_email_verification(
            runtime_ctx,
            &config,
            tenant.id,
            user.id,
            &user.email,
            request_context.locale.as_str(),
        );
    }

    let user_role = tokens.effective_role.clone();
    Ok(json_response(AuthResponse {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        token_type: "Bearer",
        expires_in: tokens.expires_in,
        user: user_info_from_model(user, user_role),
    }))
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
    tracing::debug!(email, "auth::login entry");
    let user_agent = headers
        .get(USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());

    let runtime_ctx = ctx.runtime_ctx();
    let config = ctx
        .auth_config()
        .cloned()
        .ok_or(Error::InternalServerError)?;
    let (user, tokens) = AuthLifecycleService::login_runtime(
        runtime_ctx,
        &config,
        tenant.id,
        email,
        &params.password,
        Some(addr.ip().to_string()),
        user_agent,
        runtime_ctx.settings().features.email_verification,
    )
    .await
    .map_err(|e: AuthLifecycleError| Error::from(e))?;

    let user_role = tokens.effective_role.clone();
    Ok(json_response(AuthResponse {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        token_type: "Bearer",
        expires_in: tokens.expires_in,
        user: user_info_from_model(user, user_role),
    }))
}

#[utoipa::path(post, path = "/api/auth/refresh", tag = "auth", request_body = RefreshRequest,
    responses((status = 200, description = "Token refreshed", body = AuthResponse),(status = 401, description = "Invalid or expired refresh token")))]
async fn refresh(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    Json(params): Json<RefreshRequest>,
) -> Result<Response> {
    tracing::debug!("auth::refresh entry");
    let runtime_ctx = ctx.runtime_ctx();
    let config = ctx
        .auth_config()
        .cloned()
        .ok_or(Error::InternalServerError)?;
    let (user, tokens) = AuthLifecycleService::refresh_runtime(
        runtime_ctx,
        &config,
        tenant.id,
        &params.refresh_token,
        runtime_ctx.settings().features.email_verification,
    )
    .await
    .map_err(|e: AuthLifecycleError| Error::from(e))?;

    let user_role = tokens.effective_role.clone();
    Ok(json_response(AuthResponse {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        token_type: "Bearer",
        expires_in: tokens.expires_in,
        user: user_info_from_model(user, user_role),
    }))
}

#[utoipa::path(post, path = "/api/auth/logout", tag = "auth", request_body = RefreshRequest,
    responses((status = 200, description = "Logout successful", body = LogoutResponse),(status = 401, description = "Invalid or expired refresh token")))]
async fn logout(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    Json(params): Json<RefreshRequest>,
) -> Result<Response> {
    tracing::debug!("auth::logout entry");
    AuthLifecycleService::logout_by_refresh_token_runtime(
        ctx.runtime_ctx(),
        tenant.id,
        &params.refresh_token,
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

#[derive(serde::Serialize, utoipa::ToSchema)]
pub(crate) struct CookieSessionResponse {
    access_token: String,
    token_type: &'static str,
    expires_in: u64,
    user: UserInfo,
}

impl std::fmt::Debug for CookieSessionResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CookieSessionResponse")
            .field("access_token", &"<redacted>")
            .field("token_type", &self.token_type)
            .field("expires_in", &self.expires_in)
            .field("user", &self.user)
            .finish()
    }
}

fn session_cookie_is_secure(
    headers: &axum::http::HeaderMap,
    peer_ip: std::net::IpAddr,
    settings: &RustokSettings,
) -> bool {
    session_cookie_is_secure_for_environment(
        headers,
        peer_ip,
        settings,
        crate::common::is_production_environment(),
    )
}

fn session_cookie_is_secure_for_environment(
    headers: &axum::http::HeaderMap,
    peer_ip: std::net::IpAddr,
    settings: &RustokSettings,
    is_production: bool,
) -> bool {
    // A refresh token must never be issued as a non-Secure cookie in
    // production, even when an ingress proxy omits forwarding headers or
    // forwarded header trust is intentionally disabled.
    is_production
        || crate::common::extract_effective_proto(
            headers,
            Some(peer_ip),
            &settings.runtime.request_trust,
        )
        .is_some_and(|value| value.eq_ignore_ascii_case("https"))
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

fn parse_session_cookie(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| {
            let (name, value) = part.trim().split_once('=')?;
            (name == SESSION_COOKIE_NAME && !value.trim().is_empty())
                .then(|| value.trim().to_string())
        })
}

fn require_session_request_header(headers: &axum::http::HeaderMap) -> Result<()> {
    match headers
        .get(SESSION_REQUEST_HEADER)
        .and_then(|value| value.to_str().ok())
    {
        Some("1") => Ok(()),
        _ => Err(Error::BadRequest("Missing session request header".into())),
    }
}

fn cookie_session_response(user: users::Model, tokens: AuthTokens, cookie: String) -> Response {
    let user_info = user_info_from_model(user, tokens.effective_role.clone());
    let mut response = json_response(CookieSessionResponse {
        access_token: tokens.access_token,
        token_type: "Bearer",
        expires_in: tokens.expires_in,
        user: user_info,
    });
    match axum::http::HeaderValue::from_str(&cookie) {
        Ok(value) => {
            response
                .headers_mut()
                .append(axum::http::header::SET_COOKIE, value);
        }
        Err(error) => {
            tracing::error!(%error, "session cookie header could not be encoded");
        }
    }
    response
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
    tracing::debug!(email, "auth::session_login entry");
    let user_agent = headers
        .get(USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());

    let runtime_ctx = ctx.runtime_ctx();
    let config = ctx
        .auth_config()
        .cloned()
        .ok_or(Error::InternalServerError)?;
    let (user, tokens) = AuthLifecycleService::login_runtime(
        runtime_ctx,
        &config,
        tenant.id,
        email,
        &params.password,
        Some(addr.ip().to_string()),
        user_agent,
        runtime_ctx.settings().features.email_verification,
    )
    .await
    .map_err(|e: AuthLifecycleError| Error::from(e))?;

    let cookie = build_session_cookie(
        &tokens.refresh_token,
        config.refresh_expiration,
        session_cookie_is_secure(&headers, addr.ip(), runtime_ctx.settings()),
    );
    Ok(cookie_session_response(user, tokens, cookie))
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
    tracing::debug!(email, "auth::session_register entry");
    ensure_registration_enabled(ctx.runtime_ctx().settings())?;

    let runtime_ctx = ctx.runtime_ctx();
    let config = ctx
        .auth_config()
        .cloned()
        .ok_or(Error::InternalServerError)?;
    let (user, tokens) = AuthLifecycleService::register_runtime(
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
        spawn_email_verification(
            runtime_ctx,
            &config,
            tenant.id,
            user.id,
            &user.email,
            request_context.locale.as_str(),
        );
    }

    let cookie = build_session_cookie(
        &tokens.refresh_token,
        config.refresh_expiration,
        session_cookie_is_secure(&headers, addr.ip(), runtime_ctx.settings()),
    );
    Ok(cookie_session_response(user, tokens, cookie))
}

#[utoipa::path(post, path = "/api/auth/session/refresh", tag = "auth",
    responses((status = 200, description = "Session refreshed; cookie rotated", body = CookieSessionResponse),(status = 401, description = "Invalid or expired session cookie")))]
async fn session_refresh(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: axum::http::HeaderMap,
) -> Result<Response> {
    tracing::debug!("auth::session_refresh entry");
    require_session_request_header(&headers)?;
    let refresh_token = parse_session_cookie(&headers)
        .ok_or_else(|| Error::Unauthorized("Invalid refresh token".into()))?;

    let runtime_ctx = ctx.runtime_ctx();
    let config = ctx
        .auth_config()
        .cloned()
        .ok_or(Error::InternalServerError)?;
    let (user, tokens) = AuthLifecycleService::refresh_runtime(
        runtime_ctx,
        &config,
        tenant.id,
        &refresh_token,
        runtime_ctx.settings().features.email_verification,
    )
    .await
    .map_err(|e: AuthLifecycleError| Error::from(e))?;

    let cookie = build_session_cookie(
        &tokens.refresh_token,
        config.refresh_expiration,
        session_cookie_is_secure(&headers, addr.ip(), runtime_ctx.settings()),
    );
    Ok(cookie_session_response(user, tokens, cookie))
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

    // Idempotent: an absent or already-dead cookie still results in a
    // cleared cookie and a successful logout response.
    if let Some(refresh_token) = parse_session_cookie(&headers) {
        if let Err(error) = AuthLifecycleService::logout_by_refresh_token_runtime(
            ctx.runtime_ctx(),
            tenant.id,
            &refresh_token,
        )
        .await
        {
            tracing::warn!(%error, "cookie session logout revocation failed; clearing cookie anyway");
        }
    }

    let cookie = build_session_cookie(
        "",
        0,
        session_cookie_is_secure(&headers, addr.ip(), ctx.runtime_ctx().settings()),
    );
    let mut response = json_response(LogoutResponse { status: "ok" });
    if let Ok(value) = axum::http::HeaderValue::from_str(&cookie) {
        response
            .headers_mut()
            .append(axum::http::header::SET_COOKIE, value);
    }
    Ok(response)
}

#[utoipa::path(get, path = "/api/auth/me", tag = "auth", security(("bearer_auth" = [])),
    responses((status = 200, description = "Current user info", body = UserResponse),(status = 401, description = "Unauthorized")))]
async fn me(
    CurrentUser {
        user,
        inferred_role,
        ..
    }: CurrentUser,
) -> Result<Response> {
    tracing::debug!(user_id = %user.id, "auth::me entry");
    Ok(json_response(user_response_from_model(user, inferred_role)))
}

#[utoipa::path(post, path = "/api/auth/invite/accept", tag = "auth", request_body = AcceptInviteParams,
    responses((status = 200, description = "Invite accepted", body = InviteAcceptResponse),(status = 401, description = "Invalid or expired invite token")))]
async fn accept_invite(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    Json(params): Json<AcceptInviteParams>,
) -> Result<Response> {
    tracing::debug!("auth::accept_invite entry");
    if !ctx.runtime_ctx().settings().features.invites_enabled {
        // Deliberately indistinguishable from an invalid token.
        return Err(Error::Unauthorized(
            "Invalid or already consumed invite token".into(),
        ));
    }
    let config = ctx
        .auth_config()
        .cloned()
        .ok_or(Error::InternalServerError)?;
    let accepted = AuthLifecycleService::accept_invite_once_runtime(
        ctx.runtime_ctx(),
        &config,
        tenant.id,
        &params.token,
        &params.password,
        params.name,
    )
    .await
    .map_err(|error| match error {
        crate::services::auth_invite::InviteAcceptanceError::EmailAlreadyExists => {
            Error::BadRequest("A user with this email already exists".into())
        }
        other => Error::from(other),
    })?;

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
    let email = params.email.trim();
    tracing::debug!(email, "auth::request_reset entry");
    let config = ctx
        .auth_config()
        .cloned()
        .ok_or(Error::InternalServerError)?;

    let user = Users::find_by_email(ctx.runtime_ctx().db(), tenant.id, email).await?;

    let reset_token = user
        .as_ref()
        .map(|record| {
            encode_password_reset_token(
                &config,
                tenant.id,
                &record.email,
                &record.password_hash,
                PASSWORD_RESET_TOKEN_TTL_SECS,
            )
        })
        .transpose()?;

    if let Some(reset_token_value) = reset_token.as_ref() {
        let runtime_ctx = ctx.runtime_ctx();
        let email_service = match email_service_from_ctx(
            runtime_ctx,
            request_context.locale.as_str(),
        ) {
            Ok(service) => service,
            Err(_) => {
                tracing::warn!(
                    "Password reset email could not be prepared; keeping the public response generic"
                );
                return Ok(json_response(ResetRequestResponse { status: "ok" }));
            }
        };
        let reset_url = match password_reset_url(runtime_ctx, reset_token_value) {
            Ok(url) => url,
            Err(_) => {
                tracing::warn!(
                    "Password reset URL could not be prepared; keeping the public response generic"
                );
                return Ok(json_response(ResetRequestResponse { status: "ok" }));
            }
        };
        let recipient = user
            .as_ref()
            .map(|record| record.email.clone())
            .unwrap_or_else(|| params.email.clone());

        tokio::spawn(async move {
            if let Err(error) = email_service
                .send_password_reset(PasswordResetEmail {
                    to: recipient,
                    reset_url,
                })
                .await
            {
                tracing::warn!(error = %error, "Failed to send password reset email");
            }
        });
    }

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

#[utoipa::path(post, path = "/api/auth/verify/request", tag = "auth", request_body = RequestVerificationParams,
    responses((status = 200, description = "Verification request accepted", body = VerificationRequestResponse)))]
async fn request_verification(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    request_context: RequestContext,
    Json(params): Json<RequestVerificationParams>,
) -> Result<Response> {
    let email = params.email.trim();
    tracing::debug!(email, "auth::request_verification entry");
    let config = ctx
        .auth_config()
        .cloned()
        .ok_or(Error::InternalServerError)?;
    let settings = ctx.runtime_ctx().settings();

    let user = Users::find_by_email(ctx.runtime_ctx().db(), tenant.id, email).await?;

    let verification_token = if settings.features.email_verification {
        user.filter(|record| record.email_verified_at.is_none())
            .map(|record| {
                encode_email_verification_token(
                    &config,
                    tenant.id,
                    record.id,
                    &record.email,
                    EMAIL_VERIFICATION_TOKEN_TTL_SECS,
                )
            })
            .transpose()?
    } else {
        None
    };

    if let Some(verification_token_value) = verification_token.as_ref() {
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
        let recipient = params.email.clone();
        let verification_token = verification_token_value.clone();

        tokio::spawn(async move {
            if let Err(error) = email_service
                .send_email_verification(EmailVerificationEmail {
                    to: recipient,
                    verification_token,
                })
                .await
            {
                tracing::warn!(error = %error, "Failed to send email verification email");
            }
        });
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
        .route("/api/auth/invite/accept", post(accept_invite))
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
}

fn clamp_session_limit(limit: Option<u64>) -> u64 {
    limit.unwrap_or(50).clamp(1, 100)
}
#[cfg(test)]
mod tests {
    use super::ensure_registration_enabled;
    use crate::common::RustokSettings;

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
    fn clamp_session_limit_bounds() {
        assert_eq!(super::clamp_session_limit(None), 50);
        assert_eq!(super::clamp_session_limit(Some(0)), 1);
        assert_eq!(super::clamp_session_limit(Some(25)), 25);
        assert_eq!(super::clamp_session_limit(Some(100)), 100);
        assert_eq!(super::clamp_session_limit(Some(1000)), 100);
    }
}
