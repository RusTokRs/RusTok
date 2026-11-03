use crate::auth::decode_access_token;
use crate::context::{TenantContextExt, infer_user_role_from_permissions};
use crate::models::{
    oauth_apps::{self, Entity as OAuthApps},
    oauth_consents::Entity as OAuthConsents,
    oauth_tokens::{self, Entity as OAuthTokens},
    sessions::Entity as Sessions,
    users::{self, Entity as Users},
};
use crate::services::rbac_service::RbacService;
use axum::{
    extract::{FromRef, FromRequestParts},
    http::{StatusCode, request::Parts},
};
use axum_extra::{
    TypedHeader,
    headers::{Authorization, authorization::Bearer},
};
use rustok_api::{
    AuthPrincipalKind, Permission,
    context::{restrict_permissions_to_scopes, scope_matches},
};
use rustok_core::{SecurityActorKind, UserRole};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use tracing::warn;

use crate::services::server_runtime_context::{ServerAuthRuntime, ServerRuntimeContext};

const AUTHORIZATION_CODE_GRANT_TYPE: &str = "authorization_code";
const CLIENT_CREDENTIALS_GRANT_TYPE: &str = "client_credentials";

#[derive(Debug)]
pub struct CurrentUser {
    pub user: users::Model,
    pub session_id: uuid::Uuid,
    pub permissions: Vec<Permission>,
    pub inferred_role: UserRole,
    pub actor_kind: SecurityActorKind,
    pub principal_kind: AuthPrincipalKind,
    pub client_id: Option<uuid::Uuid>,
    pub scopes: Vec<String>,
    pub grant_type: String,
    pub access_token_expires_at: chrono::DateTime<chrono::Utc>,
}

impl CurrentUser {
    pub fn security_context(&self) -> rustok_core::SecurityContext {
        match self.actor_kind {
            SecurityActorKind::Service => rustok_core::SecurityContext::service(
                self.inferred_role.clone(),
                self.permissions.iter().copied(),
            ),
            _ => rustok_core::SecurityContext::from_permissions(
                self.inferred_role.clone(),
                Some(self.user.id),
                self.permissions.iter().copied(),
            ),
        }
    }
}

/// Authenticated WebSocket authority, retained only for the connection lifetime.
/// Every revalidation re-resolves the signed credential against current storage
/// and compares the effective authority with the original request snapshot.
#[derive(Clone)]
pub(crate) struct WebSocketAuthLease {
    tenant_id: uuid::Uuid,
    access_token: String,
    initial_scope: crate::services::rbac_request_scope::RbacRequestScope,
    expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum WebSocketAuthLeaseError {
    Expired,
    ResolutionFailed,
    TimedOut,
    AuthorityChanged,
}

impl WebSocketAuthLease {
    pub(crate) fn new(access_token: String, current_user: &CurrentUser) -> Self {
        Self {
            tenant_id: current_user.user.tenant_id,
            access_token,
            initial_scope: crate::services::rbac_request_scope::RbacRequestScope::new(
                current_user.user.tenant_id,
                current_user.user.id,
                current_user.permissions.clone(),
                current_user.inferred_role.clone(),
            ),
            expires_at: current_user.access_token_expires_at,
        }
    }

    /// Time remaining until expiry; zero means no further data may be sent.
    pub(crate) fn remaining(&self) -> std::time::Duration {
        (self.expires_at - chrono::Utc::now())
            .to_std()
            .unwrap_or(std::time::Duration::ZERO)
    }

