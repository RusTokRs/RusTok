use async_graphql::MergedObject;

#[derive(MergedObject, Default)]
pub struct MarketplaceSellerCombinedQuery(
    pub crate::graphql::MarketplaceSellerQuery,
    pub crate::graphql_events::MarketplaceSellerEventQuery,
);
