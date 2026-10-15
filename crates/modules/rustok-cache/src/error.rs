use crate::{
    CacheEnvelopeError, CacheGenerationError, CacheInvalidationMessageError, CacheKeyError,
    CacheLeaseError, CachePolicyError,
};

/// Unified top-level error taxonomy for the cache capability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheError {
    Key(CacheKeyError),
    Policy(CachePolicyError),
    Invalidation(CacheInvalidationMessageError),
    Lease(CacheLeaseError),
    Generation(CacheGenerationError),
    Envelope(CacheEnvelopeError),
    Backend(String),
    Timeout(String),
    Validation(String),
}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Key(err) => write!(f, "{err}"),
            Self::Policy(err) => write!(f, "{err}"),
            Self::Invalidation(err) => write!(f, "{err}"),
            Self::Lease(err) => write!(f, "{err}"),
            Self::Generation(err) => write!(f, "{err}"),
            Self::Envelope(err) => write!(f, "{err}"),
            Self::Backend(msg) => write!(f, "cache backend error: {msg}"),
            Self::Timeout(msg) => write!(f, "cache operation timed out: {msg}"),
            Self::Validation(msg) => write!(f, "cache validation error: {msg}"),
        }
    }
}

impl std::error::Error for CacheError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Key(err) => Some(err),
            Self::Policy(err) => Some(err),
            Self::Invalidation(err) => Some(err),
            Self::Lease(err) => Some(err),
            Self::Generation(err) => Some(err),
            Self::Envelope(err) => Some(err),
            Self::Backend(_) | Self::Timeout(_) | Self::Validation(_) => None,
        }
    }
}

impl From<CacheKeyError> for CacheError {
    fn from(err: CacheKeyError) -> Self {
        Self::Key(err)
    }
}

impl From<CachePolicyError> for CacheError {
    fn from(err: CachePolicyError) -> Self {
        Self::Policy(err)
    }
}

impl From<CacheInvalidationMessageError> for CacheError {
    fn from(err: CacheInvalidationMessageError) -> Self {
        Self::Invalidation(err)
    }
}

impl From<CacheLeaseError> for CacheError {
    fn from(err: CacheLeaseError) -> Self {
        Self::Lease(err)
    }
}

impl From<CacheGenerationError> for CacheError {
    fn from(err: CacheGenerationError) -> Self {
        Self::Generation(err)
    }
}

impl From<CacheEnvelopeError> for CacheError {
    fn from(err: CacheEnvelopeError) -> Self {
        Self::Envelope(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_error_conversions_and_display() {
        let err: CacheError = CacheInvalidationMessageError::EmptyKey.into();
        assert_eq!(err.to_string(), "cache invalidation key must not be empty");

        let backend_err = CacheError::Backend("redis refused connection".to_string());
        assert_eq!(
            backend_err.to_string(),
            "cache backend error: redis refused connection"
        );

        let timeout_err = CacheError::Timeout("get timed out".to_string());
        assert_eq!(
            timeout_err.to_string(),
            "cache operation timed out: get timed out"
        );

        let val_err = CacheError::Validation("invalid ttl".to_string());
        assert_eq!(val_err.to_string(), "cache validation error: invalid ttl");
    }
}
