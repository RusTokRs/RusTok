use axum::http::StatusCode;
use chrono::Utc;
use rustok_api::context::scope_matches;
use rustok_auth::{TokenRequest, TokenResponse};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, EntityTrait,
    QueryFilter, Set, TransactionTrait, sea_query::Expr,
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
use crate::services::oauth_app::OAuthAppService;
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
/// Replaying a rotated-away OAuth refresh token inside this window is
/// treated as a client retry race; after it, as theft (whole family
/// revoked). Mirrors the first-party session grace window.
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
    ) -> Result<TokenResponse, OAuthTokenProtocolError> {
        let auth_config = runtime.auth_config().ok_or_else(|| {
            OAuthTokenProtocolError::server_error("OAuth signing configuration is unavailable")
        })?;
        let db = runtime.runtime_ctx().db();
        let app = resolve_client(db, tenant_id, request).await?;

        let response = match request.grant_type.as_str() {
            CLIENT_CREDENTIALS_GRANT => {
                require_grant(&app, CLIENT_CREDENTIALS_GRANT)?;
                authenticate_client(&app, request, true).await?;
                let requested = parse_requested_scopes(request.scope.as_deref());
                let scopes = validate_requested_scopes(&app, &requested)?;
                let (access_token, expires_in) =
                    issue_service_access_token(&app, auth_config, &scopes)?;

                TokenResponse {
                    access_token,
                    token_type: "Bearer".to_string(),
                    expires_in,
                    refresh_token: None,
                    scope: scopes.join(" "),
                }
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
                    validate_authorization_code(db, &app, tenant_id, code, redirect_uri, verifier)
                        .await?;
                let scopes = authorization.scopes_list();
                let prepared = prepare_user_tokens(
                    db,
                    &app,
                    auth_config,
                    authorization.user_id,
                    &scopes,
                    app.supports_grant_type(REFRESH_TOKEN_GRANT),
                    None,
                )
                .await?;

                commit_authorization_code_exchange(db, &authorization, &prepared).await?;
                prepared.into_response(&scopes)
            }
            REFRESH_TOKEN_GRANT => {
                require_grant(&app, REFRESH_TOKEN_GRANT)?;
                authenticate_client(&app, request, false).await?;
                let raw_refresh_token = required(
                    request.refresh_token.as_deref(),
                    "refresh_token is required",
                )?;
                let current =
                    validate_refresh_token(db, &app, tenant_id, raw_refresh_token).await?;
                let user_id = current.user_id.ok_or_else(|| {
                    OAuthTokenProtocolError::invalid_grant("Refresh token has no associated user")
                })?;
                let family = resolve_refresh_family(db, &current).await?;
                if Utc::now()
                    >= family.created_at
                        + chrono::Duration::days(USER_REFRESH_FAMILY_ABSOLUTE_LIFETIME_DAYS)
                {
                    return Err(OAuthTokenProtocolError::invalid_grant(
                        "Refresh token family exceeded its maximum lifetime",
                    ));
                }
                let scopes = current.scopes_list();
                let prepared = prepare_user_tokens(
                    db,
                    &app,
                    auth_config,
                    user_id,
                    &scopes,
                    true,
                    Some(&family),
                )
                .await?;

                commit_refresh_rotation(db, &current, &prepared).await?;
                prepared.into_response(&scopes)
            }
            other => {
                return Err(OAuthTokenProtocolError::unsupported_grant(format!(
                    "Grant type `{other}` is not supported"
                )));
            }
        };

        if let Err(error) = OAuthAppService::touch_last_used(db, app.id).await {
            tracing::warn!(app_id = %app.id, error = %error, "Failed to update OAuth app usage timestamp");
        }

        Ok(response)
    }
}

struct PreparedUserTokens {
    access_token: String,
    refresh_token: Option<String>,
    refresh_model: Option<oauth_tokens::ActiveModel>,
    expires_in: u64,
}