    pub(crate) async fn revalidate(
        &self,
        auth_runtime: &ServerAuthRuntime,
    ) -> Result<crate::services::rbac_request_scope::RbacRequestScope, WebSocketAuthLeaseError>
    {
        let remaining = self.remaining();
        if remaining.is_zero() {
            return Err(WebSocketAuthLeaseError::Expired);
        }
        let lookup_timeout = remaining.min(std::time::Duration::from_secs(5));
        let user = tokio::time::timeout(lookup_timeout, async {
            let tenant = crate::models::tenants::Entity::find_by_id(self.tenant_id)
                .one(auth_runtime.runtime_ctx().db())
                .await
                .map_err(|_| ())?
                .filter(|tenant| tenant.is_active)
                .ok_or(())?;
            resolve_current_user_from_access_token(auth_runtime, tenant.id, &self.access_token)
                .await
                .map_err(|_| ())
        })
        .await
        .map_err(|_| {
            if self.remaining().is_zero() {
                WebSocketAuthLeaseError::Expired
            } else {
                WebSocketAuthLeaseError::TimedOut
            }
        })?
        .map_err(|_| WebSocketAuthLeaseError::ResolutionFailed)?;
        // A slow authority lookup must not extend the signed token lifetime.
        if chrono::Utc::now() >= self.expires_at {
            return Err(WebSocketAuthLeaseError::Expired);
        }
        let scope = crate::services::rbac_request_scope::RbacRequestScope::new(
            user.user.tenant_id,
            user.user.id,
            user.permissions,
            user.inferred_role,
        );
        if scope != self.initial_scope {
            return Err(WebSocketAuthLeaseError::AuthorityChanged);
        }
        Ok(scope)
    }
}

fn classify_access_token_claims(
    claims: &crate::auth::Claims,
) -> Result<AuthPrincipalKind, (StatusCode, &'static str)> {
    AuthPrincipalKind::from_authenticated_facts(
        &claims.grant_type,
        claims.client_id,
        claims.session_id,
    )
    .ok_or((
        StatusCode::UNAUTHORIZED,
        "Invalid token subject or grant invariants",
    ))
}

fn validate_oauth_token_scopes(
    app: &oauth_apps::Model,
    token_scopes: &[String],
) -> Result<(), (StatusCode, &'static str)> {
    let allowed_scopes = app.scopes_list();
    if token_scopes
        .iter()
        .all(|scope| scope_matches(&allowed_scopes, scope))
    {
        return Ok(());
    }

    Err((
        StatusCode::UNAUTHORIZED,
        "OAuth token scopes are no longer allowed",
    ))
}

async fn validate_active_user_consent(
    db: &DatabaseConnection,
    app: &oauth_apps::Model,
    tenant_id: uuid::Uuid,
    user_id: uuid::Uuid,
    token_scopes: &[String],
    token_consent_granted_at: Option<sea_orm::prelude::DateTimeWithTimeZone>,
) -> Result<(), (StatusCode, &'static str)> {
    if !app.requires_user_consent() {
        return Ok(());
    }

    let consent = OAuthConsents::find_active_consent(db, app.id, user_id, tenant_id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
        .ok_or((StatusCode::UNAUTHORIZED, "OAuth consent revoked or missing"))?;
    if token_consent_granted_at != Some(consent.granted_at) {
        return Err((
            StatusCode::UNAUTHORIZED,
            "OAuth token predates the current consent epoch",
        ));
    }
    let consent_scopes = consent.scopes_list();
    if token_scopes
        .iter()
        .all(|scope| scope_matches(&consent_scopes, scope))
    {
        return Ok(());
    }

    Err((
        StatusCode::UNAUTHORIZED,
        "OAuth consent no longer covers token scopes",
    ))
}

/// Fail-closed registration fence for already-issued OAuth access JWTs.
/// The signed `jti` must name a live access row whose hash, client,
/// tenant, grant, and subject bind the presented credential. Revoking
/// that row (or its family) therefore invalidates the JWT before TTL.
async fn validate_oauth_access_token_registration(
    db: &DatabaseConnection,
    app_id: uuid::Uuid,
    tenant_id: uuid::Uuid,
    grant_type: &str,
    jti: Option<uuid::Uuid>,
    access_token: &str,
    expected_user_id: Option<uuid::Uuid>,
) -> Result<oauth_tokens::Model, (StatusCode, &'static str)> {
    let jti = jti.ok_or((
        StatusCode::UNAUTHORIZED,
        "OAuth access token is not registered",
    ))?;
    let token_hash = crate::auth::hash_refresh_token(access_token);
    let row = OAuthTokens::find_by_id(jti)
        .filter(oauth_tokens::Column::TokenHash.eq(token_hash))
        .filter(oauth_tokens::Column::AppId.eq(app_id))
        .filter(oauth_tokens::Column::TenantId.eq(tenant_id))
        .filter(oauth_tokens::Column::GrantType.eq(grant_type))
        .filter(oauth_tokens::Column::TokenKind.eq(oauth_tokens::TOKEN_KIND_ACCESS))
        .filter(oauth_tokens::Column::RevokedAt.is_null())
        .filter(oauth_tokens::Column::ExpiresAt.gt(chrono::Utc::now()))
        .one(db)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
        .ok_or((
            StatusCode::UNAUTHORIZED,
            "OAuth access token revoked or unknown",
        ))?;
    if row.user_id != expected_user_id {
        return Err((
            StatusCode::UNAUTHORIZED,
            "OAuth access token subject mismatch",
        ));
    }
    Ok(row)
}

async fn resolve_active_oauth_app(
    db: &DatabaseConnection,
    tenant_id: uuid::Uuid,
    client_id: uuid::Uuid,
    required_grant_type: &'static str,
    token_scopes: &[String],
) -> Result<oauth_apps::Model, (StatusCode, &'static str)> {
    let app = OAuthApps::find_active_security_by_client_id(db, client_id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
        .ok_or((StatusCode::UNAUTHORIZED, "OAuth app not found or inactive"))?;

    if app.tenant_id != tenant_id {
        return Err((StatusCode::FORBIDDEN, "Token belongs to another tenant"));
    }

    if !app.supports_grant_type(required_grant_type) {
        return Err((
            StatusCode::UNAUTHORIZED,
            "OAuth grant is no longer allowed for this app",
        ));
    }

    validate_oauth_token_scopes(&app, token_scopes)?;

    Ok(app)
}

fn filter_available_permissions(
    permissions: Vec<Permission>,
    availability: Option<rustok_rbac::PermissionAvailability>,
) -> Vec<Permission> {
    match availability {
        Some(availability) => availability.active_permissions(&permissions),
        None => permissions,
    }
}

async fn active_permissions_for_tenant(
    ctx: &ServerRuntimeContext,
    tenant_id: uuid::Uuid,
    principal_id: uuid::Uuid,
    permissions: Vec<Permission>,
) -> Result<Vec<Permission>, (StatusCode, &'static str)> {
    let availability =
        crate::services::rbac_module_availability::permission_availability_for(ctx, tenant_id)
            .await
            .map_err(|error| {
                tracing::error!(
                    %error,
                    tenant_id = %tenant_id,
                    principal_id = %principal_id,
                    "RBAC module availability resolution failed"
                );
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Module policy unavailable",
                )
            })?;
    Ok(filter_available_permissions(permissions, availability))
}

async fn resolve_service_token_permissions(
    ctx: &ServerRuntimeContext,
    tenant_id: uuid::Uuid,
    subject_id: uuid::Uuid,
    client_id: uuid::Uuid,
    claimed_role: UserRole,
    token_scopes: &[String],
    access_token: &str,
    jti: Option<uuid::Uuid>,
) -> Result<(Vec<Permission>, UserRole), (StatusCode, &'static str)> {
    let db = ctx.db();
    let app = resolve_active_oauth_app(
        db,
        tenant_id,
        client_id,
        CLIENT_CREDENTIALS_GRANT_TYPE,
        token_scopes,
    )
    .await?;

    if app.id != subject_id {
        return Err((
            StatusCode::UNAUTHORIZED,
            "OAuth service token subject mismatch",
        ));
    }

    validate_oauth_access_token_registration(
        db,
        app.id,
        tenant_id,
        CLIENT_CREDENTIALS_GRANT_TYPE,
        jti,
        access_token,
        None,
    )
    .await?;

    let granted_permissions = app.parsed_granted_permissions().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "OAuth app permissions are invalid",
        )
    })?;
    let effective_permissions = restrict_permissions_to_scopes(&granted_permissions, token_scopes);
    let effective_permissions =
        active_permissions_for_tenant(ctx, tenant_id, client_id, effective_permissions).await?;
    let inferred_role = infer_user_role_from_permissions(&effective_permissions);
    if claimed_role != inferred_role {
        RbacService::record_claim_role_mismatch();
        warn!(
            client_id = %client_id,
            tenant_id = %tenant_id,
            claimed_role = %claimed_role,
            inferred_role = %inferred_role,
            "rbac_claim_role_mismatch"
        );
    }

    Ok((effective_permissions, inferred_role))
}

