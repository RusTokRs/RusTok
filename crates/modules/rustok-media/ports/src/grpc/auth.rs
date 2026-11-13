use std::{collections::HashSet, fmt};

use rustok_api::PortActor;
use sha2::{Digest, Sha256};
#[cfg(feature = "grpc-server")]
use subtle::ConstantTimeEq;
use thiserror::Error;
use tonic::metadata::{Ascii, MetadataValue};
#[cfg(feature = "grpc-server")]
use tonic::service::Interceptor;
#[cfg(feature = "grpc-server")]
use tonic::{Request, Status};
#[cfg(feature = "grpc-server")]
use uuid::Uuid;

pub(crate) const AUTHORIZATION_METADATA: &str = "authorization";
pub(crate) const TENANT_ID_METADATA: &str = "x-rustok-tenant-id";
const MAX_BEARER_TOKEN_BYTES: usize = 4_096;
const AUTHORIZATION_DIGEST_BYTES: usize = 32;

/// Deployment-provided service credential for the Media gRPC boundary.
///
/// The value is stored as a prevalidated `Authorization` metadata value. Its
/// `Debug` representation is always redacted, and no API exposes the original secret text.
#[derive(Clone, Eq, PartialEq)]
pub struct MediaGrpcBearerToken {
    authorization: MetadataValue<Ascii>,
    authorization_digest: [u8; AUTHORIZATION_DIGEST_BYTES],
}

impl MediaGrpcBearerToken {
    pub fn new(secret: impl AsRef<str>) -> Result<Self, MediaGrpcAuthenticationError> {
        let secret = secret.as_ref();
        if secret.is_empty()
            || secret.len() > MAX_BEARER_TOKEN_BYTES
            || !secret.is_ascii()
            || secret
                .as_bytes()
                .iter()
                .any(|byte| *byte <= b' ' || *byte == 0x7f)
        {
            return Err(MediaGrpcAuthenticationError::InvalidBearerToken);
        }

        let authorization = format!("Bearer {secret}");
        let authorization_digest = authorization_digest(authorization.as_bytes());
        let authorization = MetadataValue::try_from(authorization.as_str())
            .map_err(|_| MediaGrpcAuthenticationError::InvalidBearerToken)?;
        Ok(Self {
            authorization,
            authorization_digest,
        })
    }

    #[cfg(feature = "grpc-client")]
    pub fn authorization_value(&self) -> MetadataValue<Ascii> {
        self.authorization.clone()
    }

    #[cfg(feature = "grpc-server")]
    pub fn matches_authorization(&self, candidate: &[u8]) -> bool {
        let candidate_digest = authorization_digest(candidate);
        bool::from(self.authorization_digest.ct_eq(&candidate_digest))
    }
}

fn authorization_digest(value: &[u8]) -> [u8; AUTHORIZATION_DIGEST_BYTES] {
    let digest = Sha256::digest(value);
    let mut output = [0_u8; AUTHORIZATION_DIGEST_BYTES];
    output.copy_from_slice(&digest);
    output
}

impl fmt::Debug for MediaGrpcBearerToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MediaGrpcBearerToken")
            .field("authorization", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, Eq, Error, PartialEq)]
pub enum MediaGrpcAuthenticationError {
    #[error("Media gRPC bearer token must be 1..=4096 visible non-whitespace ASCII bytes")]
    InvalidBearerToken,
}

/// Media operations authorized by the server-side authentication boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MediaGrpcOperation {
    GetAsset,
    GetAssetReferenceAdmission,
    ListAssetReferences,
    LookupAssetReferences,
    ListAssets,
    GetImageDescriptor,
    GetPublicImageAsset,
    GetTranslations,
    PrepareUpload,
    CompleteUpload,
    DeleteAsset,
    RetainAssetReference,
    ReleaseAssetReference,
    UpsertTranslation,
    ReconcileStorage,
}

impl MediaGrpcOperation {
    pub const ALL: [Self; 15] = [
        Self::GetAsset,
        Self::GetAssetReferenceAdmission,
        Self::ListAssetReferences,
        Self::LookupAssetReferences,
        Self::ListAssets,
        Self::GetImageDescriptor,
        Self::GetPublicImageAsset,
        Self::GetTranslations,
        Self::PrepareUpload,
        Self::CompleteUpload,
        Self::DeleteAsset,
        Self::RetainAssetReference,
        Self::ReleaseAssetReference,
        Self::UpsertTranslation,
        Self::ReconcileStorage,
    ];

    pub const READ: [Self; 8] = [
        Self::GetAsset,
        Self::GetAssetReferenceAdmission,
        Self::ListAssetReferences,
        Self::LookupAssetReferences,
        Self::ListAssets,
        Self::GetImageDescriptor,
        Self::GetPublicImageAsset,
        Self::GetTranslations,
    ];

    pub const WRITE: [Self; 7] = [
        Self::PrepareUpload,
        Self::CompleteUpload,
        Self::DeleteAsset,
        Self::RetainAssetReference,
        Self::ReleaseAssetReference,
        Self::UpsertTranslation,
        Self::ReconcileStorage,
    ];
}

