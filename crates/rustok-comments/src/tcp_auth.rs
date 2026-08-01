use std::{collections::HashSet, fmt, net::SocketAddr};

use async_trait::async_trait;
use rustok_api::{PortActor, PortContext, PortError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use thiserror::Error;
use uuid::Uuid;

use crate::tcp_server::{
    CommentsTcpAuthorityResolver, CommentsTcpOperation, TrustedCommentsTcpAuthority,
};
use crate::CommentsThreadRequest;

pub const COMMENTS_TCP_PROTOCOL_VERSION: u16 = 1;

const MAX_BEARER_TOKEN_BYTES: usize = 4_096;
const AUTHORIZATION_DIGEST_BYTES: usize = 32;
const BEARER_SCHEME: &str = "bearer";
const BEARER_PREFIX: &[u8] = b"Bearer ";

/// Deployment-provided credential for the Comments TCP boundary.
///
/// The original token remains private, its `Debug` representation is always
/// redacted, and comparisons are performed through a fixed-size SHA-256 digest.
#[derive(Clone, Eq, PartialEq)]
pub struct CommentsTcpBearerToken {
    secret: String,
    authorization_digest: [u8; AUTHORIZATION_DIGEST_BYTES],
}

impl CommentsTcpBearerToken {
    pub fn new(secret: impl AsRef<str>) -> Result<Self, CommentsTcpAuthenticationConfigError> {
        let secret = secret.as_ref();
        if !valid_token_text(secret) {
            return Err(CommentsTcpAuthenticationConfigError::InvalidBearerToken);
        }

        Ok(Self {
            secret: secret.to_string(),
            authorization_digest: bearer_authorization_digest(secret.as_bytes()),
        })
    }

    pub(crate) fn credential(&self) -> CommentsTcpCredential {
        CommentsTcpCredential::bearer(self.secret.clone())
    }

    fn matches(&self, credential: Option<&CommentsTcpCredential>) -> bool {
        let Some(credential) = credential else {
            return false;
        };
        if credential.scheme != BEARER_SCHEME || !valid_token_text(&credential.token) {
            return false;
        }

        let candidate_digest = bearer_authorization_digest(credential.token.as_bytes());
        bool::from(self.authorization_digest.ct_eq(&candidate_digest))
    }
}

impl fmt::Debug for CommentsTcpBearerToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommentsTcpBearerToken")
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, Eq, Error, PartialEq)]
pub enum CommentsTcpAuthenticationConfigError {
    #[error(
        "Comments TCP bearer token must be 1..=4096 visible non-whitespace ASCII bytes"
    )]
    InvalidBearerToken,
}

/// Credential carried by the versioned TCP request envelope.
///
/// Fields remain private so callers cannot accidentally log the token through a
/// generated struct formatter. Serialization is the only wire-facing exposure.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct CommentsTcpCredential {
    scheme: String,
    token: String,
}

impl CommentsTcpCredential {
    fn bearer(token: String) -> Self {
        Self {
            scheme: BEARER_SCHEME.to_string(),
            token,
        }
    }

    pub fn scheme(&self) -> &str {
        self.scheme.as_str()
    }
}

impl fmt::Debug for CommentsTcpCredential {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommentsTcpCredential")
            .field("scheme", &self.scheme)
            .field("token", &"[REDACTED]")
            .finish()
    }
}

/// Versioned wire envelope for one typed Comments request.
#[derive(Clone, Serialize, Deserialize)]
pub struct CommentsTcpRequestEnvelope {
    protocol_version: u16,
    credential: Option<CommentsTcpCredential>,
    request: CommentsThreadRequest,
}

impl CommentsTcpRequestEnvelope {
    pub fn unauthenticated(request: CommentsThreadRequest) -> Self {
        Self {
            protocol_version: COMMENTS_TCP_PROTOCOL_VERSION,
            credential: None,
            request,
        }
    }

    pub fn with_bearer(request: CommentsThreadRequest, token: &CommentsTcpBearerToken) -> Self {
        Self {
            protocol_version: COMMENTS_TCP_PROTOCOL_VERSION,
            credential: Some(token.credential()),
            request,
        }
    }

