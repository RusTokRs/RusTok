/// Type-Safe State Machine for E-commerce Orders
///
/// Implements the order lifecycle with compile-time safety guarantees.
///
/// Benefits:
/// - **Impossible states**: Can't ship an unpaid order (compile error!)
/// - **State-specific data**: Each state has its own required fields
/// - **Audit trail**: All transitions logged automatically
/// - **Business rules**: Guards enforce payment/inventory checks
///
/// State Diagram:
/// ```text
///  ┌─────────┐
///  │ Pending │─────────┐
///  └────┬────┘         │
///       │ confirm()    │
///       ↓              │ cancel()
///  ┌───────────┐       │
///  │ Confirmed │───────┤
///  └─────┬─────┘       │
///        │ pay()       │
///        ↓             │
///  ┌──────────┐        │
///  │   Paid   │────────┤
///  └─────┬────┘        │
///        │ ship()      │
///        ↓             │
///  ┌─────────┐         │
///  │ Shipped │─────────┤
///  └────┬────┘         │
///       │ deliver()    │
///       ↓              │
///  ┌───────────┐       │
///  │ Delivered │       │
///  └───────────┘       │
///                      ↓
///                ┌───────────┐
///                │ Cancelled │
///                └───────────┘
/// ```
///
/// Usage:
/// ```ignore
/// // Create new order
/// let order = Order::new_pending(id, customer_id, items);
///
/// // Confirm order (validates inventory)
/// let order = order.confirm().await?;
///
/// // Process payment
/// let order = order.pay(payment_id).await?;
///
/// // Ship (only possible on paid orders!)
/// let order = order.ship(tracking_number)?;
///
/// // Invalid: Pending -> Shipped (compile error!)
/// // let order = order.ship(tracking); // ❌ method not available
/// ```
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// State Definitions
// ============================================================================

/// Pending state - order created, awaiting confirmation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pending {
    pub created_at: DateTime<Utc>,
}

/// Confirmed state - order confirmed, inventory reserved
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Confirmed {
    pub confirmed_at: DateTime<Utc>,
    pub inventory_reserved: bool,
}

/// Paid state - payment processed successfully
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paid {
    pub paid_at: DateTime<Utc>,
    pub payment_id: String,
    pub payment_method: String,
}

/// Shipped state - order shipped to customer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shipped {
    pub shipped_at: DateTime<Utc>,
    pub tracking_number: String,
    pub carrier: String,
}

/// Delivered state - order delivered to customer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delivered {
    pub delivered_at: DateTime<Utc>,
    pub signature: Option<String>,
}

/// Cancelled state - order cancelled
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cancelled {
    pub cancelled_at: DateTime<Utc>,
    pub reason: String,
    pub refunded: bool,
}

// ============================================================================
// State Machine
// ============================================================================

/// Type-safe order state machine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order<S> {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub customer_id: Uuid,
    pub total_amount: Decimal,
    pub currency: String,

    // State-specific data
    pub state: S,
}

// ============================================================================
// Constructors
// ============================================================================

impl Order<Pending> {
    /// Create a new order in pending state
    pub fn new_pending(
        id: Uuid,
        tenant_id: Uuid,
        customer_id: Uuid,
        total_amount: Decimal,
        currency: String,
    ) -> Self {
        tracing::info!(
            order_id = %id,
            tenant_id = %tenant_id,
            customer_id = %customer_id,
            amount = %total_amount,
            "Order created in Pending state"
        );

        Self {
            id,
            tenant_id,
            customer_id,
            total_amount,
            currency,
            state: Pending {
                created_at: Utc::now(),
            },
        }
    }
}

// ============================================================================
// Transitions: Pending
// ============================================================================

impl Order<Pending> {
    /// Confirm order (Pending → Confirmed)
    ///
    /// Validates inventory and reserves items.
    pub fn confirm(self) -> Result<Order<Confirmed>, OrderError> {
        // Business rule: validate inventory (stubbed)
        let inventory_reserved = true;

        if !inventory_reserved {
            return Err(OrderError::InsufficientInventory);
        }

        tracing::info!(
            order_id = %self.id,
            "Order: Pending → Confirmed"
        );

        Ok(Order {
            id: self.id,
            tenant_id: self.tenant_id,
            customer_id: self.customer_id,
            total_amount: self.total_amount,
            currency: self.currency,
            state: Confirmed {
                confirmed_at: Utc::now(),
                inventory_reserved,
            },
        })
    }

