use crate::{TranslationError, TranslationResult};
use rustok_api::PortContext;

/// Canonical idempotency handle for all Translation write operations.
/// Invariant: 1..=191 bytes, trimmed (no leading/trailing whitespace), non-empty.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdempotencyKey(String);

impl IdempotencyKey {
    pub fn new(key: &str) -> TranslationResult<Self> {
        if key.is_empty() || key.len() > 191 || key.trim() != key {
            return Err(TranslationError::InvalidRequest(
                "translation idempotency key must contain 1..=191 trimmed non-empty bytes".to_string(),
            ));
        }
        Ok(Self(key.to_string()))
    }

    pub fn require(context: &PortContext) -> TranslationResult<Self> {
        let key = context.idempotency_key.as_deref().unwrap_or_default();
        Self::new(key)
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl AsRef<str> for IdempotencyKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for IdempotencyKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
