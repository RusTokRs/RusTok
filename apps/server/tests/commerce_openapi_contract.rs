use rustok_server::controllers::swagger::{ApiDoc, SecurityAddon};
use serde_json::Value;
use utoipa::{Modify, OpenApi};

fn openapi_json() -> Value {
    let mut spec = ApiDoc::openapi();
    SecurityAddon.modify(&mut spec);
    let spec = spec.to_json().expect("OpenAPI spec must serialize to JSON");
    serde_json::from_str(&spec).expect("OpenAPI JSON must parse")
}

fn response_schema_ref(spec: &Value, path: &str, method: &str, status: &str) -> Option<String> {
    spec.get("paths")?
        .get(path)?
        .get(method)?
        .get("responses")?
        .get(status)?
        .get("content")?
        .get("application/json")?
        .get("schema")?
        .get("$ref")?
        .as_str()
        .map(ToOwned::to_owned)
}

fn request_schema_ref(spec: &Value, path: &str, method: &str) -> Option<String> {
    spec.get("paths")?
        .get(path)?
        .get(method)?
        .get("requestBody")?
        .get("content")?
        .get("application/json")?
        .get("schema")?
        .get("$ref")?
        .as_str()
        .map(ToOwned::to_owned)
}

fn has_request_body(spec: &Value, path: &str, method: &str) -> bool {
    spec.get("paths")
        .and_then(|paths| paths.get(path))
        .and_then(|path_item| path_item.get(method))
        .and_then(|operation| operation.get("requestBody"))
        .is_some()
}

#[test]
fn openapi_includes_store_cart_contract_paths() {
    let spec = openapi_json();
    let paths = spec
        .get("paths")
        .and_then(Value::as_object)
        .expect("OpenAPI paths object must exist");

    for path in [
        "/store/carts",
        "/store/carts/{id}",
        "/store/carts/{id}/line-items",
        "/store/carts/{id}/line-items/{line_id}",
        "/store/carts/{id}/complete",
        "/store/payment-collections",
    ] {
        assert!(
            paths.contains_key(path),
            "OpenAPI spec must contain path `{path}`"
        );
    }
}

#[test]
fn openapi_includes_admin_order_detail_contract_path() {
    let spec = openapi_json();
    let paths = spec
        .get("paths")
        .and_then(Value::as_object)
        .expect("OpenAPI paths object must exist");

    assert!(
        paths.contains_key("/admin/orders/{id}"),
        "OpenAPI spec must contain admin order detail path"
    );
    assert!(
        paths.contains_key("/admin/orders"),
        "OpenAPI spec must contain admin orders list path"
    );

    for path in [
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
            paths.contains_key(path),
            "OpenAPI spec must contain admin lifecycle path `{path}`"
        );
    }
}

