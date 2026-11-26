use axum::http::StatusCode;
use chrono::Utc;
use rustok_api::context::scope_matches;
use rustok_auth::{AuthRefreshRetryKey, OauthAccessTokenInput, TokenRequest, TokenResponse};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend,
    EntityTrait, QueryFilter, QuerySelect, Set, TransactionTrait, sea_query::Expr,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::auth::{self, AuthConfig};
use crate::context::infer_user_role_from_permissions;
use crate::models::{
    oauth_apps, oauth_authorization_codes as oauth_codes,
    oauth_consents::Entity as OAuthConsents,
    oauth_tokens::{self, Entity as OAuthTokens},
    users::{self, Entity as Users},
};
use crate::services::oauth_app::{OAuthAppService, lock_oauth_app_for_mutation};
use crate::services::rbac_service::RbacService;
use crate::services::server_runtime_context::ServerAuthRuntime;

const CLIENT_CREDENTIALS_GRANT: &str = "client_credentials";
const AUTHORIZATION_CODE_GRANT: &str = "authorization_code";
const REFRESH_TOKEN_GRANT: &str = "refresh_token";
const SERVICE_ACCESS_TOKEN_TTL_SECS: u64 = 60 * 60;
const USER_ACCESS_TOKEN_TTL_SECS: u64 = 15 * 60;
const USER_REFRESH_TOKEN_TTL_DAYS: i64 = 30;
/// Absolute cap for one OAuth refresh-token lineage, mirroring the
/// first-party session cap: rotation slides the per-token expiry but
/// never past 90 days from the family's first token.
const USER_REFRESH_FAMILY_ABSOLUTE_LIFETIME_DAYS: i64 = 90;
/// A replay of a rotated-away OAuth refresh token inside this window is
/// rejected without revoking the family because it may be a concurrent
/// duplicate. Proof-bearing requests may recover only the committed live
/// successor. After the window reuse revokes the whole family.
const OAUTH_REFRESH_REUSE_GRACE_SECONDS: i64 = 30;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthTokenProtocolError {
    pub status: StatusCode,
    pub error: &'static str,
    pub description: String,
}

impl OAuthTokenProtocolError {
    fn invalid_request(description: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_request", description)
    }

    fn invalid_client(description: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "invalid_client", description)
    }

    fn unauthorized_client(description: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "unauthorized_client", description)
    }

    fn invalid_grant(description: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_grant", description)
    }

    fn invalid_scope(description: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_scope", description)
    }

    fn unsupported_grant(description: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "unsupported_grant_type",
            description,
        )
    }

    fn server_error(description: impl Into<String>) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "server_error",
            description,
        )
    }

    fn new(status: StatusCode, error: &'static str, description: impl Into<String>) -> Self {
        Self {
            status,
            error,
            description: description.into(),
        }
    }
}

type OAuthClock<'a> = dyn Fn() -> Result<chrono::DateTime<Utc>, OAuthTokenProtocolError> + Send + Sync + 'a;

async fn verify_application_clock(db: &impl ConnectionTrait) -> Result<(), OAuthTokenProtocolError> {
    rustok_auth::application_clock::verified_now(db).await
        .map(|_| ())
        .map_err(|_| OAuthTokenProtocolError::server_error("OAuth application database clock unavailable"))
}

fn check_application_clock() -> Result<(), OAuthTokenProtocolError> {
    rustok_auth::application_clock::check_local()
        .map_err(|_| OAuthTokenProtocolError::server_error("OAuth application database clock unavailable"))
}

pub struct OAuthTokenService;

impl OAuthTokenService {
    /// Execute OAuth token issuance at a transport-independent boundary.
    ///
    /// One-shot credentials are consumed in the same database transaction that
    /// persists their replacement. A failed replacement therefore rolls back
    /// consumption instead of destroying an otherwise valid credential family.
    pub async fn exchange(
        runtime: &ServerAuthRuntime,
        tenant_id: Uuid,
        request: &TokenRequest,
        retry_key: Option<&AuthRefreshRetryKey>,
    ) -> Result<TokenResponse, OAuthTokenProtocolError> {
        if retry_key.is_some() && request.grant_type != REFRESH_TOKEN_GRANT {
            return Err(OAuthTokenProtocolError::invalid_request(
                "Refresh retry key is only valid for the refresh_token grant",
            ));
        }
        let auth_config = runtime.auth_config().ok_or_else(|| {
            OAuthTokenProtocolError::server_error("OAuth signing configuration is unavailable")
        })?;
        let clock = || auth_config.credential_now().map_err(|_| {
            OAuthTokenProtocolError::server_error("OAuth credential clock or signing authority unavailable")
        });
        clock()?;
        let db = runtime.runtime_ctx().db();
        let require_email_verification = runtime.runtime_ctx().settings().features.email_verification;
        let app = resolve_client(db, tenant_id, request).await?;

        let (prepared, scopes) = match request.grant_type.as_str() {
            CLIENT_CREDENTIALS_GRANT => {
                require_grant(&app, CLIENT_CREDENTIALS_GRANT)?;
                authenticate_client(&app, request, true).await?;
                let tx = db.begin().await.map_err(|_| {
                    OAuthTokenProtocolError::server_error(
                        "Failed to begin client-credentials exchange",
                    )
                })?;
                let current_app = lock_active_oauth_app(&tx, app.id, tenant_id).await?;
                ensure_authenticated_client_unchanged(&app, &current_app)?;
                require_grant(&current_app, CLIENT_CREDENTIALS_GRANT)?;
                let scopes = validate_requested_scopes(&current_app, request.scope.as_deref())?;
                verify_application_clock(&tx).await?;
                let prepared = prepare_service_access_token(&current_app, auth_config, &scopes)?;
                validate_prepared_deadlines(&prepared, clock()?, None)?;
                persist_prepared_tokens(&tx, &prepared).await?;
                verify_application_clock(&tx).await?;
                validate_prepared_deadlines(&prepared, clock()?, None)?;
                tx.commit().await.map_err(|_| {
                    OAuthTokenProtocolError::server_error(
                        "Failed to commit client-credentials exchange",
                    )
                })?;

                (prepared, scopes)
            }
            AUTHORIZATION_CODE_GRANT => {
                require_grant(&app, AUTHORIZATION_CODE_GRANT)?;
                authenticate_client(&app, request, false).await?;
                let code = required(request.code.as_deref(), "code is required")?;
                let redirect_uri =
                    required(request.redirect_uri.as_deref(), "redirect_uri is required")?;
                let verifier = required(
                    request.code_verifier.as_deref(),
                    "code_verifier is required",
                )?;
                let authorization =
                    validate_authorization_code(db, &app, tenant_id, code, redirect_uri, verifier, &clock)
                        .await?;
                let scopes = authorization.scopes_list().map_err(|_| {
                    OAuthTokenProtocolError::invalid_grant("Authorization code scopes are invalid")
                })?;
                let prepared = prepare_user_tokens(
                    db,
                    &app,
                    auth_config,
                    authorization.user_id,
                    &scopes,
                    app.supports_grant_type(REFRESH_TOKEN_GRANT),
                    None,
                    Some(authorization.id),
                    None,
                    &clock,
                )
                .await?;

                commit_authorization_code_exchange(
                    db,
                    &app,
                    &authorization,
                    &prepared,
                    require_email_verification,
                    &clock,
                )
                .await?;
                (prepared, scopes)
            }
            REFRESH_TOKEN_GRANT => {
                require_grant(&app, REFRESH_TOKEN_GRANT)?;
                authenticate_client(&app, request, false).await?;
                let raw_refresh_token = required(
                    request.refresh_token.as_deref(),
                    "refresh_token is required",
                )?;
                let current =
                    validate_refresh_token(db, &app, tenant_id, raw_refresh_token, retry_key.is_some(), &clock).await?;
                let user_id = current.user_id.ok_or_else(|| {
                    OAuthTokenProtocolError::invalid_grant("Refresh token has no associated user")
                })?;
                let family = resolve_refresh_family(db, &current, &clock).await?;
                ensure_refresh_family_live(&family, clock()?)?;
                // OAuth refresh requests may narrow an existing grant, never widen it.
                // An omitted scope retains the original grant; a supplied empty
                // scope must not silently restore the full set of permissions.
                let scopes = scopes_for_refresh(&current.scopes_list(), request.scope.as_deref())?;
                let derived_refresh = retry_key
                    .map(|key| key.derive_oauth_token(
                        auth_config, tenant_id, app.id, family.family_id, current.id,
                    ))
                    .transpose()
                    .map_err(|_| OAuthTokenProtocolError::server_error("Failed to derive OAuth refresh successor"))?;
                let prepared = prepare_user_tokens(
                    db,
                    &app,
                    auth_config,
                    user_id,
                    &scopes,
                    true,
                    Some(&family),
                    current.authorization_code_id,
                    derived_refresh,
                    &clock,
                )
                .await?;

                let recovery = commit_refresh_rotation(
                    db,
                    &app,
                    &current,
                    &prepared,
                    require_email_verification,
                    retry_key.is_some(),
                    &clock,
                )
                .await?;
                if let Some(window) = recovery {
                    check_application_clock()?;
                    let now = clock()?;
                    window.ensure_live(now)?;
                    let response = prepared.into_response(&scopes, now)?;
                    // Recovery is not another rotation; do not extend deadlines.
                    return Ok(response);
                }
                (prepared, scopes)
            }
            other => {
                return Err(OAuthTokenProtocolError::unsupported_grant(format!(
                    "Grant type `{other}` is not supported"
                )));
            }
        };

        if let Err(error) = OAuthAppService::touch_last_used_scoped(db, app.tenant_id, app.id).await
        {
            tracing::warn!(app_id = %app.id, tenant_id = %app.tenant_id, error = %error, "Failed to update OAuth app usage timestamp");
        }

        // Usage telemetry is asynchronous: calculate the lifetime only
        // after it finishes, not from the pre-telemetry response.
        check_application_clock()?;
        prepared.into_response(&scopes, clock()?)
    }
}

struct PreparedOAuthTokens {
    access_token: String,
    access_model: oauth_tokens::ActiveModel,
    refresh_token: Option<String>,
    refresh_model: Option<oauth_tokens::ActiveModel>,
    consent_granted_at: Option<sea_orm::prelude::DateTimeWithTimeZone>,
    expires_in: u64,
}

impl PreparedOAuthTokens {
    fn into_response(self, scopes: &[String], now: chrono::DateTime<Utc>) -> Result<TokenResponse, OAuthTokenProtocolError> {
        let expires_in = prepared_access_lifetime(&self.access_model, now, self.expires_in)?;
        Ok(TokenResponse {
            access_token: self.access_token,
            token_type: "Bearer".to_string(),
            expires_in,
            refresh_token: self.refresh_token,
            scope: scopes.join(" "),
        })
    }
}

async fn resolve_client(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    request: &TokenRequest,
) -> Result<oauth_apps::Model, OAuthTokenProtocolError> {
    let client_id = required(request.client_id.as_deref(), "client_id is required")?;
    let client_id = Uuid::parse_str(client_id)
        .map_err(|_| OAuthTokenProtocolError::invalid_client("Invalid client_id format"))?;
    let app =
        oauth_apps::Entity::find_active_security_by_client_id_scoped(db, tenant_id, client_id)
            .await
            .map_err(|_| OAuthTokenProtocolError::server_error("Failed to resolve OAuth client"))?
            .ok_or_else(|| OAuthTokenProtocolError::invalid_client("Unknown or inactive client"))?;

    Ok(app)
}

fn require_grant(
    app: &oauth_apps::Model,
    grant_type: &'static str,
) -> Result<(), OAuthTokenProtocolError> {
    let incompatible_app_type = (app.app_type == "service"
        && (grant_type == AUTHORIZATION_CODE_GRANT || grant_type == REFRESH_TOKEN_GRANT))
        || (matches!(app.app_type.as_str(), "mobile" | "third_party")
            && grant_type == CLIENT_CREDENTIALS_GRANT);

    if app.supports_grant_type(grant_type) && !incompatible_app_type {
        Ok(())
    } else {
        Err(OAuthTokenProtocolError::unauthorized_client(format!(
            "Client is not allowed to use `{grant_type}`"
        )))
    }
}

async fn authenticate_client(
    app: &oauth_apps::Model,
    request: &TokenRequest,
    secret_required: bool,
) -> Result<(), OAuthTokenProtocolError> {
    // First-party clients intentionally combine a browser PKCE flow with a
    // confidential SSR flow. PKCE proves the browser exchange; only the
    // client_credentials grant authenticates with the optional client secret.
    if matches!(app.app_type.as_str(), "first_party" | "mobile") && !secret_required {
        return Ok(());
    }

    let Some(secret_hash) = app.client_secret_hash.as_deref() else {
        return if secret_required {
            Err(OAuthTokenProtocolError::invalid_client(
                "Confidential client credentials are required",
            ))
        } else {
            Ok(())
        };
    };
    let secret = request
        .client_secret
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| OAuthTokenProtocolError::invalid_client("client_secret is required"))?;
    let valid =
        OAuthAppService::verify_client_secret_async(secret.to_string(), secret_hash.to_string())
            .await
            .map_err(|_| OAuthTokenProtocolError::invalid_client("Invalid client credentials"))?;

    if valid {
        Ok(())
    } else {
        Err(OAuthTokenProtocolError::invalid_client(
            "Invalid client credentials",
        ))
    }
}

fn parse_requested_scopes(value: &str) -> Vec<String> {
    value.split_whitespace().map(str::to_string).collect()
}

fn validate_requested_scopes(
    app: &oauth_apps::Model,
    requested: Option<&str>,
) -> Result<Vec<String>, OAuthTokenProtocolError> {
    let allowed = app.scopes_list();
    let requested = match requested {
        Some(requested) => parse_requested_scopes(requested),
        None => allowed.clone(),
    };
    if requested.is_empty() {
        return Err(OAuthTokenProtocolError::invalid_scope(
            "Requested scope must not be empty",
        ));
    }
    validate_scope_subset(&allowed, &requested)?;
    Ok(requested)
}

fn scopes_for_refresh(
    current: &[String],
    requested: Option<&str>,
) -> Result<Vec<String>, OAuthTokenProtocolError> {
    let narrowed = match requested {
        Some(requested) => parse_requested_scopes(requested),
        None => current.to_vec(),
    };
    if narrowed.is_empty() {
        return Err(OAuthTokenProtocolError::invalid_scope(
            "Refresh scope must not be empty",
        ));
    }
    validate_scope_subset(current, &narrowed)?;
    Ok(narrowed)
}

fn validate_scope_subset(
    allowed: &[String],
    requested: &[String],
) -> Result<(), OAuthTokenProtocolError> {
    // An empty persisted authorization code or refresh grant must not pass
    // a vacuous subset check and be exchanged for a new credential.
    if !requested.is_empty() && requested.iter().all(|scope| scope_matches(allowed, scope)) {
        Ok(())
    } else {
        Err(OAuthTokenProtocolError::invalid_scope(
            "One or more requested scopes are not allowed",
        ))
    }
}

