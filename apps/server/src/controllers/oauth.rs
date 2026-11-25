//! OAuth2 REST endpoints (RFC 6749)
//!
//! OAuth authorization, token, revocation, and UserInfo endpoints.

use axum::{
    Json,
    extract::{ConnectInfo, Form, FromRequest, Query, Request, State},
    http::{
        HeaderMap, StatusCode,
        header::{
            AUTHORIZATION, CACHE_CONTROL, CONTENT_TYPE, COOKIE, LOCATION, PRAGMA, SET_COOKIE,
            WWW_AUTHENTICATE,
        },
    },
    response::{Html, IntoResponse},
    routing::{get, post},
};
use reqwest::Url;
use rustok_api::AuthPrincipalKind;
pub use rustok_auth::{
    AuthorizeRequest, BrowserAuthorizeRequest, BrowserSessionResponse, ConsentRequest,
    RevokeRequest, TokenErrorResponse, TokenRequest, TokenResponse,
};
use std::net::SocketAddr;
use uuid::Uuid;

use crate::common::RustokSettings;
use crate::context::TenantContext;
use crate::extractors::auth::{CurrentUser, resolve_current_user_from_access_token};
use crate::services::oauth_app::{AuthorizationSession, OAuthAppService};
use crate::services::oauth_token_service::OAuthTokenService;
use crate::services::server_runtime_context::{ServerAuthRuntime, ServerRuntimeContext};

const OAUTH_BROWSER_SESSION_COOKIE: &str = "rustok_oauth_browser_session";
const OAUTH_BROWSER_SESSION_TTL_SECS: u64 = 10 * 60;

fn oauth_error_response(error: TokenErrorResponse) -> axum::response::Response {
    let status = match error.error.as_str() {
        "invalid_client" => StatusCode::UNAUTHORIZED,
        "invalid_grant" | "unsupported_grant_type" => StatusCode::BAD_REQUEST,
        "invalid_scope" => StatusCode::BAD_REQUEST,
        "insufficient_scope" | "access_denied" => StatusCode::FORBIDDEN,
        "server_error" => StatusCode::INTERNAL_SERVER_ERROR,
        "temporarily_unavailable" => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::BAD_REQUEST,
    };
    let mut response = (status, Json(error)).into_response();
    response.headers_mut().insert(
        CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
        .headers_mut()
        .insert(PRAGMA, axum::http::HeaderValue::from_static("no-cache"));
    response
}

fn oauth_client_lookup_error() -> TokenErrorResponse {
    TokenErrorResponse {
        error: "server_error".to_string(),
        error_description: "OAuth client lookup is temporarily unavailable".to_string(),
    }
}

fn direct_user_required_error() -> TokenErrorResponse {
    TokenErrorResponse {
        error: "access_denied".to_string(),
        error_description: "This authorization must be completed with a direct user session; delegated and service tokens cannot authorize or consent".to_string(),
    }
}

/// Hosted browser authorization can rely on an ambient `SameSite=Lax`
/// session cookie, which is still sent on cross-site top-level GET navigation.
/// Therefore cookie-authenticated GET requests must render a confirmation
/// form and never issue a code directly. The form is protected by a short-lived
/// keyed MAC bound to the signed-in user and exact authorization parameters;
/// a forged cross-site submission (including from a same-site subdomain)
/// cannot produce a valid value.
const CONSENT_CSRF_TTL_SECS: i64 = 10 * 60;

#[allow(clippy::too_many_arguments)]
fn append_csrf_mac_field(payload: &mut Vec<u8>, value: &str) {
    payload.extend_from_slice(&(value.len() as u64).to_be_bytes());
    payload.extend_from_slice(value.as_bytes());
}

fn append_optional_csrf_mac_field(payload: &mut Vec<u8>, value: Option<&str>) {
    match value {
        Some(value) => {
            payload.push(1);
            append_csrf_mac_field(payload, value);
        }
        None => payload.push(0),
    }
}

/// RFC 2104 HMAC-SHA256. Keeping this small primitive local avoids introducing
/// a second crypto dependency for one fixed-size authentication use.
fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};

    const BLOCK_SIZE: usize = 64;
    let mut normalized_key = [0_u8; BLOCK_SIZE];
    if key.len() > BLOCK_SIZE {
        let digest = Sha256::digest(key);
        normalized_key[..digest.len()].copy_from_slice(&digest);
    } else {
        normalized_key[..key.len()].copy_from_slice(key);
    }

    let mut inner_pad = [0_u8; BLOCK_SIZE];
    let mut outer_pad = [0_u8; BLOCK_SIZE];
    for index in 0..BLOCK_SIZE {
        inner_pad[index] = normalized_key[index] ^ 0x36;
        outer_pad[index] = normalized_key[index] ^ 0x5c;
    }

    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(message);
    let inner_digest = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner_digest);
    outer.finalize().into()
}

#[allow(clippy::too_many_arguments)]
fn consent_csrf_mac(
    config: &crate::auth::AuthConfig,
    tenant_id: Uuid,
    user_id: Uuid,
    client_id: &str,
    redirect_uri: &str,
    scope: Option<&str>,
    effective_scopes: &[String],
    state: Option<&str>,
    code_challenge: &str,
    expires_at: i64,
) -> String {
    let mut payload = Vec::new();
    payload.extend_from_slice(b"rustok:oauth:consent-csrf:v3");
    append_csrf_mac_field(&mut payload, &tenant_id.to_string());
    append_csrf_mac_field(&mut payload, &user_id.to_string());
    append_csrf_mac_field(&mut payload, client_id);
    append_csrf_mac_field(&mut payload, redirect_uri);
    append_optional_csrf_mac_field(&mut payload, scope);
    // Bind the effective scope set displayed on GET, not just the optional
    // query parameter. Omitted scope is resolved from mutable client policy.
    // Ordering and duplicate entries do not change the granted authority.
    let mut scopes = effective_scopes.iter().map(String::as_str).collect::<Vec<_>>();
    scopes.sort_unstable();
    scopes.dedup();
    payload.extend_from_slice(&(scopes.len() as u64).to_be_bytes());
    for scope in scopes {
        append_csrf_mac_field(&mut payload, scope);
    }
    append_optional_csrf_mac_field(&mut payload, state);
    append_csrf_mac_field(&mut payload, code_challenge);
    append_csrf_mac_field(&mut payload, &expires_at.to_string());

    hex::encode(hmac_sha256(config.secret.as_bytes(), &payload))
}

#[allow(clippy::too_many_arguments)]
fn issue_consent_csrf(
    config: &crate::auth::AuthConfig,
    tenant_id: Uuid,
    user_id: Uuid,
    client_id: &str,
    redirect_uri: &str,
    scope: Option<&str>,
    effective_scopes: &[String],
    state: Option<&str>,
    code_challenge: &str,
) -> String {
    let expires_at = chrono::Utc::now().timestamp() + CONSENT_CSRF_TTL_SECS;
    format!(
        "{expires_at}.{}",
        consent_csrf_mac(
            config,
            tenant_id,
            user_id,
            client_id,
            redirect_uri,
            scope,
            effective_scopes,
            state,
            code_challenge,
            expires_at,
        )
    )
}

#[allow(clippy::too_many_arguments)]
fn verify_consent_csrf(
    config: &crate::auth::AuthConfig,
    tenant_id: Uuid,
    user_id: Uuid,
    client_id: &str,
    redirect_uri: &str,
    scope: Option<&str>,
    effective_scopes: &[String],
    state: Option<&str>,
    code_challenge: &str,
    presented: Option<&str>,
) -> bool {
    use subtle::ConstantTimeEq;

    let Some(presented) = presented else {
        return false;
    };
    let Some((expires_at, mac)) = presented.split_once('.') else {
        return false;
    };
    let Ok(expires_at) = expires_at.parse::<i64>() else {
        return false;
    };
    if chrono::Utc::now().timestamp() >= expires_at {
        return false;
    }
    let expected = consent_csrf_mac(
        config,
        tenant_id,
        user_id,
        client_id,
        redirect_uri,
        scope,
        effective_scopes,
        state,
        code_challenge,
        expires_at,
    );
    expected.as_bytes().ct_eq(mac.as_bytes()).into()
}

#[derive(Debug, Clone)]
struct ValidatedAuthorizeRequest {
    app: crate::models::oauth_apps::Model,
    redirect_uri: String,
    requested_scopes: Vec<String>,
    state: Option<String>,
    code_challenge: String,
}

/// Decode one application/x-www-form-urlencoded credential component after
/// the HTTP Basic payload has been Base64-decoded.
fn decode_basic_form_component(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len()
                || !bytes[index + 1].is_ascii_hexdigit()
                || !bytes[index + 2].is_ascii_hexdigit()
            {
                return None;
            }
            index += 3;
        } else {
            index += 1;
        }
    }

    let query = format!("value={value}");
    let mut pairs = url::form_urlencoded::parse(query.as_bytes());
    let (key, value) = pairs.next()?;
    if key != "value" || pairs.next().is_some() {
        return None;
    }
    Some(value.into_owned())
}

fn oauth_invalid_client_error(description: &str) -> TokenErrorResponse {
    TokenErrorResponse {
        error: "invalid_client".to_string(),
        error_description: description.to_string(),
    }
}

fn oauth_invalid_request_error(description: &str) -> TokenErrorResponse {
    TokenErrorResponse {
        error: "invalid_request".to_string(),
        error_description: description.to_string(),
    }
}

