#[test]
fn exposes_store_and_admin_route_groups() {
    let store_routes = include_str!("../src/controllers/store/mod.rs");
    let admin_routes = include_str!("../src/controllers/admin/mod.rs");

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
        "/store/orders/{id}/returns",
        "/store/orders/{id}/refunds",
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
        "/admin/orders/{id}/returns",
        "/admin/orders/{id}/returns/decision",
        "/admin/payment-collections",
        "/admin/payment-collections/{id}",
        "/admin/payment-collections/{id}/authorize",
        "/admin/payment-collections/{id}/capture",
        "/admin/payment-collections/{id}/cancel",
        "/admin/payment-collections/{id}/refunds",
        "/admin/returns",
        "/admin/refunds",
        "/admin/refunds/{id}",
        "/admin/refunds/{id}/complete",
        "/admin/refunds/{id}/cancel",
        "/admin/fulfillments",
        "/admin/fulfillments/{id}",
        "/admin/fulfillments/{id}/ship",
        "/admin/fulfillments/{id}/deliver",
        "/admin/fulfillments/{id}/cancel",
    ] {
        let (source, route) = if let Some(route) = expected.strip_prefix("/store") {
            (store_routes, route)
        } else {
            (
                admin_routes,
                expected.strip_prefix("/admin").unwrap_or(expected),
            )
        };
        assert!(
            source.contains(&format!("\"{route}\"")),
            "expected route `{expected}` to be registered"
        );
    }
}
