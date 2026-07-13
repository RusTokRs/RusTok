use async_graphql::MergedObject;

pub mod cart;
pub mod catalog;
pub mod checkout;
pub mod fulfillment;
pub mod helpers;
pub mod pricing;
pub mod provider_operations;

#[derive(MergedObject, Default)]
pub struct CommerceMutation(
    pub cart::CommerceCartMutation,
    pub catalog::CommerceCatalogMutation,
    pub checkout::CommerceCheckoutMutation,
    pub fulfillment::CommerceFulfillmentMutation,
    pub pricing::CommercePricingMutation,
    pub provider_operations::CommerceProviderMutation,
);
