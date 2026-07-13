use serde::{Deserialize, Serialize};
use std::fmt;
use utoipa::ToSchema;

use super::CartResponse;
use crate::{CartError, CartResult};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, ToSchema, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum CartStatus {
    Active,
    CheckingOut,
    Completed,
    Abandoned,
}

impl CartStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::CheckingOut => "checking_out",
            Self::Completed => "completed",
            Self::Abandoned => "abandoned",
        }
    }

    pub const fn parse(value: &str) -> Option<Self> {
        match value {
            "active" => Some(Self::Active),
            "checking_out" => Some(Self::CheckingOut),
            "completed" => Some(Self::Completed),
            "abandoned" => Some(Self::Abandoned),
            _ => None,
        }
    }
}

impl AsRef<str> for CartStatus {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for CartStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl PartialEq<&str> for CartStatus {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl CartResponse {
    /// Returns the typed lifecycle state while the persisted and transport field
    /// remains string-compatible during the incremental boundary migration.
    pub fn lifecycle_status(&self) -> CartResult<CartStatus> {
        CartStatus::parse(self.status.as_str()).ok_or_else(|| {
            CartError::Validation(format!(
                "cart {} contains unsupported lifecycle status `{}`",
                self.id, self.status
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::CartStatus;

    #[test]
    fn cart_status_round_trips_through_storage_and_json_names() {
        for status in [
            CartStatus::Active,
            CartStatus::CheckingOut,
            CartStatus::Completed,
            CartStatus::Abandoned,
        ] {
            assert_eq!(CartStatus::parse(status.as_str()), Some(status));
            assert_eq!(
                serde_json::to_string(&status).expect("status should serialize"),
                format!("\"{}\"", status.as_str())
            );
        }
        assert_eq!(CartStatus::parse("unknown"), None);
    }
}
