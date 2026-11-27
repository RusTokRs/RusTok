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
    /// Effective access deadline: the earlier of the signed JWT and durable credential.
    pub access_token_expires_at: chrono::DateTime<chrono::Utc>,
    pub(crate) oauth_credential: Option<crate::services::rbac_request_scope::OAuthCredentialEvidence>,
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
    monotonic_deadline: std::time::Instant,
    expired: std::sync::Arc<std::sync::atomic::AtomicBool>,
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
        let started = std::time::Instant::now();
        let remaining = (current_user.access_token_expires_at - chrono::Utc::now())
            .to_std().unwrap_or_default();
        Self {
            tenant_id: current_user.user.tenant_id,
            access_token,
            initial_scope: crate::services::rbac_request_scope::RbacRequestScope::authenticated(
                current_user,
                current_user.access_token_expires_at,
            ),
            expires_at: current_user.access_token_expires_at,
            monotonic_deadline: started.checked_add(remaining).unwrap_or(started),
            expired: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Time remaining until expiry; zero means no further data may be sent.
    pub(crate) fn remaining(&self) -> std::time::Duration {
        if rustok_auth::application_clock::check_local().is_err() {
            self.expired.store(true, std::sync::atomic::Ordering::Release);
            return std::time::Duration::ZERO;
        }
        self.remaining_at(chrono::Utc::now(), std::time::Instant::now())
    }

    fn remaining_at(&self, wall: chrono::DateTime<chrono::Utc>, mono: std::time::Instant) -> std::time::Duration {
        use std::sync::atomic::Ordering;
        let remaining = (self.expires_at - wall).to_std().unwrap_or_default()
            .min(self.monotonic_deadline.saturating_duration_since(mono));
        if remaining.is_zero() {
            self.expired.store(true, Ordering::Release);
        }
        if self.expired.load(Ordering::Acquire) { std::time::Duration::ZERO } else { remaining }
    }

    pub(crate) fn ensure_local_authority(&self, auth_runtime: &ServerAuthRuntime) -> Result<(), WebSocketAuthLeaseError> {
        if self.remaining().is_zero() {
            return Err(WebSocketAuthLeaseError::Expired);
        }
        let config = auth_runtime.auth_config().ok_or(WebSocketAuthLeaseError::ResolutionFailed)?;
        decode_access_token(config, &self.access_token).map_err(|_| WebSocketAuthLeaseError::ResolutionFailed)?;
        if self.remaining().is_zero() {
            return Err(WebSocketAuthLeaseError::Expired);
        }
        Ok(())
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
            resolve_current_user_from_access_token(auth_runtime, self.tenant_id, &self.access_token)
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
        // A slow authority lookup must not extend the original credential lifetime.
        self.ensure_local_authority(auth_runtime)?;
        // A shortened durable deadline invalidates the original lease, even
        // if the effective permissions have not changed. A later deadline
        // never extends an already admitted connection.
        if user.access_token_expires_at < self.expires_at {
            return Err(WebSocketAuthLeaseError::AuthorityChanged);
        }
        let scope = crate::services::rbac_request_scope::RbacRequestScope::authenticated(
            &user,
            self.expires_at,
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

pub(crate) fn validate_oauth_token_scopes(
    app: &oauth_apps::Model,
    token_scopes: &[String],
) -> Result<(), (StatusCode, &'static str)> {
    let allowed_scopes = app.decoded_scopes().map_err(|_| {
        (StatusCode::UNAUTHORIZED, "Stored OAuth app scopes are invalid")
    })?;
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

pub(crate) async fn validate_active_user_consent(
    db: &impl sea_orm::ConnectionTrait,
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
    let consent_scopes = consent.scopes_list().map_err(|_| {
        (StatusCode::UNAUTHORIZED, "Stored OAuth consent scopes are invalid")
    })?;
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
/// tenant, grant, subject, and exact issued scopes bind the credential.
/// Revoking that row (or its family) invalidates the JWT before TTL.
#[allow(clippy::too_many_arguments)]
async fn validate_oauth_access_token_registration(
    db: &DatabaseConnection,
    app_id: uuid::Uuid,
    tenant_id: uuid::Uuid,
    grant_type: &str,
    jti: Option<uuid::Uuid>,
    access_token: &str,
    expected_user_id: Option<uuid::Uuid>,
    expected_scopes: &[String],
) -> Result<oauth_tokens::Model, (StatusCode, &'static str)> {
    let jti = jti.ok_or((
        StatusCode::UNAUTHORIZED,
        "OAuth access token is not registered",
    ))?;
    let token_hash = crate::auth::hash_refresh_token(access_token);
    let now = rustok_auth::application_clock::admit_now(db).await
        .map_err(|_| (StatusCode::SERVICE_UNAVAILABLE, "Application database clock unavailable"))?;
    let row = OAuthTokens::find_by_id(jti)
        .filter(oauth_tokens::Column::TokenHash.eq(token_hash))
        .filter(oauth_tokens::Column::AppId.eq(app_id))
        .filter(oauth_tokens::Column::TenantId.eq(tenant_id))
        .filter(oauth_tokens::Column::GrantType.eq(grant_type))
        .filter(oauth_tokens::Column::TokenKind.eq(oauth_tokens::TOKEN_KIND_ACCESS))
        .filter(oauth_tokens::Column::RevokedAt.is_null())
        .filter(oauth_tokens::Column::CreatedAt.lte(now))
        .filter(oauth_tokens::Column::ExpiresAt.gt(now))
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
    let registered_scopes = row.decoded_scopes().map_err(|_| {
        (StatusCode::UNAUTHORIZED, "Stored OAuth access token scopes are invalid")
    })?;
    if registered_scopes.as_slice() != expected_scopes {
        return Err((
            StatusCode::UNAUTHORIZED,
            "OAuth access token scopes mismatch",
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
    let app = OAuthApps::find_active_security_by_client_id_scoped(db, tenant_id, client_id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
        .ok_or((StatusCode::UNAUTHORIZED, "OAuth app not found or inactive"))?;

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
        crate::services::rbac_module_availability::authoritative_permission_availability_for(ctx, tenant_id)
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
) -> Result<(Vec<Permission>, UserRole, chrono::DateTime<chrono::Utc>, crate::services::rbac_request_scope::OAuthCredentialEvidence), (StatusCode, &'static str)> {
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

    let access_row = validate_oauth_access_token_registration(
        db,
        app.id,
        tenant_id,
        CLIENT_CREDENTIALS_GRANT_TYPE,
        jti,
        access_token,
        None,
        token_scopes,
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

    Ok((
        effective_permissions,
        inferred_role,
        access_row.expires_at.with_timezone(&chrono::Utc),
        crate::services::rbac_request_scope::oauth_credential_from_registration(
            &access_row, client_id, token_scopes,
        ),
    ))
}

pub(crate) async fn resolve_current_user<S>(
    parts: &mut Parts,
    state: &S,
) -> Result<CurrentUser, (StatusCode, &'static str)>
where
    S: Send + Sync,
    ServerAuthRuntime: FromRef<S>,
{
    // This extractor is also used independently of the optional auth
    // middleware. Reject ambiguous credentials at the extraction boundary.
    if parts
        .headers
        .get_all(axum::http::header::AUTHORIZATION)
        .iter()
        .nth(1)
        .is_some()
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "Multiple Authorization headers are not allowed",
        ));
    }
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

    rustok_auth::application_clock::admit_now(db).await
        .map_err(|_| (StatusCode::SERVICE_UNAVAILABLE, "Application database clock unavailable"))?;
    auth_config.credential_now()
        .map_err(|_| (StatusCode::SERVICE_UNAVAILABLE, "Credential authority unavailable"))?;
    let claims = decode_access_token(auth_config, access_token)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid token signature"))?;
    let expiry_seconds = i64::try_from(claims.exp)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid token expiry"))?;
    let access_token_expires_at =
        chrono::DateTime::<chrono::Utc>::from_timestamp(expiry_seconds, 0)
            .ok_or((StatusCode::UNAUTHORIZED, "Invalid token expiry"))?;

    // INVARIANT: In-memory verification ensuring the decoded JWT token payload belongs to the requested tenant.
    if claims.tenant_id != tenant_id {
        return Err((StatusCode::FORBIDDEN, "Token belongs to another tenant"));
    }

    let principal_kind = classify_access_token_claims(&claims)?;
    let mut delegated_credential_issued_at = None;
    let mut oauth_credential = None;
    let mut static_authority_revision = None;
    let mut durable_credential_expires_at = access_token_expires_at;

    // Tenant middleware is intentionally bypassed on global operator routes,
    // and a cached tenant context may predate deactivation. Resolve the live
    // tenant at the common credential boundary for every principal kind.
    let tenant = crate::models::tenants::Entity::find_by_id(tenant_id)
        .one(db)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
        .ok_or((StatusCode::FORBIDDEN, "Tenant is unavailable"))?;
    if !tenant.is_active {
        return Err((StatusCode::FORBIDDEN, "Tenant is unavailable"));
    }

    if principal_kind == AuthPrincipalKind::DirectUser {
        let session = Sessions::find_by_id(claims.session_id)
            .filter(crate::models::sessions::Column::TenantId.eq(tenant_id))
            .one(db)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
            .ok_or((StatusCode::UNAUTHORIZED, "Session not found"))?;

        if session.user_id != claims.sub {
            return Err((StatusCode::UNAUTHORIZED, "Session subject mismatch"));
        }

        // INVARIANT: In-memory check validating session activity and confirming database-scoped tenant identity.
        let now = rustok_auth::application_clock::admit_now(db).await
            .map_err(|_| (StatusCode::SERVICE_UNAVAILABLE, "Application database clock unavailable"))?;
        if session.tenant_id != tenant_id || !session.is_active_at(now) {
            return Err((StatusCode::UNAUTHORIZED, "Session expired"));
        }
        durable_credential_expires_at = session
            .expires_at
            .with_timezone(&chrono::Utc)
            .min(session.absolute_deadline());
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
            &claims.scopes,
        )
        .await?;
        // The durable registration row retains subsecond issuance time;
        // JWT NumericDate is whole seconds and can misorder a same-second
        // password change and token issuance.
        delegated_credential_issued_at =
            Some(access_row.created_at.with_timezone(&chrono::Utc));
        durable_credential_expires_at = access_row.expires_at.with_timezone(&chrono::Utc);
        oauth_credential = Some(crate::services::rbac_request_scope::oauth_credential_from_registration(
            &access_row, client_id, &claims.scopes,
        ));
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
                .filter(users::Column::TenantId.eq(tenant_id))
                .one(db)
                .await
                .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
                .ok_or((StatusCode::UNAUTHORIZED, "User not found"))?;

            // INVARIANT: In-memory verification confirming database-scoped user tenant identity.
            if user.tenant_id != tenant_id {
                return Err((StatusCode::FORBIDDEN, "User belongs to another tenant"));
            }

            if !user.is_active() {
                return Err((StatusCode::FORBIDDEN, "User is inactive"));
            }
            if ctx
                .runtime_ctx()
                .settings()
                .features
                .email_verification
                && !user.is_email_verified()
            {
                return Err((StatusCode::FORBIDDEN, "Email verification required"));
            }

            if principal_kind == AuthPrincipalKind::DelegatedUser {
                let issued_at = delegated_credential_issued_at.ok_or((
                    StatusCode::UNAUTHORIZED,
                    "OAuth access token is not registered",
                ))?;
                if user.rejects_delegated_token_issued_at(issued_at) {
                    return Err((
                        StatusCode::UNAUTHORIZED,
                        "OAuth token predates the current credential state",
                    ));
                }
            }

            static_authority_revision = Some(read_static_authority_revision(db, tenant_id, user.id).await?);
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
            let (permissions, inferred_role, registration_expires_at, registration) =
                resolve_service_token_permissions(
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
            durable_credential_expires_at = registration_expires_at;
            oauth_credential = Some(registration);

            (
                users::Model::default_service_user(claims.sub, tenant_id),
                permissions,
                inferred_role,
                uuid::Uuid::nil(),
                SecurityActorKind::Service,
            )
        }
    };

    // Storage and policy resolution may outlast either credential fence.
    // The signed JWT cannot extend the observed session (including its
    // absolute cap) or durable OAuth registration deadline.
    let verified = rustok_auth::application_clock::admit_now(db).await
        .map_err(|_| (StatusCode::SERVICE_UNAVAILABLE, "Application database clock unavailable"))?;
    let live = auth_config.credential_now()
        .map_err(|_| (StatusCode::SERVICE_UNAVAILABLE, "Credential authority unavailable"))?;
    let decision = rustok_auth::application_clock::decision_time(verified, live)
        .map_err(|_| (StatusCode::SERVICE_UNAVAILABLE, "Application database clock unavailable"))?;
    ensure_access_token_not_expired(
        access_token_expires_at,
        durable_credential_expires_at,
        decision,
    )?;
    // If the deadline was crossed during the clock commit, do not grant
    // access on the older sample. Persist the crossing before classifying
    // it as expiry; an inconclusive observation still denies admission.
    let deadline = access_token_expires_at.min(durable_credential_expires_at);
    if live >= deadline {
        let crossing = rustok_auth::application_clock::admit_now(db).await
            .map_err(|_| (StatusCode::SERVICE_UNAVAILABLE, "Application database clock unavailable"))?;
        if crossing < deadline {
            return Err((StatusCode::SERVICE_UNAVAILABLE, "Application database clock unavailable"));
        }
        ensure_access_token_not_expired(access_token_expires_at, durable_credential_expires_at, crossing)?;
    }

    // The initial session, account, registration and policy reads preceded
    // asynchronous authority resolution and the independent clock commit.
    // Check the live owner state again; a shortened deadline never extends
    // or silently changes the admitted snapshot.
    revalidate_admitted_authority(
        ctx, tenant_id, &claims, access_token, &user, &permissions,
        &inferred_role, principal_kind, delegated_credential_issued_at,
        durable_credential_expires_at, decision, static_authority_revision.as_ref(),
    ).await?;

    // A healthy replacement keyring need not retain the original key.
    decode_access_token(auth_config, access_token)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid token signature"))?;
    rustok_auth::application_clock::check_local()
        .map_err(|_| (StatusCode::SERVICE_UNAVAILABLE, "Application database clock unavailable"))?;
    let final_now = auth_config.credential_now()
        .map_err(|_| (StatusCode::SERVICE_UNAVAILABLE, "Credential authority unavailable"))?;
    if final_now >= deadline {
        let crossing = rustok_auth::application_clock::admit_now(db).await
            .map_err(|_| (StatusCode::SERVICE_UNAVAILABLE, "Application database clock unavailable"))?;
        if crossing < deadline {
            return Err((StatusCode::SERVICE_UNAVAILABLE, "Application database clock unavailable"));
        }
        return Err((StatusCode::UNAUTHORIZED, "Access token expired"));
    }

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
        access_token_expires_at: access_token_expires_at.min(durable_credential_expires_at),
        oauth_credential,
    })
}

async fn read_static_authority_revision(
    db: &DatabaseConnection, tenant_id: uuid::Uuid, user_id: uuid::Uuid,
) -> Result<rustok_rbac::StaticActorAuthorityRevision, (StatusCode, &'static str)> {
    rustok_rbac::read_static_actor_authority_revision_on(db, tenant_id, user_id).await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))
}

#[allow(clippy::too_many_arguments)]
async fn revalidate_admitted_authority(
    ctx: &ServerAuthRuntime,
    tenant_id: uuid::Uuid,
    claims: &crate::auth::Claims,
    access_token: &str,
    original_user: &users::Model,
    permissions: &[Permission],
    role: &UserRole,
    principal_kind: AuthPrincipalKind,
    delegated_issued_at: Option<chrono::DateTime<chrono::Utc>>,
    durable_deadline: chrono::DateTime<chrono::Utc>,
    admitted_at: chrono::DateTime<chrono::Utc>,
    static_revision: Option<&rustok_rbac::StaticActorAuthorityRevision>,
) -> Result<(), (StatusCode, &'static str)> {
    let db = ctx.runtime_ctx().db();
    let changed = (StatusCode::UNAUTHORIZED, "Credential authority changed during admission");
    let database_error = (StatusCode::INTERNAL_SERVER_ERROR, "Database error");
    if principal_kind == AuthPrincipalKind::Service {
        let client_id = claims.client_id.ok_or(changed)?;
        let (current_permissions, current_role, current_deadline, _) = resolve_service_token_permissions(
            ctx.runtime_ctx(), tenant_id, claims.sub, client_id, claims.role.clone(),
            &claims.scopes, access_token, claims.jti,
        ).await?;
        if &current_role != role || current_deadline < durable_deadline
            || permissions.iter().any(|permission| !current_permissions.contains(permission))
        {
            return Err(changed);
        }
    } else {
        let current_user = Users::find_by_id(original_user.id)
            .filter(users::Column::TenantId.eq(tenant_id))
            .one(db).await.map_err(|_| database_error)?.ok_or(changed)?;
        if !current_user.is_active() || current_user.recovery_revision != original_user.recovery_revision
            || (ctx.runtime_ctx().settings().features.email_verification && !current_user.is_email_verified())
        {
            return Err(changed);
        }
        match principal_kind {
            AuthPrincipalKind::DirectUser => {
                let session = Sessions::find_by_id(claims.session_id)
                    .filter(crate::models::sessions::Column::TenantId.eq(tenant_id))
                    .one(db).await.map_err(|_| database_error)?.ok_or(changed)?;
                if session.user_id != original_user.id || !session.is_active_at(admitted_at)
                    || session.expires_at.with_timezone(&chrono::Utc).min(session.absolute_deadline()) < durable_deadline
                {
                    return Err(changed);
                }
            }
            AuthPrincipalKind::DelegatedUser => {
                let issued_at = delegated_issued_at.ok_or(changed)?;
                if current_user.rejects_delegated_token_issued_at(issued_at) {
                    return Err(changed);
                }
                let client_id = claims.client_id.ok_or(changed)?;
                let app = resolve_active_oauth_app(db, tenant_id, client_id, AUTHORIZATION_CODE_GRANT_TYPE, &claims.scopes).await?;
                let row = validate_oauth_access_token_registration(
                    db, app.id, tenant_id, AUTHORIZATION_CODE_GRANT_TYPE,
                    claims.jti, access_token, Some(original_user.id), &claims.scopes,
                ).await?;
                if row.created_at.with_timezone(&chrono::Utc) != issued_at
                    || row.expires_at.with_timezone(&chrono::Utc) < durable_deadline
                {
                    return Err(changed);
                }
                validate_active_user_consent(db, &app, tenant_id, original_user.id, &claims.scopes, row.consent_granted_at).await?;
            }
            AuthPrincipalKind::Service => return Err(changed),
        }
        let active = active_permissions_for_tenant(ctx.runtime_ctx(), tenant_id, original_user.id, permissions.to_vec()).await?;
        if permissions.iter().any(|permission| !active.contains(permission)) {
            return Err(changed);
        }
        let current_revision = read_static_authority_revision(db, tenant_id, original_user.id).await?;
        if static_revision != Some(&current_revision) {
            return Err(changed);
        }
    }
    let tenant = crate::models::tenants::Entity::find_by_id(tenant_id)
        .one(db).await.map_err(|_| database_error)?.ok_or(changed)?;
    if !tenant.is_active {
        return Err((StatusCode::FORBIDDEN, "Tenant is unavailable"));
    }
    Ok(())
}

fn ensure_access_token_not_expired(
    signed_expires_at: chrono::DateTime<chrono::Utc>,
    durable_expires_at: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), (StatusCode, &'static str)> {
    if now >= signed_expires_at.min(durable_expires_at) {
        return Err((StatusCode::UNAUTHORIZED, "Access token expired"));
    }
    Ok(())
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