async fn validate_authorization_code(
    db: &DatabaseConnection,
    app: &oauth_apps::Model,
    tenant_id: Uuid,
    raw_code: &str,
    redirect_uri: &str,
    verifier: &str,
    clock: &OAuthClock<'_>,
) -> Result<oauth_codes::Model, OAuthTokenProtocolError> {
    let code_hash = hex::encode(Sha256::digest(raw_code.as_bytes()));
    let code = oauth_codes::Entity::find()
        .filter(oauth_codes::Column::CodeHash.eq(code_hash))
        .filter(oauth_codes::Column::AppId.eq(app.id))
        .filter(oauth_codes::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(|_| {
            OAuthTokenProtocolError::server_error("Failed to validate authorization code")
        })?
        .ok_or_else(|| {
            OAuthTokenProtocolError::invalid_grant(
                "Authorization code is invalid, expired, or already used",
            )
        })?;

    if code.app_id != app.id || code.tenant_id != tenant_id {
        return Err(OAuthTokenProtocolError::invalid_grant(
            "Authorization code is not bound to this client and tenant",
        ));
    }
    if code.redirect_uri != redirect_uri {
        return Err(OAuthTokenProtocolError::invalid_grant(
            "redirect_uri does not match the authorization request",
        ));
    }
    if !app.redirect_uris_list().contains(&code.redirect_uri) {
        return Err(OAuthTokenProtocolError::invalid_grant(
            "Authorization code redirect URI is no longer registered",
        ));
    }
    if code.code_challenge_method != "S256" {
        return Err(OAuthTokenProtocolError::invalid_grant(
            "Unsupported PKCE challenge method",
        ));
    }

    if !OAuthAppService::verify_pkce(&code.code_challenge, verifier) {
        return Err(OAuthTokenProtocolError::invalid_grant(
            "PKCE verification failed",
        ));
    }
    if code.used_at.is_some() {
        revoke_authorization_code_family(db, app, &code).await?;
        return Err(OAuthTokenProtocolError::invalid_grant(
            "Authorization code is invalid, expired, or already used",
        ));
    }
    verify_application_clock(db).await?;
    if !code.is_active_at(clock()?) {
        return Err(OAuthTokenProtocolError::invalid_grant(
            "Authorization code is invalid, expired, or already used",
        ));
    }

    let scopes = code.scopes_list().map_err(|_| {
        OAuthTokenProtocolError::invalid_grant("Authorization code scopes are invalid")
    })?;
    validate_scope_subset(&app.scopes_list(), &scopes)?;
    let (user, consent_granted_at) =
        validate_active_subject_and_consent(db, app, tenant_id, code.user_id, &scopes).await?;
    validate_code_consent_epoch(&code, consent_granted_at.as_ref())?;
    if user.rejects_delegated_token_issued_at(code.created_at.with_timezone(&Utc)) {
        return Err(OAuthTokenProtocolError::invalid_grant(
            "Authorization code predates the current credential state",
        ));
    }
    Ok(code)
}

// Also fence pre-existing codes from before the re-grant invalidation fix.
// Exact persisted timestamps are used; equality is allowed for coarse DB clocks.
fn validate_code_consent_epoch(
    code: &oauth_codes::Model,
    consent_granted_at: Option<&sea_orm::prelude::DateTimeWithTimeZone>,
) -> Result<(), OAuthTokenProtocolError> {
    if consent_granted_at.is_some_and(|granted_at| &code.created_at < granted_at) {
        return Err(OAuthTokenProtocolError::invalid_grant(
            "Authorization code predates the current consent epoch",
        ));
    }
    Ok(())
}

async fn validate_refresh_token(
    db: &DatabaseConnection,
    app: &oauth_apps::Model,
    tenant_id: Uuid,
    raw_token: &str,
    allow_recovery: bool,
    clock: &OAuthClock<'_>,
) -> Result<oauth_tokens::Model, OAuthTokenProtocolError> {
    let token_hash = auth::hash_refresh_token(raw_token);
    let token = OAuthTokens::find()
        .filter(oauth_tokens::Column::TokenHash.eq(token_hash.clone()))
        .filter(oauth_tokens::Column::AppId.eq(app.id))
        .filter(oauth_tokens::Column::TenantId.eq(tenant_id))
        .filter(oauth_tokens::Column::GrantType.eq(AUTHORIZATION_CODE_GRANT))
        .filter(oauth_tokens::Column::TokenKind.eq(oauth_tokens::TOKEN_KIND_REFRESH))
        .one(db)
        .await
        .map_err(|_| OAuthTokenProtocolError::server_error("Failed to validate refresh token"))?;
    verify_application_clock(db).await?;
    let now = clock()?;
    let token = match token {
        Some(token) if (token.revoked_at.is_none()
            && token.created_at.with_timezone(&Utc) <= now
            && token.expires_at.with_timezone(&Utc) > now)
            || (allow_recovery && within_refresh_rotation_grace(&token, now)) => token,
        _ => {
            // No admissible live or recoverable token holds this hash. If it is a rotated-away
            // token replayed after the grace window, that is theft
            // evidence: revoke the whole family before failing closed
            // (RFC 9700 refresh-token reuse guidance).
            detect_oauth_refresh_reuse(db, app, tenant_id, &token_hash, clock).await?;
            return Err(OAuthTokenProtocolError::invalid_grant(
                "Refresh token is invalid, expired, or already used",
            ));
        }
    };
    let user_id = token.user_id.ok_or_else(|| {
        OAuthTokenProtocolError::invalid_grant("Refresh token has no associated user")
    })?;
    let scopes = token.scopes_list();
    validate_scope_subset(&app.scopes_list(), &scopes)?;
    let (user, consent_granted_at) =
        validate_active_subject_and_consent(db, app, tenant_id, user_id, &scopes).await?;
    if user.rejects_delegated_token_issued_at(token.created_at.with_timezone(&Utc)) {
        return Err(OAuthTokenProtocolError::invalid_grant(
            "Refresh token predates the current credential state",
        ));
    }
    if app.requires_user_consent() && token.consent_granted_at != consent_granted_at {
        return Err(OAuthTokenProtocolError::invalid_grant(
            "Refresh token predates the current consent epoch",
        ));
    }
    Ok(token)
}

#[derive(Debug)]
struct RefreshTokenFamily {
    family_id: Uuid,
    created_at: chrono::DateTime<Utc>,
}

fn refresh_family_deadline(
    created_at: chrono::DateTime<Utc>,
) -> Result<chrono::DateTime<Utc>, OAuthTokenProtocolError> {
    created_at
        .checked_add_signed(chrono::Duration::days(
            USER_REFRESH_FAMILY_ABSOLUTE_LIFETIME_DAYS,
        ))
        .ok_or_else(|| {
            OAuthTokenProtocolError::invalid_grant(
                "Refresh token family has an invalid creation timestamp",
            )
        })
}

fn ensure_refresh_family_live(
    family: &RefreshTokenFamily,
    now: chrono::DateTime<Utc>,
) -> Result<(), OAuthTokenProtocolError> {
    if family.created_at > now {
        return Err(OAuthTokenProtocolError::invalid_grant(
            "Refresh token family has a future creation timestamp",
        ));
    }
    if now >= refresh_family_deadline(family.created_at)? {
        return Err(OAuthTokenProtocolError::invalid_grant(
            "Refresh token family exceeded its maximum lifetime",
        ));
    }
    Ok(())
}

/// Floor the response lifetime to complete seconds, rejecting credentials
/// whose remaining lifetime is too short to advertise a positive TTL.
fn remaining_access_lifetime(
    expires_at: chrono::DateTime<Utc>,
    now: chrono::DateTime<Utc>,
    maximum: u64,
) -> Result<u64, OAuthTokenProtocolError> {
    let seconds = expires_at.signed_duration_since(now).num_seconds();
    if seconds <= 0 {
        return Err(OAuthTokenProtocolError::invalid_grant(
            "OAuth access credential has no remaining lifetime",
        ));
    }
    Ok((seconds as u64).min(maximum))
}

fn prepared_token_expiry(
    model: &oauth_tokens::ActiveModel,
) -> Result<chrono::DateTime<Utc>, OAuthTokenProtocolError> {
    match &model.expires_at {
        sea_orm::ActiveValue::Set(value) => Ok(value.with_timezone(&Utc)),
        _ => Err(OAuthTokenProtocolError::server_error("Missing prepared OAuth token expiry")),
    }
}

fn prepared_token_creation(
    model: &oauth_tokens::ActiveModel,
) -> Result<chrono::DateTime<Utc>, OAuthTokenProtocolError> {
    match &model.created_at {
        sea_orm::ActiveValue::Set(value) => Ok(value.with_timezone(&Utc)),
        _ => Err(OAuthTokenProtocolError::server_error("Missing prepared OAuth token creation time")),
    }
}

fn prepared_access_lifetime(
    model: &oauth_tokens::ActiveModel,
    now: chrono::DateTime<Utc>,
    maximum: u64,
) -> Result<u64, OAuthTokenProtocolError> {
    if now < prepared_token_creation(model)? {
        return Err(OAuthTokenProtocolError::invalid_grant("OAuth credential creation time is in the future"));
    }
    remaining_access_lifetime(prepared_token_expiry(model)?, now, maximum)
}

fn validate_prepared_deadlines(
    prepared: &PreparedOAuthTokens,
    now: chrono::DateTime<Utc>,
    family_deadline: Option<chrono::DateTime<Utc>>,
) -> Result<(), OAuthTokenProtocolError> {
    prepared_access_lifetime(&prepared.access_model, now, prepared.expires_in)?;
    for model in std::iter::once(&prepared.access_model).chain(prepared.refresh_model.iter()) {
        let expiry = prepared_token_expiry(model)?;
        if prepared_token_creation(model)? > now
            || expiry <= now
            || family_deadline.is_some_and(|deadline| expiry > deadline)
        {
            return Err(OAuthTokenProtocolError::invalid_grant(
                "Prepared OAuth credential exceeds its current lifetime boundary",
            ));
        }
    }
    Ok(())
}

/// Only a committed rotation records matching consumption and revocation
/// timestamps. An explicit revoke, future timestamp or corrupt history
/// is not evidence of a benign concurrent rotation.
fn committed_refresh_rotation_at(
    token: &oauth_tokens::Model,
) -> Option<chrono::DateTime<Utc>> {
    let revoked_at = token.revoked_at.as_ref()?;
    if token.last_used_at.as_ref() != Some(revoked_at)
        || revoked_at < &token.created_at
        || revoked_at >= &token.expires_at
    {
        return None;
    }
    Some(revoked_at.with_timezone(&Utc))
}

fn within_refresh_rotation_grace(
    stale: &oauth_tokens::Model,
    now: chrono::DateTime<Utc>,
) -> bool {
    let Some(rotated_at) = committed_refresh_rotation_at(stale) else {
        return false;
    };
    let age = now.signed_duration_since(rotated_at);
    age >= chrono::Duration::zero()
        && age < chrono::Duration::seconds(OAUTH_REFRESH_REUSE_GRACE_SECONDS)
}

/// Resolve the canonical lineage of the presented refresh token.
/// A referenced root that is missing or belongs to another
/// client/tenant/subject fails closed. A referenced row must itself be a
/// root of the same authorization-code lineage and cannot postdate the
/// descendant: accepting a different or newer root could reset the cap.
async fn resolve_refresh_family<C>(
    db: &C,
    current: &oauth_tokens::Model,
    clock: &OAuthClock<'_>,
) -> Result<RefreshTokenFamily, OAuthTokenProtocolError>
where
    C: ConnectionTrait,
{
    let now = clock()?;
    let family_id = current.family_id;
    if family_id.is_nil() || current.id.is_nil()
        || current.user_id.is_none_or(|id| id.is_nil())
        || current.created_at.with_timezone(&Utc) > now
    {
        return Err(OAuthTokenProtocolError::invalid_grant(
            "Refresh token family identity or creation time is invalid",
        ));
    }
    let created_at = if family_id == current.id {
        current.created_at.with_timezone(&Utc)
    } else {
        let root = OAuthTokens::find_by_id(family_id)
            .filter(oauth_tokens::Column::AppId.eq(current.app_id))
            .filter(oauth_tokens::Column::TenantId.eq(current.tenant_id))
            .filter(oauth_tokens::Column::UserId.eq(current.user_id))
            .filter(oauth_tokens::Column::GrantType.eq(AUTHORIZATION_CODE_GRANT))
            .filter(oauth_tokens::Column::TokenKind.eq(oauth_tokens::TOKEN_KIND_REFRESH))
            .one(db)
            .await
            .map_err(|_| {
                OAuthTokenProtocolError::server_error("Failed to resolve refresh token family")
            })?
            .ok_or_else(|| {
                OAuthTokenProtocolError::invalid_grant("Refresh token family root is unavailable")
            })?;
        let now = clock()?;
        if root.family_id != root.id
            || root.authorization_code_id != current.authorization_code_id
            || root.consent_granted_at != current.consent_granted_at
            || root.created_at > current.created_at
            || committed_refresh_rotation_at(&root).is_none_or(|at| at > now)
        {
            return Err(OAuthTokenProtocolError::invalid_grant(
                "Refresh token family root is inconsistent",
            ));
        }
        let invalid_scopes = || OAuthTokenProtocolError::invalid_grant(
            "Refresh token family scopes are inconsistent",
        );
        let root_scopes = root.decoded_scopes().map_err(|_| invalid_scopes())?;
        let current_scopes = current.decoded_scopes().map_err(|_| invalid_scopes())?;
        validate_scope_subset(&root_scopes, &current_scopes).map_err(|_| invalid_scopes())?;
        root.created_at.with_timezone(&Utc)
    };

    Ok(RefreshTokenFamily {
        family_id,
        created_at,
    })
}

/// Rotation is serialized by the app/subject mutation fences. Reject a
/// forked family before consuming anything; choosing one branch would hide
/// inconsistent durable state and allow the other branch to survive.
async fn ensure_single_live_refresh<C>(
    db: &C,
    current: &oauth_tokens::Model,
    family: &RefreshTokenFamily,
    now: chrono::DateTime<Utc>,
) -> Result<(), OAuthTokenProtocolError>
where
    C: ConnectionTrait,
{
    let live = OAuthTokens::find()
        .filter(oauth_tokens::Column::AppId.eq(current.app_id))
        .filter(oauth_tokens::Column::TenantId.eq(current.tenant_id))
        .filter(oauth_tokens::Column::UserId.eq(current.user_id))
        .filter(oauth_tokens::Column::GrantType.eq(AUTHORIZATION_CODE_GRANT))
        .filter(oauth_tokens::Column::TokenKind.eq(oauth_tokens::TOKEN_KIND_REFRESH))
        .filter(oauth_tokens::Column::RevokedAt.is_null())
        .filter(oauth_tokens::Column::ExpiresAt.gt(now))
        .filter(oauth_tokens::Column::FamilyId.eq(family.family_id))
        .limit(2)
        .all(db)
        .await
        .map_err(|_| OAuthTokenProtocolError::server_error("Failed to validate OAuth refresh lineage"))?;
    if live.len() > 1 {
        return Err(OAuthTokenProtocolError::invalid_grant(
            "Refresh token family has multiple live branches",
        ));
    }
    Ok(())
}

/// Detect a replayed revoked OAuth refresh token and durably revoke the
/// entire active family. A DB failure must propagate so callers can retry.
/// Invalid/unknown tokens without a revoked record remain invalid_grant.
async fn detect_oauth_refresh_reuse(
    db: &DatabaseConnection,
    authenticated_app: &oauth_apps::Model,
    tenant_id: Uuid,
    token_hash: &str,
    clock: &OAuthClock<'_>,
) -> Result<(), OAuthTokenProtocolError> {
    let app_id = authenticated_app.id;
    // Reject unknown tokens without taking an app mutation lock. A recognized
    // revoked token needs a second authoritative read after that lock; the
    // preflight record itself must never drive a security-sensitive update.
    let candidate = OAuthTokens::find()
        .filter(oauth_tokens::Column::TokenHash.eq(token_hash))
        .filter(oauth_tokens::Column::AppId.eq(app_id))
        .filter(oauth_tokens::Column::TenantId.eq(tenant_id))
        .filter(oauth_tokens::Column::GrantType.eq(AUTHORIZATION_CODE_GRANT))
        .filter(oauth_tokens::Column::TokenKind.eq(oauth_tokens::TOKEN_KIND_REFRESH))
        .filter(oauth_tokens::Column::RevokedAt.is_not_null())
        .one(db)
        .await
        .map_err(|error| {
            tracing::error!(%error, %app_id, %tenant_id, "Failed to check OAuth refresh token reuse");
            OAuthTokenProtocolError::server_error("Failed to check refresh-token reuse")
        })?;
    let Some(candidate) = candidate else {
        return Ok(());
    };

    // Token issuance and refresh rotation both acquire the app row first.
    // Hold that same fence across the decisive replay read and family revoke,
    // so a concurrent exchange cannot mint a live successor after this update.
    // Replay revocation is itself an authenticated client mutation. An app
    // that rotated its secret or lost authority during preflight cannot
    // authorize this security effect with the stale client snapshot.
    let tx = db.begin().await.map_err(|error| {
        tracing::error!(%error, %app_id, %tenant_id, "Failed to start OAuth refresh reuse transaction");
        OAuthTokenProtocolError::server_error("Failed to begin refresh-token reuse revocation")
    })?;
    let locked_app = lock_active_oauth_app(&tx, app_id, tenant_id).await?;
    ensure_authenticated_client_unchanged(authenticated_app, &locked_app)?;
    require_grant(&locked_app, REFRESH_TOKEN_GRANT)?;
    let stale = OAuthTokens::find_by_id(candidate.id)
        .filter(oauth_tokens::Column::TokenHash.eq(token_hash))
        .filter(oauth_tokens::Column::AppId.eq(app_id))
        .filter(oauth_tokens::Column::TenantId.eq(tenant_id))
        .filter(oauth_tokens::Column::GrantType.eq(AUTHORIZATION_CODE_GRANT))
        .filter(oauth_tokens::Column::TokenKind.eq(oauth_tokens::TOKEN_KIND_REFRESH))
        .filter(oauth_tokens::Column::RevokedAt.is_not_null())
        .one(&tx)
        .await
        .map_err(|error| {
            tracing::error!(%error, %app_id, %tenant_id, "Failed to recheck OAuth refresh replay under lock");
            OAuthTokenProtocolError::server_error("Failed to recheck refresh-token reuse")
        })?;
    let Some(stale) = stale else {
        tx.rollback().await.map_err(|_| {
            OAuthTokenProtocolError::server_error("Failed to finish refresh-token reuse check")
        })?;
        return Ok(());
    };
    verify_application_clock(&tx).await?;
    let now = clock()?;
    // A rotation recorded in the future is not elapsed-grace evidence.
    // Do not destroy a lineage because another replica's clock is ahead.
    if committed_refresh_rotation_at(&stale).is_some_and(|at| at > now) {
        return Err(OAuthTokenProtocolError::server_error("OAuth rotation time is in the future"));
    }
    if within_refresh_rotation_grace(&stale, now) {
        // Do not keep the SQLite writer reserved until a detached Drop rollback
        // completes; release it before the caller returns invalid_grant.
        tx.rollback().await.map_err(|_| {
            OAuthTokenProtocolError::server_error("Failed to finish refresh-token reuse check")
        })?;
        return Ok(());
    }

    let family_id = stale.family_id;
    let user_id = stale.user_id.ok_or_else(|| {
        OAuthTokenProtocolError::server_error("Replayed OAuth refresh token has no subject")
    })?;
    tracing::warn!(
        app_id = %app_id,
        user_id = ?stale.user_id,
        family_id = %family_id,
        "OAuth refresh token reuse detected; revoking token family"
    );

    let result = OAuthTokens::update_many()
        .col_expr(oauth_tokens::Column::RevokedAt, Expr::value(now))
        .col_expr(oauth_tokens::Column::UpdatedAt, Expr::value(now))
        .filter(oauth_tokens::Column::AppId.eq(app_id))
        .filter(oauth_tokens::Column::TenantId.eq(tenant_id))
        .filter(oauth_tokens::Column::UserId.eq(user_id))
        .filter(oauth_tokens::Column::RevokedAt.is_null())
        .filter(oauth_tokens::Column::FamilyId.eq(family_id))
        .exec(&tx)
        .await;
    if let Err(error) = result {
        tracing::error!(
            %error,
            family_id = %family_id,
            "failed to revoke OAuth refresh token family after reuse detection"
        );
        tx.rollback().await.map_err(|rollback| {
            tracing::error!(%rollback, family_id = %family_id, "Failed to roll back OAuth replay revocation");
            OAuthTokenProtocolError::server_error("Failed to roll back refresh-token reuse revocation")
        })?;
        return Err(OAuthTokenProtocolError::server_error(
            "Failed to revoke replayed refresh-token family",
        ));
    }
    verify_application_clock(&tx).await?;
    clock()?;
    tx.commit().await.map_err(|error| {
        tracing::error!(%error, family_id = %family_id, "Failed to commit OAuth refresh replay revocation");
        OAuthTokenProtocolError::server_error("Failed to commit refresh-token reuse revocation")
    })?;
    Ok(())
}

/// Revoke the delegated token lineage issued from one authorization code.
/// RFC 6749 §4.1.2 requires replay of an authorization code to invalidate the
/// tokens previously issued from that code. The predicate is scoped to the
/// authenticated client, tenant, and subject and covers both registered access
/// rows and refresh rows, so already-issued access JWTs are fenced too.
async fn revoke_authorization_code_family(
    db: &DatabaseConnection,
    authenticated_app: &oauth_apps::Model,
    code: &oauth_codes::Model,
) -> Result<(), OAuthTokenProtocolError> {
    // A replayed authorization code can be observed outside the exchange
    // transaction. Revoke its lineage under the same app-row fence used by
    // code exchange and refresh issuance, then commit the revocation before
    // returning invalid_grant to the caller.
    let tx = db.begin().await.map_err(|_| {
        OAuthTokenProtocolError::server_error("Failed to begin authorization-code replay revocation")
    })?;
    let locked_app = lock_active_oauth_app(&tx, code.app_id, code.tenant_id).await?;
    ensure_authenticated_client_unchanged(authenticated_app, &locked_app)?;
    require_grant(&locked_app, AUTHORIZATION_CODE_GRANT)?;
    if let Err(error) = revoke_authorization_code_family_in_tx(&tx, code).await {
        tx.rollback().await.map_err(|_| {
            OAuthTokenProtocolError::server_error("Failed to roll back authorization-code replay revocation")
        })?;
        return Err(error);
    }
    tx.commit().await.map_err(|_| {
        OAuthTokenProtocolError::server_error("Failed to commit authorization-code replay revocation")
    })
}

async fn revoke_authorization_code_family_in_tx<C>(
    db: &C,
    code: &oauth_codes::Model,
) -> Result<(), OAuthTokenProtocolError>
where
    C: ConnectionTrait,
{
    let now = Utc::now();
    let revoked = OAuthTokens::update_many()
        .col_expr(oauth_tokens::Column::RevokedAt, Expr::value(now))
        .col_expr(oauth_tokens::Column::UpdatedAt, Expr::value(now))
        .filter(oauth_tokens::Column::AuthorizationCodeId.eq(code.id))
        .filter(oauth_tokens::Column::AppId.eq(code.app_id))
        .filter(oauth_tokens::Column::TenantId.eq(code.tenant_id))
        .filter(oauth_tokens::Column::UserId.eq(code.user_id))
        .filter(oauth_tokens::Column::GrantType.eq(AUTHORIZATION_CODE_GRANT))
        .filter(oauth_tokens::Column::RevokedAt.is_null())
        .exec(db)
        .await
        .map_err(|_| {
            OAuthTokenProtocolError::server_error("Failed to revoke replayed authorization code")
        })?;
    tracing::warn!(
        app_id = %code.app_id,
        tenant_id = %code.tenant_id,
        user_id = %code.user_id,
        authorization_code_id = %code.id,
        revoked_tokens = revoked.rows_affected,
        "OAuth authorization code replay detected; revoked issued token family"
    );
    Ok(())
}

/// Revoke every live delegated token row for a user inside the caller's
/// credential-change transaction. Registered access rows are revoked here,
/// and the user's `password_changed_at` epoch remains a second fence at
/// validation time.
pub(crate) async fn revoke_user_delegated_tokens_in_tx<C>(
    db: &C,
    tenant_id: Uuid,
    user_id: Uuid,
    now: chrono::DateTime<Utc>,
) -> Result<u64, sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    let revoked = oauth_tokens::Entity::update_many()
        .col_expr(oauth_tokens::Column::RevokedAt, Expr::value(now))
        .col_expr(oauth_tokens::Column::UpdatedAt, Expr::value(now))
        .filter(oauth_tokens::Column::TenantId.eq(tenant_id))
        .filter(oauth_tokens::Column::UserId.eq(user_id))
        .filter(oauth_tokens::Column::GrantType.eq(AUTHORIZATION_CODE_GRANT))
        .filter(oauth_tokens::Column::RevokedAt.is_null())
        .exec(db)
        .await?;
    oauth_codes::Entity::update_many()
        .col_expr(oauth_codes::Column::UsedAt, Expr::value(now))
        .filter(oauth_codes::Column::TenantId.eq(tenant_id))
        .filter(oauth_codes::Column::UserId.eq(user_id))
        .filter(oauth_codes::Column::UsedAt.is_null())
        .exec(db)
        .await?;
    Ok(revoked.rows_affected)
}

