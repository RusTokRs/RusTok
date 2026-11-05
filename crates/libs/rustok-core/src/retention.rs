use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;

/// Lifecycle policy for retained domain evidence.
///
/// `owner_lifecycle` keeps evidence until its owner is retired, `retain_until`
/// keeps it through one explicit future deadline, and `legal_hold` prevents
/// automatic collection until an authorized owner releases the hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetentionPolicy {
    OwnerLifecycle,
    RetainUntil,
    LegalHold,
}

impl RetentionPolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OwnerLifecycle => "owner_lifecycle",
            Self::RetainUntil => "retain_until",
            Self::LegalHold => "legal_hold",
        }
    }

    pub fn validate(
        self,
        retain_until: Option<&DateTime<FixedOffset>>,
        now: DateTime<FixedOffset>,
    ) -> Result<(), RetentionPolicyError> {
        match (self, retain_until) {
            (Self::RetainUntil, Some(retain_until)) if *retain_until > now => Ok(()),
            (Self::RetainUntil, Some(_)) => Err(RetentionPolicyError::DeadlineNotFuture),
            (Self::RetainUntil, None) => Err(RetentionPolicyError::MissingDeadline),
            (Self::OwnerLifecycle | Self::LegalHold, None) => Ok(()),
            _ => Err(RetentionPolicyError::UnexpectedDeadline),
        }
    }

    pub const fn automatically_collectible(self) -> bool {
        !matches!(self, Self::LegalHold)
    }
}

impl FromStr for RetentionPolicy {
    type Err = RetentionPolicyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "owner_lifecycle" => Ok(Self::OwnerLifecycle),
            "retain_until" => Ok(Self::RetainUntil),
            "legal_hold" => Ok(Self::LegalHold),
            _ => Err(RetentionPolicyError::UnknownPolicy),
        }
    }
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum RetentionPolicyError {
    #[error("retention policy is unknown")]
    UnknownPolicy,
    #[error("retain_until policy requires a deadline")]
    MissingDeadline,
    #[error("retain_until deadline must be in the future")]
    DeadlineNotFuture,
    #[error("only retain_until policy may carry a deadline")]
    UnexpectedDeadline,
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};
    use std::str::FromStr;

    use super::{RetentionPolicy, RetentionPolicyError};

    #[test]
    fn retain_until_requires_a_future_deadline() {
        let now = Utc::now().fixed_offset();
        assert_eq!(
            RetentionPolicy::RetainUntil.validate(None, now),
            Err(RetentionPolicyError::MissingDeadline)
        );
        assert_eq!(
            RetentionPolicy::RetainUntil.validate(Some(&(now - Duration::seconds(1))), now),
            Err(RetentionPolicyError::DeadlineNotFuture)
        );
        assert!(
            RetentionPolicy::RetainUntil
                .validate(Some(&(now + Duration::seconds(1))), now)
                .is_ok()
        );
    }

    #[test]
    fn legal_hold_is_never_an_automatic_collection_candidate() {
        assert!(!RetentionPolicy::LegalHold.automatically_collectible());
        assert!(RetentionPolicy::OwnerLifecycle.automatically_collectible());
    }

    #[test]
    fn persisted_names_are_closed() {
        assert_eq!(
            RetentionPolicy::from_str("retain_until"),
            Ok(RetentionPolicy::RetainUntil)
        );
        assert_eq!(
            RetentionPolicy::from_str("forever"),
            Err(RetentionPolicyError::UnknownPolicy)
        );
    }
}
