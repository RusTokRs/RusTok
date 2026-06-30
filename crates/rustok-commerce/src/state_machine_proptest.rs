//! Property-Based Tests for Order State Machine
//!
//! These tests use proptest to verify order state machine invariants across
//! randomly generated inputs, ensuring robustness and catching edge cases.
//!
//! Properties tested:
//! - ID preservation across all valid transitions
//! - Tenant and customer isolation
//! - Monetary value preservation
//! - State transition validity
//! - Error conditions

#[cfg(test)]
mod tests {
    use crate::state_machine::{Order, OrderError, Pending};
    use proptest::prelude::*;
    use rust_decimal::Decimal;
    use uuid::Uuid;

    // ============================================================================
    // Strategy Definitions
    // ============================================================================

    /// Generate random UUIDs
    fn uuid_strategy() -> impl Strategy<Value = Uuid> {
        prop::array::uniform16(0u8..=255).prop_map(Uuid::from_bytes)
    }

    /// Generate valid currency codes
    fn currency_strategy() -> impl Strategy<Value = String> {
        prop::sample::select(vec!["USD", "EUR", "GBP", "JPY", "CAD", "AUD"]).prop_map(String::from)
    }

    /// Generate valid payment methods
    fn payment_method_strategy() -> impl Strategy<Value = String> {
        prop::sample::select(vec!["credit_card", "paypal", "bank_transfer", "crypto"])
            .prop_map(String::from)
    }

    /// Generate valid order amounts (positive, up to $100,000)
    fn amount_strategy() -> impl Strategy<Value = Decimal> {
        (1i64..=10_000_000i64).prop_map(|cents| Decimal::new(cents, 2))
    }

    /// Generate valid tracking numbers
    fn tracking_number_strategy() -> impl Strategy<Value = String> {
        prop::sample::select(vec![
            "TRACK123",
            "1Z999AA10123456784",
            "9400111899223456789012",
        ])
        .prop_map(String::from)
    }

    /// Generate carriers
    fn carrier_strategy() -> impl Strategy<Value = String> {
        prop::sample::select(vec!["FedEx", "UPS", "USPS", "DHL", "Amazon Logistics"])
            .prop_map(String::from)
    }

    /// Generate cancellation reasons
    fn cancel_reason_strategy() -> impl Strategy<Value = String> {
        prop::sample::select(vec![
            "Customer request",
            "Out of stock",
            "Payment failed",
            "Fraud suspected",
            "Shipping unavailable",
        ])
        .prop_map(String::from)
    }

    /// Generate pending orders
    fn pending_order_strategy() -> impl Strategy<Value = Order<Pending>> {
        (
            uuid_strategy(),
            uuid_strategy(),
            uuid_strategy(),
            amount_strategy(),
            currency_strategy(),
        )
            .prop_map(|(id, tenant_id, customer_id, amount, currency)| {
                Order::new_pending(id, tenant_id, customer_id, amount, currency)
            })
    }

    /// Generate valid payment IDs
    fn payment_id_strategy() -> impl Strategy<Value = String> {
        "pay_[a-z0-9]{8,16}".prop_map(String::from)
    }

    /// Generate refund IDs
    fn refund_id_strategy() -> impl Strategy<Value = String> {
        "ref_[a-z0-9]{8,16}".prop_map(String::from)
    }

    // ============================================================================
    // Property Tests: ID Preservation
    // ============================================================================

    proptest! {
        /// Property: Order ID is preserved through Pending -> Confirmed transition
        #[test]
        fn id_preserved_pending_to_confirmed(
            order in pending_order_strategy()
        ) {
            let original_id = order.id;
            let confirmed = order.confirm().unwrap();
            prop_assert_eq!(confirmed.id, original_id);
        }

        /// Property: Order ID is preserved through full happy path
        #[test]
        fn id_preserved_full_happy_path(
            (order, payment_id, payment_method, tracking, carrier) in (
                pending_order_strategy(),
                payment_id_strategy(),
                payment_method_strategy(),
                tracking_number_strategy(),
                carrier_strategy()
            )
        ) {
            let original_id = order.id;

            let delivered = order
                .confirm().unwrap()
                .pay(payment_id, payment_method).unwrap()
                .ship(tracking, carrier).unwrap()
                .deliver(None);

            prop_assert_eq!(delivered.id, original_id);
        }
    }

    // ============================================================================
    // Property Tests: Tenant and Customer Isolation
    // ============================================================================

