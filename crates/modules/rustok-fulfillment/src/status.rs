use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::FulfillmentResponse;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FulfillmentStatusKind {
    Pending,
    Shipped,
    Delivered,
    Cancelled,
    Unknown,
}

impl FulfillmentStatusKind {
    pub fn from_raw(status: &str) -> Self {
        match status.trim() {
            s if s.eq_ignore_ascii_case("pending") => Self::Pending,
            s if s.eq_ignore_ascii_case("shipped") => Self::Shipped,
            s if s.eq_ignore_ascii_case("delivered") => Self::Delivered,
            s if s.eq_ignore_ascii_case("cancelled") => Self::Cancelled,
            _ => Self::Unknown,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Shipped => "shipped",
            Self::Delivered => "delivered",
            Self::Cancelled => "cancelled",
            Self::Unknown => "unknown",
        }
    }

    pub const fn can_ship(self) -> bool {
        matches!(self, Self::Pending | Self::Shipped)
    }

    pub const fn can_deliver(self) -> bool {
        matches!(self, Self::Shipped)
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Delivered | Self::Cancelled)
    }
}

impl FulfillmentResponse {
    pub fn status_kind(&self) -> FulfillmentStatusKind {
        FulfillmentStatusKind::from_raw(self.status.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fulfillment_status_transitions_are_typed() {
        assert!(FulfillmentStatusKind::Pending.can_ship());
        assert!(FulfillmentStatusKind::Shipped.can_ship());
        assert!(FulfillmentStatusKind::Shipped.can_deliver());
        assert!(FulfillmentStatusKind::Delivered.is_terminal());
        assert!(FulfillmentStatusKind::Cancelled.is_terminal());
    }

    #[test]
    fn fulfillment_status_from_raw_handles_case_and_whitespace() {
        assert_eq!(
            FulfillmentStatusKind::from_raw("  PENDING  "),
            FulfillmentStatusKind::Pending
        );
        assert_eq!(
            FulfillmentStatusKind::from_raw("Shipped"),
            FulfillmentStatusKind::Shipped
        );
        assert_eq!(
            FulfillmentStatusKind::from_raw("DELIVERED"),
            FulfillmentStatusKind::Delivered
        );
        assert_eq!(
            FulfillmentStatusKind::from_raw(" cancelled "),
            FulfillmentStatusKind::Cancelled
        );
    }

    #[test]
    fn fulfillment_status_as_str_round_trip() {
        for kind in [
            FulfillmentStatusKind::Pending,
            FulfillmentStatusKind::Shipped,
            FulfillmentStatusKind::Delivered,
            FulfillmentStatusKind::Cancelled,
        ] {
            assert_eq!(FulfillmentStatusKind::from_raw(kind.as_str()), kind);
        }
        assert_eq!(FulfillmentStatusKind::Unknown.as_str(), "unknown");
    }

    #[test]
    fn unknown_fulfillment_value_fails_closed() {
        assert_eq!(
            FulfillmentStatusKind::from_raw("carrier_custom"),
            FulfillmentStatusKind::Unknown
        );
        assert_eq!(
            FulfillmentStatusKind::from_raw("   "),
            FulfillmentStatusKind::Unknown
        );
    }
}