    pub fn protocol_version(&self) -> u16 {
        self.protocol_version
    }

    pub fn credential(&self) -> Option<&CommentsTcpCredential> {
        self.credential.as_ref()
    }

    pub fn request(&self) -> &CommentsThreadRequest {
        &self.request
    }

    pub(crate) fn into_parts(
        self,
    ) -> (u16, Option<CommentsTcpCredential>, CommentsThreadRequest) {
        (self.protocol_version, self.credential, self.request)
    }
}

impl fmt::Debug for CommentsTcpRequestEnvelope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommentsTcpRequestEnvelope")
            .field("protocol_version", &self.protocol_version)
            .field("credential", &self.credential)
            .field("request", &self.request)
            .finish()
    }
}

/// Static service-to-service bearer resolver for the loopback Comments boundary.
///
/// A valid credential authenticates the configured service actor. The claimed
/// tenant must be a canonical UUID and is copied into trusted authority only
/// after authentication. Principal fields are replaced later by the server
/// adapter before provider dispatch.
#[derive(Clone)]
pub struct CommentsTcpBearerAuthorityResolver {
    token: CommentsTcpBearerToken,
    actor: PortActor,
    claims: Vec<String>,
    roles: Vec<String>,
    allowed_operations: HashSet<CommentsTcpOperation>,
}

impl CommentsTcpBearerAuthorityResolver {
    pub fn new(
        secret: impl AsRef<str>,
        actor: PortActor,
    ) -> Result<Self, CommentsTcpAuthenticationConfigError> {
        Ok(Self::from_token(
            CommentsTcpBearerToken::new(secret)?,
            actor,
        ))
    }

    pub fn from_token(token: CommentsTcpBearerToken, actor: PortActor) -> Self {
        Self {
            token,
            actor,
            claims: Vec::new(),
            roles: Vec::new(),
            allowed_operations: HashSet::from(CommentsTcpOperation::ALL),
        }
    }

    pub fn with_claim(mut self, claim: impl Into<String>) -> Self {
        self.claims.push(claim.into());
        self
    }

    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.roles.push(role.into());
        self
    }

    pub fn with_allowed_operations(
        mut self,
        operations: impl IntoIterator<Item = CommentsTcpOperation>,
    ) -> Self {
        self.allowed_operations = operations.into_iter().collect();
        self
    }
}

impl fmt::Debug for CommentsTcpBearerAuthorityResolver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommentsTcpBearerAuthorityResolver")
            .field("token", &"[REDACTED]")
            .field("actor", &self.actor)
            .field("claims", &self.claims)
            .field("roles", &self.roles)
            .field("allowed_operations", &self.allowed_operations)
            .finish()
    }
}

#[async_trait]
impl CommentsTcpAuthorityResolver for CommentsTcpBearerAuthorityResolver {
    async fn authorize(
        &self,
        peer_addr: SocketAddr,
        operation: CommentsTcpOperation,
        credential: Option<&CommentsTcpCredential>,
        claimed_context: &PortContext,
    ) -> Result<TrustedCommentsTcpAuthority, PortError> {
        if !peer_addr.ip().is_loopback()
            || !self.token.matches(credential)
            || !is_canonical_uuid(&claimed_context.tenant_id)
        {
            return Err(authentication_failed());
        }

        if !self.allowed_operations.contains(&operation) {
            return Err(PortError::forbidden(
                "comments.tcp_operation_forbidden",
                "Comments TCP authenticated authority does not allow this operation",
            ));
        }

        Ok(TrustedCommentsTcpAuthority {
            tenant_id: claimed_context.tenant_id.clone(),
            actor: self.actor.clone(),
            claims: self.claims.clone(),
            roles: self.roles.clone(),
        })
    }
}

fn valid_token_text(secret: &str) -> bool {
    !secret.is_empty()
        && secret.len() <= MAX_BEARER_TOKEN_BYTES
        && secret.is_ascii()
        && !secret
            .as_bytes()
            .iter()
            .any(|byte| *byte <= b' ' || *byte == 0x7f)
}

