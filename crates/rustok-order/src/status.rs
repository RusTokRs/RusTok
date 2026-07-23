use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{OrderChangeResponse, OrderResponse, OrderReturnResponse};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatusKind {
    Pending,
    Confirmed,
    Paid,
    Shipped,
    Delivered,
    Cancelled,
    Unknown,
}

impl OrderStatusKind {
    pub fn from_raw(status: &str) -> Self {
        match status {
            "pending" => Self::Pending,
            "confirmed" => Self::Confirmed,
            "paid" => Self::Paid,
            "shipped" => Self::Shipped,
            "delivered" => Self::Delivered,
            "cancelled" => Self::Cancelled,
            _ => Self::Unknown,
        }
    }

    pub const fn can_confirm(self) -> bool {
        matches!(self, Self::Pending)
    }

    pub const fn can_mark_paid(self) -> bool {
        matches!(self, Self::Confirmed)
    }

    pub const fn can_ship(self) -> bool {
        matches!(self, Self::Paid)
    }

    pub const fn can_deliver(self) -> bool {
        matches!(self, Self::Shipped)
    }

    pub const fn has_financial_effect(self) -> bool {
        matches!(self, Self::Paid | Self::Shipped | Self::Delivered)
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Delivered | Self::Cancelled)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum OrderChangeStatusKind {
    Pending,
    Applied,
    Cancelled,
    Unknown,
}

impl OrderChangeStatusKind {
    pub fn from_raw(status: &str) -> Self {
        match status {
            "pending" => Self::Pending,
            "applied" => Self::Applied,
            "cancelled" => Self::Cancelled,
            _ => Self::Unknown,
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Applied | Self::Cancelled)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum OrderReturnStatusKind {
    Pending,
    Completed,
    Cancelled,
    Unknown,
}

impl OrderReturnStatusKind {
    pub fn from_raw(status: &str) -> Self {
        match status {
            "pending" => Self::Pending,
            "completed" => Self::Completed,
            "cancelled" => Self::Cancelled,
            _ => Self::Unknown,
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled)
    }
}

impl OrderResponse {
    pub fn status_kind(&self) -> OrderStatusKind {
        OrderStatusKind::from_raw(self.status.as_str())
    }
}

impl OrderChangeResponse {
    pub fn status_kind(&self) -> OrderChangeStatusKind {
        OrderChangeStatusKind::from_raw(self.status.as_str())
    }
}

impl OrderReturnResponse {
    pub fn status_kind(&self) -> OrderReturnStatusKind {
        OrderReturnStatusKind::from_raw(self.status.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_status_transitions_are_typed() {
        assert!(OrderStatusKind::Pending.can_confirm());
        assert!(OrderStatusKind::Confirmed.can_mark_paid());
        assert!(OrderStatusKind::Paid.can_ship());
        assert!(OrderStatusKind::Shipped.can_deliver());
        assert!(OrderStatusKind::Delivered.has_financial_effect());
        assert!(OrderStatusKind::Cancelled.is_terminal());
    }

    #[test]
    fn unknown_order_values_fail_closed() {
        assert_eq!(OrderStatusKind::from_raw("provider_custom"), OrderStatusKind::Unknown);
        assert_eq!(
            OrderChangeStatusKind::from_raw("legacy_custom"),
            OrderChangeStatusKind::Unknown
        );
        assert_eq!(
            OrderReturnStatusKind::from_raw("legacy_custom"),
            OrderReturnStatusKind::Unknown
        );
    }
}