#[test]
fn openapi_preserves_store_cart_request_and_response_shapes() {
    let spec = openapi_json();

    assert_eq!(
        request_schema_ref(&spec, "/store/carts", "post"),
        Some("#/components/schemas/StoreCreateCartInput".to_string())
    );
    assert_eq!(
        response_schema_ref(&spec, "/store/carts", "post", "201"),
        Some("#/components/schemas/StoreCartResponse".to_string())
    );

    assert!(
        has_request_body(&spec, "/store/carts/{id}", "post"),
        "store cart update endpoint must keep a request body contract"
    );
    assert_eq!(
        response_schema_ref(&spec, "/store/carts/{id}", "post", "200"),
        Some("#/components/schemas/StoreCartResponse".to_string())
    );

    assert_eq!(
        request_schema_ref(&spec, "/store/payment-collections", "post"),
        Some("#/components/schemas/StoreCreatePaymentCollectionInput".to_string())
    );
    assert_eq!(
        response_schema_ref(&spec, "/store/payment-collections", "post", "201"),
        Some("#/components/schemas/PaymentCollectionResponse".to_string())
    );

    assert_eq!(
        request_schema_ref(&spec, "/store/carts/{id}/complete", "post"),
        Some("#/components/schemas/StoreCompleteCartInput".to_string())
    );
    assert_eq!(
        response_schema_ref(&spec, "/store/carts/{id}/complete", "post", "200"),
        Some("#/components/schemas/CompleteCheckoutResponse".to_string())
    );

    assert_eq!(
        response_schema_ref(&spec, "/admin/orders", "get", "200"),
        Some("#/components/schemas/PaginatedResponse_OrderResponse".to_string())
    );
    assert_eq!(
        response_schema_ref(&spec, "/admin/orders/{id}", "get", "200"),
        Some("#/components/schemas/AdminOrderDetailResponse".to_string())
    );
    assert_eq!(
        request_schema_ref(&spec, "/admin/orders/{id}/mark-paid", "post"),
        Some("#/components/schemas/MarkPaidOrderInput".to_string())
    );
    assert_eq!(
        request_schema_ref(&spec, "/admin/orders/{id}/ship", "post"),
        Some("#/components/schemas/ShipOrderInput".to_string())
    );
    assert_eq!(
        request_schema_ref(&spec, "/admin/orders/{id}/deliver", "post"),
        Some("#/components/schemas/DeliverOrderInput".to_string())
    );
    assert_eq!(
        request_schema_ref(&spec, "/admin/orders/{id}/cancel", "post"),
        Some("#/components/schemas/CancelOrderInput".to_string())
    );
    assert_eq!(
        response_schema_ref(&spec, "/admin/payment-collections", "get", "200"),
        Some("#/components/schemas/PaginatedResponse_PaymentCollectionResponse".to_string())
    );
    assert_eq!(
        response_schema_ref(&spec, "/admin/payment-collections/{id}", "get", "200"),
        Some("#/components/schemas/PaymentCollectionResponse".to_string())
    );
    assert_eq!(
        request_schema_ref(&spec, "/admin/payment-collections/{id}/authorize", "post"),
        Some("#/components/schemas/AuthorizePaymentInput".to_string())
    );
    assert_eq!(
        request_schema_ref(&spec, "/admin/payment-collections/{id}/capture", "post"),
        Some("#/components/schemas/CapturePaymentInput".to_string())
    );
    assert_eq!(
        request_schema_ref(&spec, "/admin/payment-collections/{id}/cancel", "post"),
        Some("#/components/schemas/CancelPaymentInput".to_string())
    );
    assert_eq!(
        response_schema_ref(&spec, "/admin/fulfillments", "get", "200"),
        Some("#/components/schemas/PaginatedResponse_FulfillmentResponse".to_string())
    );
    assert_eq!(
        response_schema_ref(&spec, "/admin/fulfillments/{id}", "get", "200"),
        Some("#/components/schemas/FulfillmentResponse".to_string())
    );
    assert_eq!(
        request_schema_ref(&spec, "/admin/fulfillments/{id}/ship", "post"),
        Some("#/components/schemas/ShipFulfillmentInput".to_string())
    );
    assert_eq!(
        request_schema_ref(&spec, "/admin/fulfillments/{id}/deliver", "post"),
        Some("#/components/schemas/DeliverFulfillmentInput".to_string())
    );
    assert_eq!(
        request_schema_ref(&spec, "/admin/fulfillments/{id}/cancel", "post"),
        Some("#/components/schemas/CancelFulfillmentInput".to_string())
    );
}

#[test]
fn openapi_registers_store_cart_related_component_schemas() {
    let spec = openapi_json();
    let schemas = spec
        .get("components")
        .and_then(|components| components.get("schemas"))
        .and_then(Value::as_object)
        .expect("OpenAPI component schemas must exist");

    for schema in [
        "StoreCreateCartInput",
        "StoreUpdateCartInput",
        "StoreCartResponse",
        "StoreCreatePaymentCollectionInput",
        "StoreCompleteCartInput",
        "CartResponse",
        "StoreContextResponse",
        "PaymentCollectionResponse",
        "CompleteCheckoutResponse",
        "AdminOrderDetailResponse",
        "PaginatedResponse_OrderResponse",
        "MarkPaidOrderInput",
        "ShipOrderInput",
        "DeliverOrderInput",
        "CancelOrderInput",
        "AuthorizePaymentInput",
        "CapturePaymentInput",
        "CancelPaymentInput",
        "PaginatedResponse_PaymentCollectionResponse",
        "FulfillmentResponse",
        "ShipFulfillmentInput",
        "DeliverFulfillmentInput",
        "CancelFulfillmentInput",
        "PaginatedResponse_FulfillmentResponse",
    ] {
        assert!(
            schemas.contains_key(schema),
            "OpenAPI components must contain schema `{schema}`"
        );
    }
}