/// Read the single client authentication method from the Authorization header.
/// Body credentials remain supported as client_secret_post for compatibility.
fn parse_basic_client_credentials(
    headers: &HeaderMap,
) -> Result<Option<(String, String)>, TokenErrorResponse> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    let mut values = headers.get_all(AUTHORIZATION).iter();
    let Some(value) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(oauth_invalid_request_error(
            "Multiple Authorization headers are not allowed",
        ));
    }

    let value = value
        .to_str()
        .map_err(|_| oauth_invalid_client_error("Invalid client authentication"))?;
    let (scheme, encoded) = value
        .trim()
        .split_once(' ')
        .ok_or_else(|| oauth_invalid_client_error("Invalid client authentication"))?;
    if !scheme.eq_ignore_ascii_case("basic") {
        return Err(oauth_invalid_client_error(
            "Unsupported OAuth client authentication scheme",
        ));
    }

    let encoded = encoded.trim();
    if encoded.is_empty() || encoded.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err(oauth_invalid_client_error("Invalid client authentication"));
    }

    let decoded = STANDARD
        .decode(encoded)
        .map_err(|_| oauth_invalid_client_error("Invalid client authentication"))?;
    let decoded = String::from_utf8(decoded)
        .map_err(|_| oauth_invalid_client_error("Invalid client authentication"))?;
    let (raw_client_id, raw_client_secret) = decoded
        .split_once(':')
        .ok_or_else(|| oauth_invalid_client_error("Invalid client authentication"))?;

    let client_id = decode_basic_form_component(raw_client_id)
        .ok_or_else(|| oauth_invalid_client_error("Invalid client authentication"))?;
    let client_secret = decode_basic_form_component(raw_client_secret)
        .ok_or_else(|| oauth_invalid_client_error("Invalid client authentication"))?;
    if client_id.trim().is_empty()
        || client_secret.trim().is_empty()
        || client_id.chars().any(char::is_control)
        || client_secret.chars().any(char::is_control)
    {
        return Err(oauth_invalid_client_error("Invalid client authentication"));
    }

    Ok(Some((client_id, client_secret)))
}

fn apply_basic_client_credentials(
    headers: &HeaderMap,
    client_id: &mut Option<String>,
    client_secret: &mut Option<String>,
) -> Result<(), TokenErrorResponse> {
    let Some((basic_client_id, basic_client_secret)) =
        parse_basic_client_credentials(headers)?
    else {
        return Ok(());
    };

    if client_id.is_some() || client_secret.is_some() {
        return Err(oauth_invalid_request_error(
            "Use only one OAuth client authentication method per request",
        ));
    }

    *client_id = Some(basic_client_id);
    *client_secret = Some(basic_client_secret);
    Ok(())
}

fn add_basic_auth_challenge(
    mut response: axum::response::Response,
) -> axum::response::Response {
    if response.status() == StatusCode::UNAUTHORIZED {
        response.headers_mut().insert(
            WWW_AUTHENTICATE,
            axum::http::HeaderValue::from_static("Basic realm=\"oauth\""),
        );
    }
    response
}

fn oauth_token_http_response(
    status: StatusCode,
    body: impl serde::Serialize,
) -> axum::response::Response {
    let mut response = (status, Json(body)).into_response();
    response.headers_mut().insert(
        CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
        .headers_mut()
        .insert(PRAGMA, axum::http::HeaderValue::from_static("no-cache"));
    add_basic_auth_challenge(response)
}

#[utoipa::path(
    post,
    path = "/api/oauth/token",
    tag = "oauth",
    request_body(
        description = "OAuth token request; supports HTTP Basic client_secret_basic and body client_secret_post authentication. JSON is retained for compatibility and form encoding is RFC-compliant",
        content(
            (TokenRequest = "application/json"),
            (TokenRequest = "application/x-www-form-urlencoded")
        )
    ),
    responses(
        (status = 200, description = "OAuth token response", body = TokenResponse),
        (status = 400, description = "Invalid OAuth request", body = TokenErrorResponse),
        (status = 401, description = "Invalid client", body = TokenErrorResponse)
    )
)]
pub(crate) async fn token_handler(
    State(ctx): State<ServerAuthRuntime>,
    tenant_ctx: TenantContext,
    request: Request,
) -> axum::response::Response {
    tracing::debug!("oauth::token_handler entry");
    let req = match parse_token_request(request, &ctx).await {
        Ok(request) => request,
        Err(error) => return add_basic_auth_challenge(oauth_error_response(error)),
    };

    match OAuthTokenService::exchange(&ctx, tenant_ctx.id, &req).await {
        Ok(response) => oauth_token_http_response(StatusCode::OK, response),
        Err(error) => oauth_token_http_response(
            error.status,
            TokenErrorResponse {
                error: error.error.to_string(),
                error_description: error.description,
            },
        ),
    }
}

async fn parse_token_request(
    request: Request,
    state: &ServerAuthRuntime,
) -> Result<TokenRequest, TokenErrorResponse> {
    let headers = request.headers().clone();
    let is_form = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(is_form_encoded_content_type)
        .unwrap_or(false);

    let invalid_request = || oauth_invalid_request_error("Invalid OAuth token request");
    let mut parsed = if is_form {
        Form::<TokenRequest>::from_request(request, state)
            .await
            .map(|Form(request)| request)
            .map_err(|_| invalid_request())?
    } else {
        Json::<TokenRequest>::from_request(request, state)
            .await
            .map(|Json(request)| request)
            .map_err(|_| invalid_request())?
    };

    apply_basic_client_credentials(
        &headers,
        &mut parsed.client_id,
        &mut parsed.client_secret,
    )?;
    Ok(parsed)
}

fn is_form_encoded_content_type(value: &str) -> bool {
    value
        .split(';')
        .next()
        .map(str::trim)
        .is_some_and(|media_type| {
            media_type.eq_ignore_ascii_case("application/x-www-form-urlencoded")
        })
}
#[utoipa::path(
    post,
    path = "/api/oauth/authorize",
    tag = "oauth",
    security(("bearer_auth" = [])),
    request_body = AuthorizeRequest,
    responses(
        (status = 200, description = "Authorization code response"),
        (status = 302, description = "OAuth redirect"),
        (status = 400, description = "Invalid authorization request", body = TokenErrorResponse),
        (status = 401, description = "Authentication required", body = TokenErrorResponse),
        (status = 403, description = "Authorization denied", body = TokenErrorResponse)
    )
)]
pub(crate) async fn authorize_handler(
    State(ctx): State<ServerRuntimeContext>,
    tenant_ctx: TenantContext,
    current_user: CurrentUser,
    Json(req): Json<AuthorizeRequest>,
) -> axum::response::Response {
    tracing::debug!("oauth::authorize_handler entry");
    match authorize_handler_inner(ctx, tenant_ctx, current_user, req).await {
        Ok(response) => response.into_response(),
        Err(error) => oauth_error_response(error),
    }
}

async fn authorize_handler_inner(
    ctx: ServerRuntimeContext,
    tenant_ctx: TenantContext,
    current_user: CurrentUser,
    req: AuthorizeRequest,
) -> Result<Json<serde_json::Value>, TokenErrorResponse> {
    // Authorization is the user personally delegating authority: a
    // delegated token presented by application A must never mint codes or
    // trigger consent decisions for application B.
    if current_user.principal_kind != AuthPrincipalKind::DirectUser {
        return Err(direct_user_required_error());
    }

    let validated = validate_authorize_request(
        &ctx,
        tenant_ctx.id,
        req.client_id,
        req.redirect_uri,
        req.scope,
        req.state,
        req.response_type,
        req.code_challenge,
        req.code_challenge_method,
    )
    .await?;

    if validated.app.requires_user_consent() {
        let has_consent = OAuthAppService::get_active_consent(
            ctx.db(),
            validated.app.id,
            current_user.user.id,
            tenant_ctx.id,
            &validated.requested_scopes,
        )
        .await
        .map_err(|_| TokenErrorResponse {
            error: "server_error".to_string(),
            error_description: "Failed to verify consent".to_string(),
        })?;

        if !has_consent {
            return Err(TokenErrorResponse {
                error: "interaction_required".to_string(),
                error_description:
                    "User consent is required. Please prompt the user to grant access.".to_string(),
            });
        }
    }

    let code =
        issue_authorization_code(&ctx, tenant_ctx.id, &validated, &current_user, false).await?;

    let mut response = serde_json::json!({
        "code": code,
        "redirect_uri": validated.redirect_uri,
    });

    if let Some(state) = validated.state {
        response["state"] = serde_json::json!(state);
    }

    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/oauth/authorize",
    tag = "oauth",
    security(("bearer_auth" = [])),
    params(
        ("response_type" = String, Query, description = "OAuth response type"),
        ("client_id" = String, Query, description = "OAuth client identifier"),
        ("redirect_uri" = String, Query, description = "Registered redirect URI"),
        ("scope" = Option<String>, Query, description = "Requested scopes"),
        ("state" = Option<String>, Query, description = "Opaque client state"),
        ("code_challenge" = String, Query, description = "PKCE code challenge"),
        ("code_challenge_method" = Option<String>, Query, description = "PKCE code challenge method")
    ),
    responses(
        (status = 200, description = "Authorization or consent page"),
        (status = 302, description = "OAuth redirect"),
        (status = 400, description = "Invalid authorization request", body = TokenErrorResponse),
        (status = 401, description = "Authentication required")
    )
)]
pub(crate) async fn authorize_browser_handler(
    State(ctx): State<ServerAuthRuntime>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    tenant_ctx: TenantContext,
    headers: HeaderMap,
    Query(req): Query<BrowserAuthorizeRequest>,
) -> axum::response::Response {
    tracing::debug!("oauth::authorize_browser_handler entry");
    let runtime_ctx = ctx.runtime_ctx();
    let validated = match validate_authorize_request(
        runtime_ctx,
        tenant_ctx.id,
        req.client_id.clone(),
        req.redirect_uri.clone(),
        req.scope.clone(),
        req.state.clone(),
        req.response_type.clone(),
        req.code_challenge.clone(),
        req.code_challenge_method.clone(),
    )
    .await
    {
        Ok(validated) => validated,
        Err(error) => return oauth_error_response(error),
    };

    let access_token = extract_browser_access_token(&headers);
    let Some(access_token) = access_token else {
        return render_authorization_required(&validated.app.name).into_response();
    };

    let current_user =
        match resolve_current_user_from_access_token(&ctx, tenant_ctx.id, &access_token).await {
            Ok(current_user) => current_user,
            Err((status, message)) => {
                return (
                    status,
                    Html(render_error_page(
                        "Authorization required",
                        &format!("Sign in again to continue: {message}"),
                    )),
                )
                    .into_response();
            }
        };

    if current_user.principal_kind != AuthPrincipalKind::DirectUser {
        return (
            StatusCode::FORBIDDEN,
            Html(render_error_page(
                "Access denied",
                "This authorization must be completed with a direct user session.",
            )),
        )
            .into_response();
    }

    // An Authorization header is an explicit, non-ambient credential: a
    // cross-site navigation cannot attach it. Preserve the existing fast path
    // only for that transport when no consent is required or the requested
    // scopes are already covered. Cookie-backed requests always require the
    // CSRF-bound POST confirmation, even when consent is already active.
    let explicit_bearer = headers.contains_key(AUTHORIZATION);
    let requires_user_consent = validated.app.requires_user_consent();
    let has_consent = if explicit_bearer && requires_user_consent {
        match OAuthAppService::get_active_consent(
            runtime_ctx.db(),
            validated.app.id,
            current_user.user.id,
            tenant_ctx.id,
            &validated.requested_scopes,
        )
        .await
        {
            Ok(has_consent) => has_consent,
            Err(_) => {
                return oauth_error_response(TokenErrorResponse {
                    error: "server_error".to_string(),
                    error_description: "Failed to verify consent".to_string(),
                });
            }
        }
    } else {
        false
    };

    if may_skip_browser_authorization_confirmation(
        explicit_bearer,
        requires_user_consent,
        has_consent,
    ) {
        return match issue_authorization_code(
            runtime_ctx,
            tenant_ctx.id,
            &validated,
            &current_user,
            false,
        )
        .await
        {
            Ok(code) => redirect_with_code(
                &validated.redirect_uri,
                &code,
                validated.state.as_deref(),
                &headers,
                addr.ip(),
                runtime_ctx.settings(),
            )
            .into_response(),
            Err(error) => oauth_error_response(error),
        };
    }

    let Some(auth_config) = ctx.auth_config() else {
        return oauth_error_response(TokenErrorResponse {
            error: "server_error".to_string(),
            error_description: "Authorization is temporarily unavailable".to_string(),
        });
    };
    let consent_csrf = issue_consent_csrf(
        auth_config,
        tenant_ctx.id,
        current_user.user.id,
        &req.client_id,
        &req.redirect_uri,
        req.scope.as_deref(),
        &validated.requested_scopes,
        req.state.as_deref(),
        &req.code_challenge,
    );

    Html(render_consent_page(
        &validated,
        &req,
        &current_user.user.email,
        &consent_csrf,
    ))
    .into_response()
}

