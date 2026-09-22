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
use chrono::Utc;
use rustok_telemetry::metrics;
use rustok_web::json_response;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
    sea_query::Expr,
};
use std::net::SocketAddr;

use crate::auth::{
    decode_email_verification_token, decode_invite_token, encode_email_verification_token,
    encode_password_reset_token, hash_refresh_token,
};
use crate::common::RequestContext;
use crate::extractors::{auth::CurrentUser, tenant::CurrentTenant};
use crate::models::{
    sessions,
    users::{self, Entity as Users},
};
use crate::services::auth_lifecycle::{AuthLifecycleError, AuthLifecycleService};
use crate::services::email::{
    EmailVerificationEmail, PasswordResetEmail, email_service_from_ctx, password_reset_url,
};
use crate::services::server_runtime_context::ServerAuthRuntime;
pub use rustok_auth::{
    AcceptInviteParams, AuthResponse, ChangePasswordParams, ConfirmResetParams,
    ConfirmVerificationParams, GenericStatusResponse, InviteAcceptResponse, LoginParams,
    LogoutResponse, RefreshRequest, RegisterParams, RequestResetParams, RequestVerificationParams,
    ResetRequestResponse, SessionItem, SessionListParams, SessionsResponse, UpdateProfileParams,
    UserInfo, UserResponse, VerificationRequestResponse,
};

const DEFAULT_RESET_TOKEN_TTL_SECS: u64 = 15 * 60;
const DEFAULT_VERIFY_TOKEN_TTL_SECS: u64 = 24 * 60 * 60;

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

#[utoipa::path(post, path = "/api/auth/register", tag = "auth", request_body = RegisterParams,
    responses((status = 200, description = "Registration successful", body = AuthResponse),(status = 400, description = "Email already exists")))]
async fn register(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    Json(params): Json<RegisterParams>,
) -> Result<Response> {
    let runtime_ctx = ctx.runtime_ctx();
    let config = ctx
        .auth_config()
        .cloned()
        .ok_or(Error::InternalServerError)?;
    let (user, tokens) = AuthLifecycleService::register_runtime(
        runtime_ctx,
        &config,
        tenant.id,
        &params.email,
        &params.password,
        params.name,
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

#[utoipa::path(post, path = "/api/auth/login", tag = "auth", request_body = LoginParams,
    responses((status = 200, description = "Login successful", body = AuthResponse),(status = 401, description = "Unauthorized")))]
async fn login(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: axum::http::HeaderMap,
    Json(params): Json<LoginParams>,
) -> Result<Response> {
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
        &params.email,
        &params.password,
        Some(addr.ip().to_string()),
        user_agent,
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
    let token_hash = hash_refresh_token(&params.refresh_token);
    let session =
        sessions::Entity::find_by_token_hash(ctx.runtime_ctx().db(), tenant.id, &token_hash)
            .await?
            .ok_or_else(|| Error::Unauthorized("Invalid refresh token".into()))?;

    if session.revoked_at.is_none() {
        let mut session_model: sessions::ActiveModel = session.into();
        session_model.revoked_at = Set(Some(Utc::now().into()));
        session_model.update(ctx.runtime_ctx().db()).await?;
    }

    Ok(json_response(LogoutResponse { status: "ok" }))
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
    Ok(json_response(user_response_from_model(user, inferred_role)))
}

#[utoipa::path(post, path = "/api/auth/invite/accept", tag = "auth", request_body = AcceptInviteParams,
    responses((status = 200, description = "Invite accepted", body = InviteAcceptResponse),(status = 401, description = "Invalid or expired invite token")))]