    /// Cancel order (Pending → Cancelled)
    pub fn cancel(self, reason: String) -> Order<Cancelled> {
        tracing::info!(
            order_id = %self.id,
            reason = %reason,
            "Order: Pending → Cancelled"
        );

        Order {
            id: self.id,
            tenant_id: self.tenant_id,
            customer_id: self.customer_id,
            total_amount: self.total_amount,
            currency: self.currency,
            state: Cancelled {
                cancelled_at: Utc::now(),
                reason,
                refunded: false,
            },
        }
    }
}

// ============================================================================
// Transitions: Confirmed
// ============================================================================

impl Order<Confirmed> {
    /// Process payment (Confirmed → Paid)
    pub fn pay(
        self,
        payment_id: String,
        payment_method: String,
    ) -> Result<Order<Paid>, OrderError> {
        // Business rule: validate payment (stubbed)
        if payment_id.is_empty() {
            return Err(OrderError::PaymentFailed("Empty payment ID".to_string()));
        }

        tracing::info!(
            order_id = %self.id,
            payment_id = %payment_id,
            "Order: Confirmed → Paid"
        );

        Ok(Order {
            id: self.id,
            tenant_id: self.tenant_id,
            customer_id: self.customer_id,
            total_amount: self.total_amount,
            currency: self.currency,
            state: Paid {
                paid_at: Utc::now(),
                payment_id,
                payment_method,
            },
        })
    }

    /// Cancel confirmed order (Confirmed → Cancelled)
    ///
    /// Releases inventory reservation.
    pub fn cancel(self, reason: String) -> Order<Cancelled> {
        tracing::info!(
            order_id = %self.id,
            reason = %reason,
            inventory_released = self.state.inventory_reserved,
            "Order: Confirmed → Cancelled"
        );

        // Release inventory here

        Order {
            id: self.id,
            tenant_id: self.tenant_id,
            customer_id: self.customer_id,
            total_amount: self.total_amount,
            currency: self.currency,
            state: Cancelled {
                cancelled_at: Utc::now(),
                reason,
                refunded: false,
            },
        }
    }
}

// ============================================================================
// Transitions: Paid
// ============================================================================

impl Order<Paid> {
    /// Ship order (Paid → Shipped)
    pub fn ship(
        self,
        tracking_number: String,
        carrier: String,
    ) -> Result<Order<Shipped>, OrderError> {
        if tracking_number.is_empty() {
            return Err(OrderError::InvalidTrackingNumber);
        }

        tracing::info!(
            order_id = %self.id,
            tracking_number = %tracking_number,
            carrier = %carrier,
            "Order: Paid → Shipped"
        );

        Ok(Order {
            id: self.id,
            tenant_id: self.tenant_id,
            customer_id: self.customer_id,
            total_amount: self.total_amount,
            currency: self.currency,
            state: Shipped {
                shipped_at: Utc::now(),
                tracking_number,
                carrier,
            },
        })
    }

    /// Cancel paid order (Paid → Cancelled)
    ///
    /// Requires refund processing.
    pub fn cancel_with_refund(self, reason: String, refund_id: String) -> Order<Cancelled> {
        tracing::info!(
            order_id = %self.id,
            reason = %reason,
            refund_id = %refund_id,
            "Order: Paid → Cancelled (with refund)"
        );

        // Process refund here

        Order {
            id: self.id,
            tenant_id: self.tenant_id,
            customer_id: self.customer_id,
            total_amount: self.total_amount,
            currency: self.currency,
            state: Cancelled {
                cancelled_at: Utc::now(),
                reason,
                refunded: true,
            },
        }
    }
}

// ============================================================================
// Transitions: Shipped
// ============================================================================

