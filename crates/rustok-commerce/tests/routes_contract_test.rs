use rustok_commerce::controllers;

#[test]
fn exposes_store_and_admin_route_groups() {
    let routes = controllers::routes();
    let uris = routes
        .handlers
        .iter()
        .map(|handler| handler.uri.as_str())
        .collect::<Vec<_>>();

    for expected in [
        "/store/products",
        "/store/products/{id}",
        "/store/regions",
        "/store/shipping-options",
        "/store/carts",
        "/store/carts/{id}",
        "/store/carts/{id}/line-items",
        "/store/carts/{id}/line-items/{line_id}",
        "/store/carts/{id}/complete",
        "/store/payment-collections",
        "/store/orders/{id}",
        "/store/customers/me",
        "/admin/products",
        "/admin/products/{id}",
        "/admin/products/{id}/publish",
        "/admin/products/{id}/unpublish",
        "/admin/orders",
        "/admin/orders/{id}",
        "/admin/orders/{id}/mark-paid",
        "/admin/orders/{id}/ship",
        "/admin/orders/{id}/deliver",
        "/admin/orders/{id}/cancel",
        "/admin/payment-collections",
        "/admin/payment-collections/{id}",
        "/admin/payment-collections/{id}/authorize",
        "/admin/payment-collections/{id}/capture",
        "/admin/payment-collections/{id}/cancel",
        "/admin/fulfillments",
        "/admin/fulfillments/{id}",
        "/admin/fulfillments/{id}/ship",
        "/admin/fulfillments/{id}/deliver",
        "/admin/fulfillments/{id}/cancel",
    ] {
        assert!(
            uris.contains(&expected),
            "expected route `{expected}` to be registered, got {:?}",
            uris
        );
    }
}