#[utoipa::path(
    post,
    path = "/api/oauth/consent",
    tag = "oauth",
    security(("bearer_auth" = [])),
    request_body(
        content = ConsentRequest,
        content_type = "application/x-www-form-urlencoded"
    ),
    responses(
        (status = 302, description = "OAuth redirect"),
        (status = 400, description = "Invalid consent request", body = TokenErrorResponse),
        (status = 401, description = "Authentication required", body = TokenErrorResponse)
    )
)]
pub(crate) async fn consent_handler(
    State(ctx): State<ServerAuthRuntime>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    tenant_ctx: TenantContext,
    headers: HeaderMap,
    Form(req): Form<ConsentRequest>,
) -> axum::response::Response {
    tracing::debug!("oauth::consent_handler entry");
    let runtime_ctx = ctx.runtime_ctx();
    let validated = match validate_authorize_request(
        runtime_ctx,
        tenant_ctx.id,
        req.client_id.clone(),
        req.redirect_uri.clone(),
        req.scope.clone(),
        req.state.clone(),
        "code".to_string(),
        req.code_challenge.clone(),
        req.code_challenge_method.clone(),
    )
    .await
    {
        Ok(validated) => validated,
        Err(error) => return oauth_error_response(error),
    };

    let access_token = extract_browser_access_token(&headers);
    let Some(access_token) = access_token else {
        return render_authorization_required(&validated.app.name).into_response();
    };

    let current_user =
        match resolve_current_user_from_access_token(&ctx, tenant_ctx.id, &access_token).await {
            Ok(current_user) => current_user,
            Err((status, message)) => {
                return (
                    status,
                    Html(render_error_page(
                        "Authorization required",
                        &format!("Sign in again to continue: {message}"),
                    )),
                )
                    .into_response();
            }
        };

    if current_user.principal_kind != AuthPrincipalKind::DirectUser {
        return oauth_error_response(direct_user_required_error());
    }

    let csrf_valid = ctx.auth_config().is_some_and(|auth_config| {
        verify_consent_csrf(
            auth_config,
            tenant_ctx.id,
            current_user.user.id,
            &req.client_id,
            &req.redirect_uri,
            req.scope.as_deref(),
            &validated.requested_scopes,
            req.state.as_deref(),
            &req.code_challenge,
            req.consent_csrf.as_deref(),
        )
    });
    if !csrf_valid {
        return oauth_error_response(TokenErrorResponse {
            error: "invalid_request".to_string(),
            error_description: "Invalid or expired authorization confirmation; restart the authorization flow"
                .to_string(),
        });
    }

    if req.action == "deny" {
        return redirect_with_error(
            &validated.redirect_uri,
            "access_denied",
            "The user denied the authorization request",
            validated.state.as_deref(),
            &headers,
            addr.ip(),
            runtime_ctx.settings(),
        )
        .into_response();
    }

    if req.action != "approve" {
        return oauth_error_response(TokenErrorResponse {
            error: "invalid_request".to_string(),
            error_description: "Unknown consent action".to_string(),
        });
    }

    // Approval and issuance share one transaction, with a fresh direct-session
    // check under the mutation locks. Never persist consent before issuance.
    match issue_authorization_code(runtime_ctx, tenant_ctx.id, &validated, &current_user, true)
        .await
    {
        Ok(code) => redirect_with_code(
            &validated.redirect_uri,
            &code,
            validated.state.as_deref(),
            &headers,
            addr.ip(),
            runtime_ctx.settings(),
        )
        .into_response(),
        Err(error) => oauth_error_response(error),
    }
}

#[utoipa::path(
    post,
    path = "/api/oauth/browser-session",
    tag = "oauth",
    security(("bearer_auth" = [])),
    responses(
        (status = 204, description = "Browser OAuth session cookie created"),
        (status = 401, description = "Authentication required", body = TokenErrorResponse)
    )
)]
pub(crate) async fn create_browser_session_handler(
    State(ctx): State<ServerRuntimeContext>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    current_user: CurrentUser,
) -> axum::response::Response {
    if current_user.principal_kind != AuthPrincipalKind::DirectUser {
        return oauth_error_response(direct_user_required_error());
    }

    let Some(access_token) = extract_bearer_token(&headers) else {
        return oauth_error_response(TokenErrorResponse {
            error: "invalid_request".to_string(),
            error_description: "Missing bearer token for OAuth browser session".to_string(),
        });
    };

    (
        StatusCode::NO_CONTENT,
        [(
            SET_COOKIE,
            build_oauth_browser_session_cookie(&access_token, &headers, addr.ip(), ctx.settings()),
        )],
    )
        .into_response()
}

