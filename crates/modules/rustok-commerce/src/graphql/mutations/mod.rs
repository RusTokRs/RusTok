use async_graphql::MergedObject;

#[cfg(feature = "marketplace-financial")]
use super::marketplace_financial::MarketplaceFinancialMutation;

#[path = "safe_cart.rs"]
pub mod cart;
#[allow(dead_code)]
#[path = "safe_helpers.rs"]
mod cart_safe_helpers;
pub mod catalog;
#[path = "safe_checkout.rs"]
pub mod checkout;
pub mod fulfillment;
#[path = "layered_order_helpers.rs"]
pub mod helpers;
#[allow(dead_code)]
#[path = "safe_legacy_helpers.rs"]
mod legacy_helpers;
pub mod pricing;
pub mod provider_operations;
pub mod reconciliation;
#[path = "safe_order_helpers.rs"]
mod safe_order_helpers_impl;
#[path = "shipping_option_read_context.rs"]
mod shipping_option_read_context;
#[path = "typed_line_item_helpers.rs"]
mod typed_line_item_helpers;
#[path = "typed_reprice_helper.rs"]
mod typed_reprice_helper;
#[path = "typed_shipping_enrichment_helper.rs"]
mod typed_shipping_enrichment_helper;
#[path = "typed_shipping_option_helper.rs"]
mod typed_shipping_option_helper;

#[cfg(feature = "marketplace-financial")]
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

#[cfg(not(feature = "marketplace-financial"))]
#[derive(MergedObject, Default)]
pub struct CommerceMutation(
    pub cart::CommerceCartMutation,
    pub catalog::CommerceCatalogMutation,
    pub checkout::CommerceCheckoutMutation,
    pub fulfillment::CommerceFulfillmentMutation,
    pub pricing::CommercePricingMutation,
    pub provider_operations::CommerceProviderMutation,
    pub reconciliation::CommerceReconciliationMutation,
);

#[cfg(all(test, feature = "marketplace-financial"))]
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
