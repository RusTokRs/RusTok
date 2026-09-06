#[test]
fn admin_graphql_queries_keep_catalog_contract_stable() {
    let source = include_str!("../admin/src/transport/graphql_adapter.rs");

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
fn storefront_graphql_transports_keep_owner_handoff_stable() {
    let commerce_source = include_str!("../storefront/src/transport/shared_adapter.rs");
    let payment_source =
        include_str!("../../rustok-payment/storefront/src/transport/graphql_adapter.rs");
    let order_source =
        include_str!("../../rustok-order/storefront/src/transport/graphql_adapter.rs");
    let fulfillment_source =
        include_str!("../../rustok-fulfillment/storefront/src/transport/graphql_adapter.rs");

    for (source, required) in [
        (
            payment_source,
            "query StorefrontPaymentCollection($cartId: UUID!)",
        ),
        (
            payment_source,
            "storefrontPaymentCollection(cartId: $cartId)",
        ),
        (
            payment_source,
            "query StorefrontRefundsSummary($orderId: UUID!",
        ),
        (payment_source, "storefrontRefunds(orderId: $orderId"),
        (
            payment_source,
            "mutation CreateStorefrontPaymentCollection($input: CreateStorefrontPaymentCollectionInput!)",
        ),
        (
            payment_source,
            "createStorefrontPaymentCollection(input: $input)",
        ),
        (
            order_source,
            "mutation CompleteStorefrontCheckout($input: CompleteStorefrontCheckoutInput!)",
        ),
        (order_source, "completeStorefrontCheckout(input: $input)"),
        (
            fulfillment_source,
            "mutation SelectStorefrontShippingOption($cartId: UUID!, $input: UpdateStorefrontCartContextInput!)",
        ),
        (
            fulfillment_source,
            "updateStorefrontCartContext(cartId: $cartId, input: $input)",
        ),
    ] {
        assert!(
            source.contains(required),
            "owner storefront GraphQL transport must keep marker `{required}`"
        );
    }

    for forbidden in [
        "CreateStorefrontPaymentCollection",
        "CompleteStorefrontCheckout",
        "SelectStorefrontShippingOption",
        "StorefrontRefundsSummary",
        "storefrontRefunds(",
        "storefrontProducts(",
        "storefrontProduct(",
        "createProduct(",
        "updateProduct(",
        "shippingProfiles(",
    ] {
        assert!(
            !commerce_source.contains(forbidden),
            "commerce aggregate read transport must stay isolated from owner marker `{forbidden}`"
        );
    }
}

#[test]
fn commerce_graphql_module_keeps_expected_root_fields() {
    let query_source = include_str!("../src/graphql/query.rs");
    let mutation_source = format!(
        "{}\\n{}\\n{}\\n{}\\n{}",
        include_str!("../src/graphql/mutations/cart.rs"),
        include_str!("../src/graphql/mutations/catalog.rs"),
        include_str!("../src/graphql/mutations/checkout.rs"),
        include_str!("../src/graphql/mutations/fulfillment.rs"),
        include_str!("../src/graphql/mutations/pricing.rs"),
    );

    for required in [
        "async fn product(",
        "async fn products(",
        "async fn storefront_cart(",
        "async fn storefront_payment_collection(",
        "async fn storefront_me(",
        "async fn storefront_order(",
        "async fn storefront_refunds(",
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
        "async fn order_change(",
        "async fn order_changes(",
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
        "async fn create_order_change(",
        "async fn create_order_return_decision(",
        "async fn apply_order_change(",
        "async fn cancel_order_change(",
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