fn is_canonical_uuid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok_and(|parsed| parsed.to_string() == value)
}

fn bearer_authorization_digest(secret: &[u8]) -> [u8; AUTHORIZATION_DIGEST_BYTES] {
    let mut hasher = Sha256::new();
    hasher.update(BEARER_PREFIX);
    hasher.update(secret);
    let digest = hasher.finalize();
    let mut output = [0_u8; AUTHORIZATION_DIGEST_BYTES];
    output.copy_from_slice(&digest);
    output
}

fn authentication_failed() -> PortError {
    PortError::forbidden(
        "comments.tcp_authentication_failed",
        "Comments TCP service authentication failed",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bearer_token_and_credential_debug_are_redacted() {
        let token = CommentsTcpBearerToken::new("comments-secret").unwrap();
        let credential = token.credential();
        let token_debug = format!("{token:?}");
        let credential_debug = format!("{credential:?}");

        assert!(token_debug.contains("[REDACTED]"));
        assert!(credential_debug.contains("[REDACTED]"));
        assert!(!token_debug.contains("comments-secret"));
        assert!(!credential_debug.contains("comments-secret"));
    }

    #[test]
    fn bearer_token_rejects_whitespace_and_control_bytes() {
        for token in ["", "has space", " leading", "trailing ", "line\nbreak"] {
            assert_eq!(
                CommentsTcpBearerToken::new(token),
                Err(CommentsTcpAuthenticationConfigError::InvalidBearerToken)
            );
        }
    }

    #[test]
    fn tenant_authority_requires_canonical_uuid_text() {
        let tenant_id = Uuid::new_v4();
        assert!(is_canonical_uuid(&tenant_id.to_string()));
        assert!(!is_canonical_uuid(&tenant_id.to_string().to_ascii_uppercase()));
        assert!(!is_canonical_uuid("not-a-uuid"));
    }

    #[tokio::test]
    async fn bearer_resolver_authenticates_service_and_tenant() {
        let tenant_id = Uuid::new_v4().to_string();
        let actor_id = Uuid::new_v4().to_string();
        let token = CommentsTcpBearerToken::new("comments-secret").unwrap();
        let credential = token.credential();
        let resolver = CommentsTcpBearerAuthorityResolver::from_token(
            token,
            PortActor::service(actor_id.clone()),
        )
        .with_claim("comments:manage")
        .with_role("admin");
        let context = PortContext::new(
            tenant_id.clone(),
            PortActor::user(Uuid::new_v4().to_string()),
            "en",
            "corr-1",
        );

        let authority = resolver
            .authorize(
                "127.0.0.1:9000".parse().unwrap(),
                CommentsTcpOperation::GetComment,
                Some(&credential),
                &context,
            )
            .await
            .unwrap();

        assert_eq!(authority.tenant_id, tenant_id);
        assert_eq!(authority.actor, PortActor::service(actor_id));
        assert_eq!(authority.claims, ["comments:manage"]);
        assert_eq!(authority.roles, ["admin"]);
    }

    #[tokio::test]
    async fn missing_and_wrong_credentials_share_static_failure() {
        let tenant_id = Uuid::new_v4().to_string();
        let resolver = CommentsTcpBearerAuthorityResolver::new(
            "comments-secret",
            PortActor::service(Uuid::new_v4().to_string()),
        )
        .unwrap();
        let wrong = CommentsTcpBearerToken::new("wrong-secret")
            .unwrap()
            .credential();
        let context = PortContext::new(
            tenant_id,
            PortActor::service(Uuid::new_v4().to_string()),
            "en",
            "corr-1",
        );

        for credential in [None, Some(&wrong)] {
            let error = resolver
                .authorize(
                    "127.0.0.1:9000".parse().unwrap(),
                    CommentsTcpOperation::GetComment,
                    credential,
                    &context,
                )
                .await
                .unwrap_err();
            assert_eq!(error.code, "comments.tcp_authentication_failed");
            assert_eq!(error.message, "Comments TCP service authentication failed");
        }
    }
}