async fn validate_active_subject_and_consent(
    db: &DatabaseConnection,
    app: &oauth_apps::Model,
    tenant_id: Uuid,
    user_id: Uuid,
    scopes: &[String],
) -> Result<(users::Model, Option<sea_orm::prelude::DateTimeWithTimeZone>), OAuthTokenProtocolError>
{
    let user = Users::find_by_id(user_id)
        .filter(users::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(|_| OAuthTokenProtocolError::server_error("Failed to validate OAuth subject"))?
        .filter(|user| user.is_active())
        .ok_or_else(|| {
            OAuthTokenProtocolError::invalid_grant("OAuth subject is missing or inactive")
        })?;

    let mut consent_granted_at = None;
    if app.requires_user_consent() {
        let consent = OAuthConsents::find_active_consent(db, app.id, user.id, tenant_id)
            .await
            .map_err(|_| OAuthTokenProtocolError::server_error("Failed to validate OAuth consent"))?
            .ok_or_else(|| {
                OAuthTokenProtocolError::invalid_grant("OAuth consent is missing or revoked")
            })?;
        let consent_scopes = consent.scopes_list().map_err(|_| {
            OAuthTokenProtocolError::invalid_grant("Stored OAuth consent scopes are invalid")
        })?;
        validate_scope_subset(&consent_scopes, scopes)?;
        consent_granted_at = Some(consent.granted_at);
    }

    Ok((user, consent_granted_at))
}

async fn validate_locked_oauth_consent<C>(
    db: &C,
    app: &oauth_apps::Model,
    tenant_id: Uuid,
    user_id: Uuid,
    scopes: &[String],
    expected_granted_at: Option<sea_orm::prelude::DateTimeWithTimeZone>,
) -> Result<(), OAuthTokenProtocolError>
where
    C: ConnectionTrait,
{
    if !app.requires_user_consent() {
        return Ok(());
    }

    let consent = OAuthConsents::find_active_consent(db, app.id, user_id, tenant_id)
        .await
        .map_err(|_| {
            OAuthTokenProtocolError::server_error(
                "Failed to revalidate OAuth consent during token issuance",
            )
        })?
        .ok_or_else(|| {
            OAuthTokenProtocolError::invalid_grant(
                "OAuth consent was revoked during token issuance",
            )
        })?;

    let consent_scopes = consent.scopes_list().map_err(|_| {
        OAuthTokenProtocolError::invalid_grant("Stored OAuth consent scopes are invalid")
    })?;
    validate_scope_subset(&consent_scopes, scopes)?;
    if expected_granted_at.as_ref() != Some(&consent.granted_at) {
        return Err(OAuthTokenProtocolError::invalid_grant(
            "OAuth consent changed during token issuance",
        ));
    }

    Ok(())
}

async fn lock_active_oauth_app<C>(
    db: &C,
    app_id: Uuid,
    tenant_id: Uuid,
) -> Result<oauth_apps::Model, OAuthTokenProtocolError>
where
    C: ConnectionTrait,
{
    let app = lock_oauth_app_for_mutation(db, app_id, tenant_id)
        .await
        .map_err(|_| OAuthTokenProtocolError::server_error("Failed to lock OAuth client"))?;
    let app =
        app.ok_or_else(|| OAuthTokenProtocolError::invalid_client("OAuth client is unavailable"))?;
    if !app.is_active() {
        return Err(OAuthTokenProtocolError::invalid_client(
            "OAuth client is inactive",
        ));
    }
    Ok(app)
}

async fn lock_active_oauth_subject<C>(
    db: &C,
    tenant_id: Uuid,
    user_id: Uuid,
    credential_issued_at: sea_orm::prelude::DateTimeWithTimeZone,
    require_email_verification: bool,
) -> Result<users::Model, OAuthTokenProtocolError>
where
    C: ConnectionTrait,
{
    let query = || Users::find_by_id(user_id).filter(users::Column::TenantId.eq(tenant_id));
    let user = match db.get_database_backend() {
        DbBackend::Postgres | DbBackend::MySql => {
            query().lock_exclusive().one(db).await.map_err(|_| {
                OAuthTokenProtocolError::server_error("Failed to lock OAuth subject")
            })?
        }
        DbBackend::Sqlite => {
            // The subject id is known. Acquire the SQLite write fence before
            // reading current status/password epoch so issuance cannot act on
            // a stale pre-lock snapshot.
            let locked = users::Entity::update_many()
                .col_expr(
                    users::Column::UpdatedAt,
                    Expr::col(users::Column::UpdatedAt),
                )
                .filter(users::Column::Id.eq(user_id))
                .filter(users::Column::TenantId.eq(tenant_id))
                .exec(db)
                .await
                .map_err(|_| {
                    OAuthTokenProtocolError::server_error("Failed to lock OAuth subject")
                })?;
            if locked.rows_affected != 1 {
                return Err(OAuthTokenProtocolError::invalid_grant(
                    "OAuth subject is unavailable",
                ));
            }
            query().one(db).await.map_err(|_| {
                OAuthTokenProtocolError::server_error("Failed to reload OAuth subject")
            })?
        }
        _ => {
            return Err(OAuthTokenProtocolError::server_error(
                "Unsupported database backend for OAuth token issuance",
            ));
        }
    };
    let user =
        user.ok_or_else(|| OAuthTokenProtocolError::invalid_grant("OAuth subject is unavailable"))?;
    if !user.is_active()
        || user.rejects_delegated_token_issued_at(credential_issued_at.with_timezone(&Utc))
        || (require_email_verification && !user.is_email_verified())
    {
        return Err(OAuthTokenProtocolError::invalid_grant(
            "OAuth subject is inactive, unverified, or its credentials changed",
        ));
    }
    Ok(user)
}

fn ensure_authenticated_client_unchanged(
    authenticated_app: &oauth_apps::Model,
    current_app: &oauth_apps::Model,
) -> Result<(), OAuthTokenProtocolError> {
    // Client authentication also depends on app type (public vs confidential).
    // An unchanged secret hash alone cannot authenticate a changed identity
    // or a client whose required authentication method changed.
    if !current_app.is_active()
        || authenticated_app.id != current_app.id
        || authenticated_app.tenant_id != current_app.tenant_id
        || authenticated_app.client_id != current_app.client_id
        || authenticated_app.app_type != current_app.app_type
        || authenticated_app.client_secret_hash != current_app.client_secret_hash
    {
        return Err(OAuthTokenProtocolError::invalid_client(
            "OAuth client credentials changed during token issuance",
        ));
    }
    Ok(())
}

fn prepare_service_access_token(
    app: &oauth_apps::Model,
    config: &AuthConfig,
    scopes: &[String],
) -> Result<PreparedOAuthTokens, OAuthTokenProtocolError> {
    let granted_permissions = app.parsed_granted_permissions().map_err(|_| {
        OAuthTokenProtocolError::server_error("OAuth app permission policy is invalid")
    })?;
    let role = infer_user_role_from_permissions(&granted_permissions);
    let token_id = Uuid::new_v4();
    let issued = auth::issue_oauth_access_token(
        config,
        OauthAccessTokenInput {
            app_id: app.id,
            tenant_id: app.tenant_id,
            role,
            client_id: app.client_id,
            scopes,
            grant_type: CLIENT_CREDENTIALS_GRANT,
            expires_in_secs: SERVICE_ACCESS_TOKEN_TTL_SECS,
            token_id,
        },
        None,
    )
    .map_err(|_| OAuthTokenProtocolError::server_error("Failed to encode access token"))?;
    let access_model = oauth_tokens::ActiveModel {
        id: Set(token_id),
        app_id: Set(app.id),
        user_id: Set(None),
        tenant_id: Set(app.tenant_id),
        token_hash: Set(auth::hash_refresh_token(&issued.token)),
        family_id: Set(token_id),
        authorization_code_id: Set(None),
        grant_type: Set(CLIENT_CREDENTIALS_GRANT.to_string()),
        token_kind: Set(oauth_tokens::TOKEN_KIND_ACCESS.to_string()),
        consent_granted_at: Set(None),
        scopes: Set(serde_json::to_value(scopes).map_err(|_| {
            OAuthTokenProtocolError::server_error("Failed to serialize token scopes")
        })?),
        expires_at: Set(issued.expires_at.into()),
        revoked_at: Set(None),
        last_used_at: Set(None),
        created_at: Set(issued.issued_at.into()),
        updated_at: Set(issued.issued_at.into()),
    };
    Ok(PreparedOAuthTokens {
        access_token: issued.token,
        access_model,
        refresh_token: None,
        refresh_model: None,
        consent_granted_at: None,
        expires_in: SERVICE_ACCESS_TOKEN_TTL_SECS,
    })
}

async fn prepare_user_tokens(
    db: &DatabaseConnection,
    app: &oauth_apps::Model,
    config: &AuthConfig,
    user_id: Uuid,
    scopes: &[String],
    include_refresh_token: bool,
    family: Option<&RefreshTokenFamily>,
    authorization_code_id: Option<Uuid>,
    derived_refresh: Option<String>,
    clock: &OAuthClock<'_>,
) -> Result<PreparedOAuthTokens, OAuthTokenProtocolError> {
    let (_user, consent_granted_at) =
        validate_active_subject_and_consent(db, app, app.tenant_id, user_id, scopes).await?;
    let permissions = RbacService::get_user_permissions_authoritative(db, &app.tenant_id, &user_id)
        .await
        .map_err(|_| {
            OAuthTokenProtocolError::server_error("Failed to resolve current user permissions")
        })?;
    let role = infer_user_role_from_permissions(&permissions);
    let now = clock()?;
    let token_id = Uuid::new_v4();
    let (family_id, family_created_at) = match family {
        Some(family) => {
            ensure_refresh_family_live(family, now)?;
            (family.family_id, family.created_at)
        }
        None => (token_id, now),
    };
    let family_deadline = refresh_family_deadline(family_created_at)?;
    remaining_access_lifetime(family_deadline, now, USER_ACCESS_TOKEN_TTL_SECS)?;
    let access_token_id = Uuid::new_v4();
    let issued = auth::issue_oauth_access_token(
        config,
        OauthAccessTokenInput {
            app_id: user_id,
            tenant_id: app.tenant_id,
            role,
            client_id: app.client_id,
            scopes,
            grant_type: AUTHORIZATION_CODE_GRANT,
            expires_in_secs: USER_ACCESS_TOKEN_TTL_SECS,
            token_id: access_token_id,
        },
        Some(family_deadline),
    )
    .map_err(|_| OAuthTokenProtocolError::server_error("Failed to encode access token"))?;
    if issued.issued_at < now {
        return Err(OAuthTokenProtocolError::invalid_grant("OAuth issuance clock moved backwards"));
    }
    let scopes_json = serde_json::to_value(scopes)
        .map_err(|_| OAuthTokenProtocolError::server_error("Failed to serialize token scopes"))?;

    let refresh_token = if include_refresh_token {
        Some(match derived_refresh {
            Some(token) => token,
            None => auth::generate_refresh_token().map_err(|_| {
                OAuthTokenProtocolError::server_error("Failed to generate refresh token")
            })?,
        })
    } else {
        None
    };
    let refresh_hash = refresh_token.as_deref().map(auth::hash_refresh_token);
    // Without a refresh grant the access row roots its own revocation
    // family; with one, it joins the refresh lineage so RFC 7009 and
    // reuse/replay revocation fence every issued credential together.
    let access_family_id = if include_refresh_token {
        family_id
    } else {
        access_token_id
    };
    let access_model = oauth_tokens::ActiveModel {
        id: Set(access_token_id),
        app_id: Set(app.id),
        user_id: Set(Some(user_id)),
        tenant_id: Set(app.tenant_id),
        token_hash: Set(auth::hash_refresh_token(&issued.token)),
        family_id: Set(access_family_id),
        authorization_code_id: Set(authorization_code_id),
        grant_type: Set(AUTHORIZATION_CODE_GRANT.to_string()),
        token_kind: Set(oauth_tokens::TOKEN_KIND_ACCESS.to_string()),
        consent_granted_at: Set(consent_granted_at),
        scopes: Set(scopes_json.clone()),
        expires_at: Set(issued.expires_at.into()),
        revoked_at: Set(None),
        last_used_at: Set(None),
        created_at: Set(issued.issued_at.into()),
        updated_at: Set(issued.issued_at.into()),
    };

    let sliding_deadline = now
        .checked_add_signed(chrono::Duration::days(USER_REFRESH_TOKEN_TTL_DAYS))
        .ok_or_else(|| {
            OAuthTokenProtocolError::server_error("Refresh token expiry is out of range")
        })?;
    let refresh_model = refresh_hash.map(|refresh_hash| {
        let expires_at = sliding_deadline.min(family_deadline);
        oauth_tokens::ActiveModel {
            id: Set(token_id),
            app_id: Set(app.id),
            user_id: Set(Some(user_id)),
            tenant_id: Set(app.tenant_id),
            token_hash: Set(refresh_hash),
            family_id: Set(family_id),
            authorization_code_id: Set(authorization_code_id),
            grant_type: Set(AUTHORIZATION_CODE_GRANT.to_string()),
            token_kind: Set(oauth_tokens::TOKEN_KIND_REFRESH.to_string()),
            consent_granted_at: Set(consent_granted_at),
            scopes: Set(scopes_json),
            expires_at: Set(expires_at.into()),
            revoked_at: Set(None),
            last_used_at: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
    });

    Ok(PreparedOAuthTokens {
        access_token: issued.token,
        access_model,
        refresh_token,
        refresh_model,
        consent_granted_at,
        expires_in: USER_ACCESS_TOKEN_TTL_SECS,
    })
}

async fn commit_authorization_code_exchange(
    db: &DatabaseConnection,
    authenticated_app: &oauth_apps::Model,
    code: &oauth_codes::Model,
    prepared: &PreparedOAuthTokens,
    require_email_verification: bool,
    clock: &OAuthClock<'_>,
) -> Result<(), OAuthTokenProtocolError> {
    let tx = db.begin().await.map_err(|_| {
        OAuthTokenProtocolError::server_error("Failed to begin authorization-code exchange")
    })?;
    let current_app = lock_active_oauth_app(&tx, authenticated_app.id, code.tenant_id).await?;
    ensure_authenticated_client_unchanged(authenticated_app, &current_app)?;
    require_grant(&current_app, AUTHORIZATION_CODE_GRANT)?;
    if prepared.refresh_model.is_some() {
        // Preparation is outside the transaction; a removed refresh grant
        // must not result in a persisted refresh credential.
        require_grant(&current_app, REFRESH_TOKEN_GRANT)?;
    }
    validate_code_consent_epoch(code, prepared.consent_granted_at.as_ref())?;
    if !current_app
        .redirect_uris_list()
        .contains(&code.redirect_uri)
    {
        return Err(OAuthTokenProtocolError::invalid_grant(
            "Authorization code redirect URI is no longer registered",
        ));
    }
    let code_scopes = code.scopes_list().map_err(|_| {
        OAuthTokenProtocolError::invalid_grant("Authorization code scopes are invalid")
    })?;
    validate_scope_subset(&current_app.scopes_list(), &code_scopes)?;
    lock_active_oauth_subject(
        &tx,
        code.tenant_id,
        code.user_id,
        code.created_at.clone(),
        require_email_verification,
    )
    .await?;
    validate_locked_oauth_consent(
        &tx,
        &current_app,
        code.tenant_id,
        code.user_id,
        &code_scopes,
        prepared.consent_granted_at.clone(),
    )
    .await?;

    verify_application_clock(&tx).await?;
    let now = clock()?;
    validate_prepared_deadlines(prepared, now, None)?;
    if !consume_authorization_code(&tx, code, now).await? {
        // A concurrent exchange won the CAS. If the code is now marked used,
        // revoke its issued lineage and commit that security effect before
        // reporting invalid_grant. An expired-but-unused code grants nothing.
        let used = oauth_codes::Entity::find_by_id(code.id)
            .filter(oauth_codes::Column::AppId.eq(code.app_id))
            .filter(oauth_codes::Column::TenantId.eq(code.tenant_id))
            .one(&tx)
            .await
            .map_err(|_| {
                OAuthTokenProtocolError::server_error("Failed to inspect authorization code race")
            })?
            .and_then(|stored| stored.used_at)
            .is_some();
        if used {
            revoke_authorization_code_family_in_tx(&tx, code).await?;
            tx.commit().await.map_err(|_| {
                OAuthTokenProtocolError::server_error(
                    "Failed to commit authorization-code replay revocation",
                )
            })?;
        } else {
            tx.rollback().await.map_err(|_| {
                OAuthTokenProtocolError::server_error(
                    "Failed to roll back authorization-code exchange",
                )
            })?;
        }
        return Err(OAuthTokenProtocolError::invalid_grant(
            "Authorization code is invalid, expired, or already used",
        ));
    }
    persist_prepared_tokens(&tx, prepared).await?;
    verify_application_clock(&tx).await?;
    validate_prepared_deadlines(prepared, clock()?, None)?;

    tx.commit().await.map_err(|_| {
        OAuthTokenProtocolError::server_error("Failed to commit authorization-code exchange")
    })
}

#[derive(Debug)]
struct OAuthRecoveryWindow {
    rotated_at: chrono::DateTime<Utc>,
    deadline: chrono::DateTime<Utc>,
}

impl OAuthRecoveryWindow {
    fn ensure_live(&self, now: chrono::DateTime<Utc>) -> Result<(), OAuthTokenProtocolError> {
        if now < self.rotated_at || now >= self.deadline {
            return Err(OAuthTokenProtocolError::invalid_grant("OAuth refresh recovery expired"));
        }
        Ok(())
    }
}

/// Resolve only the hash-matched, still-live successor of this operation.
/// The caller holds the canonical app and subject mutation locks.
async fn recover_refresh_in_tx<C: ConnectionTrait>(
    db: &C,
    stale: &oauth_tokens::Model,
    family: &RefreshTokenFamily,
    prepared: &PreparedOAuthTokens,
    clock: &OAuthClock<'_>,
) -> Result<OAuthRecoveryWindow, OAuthTokenProtocolError> {
    let invalid = || OAuthTokenProtocolError::invalid_grant("OAuth refresh recovery unavailable");
    let rotated_at = committed_refresh_rotation_at(stale).ok_or_else(invalid)?;
    let now = clock()?;
    if !within_refresh_rotation_grace(stale, now) {
        return Err(invalid());
    }
    let candidate = prepared.refresh_model.as_ref().ok_or_else(invalid)?;
    let (Set(hash), Set(scopes)) = (&candidate.token_hash, &prepared.access_model.scopes) else {
        return Err(OAuthTokenProtocolError::server_error("Missing OAuth refresh recovery candidate"));
    };
    let successor = OAuthTokens::find()
        .filter(oauth_tokens::Column::TokenHash.eq(hash))
        .filter(oauth_tokens::Column::AppId.eq(stale.app_id))
        .filter(oauth_tokens::Column::TenantId.eq(stale.tenant_id))
        .filter(oauth_tokens::Column::UserId.eq(stale.user_id))
        .filter(oauth_tokens::Column::GrantType.eq(AUTHORIZATION_CODE_GRANT))
        .filter(oauth_tokens::Column::TokenKind.eq(oauth_tokens::TOKEN_KIND_REFRESH))
        .filter(oauth_tokens::Column::FamilyId.eq(family.family_id))
        .filter(oauth_tokens::Column::RevokedAt.is_null())
        .filter(oauth_tokens::Column::ExpiresAt.gt(now))
        .one(db)
        .await
        .map_err(|_| OAuthTokenProtocolError::server_error("Failed to resolve OAuth refresh successor"))?
        .ok_or_else(invalid)?;
    if successor.id == stale.id
        || successor.created_at < stale.created_at
        || successor.authorization_code_id != stale.authorization_code_id
        || successor.consent_granted_at != stale.consent_granted_at
        || &successor.scopes != scopes
    {
        return Err(invalid());
    }
    let successor_family = resolve_refresh_family(db, &successor, clock).await?;
    if successor_family.family_id != family.family_id || successor_family.created_at != family.created_at {
        return Err(invalid());
    }
    let absolute_deadline = refresh_family_deadline(family.created_at)?;
    if successor.expires_at.with_timezone(&Utc) > absolute_deadline {
        return Err(invalid());
    }
    let window = OAuthRecoveryWindow {
        rotated_at,
        deadline: rotated_at.checked_add_signed(chrono::Duration::seconds(OAUTH_REFRESH_REUSE_GRACE_SECONDS))
            .ok_or_else(invalid)?
            .min(successor.expires_at.with_timezone(&Utc))
            .min(absolute_deadline),
    };
    window.ensure_live(clock()?)?;
    ensure_single_live_refresh(db, &successor, family, clock()?).await?;
    verify_application_clock(db).await?;
    let now = clock()?;
    window.ensure_live(now)?;
    validate_prepared_deadlines(prepared, now, Some(absolute_deadline))?;
    // Fresh access JWT, same refresh bearer. Never insert the prepared
    // refresh model or update the successor's lifetime during recovery.
    prepared.access_model.clone().insert(db).await
        .map_err(|_| OAuthTokenProtocolError::server_error("Failed to register recovered OAuth access token"))?;
    verify_application_clock(db).await?;
    let now = clock()?;
    window.ensure_live(now)?;
    validate_prepared_deadlines(prepared, now, Some(absolute_deadline))?;
    Ok(window)
}

async fn commit_refresh_rotation(
    db: &DatabaseConnection,
    authenticated_app: &oauth_apps::Model,
    current: &oauth_tokens::Model,
    prepared: &PreparedOAuthTokens,
    require_email_verification: bool,
    allow_recovery: bool,
    clock: &OAuthClock<'_>,
) -> Result<Option<OAuthRecoveryWindow>, OAuthTokenProtocolError> {
    let tx = db.begin().await.map_err(|_| {
        OAuthTokenProtocolError::server_error("Failed to begin refresh-token rotation")
    })?;
    let current_app = lock_active_oauth_app(&tx, authenticated_app.id, current.tenant_id).await?;
    ensure_authenticated_client_unchanged(authenticated_app, &current_app)?;
    require_grant(&current_app, REFRESH_TOKEN_GRANT)?;
    validate_scope_subset(&current_app.scopes_list(), &current.scopes_list())?;
    let user_id = current.user_id.ok_or_else(|| {
        OAuthTokenProtocolError::invalid_grant("Refresh token has no associated user")
    })?;
    lock_active_oauth_subject(
        &tx,
        current.tenant_id,
        user_id,
        current.created_at.clone(),
        require_email_verification,
    )
    .await?;
    validate_locked_oauth_consent(
        &tx,
        &current_app,
        current.tenant_id,
        user_id,
        &current.scopes_list(),
        current.consent_granted_at.clone(),
    )
    .await?;

    if prepared.consent_granted_at != current.consent_granted_at {
        return Err(OAuthTokenProtocolError::invalid_grant("OAuth consent changed during refresh preparation"));
    }
    let stored = OAuthTokens::find_by_id(current.id)
        .filter(oauth_tokens::Column::AppId.eq(current.app_id))
        .filter(oauth_tokens::Column::TenantId.eq(current.tenant_id))
        .filter(oauth_tokens::Column::TokenKind.eq(oauth_tokens::TOKEN_KIND_REFRESH))
        .one(&tx).await
        .map_err(|_| OAuthTokenProtocolError::server_error("Failed to reload OAuth refresh credential"))?
        .ok_or_else(|| OAuthTokenProtocolError::invalid_grant("OAuth refresh credential unavailable"))?;
    // Only consumption/revocation metadata may change since preflight.
    let mut expected = current.clone();
    expected.revoked_at = stored.revoked_at;
    expected.last_used_at = stored.last_used_at;
    expected.updated_at = stored.updated_at;
    if expected != stored {
        return Err(OAuthTokenProtocolError::invalid_grant("OAuth refresh credential changed"));
    }
    let family = resolve_refresh_family(&tx, &stored, clock).await?;
    verify_application_clock(&tx).await?;
    let now = clock()?;
    ensure_refresh_family_live(&family, now)?;
    validate_prepared_deadlines(prepared, now, Some(refresh_family_deadline(family.created_at)?))?;
    ensure_single_live_refresh(&tx, &stored, &family, now).await?;
    if stored.revoked_at.is_some() && allow_recovery {
        // Preflight admitted a live predecessor or a rotated one in grace.
        // If the request ages out while waiting, refuse recovery without
        // converting the admitted request into theft evidence. New post-grace
        // presentations are handled by validate_refresh_token at admission.
        let window = recover_refresh_in_tx(&tx, &stored, &family, prepared, clock).await?;
        tx.commit().await.map_err(|_| OAuthTokenProtocolError::server_error("Failed to commit OAuth refresh recovery"))?;
        window.ensure_live(clock()?)?;
        return Ok(Some(window));
    }
    // The lineage queries can consume the last portion of the lifetime.
    verify_application_clock(&tx).await?;
    let now = clock()?;
    ensure_refresh_family_live(&family, now)?;
    validate_prepared_deadlines(prepared, now, Some(refresh_family_deadline(family.created_at)?))?;
    let consumed = OAuthTokens::update_many()
        .col_expr(oauth_tokens::Column::RevokedAt, Expr::value(now))
        .col_expr(oauth_tokens::Column::LastUsedAt, Expr::value(now))
        .col_expr(oauth_tokens::Column::UpdatedAt, Expr::value(now))
        .filter(oauth_tokens::Column::Id.eq(current.id))
        .filter(oauth_tokens::Column::TokenHash.eq(&current.token_hash))
        .filter(oauth_tokens::Column::AppId.eq(current.app_id))
        .filter(oauth_tokens::Column::TenantId.eq(current.tenant_id))
        .filter(oauth_tokens::Column::TokenKind.eq(oauth_tokens::TOKEN_KIND_REFRESH))
        .filter(oauth_tokens::Column::RevokedAt.is_null())
        .filter(oauth_tokens::Column::ExpiresAt.gt(now))
        .exec(&tx)
        .await
        .map_err(|_| OAuthTokenProtocolError::server_error("Failed to consume refresh token"))?;
    if consumed.rows_affected != 1 {
        tx.rollback().await.map_err(|_| {
            OAuthTokenProtocolError::server_error("Failed to roll back refresh-token rotation")
        })?;
        detect_oauth_refresh_reuse(
            db,
            authenticated_app,
            current.tenant_id,
            &current.token_hash,
            clock,
        )
        .await?;
        return Err(OAuthTokenProtocolError::invalid_grant(
            "Refresh token is invalid, expired, or already used",
        ));
    }

    persist_prepared_tokens(&tx, prepared).await?;
    verify_application_clock(&tx).await?;
    validate_prepared_deadlines(prepared, clock()?, Some(refresh_family_deadline(family.created_at)?))?;
    tx.commit().await.map_err(|_| {
        OAuthTokenProtocolError::server_error("Failed to commit refresh-token rotation")
    })?;
    Ok(None)
}

async fn consume_authorization_code<C>(
    db: &C,
    code: &oauth_codes::Model,
    now: chrono::DateTime<Utc>,
) -> Result<bool, OAuthTokenProtocolError>
where
    C: ConnectionTrait,
{
    let consumed = oauth_codes::Entity::update_many()
        .col_expr(oauth_codes::Column::UsedAt, Expr::value(now))
        .filter(oauth_codes::Column::Id.eq(code.id))
        .filter(oauth_codes::Column::AppId.eq(code.app_id))
        .filter(oauth_codes::Column::TenantId.eq(code.tenant_id))
        .filter(oauth_codes::Column::UsedAt.is_null())
        .filter(oauth_codes::Column::CreatedAt.lte(now))
        .filter(oauth_codes::Column::ExpiresAt.gt(now))
        .exec(db)
        .await
        .map_err(|_| {
            OAuthTokenProtocolError::server_error("Failed to consume authorization code")
        })?;

    Ok(consumed.rows_affected == 1)
}

async fn persist_prepared_tokens<C>(
    db: &C,
    prepared: &PreparedOAuthTokens,
) -> Result<(), OAuthTokenProtocolError>
where
    C: ConnectionTrait,
{
    // A fresh family must insert its refresh root before its access row.
    // Both writes remain inside the caller's one-shot transaction.
    if let Some(model) = prepared.refresh_model.clone() {
        model.insert(db).await.map_err(|_| {
            OAuthTokenProtocolError::server_error("Failed to persist replacement refresh token")
        })?;
    }
    prepared
        .access_model
        .clone()
        .insert(db)
        .await
        .map_err(|_| OAuthTokenProtocolError::server_error("Failed to persist access token"))?;
    Ok(())
}

fn required<'a>(
    value: Option<&'a str>,
    description: &'static str,
) -> Result<&'a str, OAuthTokenProtocolError> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| OAuthTokenProtocolError::invalid_request(description))
}

