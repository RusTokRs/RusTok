use crate::auth::decode_access_token;
use crate::context::{infer_user_role_from_permissions, TenantContextExt};
use crate::models::{
    oauth_apps::{self, Entity as OAuthApps},
    oauth_consents::Entity as OAuthConsents,
    sessions::Entity as Sessions,
    users::{self, Entity as Users},
};
use crate::services::rbac_service::RbacService;
use axum::{
    extract::{FromRef, FromRequestParts},
    http::{request::Parts, StatusCode},
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use rustok_api::{
    context::{restrict_permissions_to_scopes, scope_matches},
    Permission,
};
use rustok_core::{SecurityActorKind, UserRole};
use sea_orm::{DatabaseConnection, EntityTrait};
use tracing::warn;

use crate::services::server_runtime_context::ServerAuthRuntime;

const DIRECT_GRANT_TYPE: &str = "direct";
const AUTHORIZATION_CODE_GRANT_TYPE: &str = "authorization_code";
const CLIENT_CREDENTIALS_GRANT_TYPE: &str = "client_credentials";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccessTokenSubjectKind {
    User,
    Service,
}

#[derive(Debug)]
pub struct CurrentUser {
    pub user: users::Model,
    pub session_id: uuid::Uuid,
    pub permissions: Vec<Permission>,
    pub inferred_role: UserRole,
    pub actor_kind: SecurityActorKind,
    pub client_id: Option<uuid::Uuid>,
    pub scopes: Vec<String>,
    pub grant_type: String,
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

fn classify_access_token_claims(
    claims: &crate::auth::Claims,
) -> Result<AccessTokenSubjectKind, (StatusCode, &'static str)> {
    match claims.grant_type.as_str() {
        DIRECT_GRANT_TYPE if claims.client_id.is_none() && !claims.session_id.is_nil() => {
            Ok(AccessTokenSubjectKind::User)
        }
        AUTHORIZATION_CODE_GRANT_TYPE
            if claims.client_id.is_some() && claims.session_id.is_nil() =>
        {
            Ok(AccessTokenSubjectKind::User)
        }
        CLIENT_CREDENTIALS_GRANT_TYPE
            if claims.client_id.is_some() && claims.session_id.is_nil() =>
        {
            Ok(AccessTokenSubjectKind::Service)
        }
        _ => Err((
            StatusCode::UNAUTHORIZED,
            "Invalid token subject or grant invariants",
        )),
    }
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
) -> Result<(), (StatusCode, &'static str)> {
    if !app.requires_user_consent() {
        return Ok(());
    }

    let consent = OAuthConsents::find_active_consent(db, app.id, user_id, tenant_id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
        .ok_or((StatusCode::UNAUTHORIZED, "OAuth consent revoked or missing"))?;
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

async fn resolve_active_oauth_app(
    db: &DatabaseConnection,
    tenant_id: uuid::Uuid,
    client_id: uuid::Uuid,
    required_grant_type: &'static str,
    token_scopes: &[String],
) -> Result<oauth_apps::Model, (StatusCode, &'static str)> {
    let app = OAuthApps::find_active_by_client_id(db, client_id)
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

async fn resolve_service_token_permissions(
    db: &DatabaseConnection,
    tenant_id: uuid::Uuid,
    subject_id: uuid::Uuid,
    client_id: uuid::Uuid,
    claimed_role: UserRole,
    token_scopes: &[String],
) -> Result<(Vec<Permission>, UserRole), (StatusCode, &'static str)> {
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

    let granted_permissions = app.parsed_granted_permissions().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "OAuth app permissions are invalid",
        )
    })?;
    let effective_permissions = restrict_permissions_to_scopes(&granted_permissions, token_scopes);
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

    if claims.tenant_id != tenant_id {
        return Err((StatusCode::FORBIDDEN, "Token belongs to another tenant"));
    }

    let subject_kind = classify_access_token_claims(&claims)?;

    if claims.grant_type == DIRECT_GRANT_TYPE {
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
    } else if claims.grant_type == AUTHORIZATION_CODE_GRANT_TYPE {
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
        validate_active_user_consent(db, &app, tenant_id, claims.sub, &claims.scopes).await?;
    }

    let (user, permissions, inferred_role, session_id, actor_kind) = match subject_kind {
        AccessTokenSubjectKind::User => {
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

            let granted_permissions =
                RbacService::get_user_permissions_authoritative(db, &tenant_id, &user.id)
                    .await
                    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

            let effective_permissions = if claims.client_id.is_some() {
                restrict_permissions_to_scopes(&granted_permissions, &claims.scopes)
            } else {
                granted_permissions
            };
            let inferred_role = infer_user_role_from_permissions(&effective_permissions);
            if claims.role != inferred_role {
                RbacService::record_claim_role_mismatch();
                warn!(
                    user_id = %user.id,
                    tenant_id = %tenant_id,
                    claimed_role = %claims.role,
                    inferred_role = %inferred_role,
                    "rbac_claim_role_mismatch"
                );
            }

            (
                user,
                effective_permissions,
                inferred_role,
                claims.session_id,
                SecurityActorKind::User,
            )
        }
        AccessTokenSubjectKind::Service => {
            let client_id = claims.client_id.ok_or((
                StatusCode::UNAUTHORIZED,
                "OAuth service token is missing client_id",
            ))?;
            let (permissions, inferred_role) = resolve_service_token_permissions(
                db,
                tenant_id,
                claims.sub,
                client_id,
                claims.role,
                &claims.scopes,
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
        client_id: claims.client_id,
        scopes: claims.scopes,
        grant_type: claims.grant_type,
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
