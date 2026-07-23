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
        match status {
            "pending" => Self::Pending,
            "shipped" => Self::Shipped,
            "delivered" => Self::Delivered,
            "cancelled" => Self::Cancelled,
            _ => Self::Unknown,
        }
    }

    pub const fn can_ship(self) -> bool {
        matches!(self, Self::Pending)
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
        assert!(FulfillmentStatusKind::Shipped.can_deliver());
        assert!(FulfillmentStatusKind::Delivered.is_terminal());
        assert!(FulfillmentStatusKind::Cancelled.is_terminal());
    }

    #[test]
    fn unknown_fulfillment_value_fails_closed() {
        assert_eq!(
            FulfillmentStatusKind::from_raw("carrier_custom"),
            FulfillmentStatusKind::Unknown
        );
    }
}