#[cfg(test)]
mod tests {
    use super::{
        PreparedOAuthTokens, REFRESH_TOKEN_GRANT, USER_REFRESH_FAMILY_ABSOLUTE_LIFETIME_DAYS,
        authenticate_client, commit_refresh_rotation,
        lock_active_oauth_subject, refresh_family_deadline, require_grant, resolve_client,
        scopes_for_refresh, validate_requested_scopes,
    };
    use crate::models::{
        oauth_apps, oauth_authorization_codes as oauth_codes, oauth_consents, oauth_tokens,
        tenants, users,
    };
    use chrono::Utc;
    use rustok_auth::TokenRequest;
    use sea_orm::{
        ActiveModelTrait, ColumnTrait, ConnectionTrait, Database, EntityTrait, PaginatorTrait,
        QueryFilter, Schema, Set, TransactionTrait, prelude::DateTimeWithTimeZone,
    };
    use uuid::Uuid;

    fn test_clock() -> Result<chrono::DateTime<Utc>, super::OAuthTokenProtocolError> {
        Ok(Utc::now())
    }

    #[tokio::test]
    async fn oauth_subject_lock_requires_verified_email_when_enabled() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("SQLite database");
        let backend = db.get_database_backend();
        let schema = Schema::new(backend);
        db.execute_raw(backend.build(&schema.create_table_from_entity(tenants::Entity)))
            .await
            .expect("tenants table");
        db.execute_raw(backend.build(&schema.create_table_from_entity(users::Entity)))
            .await
            .expect("users table");

