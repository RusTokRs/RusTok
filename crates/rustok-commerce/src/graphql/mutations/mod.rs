use async_graphql::MergedObject;

use super::marketplace_financial::MarketplaceFinancialMutation;

#[path = "safe_cart.rs"]
pub mod cart;
#[path = "safe_helpers.rs"]
mod cart_safe_helpers;
#[path = "typed_line_item_helpers.rs"]
mod typed_line_item_helpers;
pub mod catalog;
#[path = "safe_checkout.rs"]
pub mod checkout;
pub mod fulfillment;
#[path = "safe_order_helpers.rs"]
mod safe_order_helpers_impl;
#[path = "layered_order_helpers.rs"]
pub mod helpers;
#[path = "safe_legacy_helpers.rs"]
mod legacy_helpers;
pub mod pricing;
pub mod provider_operations;
pub mod reconciliation;

#[derive(MergedObject, Default)]
pub struct CommerceMutation(
    pub cart::CommerceCartMutation,
    pub catalog::CommerceCatalogMutation,
    pub checkout::CommerceCheckoutMutation,
    pub fulfillment::CommerceFulfillmentMutation,
    pub pricing::CommercePricingMutation,
    pub provider_operations::CommerceProviderMutation,
    pub reconciliation::CommerceReconciliationMutation,
    pub MarketplaceFinancialMutation,
);

#[cfg(test)]
mod tests {
    use async_graphql::{EmptySubscription, Schema};

    use super::CommerceMutation;
    use crate::graphql::CommerceQuery;

    #[test]
    fn provider_operations_remain_in_merged_schema() {
        let schema = Schema::build(
            CommerceQuery::default(),
            CommerceMutation::default(),
            EmptySubscription,
        )
        .finish();
        let sdl = schema.sdl();

        for field in [
            "authorizePaymentCollection",
            "capturePaymentCollection",
            "cancelPaymentCollection",
            "createRefund",
            "completeRefund",
            "cancelRefund",
            "retryRefundProvider",
            "createFulfillment",
            "shipFulfillment",
            "deliverFulfillment",
            "reopenFulfillment",
            "reshipFulfillment",
            "cancelFulfillment",
            "retryMarketplaceFinancialOperation",
            "retryMarketplacePaidEvent",
            "runMarketplaceFinancialRecoverySweep",
        ] {
            assert!(
                sdl.contains(field),
                "merged commerce schema must retain mutation field `{field}`"
            );
        }
    }
}