async fn accept_invite(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    Json(params): Json<AcceptInviteParams>,
) -> Result<Response> {
    let config = ctx
        .auth_config()
        .cloned()
        .ok_or(Error::InternalServerError)?;
    let claims = decode_invite_token(&config, &params.token)?;

    if claims.tenant_id != tenant.id {
        return Err(Error::Unauthorized("Invalid invite token".into()));
    }

    let email = claims.sub.clone();
    let role = claims.role.clone();
    let runtime_ctx = ctx.runtime_ctx();

    AuthLifecycleService::create_user_runtime(
        runtime_ctx,
        tenant.id,
        &email,
        &params.password,
        params.name,
        role.clone(),
        Some(rustok_core::UserStatus::Active),
    )
    .await
    .map_err(|e: AuthLifecycleError| match e {
        AuthLifecycleError::EmailAlreadyExists => {
            Error::BadRequest("A user with this email already exists".into())
        }
        other => Error::from(other),
    })?;

    Ok(json_response(InviteAcceptResponse {
        status: "ok",
        email,
        role,
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
    let config = ctx
        .auth_config()
        .cloned()
        .ok_or(Error::InternalServerError)?;

    let user = Users::find_by_email(ctx.runtime_ctx().db(), tenant.id, &params.email).await?;

    let expose_token = std::env::var("RUSTOK_DEMO_MODE")
        .map(|value| value == "1")
        .unwrap_or(false);

    let reset_token = user
        .as_ref()
        .map(|record| {
            encode_password_reset_token(
                &config,
                tenant.id,
                &record.email,
                &record.password_hash,
                DEFAULT_RESET_TOKEN_TTL_SECS,
            )
        })
        .transpose()?;

    if let Some(reset_token_value) = reset_token.as_ref() {
        let runtime_ctx = ctx.runtime_ctx();
        let email_service = email_service_from_ctx(runtime_ctx, request_context.locale.as_str())
            .map_err(|_| Error::InternalServerError)?;
        let reset_url = password_reset_url(runtime_ctx, reset_token_value)
            .map_err(|_| Error::InternalServerError)?;
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

    Ok(json_response(ResetRequestResponse {
        status: "ok",
        reset_token: if expose_token { reset_token } else { None },
    }))
}

#[utoipa::path(post, path = "/api/auth/reset/confirm", tag = "auth", request_body = ConfirmResetParams,
    responses((status = 200, description = "Password updated", body = GenericStatusResponse),(status = 401, description = "Invalid token")))]
async fn confirm_reset(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    Json(params): Json<ConfirmResetParams>,
) -> Result<Response> {
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
    let config = ctx
        .auth_config()
        .cloned()
        .ok_or(Error::InternalServerError)?;
    let settings = ctx.runtime_ctx().settings();

    let user = Users::find_by_email(ctx.runtime_ctx().db(), tenant.id, &params.email).await?;

    let expose_token = std::env::var("RUSTOK_DEMO_MODE")
        .map(|value| value == "1")
        .unwrap_or(false);

    let verification_token = if settings.features.email_verification {
        user.filter(|record| record.email_verified_at.is_none())
            .map(|record| {
                encode_email_verification_token(
                    &config,
                    tenant.id,
                    record.id,
                    &record.email,
                    DEFAULT_VERIFY_TOKEN_TTL_SECS,
                )
            })
            .transpose()?
    } else {
        None
    };

    if let Some(verification_token_value) = verification_token.as_ref() {
        let runtime_ctx = ctx.runtime_ctx();
        let email_service = email_service_from_ctx(runtime_ctx, request_context.locale.as_str())
            .map_err(|_| Error::InternalServerError)?;
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

    Ok(json_response(VerificationRequestResponse {
        status: "ok",
        verification_token: if expose_token {
            verification_token
        } else {
            None
        },
    }))
}

#[utoipa::path(post, path = "/api/auth/verify/confirm", tag = "auth", request_body = ConfirmVerificationParams,
    responses((status = 200, description = "Email verified", body = GenericStatusResponse),(status = 401, description = "Invalid token")))]
async fn confirm_verification(
    State(ctx): State<ServerAuthRuntime>,
    CurrentTenant(tenant): CurrentTenant,
    Json(params): Json<ConfirmVerificationParams>,
) -> Result<Response> {
    let config = ctx
        .auth_config()
        .cloned()
        .ok_or(Error::InternalServerError)?;
    let claims = decode_email_verification_token(&config, &params.token)?;

    if claims.tenant_id != tenant.id {
        return Err(Error::Unauthorized("Invalid verification token".into()));
    }

    let user = Users::find_by_id(claims.user_id)
        .filter(users::Column::TenantId.eq(tenant.id))
        .one(ctx.runtime_ctx().db())
        .await?
        .filter(|record| record.email.eq_ignore_ascii_case(&claims.sub))
        .ok_or_else(|| Error::Unauthorized("Invalid verification token".into()))?;

    if user.email_verified_at.is_none() {
        let mut user_active: users::ActiveModel = user.into();
        user_active.email_verified_at = Set(Some(Utc::now().into()));
        user_active.update(ctx.runtime_ctx().db()).await?;
    }

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
    let requested_limit = params.limit;
    let limit = clamp_session_limit(params.limit);

    let rows = sessions::Entity::find()
        .filter(sessions::Column::TenantId.eq(tenant.id))
        .filter(sessions::Column::UserId.eq(current.user.id))
        .filter(sessions::Column::RevokedAt.is_null())
        .filter(sessions::Column::ExpiresAt.gt(Utc::now()))
        .order_by_desc(sessions::Column::CreatedAt)
        .limit(limit)
        .all(ctx.runtime_ctx().db())
        .await?;

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
    let now = Utc::now();

    sessions::Entity::update_many()
        .col_expr(sessions::Column::RevokedAt, Expr::value(now))
        .filter(sessions::Column::TenantId.eq(tenant.id))
        .filter(sessions::Column::UserId.eq(current.user.id))
        .filter(sessions::Column::RevokedAt.is_null())
        .filter(sessions::Column::Id.ne(current.session_id))
        .exec(ctx.runtime_ctx().db())
        .await?;

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
    let runtime_ctx = ctx.runtime_ctx();
    AuthLifecycleService::change_password_runtime(
        runtime_ctx,
        tenant.id,
        current.user.id,
        current.session_id,
        &params.current_password,
        &params.new_password,
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