        let tenant = tenants::ActiveModel::new("Email verification tenant", "email-verification")
            .insert(&db)
            .await
            .expect("tenant");
        let user = users::ActiveModel::new(tenant.id, "unverified@example.com", "hash")
            .insert(&db)
            .await
            .expect("unverified user");

        let tx = db.begin().await.expect("begin unverified subject transaction");
        let error = lock_active_oauth_subject(
            &tx,
            tenant.id,
            user.id,
            user.created_at.clone(),
            true,
        )
        .await
        .expect_err("unverified OAuth subjects must be rejected");
        assert_eq!(error.error, "invalid_grant");
        tx.rollback().await.expect("rollback unverified subject transaction");

        users::ActiveModel {
            id: Set(user.id),
            email_verified_at: Set(Some(Utc::now().into())),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("verify user");

        let tx = db.begin().await.expect("begin verified subject transaction");
        lock_active_oauth_subject(
            &tx,
            tenant.id,
            user.id,
            user.created_at,
            true,
        )
        .await
        .expect("verified OAuth subject must be accepted");
        tx.rollback().await.expect("rollback verified subject transaction");
    }

    #[tokio::test]
    async fn first_party_pkce_does_not_require_secret_even_when_ssr_secret_exists() {
        let mut app = app(
            serde_json::json!(["profile"]),
            serde_json::json!(["authorization_code", "refresh_token", "client_credentials"]),
        );
        app.app_type = "first_party".to_string();
        app.client_secret_hash = Some("stored-ssr-secret-hash".to_string());

        let request = TokenRequest {
            grant_type: "authorization_code".to_string(),
            client_id: Some(app.client_id.to_string()),
            client_secret: None,
            scope: None,
            code: None,
            redirect_uri: None,
            code_verifier: None,
            refresh_token: None,
        };

        assert!(authenticate_client(&app, &request, false).await.is_ok());
    }

    #[tokio::test]
    async fn first_party_client_credentials_requires_a_provisioned_secret() {
        let mut app = app(
            serde_json::json!(["profile"]),
            serde_json::json!(["authorization_code", "refresh_token", "client_credentials"]),
        );
        app.app_type = "first_party".to_string();
        app.client_secret_hash = None;

        let request = TokenRequest {
            grant_type: "client_credentials".to_string(),
            client_id: Some(app.client_id.to_string()),
            client_secret: None,
            scope: None,
            code: None,
            redirect_uri: None,
            code_verifier: None,
            refresh_token: None,
        };

        assert!(authenticate_client(&app, &request, true).await.is_err());
    }

    #[test]
    fn grant_types_are_enforced_against_application_type_at_exchange_time() {
        let mut third_party = app(
            serde_json::json!(["catalog:read"]),
            serde_json::json!(["client_credentials"]),
        );
        third_party.app_type = "third_party".to_string();
        assert!(require_grant(&third_party, "client_credentials").is_err());

        let mut mobile = third_party.clone();
        mobile.app_type = "mobile".to_string();
        assert!(require_grant(&mobile, "client_credentials").is_err());

        let mut service = third_party;
        service.app_type = "service".to_string();
        service.grant_types = serde_json::json!(["authorization_code"]);
        assert!(require_grant(&service, "authorization_code").is_err());

        let mut first_party = app(
            serde_json::json!(["catalog:read"]),
            serde_json::json!([
                "authorization_code",
                "refresh_token",
                "client_credentials"
            ]),
        );
        first_party.app_type = "first_party".to_string();
        assert!(require_grant(&first_party, "authorization_code").is_ok());
        assert!(require_grant(&first_party, "client_credentials").is_ok());
    }

    fn app(scopes: serde_json::Value, grants: serde_json::Value) -> oauth_apps::Model {
        let now: DateTimeWithTimeZone = chrono::Utc::now().into();
        oauth_apps::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            name: "test".to_string(),
            slug: "test".to_string(),
            description: None,
            app_type: "third_party".to_string(),
            icon_url: None,
            client_id: Uuid::new_v4(),
            client_secret_hash: Some("hash".to_string()),
            redirect_uris: serde_json::json!([]),
            scopes,
            grant_types: grants,
            granted_permissions: serde_json::json!([]),
            manifest_ref: None,
            auto_created: false,
            is_active: true,
            revoked_at: None,
            last_used_at: None,
            metadata: serde_json::json!({}),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn authenticated_client_snapshot_rejects_identity_and_policy_drift() {
        let original = app(
            serde_json::json!(["profile"]),
            serde_json::json!(["authorization_code", "refresh_token"]),
        );
        assert!(super::ensure_authenticated_client_unchanged(&original, &original).is_ok());

        let mut changed = original.clone();
        changed.app_type = "mobile".to_string();
        assert_eq!(
            super::ensure_authenticated_client_unchanged(&original, &changed).unwrap_err().error,
            "invalid_client"
        );
        let mut changed = original.clone();
        changed.client_id = Uuid::new_v4();
        assert!(super::ensure_authenticated_client_unchanged(&original, &changed).is_err());
        let mut changed = original.clone();
        changed.tenant_id = Uuid::new_v4();
        assert!(super::ensure_authenticated_client_unchanged(&original, &changed).is_err());
        let mut changed = original.clone();
        changed.id = Uuid::new_v4();
        assert!(super::ensure_authenticated_client_unchanged(&original, &changed).is_err());
        let mut changed = original.clone();
        changed.client_secret_hash = None;
        assert!(super::ensure_authenticated_client_unchanged(&original, &changed).is_err());
        let mut changed = original.clone();
        changed.is_active = false;
        assert!(super::ensure_authenticated_client_unchanged(&original, &changed).is_err());
    }

    #[tokio::test]
    async fn authorization_code_consumption_requires_current_validity_interval() {
        let db = Database::connect("sqlite::memory:").await.expect("SQLite database");
        // Isolate the consumption CAS from unrelated user/app fixtures.
        db.execute_unprepared(
            r#"CREATE TABLE oauth_authorization_codes (
                id TEXT PRIMARY KEY NOT NULL,
                app_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                tenant_id TEXT NOT NULL,
                code_hash TEXT NOT NULL,
                redirect_uri TEXT NOT NULL,
                scopes TEXT NOT NULL,
                code_challenge TEXT NOT NULL,
                code_challenge_method TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                used_at TEXT NULL,
                created_at TEXT NOT NULL
            )"#,
        )
        .await
        .expect("authorization codes table");
        // Use exact seconds so database timestamp precision cannot affect boundaries.
        let now = chrono::DateTime::<Utc>::from_timestamp(1_800_000_000, 0).unwrap();
        for (created, expires, admissible) in [
            (now + chrono::Duration::seconds(1), now + chrono::Duration::minutes(1), false),
            (now - chrono::Duration::minutes(1), now, false),
            (now, now + chrono::Duration::minutes(1), true),
        ] {
            let code = oauth_codes::ActiveModel {
                id: Set(Uuid::new_v4()),
                app_id: Set(Uuid::new_v4()),
                user_id: Set(Uuid::new_v4()),
                tenant_id: Set(Uuid::new_v4()),
                code_hash: Set(Uuid::new_v4().to_string()),
                redirect_uri: Set("https://example.com/callback".to_string()),
                scopes: Set(serde_json::json!(["openid"])),
                code_challenge: Set("challenge".to_string()),
                code_challenge_method: Set("S256".to_string()),
                expires_at: Set(expires.into()),
                used_at: Set(None),
                created_at: Set(created.into()),
            }
            .insert(&db)
            .await
            .expect("insert authorization code");
            assert_eq!(code.is_active_at(now), admissible);
            assert_eq!(super::consume_authorization_code(&db, &code, now).await.unwrap(), admissible);
            let stored = oauth_codes::Entity::find_by_id(code.id)
                .one(&db).await.unwrap().unwrap();
            assert_eq!(stored.used_at.map(|at| at.with_timezone(&Utc)), admissible.then_some(now));
            assert!(!super::consume_authorization_code(&db, &code, now).await.unwrap());
        }
    }

    #[test]
    fn authorization_code_cannot_adopt_a_newer_consent_epoch() {
        let now = Utc::now();
        let code = oauth_codes::Model {
            id: Uuid::new_v4(),
            app_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            code_hash: "digest".to_string(),
            redirect_uri: "https://example.com/callback".to_string(),
            scopes: serde_json::json!(["profile"]),
            code_challenge: "challenge".to_string(),
            code_challenge_method: "S256".to_string(),
            expires_at: (now + chrono::Duration::minutes(10)).into(),
            used_at: None,
            created_at: now.into(),
        };
        assert_eq!(
            code.scopes_list().expect("valid stored scopes"),
            vec!["profile".to_string()]
        );
        for malformed in [
            serde_json::json!(["profile", 42]),
            serde_json::json!(["profile", null]),
            serde_json::json!("profile"),
        ] {
            let mut damaged = code.clone();
            damaged.scopes = malformed;
            assert!(damaged.scopes_list().is_err());
        }
        let older = (now - chrono::Duration::seconds(1)).into();
        let newer = (now + chrono::Duration::seconds(1)).into();
        assert!(super::validate_code_consent_epoch(&code, None).is_ok());
        assert!(super::validate_code_consent_epoch(&code, Some(&older)).is_ok());
        assert!(super::validate_code_consent_epoch(&code, Some(&code.created_at)).is_ok());
        let error = super::validate_code_consent_epoch(&code, Some(&newer))
            .expect_err("a stale code cannot adopt later consent");
        assert_eq!(error.error, "invalid_grant");
    }

    #[tokio::test]
    async fn locked_consent_revalidation_rejects_stale_grant_epoch_and_scopes() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("SQLite database");
        let backend = db.get_database_backend();
        let schema = Schema::new(backend);
        db.execute_raw(backend.build(&schema.create_table_from_entity(tenants::Entity)))
            .await
            .expect("tenants table");
        db.execute_raw(backend.build(&schema.create_table_from_entity(oauth_apps::Entity)))
            .await
            .expect("OAuth apps table");
        db.execute_raw(backend.build(&schema.create_table_from_entity(users::Entity)))
            .await
            .expect("users table");
        db.execute_raw(backend.build(&schema.create_table_from_entity(oauth_consents::Entity)))
            .await
            .expect("OAuth consents table");
        db.execute_raw(backend.build(&schema.create_table_from_entity(oauth_tokens::Entity)))
            .await
            .expect("OAuth tokens table");