#[utoipa::path(
    delete,
    path = "/api/oauth/browser-session",
    tag = "oauth",
    responses(
        (status = 204, description = "Browser OAuth session cookie cleared")
    )
)]
pub(crate) async fn clear_browser_session_handler(
    State(ctx): State<ServerRuntimeContext>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> axum::response::Response {
    (
        StatusCode::NO_CONTENT,
        [(
            SET_COOKIE,
            clear_oauth_browser_session_cookie(&headers, addr.ip(), ctx.settings()),
        )],
    )
        .into_response()
}

fn may_skip_browser_authorization_confirmation(
    explicit_bearer: bool,
    requires_user_consent: bool,
    has_consent: bool,
) -> bool {
    explicit_bearer && (!requires_user_consent || has_consent)
}

fn is_valid_s256_challenge(challenge: &str) -> bool {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

    if challenge.len() != 43
        || !challenge
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
    {
        return false;
    }

    URL_SAFE_NO_PAD
        .decode(challenge)
        .is_ok_and(|digest| digest.len() == 32 && URL_SAFE_NO_PAD.encode(digest) == challenge)
}

#[allow(clippy::too_many_arguments)]
async fn validate_authorize_request(
    ctx: &ServerRuntimeContext,
    tenant_id: Uuid,
    client_id: String,
    redirect_uri: String,
    scope: Option<String>,
    state: Option<String>,
    response_type: String,
    code_challenge: String,
    code_challenge_method: Option<String>,
) -> Result<ValidatedAuthorizeRequest, TokenErrorResponse> {
    if response_type != "code" {
        return Err(TokenErrorResponse {
            error: "unsupported_response_type".to_string(),
            error_description: "Only response_type=code is supported".to_string(),
        });
    }

    if code_challenge_method.as_deref() != Some("S256") {
        return Err(TokenErrorResponse {
            error: "invalid_request".to_string(),
            error_description: "code_challenge_method must be S256".to_string(),
        });
    }
    if !is_valid_s256_challenge(&code_challenge) {
        return Err(TokenErrorResponse {
            error: "invalid_request".to_string(),
            error_description: "code_challenge must be a 43-character base64url S256 value"
                .to_string(),
        });
    }

    let client_id = Uuid::parse_str(&client_id).map_err(|_| TokenErrorResponse {
        error: "invalid_client".to_string(),
        error_description: "Invalid client_id format".to_string(),
    })?;

    let app = OAuthAppService::find_security_by_client_id_scoped(ctx.db(), tenant_id, client_id)
        .await
        .map_err(|_| oauth_client_lookup_error())?
        .ok_or_else(|| TokenErrorResponse {
            error: "invalid_client".to_string(),
            error_description: "Unknown client_id".to_string(),
        })?;

    if !app.supports_grant_type("authorization_code") {
        return Err(TokenErrorResponse {
            error: "invalid_grant".to_string(),
            error_description: "This app does not support authorization_code grant".to_string(),
        });
    }

    if Url::parse(&redirect_uri).is_err() {
        return Err(TokenErrorResponse {
            error: "invalid_request".to_string(),
            error_description: "redirect_uri is not a valid absolute URI".to_string(),
        });
    }

    if !app.redirect_uris_list().contains(&redirect_uri) {
        return Err(TokenErrorResponse {
            error: "invalid_request".to_string(),
            error_description: "redirect_uri is not configured for this client".to_string(),
        });
    }

    let allowed_scopes = app.scopes_list();
    let requested_scopes = validate_authorization_scopes(scope.as_deref(), &allowed_scopes)?;

    Ok(ValidatedAuthorizeRequest {
        app,
        redirect_uri,
        requested_scopes,
        state,
        code_challenge,
    })
}

fn validate_authorization_scopes(
    requested: Option<&str>,
    allowed_scopes: &[String],
) -> Result<Vec<String>, TokenErrorResponse> {
    let requested_scopes = match requested {
        None => allowed_scopes.to_vec(),
        Some(value) => value.split_whitespace().map(str::to_string).collect(),
    };

    if requested_scopes.is_empty() {
        let error_description = if requested.is_some() {
            "scope must not be empty when provided"
        } else {
            "client has no registered scopes"
        };
        return Err(TokenErrorResponse {
            error: "invalid_scope".to_string(),
            error_description: error_description.to_string(),
        });
    }

    for requested_scope in &requested_scopes {
        if !crate::context::scope_matches(allowed_scopes, requested_scope) {
            return Err(TokenErrorResponse {
                error: "invalid_scope".to_string(),
                error_description: format!(
                    "Scope '{requested_scope}' is not allowed for this client"
                ),
            });
        }
    }

    Ok(requested_scopes)
}

/// A stale OAuth authorization preflight is a protocol rejection, not an
/// internal server outage. Keep DB/persistence failures distinguishable and
/// never return raw database details to the client.
fn map_code_issuance_error(error: crate::error::Error) -> TokenErrorResponse {
    let (code, description) = match error {
        crate::error::Error::NotFound => (
            "invalid_client",
            "OAuth client is unavailable; restart the authorization flow",
        ),
        crate::error::Error::BadRequest(_) => (
            "invalid_request",
            "OAuth client policy changed; restart the authorization flow",
        ),
        crate::error::Error::Unauthorized(_) => (
            "access_denied",
            "OAuth subject or consent is no longer authorized",
        ),
        _ => {
            tracing::error!(
                error_code = "oauth_authorization_code_issuance_failed",
                "Failed to issue OAuth authorization code"
            );
            ("server_error", "Failed to generate authorization code")
        }
    };
    TokenErrorResponse {
        error: code.to_string(),
        error_description: description.to_string(),
    }
}

async fn issue_authorization_code(
    ctx: &ServerRuntimeContext,
    tenant_id: Uuid,
    validated: &ValidatedAuthorizeRequest,
    current_user: &CurrentUser,
    approve_consent: bool,
) -> Result<String, TokenErrorResponse> {
    OAuthAppService::generate_authorization_code(
        ctx.db(),
        validated.app.id,
        AuthorizationSession {
            user_id: current_user.user.id,
            session_id: current_user.session_id,
            access_expires_at: current_user.access_token_expires_at,
            require_email_verification: ctx.settings().features.email_verification,
        },
        tenant_id,
        validated.redirect_uri.clone(),
        validated.requested_scopes.clone(),
        validated.code_challenge.clone(),
        approve_consent,
    )
    .await
    .map_err(map_code_issuance_error)
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<String> {
    // All auth entrypoints must reject ambiguous Authorization fields.
    // Taking the first one can disagree with upstream authentication layers.
    let mut values = headers.get_all(AUTHORIZATION).iter();
    let value = values.next()?.to_str().ok()?;
    if values.next().is_some() {
        return None;
    }
    let mut parts = value.split_ascii_whitespace();

    match (parts.next(), parts.next(), parts.next()) {
        (Some(scheme), Some(token), None)
            if scheme.eq_ignore_ascii_case("Bearer") && !token.is_empty() =>
        {
            Some(token.to_string())
        }
        _ => None,
    }
}

fn extract_browser_access_token(headers: &HeaderMap) -> Option<String> {
    // An explicitly supplied (but malformed/duplicated) bearer credential
    // must not silently fall back to a different cookie principal.
    if headers.contains_key(AUTHORIZATION) {
        return extract_bearer_token(headers);
    }
    parse_oauth_browser_cookie(headers)
}

fn parse_oauth_browser_cookie(headers: &HeaderMap) -> Option<String> {
    let mut token = None;
    // HTTP/2 may split Cookie across multiple fields. Inspect every field and
    // reject repeated session-cookie names rather than choosing by order.
    for header in headers.get_all(COOKIE).iter() {
        for part in header.to_str().ok()?.split(';') {
            let Some((name, value)) = part.trim().split_once('=') else {
                continue;
            };
            if name == OAUTH_BROWSER_SESSION_COOKIE {
                let value = value.trim();
                if value.is_empty() || token.is_some() {
                    return None;
                }
                token = Some(value.to_string());
            }
        }
    }
    token
}

fn build_oauth_browser_session_cookie(
    access_token: &str,
    headers: &HeaderMap,
    peer_ip: std::net::IpAddr,
    settings: &RustokSettings,
) -> String {
    build_oauth_browser_session_cookie_for_secure(
        access_token,
        should_use_secure_cookie(headers, Some(peer_ip), settings),
        Some(OAUTH_BROWSER_SESSION_TTL_SECS),
    )
}

fn clear_oauth_browser_session_cookie(
    headers: &HeaderMap,
    peer_ip: std::net::IpAddr,
    settings: &RustokSettings,
) -> String {
    build_oauth_browser_session_cookie_for_secure(
        "",
        should_use_secure_cookie(headers, Some(peer_ip), settings),
        Some(0),
    )
}

fn clear_oauth_browser_session_cookie_for_request(
    headers: &HeaderMap,
    peer_ip: std::net::IpAddr,
    settings: &RustokSettings,
) -> String {
    clear_oauth_browser_session_cookie(headers, peer_ip, settings)
}

fn build_oauth_browser_session_cookie_for_secure(
    access_token: &str,
    secure: bool,
    max_age: Option<u64>,
) -> String {
    let mut cookie = format!(
        "{OAUTH_BROWSER_SESSION_COOKIE}={access_token}; Path=/api/oauth; HttpOnly; SameSite=Lax"
    );
    if let Some(max_age) = max_age {
        cookie.push_str(&format!("; Max-Age={max_age}"));
    }
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

fn should_use_secure_cookie(
    headers: &HeaderMap,
    peer_ip: Option<std::net::IpAddr>,
    settings: &RustokSettings,
) -> bool {
    crate::common::secure_cookie_required(
        headers,
        peer_ip,
        &settings.runtime.request_trust,
    )
}

fn redirect_with_code(
    redirect_uri: &str,
    code: &str,
    state: Option<&str>,
    headers: &HeaderMap,
    peer_ip: std::net::IpAddr,
    settings: &RustokSettings,
) -> (StatusCode, [(axum::http::header::HeaderName, String); 2]) {
    let mut url = match Url::parse(redirect_uri) {
        Ok(url) => url,
        Err(_err) => {
            tracing::error!(
                error_code = "oauth.redirect_with_code.invalid_uri",
                redirect_uri_length = redirect_uri.len(),
                "Failed to parse redirect URI in redirect_with_code"
            );
            return (
                StatusCode::BAD_REQUEST,
                [
                    (LOCATION, String::new()),
                    (
                        SET_COOKIE,
                        clear_oauth_browser_session_cookie_for_request(headers, peer_ip, settings),
                    ),
                ],
            );
        }
    };
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("code", code);
        if let Some(state) = state {
            query.append_pair("state", state);
        }
    }
    (
        StatusCode::FOUND,
        [
            (LOCATION, url.to_string()),
            (
                SET_COOKIE,
                clear_oauth_browser_session_cookie_for_request(headers, peer_ip, settings),
            ),
        ],
    )
}

fn redirect_with_error(
    redirect_uri: &str,
    error: &str,
    description: &str,
    state: Option<&str>,
    headers: &HeaderMap,
    peer_ip: std::net::IpAddr,
    settings: &RustokSettings,
) -> (StatusCode, [(axum::http::header::HeaderName, String); 2]) {
    let mut url = match Url::parse(redirect_uri) {
        Ok(url) => url,
        Err(_err) => {
            tracing::error!(
                error_code = "oauth.redirect_with_error.invalid_uri",
                redirect_uri_length = redirect_uri.len(),
                "Failed to parse redirect URI in redirect_with_error"
            );
            return (
                StatusCode::BAD_REQUEST,
                [
                    (LOCATION, String::new()),
                    (
                        SET_COOKIE,
                        clear_oauth_browser_session_cookie_for_request(headers, peer_ip, settings),
                    ),
                ],
            );
        }
    };
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("error", error);
        query.append_pair("error_description", description);
        if let Some(state) = state {
            query.append_pair("state", state);
        }
    }
    (
        StatusCode::FOUND,
        [
            (LOCATION, url.to_string()),
            (
                SET_COOKIE,
                clear_oauth_browser_session_cookie_for_request(headers, peer_ip, settings),
            ),
        ],
    )
}

fn render_authorization_required(app_name: &str) -> (StatusCode, Html<String>) {
    (
        StatusCode::UNAUTHORIZED,
        Html(render_error_page(
            "Authorization required",
            &format!(
                "Create an OAuth browser session first, then retry authorization for '{}'. First-party frontends should POST /api/oauth/browser-session with the current bearer token before opening the browser authorization URL.",
                app_name
            ),
        )),
    )
}

fn render_error_page(title: &str, message: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{}</title><style>body{{font-family:system-ui,sans-serif;max-width:720px;margin:48px auto;padding:0 16px;color:#111827}}.card{{border:1px solid #d1d5db;border-radius:12px;padding:24px;background:#fff}}p{{line-height:1.5;color:#374151}}</style></head><body><div class=\"card\"><h1>{}</h1><p>{}</p></div></body></html>",
        escape_html(title),
        escape_html(title),
        escape_html(message)
    )
}

fn render_consent_page(
    validated: &ValidatedAuthorizeRequest,
    request: &BrowserAuthorizeRequest,
    user_email: &str,
    consent_csrf: &str,
) -> String {
    let scopes = validated
        .requested_scopes
        .iter()
        .map(|scope| format!("<li>{}</li>", escape_html(scope)))
        .collect::<Vec<_>>()
        .join("");

    // An absent OAuth parameter must stay absent on POST. An empty
    // hidden input changes None to Some(""), invalidating the CSRF binding.
    let scope_input = request
        .scope
        .as_deref()
        .map(|scope| {
            format!(
                "<input type=\"hidden\" name=\"scope\" value=\"{}\">",
                escape_attr(scope)
            )
        })
        .unwrap_or_default();
    let state_input = request
        .state
        .as_deref()
        .map(|state| {
            format!(
                "<input type=\"hidden\" name=\"state\" value=\"{}\">",
                escape_attr(state)
            )
        })
        .unwrap_or_default();

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Authorize {app}</title><style>body{{font-family:system-ui,sans-serif;background:#f8fafc;color:#0f172a;max-width:760px;margin:40px auto;padding:0 16px}}.card{{background:#fff;border:1px solid #dbe3f0;border-radius:16px;padding:24px;box-shadow:0 12px 30px rgba(15,23,42,0.08)}}.meta{{color:#475569}}ul{{padding-left:20px}}form{{display:flex;gap:12px;margin-top:24px}}button{{border:0;border-radius:10px;padding:12px 18px;font-weight:600;cursor:pointer}}.approve{{background:#0f766e;color:white}}.deny{{background:#e2e8f0;color:#0f172a}}</style></head><body><div class=\"card\"><h1>Authorize {app}</h1><p class=\"meta\">Signed in as {user}. This app is requesting access to:</p><ul>{scopes}</ul><p class=\"meta\">Redirect URI: {redirect}</p><form method=\"post\" action=\"/api/oauth/consent\"><input type=\"hidden\" name=\"client_id\" value=\"{client_id}\"><input type=\"hidden\" name=\"redirect_uri\" value=\"{redirect_attr}\">{scope_input}{state_input}<input type=\"hidden\" name=\"code_challenge\" value=\"{challenge_attr}\"><input type=\"hidden\" name=\"code_challenge_method\" value=\"S256\"><input type=\"hidden\" name=\"consent_csrf\" value=\"{csrf_attr}\"><button class=\"approve\" type=\"submit\" name=\"action\" value=\"approve\">Approve</button><button class=\"deny\" type=\"submit\" name=\"action\" value=\"deny\">Deny</button></form></div></body></html>",
        app = escape_html(&validated.app.name),
        user = escape_html(user_email),
        scopes = scopes,
        redirect = escape_html(&validated.redirect_uri),
        client_id = escape_attr(&request.client_id),
        redirect_attr = escape_attr(&request.redirect_uri),
        scope_input = scope_input,
        state_input = state_input,
        challenge_attr = escape_attr(&request.code_challenge),
        csrf_attr = escape_attr(consent_csrf),
    )
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn escape_attr(value: &str) -> String {
    escape_html(value)
}

/// Public PKCE clients do not authenticate revocation with a secret. Service
/// clients always do; other confidential clients require a configured secret.
fn oauth_revocation_requires_client_secret(app: &crate::models::oauth_apps::Model) -> bool {
    if app.app_type == "service" {
        return true;
    }
    if matches!(app.app_type.as_str(), "first_party" | "mobile") {
        return false;
    }
    app.client_secret_hash.is_some()
}

/// Token Revocation Endpoint (RFC 7009).
/// Revokes registered access and refresh credentials for the authenticated client.
#[utoipa::path(
    post,
    path = "/api/oauth/revoke",
    tag = "oauth",
    request_body(
        description = "OAuth revocation request; supports HTTP Basic client_secret_basic and body client_secret_post authentication. RFC 7009 form encoding is required, JSON is retained for compatibility",
        content(
            (RevokeRequest = "application/x-www-form-urlencoded"),
            (RevokeRequest = "application/json")
        )
    ),
    responses(
        (status = 200, description = "Token revoked"),
        (status = 400, description = "Invalid revocation request", body = TokenErrorResponse),
        (status = 401, description = "Invalid client", body = TokenErrorResponse)
    )
)]
pub(crate) async fn revoke_handler(
    State(ctx): State<ServerRuntimeContext>,
    tenant_ctx: TenantContext,
    request: Request,
) -> axum::response::Response {
    tracing::debug!("oauth::revoke_handler entry");
    let req = match parse_revoke_request(request, &ctx).await {
        Ok(request) => request,
        Err(error) => return add_basic_auth_challenge(oauth_error_response(error)),
    };

    match revoke_handler_inner(ctx, tenant_ctx, req).await {
        Ok(status) => status.into_response(),
        Err(error) => add_basic_auth_challenge(oauth_error_response(error)),
    }
}

async fn parse_revoke_request(
    request: Request,
    state: &ServerRuntimeContext,
) -> Result<RevokeRequest, TokenErrorResponse> {
    let headers = request.headers().clone();
    let is_form = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(is_form_encoded_content_type)
        .unwrap_or(false);

    let invalid_request = || oauth_invalid_request_error("Invalid OAuth revocation request");
    let mut parsed = if is_form {
        Form::<RevokeRequest>::from_request(request, state)
            .await
            .map(|Form(request)| request)
            .map_err(|_| invalid_request())?
    } else {
        Json::<RevokeRequest>::from_request(request, state)
            .await
            .map(|Json(request)| request)
            .map_err(|_| invalid_request())?
    };

    apply_basic_client_credentials(
        &headers,
        &mut parsed.client_id,
        &mut parsed.client_secret,
    )?;
    Ok(parsed)
}

async fn revoke_handler_inner(
    ctx: ServerRuntimeContext,
    tenant_ctx: TenantContext,
    req: RevokeRequest,
) -> Result<axum::http::StatusCode, TokenErrorResponse> {
    // 1. Authenticate the client
    let client_id_str = req.client_id.as_deref().ok_or_else(|| TokenErrorResponse {
        error: "invalid_client".to_string(),
        error_description: "client_id is required".to_string(),
    })?;
    let client_id = Uuid::parse_str(client_id_str).map_err(|_| TokenErrorResponse {
        error: "invalid_client".to_string(),
        error_description: "Invalid client_id format".to_string(),
    })?;

    let app =
        OAuthAppService::find_security_by_client_id_scoped(ctx.db(), tenant_ctx.id, client_id)
            .await
            .map_err(|_| oauth_client_lookup_error())?
            .ok_or_else(|| TokenErrorResponse {
                error: "invalid_client".to_string(),
                error_description: "Unknown client_id".to_string(),
            })?;

    // First-party/mobile PKCE clients use public-client semantics for revocation.
    // Service clients always require one; other confidential clients require it
    // whenever a secret hash is configured.
    if oauth_revocation_requires_client_secret(&app) {
        let secret_hash = app.client_secret_hash.as_deref().ok_or_else(|| {
            oauth_invalid_client_error("Confidential client credentials are required")
        })?;
        let client_secret = req
            .client_secret
            .as_deref()
            .filter(|secret| !secret.is_empty())
            .ok_or_else(|| oauth_invalid_client_error("client_secret is required"))?;
        let valid = OAuthAppService::verify_client_secret_async(
            client_secret.to_string(),
            secret_hash.to_string(),
        )
        .await
        .map_err(|_| oauth_invalid_client_error("Invalid client credentials"))?;
        if !valid {
            return Err(oauth_invalid_client_error("Invalid client credentials"));
        }
    }

    if req.token.trim().is_empty() || req.token.chars().any(char::is_whitespace) {
        return Err(oauth_invalid_request_error("A non-empty token is required"));
    }

    // 2. Hash the token and try to revoke it. The canonical helper is
    // shared with refresh/access registration so RFC 7009 matches the
    // stored hash for either token kind.
    let token_hash = crate::auth::hash_refresh_token(&req.token);

    OAuthAppService::revoke_authenticated_token_by_hash(ctx.db(), &token_hash, &app, tenant_ctx.id)
        .await
        .map_err(|error| match error {
            crate::error::Error::Unauthorized(_) =>
                oauth_invalid_client_error("Invalid client credentials"),
            _ => TokenErrorResponse {
                error: "server_error".to_string(),
                error_description: "Failed to revoke token".to_string(),
            },
        })?;

    // RFC 7009: always return 200 OK regardless of whether token existed
    Ok(axum::http::StatusCode::OK)
}

/// OpenID Connect Core 1.0 UserInfo Endpoint (§5.3).
/// Requires `openid`; standard profile and email claims are gated by their scopes.
#[utoipa::path(
    get,
    path = "/api/oauth/userinfo",
    tag = "oauth",
    operation_id = "oauth_userinfo_get",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "OpenID Connect UserInfo response"),
        (status = 403, description = "Insufficient scope", body = TokenErrorResponse),
        (status = 401, description = "Authentication required", body = TokenErrorResponse)
    )
)]
pub(crate) async fn userinfo_handler(
    current_user: CurrentUser, // Automatically extracts and validates Bearer token
) -> axum::response::Response {
    tracing::debug!("oauth::userinfo_handler entry");
    match userinfo_handler_inner(current_user).await {
        Ok(response) => response.into_response(),
        Err(error) => oauth_error_response(error),
    }
}