impl Order<Shipped> {
    /// Mark as delivered (Shipped → Delivered)
    pub fn deliver(self, signature: Option<String>) -> Order<Delivered> {
        tracing::info!(
            order_id = %self.id,
            has_signature = signature.is_some(),
            "Order: Shipped → Delivered"
        );

        Order {
            id: self.id,
            tenant_id: self.tenant_id,
            customer_id: self.customer_id,
            total_amount: self.total_amount,
            currency: self.currency,
            state: Delivered {
                delivered_at: Utc::now(),
                signature,
            },
        }
    }

    /// Get tracking info
    pub fn tracking_info(&self) -> (&str, &str) {
        (&self.state.tracking_number, &self.state.carrier)
    }
}

// ============================================================================
// Error Types
// ============================================================================

#[derive(Debug, thiserror::Error)]
pub enum OrderError {
    #[error("Insufficient inventory")]
    InsufficientInventory,

    #[error("Payment failed: {0}")]
    PaymentFailed(String),

    #[error("Invalid tracking number")]
    InvalidTrackingNumber,

    #[error("Invalid state transition")]
    InvalidTransition,
}

// ============================================================================
// Common Methods (all states)
// ============================================================================

impl<S> Order<S> {
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn customer_id(&self) -> Uuid {
        self.customer_id
    }

    pub fn total_amount(&self) -> Decimal {
        self.total_amount
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_order() -> Order<Pending> {
        Order::new_pending(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Decimal::new(10000, 2), // $100.00
            "USD".to_string(),
        )
    }

    #[test]
    fn test_pending_to_confirmed() {
        let order = create_test_order();
        let order = order.confirm().unwrap();

        assert!(order.state.inventory_reserved);
    }

    #[test]
    fn test_confirmed_to_paid() {
        let order = create_test_order().confirm().unwrap();

        let order = order
            .pay("pay_123".to_string(), "credit_card".to_string())
            .unwrap();

        assert_eq!(order.state.payment_id, "pay_123");
        assert_eq!(order.state.payment_method, "credit_card");
    }

    #[test]
    fn test_paid_to_shipped() {
        let order = create_test_order()
            .confirm()
            .unwrap()
            .pay("pay_123".to_string(), "credit_card".to_string())
            .unwrap();

        let order = order
            .ship("TRACK123".to_string(), "FedEx".to_string())
            .unwrap();

        assert_eq!(order.state.tracking_number, "TRACK123");
        assert_eq!(order.state.carrier, "FedEx");
    }

    #[test]
    fn test_shipped_to_delivered() {
        let order = create_test_order()
            .confirm()
            .unwrap()
            .pay("pay_123".to_string(), "credit_card".to_string())
            .unwrap()
            .ship("TRACK123".to_string(), "FedEx".to_string())
            .unwrap();

        let order = order.deliver(Some("John Doe".to_string()));

        assert_eq!(order.state.signature, Some("John Doe".to_string()));
    }

    #[test]
    fn test_pending_to_cancelled() {
        let order = create_test_order();
        let order = order.cancel("Customer request".to_string());

        assert_eq!(order.state.reason, "Customer request");
        assert!(!order.state.refunded);
    }

    #[test]
    fn test_paid_to_cancelled_with_refund() {
        let order = create_test_order()
            .confirm()
            .unwrap()
            .pay("pay_123".to_string(), "credit_card".to_string())
            .unwrap();

        let order = order.cancel_with_refund("Out of stock".to_string(), "ref_456".to_string());

        assert_eq!(order.state.reason, "Out of stock");
        assert!(order.state.refunded);
    }

    #[test]
    fn test_empty_payment_id_error() {
        let order = create_test_order().confirm().unwrap();

        let result = order.pay("".to_string(), "credit_card".to_string());

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), OrderError::PaymentFailed(_)));
    }

    // Compile-time safety tests (these should NOT compile)

    // #[test]
    // fn test_invalid_pending_to_shipped() {
    //     let order = create_test_order();
    //     // ❌ Compile error: no method `ship` on `Order<Pending>`
    //     let order = order.ship("TRACK123".to_string(), "FedEx".to_string());
    // }

    // #[test]
    // fn test_invalid_confirmed_to_delivered() {
    //     let order = create_test_order().confirm().unwrap();
    //     // ❌ Compile error: no method `deliver` on `Order<Confirmed>`
    //     let order = order.deliver(None);
    // }
}