        let tenant = tenants::ActiveModel::new("Consent lock tenant", "consent-lock")
            .insert(&db)
            .await
            .expect("tenant");
        let mut client = app(
            serde_json::json!(["profile", "email"]),
            serde_json::json!(["authorization_code", "refresh_token"]),
        );
        client.tenant_id = tenant.id;
        let active_app: crate::models::_entities::oauth_apps::ActiveModel = client.clone().into();
        active_app.insert(&db).await.expect("OAuth app");
        let user = users::ActiveModel::new(
            tenant.id,
            "consent-lock@example.com",
            "unused-password-hash",
        )
        .insert(&db)
        .await
        .expect("OAuth subject");

        let granted_at: DateTimeWithTimeZone = Utc::now().into();
        let consent = oauth_consents::ActiveModel {
            id: Set(Uuid::new_v4()),
            app_id: Set(client.id),
            user_id: Set(user.id),
            tenant_id: Set(tenant.id),
            scopes: Set(serde_json::json!(["profile", "email"])),
            granted_at: Set(granted_at.clone()),
            revoked_at: Set(None),
        }
        .insert(&db)
        .await
        .expect("active consent");

        let now = Utc::now();
        let current_id = Uuid::new_v4();
        let current = oauth_tokens::ActiveModel {
            id: Set(current_id),
            app_id: Set(client.id),
            user_id: Set(Some(user.id)),
            tenant_id: Set(tenant.id),
            token_hash: Set("consent-bound-refresh-token".to_string()),
            family_id: Set(current_id),
            authorization_code_id: Set(None),
            grant_type: Set(super::AUTHORIZATION_CODE_GRANT.to_string()),
            token_kind: Set(oauth_tokens::TOKEN_KIND_REFRESH.to_string()),
            consent_granted_at: Set(Some(granted_at.clone())),
            scopes: Set(serde_json::json!(["profile"])),
            expires_at: Set((now + chrono::Duration::hours(1)).into()),
            revoked_at: Set(None),
            last_used_at: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&db)
        .await
        .expect("current refresh token");
        let prepared = PreparedOAuthTokens {
            access_token: "access-after-consent-change".to_string(),
            access_model: oauth_tokens::ActiveModel {
                id: Set(Uuid::new_v4()),
                app_id: Set(client.id),
                user_id: Set(Some(user.id)),
                tenant_id: Set(tenant.id),
                token_hash: Set("access-after-consent-change".to_string()),
                family_id: Set(current_id),
                authorization_code_id: Set(None),
                grant_type: Set(super::AUTHORIZATION_CODE_GRANT.to_string()),
                token_kind: Set(oauth_tokens::TOKEN_KIND_ACCESS.to_string()),
                consent_granted_at: Set(Some(granted_at.clone())),
                scopes: Set(serde_json::json!(["profile"])),
                expires_at: Set((now + chrono::Duration::minutes(15)).into()),
                revoked_at: Set(None),
                last_used_at: Set(None),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            },
            refresh_token: Some("replacement-after-consent-change".to_string()),
            refresh_model: Some(oauth_tokens::ActiveModel {
                id: Set(Uuid::new_v4()),
                app_id: Set(client.id),
                user_id: Set(Some(user.id)),
                tenant_id: Set(tenant.id),
                token_hash: Set("replacement-after-consent-change".to_string()),
                family_id: Set(current_id),
                authorization_code_id: Set(None),
                grant_type: Set(super::AUTHORIZATION_CODE_GRANT.to_string()),
                token_kind: Set(oauth_tokens::TOKEN_KIND_REFRESH.to_string()),
                consent_granted_at: Set(Some(granted_at.clone())),
                scopes: Set(serde_json::json!(["profile"])),
                expires_at: Set((now + chrono::Duration::hours(1)).into()),
                revoked_at: Set(None),
                last_used_at: Set(None),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }),
            consent_granted_at: Some(granted_at.clone()),
            expires_in: 900,
        };

        let mut changed_consent: oauth_consents::ActiveModel = consent.clone().into();
        changed_consent.granted_at =
            Set((granted_at.clone() + chrono::Duration::seconds(1)).into());
        let changed_consent = changed_consent
            .update(&db)
            .await
            .expect("change consent epoch without revoking");

        let stale_epoch = super::validate_locked_oauth_consent(
            &db,
            &client,
            tenant.id,
            user.id,
            &["profile".to_string()],
            Some(granted_at.clone()),
        )
        .await
        .expect_err("a changed consent epoch must reject in-flight issuance");
        assert_eq!(stale_epoch.error, "invalid_grant");

        let commit_error = super::commit_refresh_rotation(&db, &client, &current, &prepared, false, false, &test_clock)
            .await
            .expect_err("refresh commit must revalidate consent under app/subject locks");
        assert_eq!(commit_error.error, "invalid_grant");
        let unchanged = oauth_tokens::Entity::find_by_id(current.id)
            .one(&db)
            .await
            .expect("reload refresh token")
            .expect("refresh token remains present");
        assert!(
            unchanged.revoked_at.is_none(),
            "a consent-epoch mismatch must not consume the presented refresh token"
        );
        assert_eq!(
            oauth_tokens::Entity::find()
                .count(&db)
                .await
                .expect("count OAuth tokens"),
            1,
            "a rejected in-flight rotation must not persist replacement credentials"
        );

        let mut narrowed: oauth_consents::ActiveModel = changed_consent.into();
        narrowed.scopes = Set(serde_json::json!(["profile"]));
        let narrowed = narrowed.update(&db).await.expect("narrow consent");
        let widened_token = super::validate_locked_oauth_consent(
            &db,
            &client,
            tenant.id,
            user.id,
            &["profile".to_string(), "email".to_string()],
            Some(narrowed.granted_at.clone()),
        )
        .await
        .expect_err("token scopes must still be covered by current consent");
        assert_eq!(widened_token.error, "invalid_scope");

        let epoch = narrowed.granted_at.clone();
        let mut malformed: oauth_consents::ActiveModel = narrowed.into();
        malformed.scopes = Set(serde_json::json!(["profile", 42]));
        let malformed = malformed.update(&db).await.expect("persist malformed consent");
        assert!(malformed.scopes_list().is_err());

