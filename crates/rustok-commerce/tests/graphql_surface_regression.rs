#[test]
fn admin_graphql_queries_keep_catalog_contract_stable() {
    let source = include_str!("../admin/src/api.rs");

    for required in [
        "query CommerceAdminBootstrap { currentTenant { id slug name } }",
        "query CommerceShippingProfiles($tenantId: UUID!, $filter: ShippingProfilesFilter)",
        "shippingProfiles(tenantId: $tenantId, filter: $filter)",
        "query CommerceShippingProfile($tenantId: UUID!, $id: UUID!)",
        "shippingProfile(tenantId: $tenantId, id: $id)",
        "mutation CommerceCreateShippingProfile($tenantId: UUID!, $input: CreateShippingProfileInput!)",
        "createShippingProfile(tenantId: $tenantId, input: $input)",
        "mutation CommerceUpdateShippingProfile($tenantId: UUID!, $id: UUID!, $input: UpdateShippingProfileInput!)",
        "updateShippingProfile(tenantId: $tenantId, id: $id, input: $input)",
    ] {
        assert!(
            source.contains(required),
            "admin GraphQL surface must keep marker `{required}`"
        );
    }

    for forbidden in [
        "cartId",
        "regionId",
        "countryCode",
        "localeCode",
        "selectedShippingOptionId",
        "paymentCollection",
    ] {
        assert!(
            !source.contains(forbidden),
            "admin aggregate GraphQL queries must stay isolated from store cart snapshot marker `{forbidden}`"
        );
    }
}

#[test]
fn storefront_graphql_queries_keep_read_path_stable() {
    let source = include_str!("../storefront/src/api.rs");

    for required in [
        "query StorefrontCheckoutWorkspace($id: UUID!)",
        "storefrontCart(id: $id)",
        "mutation CreateStorefrontPaymentCollection($input: CreateStorefrontPaymentCollectionInput!)",
        "createStorefrontPaymentCollection(input: $input)",
        "mutation CompleteStorefrontCheckout($input: CompleteStorefrontCheckoutInput!)",
        "completeStorefrontCheckout(input: $input)",
        "mutation SelectStorefrontShippingOption($cartId: UUID!, $input: UpdateStorefrontCartContextInput!)",
        "updateStorefrontCartContext(cartId: $cartId, input: $input)",
    ] {
        assert!(
            source.contains(required),
            "storefront GraphQL surface must keep marker `{required}`"
        );
    }

    for forbidden in [
        "storefrontProducts(",
        "storefrontProduct(",
        "createProduct(",
        "updateProduct(",
        "shippingProfiles(",
    ] {
        assert!(
            !source.contains(forbidden),
            "storefront aggregate checkout GraphQL queries must stay isolated from marker `{forbidden}`"
        );
    }
}

#[test]
fn commerce_graphql_module_keeps_expected_root_fields() {
    let query_source = include_str!("../src/graphql/query.rs");
    let mutation_source = include_str!("../src/graphql/mutation.rs");

    for required in [
        "async fn product(",
        "async fn products(",
        "async fn storefront_cart(",
        "async fn storefront_me(",
        "async fn storefront_order(",
        "async fn admin_pricing_product(",
        "async fn storefront_regions(",
        "async fn storefront_shipping_options(",
        "async fn storefront_pricing_channels(",
        "async fn storefront_active_price_lists(",
        "async fn storefront_pricing_product(",
        "async fn storefront_product(",
        "async fn storefront_products(",
        "async fn payment_collections(",
        "async fn refunds(",
        "async fn fulfillments(",
    ] {
        assert!(
            query_source.contains(required),
            "commerce GraphQL query module must keep root field `{required}`"
        );
    }

    for required in [
        "async fn create_storefront_cart(",
        "async fn update_storefront_cart_context(",
        "async fn add_storefront_cart_line_item(",
        "async fn update_storefront_cart_line_item(",
        "async fn remove_storefront_cart_line_item(",
        "async fn create_storefront_payment_collection(",
        "async fn complete_storefront_checkout(",
        "async fn create_product(",
        "async fn update_product(",
        "async fn create_refund(",
        "async fn complete_refund(",
        "async fn cancel_refund(",
        "async fn publish_product(",
        "async fn delete_product(",
    ] {
        assert!(
            mutation_source.contains(required),
            "commerce GraphQL mutation module must keep root field `{required}`"
        );
    }
}

#[test]
fn commerce_graphql_marks_generic_catalog_prices_as_non_authoritative() {
    let query_source = include_str!("../src/graphql/query.rs");
    let types_source = include_str!("../src/graphql/types.rs");

    for required in [
        "pricing-authoritative reads live under `adminPricingProduct`",
        "pricing-authoritative reads live under `storefrontPricingProduct`",
        "Catalog compatibility snapshot only; use adminPricingProduct/storefrontPricingProduct or rustok-pricing module surfaces for pricing-authoritative reads.",
    ] {
        assert!(
            query_source.contains(required) || types_source.contains(required),
            "commerce GraphQL source must keep semantic-boundary marker `{required}`"
        );
    }
}
