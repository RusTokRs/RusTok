//! OAuth 2.0 Authorization Server Metadata (RFC 8414)
//! OpenID Connect Discovery 1.0

use crate::auth::auth_config_from_ctx;
use axum::{extract::State, routing::get, Json};
use loco_rs::app::AppContext;
use loco_rs::controller::Routes;
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
    State(ctx): State<AppContext>,
) -> Result<Json<OAuthAuthorizationServerMetadata>, loco_rs::Error> {
    let auth_config = auth_config_from_ctx(&ctx)
        .map_err(|_| loco_rs::Error::Message("Auth config error".into()))?;

    // Generate issuer base URL
    // In a real environment, this should be the public URL from config.
    // Fallback to localhost if not configured (useful for dev)
    let domain =
        std::env::var("RUSTOK_PUBLIC_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
    let issuer = auth_config.issuer.clone();

    // Normally issuer is a URL in OIDC, but if it's just "rustok", we return the domain base
    let issuer_url = if issuer.starts_with("http") {
        issuer
    } else {
        domain.clone()
    };

    Ok(Json(OAuthAuthorizationServerMetadata {
        issuer: issuer_url,
        authorization_endpoint: format!("{}/api/oauth/authorize", domain),
        token_endpoint: format!("{}/api/oauth/token", domain),
        userinfo_endpoint: format!("{}/api/oauth/userinfo", domain),
        revocation_endpoint: format!("{}/api/oauth/revoke", domain),

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
            "iss".into(),
            "aud".into(),
            "exp".into(),
            "iat".into(),
            "client_id".into(),
            "role".into(),
            "tenant_id".into(),
            "email".into(),
            "name".into(),
        ],
    }))
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/.well-known/oauth-authorization-server", get(get_metadata))
        .add("/.well-known/openid-configuration", get(get_metadata))
}
