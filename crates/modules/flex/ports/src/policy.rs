use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlexDataClassification {
    Public,
    TenantPrivate,
    Personal,
    Sensitive,
    Secret,
    ImmutableTransaction,
}

impl FlexDataClassification {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::TenantPrivate => "tenant_private",
            Self::Personal => "personal",
            Self::Sensitive => "sensitive",
            Self::Secret => "secret",
            Self::ImmutableTransaction => "immutable_transaction",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexAttachedFieldPolicy {
    pub classification: FlexDataClassification,
    pub ai_export_allowed: bool,
}

impl Default for FlexAttachedFieldPolicy {
    fn default() -> Self {
        Self {
            classification: FlexDataClassification::TenantPrivate,
            ai_export_allowed: false,
        }
    }
}

impl FlexAttachedFieldPolicy {
    pub fn validate(self) -> FlexAttachedFieldPolicyResult<()> {
        if self.ai_export_allowed
            && matches!(
                self.classification,
                FlexDataClassification::Secret | FlexDataClassification::ImmutableTransaction
            )
        {
            return Err(FlexAttachedFieldPolicyError::Invalid(
                "AI export cannot be enabled for secret or immutable-transaction attached fields"
                    .to_string(),
            ));
        }
        Ok(())
    }
}

/// Effective attached-field policy together with provenance.
///
/// `explicit=false` means no policy row exists and the returned policy is the fail-closed default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FlexAttachedFieldPolicyResolution {
    pub policy: FlexAttachedFieldPolicy,
    pub explicit: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlexAttachedFieldPolicyError {
    Invalid(String),
    Storage(String),
}

impl fmt::Display for FlexAttachedFieldPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) | Self::Storage(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for FlexAttachedFieldPolicyError {}

pub type FlexAttachedFieldPolicyResult<T> = Result<T, FlexAttachedFieldPolicyError>;

impl std::str::FromStr for FlexDataClassification {
    type Err = FlexAttachedFieldPolicyError;

    fn from_str(value: &str) -> FlexAttachedFieldPolicyResult<Self> {
        match value {
            "public" => Ok(Self::Public),
            "tenant_private" => Ok(Self::TenantPrivate),
            "personal" => Ok(Self::Personal),
            "sensitive" => Ok(Self::Sensitive),
            "secret" => Ok(Self::Secret),
            "immutable_transaction" => Ok(Self::ImmutableTransaction),
            _ => Err(FlexAttachedFieldPolicyError::Invalid(
                "Unknown Flex data classification".into(),
            )),
        }
    }
}
