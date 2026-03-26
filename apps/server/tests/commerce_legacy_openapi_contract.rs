use rustok_server::controllers::swagger::{ApiDoc, SecurityAddon};
use serde_json::Value;
use utoipa::{Modify, OpenApi};

fn openapi_json() -> Value {
    let mut spec = ApiDoc::openapi();
    SecurityAddon.modify(&mut spec);
    let spec = spec
        .to_json()
        .expect("OpenAPI spec must serialize to JSON");
    serde_json::from_str(&spec).expect("OpenAPI JSON must parse")
}

fn response_schema_ref(
    spec: &Value,
    path: &str,
    method: &str,
    status: &str,
) -> Option<String> {
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

fn response_array_item_schema_ref(
    spec: &Value,
    path: &str,
    method: &str,
    status: &str,
) -> Option<String> {
    spec.get("paths")?
        .get(path)?
        .get(method)?
        .get("responses")?
        .get(status)?
        .get("content")?
        .get("application/json")?
        .get("schema")?
        .get("items")?
        .get("$ref")?
        .as_str()
        .map(ToOwned::to_owned)
}

#[test]
fn openapi_preserves_legacy_commerce_paths() {
    let spec = openapi_json();
    let paths = spec
        .get("paths")
        .and_then(Value::as_object)
        .expect("OpenAPI paths object must exist");

    for path in [
        "/api/commerce/products",
        "/api/commerce/products/{id}",
        "/api/commerce/products/{id}/publish",
        "/api/commerce/products/{id}/unpublish",
        "/api/commerce/variants/{id}",
        "/api/commerce/variants/{id}/prices",
        "/api/commerce/variants/{id}/inventory",
        "/api/commerce/variants/{id}/inventory/adjust",
        "/api/commerce/variants/{id}/inventory/set",
        "/api/commerce/inventory/check",
    ] {
        assert!(
            paths.contains_key(path),
            "OpenAPI spec must contain legacy commerce path `{path}`"
        );
    }
}

#[test]
fn openapi_preserves_legacy_commerce_request_and_response_shapes() {
    let spec = openapi_json();

    assert_eq!(
        request_schema_ref(&spec, "/api/commerce/products", "post"),
        Some("#/components/schemas/CreateProductInput".to_string())
    );
    assert!(
        response_schema_ref(&spec, "/api/commerce/products", "get", "200").is_some(),
        "legacy product list must keep a JSON response schema"
    );
    assert_eq!(
        response_schema_ref(&spec, "/api/commerce/products/{id}", "get", "200"),
        Some("#/components/schemas/ProductResponse".to_string())
    );

    assert_eq!(
        request_schema_ref(&spec, "/api/commerce/variants/{id}/inventory/adjust", "post"),
        Some("#/components/schemas/AdjustInput".to_string())
    );
    assert_eq!(
        request_schema_ref(&spec, "/api/commerce/variants/{id}/inventory/set", "post"),
        Some("#/components/schemas/SetInventoryInput".to_string())
    );
    assert_eq!(
        request_schema_ref(&spec, "/api/commerce/inventory/check", "post"),
        Some("#/components/schemas/CheckAvailabilityInput".to_string())
    );
    assert_eq!(
        response_array_item_schema_ref(&spec, "/api/commerce/inventory/check", "post", "200"),
        Some("#/components/schemas/AvailabilityResult".to_string())
    );
}