    proptest! {
        /// Property: Tenant ID never changes across any transition
        #[test]
        fn tenant_id_preserved_all_transitions(
            (order, payment_id, payment_method) in (
                pending_order_strategy(),
                payment_id_strategy(),
                payment_method_strategy()
            )
        ) {
            let original_tenant_id = order.tenant_id;

            let confirmed = order.confirm().unwrap();
            prop_assert_eq!(confirmed.tenant_id, original_tenant_id);

            let paid = confirmed.pay(payment_id, payment_method).unwrap();
            prop_assert_eq!(paid.tenant_id, original_tenant_id);
        }

        /// Property: Customer ID never changes across any transition
        #[test]
        fn customer_id_preserved_all_transitions(
            order in pending_order_strategy()
        ) {
            let original_customer_id = order.customer_id;

            let cancelled = order.cancel("Test".to_string());
            prop_assert_eq!(cancelled.customer_id, original_customer_id);
        }
    }

    // ============================================================================
    // Property Tests: Monetary Value Preservation
    // ============================================================================

    proptest! {
        /// Property: Total amount is preserved through all transitions
        #[test]
        fn total_amount_preserved_all_transitions(
            (order, payment_id, payment_method) in (
                pending_order_strategy(),
                payment_id_strategy(),
                payment_method_strategy()
            )
        ) {
            let original_amount = order.total_amount;

            let paid = order
                .confirm().unwrap()
                .pay(payment_id, payment_method).unwrap();

            prop_assert_eq!(paid.total_amount, original_amount);
        }

        /// Property: Currency is preserved through all transitions
        #[test]
        fn currency_preserved_all_transitions(
            order in pending_order_strategy()
        ) {
            let original_currency = order.currency.clone();

            let cancelled = order.cancel("Test".to_string());
            prop_assert_eq!(cancelled.currency, original_currency);
        }
    }

    // ============================================================================
    // Property Tests: State Transitions
    // ============================================================================

    proptest! {
        /// Property: Confirmed state has valid confirmed_at timestamp
        #[test]
        fn confirmed_has_valid_timestamp(
            order in pending_order_strategy()
        ) {
            let before_confirm = chrono::Utc::now();
            let confirmed = order.confirm().unwrap();
            let after_confirm = chrono::Utc::now();

            prop_assert!(
                confirmed.state.confirmed_at >= before_confirm
                    && confirmed.state.confirmed_at <= after_confirm
            );
        }

        /// Property: Paid state stores payment information correctly
        #[test]
        fn paid_stores_payment_info(
            (order, payment_id, payment_method) in (
                pending_order_strategy(),
                payment_id_strategy(),
                payment_method_strategy()
            )
        ) {
            let paid = order
                .confirm().unwrap()
                .pay(payment_id.clone(), payment_method.clone()).unwrap();

            prop_assert_eq!(paid.state.payment_id, payment_id);
            prop_assert_eq!(paid.state.payment_method, payment_method);
        }

        /// Property: Shipped state stores tracking info correctly
        #[test]
        fn shipped_stores_tracking_info(
            (order, payment_id, payment_method, tracking, carrier) in (
                pending_order_strategy(),
                payment_id_strategy(),
                payment_method_strategy(),
                tracking_number_strategy(),
                carrier_strategy()
            )
        ) {
            let shipped = order
                .confirm().unwrap()
                .pay(payment_id, payment_method).unwrap()
                .ship(tracking.clone(), carrier.clone()).unwrap();

            prop_assert_eq!(shipped.state.tracking_number, tracking);
            prop_assert_eq!(shipped.state.carrier, carrier);
        }
    }

    // ============================================================================
    // Property Tests: Cancellation
    // ============================================================================

    proptest! {
        /// Property: Pending order cancellation stores reason correctly
        #[test]
        fn pending_cancellation_stores_reason(
            (order, reason) in (pending_order_strategy(), cancel_reason_strategy())
        ) {
            let cancelled = order.cancel(reason.clone());
            prop_assert_eq!(cancelled.state.reason, reason);
            prop_assert!(!cancelled.state.refunded);
        }

        /// Property: Paid order cancellation marks as refunded
        #[test]
        fn paid_cancellation_marks_refunded(
            (order, payment_id, payment_method, reason, refund_id) in (
                pending_order_strategy(),
                payment_id_strategy(),
                payment_method_strategy(),
                cancel_reason_strategy(),
                refund_id_strategy()
            )
        ) {
            let cancelled = order
                .confirm().unwrap()
                .pay(payment_id, payment_method).unwrap()
                .cancel_with_refund(reason.clone(), refund_id);

            prop_assert!(cancelled.state.refunded);
            prop_assert_eq!(cancelled.state.reason, reason);
        }
    }

    // ============================================================================
    // Property Tests: Error Conditions
    // ============================================================================

