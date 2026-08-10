#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{GraphqlHttpError, GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Mutex, OnceLock};

use crate::model::{
    ProductAttributeValueItem, ProductAttributeValuePatchDraft,
};
use crate::schema_retry_identity::{
    ProductAdminSchemaOperation, ProductAdminSchemaRetryIdentity,
};

pub type ApiError = GraphqlHttpError;

type RetryIdentity = ProductAdminSchemaRetryIdentity<String>;

const SAVE_ATTRIBUTE_VALUES_MUTATION: &str = "mutation ProductAdminSaveAttributeValues($idempotencyKey: String!, $productId: UUID!, $locale: String!, $patches: [ProductAttributeValuePatchInput!]!) { saveProductAttributeValues(idempotencyKey: $idempotencyKey, productId: $productId, locale: $locale, patches: $patches) { attributeId kind text integer decimal boolean date datetime optionId optionIds json detached } }";
const CLEAR_DETACHED_ATTRIBUTE_VALUES_MUTATION: &str = "mutation ProductAdminClearDetachedAttributeValues($idempotencyKey: String!, $productId: UUID!, $locale: String!, $attributeIds: [UUID!]!) { clearDetachedProductAttributeValues(idempotencyKey: $idempotencyKey, productId: $productId, locale: $locale, attributeIds: $attributeIds) { attributeId kind text integer decimal boolean date datetime optionId optionIds json detached } }";

#[derive(Debug, Deserialize)]
struct SaveAttributeValuesResponse {
    #[serde(rename = "saveProductAttributeValues")]
    save_product_attribute_values: Vec<ProductAttributeValueItem>,
}

#[derive(Debug, Deserialize)]
struct ClearDetachedAttributeValuesResponse {
    #[serde(rename = "clearDetachedProductAttributeValues")]
    clear_detached_product_attribute_values: Vec<ProductAttributeValueItem>,
}

#[derive(Debug, Serialize)]
struct SaveAttributeValuesVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    #[serde(rename = "productId")]
    product_id: String,
    locale: String,
    patches: Vec<ProductAttributeValuePatchDraft>,
}

#[derive(Debug, Serialize)]
struct ClearDetachedAttributeValuesVariables {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    #[serde(rename = "productId")]
    product_id: String,
    locale: String,
    #[serde(rename = "attributeIds")]
    attribute_ids: Vec<String>,
}

fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }

    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}/api/graphql")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}/api/graphql")
    }
}

async fn request<V, T>(
    query: &str,
    variables: Option<V>,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, variables),
        token,
        tenant_slug,
        None,
    )
    .await
}

fn retry_registry() -> &'static Mutex<HashMap<String, RetryIdentity>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, RetryIdentity>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn retry_slot(
    operation: ProductAdminSchemaOperation,
    tenant_id: &str,
    actor_id: &str,
    target: &str,
) -> String {
    format!(
        "{}:{}:{}:{}",
        operation.key_segment(),
        tenant_id,
        actor_id,
        target
    )
}

fn write_intent<T: Debug>(
    operation: ProductAdminSchemaOperation,
    tenant_id: &str,
    actor_id: &str,
    payload: &T,
) -> String {
    format!(
        "operation={};tenant={tenant_id:?};actor={actor_id:?};payload={payload:?}",
        operation.key_segment()
    )
}

fn retained_caller_key(
    slot: &str,
    operation: ProductAdminSchemaOperation,
    intent: String,
) -> String {
    let mut registry = retry_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    registry
        .entry(slot.to_string())
        .or_default()
        .idempotency_key_for(operation, &intent)
}

fn mark_succeeded(slot: &str) {
    let mut registry = retry_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(identity) = registry.get_mut(slot) {
        identity.mark_succeeded();
    }
    registry.remove(slot);
}

pub(crate) async fn save_product_attribute_values(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    product_id: String,
    locale: String,
    mut patches: Vec<ProductAttributeValuePatchDraft>,
) -> Result<Vec<ProductAttributeValueItem>, ApiError> {
    for patch in &mut patches {
        patch.kind = patch.kind.trim().to_ascii_uppercase();
    }
    let operation = ProductAdminSchemaOperation::SaveAttributeValues;
    let slot = retry_slot(operation, &tenant_id, &user_id, product_id.as_str());
    let intent = write_intent(
        operation,
        &tenant_id,
        &user_id,
        &(product_id.as_str(), locale.as_str(), &patches),
    );
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result: Result<SaveAttributeValuesResponse, ApiError> = request(
        SAVE_ATTRIBUTE_VALUES_MUTATION,
        Some(SaveAttributeValuesVariables {
            idempotency_key,
            product_id,
            locale,
            patches,
        }),
        token,
        tenant_slug,
    )
    .await;
    if result.is_ok() {
        mark_succeeded(&slot);
    }
    result.map(|response| response.save_product_attribute_values)
}

pub(crate) async fn clear_detached_product_attribute_values(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    product_id: String,
    locale: String,
    attribute_ids: Vec<String>,
) -> Result<Vec<ProductAttributeValueItem>, ApiError> {
    let operation = ProductAdminSchemaOperation::ClearDetachedAttributeValues;
    let slot = retry_slot(operation, &tenant_id, &user_id, product_id.as_str());
    let intent = write_intent(
        operation,
        &tenant_id,
        &user_id,
        &(product_id.as_str(), locale.as_str(), &attribute_ids),
    );
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result: Result<ClearDetachedAttributeValuesResponse, ApiError> = request(
        CLEAR_DETACHED_ATTRIBUTE_VALUES_MUTATION,
        Some(ClearDetachedAttributeValuesVariables {
            idempotency_key,
            product_id,
            locale,
            attribute_ids,
        }),
        token,
        tenant_slug,
    )
    .await;
    if result.is_ok() {
        mark_succeeded(&slot);
    }
    result.map(|response| response.clear_detached_product_attribute_values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_query_strings_contain_expected_operations() {
        assert!(SAVE_ATTRIBUTE_VALUES_MUTATION.contains("saveProductAttributeValues"));
        assert!(CLEAR_DETACHED_ATTRIBUTE_VALUES_MUTATION.contains("clearDetachedProductAttributeValues"));
    }
}