impl PreparedUserTokens {
    fn into_response(self, scopes: &[String]) -> TokenResponse {
        TokenResponse {
            access_token: self.access_token,
            token_type: "Bearer".to_string(),
            expires_in: self.expires_in,
            refresh_token: self.refresh_token,
            scope: scopes.join(" "),
        }
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
    let app = oauth_apps::Entity::find_active_security_by_client_id(db, client_id)
        .await
        .map_err(|_| OAuthTokenProtocolError::server_error("Failed to resolve OAuth client"))?
        .ok_or_else(|| OAuthTokenProtocolError::invalid_client("Unknown or inactive client"))?;

    if app.tenant_id != tenant_id {
        return Err(OAuthTokenProtocolError::invalid_client(
            "Client is not registered for this tenant",
        ));
    }

    Ok(app)
}

fn require_grant(
    app: &oauth_apps::Model,
    grant_type: &'static str,
) -> Result<(), OAuthTokenProtocolError> {
    if app.supports_grant_type(grant_type) {
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

fn parse_requested_scopes(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split_whitespace()
        .filter(|scope| !scope.is_empty())
        .map(str::to_string)
        .collect()
}

fn validate_requested_scopes(
    app: &oauth_apps::Model,
    requested: &[String],
) -> Result<Vec<String>, OAuthTokenProtocolError> {
    let allowed = app.scopes_list();
    if requested.is_empty() {
        return Ok(allowed);
    }
    validate_scope_subset(&allowed, requested)?;
    Ok(requested.to_vec())
}

fn validate_scope_subset(
    allowed: &[String],
    requested: &[String],
) -> Result<(), OAuthTokenProtocolError> {
    if requested.iter().all(|scope| scope_matches(allowed, scope)) {
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
) -> Result<oauth_codes::Model, OAuthTokenProtocolError> {
    let code_hash = hex::encode(Sha256::digest(raw_code.as_bytes()));
    let code = oauth_codes::Entity::find()
        .filter(oauth_codes::Column::CodeHash.eq(code_hash))
        .filter(oauth_codes::Column::UsedAt.is_null())
        .filter(oauth_codes::Column::ExpiresAt.gt(Utc::now()))
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

    let scopes = code.scopes_list();
    validate_scope_subset(&app.scopes_list(), &scopes)?;
    validate_active_subject_and_consent(db, app, tenant_id, code.user_id, &scopes).await?;
    Ok(code)
}

async fn validate_refresh_token(
    db: &DatabaseConnection,
    app: &oauth_apps::Model,
    tenant_id: Uuid,
    raw_token: &str,
) -> Result<oauth_tokens::Model, OAuthTokenProtocolError> {
    let token_hash = auth::hash_refresh_token(raw_token);
    let token = OAuthTokens::find()
        .filter(oauth_tokens::Column::TokenHash.eq(token_hash.clone()))
        .filter(oauth_tokens::Column::AppId.eq(app.id))
        .filter(oauth_tokens::Column::TenantId.eq(tenant_id))
        .filter(oauth_tokens::Column::GrantType.eq(AUTHORIZATION_CODE_GRANT))
        .filter(oauth_tokens::Column::RevokedAt.is_null())
        .filter(oauth_tokens::Column::ExpiresAt.gt(Utc::now()))
        .one(db)
        .await
        .map_err(|_| OAuthTokenProtocolError::server_error("Failed to validate refresh token"))?;
    let token = match token {
        Some(token) => token,
        None => {
            // No live token holds this hash. If it is a rotated-away
            // token replayed after the grace window, that is theft
            // evidence: revoke the whole family before failing closed
            // (RFC 9700 refresh-token reuse guidance).
            detect_oauth_refresh_reuse(db, app.id, tenant_id, &token_hash).await;
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
    validate_active_subject_and_consent(db, app, tenant_id, user_id, &scopes).await?;
    Ok(token)
}

struct RefreshTokenFamily {
    family_id: Uuid,
    created_at: chrono::DateTime<Utc>,
}

/// Resolve the lineage of the presented refresh token. Pre-migration
/// rows have no family id and start their own family rooted at
/// themselves; a missing root row (already purged) falls back to the
/// current token's creation time.
async fn resolve_refresh_family(
    db: &DatabaseConnection,
    current: &oauth_tokens::Model,
) -> Result<RefreshTokenFamily, OAuthTokenProtocolError> {
    let family_id = current.family_id.unwrap_or(current.id);
    let created_at = if family_id == current.id {
        current.created_at.with_timezone(&Utc)
    } else {
        OAuthTokens::find_by_id(family_id)
            .one(db)
            .await
            .map_err(|_| {
                OAuthTokenProtocolError::server_error("Failed to resolve refresh token family")
            })?
            .map(|root| root.created_at.with_timezone(&Utc))
            .unwrap_or_else(|| current.created_at.with_timezone(&Utc))
    };

    Ok(RefreshTokenFamily {
        family_id,
        created_at,
    })
}

/// OAuth refresh-token reuse detection: a presented hash that belongs
/// to an already-rotated (revoked) token and is replayed after the
/// grace window can only have been copied, so every live token in the
/// family is revoked. Best-effort by design — the caller fails closed
/// with invalid_grant regardless of the outcome here.
async fn detect_oauth_refresh_reuse(
    db: &DatabaseConnection,
    app_id: Uuid,
    tenant_id: Uuid,
    token_hash: &str,
) {
    let stale = OAuthTokens::find()
        .filter(oauth_tokens::Column::TokenHash.eq(token_hash))
        .filter(oauth_tokens::Column::AppId.eq(app_id))
        .filter(oauth_tokens::Column::TenantId.eq(tenant_id))
        .filter(oauth_tokens::Column::GrantType.eq(AUTHORIZATION_CODE_GRANT))
        .filter(oauth_tokens::Column::RevokedAt.is_not_null())
        .one(db)
        .await;
    let Ok(Some(stale)) = stale else {
        return;
    };
    let Some(revoked_at) = stale.revoked_at else {
        return;
    };
    if Utc::now().timestamp() - revoked_at.timestamp() < OAUTH_REFRESH_REUSE_GRACE_SECONDS {
        return;
    }

    let family_id = stale.family_id.unwrap_or(stale.id);
    tracing::warn!(
        app_id = %app_id,
        user_id = ?stale.user_id,
        family_id = %family_id,
        "OAuth refresh token reuse detected; revoking token family"
    );

    let now = Utc::now();
    if let Err(error) = OAuthTokens::update_many()
        .col_expr(oauth_tokens::Column::RevokedAt, Expr::value(now))
        .col_expr(oauth_tokens::Column::UpdatedAt, Expr::value(now))
        .filter(oauth_tokens::Column::RevokedAt.is_null())
        .filter(
            Condition::any()
                .add(oauth_tokens::Column::FamilyId.eq(family_id))
                .add(oauth_tokens::Column::Id.eq(family_id)),
        )
        .exec(db)
        .await
    {
        tracing::error!(
            %error,
            family_id = %family_id,
            "failed to revoke OAuth refresh token family after reuse detection"
        );
    }
}

async fn validate_active_subject_and_consent(
    db: &DatabaseConnection,
    app: &oauth_apps::Model,
    tenant_id: Uuid,
    user_id: Uuid,
    scopes: &[String],
) -> Result<users::Model, OAuthTokenProtocolError> {
    let user = Users::find_by_id(user_id)
        .filter(users::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(|_| OAuthTokenProtocolError::server_error("Failed to validate OAuth subject"))?
        .filter(|user| user.is_active())
        .ok_or_else(|| {
            OAuthTokenProtocolError::invalid_grant("OAuth subject is missing or inactive")
        })?;

    if app.requires_user_consent() {
        let consent = OAuthConsents::find_active_consent(db, app.id, user.id, tenant_id)
            .await
            .map_err(|_| OAuthTokenProtocolError::server_error("Failed to validate OAuth consent"))?
            .ok_or_else(|| {
                OAuthTokenProtocolError::invalid_grant("OAuth consent is missing or revoked")
            })?;
        validate_scope_subset(&consent.scopes_list(), scopes)?;
    }

    Ok(user)
}

fn issue_service_access_token(
    app: &oauth_apps::Model,
    config: &AuthConfig,
    scopes: &[String],
) -> Result<(String, u64), OAuthTokenProtocolError> {
    let granted_permissions = app.parsed_granted_permissions().map_err(|_| {
        OAuthTokenProtocolError::server_error("OAuth app permission policy is invalid")
    })?;
    let role = infer_user_role_from_permissions(&granted_permissions);
    let token = auth::encode_oauth_access_token(
        config,
        app.id,
        app.tenant_id,
        role,
        app.client_id,
        scopes,
        CLIENT_CREDENTIALS_GRANT,
        SERVICE_ACCESS_TOKEN_TTL_SECS,
    )
    .map_err(|_| OAuthTokenProtocolError::server_error("Failed to encode access token"))?;
    Ok((token, SERVICE_ACCESS_TOKEN_TTL_SECS))
}

async fn prepare_user_tokens(
    db: &DatabaseConnection,
    app: &oauth_apps::Model,
    config: &AuthConfig,
    user_id: Uuid,
    scopes: &[String],
    include_refresh_token: bool,
    family: Option<&RefreshTokenFamily>,
) -> Result<PreparedUserTokens, OAuthTokenProtocolError> {
    validate_active_subject_and_consent(db, app, app.tenant_id, user_id, scopes).await?;
    let permissions = RbacService::get_user_permissions_authoritative(db, &app.tenant_id, &user_id)
        .await
        .map_err(|_| {
            OAuthTokenProtocolError::server_error("Failed to resolve current user permissions")
        })?;
    let role = infer_user_role_from_permissions(&permissions);
    let access_token = auth::encode_oauth_access_token(
        config,
        user_id,
        app.tenant_id,
        role,
        app.client_id,
        scopes,
        AUTHORIZATION_CODE_GRANT,
        USER_ACCESS_TOKEN_TTL_SECS,
    )
    .map_err(|_| OAuthTokenProtocolError::server_error("Failed to encode access token"))?;

    if !include_refresh_token {
        return Ok(PreparedUserTokens {
            access_token,
            refresh_token: None,
            refresh_model: None,
            expires_in: USER_ACCESS_TOKEN_TTL_SECS,
        });
    }

    let refresh_token = auth::generate_refresh_token()
        .map_err(|_| OAuthTokenProtocolError::server_error("Failed to generate refresh token"))?;
    let refresh_hash = auth::hash_refresh_token(&refresh_token);
    let now = Utc::now();
    let token_id = Uuid::new_v4();
    // A fresh authorization-code exchange starts a new family rooted
    // at this token; a rotation inherits the family. Either way the
    // sliding expiry is clamped to the family's absolute deadline.
    let (family_id, family_created_at) = match family {
        Some(family) => (family.family_id, family.created_at),
        None => (token_id, now),
    };
    let expires_at = (now + chrono::Duration::days(USER_REFRESH_TOKEN_TTL_DAYS)).min(
        family_created_at + chrono::Duration::days(USER_REFRESH_FAMILY_ABSOLUTE_LIFETIME_DAYS),
    );
    let refresh_model = oauth_tokens::ActiveModel {
        id: Set(token_id),
        app_id: Set(app.id),
        user_id: Set(Some(user_id)),
        tenant_id: Set(app.tenant_id),
        token_hash: Set(refresh_hash),
        family_id: Set(Some(family_id)),
        grant_type: Set(AUTHORIZATION_CODE_GRANT.to_string()),
        scopes: Set(serde_json::to_value(scopes).map_err(|_| {
            OAuthTokenProtocolError::server_error("Failed to serialize token scopes")
        })?),
        expires_at: Set(expires_at.into()),
        revoked_at: Set(None),
        last_used_at: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    };

    Ok(PreparedUserTokens {
        access_token,
        refresh_token: Some(refresh_token),
        refresh_model: Some(refresh_model),
        expires_in: USER_ACCESS_TOKEN_TTL_SECS,
    })
}

async fn commit_authorization_code_exchange(
    db: &DatabaseConnection,
    code: &oauth_codes::Model,
    prepared: &PreparedUserTokens,
) -> Result<(), OAuthTokenProtocolError> {
    let tx = db.begin().await.map_err(|_| {
        OAuthTokenProtocolError::server_error("Failed to begin authorization-code exchange")
    })?;

    consume_authorization_code(&tx, code).await?;
    persist_prepared_refresh(&tx, prepared).await?;

    tx.commit().await.map_err(|_| {
        OAuthTokenProtocolError::server_error("Failed to commit authorization-code exchange")
    })
}

async fn commit_refresh_rotation(
    db: &DatabaseConnection,
    current: &oauth_tokens::Model,
    prepared: &PreparedUserTokens,
) -> Result<(), OAuthTokenProtocolError> {
    let tx = db.begin().await.map_err(|_| {
        OAuthTokenProtocolError::server_error("Failed to begin refresh-token rotation")
    })?;
    let now = Utc::now();
    let consumed = OAuthTokens::update_many()
        .col_expr(oauth_tokens::Column::RevokedAt, Expr::value(now))
        .col_expr(oauth_tokens::Column::LastUsedAt, Expr::value(now))
        .col_expr(oauth_tokens::Column::UpdatedAt, Expr::value(now))
        .filter(oauth_tokens::Column::Id.eq(current.id))
        .filter(oauth_tokens::Column::TokenHash.eq(&current.token_hash))
        .filter(oauth_tokens::Column::AppId.eq(current.app_id))
        .filter(oauth_tokens::Column::TenantId.eq(current.tenant_id))
        .filter(oauth_tokens::Column::RevokedAt.is_null())
        .filter(oauth_tokens::Column::ExpiresAt.gt(now))
        .exec(&tx)
        .await
        .map_err(|_| OAuthTokenProtocolError::server_error("Failed to consume refresh token"))?;
    if consumed.rows_affected != 1 {
        tx.rollback().await.map_err(|_| {
            OAuthTokenProtocolError::server_error("Failed to roll back refresh-token rotation")
        })?;
        return Err(OAuthTokenProtocolError::invalid_grant(
            "Refresh token is invalid, expired, or already used",
        ));
    }

    persist_prepared_refresh(&tx, prepared).await?;
    tx.commit().await.map_err(|_| {
        OAuthTokenProtocolError::server_error("Failed to commit refresh-token rotation")
    })
}

async fn consume_authorization_code<C>(
    db: &C,
    code: &oauth_codes::Model,
) -> Result<(), OAuthTokenProtocolError>
where
    C: ConnectionTrait,
{
    let consumed = oauth_codes::Entity::update_many()
        .col_expr(oauth_codes::Column::UsedAt, Expr::value(Utc::now()))
        .filter(oauth_codes::Column::Id.eq(code.id))
        .filter(oauth_codes::Column::AppId.eq(code.app_id))
        .filter(oauth_codes::Column::TenantId.eq(code.tenant_id))
        .filter(oauth_codes::Column::UsedAt.is_null())
        .filter(oauth_codes::Column::ExpiresAt.gt(Utc::now()))
        .exec(db)
        .await
        .map_err(|_| {
            OAuthTokenProtocolError::server_error("Failed to consume authorization code")
        })?;

    if consumed.rows_affected == 1 {
        Ok(())
    } else {
        Err(OAuthTokenProtocolError::invalid_grant(
            "Authorization code is invalid, expired, or already used",
        ))
    }
}

async fn persist_prepared_refresh<C>(
    db: &C,
    prepared: &PreparedUserTokens,
) -> Result<(), OAuthTokenProtocolError>
where
    C: ConnectionTrait,
{
    if let Some(model) = prepared.refresh_model.clone() {
        model.insert(db).await.map_err(|_| {
            OAuthTokenProtocolError::server_error("Failed to persist replacement refresh token")
        })?;
    }
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
        PreparedUserTokens, REFRESH_TOKEN_GRANT, commit_refresh_rotation, require_grant,
        resolve_client, validate_requested_scopes,
    };
    use crate::models::{oauth_apps, oauth_tokens};
    use rustok_auth::TokenRequest;
    use sea_orm::{
        ActiveModelTrait, ConnectionTrait, Database, EntityTrait, PaginatorTrait, Schema, Set,
        prelude::DateTimeWithTimeZone,
    };
    use uuid::Uuid;

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

    #[tokio::test]
    async fn resolve_client_does_not_depend_on_presentation_translations() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("SQLite database");
        let backend = db.get_database_backend();
        let schema = Schema::new(backend);
        db.execute_raw(backend.build(&schema.create_table_from_entity(oauth_apps::Entity)))
            .await
            .expect("OAuth apps table");

        let tenant_id = Uuid::new_v4();
        let app = app(
            serde_json::json!(["catalog:read"]),
            serde_json::json!(["client_credentials"]),
        );
        let client_id = app.client_id;
        let mut active: oauth_apps::ActiveModel = app.into();
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
    fn client_credentials_rejects_partial_scope_truncation() {
        let app = app(
            serde_json::json!(["catalog:*"]),
            serde_json::json!(["client_credentials"]),
        );
        assert!(validate_requested_scopes(&app, &["catalog:read".to_string()]).is_ok());
        assert!(
            validate_requested_scopes(&app, &["catalog:read".to_string(), "admin:*".to_string()])
                .is_err()
        );
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
                family_id TEXT NULL
            )"#,
        )
        .await
        .expect("OAuth token table");

        let now = chrono::Utc::now();
        let current = oauth_tokens::ActiveModel {
            id: Set(Uuid::new_v4()),
            app_id: Set(Uuid::new_v4()),
            user_id: Set(Some(Uuid::new_v4())),
            tenant_id: Set(Uuid::new_v4()),
            token_hash: Set("current-hash".to_string()),
            family_id: Set(None),
            grant_type: Set("refresh_token".to_string()),
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

        let replacement = |hash: &str| PreparedUserTokens {
            access_token: "access".to_string(),
            refresh_token: Some("refresh".to_string()),
            refresh_model: Some(oauth_tokens::ActiveModel {
                id: Set(Uuid::new_v4()),
                app_id: Set(current.app_id),
                user_id: Set(current.user_id),
                tenant_id: Set(current.tenant_id),
                token_hash: Set(hash.to_string()),
                family_id: Set(None),
                grant_type: Set("refresh_token".to_string()),
                scopes: Set(serde_json::json!(["profile"])),
                expires_at: Set((now + chrono::Duration::hours(1)).into()),
                revoked_at: Set(None),
                last_used_at: Set(None),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }),
            expires_in: 900,
        };

        commit_refresh_rotation(&db, &current, &replacement("replacement-1"))
            .await
            .expect("first rotation");
        let replay = commit_refresh_rotation(&db, &current, &replacement("replacement-2"))
            .await
            .expect_err("refresh token replay must fail");

        assert_eq!(replay.error, "invalid_grant");
        assert_eq!(
            oauth_tokens::Entity::find()
                .count(&db)
                .await
                .expect("token count"),
            2
        );
    }
}