        let error = super::validate_locked_oauth_consent(
            &db,
            &client,
            tenant.id,
            user.id,
            &["profile".to_string()],
            Some(epoch),
        )
        .await
        .expect_err("partially malformed consent must not authorize token issuance");
        assert_eq!(error.error, "invalid_grant");
    }

    #[tokio::test]
    async fn refresh_reuse_revocation_failure_is_reported_and_can_be_retried() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("SQLite database");
        let backend = db.get_database_backend();
        let schema = Schema::new(backend);
        db.execute_raw(backend.build(&schema.create_table_from_entity(tenants::Entity)))
            .await
            .expect("tenants table");
        db.execute_raw(backend.build(&schema.create_table_from_entity(oauth_apps::Entity)))
            .await
            .expect("oauth_apps table");
        db.execute_raw(backend.build(&schema.create_table_from_entity(oauth_tokens::Entity)))
            .await
            .expect("oauth_tokens table");

        let app_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let family_id = Uuid::new_v4();
        let now = Utc::now();
        let raw_stale_token = "revoked-refresh-cookie";
        let stale_hash = crate::auth::hash_refresh_token(raw_stale_token);
        let mut client = app(
            serde_json::json!(["openid"]),
            serde_json::json!(["authorization_code", "refresh_token"]),
        );
        client.id = app_id;
        client.tenant_id = tenant_id;
        let mut tenant = tenants::ActiveModel::new("Replay test tenant", "replay-test-tenant");
        tenant.id = Set(tenant_id);
        tenant.insert(&db).await.expect("insert tenant");
        let active_app: crate::models::_entities::oauth_apps::ActiveModel = client.clone().into();
        active_app.insert(&db).await.expect("insert OAuth app");

        let make_token = |id, hash: &str, revoked_at| oauth_tokens::ActiveModel {
            id: Set(id),
            app_id: Set(app_id),
            user_id: Set(Some(user_id)),
            tenant_id: Set(tenant_id),
            token_hash: Set(hash.to_string()),
            family_id: Set(family_id),
            authorization_code_id: Set(None),
            grant_type: Set(super::AUTHORIZATION_CODE_GRANT.to_string()),
            token_kind: Set(oauth_tokens::TOKEN_KIND_REFRESH.to_string()),
            consent_granted_at: Set(None),
            scopes: Set(serde_json::json!(["openid"])),
            expires_at: Set((now + chrono::Duration::days(1)).into()),
            revoked_at: Set(revoked_at),
            last_used_at: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        };
        make_token(
            family_id,
            &stale_hash,
            Some((now - chrono::Duration::minutes(2)).into()),
        )
        .insert(&db)
        .await
        .expect("insert revoked root");
        let live_id = Uuid::new_v4();
        make_token(live_id, "live-hash", None)
            .insert(&db)
            .await
            .expect("insert live descendant");

        // Even a corrupt shared family id cannot revoke another subject's
        // credentials within the same app and tenant.
        let unrelated_id = Uuid::new_v4();
        let mut unrelated = make_token(unrelated_id, "other-subject-hash", None);
        unrelated.user_id = Set(Some(Uuid::new_v4()));
        unrelated
            .insert(&db)
            .await
            .expect("insert unrelated subject");

        let mut stale_client = client.clone();
        stale_client.client_secret_hash = Some("superseded-hash".to_string());
        let error = super::validate_refresh_token(&db, &stale_client, tenant_id, raw_stale_token, false, &test_clock)
            .await
            .expect_err("stale client cannot authorize refresh replay revocation");
        assert_eq!(error.error, "invalid_client");
        assert!(oauth_tokens::Entity::find_by_id(live_id)
            .one(&db)
            .await
            .expect("lookup")
            .expect("descendant")
            .revoked_at
            .is_none());

        db.execute_unprepared(
            "CREATE TRIGGER fail_refresh_reuse_revocation BEFORE UPDATE ON oauth_tokens
             BEGIN SELECT RAISE(FAIL, 'simulated persistence failure'); END;",
        )
        .await
        .expect("create fault injection");

        let error = super::validate_refresh_token(&db, &client, tenant_id, raw_stale_token, false, &test_clock)
            .await
            .expect_err("real token validation must surface revocation failure");
        assert_eq!(error.error, "server_error");
        assert!(
            oauth_tokens::Entity::find_by_id(live_id)
                .one(&db)
                .await
                .expect("lookup")
                .expect("live token")
                .revoked_at
                .is_none()
        );

        db.execute_unprepared("DROP TRIGGER fail_refresh_reuse_revocation")
            .await
            .expect("remove fault injection");
        let retry = super::validate_refresh_token(&db, &client, tenant_id, raw_stale_token, false, &test_clock)
            .await
            .expect_err("already-revoked stale token still cannot be used");
        assert_eq!(retry.error, "invalid_grant");
        assert!(
            oauth_tokens::Entity::find_by_id(live_id)
                .one(&db)
                .await
                .expect("lookup")
                .expect("descendant")
                .revoked_at
                .is_some()
        );
        assert!(
            oauth_tokens::Entity::find_by_id(unrelated_id)
                .one(&db)
                .await
                .expect("lookup")
                .expect("unrelated subject")
                .revoked_at
                .is_none()
        );
    }

    #[tokio::test]
    async fn credential_change_revokes_live_delegated_refresh_tokens() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("SQLite database");
        let backend = db.get_database_backend();
        let schema = Schema::new(backend);
        db.execute_raw(backend.build(&schema.create_table_from_entity(oauth_tokens::Entity)))
            .await
            .expect("oauth_tokens table");
        db.execute_unprepared(
            r#"CREATE TABLE oauth_authorization_codes (
                id TEXT PRIMARY KEY NOT NULL,
                app_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                tenant_id TEXT NOT NULL,
                code_hash TEXT NOT NULL,
                redirect_uri TEXT NOT NULL,
                scopes TEXT NOT NULL,
                code_challenge TEXT NOT NULL,
                code_challenge_method TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                used_at TEXT NULL,
                created_at TEXT NOT NULL
            )"#,
        )
        .await
        .expect("oauth_authorization_codes table");

        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let token_id = Uuid::new_v4();
        oauth_tokens::ActiveModel {
            id: Set(token_id),
            app_id: Set(Uuid::new_v4()),
            user_id: Set(Some(user_id)),
            tenant_id: Set(tenant_id),
            token_hash: Set("delegated-refresh-hash".to_string()),
            family_id: Set(token_id),
            authorization_code_id: Set(None),
            grant_type: Set(super::AUTHORIZATION_CODE_GRANT.to_string()),
            token_kind: Set(oauth_tokens::TOKEN_KIND_REFRESH.to_string()),
            consent_granted_at: Set(None),
            scopes: Set(serde_json::json!(["openid"])),
            expires_at: Set((now + chrono::Duration::days(1)).into()),
            revoked_at: Set(None),
            last_used_at: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&db)
        .await
        .expect("insert delegated refresh token");

        let code_id = Uuid::new_v4();
        oauth_codes::ActiveModel {
            id: Set(code_id),
            app_id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            tenant_id: Set(tenant_id),
            code_hash: Set("pending-authorization-code".to_string()),
            redirect_uri: Set("https://client.example/callback".to_string()),
            scopes: Set(serde_json::json!(["openid"])),
            code_challenge: Set("challenge".to_string()),
            code_challenge_method: Set("S256".to_string()),
            expires_at: Set((now + chrono::Duration::minutes(10)).into()),
            used_at: Set(None),
            created_at: Set(now.into()),
        }
        .insert(&db)
        .await
        .expect("insert pending authorization code");

        let revoked = super::revoke_user_delegated_tokens_in_tx(&db, tenant_id, user_id, now)
            .await
            .expect("revoke delegated tokens");
        assert_eq!(revoked, 1);

        let stored = oauth_tokens::Entity::find_by_id(token_id)
            .one(&db)
            .await
            .expect("query token")
            .expect("token exists");
        assert!(stored.revoked_at.is_some());

        let stored_code = oauth_codes::Entity::find_by_id(code_id)
            .one(&db)
            .await
            .expect("query authorization code")
            .expect("authorization code exists");
        assert!(stored_code.used_at.is_some());

        let second = super::revoke_user_delegated_tokens_in_tx(&db, tenant_id, user_id, now)
            .await
            .expect("idempotent revoke");
        assert_eq!(second, 0);
    }

    #[tokio::test]
    async fn resolve_client_does_not_depend_on_presentation_translations() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("SQLite database");
        let backend = db.get_database_backend();
        let schema = Schema::new(backend);
        db.execute_raw(
            backend.build(&schema.create_table_from_entity(crate::models::tenants::Entity)),
        )
        .await
        .expect("tenants table");
        db.execute_raw(backend.build(&schema.create_table_from_entity(oauth_apps::Entity)))
            .await
            .expect("OAuth apps table");

        let tenant_id = Uuid::new_v4();
        let mut tenant = crate::models::tenants::ActiveModel::new("Test tenant", "test-tenant");
        tenant.id = Set(tenant_id);
        tenant.insert(&db).await.expect("insert tenant");
        let app = app(
            serde_json::json!(["catalog:read"]),
            serde_json::json!(["client_credentials"]),
        );
        let client_id = app.client_id;
        // Insert at the entity level, bypassing the display-copy sync:
        // this test proves client resolution works without any
        // presentation translations being written.
        let mut active: crate::models::_entities::oauth_apps::ActiveModel = app.into();
        active.tenant_id = Set(tenant_id);
        active.insert(&db).await.expect("OAuth app");

        let request = TokenRequest {
            grant_type: "client_credentials".to_string(),
            client_id: Some(client_id.to_string()),
            client_secret: None,
            scope: Some("catalog:read".to_string()),
            code: None,
            redirect_uri: None,
            code_verifier: None,
            refresh_token: None,
        };

        let resolved = resolve_client(&db, tenant_id, &request)
            .await
            .expect("security client lookup");
        assert_eq!(resolved.client_id, client_id);
    }

    #[test]
    fn client_credentials_defaults_scopes_only_when_scope_is_omitted() {
        let app = app(
            serde_json::json!(["catalog:*", "profile"]),
            serde_json::json!(["client_credentials"]),
        );

        assert_eq!(
            validate_requested_scopes(&app, None).unwrap(),
            app.scopes_list()
        );
        assert_eq!(
            validate_requested_scopes(&app, Some("catalog:read")).unwrap(),
            vec!["catalog:read".to_string()]
        );
        for empty in ["", "   \t"] {
            assert_eq!(
                validate_requested_scopes(&app, Some(empty))
                    .unwrap_err()
                    .error,
                "invalid_scope"
            );
        }
    }

    #[test]
    fn client_credentials_rejects_missing_or_malformed_default_scopes() {
        for scopes in [
            serde_json::json!([]),
            serde_json::json!(null),
            serde_json::json!(["catalog:read", 42]),
        ] {
            let app = app(scopes, serde_json::json!(["client_credentials"]));
            assert_eq!(
                validate_requested_scopes(&app, None).unwrap_err().error,
                "invalid_scope"
            );
            assert_eq!(
                validate_requested_scopes(&app, Some("catalog:read"))
                    .unwrap_err()
                    .error,
                "invalid_scope"
            );
        }
    }

    #[test]
    fn empty_persisted_grant_cannot_be_exchanged_or_rotated() {
        let allowed = vec!["catalog:read".to_string()];
        assert_eq!(
            super::validate_scope_subset(&allowed, &[]).unwrap_err().error,
            "invalid_scope"
        );
        for requested in [None, Some(""), Some("catalog:read")] {
            assert_eq!(
                scopes_for_refresh(&[], requested).unwrap_err().error,
                "invalid_scope"
            );
        }
    }

    #[test]
    fn client_credentials_rejects_partial_scope_truncation() {
        let app = app(
            serde_json::json!(["catalog:*"]),
            serde_json::json!(["client_credentials"]),
        );
        assert!(validate_requested_scopes(&app, Some("catalog:read")).is_ok());
        assert!(validate_requested_scopes(&app, Some("catalog:read admin:*")).is_err());
    }

    #[test]
    fn refresh_family_deadline_rejects_unrepresentable_timestamps() {
        let now = Utc::now();
        assert_eq!(
            refresh_family_deadline(now).expect("normal family"),
            now + chrono::Duration::days(90)
        );
        let invalid = refresh_family_deadline(chrono::DateTime::<Utc>::MAX_UTC)
            .expect_err("corrupt future timestamp must not panic");
        assert_eq!(invalid.error, "invalid_grant");
    }

    #[test]
    fn refresh_family_lifetime_rejects_future_and_exact_deadline() {
        let now = Utc::now();
        let mut family = super::RefreshTokenFamily {
            family_id: Uuid::new_v4(),
            created_at: now + chrono::Duration::seconds(1),
        };
        assert_eq!(
            super::ensure_refresh_family_live(&family, now).unwrap_err().error,
            "invalid_grant"
        );
        family.created_at = now;
        let deadline = refresh_family_deadline(now).unwrap();
        assert!(super::ensure_refresh_family_live(
            &family,
            deadline - chrono::Duration::nanoseconds(1),
        )
        .is_ok());
        assert_eq!(
            super::ensure_refresh_family_live(&family, deadline).unwrap_err().error,
            "invalid_grant"
        );
    }

    #[test]
    fn service_registration_uses_exact_signed_expiry_and_rejects_future_creation() {
        let config = crate::auth::test_auth_config("test-secret-key-for-unit-tests-only-32bytes!".to_string());
        let mut client = app(
            serde_json::json!(["openid"]),
            serde_json::json!(["client_credentials"]),
        );
        client.app_type = "service".to_string();
        let scopes = vec!["openid".to_string()];
        let prepared = super::prepare_service_access_token(&client, &config, &scopes).unwrap();
        let claims = crate::auth::decode_access_token(&config, &prepared.access_token).unwrap();
        let expiry = super::prepared_token_expiry(&prepared.access_model).unwrap();
        let created = super::prepared_token_creation(&prepared.access_model).unwrap();
        assert_eq!(expiry.timestamp() as usize, claims.exp);
        assert_eq!(expiry.timestamp_subsec_nanos(), 0);
        assert_eq!(created.timestamp() as usize, claims.iat);
        assert!(super::validate_prepared_deadlines(&prepared, created - chrono::Duration::nanoseconds(1), None).is_err());
        assert_eq!(super::prepared_access_lifetime(
            &prepared.access_model, expiry - chrono::Duration::milliseconds(1999), prepared.expires_in,
        ).unwrap(), 1);
        assert!(super::validate_prepared_deadlines(&prepared, expiry, None).is_err());
    }

    #[test]
    fn remaining_access_lifetime_is_bounded_and_never_advertises_zero() {
        let now = Utc::now();
        assert_eq!(super::remaining_access_lifetime(
            now + chrono::Duration::milliseconds(1999), now, 900,
        ).unwrap(), 1);
        assert_eq!(super::remaining_access_lifetime(
            now + chrono::Duration::seconds(1000), now, 900,
        ).unwrap(), 900);
        assert!(super::remaining_access_lifetime(
            now + chrono::Duration::milliseconds(999), now, 900,
        ).is_err());
        assert!(super::remaining_access_lifetime(now, now, 900).is_err());
    }

    #[test]
    fn refresh_scope_is_never_widened_and_supports_narrowing() {
        let original = vec!["profile:read".to_string(), "email:read".to_string()];

        assert_eq!(scopes_for_refresh(&original, None).unwrap(), original);
        assert_eq!(
            scopes_for_refresh(&original, Some("profile:read")).unwrap(),
            vec!["profile:read".to_string()]
        );
        assert_eq!(
            scopes_for_refresh(&original, Some("email:read profile:read")).unwrap(),
            vec!["email:read".to_string(), "profile:read".to_string()]
        );
        assert_eq!(
            scopes_for_refresh(&original, Some("profile:read admin:write"))
                .unwrap_err()
                .error,
            "invalid_scope"
        );
        for empty in ["", "   \t"] {
            assert_eq!(
                scopes_for_refresh(&original, Some(empty))
                    .unwrap_err()
                    .error,
                "invalid_scope"
            );
        }
    }

    #[test]
    fn refresh_requires_explicit_grant() {
        let app = app(
            serde_json::json!(["profile"]),
            serde_json::json!(["authorization_code"]),
        );
        assert!(require_grant(&app, REFRESH_TOKEN_GRANT).is_err());
    }

    #[test]
    fn auto_created_apps_also_require_an_explicit_refresh_grant() {
        let mut auto_app = app(
            serde_json::json!(["profile"]),
            serde_json::json!(["authorization_code"]),
        );
        auto_app.auto_created = true;
        assert!(require_grant(&auto_app, REFRESH_TOKEN_GRANT).is_err());

        auto_app.grant_types = serde_json::json!(["authorization_code", "refresh_token"]);
        assert!(require_grant(&auto_app, REFRESH_TOKEN_GRANT).is_ok());
    }

    #[tokio::test]
    async fn refresh_rotation_consumes_a_token_exactly_once() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("SQLite database");
        let backend = db.get_database_backend();
        let schema = Schema::new(backend);
        db.execute_raw(backend.build(&schema.create_table_from_entity(tenants::Entity)))
            .await
            .expect("tenants table");
        db.execute_raw(backend.build(&schema.create_table_from_entity(oauth_apps::Entity)))
            .await
            .expect("OAuth apps table");
        db.execute_raw(backend.build(&schema.create_table_from_entity(users::Entity)))
            .await
            .expect("users table");
        db.execute_unprepared(
            r#"CREATE TABLE oauth_tokens (
                id TEXT PRIMARY KEY NOT NULL,
                app_id TEXT NOT NULL,
                user_id TEXT NULL,
                tenant_id TEXT NOT NULL,
                token_hash TEXT NOT NULL,
                grant_type TEXT NOT NULL,
                scopes TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                revoked_at TEXT NULL,
                last_used_at TEXT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                family_id TEXT NOT NULL,
                authorization_code_id TEXT NULL,
                token_kind TEXT NOT NULL DEFAULT 'refresh',
                consent_granted_at TEXT NULL
            )"#,
        )
        .await
        .expect("OAuth token table");

        let tenant_id = Uuid::new_v4();
        let mut tenant = tenants::ActiveModel::new("Refresh test tenant", "refresh-test-tenant");
        tenant.id = Set(tenant_id);
        tenant.insert(&db).await.expect("insert tenant");
        let app_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let mut client = app(
            serde_json::json!(["profile"]),
            serde_json::json!(["authorization_code", "refresh_token"]),
        );
        client.id = app_id;
        client.tenant_id = tenant_id;
        // This test exercises refresh CAS, not consent enforcement.
        // A first-party client supports delegated refresh without consent;
        // a service client is forbidden from using the refresh grant.
        client.app_type = "first_party".to_string();
        let active_app: crate::models::_entities::oauth_apps::ActiveModel = client.clone().into();
        active_app.insert(&db).await.expect("insert OAuth app");
        let mut subject = users::ActiveModel::new(
            tenant_id,
            "refresh-rotation@example.com",
            "unused-password-hash",
        );
        subject.id = Set(user_id);
        subject.insert(&db).await.expect("insert OAuth subject");

        let now = chrono::Utc::now();
        let current_id = Uuid::new_v4();
        let current = oauth_tokens::ActiveModel {
            id: Set(current_id),
            app_id: Set(app_id),
            user_id: Set(Some(user_id)),
            tenant_id: Set(tenant_id),
            token_hash: Set("current-hash".to_string()),
            family_id: Set(current_id),
            authorization_code_id: Set(None),
            grant_type: Set("authorization_code".to_string()),
            token_kind: Set(oauth_tokens::TOKEN_KIND_REFRESH.to_string()),
            consent_granted_at: Set(None),
            scopes: Set(serde_json::json!(["profile"])),
            expires_at: Set((now + chrono::Duration::hours(1)).into()),
            revoked_at: Set(None),
            last_used_at: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&db)
        .await
        .expect("current refresh token");

        let replacement = |hash: &str| PreparedOAuthTokens {
            access_token: "access".to_string(),
            access_model: oauth_tokens::ActiveModel {
                id: Set(Uuid::new_v4()),
                app_id: Set(current.app_id),
                user_id: Set(current.user_id),
                tenant_id: Set(current.tenant_id),
                token_hash: Set(format!("access-{hash}")),
                family_id: Set(current_id),
                authorization_code_id: Set(None),
                grant_type: Set("authorization_code".to_string()),
                token_kind: Set(oauth_tokens::TOKEN_KIND_ACCESS.to_string()),
                consent_granted_at: Set(None),
                scopes: Set(serde_json::json!(["profile"])),
                expires_at: Set((now + chrono::Duration::minutes(15)).into()),
                revoked_at: Set(None),
                last_used_at: Set(None),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            },
            refresh_token: Some("refresh".to_string()),
            refresh_model: Some(oauth_tokens::ActiveModel {
                id: Set(Uuid::new_v4()),
                app_id: Set(current.app_id),
                user_id: Set(current.user_id),
                tenant_id: Set(current.tenant_id),
                token_hash: Set(hash.to_string()),
                family_id: Set(current_id),
                authorization_code_id: Set(None),
                grant_type: Set("authorization_code".to_string()),
                token_kind: Set(oauth_tokens::TOKEN_KIND_REFRESH.to_string()),
                consent_granted_at: Set(None),
                scopes: Set(serde_json::json!(["profile"])),
                expires_at: Set((now + chrono::Duration::hours(1)).into()),
                revoked_at: Set(None),
                last_used_at: Set(None),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }),
            consent_granted_at: None,
            expires_in: 900,
        };

        // The final clock sample fails after both replacement rows were
        // written. Neither consumption nor the replacement may survive.
        let samples = std::sync::atomic::AtomicUsize::new(0);
        let failing_clock = || {
            if samples.fetch_add(1, std::sync::atomic::Ordering::SeqCst) >= 3 {
                Err(super::OAuthTokenProtocolError::server_error("injected clock fault"))
            } else {
                test_clock()
            }
        };
        let error = commit_refresh_rotation(
            &db, &client, &current, &replacement("clock-fault"), false, true, &failing_clock,
        ).await.expect_err("clock fault before commit must abort rotation");
        assert_eq!(error.error, "server_error");
        assert_eq!(samples.load(std::sync::atomic::Ordering::SeqCst), 4);
        assert_eq!(oauth_tokens::Entity::find().count(&db).await.unwrap(), 1);
        assert!(oauth_tokens::Entity::find_by_id(current.id)
            .one(&db).await.unwrap().unwrap().revoked_at.is_none());

        commit_refresh_rotation(&db, &client, &current, &replacement("replacement-1"), false, true, &test_clock)
            .await
            .expect("first rotation");

        // The replacement remains individually unexpired, but its original
        // family may cross the absolute deadline while issuance is in flight.
        let successor = oauth_tokens::Entity::find()
            .filter(oauth_tokens::Column::TokenHash.eq("replacement-1"))
            .one(&db)
            .await
            .expect("reload successor")
            .expect("successor exists");
        let fork_id = Uuid::new_v4();
        let mut fork: oauth_tokens::ActiveModel = successor.clone().into();
        fork.id = Set(fork_id);
        fork.token_hash = Set("forked-refresh".to_string());
        fork.insert(&db).await.expect("insert inconsistent live branch");
        let error = commit_refresh_rotation(
            &db, &client, &successor, &replacement("replacement-forked"), false, false, &test_clock,
        )
        .await
        .expect_err("a forked lineage must not rotate either live branch");
        assert_eq!(error.error, "invalid_grant");
        assert!(error.description.contains("multiple live branches"));
        for id in [successor.id, fork_id] {
            assert!(oauth_tokens::Entity::find_by_id(id)
                .one(&db).await.unwrap().unwrap().revoked_at.is_none());
        }
        oauth_tokens::Entity::delete_by_id(fork_id)
            .exec(&db).await.expect("remove injected fork");

        let mut root: oauth_tokens::ActiveModel = oauth_tokens::Entity::find_by_id(current_id)
            .one(&db)
            .await
            .expect("reload family root")
            .expect("family root exists")
            .into();
        root.created_at = Set(
            (now - chrono::Duration::days(USER_REFRESH_FAMILY_ABSOLUTE_LIFETIME_DAYS + 1)).into(),
        );
        let expired_root = root.update(&db).await.expect("expire family root");
        let expired_family = commit_refresh_rotation(
            &db,
            &client,
            &successor,
            &replacement("replacement-expired-family"),
            false,
            false, &test_clock,
        )
        .await
        .expect_err("rotation must recheck the absolute family deadline under lock");
        assert_eq!(expired_family.error, "invalid_grant");
        assert!(expired_family.description.contains("maximum lifetime"));
        let unchanged = oauth_tokens::Entity::find_by_id(successor.id)
            .one(&db)
            .await
            .expect("reload successor")
            .expect("successor remains present");
        assert!(unchanged.revoked_at.is_none());
        assert_eq!(oauth_tokens::Entity::find().count(&db).await.unwrap(), 3);

        let mut root: oauth_tokens::ActiveModel = expired_root.into();
        root.created_at = Set(
            (now - chrono::Duration::days(USER_REFRESH_FAMILY_ABSOLUTE_LIFETIME_DAYS)
                + chrono::Duration::seconds(5)).into(),
        );
        let nearly_expired_root = root.update(&db).await.expect("shorten family lifetime");
        let error = commit_refresh_rotation(
            &db, &client, &successor, &replacement("replacement-over-deadline"), false, false, &test_clock,
        )
        .await
        .expect_err("prepared credentials must not exceed the current absolute cap");
        assert_eq!(error.error, "invalid_grant");
        assert!(oauth_tokens::Entity::find_by_id(successor.id)
            .one(&db).await.unwrap().unwrap().revoked_at.is_none());

        let mut root: oauth_tokens::ActiveModel = nearly_expired_root.into();
        root.created_at = Set(current.created_at.clone());
        let root = root.update(&db).await.expect("restore family root for replay test");
        let rotated_at = root.revoked_at.unwrap().with_timezone(&Utc);
        assert!(super::within_refresh_rotation_grace(&root, rotated_at));
        assert!(!super::within_refresh_rotation_grace(
            &root, rotated_at + chrono::Duration::seconds(30),
        ));
        assert!(!super::within_refresh_rotation_grace(
            &root, rotated_at - chrono::Duration::nanoseconds(1),
        ));
        let mut explicitly_revoked = root;
        explicitly_revoked.last_used_at = None;
        assert!(!super::within_refresh_rotation_grace(&explicitly_revoked, rotated_at));

        let replay = commit_refresh_rotation(&db, &client, &current, &replacement("replacement-2"), false, false, &test_clock)
            .await
            .expect_err("refresh token replay must fail");

        assert_eq!(replay.error, "invalid_grant");
        assert_eq!(
            oauth_tokens::Entity::find()
                .count(&db)
                .await
                .expect("token count"),
            3
        );

        let wrong = commit_refresh_rotation(
            &db, &client, &current, &replacement("wrong-proof"), false, true, &test_clock,
        ).await.expect_err("wrong proof must not recover");
        assert_eq!(wrong.error, "invalid_grant");
        let mut altered_scope = replacement("replacement-1");
        altered_scope.access_model.scopes = Set(serde_json::json!(["other"]));
        assert!(commit_refresh_rotation(
            &db, &client, &current, &altered_scope, false, true, &test_clock,
        ).await.is_err());
        let mut recovery = replacement("replacement-1");
        recovery.access_model.token_hash = Set("recovered-access-hash".to_string());
        let window = commit_refresh_rotation(
            &db, &client, &current, &recovery, false, true, &test_clock,
        ).await.expect("matching proof recovers").expect("recovery window");
        assert!(window.ensure_live(window.rotated_at).is_ok());
        assert!(window.ensure_live(window.deadline).is_err());
        let retained = oauth_tokens::Entity::find_by_id(successor.id)
            .one(&db).await.unwrap().unwrap();
        assert_eq!(retained, successor);
        assert_eq!(oauth_tokens::Entity::find()
            .filter(oauth_tokens::Column::TokenKind.eq(oauth_tokens::TOKEN_KIND_REFRESH))
            .count(&db).await.unwrap(), 2);
        assert_eq!(oauth_tokens::Entity::find().count(&db).await.unwrap(), 4);

        // Once B rotates to C, the proof for A cannot recover B or C.
        let next_hash = "next-generation-refresh";
        commit_refresh_rotation(&db, &client, &successor, &replacement(next_hash), false, false, &test_clock)
            .await.expect("rotate successor");
        let error = commit_refresh_rotation(
            &db, &client, &current, &replacement("replacement-1"), false, true, &test_clock,
        ).await.expect_err("superseded successor must not recover");
        assert_eq!(error.error, "invalid_grant");
        let next = oauth_tokens::Entity::find()
            .filter(oauth_tokens::Column::TokenHash.eq(next_hash))
            .one(&db).await.unwrap().unwrap();
        assert!(next.revoked_at.is_none());

        // Model a request admitted before grace elapsed, then delayed.
        let past = (Utc::now() - chrono::Duration::seconds(31)).fixed_offset();
        let mut old: oauth_tokens::ActiveModel = oauth_tokens::Entity::find_by_id(current_id)
            .one(&db).await.unwrap().unwrap().into();
        old.created_at = Set(past - chrono::Duration::seconds(1));
        old.revoked_at = Set(Some(past));
        old.last_used_at = Set(Some(past));
        let old = old.update(&db).await.expect("age historical rotation");
        let delayed = commit_refresh_rotation(
            &db, &client, &old, &replacement("replacement-1"), false, true, &test_clock,
        ).await.expect_err("admitted recovery must not succeed beyond grace");
        assert_eq!(delayed.error, "invalid_grant");
        assert!(oauth_tokens::Entity::find_by_id(next.id)
            .one(&db).await.unwrap().unwrap().revoked_at.is_none());

        let mut changed_subject: users::ActiveModel = users::Entity::find_by_id(user_id)
            .one(&db)
            .await
            .expect("load refresh subject")
            .expect("refresh subject exists")
            .into();
        changed_subject.password_changed_at =
            Set(Some((now + chrono::Duration::seconds(2)).into()));
        changed_subject
            .update(&db)
            .await
            .expect("record password change");

        let pre_change_id = Uuid::new_v4();
        let pre_change = oauth_tokens::ActiveModel {
            id: Set(pre_change_id),
            app_id: Set(app_id),
            user_id: Set(Some(user_id)),
            tenant_id: Set(tenant_id),
            token_hash: Set("pre-change-refresh".to_string()),
            family_id: Set(current_id),
            authorization_code_id: Set(None),
            grant_type: Set("authorization_code".to_string()),
            token_kind: Set(oauth_tokens::TOKEN_KIND_REFRESH.to_string()),
            consent_granted_at: Set(None),
            scopes: Set(serde_json::json!(["profile"])),
            expires_at: Set((now + chrono::Duration::hours(1)).into()),
            revoked_at: Set(None),
            last_used_at: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&db)
        .await
        .expect("insert pre-change refresh token");
        let stale_credential = commit_refresh_rotation(
            &db,
            &client,
            &pre_change,
            &replacement("replacement-after-password-change"),
            false,
            false, &test_clock,
        )
        .await
        .expect_err("refresh issued before password change must fail");
        assert_eq!(stale_credential.error, "invalid_grant");
        let unchanged = oauth_tokens::Entity::find_by_id(pre_change_id)
            .one(&db)
            .await
            .expect("reload pre-change token")
            .expect("pre-change token exists");
        assert!(
            unchanged.revoked_at.is_none(),
            "rejected stale refresh must not consume the credential"
        );
    }

    #[tokio::test]
    async fn authorization_code_replay_revokes_only_its_scoped_lineage() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite");
        let backend = db.get_database_backend();
        let schema = Schema::new(backend);
        db.execute_raw(backend.build(&schema.create_table_from_entity(tenants::Entity)))
            .await
            .expect("tenants table");
        db.execute_raw(backend.build(&schema.create_table_from_entity(oauth_apps::Entity)))
            .await
            .expect("oauth_apps table");
        db.execute_unprepared(
            r#"CREATE TABLE oauth_tokens (
                id TEXT PRIMARY KEY NOT NULL,
                app_id TEXT NOT NULL,
                user_id TEXT NULL,
                tenant_id TEXT NOT NULL,
                token_hash TEXT NOT NULL,
                family_id TEXT NOT NULL,
                authorization_code_id TEXT NULL,
                grant_type TEXT NOT NULL,
                scopes TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                revoked_at TEXT NULL,
                last_used_at TEXT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                token_kind TEXT NOT NULL DEFAULT 'refresh',
                consent_granted_at TEXT NULL
            )"#,
        )
        .await
        .expect("create token table");
        let code_id = Uuid::new_v4();
        let app_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let mut tenant = tenants::ActiveModel::new("Code replay tenant", "code-replay-tenant");
        tenant.id = Set(tenant_id);
        tenant.insert(&db).await.expect("insert tenant");
        let mut client = app(
            serde_json::json!(["openid"]),
            serde_json::json!(["authorization_code", "refresh_token"]),
        );
        client.id = app_id;
        client.tenant_id = tenant_id;
        let active_app: crate::models::_entities::oauth_apps::ActiveModel = client.clone().into();
        active_app.insert(&db).await.expect("insert OAuth app");
        let now = Utc::now();
        let mut ids = Vec::new();
        for (index, (token_app, token_tenant, token_user, token_code)) in [
            (app_id, tenant_id, user_id, code_id),
            (app_id, tenant_id, user_id, code_id),
            (Uuid::new_v4(), tenant_id, user_id, code_id),
            (app_id, Uuid::new_v4(), user_id, code_id),
            (app_id, tenant_id, Uuid::new_v4(), code_id),
            (app_id, tenant_id, user_id, Uuid::new_v4()),
        ]
        .into_iter()
        .enumerate()
        {
            let id = Uuid::new_v4();
            ids.push(id);
            oauth_tokens::ActiveModel {
                id: Set(id),
                app_id: Set(token_app),
                user_id: Set(Some(token_user)),
                tenant_id: Set(token_tenant),
                token_hash: Set(format!("code-family-{index}")),
                family_id: Set(id),
                authorization_code_id: Set(Some(token_code)),
                grant_type: Set(super::AUTHORIZATION_CODE_GRANT.to_string()),
                token_kind: Set(oauth_tokens::TOKEN_KIND_REFRESH.to_string()),
                consent_granted_at: Set(None),
                scopes: Set(serde_json::json!(["openid"])),
                expires_at: Set((now + chrono::Duration::days(1)).into()),
                revoked_at: Set(None),
                last_used_at: Set(None),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(&db)
            .await
            .expect("insert token");
        }
        let code = oauth_codes::Model {
            id: code_id,
            app_id,
            user_id,
            tenant_id,
            code_hash: "code-hash".to_string(),
            redirect_uri: "https://client.example/callback".to_string(),
            scopes: serde_json::json!(["openid"]),
            code_challenge: "challenge".to_string(),
            code_challenge_method: "S256".to_string(),
            expires_at: (now + chrono::Duration::minutes(5)).into(),
            used_at: Some(now.into()),
            created_at: now.into(),
        };

        // A client authenticated with a superseded secret must not revoke
        // the lineage, even when it knows the original authorization code.
        let mut stale_client = client.clone();
        stale_client.client_secret_hash = Some("superseded-hash".to_string());
        let error = super::revoke_authorization_code_family(&db, &stale_client, &code)
            .await
            .expect_err("stale client cannot authorize replay revocation");
        assert_eq!(error.error, "invalid_client");
        for id in &ids[0..2] {
            assert!(oauth_tokens::Entity::find_by_id(*id)
                .one(&db)
                .await
                .expect("lookup")
                .expect("token")
                .revoked_at
                .is_none());
        }

        // A persistence error must be observable: replay revocation must not
        // silently report a security effect that the database did not commit.
        db.execute_unprepared(
            "CREATE TRIGGER fail_code_replay_revocation BEFORE UPDATE ON oauth_tokens
             BEGIN SELECT RAISE(FAIL, 'simulated code replay persistence failure'); END;",
        )
        .await
        .expect("create revocation fault injection");
        let error = super::revoke_authorization_code_family(&db, &client, &code)
            .await
            .expect_err("a failed code replay revocation must propagate");
        assert_eq!(error.error, "server_error");
        for id in &ids[0..2] {
            assert!(
                oauth_tokens::Entity::find_by_id(*id)
                    .one(&db)
                    .await
                    .expect("lookup after failed replay")
                    .expect("token still present")
                    .revoked_at
                    .is_none(),
                "failed replay revocation must not claim a committed effect"
            );
        }
        db.execute_unprepared("DROP TRIGGER fail_code_replay_revocation")
            .await
            .expect("remove revocation fault injection");
        super::revoke_authorization_code_family(&db, &client, &code)
            .await
            .expect("replay revocation succeeds after retry");

        for (index, id) in ids.into_iter().enumerate() {
            let stored = oauth_tokens::Entity::find_by_id(id)
                .one(&db)
                .await
                .expect("load token")
                .expect("token exists");
            assert_eq!(stored.revoked_at.is_some(), index < 2);
        }
        super::revoke_authorization_code_family(&db, &client, &code)
            .await
            .expect("replay revocation is idempotent");
    }

    #[tokio::test]
    async fn refresh_family_requires_a_consistent_root() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect sqlite");
        db.execute_unprepared(
            r#"CREATE TABLE oauth_tokens (
                id TEXT PRIMARY KEY NOT NULL,
                app_id TEXT NOT NULL,
                user_id TEXT NULL,
                tenant_id TEXT NOT NULL,
                token_hash TEXT NOT NULL,
                family_id TEXT NOT NULL,
                authorization_code_id TEXT NULL,
                grant_type TEXT NOT NULL,
                scopes TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                revoked_at TEXT NULL,
                last_used_at TEXT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                token_kind TEXT NOT NULL DEFAULT 'refresh',
                consent_granted_at TEXT NULL
            )"#,
        )
        .await
        .expect("create token table");
        let now = Utc::now();
        let token = oauth_tokens::ActiveModel {
            id: Set(Uuid::new_v4()),
            app_id: Set(Uuid::new_v4()),
            user_id: Set(Some(Uuid::new_v4())),
            tenant_id: Set(Uuid::new_v4()),
            token_hash: Set("orphan-family".to_string()),
            family_id: Set(Uuid::new_v4()),
            authorization_code_id: Set(None),
            grant_type: Set(super::AUTHORIZATION_CODE_GRANT.to_string()),
            token_kind: Set(oauth_tokens::TOKEN_KIND_REFRESH.to_string()),
            consent_granted_at: Set(None),
            scopes: Set(serde_json::json!(["openid"])),
            expires_at: Set((now + chrono::Duration::days(1)).into()),
            revoked_at: Set(None),
            last_used_at: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&db)
        .await
        .expect("insert token");

        let error = super::resolve_refresh_family(&db, &token, &test_clock)
            .await
            .expect_err("missing family root must not reset the absolute lifetime cap");
        assert_eq!(error.error, "invalid_grant");

        let root_id = token.family_id;
        let mut root: oauth_tokens::ActiveModel = token.clone().into();
        root.id = Set(root_id);
        root.token_hash = Set("family-root".to_string());
        root.family_id = Set(Uuid::new_v4());
        root.created_at = Set((now - chrono::Duration::days(1)).into());
        let root = root.insert(&db).await.expect("insert referenced row");
        assert_eq!(
            super::resolve_refresh_family(&db, &token, &test_clock).await.unwrap_err().error,
            "invalid_grant",
            "a descendant cannot serve as another family's root"
        );

        let mut root: oauth_tokens::ActiveModel = root.into();
        root.family_id = Set(root_id);
        root.created_at = Set((now + chrono::Duration::days(1)).into());
        let root = root.update(&db).await.expect("update referenced row");
        assert_eq!(
            super::resolve_refresh_family(&db, &token, &test_clock).await.unwrap_err().error,
            "invalid_grant",
            "a root cannot be newer than its descendant"
        );

        let mut root: oauth_tokens::ActiveModel = root.into();
        root.created_at = Set((now - chrono::Duration::days(1)).into());
        root.authorization_code_id = Set(Some(Uuid::new_v4()));
        let root = root.update(&db).await.expect("change root code");
        assert_eq!(
            super::resolve_refresh_family(&db, &token, &test_clock).await.unwrap_err().error,
            "invalid_grant",
            "a root must retain the original authorization-code identity"
        );

        let mut root: oauth_tokens::ActiveModel = root.into();
        root.authorization_code_id = Set(token.authorization_code_id);
        root.family_id = Set(root_id);
        let root = root.update(&db).await.expect("restore root identity");
        assert_eq!(
            super::resolve_refresh_family(&db, &token, &test_clock).await.unwrap_err().error,
            "invalid_grant",
            "a live root cannot have a live descendant"
        );

        let mut root: oauth_tokens::ActiveModel = root.into();
        let rotated_at = (now - chrono::Duration::seconds(1)).fixed_offset();
        root.revoked_at = Set(Some(rotated_at));
        root.last_used_at = Set(Some(rotated_at));
        root.consent_granted_at = Set(Some(rotated_at));
        let root = root.update(&db).await.expect("inject a foreign consent epoch");
        assert_eq!(
            super::resolve_refresh_family(&db, &token, &test_clock).await.unwrap_err().error,
            "invalid_grant",
            "a descendant cannot adopt another consent epoch"
        );
        let mut root: oauth_tokens::ActiveModel = root.into();
        root.consent_granted_at = Set(token.consent_granted_at);
        root.scopes = Set(serde_json::json!(["profile"]));
        let root = root.update(&db).await.expect("inject widened descendant scopes");
        assert_eq!(
            super::resolve_refresh_family(&db, &token, &test_clock).await.unwrap_err().error,
            "invalid_grant",
            "a descendant cannot exceed the original family scopes"
        );

        let mut root: oauth_tokens::ActiveModel = root.into();
        root.scopes = Set(token.scopes.clone());
        root.family_id = Set(root_id);
        let root = root.update(&db).await.expect("restore valid canonical root");
        let family = super::resolve_refresh_family(&db, &token, &test_clock)
            .await
            .expect("valid canonical root");
        assert_eq!(family.family_id, root_id);
        assert_eq!(family.created_at, root.created_at.with_timezone(&Utc));

        let mut malformed = token;
        malformed.family_id = Uuid::nil();
        assert_eq!(
            super::resolve_refresh_family(&db, &malformed, &test_clock).await.unwrap_err().error,
            "invalid_grant"
        );
    }
}