/// OpenID Connect Core 1.0 UserInfo Endpoint (§5.3), also available via POST.
/// Requires `openid`; standard profile and email claims are gated by their scopes.
#[utoipa::path(
    post,
    path = "/api/oauth/userinfo",
    tag = "oauth",
    operation_id = "oauth_userinfo_post",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "OpenID Connect UserInfo response"),
        (status = 403, description = "Insufficient scope", body = TokenErrorResponse),
        (status = 401, description = "Authentication required", body = TokenErrorResponse)
    )
)]
pub(crate) async fn userinfo_post_handler(current_user: CurrentUser) -> axum::response::Response {
    userinfo_handler(current_user).await
}

async fn userinfo_handler_inner(
    current_user: CurrentUser,
) -> Result<Json<serde_json::Value>, TokenErrorResponse> {
    if matches!(
        current_user.principal_kind,
        rustok_api::AuthPrincipalKind::Service
    ) {
        return Err(TokenErrorResponse {
            error: "insufficient_scope".to_string(),
            error_description: "UserInfo requires an end-user access token".to_string(),
        });
    }

    if !oauth_scope_contains(&current_user.scopes, "openid") {
        return Err(TokenErrorResponse {
            error: "insufficient_scope".to_string(),
            error_description: "openid scope is required for UserInfo".to_string(),
        });
    }

    let user = current_user.user;
    let inferred_role = current_user.inferred_role;
    let mut userinfo = serde_json::json!({
        "sub": user.id.to_string(),
        "role": inferred_role.to_string(),
        "tenant_id": user.tenant_id.to_string(),
    });

    if oauth_scope_contains(&current_user.scopes, "profile") {
        userinfo["name"] = serde_json::Value::String(user.name.unwrap_or_default());
    }

    if oauth_scope_contains(&current_user.scopes, "email") {
        userinfo["email"] = serde_json::Value::String(user.email);
        userinfo["email_verified"] = serde_json::Value::Bool(user.email_verified_at.is_some());
    }

    Ok(Json(userinfo))
}