pub(crate) async fn resolve_current_user<S>(
    parts: &mut Parts,
    state: &S,
) -> Result<CurrentUser, (StatusCode, &'static str)>
where
    S: Send + Sync,
    ServerAuthRuntime: FromRef<S>,
{
    let auth_runtime = ServerAuthRuntime::from_ref(state);

    let tenant_id = parts
        .tenant_context()
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Tenant context missing"))?
        .id;

    let TypedHeader(Authorization(bearer)) =
        TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state)
            .await
            .map_err(|_| (StatusCode::UNAUTHORIZED, "Missing or invalid token"))?;

    resolve_current_user_from_access_token(&auth_runtime, tenant_id, bearer.token()).await
}

pub async fn resolve_current_user_from_access_token(
    ctx: &ServerAuthRuntime,
    tenant_id: uuid::Uuid,
    access_token: &str,
) -> Result<CurrentUser, (StatusCode, &'static str)> {
    let auth_config = ctx.auth_config().ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        "JWT secret not configured",
    ))?;
    let db = ctx.runtime_ctx().db();

    let claims = decode_access_token(auth_config, access_token)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid token signature"))?;
    let access_token_expires_at = chrono::DateTime::<chrono::Utc>::from_timestamp(claims.exp as i64, 0)
        .ok_or((StatusCode::UNAUTHORIZED, "Invalid token expiry"))?;

    if claims.tenant_id != tenant_id {
        return Err((StatusCode::FORBIDDEN, "Token belongs to another tenant"));
    }

    let principal_kind = classify_access_token_claims(&claims)?;

    if principal_kind == AuthPrincipalKind::DirectUser {
        let session = Sessions::find_by_id(claims.session_id)
            .one(db)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
            .ok_or((StatusCode::UNAUTHORIZED, "Session not found"))?;

        if session.user_id != claims.sub {
            return Err((StatusCode::UNAUTHORIZED, "Session subject mismatch"));
        }

        if session.tenant_id != tenant_id || !session.is_active() {
            return Err((StatusCode::UNAUTHORIZED, "Session expired"));
        }
    } else if principal_kind == AuthPrincipalKind::DelegatedUser {
        let client_id = claims.client_id.ok_or((
            StatusCode::UNAUTHORIZED,
            "OAuth user token is missing client_id",
        ))?;
        let app = resolve_active_oauth_app(
            db,
            tenant_id,
            client_id,
            AUTHORIZATION_CODE_GRANT_TYPE,
            &claims.scopes,
        )
        .await?;
        let access_row = validate_oauth_access_token_registration(
            db,
            app.id,
            tenant_id,
            AUTHORIZATION_CODE_GRANT_TYPE,
            claims.jti,
            access_token,
            Some(claims.sub),
        )
        .await?;
        validate_active_user_consent(
            db,
            &app,
            tenant_id,
            claims.sub,
            &claims.scopes,
            access_row.consent_granted_at,
        )
        .await?;
    }

    let (user, permissions, inferred_role, session_id, actor_kind) = match principal_kind {
        AuthPrincipalKind::DirectUser | AuthPrincipalKind::DelegatedUser => {
            let user = Users::find_by_id(claims.sub)
                .one(db)
                .await
                .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
                .ok_or((StatusCode::UNAUTHORIZED, "User not found"))?;

            if user.tenant_id != tenant_id {
                return Err((StatusCode::FORBIDDEN, "User belongs to another tenant"));
            }

            if !user.is_active() {
                return Err((StatusCode::FORBIDDEN, "User is inactive"));
            }

            if principal_kind == AuthPrincipalKind::DelegatedUser {
                let issued_at =
                    chrono::DateTime::<chrono::Utc>::from_timestamp(claims.iat as i64, 0)
                        .ok_or((StatusCode::UNAUTHORIZED, "Invalid token issued-at"))?;
                if user.rejects_delegated_token_issued_at(issued_at) {
                    return Err((
                        StatusCode::UNAUTHORIZED,
                        "OAuth token predates the current credential state",
                    ));
                }
            }

            let granted_permissions =
                RbacService::get_user_permissions_authoritative(db, &tenant_id, &user.id)
                    .await
                    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

            // Dormant grants — permissions owned by modules disabled for
            // this tenant — never enter the request snapshot. Disabling a
            // module therefore revokes its capabilities immediately and
            // everywhere, while the persisted role grants remain intact and
            // reactivate when the module is enabled again. Availability
            // resolution failures fail closed, like role resolution above.
            let granted_permissions = active_permissions_for_tenant(
                ctx.runtime_ctx(),
                tenant_id,
                user.id,
                granted_permissions,
            )
            .await?;

            let effective_permissions = if principal_kind == AuthPrincipalKind::DelegatedUser {
                restrict_permissions_to_scopes(&granted_permissions, &claims.scopes)
            } else {
                granted_permissions
            };
            // Authoritative role resolution must not silently degrade to a
            // permission-derived heuristic on database failure.
            let effective_role =
                crate::services::auth_lifecycle::AuthLifecycleService::resolve_effective_role(
                    db, tenant_id, user.id,
                )
                .await
                .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;
            if claims.role != effective_role {
                RbacService::record_claim_role_mismatch();
                warn!(
                    user_id = %user.id,
                    tenant_id = %tenant_id,
                    claimed_role = %claims.role,
                    effective_role = %effective_role,
                    "rbac_claim_role_mismatch"
                );
            }

            (
                user,
                effective_permissions,
                effective_role,
                claims.session_id,
                SecurityActorKind::User,
            )
        }
        AuthPrincipalKind::Service => {
            let client_id = claims.client_id.ok_or((
                StatusCode::UNAUTHORIZED,
                "OAuth service token is missing client_id",
            ))?;
            let (permissions, inferred_role) = resolve_service_token_permissions(
                ctx.runtime_ctx(),
                tenant_id,
                claims.sub,
                client_id,
                claims.role,
                &claims.scopes,
                access_token,
                claims.jti,
            )
            .await?;

            (
                users::Model::default_service_user(claims.sub, tenant_id),
                permissions,
                inferred_role,
                uuid::Uuid::nil(),
                SecurityActorKind::Service,
            )
        }
    };

    Ok(CurrentUser {
        user,
        session_id,
        permissions,
        inferred_role,
        actor_kind,
        principal_kind,
        client_id: claims.client_id,
        scopes: claims.scopes,
        grant_type: claims.grant_type,
        access_token_expires_at,
    })
}

impl<S> FromRequestParts<S> for CurrentUser
where
    S: Send + Sync,
    ServerAuthRuntime: FromRef<S>,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        resolve_current_user(parts, state).await
    }
}

pub struct OptionalCurrentUser(pub Option<CurrentUser>);

impl<S> FromRequestParts<S> for OptionalCurrentUser
where
    S: Send + Sync,
    ServerAuthRuntime: FromRef<S>,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        if parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .is_none()
        {
            return Ok(Self(None));
        }

        let current_user = resolve_current_user(parts, state).await?;
        Ok(Self(Some(current_user)))
    }
}

#[cfg(test)]
mod tests;
