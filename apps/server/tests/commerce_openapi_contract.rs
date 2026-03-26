use rustok_server::controllers::swagger::ApiDoc;
use serde_json::Value;
use utoipa::OpenApi;

fn openapi_json() -> Value {
    let spec = ApiDoc::openapi()
        .to_json()
        .expect("OpenAPI spec must serialize to JSON");
    serde_json::from_str(&spec).expect("OpenAPI JSON must parse")
}

fn response_schema_ref(
    spec: &Value,
    path: &str,
    method: &str,
    status: &str,
) -> Option<&str> {
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
}

fn request_schema_ref(spec: &Value, path: &str, method: &str) -> Option<&str> {
    spec.get("paths")?
        .get(path)?
        .get(method)?
        .get("requestBody")?
        .get("content")?
        .get("application/json")?
        .get("schema")?
        .get("$ref")?
        .as_str()
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
fn openapi_preserves_store_cart_request_and_response_shapes() {
    let spec = openapi_json();

    assert_eq!(
        request_schema_ref(&spec, "/store/carts", "post"),
        Some("#/components/schemas/StoreCreateCartInput")
    );
    assert_eq!(
        response_schema_ref(&spec, "/store/carts", "post", "201"),
        Some("#/components/schemas/StoreCartResponse")
    );

    assert_eq!(
        request_schema_ref(&spec, "/store/carts/{id}", "post"),
        Some("#/components/schemas/StoreUpdateCartInput")
    );
    assert_eq!(
        response_schema_ref(&spec, "/store/carts/{id}", "post", "200"),
        Some("#/components/schemas/StoreCartResponse")
    );

    assert_eq!(
        request_schema_ref(&spec, "/store/payment-collections", "post"),
        Some("#/components/schemas/StoreCreatePaymentCollectionInput")
    );
    assert_eq!(
        response_schema_ref(&spec, "/store/payment-collections", "post", "201"),
        Some("#/components/schemas/PaymentCollectionResponse")
    );

    assert_eq!(
        request_schema_ref(&spec, "/store/carts/{id}/complete", "post"),
        Some("#/components/schemas/StoreCompleteCartInput")
    );
    assert_eq!(
        response_schema_ref(&spec, "/store/carts/{id}/complete", "post", "200"),
        Some("#/components/schemas/CompleteCheckoutResponse")
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
    ] {
        assert!(
            schemas.contains_key(schema),
            "OpenAPI components must contain schema `{schema}`"
        );
    }
}
