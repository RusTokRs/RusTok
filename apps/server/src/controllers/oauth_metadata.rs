//! OAuth 2.0 Authorization Server Metadata (RFC 8414).
//!
//! The \`openid-configuration\` path is retained as the RFC 8414 alias for
//! authorization-server metadata. It is not advertised as a full OpenID Connect
//! Provider Configuration document because this server does not implement the
//! complete OIDC ID-token/JWKS surface.

use axum::{Json, extract::State, routing::get};
use reqwest::Url;
use serde::Serialize;

use crate::error::Error;
use crate::services::server_runtime_context::ServerAuthRuntime;

const AUTHORIZATION_ENDPOINT_PATH: &str = "api/oauth/authorize";
const TOKEN_ENDPOINT_PATH: &str = "api/oauth/token";
const USERINFO_ENDPOINT_PATH: &str = "api/oauth/userinfo";
const REVOCATION_ENDPOINT_PATH: &str = "api/oauth/revoke";

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
    pub response_modes_supported: Vec<String>,
    pub grant_types_supported: Vec<String>,
    pub token_endpoint_auth_methods_supported: Vec<String>,
    pub code_challenge_methods_supported: Vec<String>,

    // UserInfo claims actually returned by the server.
    pub claims_supported: Vec<String>,
}

async fn get_metadata(
    State(ctx): State<ServerAuthRuntime>,
) -> Result<Json<OAuthAuthorizationServerMetadata>, Error> {
    let auth_config = ctx
        .auth_config()
        .ok_or_else(|| Error::Message("Auth config error".into()))?;

    Ok(Json(metadata_for_issuer(&auth_config.issuer)?))
}

fn metadata_for_issuer(issuer: &str) -> Result<OAuthAuthorizationServerMetadata, Error> {
    let issuer_url = parse_metadata_issuer(issuer)?;

    Ok(OAuthAuthorizationServerMetadata {
        issuer: issuer.to_string(),
        authorization_endpoint: endpoint_url(&issuer_url, AUTHORIZATION_ENDPOINT_PATH)?,
        token_endpoint: endpoint_url(&issuer_url, TOKEN_ENDPOINT_PATH)?,
        userinfo_endpoint: endpoint_url(&issuer_url, USERINFO_ENDPOINT_PATH)?,
        revocation_endpoint: endpoint_url(&issuer_url, REVOCATION_ENDPOINT_PATH)?,

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
        // The authorization endpoint returns parameters in the query component.
        // The implementation does not support fragment/form_post response modes.
        response_modes_supported: vec!["query".into()],

        grant_types_supported: vec![
            "authorization_code".into(),
            "client_credentials".into(),
            "refresh_token".into(),
        ],

        token_endpoint_auth_methods_supported: vec!["client_secret_post".into()],

        code_challenge_methods_supported: vec!["S256".into()],

        claims_supported: vec![
            "sub".into(),
            "role".into(),
            "tenant_id".into(),
            "email".into(),
            "name".into(),
            "email_verified".into(),
        ],
    })
}

fn parse_metadata_issuer(issuer: &str) -> Result<Url, Error> {
    let url = Url::parse(issuer)
        .map_err(|_| Error::Message("OAuth metadata requires an absolute issuer URL".into()))?;

    if url.scheme() != "https" || url.host_str().is_none() {
        return Err(Error::Message(
            "OAuth metadata requires an HTTPS issuer URL".into(),
        ));
    }

    if !url.username().is_empty() || url.password().is_some() {
        return Err(Error::Message(
            "OAuth metadata issuer must not contain userinfo".into(),
        ));
    }

    if url.query().is_some() || url.fragment().is_some() {
        return Err(Error::Message(
            "OAuth metadata issuer must not contain a query or fragment".into(),
        ));
    }

    if !url.path().is_empty() && url.path() != "/" {
        return Err(Error::Message(
            "OAuth metadata issuer must not contain a path because the server publishes the well-known endpoint at its root".into(),
        ));
    }

    Ok(url)
}

fn endpoint_url(issuer: &Url, path: &str) -> Result<String, Error> {
    issuer
        .join(path)
        .map(|url| url.to_string())
        .map_err(|_| Error::Message("Invalid OAuth metadata endpoint URL".into()))
}

pub fn router() -> crate::routes::ServerRouter {
    axum::Router::new()
        .route("/.well-known/oauth-authorization-server", get(get_metadata))
        .route("/.well-known/openid-configuration", get(get_metadata))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_uses_auth_issuer_as_the_single_public_base() {
        let metadata = metadata_for_issuer("https://api.example.com/").expect("valid issuer");

        assert_eq!(metadata.issuer, "https://api.example.com/");
        assert_eq!(
            metadata.authorization_endpoint,
            "https://api.example.com/api/oauth/authorize"
        );
        assert_eq!(
            metadata.token_endpoint,
            "https://api.example.com/api/oauth/token"
        );
        assert_eq!(
            metadata.userinfo_endpoint,
            "https://api.example.com/api/oauth/userinfo"
        );
        assert_eq!(
            metadata.revocation_endpoint,
            "https://api.example.com/api/oauth/revoke"
        );
    }

    #[test]
    fn metadata_rejects_non_url_and_non_https_issuers() {
        for issuer in ["rustok", "http://api.example.com"] {
            let error = metadata_for_issuer(issuer).expect_err("issuer must be rejected");
            assert!(!error.to_string().is_empty());
        }
    }

    #[test]
    fn metadata_rejects_query_fragment_and_path_issuers() {
        for issuer in [
            "https://api.example.com?tenant=one",
            "https://api.example.com#tenant-one",
            "https://api.example.com/issuer-one",
        ] {
            assert!(
                metadata_for_issuer(issuer).is_err(),
                "issuer must be rejected: {issuer}"
            );
        }
    }

    #[test]
    fn metadata_advertises_only_supported_query_response_mode() {
        let metadata = metadata_for_issuer("https://api.example.com").expect("valid issuer");

        assert_eq!(metadata.response_modes_supported, vec!["query".to_string()]);
        assert_eq!(
            metadata.claims_supported,
            vec![
                "sub".to_string(),
                "role".to_string(),
                "tenant_id".to_string(),
                "email".to_string(),
                "name".to_string(),
                "email_verified".to_string(),
            ]
        );
    }
}
