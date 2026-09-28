//! OAuth 2.0 Authorization Server Metadata (RFC 8414)
//! OpenID Connect Discovery 1.0

use axum::{Json, extract::State, routing::get};

use crate::error::Error;
use crate::services::server_runtime_context::ServerAuthRuntime;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct OAuthAuthorizationServerMetadata {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: String,
    pub revocation_endpoint: String,

    // Supported lists
    pub scopes_supported: Vec<String>,
    pub response_types_supported: Vec<String>,
    pub grant_types_supported: Vec<String>,
    pub token_endpoint_auth_methods_supported: Vec<String>,
    pub code_challenge_methods_supported: Vec<String>,

    // Claims
    pub claims_supported: Vec<String>,
}

async fn get_metadata(
    State(ctx): State<ServerAuthRuntime>,
) -> Result<Json<OAuthAuthorizationServerMetadata>, Error> {
    let auth_config = ctx
        .auth_config()
        .ok_or_else(|| Error::Message("Auth config error".into()))?;

    let production = crate::common::settings::is_production_environment();
    let domain = match std::env::var("RUSTOK_PUBLIC_URL") {
        Ok(value) if !value.trim().is_empty() => value.trim().trim_end_matches('/').to_string(),
        _ if production => {
            return Err(Error::BadRequest(
                "RUSTOK_PUBLIC_URL must be configured for production OAuth/OIDC metadata"
                    .to_string(),
            ));
        }
        _ => "http://localhost:5150".to_string(),
    };
    let public_url = reqwest::Url::parse(&domain).map_err(|_| {
        Error::BadRequest("RUSTOK_PUBLIC_URL must be a valid absolute HTTP(S) URL".to_string())
    })?;
    if !matches!(public_url.scheme(), "http" | "https")
        || public_url.username() != ""
        || public_url.password().is_some()
        || public_url.query().is_some()
        || public_url.fragment().is_some()
    {
        return Err(Error::BadRequest(
            "RUSTOK_PUBLIC_URL must be an absolute HTTP(S) URL without credentials, query, or fragment"
                .to_string(),
        ));
    }

    let issuer = auth_config.issuer.trim();
    let issuer_url = if issuer.starts_with("https://") || issuer.starts_with("http://") {
        issuer.to_string()
    } else {
        domain.clone()
    };

    Ok(Json(OAuthAuthorizationServerMetadata {
        issuer: issuer_url,
        authorization_endpoint: format!("{domain}/api/oauth/authorize"),
        token_endpoint: format!("{domain}/api/oauth/token"),
        userinfo_endpoint: format!("{domain}/api/oauth/userinfo"),
        revocation_endpoint: format!("{domain}/api/oauth/revoke"),

        scopes_supported: vec![
            "openid".into(),
            "profile".into(),
            "email".into(),
            "offline_access".into(),
            "catalog:read".into(),
            "cart:write".into(),
            "orders:read".into(),
            "orders:write".into(),
            "users:read".into(),
            "users:write".into(),
            "admin:*".into(),
            "storefront:*".into(),
        ],

        response_types_supported: vec!["code".into()],

        grant_types_supported: vec![
            "authorization_code".into(),
            "client_credentials".into(),
            "refresh_token".into(),
        ],

        token_endpoint_auth_methods_supported: vec!["client_secret_post".into()],

        code_challenge_methods_supported: vec!["S256".into()],

        claims_supported: vec![
            "sub".into(),
            "email".into(),
            "email_verified".into(),
            "name".into(),
            "role".into(),
            "tenant_id".into(),
        ],
    }))
}

pub fn router() -> crate::routes::ServerRouter {
    axum::Router::new()
        .route("/.well-known/oauth-authorization-server", get(get_metadata))
        .route("/.well-known/openid-configuration", get(get_metadata))
}