fn oauth_scope_contains(scopes: &[String], required_scope: &str) -> bool {
    scopes.iter().any(|scope| scope == required_scope)
}

/// Authorization codes, identity claims and consent forms must not be cached
/// by browsers or shared intermediaries, including on error and redirect paths.
async fn disable_oauth_caching(
    request: Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response
        .headers_mut()
        .insert(PRAGMA, axum::http::HeaderValue::from_static("no-cache"));
    response
}

pub fn router() -> crate::routes::ServerRouter {
    axum::Router::new()
        .route(
            "/api/oauth/authorize",
            get(authorize_browser_handler).post(authorize_handler),
        )
        .route(
            "/api/oauth/browser-session",
            post(create_browser_session_handler).delete(clear_browser_session_handler),
        )
        .route("/api/oauth/consent", post(consent_handler))
        .route("/api/oauth/token", post(token_handler))
        .route(
            "/api/oauth/userinfo",
            get(userinfo_handler).post(userinfo_post_handler),
        )
        .route("/api/oauth/revoke", post(revoke_handler))
        .layer(axum::middleware::from_fn(disable_oauth_caching))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consent_csrf_hmac_matches_rfc_4231_sha256_vector() {
        let key = [0x0b_u8; 20];
        let actual = super::hmac_sha256(&key, b"Hi There");
        assert_eq!(
            hex::encode(actual),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }

    #[test]
    fn consent_csrf_mac_fields_are_unambiguous() {
        let key = b"test-consent-key";
        let mut first = Vec::new();
        super::append_optional_csrf_mac_field(&mut first, Some("scope\0state"));
        super::append_optional_csrf_mac_field(&mut first, None);

        let mut second = Vec::new();
        super::append_optional_csrf_mac_field(&mut second, Some("scope"));
        super::append_optional_csrf_mac_field(&mut second, Some("state"));

        assert_ne!(
            super::hmac_sha256(key, &first),
            super::hmac_sha256(key, &second)
        );
    }

    #[test]
    fn consent_confirmation_binds_the_effective_scopes_even_when_scope_is_omitted() {
        let config = crate::auth::AuthConfig::new("test-oauth-consent-secret-with-32-bytes".to_string());
        let tenant = Uuid::new_v4();
        let user = Uuid::new_v4();
        let client = Uuid::new_v4().to_string();
        let redirect = "https://example.com/callback";
        let challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        let allowed = vec!["profile".to_string(), "catalog:read".to_string()];
        let displayed = validate_authorization_scopes(None, &allowed).unwrap();
        let token = issue_consent_csrf(
            &config, tenant, user, &client, redirect, None, &displayed, None, challenge,
        );
        let verify = |effective: &[String]| {
            verify_consent_csrf(
                &config, tenant, user, &client, redirect, None, effective, None,
                challenge, Some(token.as_str()),
            )
        };
        assert!(verify(&displayed));

        let expanded = vec!["profile".to_string(), "catalog:read".to_string(), "catalog:write".to_string()];
        let expanded = validate_authorization_scopes(None, &expanded).unwrap();
        assert!(!verify(&expanded), "new scopes require a new confirmation");

        let narrowed = validate_authorization_scopes(None, &allowed[..1]).unwrap();
        assert!(!verify(&narrowed), "a changed scope set requires a new confirmation");

        let reordered = vec!["catalog:read".to_string(), "profile".to_string(), "profile".to_string()];
        assert!(verify(&reordered), "equivalent scope sets have the same binding");

        // An explicit unchanged request still works when unrelated client
        // permissions change, but is not interchangeable with an omitted scope.
        let explicit = validate_authorization_scopes(Some("profile"), &allowed).unwrap();
        let explicit_token = issue_consent_csrf(
            &config, tenant, user, &client, redirect, Some("profile"), &explicit, None, challenge,
        );
        let still_explicit = validate_authorization_scopes(Some("profile"), &expanded).unwrap();
        assert!(verify_consent_csrf(
            &config, tenant, user, &client, redirect, Some("profile"), &still_explicit, None,
            challenge, Some(explicit_token.as_str()),
        ));
        assert!(!verify_consent_csrf(
            &config, tenant, user, &client, redirect, None, &still_explicit, None,
            challenge, Some(explicit_token.as_str()),
        ));
    }

    #[test]
    fn authorization_scope_defaults_only_when_scope_is_omitted() {
        let allowed = vec!["profile".to_string(), "catalog:read".to_string()];

        assert_eq!(
            validate_authorization_scopes(None, &allowed).unwrap(),
            allowed
        );
        assert_eq!(
            validate_authorization_scopes(Some("profile"), &allowed).unwrap(),
            vec!["profile".to_string()]
        );
        for empty in ["", "   \t"] {
            assert_eq!(
                validate_authorization_scopes(Some(empty), &allowed)
                    .unwrap_err()
                    .error,
                "invalid_scope"
            );
        }
        assert_eq!(
            validate_authorization_scopes(Some("profile admin:*"), &allowed)
                .unwrap_err()
                .error,
            "invalid_scope"
        );
    }

    #[test]
    fn s256_challenge_requires_exact_unpadded_base64url_shape() {
        assert!(is_valid_s256_challenge(
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        ));
        assert!(!is_valid_s256_challenge("short"));
        assert!(!is_valid_s256_challenge(
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw+cM"
        ));
        assert!(!is_valid_s256_challenge(
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM="
        ));
    }
    use crate::common::settings::{ForwardedHeadersMode, RequestTrustSettings};
    use axum::http::{HeaderName, header::HeaderValue};
    use chrono::Utc;
    use std::net::{IpAddr, Ipv4Addr};

    fn sample_app(app_type: &str) -> crate::models::oauth_apps::Model {
        crate::models::oauth_apps::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            name: "Example App".to_string(),
            slug: "example-app".to_string(),
            description: Some("Example".to_string()),
            app_type: app_type.to_string(),
            icon_url: None,
            client_id: Uuid::new_v4(),
            client_secret_hash: Some("hash".to_string()),
            redirect_uris: serde_json::json!(["https://client.example.com/callback"]),
            scopes: serde_json::json!(["profile", "catalog:read"]),
            grant_types: serde_json::json!(["authorization_code", "refresh_token"]),
            granted_permissions: serde_json::json!([]),
            manifest_ref: None,
            auto_created: false,
            is_active: true,
            revoked_at: None,
            last_used_at: None,
            metadata: serde_json::json!({}),
            created_at: Utc::now().into(),
            updated_at: Utc::now().into(),
        }
    }

    fn validated_request(app_type: &str) -> ValidatedAuthorizeRequest {
        ValidatedAuthorizeRequest {
            app: sample_app(app_type),
            redirect_uri: "https://client.example.com/callback".to_string(),
            requested_scopes: vec!["profile".to_string(), "catalog:read".to_string()],
            state: Some("opaque-state".to_string()),
            code_challenge: "challenge-value".to_string(),
        }
    }

    fn trusted_settings() -> RustokSettings {
        let mut settings = RustokSettings::default();
        settings.runtime.request_trust = RequestTrustSettings {
            forwarded_headers_mode: ForwardedHeadersMode::TrustedOnly,
            trusted_proxy_cidrs: vec!["10.0.0.0/8".to_string()],
        };
        settings
    }

    #[test]
    fn oauth_token_http_response_disables_caching() {
        let response = oauth_token_http_response(
            StatusCode::OK,
            TokenErrorResponse {
                error: "invalid_request".to_string(),
                error_description: "test".to_string(),
            },
        );

        assert_eq!(
            response
                .headers()
                .get(CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-store"),
        );
        assert_eq!(
            response
                .headers()
                .get(PRAGMA)
                .and_then(|value| value.to_str().ok()),
            Some("no-cache"),
        );
    }
    #[test]
    fn token_form_content_type_accepts_optional_parameters() {
        assert!(is_form_encoded_content_type(
            "application/x-www-form-urlencoded"
        ));
        assert!(is_form_encoded_content_type(
            "Application/X-WWW-Form-Urlencoded; charset=UTF-8",
        ));
        assert!(!is_form_encoded_content_type("application/json"));
    }
    #[test]
    fn bearer_scheme_is_case_insensitive_and_requires_exactly_one_token() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("bearer token-1"));
        assert_eq!(extract_bearer_token(&headers).as_deref(), Some("token-1"));

        headers.insert(AUTHORIZATION, HeaderValue::from_static("BEARER token-2"));
        assert_eq!(extract_bearer_token(&headers).as_deref(), Some("token-2"));

        headers.insert(AUTHORIZATION, HeaderValue::from_static("Basic token-3"));
        assert!(extract_bearer_token(&headers).is_none());

        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_static("Bearer token-4 extra"),
        );
        assert!(extract_bearer_token(&headers).is_none());
    }

    #[test]
    fn browser_cookie_is_parsed_and_authorization_header_wins() {
        let mut headers = HeaderMap::new();
        headers.insert(
            COOKIE,
            HeaderValue::from_static(
                "other=1; rustok_oauth_browser_session=session-token; another=2",
            ),
        );
        assert_eq!(
            extract_browser_access_token(&headers).as_deref(),
            Some("session-token")
        );

        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_static("Bearer header-token"),
        );
        assert_eq!(
            extract_browser_access_token(&headers).as_deref(),
            Some("header-token")
        );
    }

    #[test]
    fn oauth_browser_credentials_reject_duplicate_authorization_fields() {
        let mut headers = HeaderMap::new();
        headers.append(
            AUTHORIZATION,
            HeaderValue::from_static("Bearer first-token"),
        );
        headers.append(
            AUTHORIZATION,
            HeaderValue::from_static("Bearer second-token"),
        );
        headers.insert(
            COOKIE,
            HeaderValue::from_static("rustok_oauth_browser_session=cookie-token"),
        );

        assert!(extract_bearer_token(&headers).is_none());
        assert!(
            extract_browser_access_token(&headers).is_none(),
            "invalid explicit authorization must not fall back to the cookie"
        );

        headers.remove(AUTHORIZATION);
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Basic other-token"));
        assert!(
            extract_browser_access_token(&headers).is_none(),
            "unsupported explicit authorization must not use a different principal"
        );
    }

    #[test]
    fn oauth_browser_cookie_rejects_ambiguous_duplicates_across_fields() {
        let mut headers = HeaderMap::new();
        headers.append(
            COOKIE,
            HeaderValue::from_static("other=1; rustok_oauth_browser_session=first"),
        );
        headers.append(
            COOKIE,
            HeaderValue::from_static("rustok_oauth_browser_session=second"),
        );
        assert!(extract_browser_access_token(&headers).is_none());

        headers.remove(COOKIE);
        headers.append(COOKIE, HeaderValue::from_static("other=1"));
        headers.append(
            COOKIE,
            HeaderValue::from_static("rustok_oauth_browser_session=only-token"),
        );
        assert_eq!(
            extract_browser_access_token(&headers).as_deref(),
            Some("only-token"),
            "split Cookie fields with one session name are unambiguous"
        );

        headers.append(
            COOKIE,
            HeaderValue::from_static("rustok_oauth_browser_session="),
        );
        assert!(extract_browser_access_token(&headers).is_none());
    }

    #[test]
    fn production_oauth_browser_session_cookie_is_secure_without_forwarded_headers() {
        let headers = HeaderMap::new();
        let settings = RustokSettings::default();
        let peer_ip = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 4));
        let secure = crate::common::secure_cookie_required_for_environment(
            &headers,
            Some(peer_ip),
            &settings.runtime.request_trust,
            true,
        );

        let cookie = build_oauth_browser_session_cookie_for_secure(
            "token-123",
            secure,
            Some(OAUTH_BROWSER_SESSION_TTL_SECS),
        );
        assert!(cookie.contains("; Secure"));
    }

    #[test]
    fn browser_session_cookie_respects_secure_forwarding() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("x-forwarded-proto"),
            HeaderValue::from_static("https"),
        );
        let settings = trusted_settings();
        let peer_ip = IpAddr::V4(Ipv4Addr::new(10, 1, 2, 3));

        let cookie = build_oauth_browser_session_cookie("token-123", &headers, peer_ip, &settings);
        assert!(cookie.contains("rustok_oauth_browser_session=token-123"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Lax"));
        assert!(cookie.contains("Max-Age=600"));
        assert!(cookie.contains("Secure"));

        let cleared = clear_oauth_browser_session_cookie(&headers, peer_ip, &settings);
        assert!(cleared.contains("Max-Age=0"));
        assert!(cleared.contains("Secure"));
    }

    #[test]
    fn redirect_helpers_append_query_and_clear_cookie() {
        let settings = trusted_settings();
        let peer_ip = IpAddr::V4(Ipv4Addr::new(10, 1, 2, 3));
        let mut request_headers = HeaderMap::new();
        request_headers.insert(
            HeaderName::from_static("x-forwarded-proto"),
            HeaderValue::from_static("https"),
        );
        let (status, headers) = redirect_with_code(
            "https://client.example.com/callback",
            "auth-code",
            Some("state-123"),
            &request_headers,
            peer_ip,
            &settings,
        );
        assert_eq!(status, StatusCode::FOUND);
        assert_eq!(
            headers[0].1,
            "https://client.example.com/callback?code=auth-code&state=state-123"
        );
        assert!(headers[1].1.contains("Max-Age=0"));
        assert!(headers[1].1.contains("Secure"));

        let (_, loopback_headers) = redirect_with_code(
            "http://127.0.0.1/callback",
            "auth-code",
            None,
            &request_headers,
            peer_ip,
            &settings,
        );
        assert!(loopback_headers[1].1.contains("Secure"));

        let (status, headers) = redirect_with_error(
            "https://client.example.com/callback",
            "access_denied",
            "The user denied access",
            Some("state-123"),
            &request_headers,
            peer_ip,
            &settings,
        );
        assert_eq!(status, StatusCode::FOUND);
        assert!(headers[0].1.contains("error=access_denied"));
        assert!(headers[0].1.contains("state=state-123"));
        assert!(headers[1].1.contains("Max-Age=0"));
    }

    #[test]
    fn authorization_required_page_mentions_browser_session_endpoint() {
        let (status, html) = render_authorization_required("Admin App");
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(html.0.contains("POST /api/oauth/browser-session"));
        assert!(html.0.contains("Admin App"));
    }

    #[test]
    fn consent_page_escapes_fields_and_contains_expected_form_inputs() {
        let mut validated = validated_request("third_party");
        validated.app.name = "Partner <App>".to_string();
        validated.redirect_uri = "https://client.example.com/callback?from=<unsafe>".to_string();
        validated.requested_scopes = vec!["profile".to_string(), "catalog:read".to_string()];

        let request = BrowserAuthorizeRequest {
            response_type: "code".to_string(),
            client_id: validated.app.client_id.to_string(),
            redirect_uri: validated.redirect_uri.clone(),
            scope: Some("profile catalog:read".to_string()),
            state: Some("\"quoted-state\"".to_string()),
            code_challenge: "challenge<&>".to_string(),
            code_challenge_method: Some("S256".to_string()),
        };

        let html = render_consent_page(
            &validated,
            &request,
            "user+test@example.com",
            "1893456000.deadbeef",
        );
        assert!(html.contains("Authorize Partner &lt;App&gt;"));
        assert!(html.contains("user+test@example.com"));
        assert!(html.contains("name=\"client_id\""));
        assert!(html.contains("name=\"redirect_uri\""));
        assert!(html.contains("name=\"scope\""));
        assert!(html.contains("name=\"state\""));
        assert!(html.contains("name=\"code_challenge\""));
        assert!(html.contains("method=\"post\" action=\"/api/oauth/consent\""));
        assert!(html.contains("name=\"consent_csrf\""));
        assert!(html.contains("1893456000.deadbeef"));
        assert!(html.contains("&lt;unsafe&gt;"));
        assert!(html.contains("challenge&lt;&amp;&gt;"));
        assert!(html.contains("&quot;quoted-state&quot;"));
    }

    #[test]
    fn consent_page_preserves_absent_optional_oauth_parameters() {
        let validated = validated_request("third_party");
        let request = BrowserAuthorizeRequest {
            response_type: "code".to_string(),
            client_id: validated.app.client_id.to_string(),
            redirect_uri: validated.redirect_uri.clone(),
            scope: None,
            state: None,
            code_challenge: validated.code_challenge.clone(),
            code_challenge_method: Some("S256".to_string()),
        };

        let html = render_consent_page(
            &validated,
            &request,
            "user@example.com",
            "1893456000.deadbeef",
        );
        assert!(!html.contains("name=\"scope\""), "{html}");
        assert!(!html.contains("name=\"state\""), "{html}");
        assert!(html.contains("name=\"consent_csrf\""));
        assert!(html.contains("profile"));
        assert!(html.contains("catalog:read"));
    }

    #[test]
    fn userinfo_scope_checks_require_openid_and_gate_claims() {
        assert!(oauth_scope_contains(
            &["openid".to_string(), "profile".to_string()],
            "openid"
        ));
        assert!(!oauth_scope_contains(
            &["profile".to_string(), "email".to_string()],
            "openid"
        ));
        assert!(oauth_scope_contains(&["email".to_string()], "email"));
        assert!(!oauth_scope_contains(&["profile".to_string()], "email"));
    }
    #[test]
    fn consent_requirement_depends_on_app_type() {
        assert!(validated_request("third_party").app.requires_user_consent());
        assert!(!validated_request("first_party").app.requires_user_consent());
    }

    #[test]
    fn cookie_browser_authorization_never_skips_confirmation() {
        for (requires_consent, has_consent) in
            [(false, false), (false, true), (true, false), (true, true)]
        {
            assert!(
                !may_skip_browser_authorization_confirmation(
                    false,
                    requires_consent,
                    has_consent,
                ),
                "cookie-authenticated GET must always require POST confirmation"
            );
        }

        assert!(may_skip_browser_authorization_confirmation(true, false, false));
        assert!(may_skip_browser_authorization_confirmation(true, true, true));
        assert!(!may_skip_browser_authorization_confirmation(true, true, false));
    }

    #[tokio::test]
    async fn oauth_cache_policy_covers_success_redirects_and_errors() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::ServiceExt;

        let router = axum::Router::new()
            .route("/ok", get(|| async { StatusCode::OK }))
            .route("/redirect", get(|| async { StatusCode::FOUND }))
            .route("/error", get(|| async { StatusCode::BAD_REQUEST }))
            .layer(axum::middleware::from_fn(disable_oauth_caching));

        for path in ["/ok", "/redirect", "/error"] {
            let response = router
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .expect("OAuth response");
            assert_eq!(response.headers().get(CACHE_CONTROL).unwrap(), "no-store");
            assert_eq!(response.headers().get(PRAGMA).unwrap(), "no-cache");
        }
    }

    #[test]
    fn oauth_code_issuance_maps_stale_security_decisions_without_leaking_details() {
        let cases = [
            (crate::error::Error::NotFound, "invalid_client", StatusCode::UNAUTHORIZED),
            (
                crate::error::Error::BadRequest("secret internal reason".to_string()),
                "invalid_request",
                StatusCode::BAD_REQUEST,
            ),
            (
                crate::error::Error::Unauthorized("secret internal reason".to_string()),
                "access_denied",
                StatusCode::FORBIDDEN,
            ),
            (
                crate::error::Error::InternalServerError,
                "server_error",
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        ];
        for (error, expected_code, expected_status) in cases {
            let protocol = map_code_issuance_error(error);
            assert_eq!(protocol.error, expected_code);
            assert!(!protocol.error_description.contains("secret internal reason"));
            assert_eq!(oauth_error_response(protocol).status(), expected_status);
        }
    }

    #[test]
    fn oauth_client_lookup_outage_is_not_misreported_as_bad_credentials() {
        let error = oauth_client_lookup_error();
        assert_eq!(error.error, "server_error");
        let response = oauth_error_response(error);
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn oauth_error_response_distinguishes_provider_failures_from_bad_requests() {
        for (error, expected_status) in [
            ("server_error", StatusCode::INTERNAL_SERVER_ERROR),
            ("temporarily_unavailable", StatusCode::SERVICE_UNAVAILABLE),
            ("access_denied", StatusCode::FORBIDDEN),
            ("invalid_scope", StatusCode::BAD_REQUEST),
        ] {
            let response = oauth_error_response(TokenErrorResponse {
                error: error.to_string(),
                error_description: "An OAuth request failed".to_string(),
            });
            assert_eq!(response.status(), expected_status, "{error}");
            assert_eq!(response.headers().get(CACHE_CONTROL).unwrap(), "no-store");
        }
    }

    #[test]
    fn oauth_error_response_sets_no_store_headers() {
        let err = TokenErrorResponse {
            error: "invalid_client".to_string(),
            error_description: "Invalid client credentials".to_string(),
        };
        let response = oauth_error_response(err);
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(response.headers().get(CACHE_CONTROL).unwrap(), "no-store");
        assert_eq!(response.headers().get(PRAGMA).unwrap(), "no-cache");
    }

    fn basic_authorization_header(client_id: &str, client_secret: &str) -> HeaderValue {
        use base64::{Engine as _, engine::general_purpose::STANDARD};

        let encode_component = |value: &str| {
            url::form_urlencoded::byte_serialize(value.as_bytes()).collect::<String>()
        };
        let credentials = format!(
            "{}:{}",
            encode_component(client_id),
            encode_component(client_secret),
        );
        let header = format!("Basic {}", STANDARD.encode(credentials.as_bytes()));
        HeaderValue::from_bytes(header.as_bytes()).expect("valid Basic authorization header")
    }

    #[test]
    fn oauth_client_secret_basic_decodes_form_encoded_credentials() {
        let client_id = "client:identifier +";
        let client_secret = "secret:with + chars";
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            basic_authorization_header(client_id, client_secret),
        );

        assert_eq!(
            parse_basic_client_credentials(&headers).unwrap(),
            Some((client_id.to_string(), client_secret.to_string())),
        );

        let mut parsed_client_id = None;
        let mut parsed_client_secret = None;
        apply_basic_client_credentials(
            &headers,
            &mut parsed_client_id,
            &mut parsed_client_secret,
        )
        .unwrap();
        assert_eq!(parsed_client_id.as_deref(), Some(client_id));
        assert_eq!(parsed_client_secret.as_deref(), Some(client_secret));
    }

    #[test]
    fn oauth_client_auth_rejects_multiple_or_mixed_authentication_methods() {
        let mut duplicate_headers = HeaderMap::new();
        duplicate_headers.append(
            AUTHORIZATION,
            basic_authorization_header("client", "secret"),
        );
        duplicate_headers.append(
            AUTHORIZATION,
            basic_authorization_header("client", "secret"),
        );
        assert_eq!(
            parse_basic_client_credentials(&duplicate_headers)
                .unwrap_err()
                .error,
            "invalid_request",
        );

        let mut basic_headers = HeaderMap::new();
        basic_headers.insert(
            AUTHORIZATION,
            basic_authorization_header("client", "secret"),
        );
        let mut body_client_id = Some("body-client".to_string());
        let mut body_client_secret = None;
        assert_eq!(
            apply_basic_client_credentials(
                &basic_headers,
                &mut body_client_id,
                &mut body_client_secret,
            )
            .unwrap_err()
            .error,
            "invalid_request",
        );
    }

    #[test]
    fn oauth_client_secret_basic_rejects_unsupported_and_malformed_credentials() {
        let mut unsupported = HeaderMap::new();
        unsupported.insert(AUTHORIZATION, HeaderValue::from_static("Bearer token"));
        assert_eq!(
            parse_basic_client_credentials(&unsupported)
                .unwrap_err()
                .error,
            "invalid_client",
        );

        let mut malformed = HeaderMap::new();
        malformed.insert(AUTHORIZATION, HeaderValue::from_static("Basic !!!"));
        assert_eq!(
            parse_basic_client_credentials(&malformed)
                .unwrap_err()
                .error,
            "invalid_client",
        );

        let mut malformed_form = HeaderMap::new();
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let malformed_header =
            format!("Basic {}", STANDARD.encode(b"client%ZZ:secret"));
        malformed_form.insert(
            AUTHORIZATION,
            HeaderValue::from_bytes(malformed_header.as_bytes()).unwrap(),
        );
        assert_eq!(
            parse_basic_client_credentials(&malformed_form)
                .unwrap_err()
                .error,
            "invalid_client",
        );
    }

    #[test]
    fn oauth_revocation_secret_policy_matches_public_and_confidential_clients() {
        let mut app = sample_app("first_party");
        assert!(!oauth_revocation_requires_client_secret(&app));

        app.app_type = "mobile".to_string();
        assert!(!oauth_revocation_requires_client_secret(&app));

        app.app_type = "third_party".to_string();
        assert!(oauth_revocation_requires_client_secret(&app));
        app.client_secret_hash = None;
        assert!(!oauth_revocation_requires_client_secret(&app));

        app.app_type = "service".to_string();
        assert!(oauth_revocation_requires_client_secret(&app));
        app.client_secret_hash = None;
        assert!(oauth_revocation_requires_client_secret(&app));
    }

    #[test]
    fn oauth_invalid_client_responses_advertise_basic_authentication_only_on_client_endpoints() {
        let generic = oauth_error_response(TokenErrorResponse {
            error: "invalid_client".to_string(),
            error_description: "Invalid client credentials".to_string(),
        });
        assert!(generic.headers().get(WWW_AUTHENTICATE).is_none());

        let token = oauth_token_http_response(
            StatusCode::UNAUTHORIZED,
            TokenErrorResponse {
                error: "invalid_client".to_string(),
                error_description: "Invalid client credentials".to_string(),
            },
        );
        let revoke = add_basic_auth_challenge(oauth_error_response(TokenErrorResponse {
            error: "invalid_client".to_string(),
            error_description: "Invalid client credentials".to_string(),
        }));

        for response in [token, revoke] {
            assert_eq!(
                response
                    .headers()
                    .get(WWW_AUTHENTICATE)
                    .and_then(|value| value.to_str().ok()),
                Some("Basic realm=\"oauth\""),
            );
            assert_eq!(
                response
                    .headers()
                    .get(CACHE_CONTROL)
                    .and_then(|value| value.to_str().ok()),
                Some("no-store"),
            );
        }
    }
}
