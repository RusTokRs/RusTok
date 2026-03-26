use rustok_commerce::controllers;

#[test]
fn exposes_legacy_and_medusa_style_route_groups() {
    let routes = controllers::routes();
    let uris = routes
        .handlers
        .iter()
        .map(|handler| handler.uri.as_str())
        .collect::<Vec<_>>();

    for expected in [
        "/api/commerce/products",
        "/api/commerce/products/{id}",
        "/api/commerce/variants/{id}/inventory",
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
    ] {
        assert!(
            uris.contains(&expected),
            "expected route `{expected}` to be registered, got {:?}",
            uris
        );
    }
}