/// Authority established by a server-side authentication/authorization interceptor.
#[derive(Clone, Debug)]
pub struct TrustedMediaAuthority {
    pub tenant_id: String,
    pub actor: PortActor,
    pub claims: Vec<String>,
    pub roles: Vec<String>,
    pub(crate) allowed_operations: HashSet<MediaGrpcOperation>,
}

impl TrustedMediaAuthority {
    pub fn new(tenant_id: impl Into<String>, actor: PortActor) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            actor,
            claims: Vec::new(),
            roles: Vec::new(),
            allowed_operations: HashSet::new(),
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

    pub fn allow_operation(mut self, operation: MediaGrpcOperation) -> Self {
        self.allowed_operations.insert(operation);
        self
    }

    pub fn allow_operations(
        mut self,
        operations: impl IntoIterator<Item = MediaGrpcOperation>,
    ) -> Self {
        self.allowed_operations.extend(operations);
        self
    }
}

/// Server-side bearer authenticator that verifies the authorization header and builds a `TrustedMediaAuthority`.
#[cfg(feature = "grpc-server")]
#[derive(Clone)]
pub struct MediaGrpcBearerAuthenticator {
    token: MediaGrpcBearerToken,
    actor: PortActor,
    claims: Vec<String>,
    roles: Vec<String>,
    allowed_operations: HashSet<MediaGrpcOperation>,
}

#[cfg(feature = "grpc-server")]
impl MediaGrpcBearerAuthenticator {
    pub fn new(
        secret: impl AsRef<str>,
        actor: PortActor,
    ) -> Result<Self, MediaGrpcAuthenticationError> {
        Ok(Self {
            token: MediaGrpcBearerToken::new(secret)?,
            actor,
            claims: Vec::new(),
            roles: Vec::new(),
            allowed_operations: MediaGrpcOperation::ALL.into_iter().collect(),
        })
    }

    pub fn with_allowed_operations(
        mut self,
        operations: impl IntoIterator<Item = MediaGrpcOperation>,
    ) -> Self {
        self.allowed_operations = operations.into_iter().collect();
        self
    }

    pub fn with_claims(mut self, claims: impl IntoIterator<Item = String>) -> Self {
        self.claims = claims.into_iter().collect();
        self
    }

    pub fn with_roles(mut self, roles: impl IntoIterator<Item = String>) -> Self {
        self.roles = roles.into_iter().collect();
        self
    }

    pub fn authenticate(&self, request: &Request<()>) -> Result<TrustedMediaAuthority, Status> {
        let authorization = request
            .metadata()
            .get(AUTHORIZATION_METADATA)
            .ok_or_else(authentication_failed)?;
        if !self.token.matches_authorization(authorization.as_bytes()) {
            return Err(authentication_failed());
        }

        let tenant_id = request
            .metadata()
            .get(TENANT_ID_METADATA)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(authentication_failed)?;
        if tenant_id != tenant_id.trim() || Uuid::parse_str(tenant_id).is_err() {
            return Err(authentication_failed());
        }

        Ok(TrustedMediaAuthority {
            tenant_id: tenant_id.to_string(),
            actor: self.actor.clone(),
            claims: self.claims.clone(),
            roles: self.roles.clone(),
            allowed_operations: self.allowed_operations.clone(),
        })
    }
}

#[cfg(feature = "grpc-server")]
impl fmt::Debug for MediaGrpcBearerAuthenticator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MediaGrpcBearerAuthenticator")
            .field("token", &"[REDACTED]")
            .field("actor", &self.actor)
            .field("claims", &self.claims)
            .field("roles", &self.roles)
            .field("allowed_operations", &self.allowed_operations)
            .finish()
    }
}

#[cfg(feature = "grpc-server")]
fn authentication_failed() -> Status {
    Status::unauthenticated("Media service authentication failed")
}

#[cfg(feature = "grpc-server")]
#[derive(Clone, Debug)]
pub struct MediaGrpcBearerInterceptor {
    authenticator: MediaGrpcBearerAuthenticator,
}

#[cfg(feature = "grpc-server")]
impl MediaGrpcBearerInterceptor {
    pub fn new(authenticator: MediaGrpcBearerAuthenticator) -> Self {
        Self { authenticator }
    }

    pub fn from_bearer_token(
        secret: impl AsRef<str>,
        actor: PortActor,
    ) -> Result<Self, MediaGrpcAuthenticationError> {
        Ok(Self::new(MediaGrpcBearerAuthenticator::new(
            secret, actor,
        )?))
    }
}

#[cfg(feature = "grpc-server")]
impl Interceptor for MediaGrpcBearerInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        let authority = self.authenticator.authenticate(&request)?;
        request.extensions_mut().insert(authority);
        Ok(request)
    }
}

#[cfg(test)]
mod tests {
    use super::{MediaGrpcAuthenticationError, MediaGrpcBearerToken};

    #[test]
    fn bearer_token_debug_is_redacted() {
        let token = MediaGrpcBearerToken::new("media-secret")
            .expect("valid bearer token should be accepted");
        let debug = format!("{token:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("media-secret"));
    }

    #[test]
    fn bearer_token_rejects_whitespace_and_control_bytes() {
        for token in ["", "has space", " leading", "trailing ", "line\nbreak"] {
            assert_eq!(
                MediaGrpcBearerToken::new(token).unwrap_err(),
                MediaGrpcAuthenticationError::InvalidBearerToken
            );
        }
    }
}

