//! OAuth 2.0 Authorization Server Metadata (RFC 8414).
//!
//! The \`openid-configuration\` path is retained as a compatibility alias for
//! authorization-server metadata. It is not advertised as a full OpenID Connect
//! Provider Configuration document because this server does not implement the
//! complete OIDC ID-token/JWKS surface.

use axum::{
    Json,
    extract::State,
    routing::get,
};
use reqwest::Url;
use serde::Serialize;
use crate::error::Error;
use crate::services::server_runtime_context::ServerAuthRuntime;

const AUTHORIZATION_ENDPOINT_PATH: &str = "api/oauth/authorize";
const TOKEN_ENDPOINT_PATH: &str = "api/oauth/token";
const USERINFO_ENDPOINT_PATH: &str = "api/oauth/userinfo";
const REVOCATION_ENDPOINT_PATH: &str = "api/oauth/revoke";

/// OAuth 2.0 authorization-server metadata.
///
/// `scopes_supported` is intentionally omitted: allowed scopes are configured
/// per OAuth application, so there is no accurate server-wide static list.
#[derive(Debug, Serialize)]
pub struct OAuthAuthorizationServerMetadata {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: String,
    pub revocation_endpoint: String,

    // Supported lists
    pub response_types_supported: Vec<String>,
    pub response_modes_supported: Vec<String>,
    pub grant_types_supported: Vec<String>,
    pub token_endpoint_auth_methods_supported: Vec<String>,
    pub revocation_endpoint_auth_methods_supported: Vec<String>,
    pub code_challenge_methods_supported: Vec<String>,

    // UserInfo claims actually returned by the server.
    pub claims_supported: Vec<String>,
}

async fn get_metadata(
    State(ctx): State<ServerAuthRuntime>,
) -> Result<Json<OAuthAuthorizationServerMetadata>, Error> {
    tracing::debug!("oauth_metadata::get_metadata entry");
    let auth_config = ctx
        .auth_config()
        .ok_or_else(|| Error::Message("Auth config error".into()))?;

    // Discovery must advertise the exact issuer used to sign JWTs. Never derive
    // it from Host or forwarded headers: those are request-controlled and can
    // differ from the issuer embedded in already-issued tokens.
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

        response_types_supported: vec!["code".into()],
        // The authorization endpoint returns parameters in the query component.
        // The implementation does not support fragment/form_post response modes.
        response_modes_supported: vec!["query".into()],

        grant_types_supported: vec![
            "authorization_code".into(),
            "client_credentials".into(),
            "refresh_token".into(),
        ],

        token_endpoint_auth_methods_supported: vec![
            "none".into(),
            "client_secret_post".into(),
            "client_secret_basic".into(),
        ],
        revocation_endpoint_auth_methods_supported: vec![
            "none".into(),
            "client_secret_post".into(),
            "client_secret_basic".into(),
        ],

        code_challenge_methods_supported: vec!["S256".into()],

        claims_supported: vec![
            "sub".into(),
            "role".into(),
            "tenant_id".into(),
            "name".into(),
            "email".into(),
            "email_verified".into(),
        ],
    })
}

fn parse_metadata_issuer(issuer: &str) -> Result<Url, Error> {
    if issuer.chars().any(char::is_whitespace) {
        return Err(Error::Message(
            "OAuth metadata issuer URL must not contain whitespace".into(),
        ));
    }

    let url = Url::parse(issuer)
        .map_err(|_| Error::Message("OAuth metadata requires an absolute issuer URL".into()))?;

    let is_local_http =
        url.scheme() == "http" && matches!(url.host_str(), Some("localhost") | Some("127.0.0.1"));
    if (url.scheme() != "https" && !is_local_http) || url.host_str().is_none() {
        return Err(Error::Message(
            "OAuth metadata requires an HTTPS issuer URL (or http://localhost or http://127.0.0.1 for local development)".into(),
        ));
    }

    if issuer.contains('@') || !url.username().is_empty() || url.password().is_some() {
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

        assert_eq!(
            metadata.issuer,
            "https://api.example.com/",
            "discovery must preserve the exact configured JWT issuer"
        );
        assert_eq!(
            metadata.authorization_endpoint,
            "https://api.example.com/api/oauth/authorize"
        );
        let serialized = serde_json::to_value(&metadata).expect("metadata serializes");
        assert!(
            serialized.get("scopes_supported").is_none(),
            "per-application scopes must not be advertised as a false global list"
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
        assert_eq!(
            metadata.claims_supported,
            vec![
                "sub".to_string(),
                "role".to_string(),
                "tenant_id".to_string(),
                "name".to_string(),
                "email".to_string(),
                "email_verified".to_string(),
            ],
            "discovery must advertise only claims the UserInfo handler actually returns"
        );
        let supported_client_auth_methods = vec![
            "none".to_string(),
            "client_secret_post".to_string(),
            "client_secret_basic".to_string(),
        ];
        assert_eq!(
            metadata.token_endpoint_auth_methods_supported,
            supported_client_auth_methods,
            "discovery must advertise public-client auth and every confidential method implemented by the token endpoint"
        );
        assert_eq!(
            metadata.revocation_endpoint_auth_methods_supported,
            supported_client_auth_methods,
            "discovery must advertise public-client revocation and every confidential method implemented by the revocation endpoint"
        );
        assert_eq!(
            serialized["revocation_endpoint_auth_methods_supported"],
            serde_json::json!(["none", "client_secret_post", "client_secret_basic"]),
            "revocation authentication methods must be present in the serialized discovery document"
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
            "https://user:password@api.example.com",
            "https://@api.example.com",
            "https://api.example.com ",
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
    }

    #[test]
    fn metadata_accepts_localhost_and_loopback_http_issuers() {
        for local_issuer in [
            "http://localhost:3000",
            "http://localhost:8080/",
            "http://127.0.0.1:3000",
        ] {
            let metadata =
                metadata_for_issuer(local_issuer).expect("local issuer should be accepted");
            assert!(
                metadata
                    .authorization_endpoint
                    .contains("/api/oauth/authorize")
            );
            assert!(metadata.token_endpoint.contains("/api/oauth/token"));
        }
    }
}
