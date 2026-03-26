#[test]
fn admin_graphql_queries_keep_catalog_contract_stable() {
    let source = include_str!("../admin/src/api.rs");

    for required in [
        "query CommerceProducts($tenantId: UUID!, $locale: String, $filter: ProductsFilter)",
        "products(tenantId: $tenantId, locale: $locale, filter: $filter)",
        "query CommerceProduct($tenantId: UUID!, $id: UUID!, $locale: String)",
        "product(tenantId: $tenantId, id: $id, locale: $locale)",
        "mutation CommerceCreateProduct($tenantId: UUID!, $userId: UUID!, $input: CreateProductInput!)",
        "createProduct(tenantId: $tenantId, userId: $userId, input: $input)",
        "mutation CommerceUpdateProduct($tenantId: UUID!, $userId: UUID!, $id: UUID!, $input: UpdateProductInput!)",
        "updateProduct(tenantId: $tenantId, userId: $userId, id: $id, input: $input)",
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
            "admin catalog GraphQL queries must stay isolated from store cart snapshot marker `{forbidden}`"
        );
    }
}

#[test]
fn storefront_graphql_queries_keep_read_path_stable() {
    let source = include_str!("../storefront/src/api.rs");

    for required in [
        "query StorefrontCommerceProducts($locale: String, $filter: StorefrontProductsFilter)",
        "storefrontProducts(locale: $locale, filter: $filter)",
        "query StorefrontCommerceProduct($locale: String, $handle: String!)",
        "storefrontProduct(locale: $locale, handle: $handle)",
        "items { id status title handle vendor productType createdAt publishedAt }",
        "translations { locale title handle description }",
        "variants { id title sku inventoryQuantity inStock prices { currencyCode amount compareAtAmount onSale } }",
    ] {
        assert!(
            source.contains(required),
            "storefront GraphQL surface must keep marker `{required}`"
        );
    }

    for forbidden in [
        "cartId",
        "regionId",
        "countryCode",
        "localeCode",
        "selectedShippingOptionId",
        "paymentCollection",
        "completeCheckout",
    ] {
        assert!(
            !source.contains(forbidden),
            "storefront read-path GraphQL queries must not depend on store cart snapshot marker `{forbidden}`"
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
        "async fn storefront_product(",
        "async fn storefront_products(",
    ] {
        assert!(
            query_source.contains(required),
            "commerce GraphQL query module must keep root field `{required}`"
        );
    }

    for required in [
        "async fn create_product(",
        "async fn update_product(",
        "async fn publish_product(",
        "async fn delete_product(",
    ] {
        assert!(
            mutation_source.contains(required),
            "commerce GraphQL mutation module must keep root field `{required}`"
        );
    }
}