    proptest! {
        /// Property: Empty payment ID fails
        #[test]
        fn empty_payment_id_fails(
            order in pending_order_strategy()
        ) {
            let confirmed = order.confirm().unwrap();
            let result = confirmed.pay(String::new(), "credit_card".to_string());
            prop_assert!(result.is_err());
        }

        /// Property: Empty tracking number fails
        #[test]
        fn empty_tracking_number_fails(
            (order, payment_id, payment_method) in (
                pending_order_strategy(),
                payment_id_strategy(),
                payment_method_strategy()
            )
        ) {
            let paid = order
                .confirm().unwrap()
                .pay(payment_id, payment_method).unwrap();

            let result = paid.ship(String::new(), "FedEx".to_string());
            prop_assert!(result.is_err());
            prop_assert!(matches!(result.unwrap_err(), OrderError::InvalidTrackingNumber));
        }
    }

    // ============================================================================
    // Property Tests: Common Methods
    // ============================================================================

    proptest! {
        /// Property: id() method returns correct ID
        #[test]
        fn id_method_returns_correct_id(
            order in pending_order_strategy()
        ) {
            prop_assert_eq!(order.id(), order.id);
        }

        /// Property: tenant_id() method returns correct tenant ID
        #[test]
        fn tenant_id_method_returns_correct_id(
            order in pending_order_strategy()
        ) {
            prop_assert_eq!(order.tenant_id(), order.tenant_id);
        }

        /// Property: customer_id() method returns correct customer ID
        #[test]
        fn customer_id_method_returns_correct_id(
            order in pending_order_strategy()
        ) {
            prop_assert_eq!(order.customer_id(), order.customer_id);
        }

        /// Property: total_amount() method returns correct amount
        #[test]
        fn total_amount_method_returns_correct_value(
            order in pending_order_strategy()
        ) {
            prop_assert_eq!(order.total_amount(), order.total_amount);
        }
    }

    // ============================================================================
    // Property Tests: Shipped Order Operations
    // ============================================================================

    proptest! {
        /// Property: tracking_info returns correct tracking data
        #[test]
        fn tracking_info_returns_correct_data(
            (order, payment_id, payment_method, tracking, carrier) in (
                pending_order_strategy(),
                payment_id_strategy(),
                payment_method_strategy(),
                tracking_number_strategy(),
                carrier_strategy()
            )
        ) {
            let shipped = order
                .confirm().unwrap()
                .pay(payment_id, payment_method).unwrap()
                .ship(tracking.clone(), carrier.clone()).unwrap();

            let (retrieved_tracking, retrieved_carrier) = shipped.tracking_info();
            prop_assert_eq!(retrieved_tracking, tracking);
            prop_assert_eq!(retrieved_carrier, carrier);
        }
    }

    // ============================================================================
    // Property Tests: Edge Cases
    // ============================================================================

    proptest! {
        /// Property: Very small amounts are handled correctly
        #[test]
        fn small_amounts_handled_correctly(
            (id, tenant_id, customer_id, currency) in (
                uuid_strategy(),
                uuid_strategy(),
                uuid_strategy(),
                currency_strategy()
            )
        ) {
            let small_amount = Decimal::new(1, 2); // $0.01
            let order = Order::new_pending(id, tenant_id, customer_id, small_amount, currency);
            prop_assert_eq!(order.total_amount, small_amount);
        }

        /// Property: Large amounts are handled correctly
        #[test]
        fn large_amounts_handled_correctly(
            (id, tenant_id, customer_id, currency) in (
                uuid_strategy(),
                uuid_strategy(),
                uuid_strategy(),
                currency_strategy()
            )
        ) {
            let large_amount = Decimal::new(99_999_999_999_i64, 2); // $999,999,999.99
            let order = Order::new_pending(id, tenant_id, customer_id, large_amount, currency.clone());
            prop_assert_eq!(order.total_amount, large_amount);
            prop_assert_eq!(order.currency, currency);
        }

        /// Property: Delivered order accepts signature
        #[test]
        fn delivered_accepts_signature(
            (order, payment_id, payment_method, tracking, carrier) in (
                pending_order_strategy(),
                payment_id_strategy(),
                payment_method_strategy(),
                tracking_number_strategy(),
                carrier_strategy()
            )
        ) {
            let delivered = order
                .confirm().unwrap()
                .pay(payment_id, payment_method).unwrap()
                .ship(tracking, carrier).unwrap()
                .deliver(Some("John Doe".to_string()));

            prop_assert_eq!(delivered.state.signature, Some("John Doe".to_string()));
        }

        /// Property: Delivered order works without signature
        #[test]
        fn delivered_works_without_signature(
            (order, payment_id, payment_method, tracking, carrier) in (
                pending_order_strategy(),
                payment_id_strategy(),
                payment_method_strategy(),
                tracking_number_strategy(),
                carrier_strategy()
            )
        ) {
            let delivered = order
                .confirm().unwrap()
                .pay(payment_id, payment_method).unwrap()
                .ship(tracking, carrier).unwrap()
                .deliver(None);

            prop_assert_eq!(delivered.state.signature, None);
        }
    }
}
