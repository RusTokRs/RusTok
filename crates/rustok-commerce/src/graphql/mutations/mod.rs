use async_graphql::MergedObject;

pub mod cart;
pub mod catalog;
pub mod checkout;
pub mod fulfillment;
pub mod helpers;
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
    pub super::marketplace_financial::MarketplaceFinancialMutation,
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
