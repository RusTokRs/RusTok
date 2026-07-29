use std::fmt;

use subtle::ConstantTimeEq;
use thiserror::Error;
use tonic::metadata::{Ascii, MetadataValue};

pub(crate) const AUTHORIZATION_METADATA: &str = "authorization";
pub(crate) const TENANT_ID_METADATA: &str = "x-rustok-tenant-id";
const MAX_BEARER_TOKEN_BYTES: usize = 4_096;

/// Deployment-provided service credential for the Product catalog gRPC boundary.
///
/// The value is stored as a prevalidated `Authorization` metadata value. Its
/// `Debug` representation is always redacted, and no API exposes the original
/// secret text.
#[derive(Clone, Eq, PartialEq)]
pub struct ProductCatalogGrpcBearerToken {
    authorization: MetadataValue<Ascii>,
}

impl ProductCatalogGrpcBearerToken {
    pub fn new(secret: impl AsRef<str>) -> Result<Self, ProductCatalogGrpcAuthenticationError> {
        let secret = secret.as_ref();
        if secret.is_empty()
            || secret.len() > MAX_BEARER_TOKEN_BYTES
            || !secret.is_ascii()
            || secret
                .as_bytes()
                .iter()
                .any(|byte| *byte <= b' ' || *byte == 0x7f)
        {
            return Err(ProductCatalogGrpcAuthenticationError::InvalidBearerToken);
        }

        let authorization = format!("Bearer {secret}");
        let authorization = MetadataValue::try_from(authorization.as_str())
            .map_err(|_| ProductCatalogGrpcAuthenticationError::InvalidBearerToken)?;
        Ok(Self { authorization })
    }

    pub(crate) fn authorization_value(&self) -> MetadataValue<Ascii> {
        self.authorization.clone()
    }

    pub(crate) fn matches_authorization(&self, candidate: &[u8]) -> bool {
        bool::from(self.authorization.as_bytes().ct_eq(candidate))
    }
}

impl fmt::Debug for ProductCatalogGrpcBearerToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductCatalogGrpcBearerToken")
            .field("authorization", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, Copy, Eq, Error, PartialEq)]
pub enum ProductCatalogGrpcAuthenticationError {
    #[error(
        "Product catalog gRPC bearer token must be 1..=4096 visible non-whitespace ASCII bytes"
    )]
    InvalidBearerToken,
}

#[cfg(test)]
mod tests {
    use super::{ProductCatalogGrpcAuthenticationError, ProductCatalogGrpcBearerToken};

    #[test]
    fn bearer_token_debug_is_redacted() {
        let token = ProductCatalogGrpcBearerToken::new("catalog-secret")
            .expect("valid bearer token should be accepted");
        let debug = format!("{token:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("catalog-secret"));
    }

    #[test]
    fn bearer_token_rejects_whitespace_and_control_bytes() {
        for token in ["", "has space", " leading", "trailing ", "line\nbreak"] {
            assert_eq!(
                ProductCatalogGrpcBearerToken::new(token),
                Err(ProductCatalogGrpcAuthenticationError::InvalidBearerToken)
            );
        }
    }

    #[test]
    fn bearer_token_comparison_matches_full_authorization_value() {
        let token = ProductCatalogGrpcBearerToken::new("catalog-secret")
            .expect("valid bearer token should be accepted");
        assert!(token.matches_authorization(b"Bearer catalog-secret"));
        assert!(!token.matches_authorization(b"Bearer catalog-other"));
    }
}
