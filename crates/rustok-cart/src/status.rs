use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::CartResponse;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CartStatusKind {
    Active,
    CheckingOut,
    Completed,
    Abandoned,
    Unknown,
}

impl CartStatusKind {
    pub fn from_raw(status: &str) -> Self {
        match status {
            "active" => Self::Active,
            "checking_out" => Self::CheckingOut,
            "completed" => Self::Completed,
            "abandoned" => Self::Abandoned,
            _ => Self::Unknown,
        }
    }

    pub const fn can_begin_checkout(self) -> bool {
        matches!(self, Self::Active)
    }

    pub const fn can_complete_checkout(self) -> bool {
        matches!(self, Self::CheckingOut)
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Abandoned)
    }
}

impl CartResponse {
    pub fn status_kind(&self) -> CartStatusKind {
        CartStatusKind::from_raw(self.status.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cart_status_transitions_are_typed() {
        assert!(CartStatusKind::Active.can_begin_checkout());
        assert!(CartStatusKind::CheckingOut.can_complete_checkout());
        assert!(CartStatusKind::Completed.is_terminal());
        assert!(CartStatusKind::Abandoned.is_terminal());
    }

    #[test]
    fn unknown_cart_value_fails_closed() {
        assert_eq!(
            CartStatusKind::from_raw("legacy_custom"),
            CartStatusKind::Unknown
        );
    }
}
