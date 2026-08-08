#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{GraphqlHttpError, GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Mutex, OnceLock};

use crate::model::{
    BindCategoryAttributeDraft, BindSchemaAttributeDraft, CatalogCategoryDraft,
    CategoryAttributeGroupDraft, ProductAttributeDraft, ProductAttributeOptionDraft,
    ProductAttributeSchemaDraft, ProductAttributeSchemaGroupDraft, ProductAttributeValueItem,
    ProductAttributeValuePatchDraft, SetCategorySchemaModeDraft,
};
use crate::schema_retry_identity::{
    ProductAdminSchemaOperation, ProductAdminSchemaRetryIdentity,
};

pub type ApiError = GraphqlHttpError;

type RetryIdentity = ProductAdminSchemaRetryIdentity<String>;

const CREATE_PRODUCT_ATTRIBUTE_MUTATION: &str = "mutation ProductAdminCreateAttribute($idempotencyKey: String!, $locale: String!, $input: CreateProductAttributeInput!) { createProductAttribute(idempotencyKey: $idempotencyKey, locale: $locale, input: $input) }";
const CREATE_PRODUCT_ATTRIBUTE_OPTION_MUTATION: &str = "mutation ProductAdminCreateAttributeOption($idempotencyKey: String!, $locale: String!, $input: CreateProductAttributeOptionInput!) { createProductAttributeOption(idempotencyKey: $idempotencyKey, locale: $locale, input: $input) }";
const CREATE_CATALOG_CATEGORY_MUTATION: &str = "mutation ProductAdminCreateCatalogCategory($idempotencyKey: String!, $locale: String!, $input: CreateCatalogCategoryInput!) { createCatalogCategory(idempotencyKey: $idempotencyKey, locale: $locale, input: $input) }";
const CREATE_ATTRIBUTE_SCHEMA_MUTATION: &str = "mutation ProductAdminCreateAttributeSchema($idempotencyKey: String!, $locale: String!, $input: CreateProductAttributeSchemaInput!) { createProductAttributeSchema(idempotencyKey: $idempotencyKey, locale: $locale, input: $input) }";
const CREATE_SCHEMA_GROUP_MUTATION: &str = "mutation ProductAdminCreateSchemaGroup($idempotencyKey: String!, $locale: String!, $input: CreateProductAttributeSchemaGroupInput!) { createProductAttributeSchemaGroup(idempotencyKey: $idempotencyKey, locale: $locale, input: $input) }";
const CREATE_CATEGORY_GROUP_MUTATION: &str = "mutation ProductAdminCreateCategoryGroup($idempotencyKey: String!, $locale: String!, $input: CreateCategoryAttributeGroupInput!) { createCatalogCategoryAttributeGroup(idempotencyKey: $idempotencyKey, locale: $locale, input: $input) }";
const SET_CATEGORY_SCHEMA_MODE_MUTATION: &str = "mutation ProductAdminSetCategorySchemaMode($idempotencyKey: String!, $input: SetCategorySchemaModeInput!) { setCatalogCategorySchemaMode(idempotencyKey: $idempotencyKey, input: $input) }";
const BIND_SCHEMA_ATTRIBUTE_MUTATION: &str = "mutation ProductAdminBindSchemaAttribute($idempotencyKey: String!, $input: BindSchemaAttributeInput!) { bindProductAttributeSchemaAttribute(idempotencyKey: $idempotencyKey, input: $input) }";
const BIND_CATEGORY_ATTRIBUTE_MUTATION: &str = "mutation ProductAdminBindCategoryAttribute($idempotencyKey: String!, $input: BindCategoryAttributeInput!) { bindCatalogCategoryAttribute(idempotencyKey: $idempotencyKey, input: $input) }";
const SAVE_ATTRIBUTE_VALUES_MUTATION: &str = "mutation ProductAdminSaveAttributeValues($idempotencyKey: String!, $productId: UUID!, $locale: String!, $patches: [ProductAttributeValuePatchInput!]!) { saveProductAttributeValues(idempotencyKey: $idempotencyKey, productId: $productId, locale: $locale, patches: $patches) { attributeId kind text integer decimal boolean date datetime optionId optionIds json detached } }";
const CLEAR_DETACHED_ATTRIBUTE_VALUES_MUTATION: &str = "mutation ProductAdminClearDetachedAttributeValues($idempotencyKey: String!, $productId: UUID!, $locale: String!, $attributeIds: [UUID!]!) { clearDetachedProductAttributeValues(idempotencyKey: $idempotencyKey, productId: $productId, locale: $locale, attributeIds: $attributeIds) { attributeId kind text integer decimal boolean date datetime optionId optionIds json detached } }";

#[derive(Debug, Deserialize)]
struct BoolMutationResponse {
    #[serde(rename = "createProductAttribute")]
    create_product_attribute: Option<bool>,
    #[serde(rename = "createProductAttributeOption")]
    create_product_attribute_option: Option<bool>,
    #[serde(rename = "createCatalogCategory")]
    create_catalog_category: Option<bool>,
    #[serde(rename = "createProductAttributeSchema")]
    create_product_attribute_schema: Option<bool>,
    #[serde(rename = "createProductAttributeSchemaGroup")]
    create_product_attribute_schema_group: Option<bool>,
    #[serde(rename = "createCatalogCategoryAttributeGroup")]
    create_catalog_category_attribute_group: Option<bool>,
    #[serde(rename = "setCatalogCategorySchemaMode")]
    set_catalog_category_schema_mode: Option<bool>,
    #[serde(rename = "bindProductAttributeSchemaAttribute")]
    bind_product_attribute_schema_attribute: Option<bool>,
    #[serde(rename = "bindCatalogCategoryAttribute")]
    bind_catalog_category_attribute: Option<bool>,
}

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
struct LocaleMutationVariables<T> {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    locale: String,
    input: T,
}

#[derive(Debug, Serialize)]
struct InputVariables<T> {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    input: T,
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

pub(crate) async fn create_product_attribute(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    locale: String,
    draft: ProductAttributeDraft,
) -> Result<bool, ApiError> {
    let operation = ProductAdminSchemaOperation::CreateAttribute;
    let slot = retry_slot(operation, &tenant_id, &user_id, draft.code.as_str());
    let intent = write_intent(operation, &tenant_id, &user_id, &(locale.as_str(), &draft));
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result: Result<BoolMutationResponse, ApiError> = request(
        CREATE_PRODUCT_ATTRIBUTE_MUTATION,
        Some(LocaleMutationVariables {
            idempotency_key,
            locale,
            input: draft,
        }),
        token,
        tenant_slug,
    )
    .await;
    if result.is_ok() {
        mark_succeeded(&slot);
    }
    result.map(|response| response.create_product_attribute.unwrap_or(false))
}

pub(crate) async fn create_product_attribute_option(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    locale: String,
    draft: ProductAttributeOptionDraft,
) -> Result<bool, ApiError> {
    let operation = ProductAdminSchemaOperation::CreateAttributeOption;
    let target = format!("{}:{}", draft.attribute_id, draft.code);
    let slot = retry_slot(operation, &tenant_id, &user_id, &target);
    let intent = write_intent(operation, &tenant_id, &user_id, &(locale.as_str(), &draft));
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result: Result<BoolMutationResponse, ApiError> = request(
        CREATE_PRODUCT_ATTRIBUTE_OPTION_MUTATION,
        Some(LocaleMutationVariables {
            idempotency_key,
            locale,
            input: draft,
        }),
        token,
        tenant_slug,
    )
    .await;
    if result.is_ok() {
        mark_succeeded(&slot);
    }
    result.map(|response| response.create_product_attribute_option.unwrap_or(false))
}

pub(crate) async fn create_catalog_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    locale: String,
    draft: CatalogCategoryDraft,
) -> Result<bool, ApiError> {
    let operation = ProductAdminSchemaOperation::CreateCategory;
    let slot = retry_slot(operation, &tenant_id, &user_id, draft.code.as_str());
    let intent = write_intent(operation, &tenant_id, &user_id, &(locale.as_str(), &draft));
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result: Result<BoolMutationResponse, ApiError> = request(
        CREATE_CATALOG_CATEGORY_MUTATION,
        Some(LocaleMutationVariables {
            idempotency_key,
            locale,
            input: draft,
        }),
        token,
        tenant_slug,
    )
    .await;
    if result.is_ok() {
        mark_succeeded(&slot);
    }
    result.map(|response| response.create_catalog_category.unwrap_or(false))
}

pub(crate) async fn create_attribute_schema(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    locale: String,
    draft: ProductAttributeSchemaDraft,
) -> Result<bool, ApiError> {
    let operation = ProductAdminSchemaOperation::CreateSchema;
    let slot = retry_slot(operation, &tenant_id, &user_id, draft.code.as_str());
    let intent = write_intent(operation, &tenant_id, &user_id, &(locale.as_str(), &draft));
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result: Result<BoolMutationResponse, ApiError> = request(
        CREATE_ATTRIBUTE_SCHEMA_MUTATION,
        Some(LocaleMutationVariables {
            idempotency_key,
            locale,
            input: draft,
        }),
        token,
        tenant_slug,
    )
    .await;
    if result.is_ok() {
        mark_succeeded(&slot);
    }
    result.map(|response| response.create_product_attribute_schema.unwrap_or(false))
}

pub(crate) async fn create_product_attribute_schema_group(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    locale: String,
    draft: ProductAttributeSchemaGroupDraft,
) -> Result<bool, ApiError> {
    let operation = ProductAdminSchemaOperation::CreateSchemaGroup;
    let target = format!("{}:{}", draft.schema_id, draft.code);
    let slot = retry_slot(operation, &tenant_id, &user_id, &target);
    let intent = write_intent(operation, &tenant_id, &user_id, &(locale.as_str(), &draft));
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result: Result<BoolMutationResponse, ApiError> = request(
        CREATE_SCHEMA_GROUP_MUTATION,
        Some(LocaleMutationVariables {
            idempotency_key,
            locale,
            input: draft,
        }),
        token,
        tenant_slug,
    )
    .await;
    if result.is_ok() {
        mark_succeeded(&slot);
    }
    result.map(|response| response.create_product_attribute_schema_group.unwrap_or(false))
}

pub(crate) async fn create_category_attribute_group(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    locale: String,
    draft: CategoryAttributeGroupDraft,
) -> Result<bool, ApiError> {
    let operation = ProductAdminSchemaOperation::CreateCategoryGroup;
    let target = format!("{}:{}", draft.category_id, draft.code);
    let slot = retry_slot(operation, &tenant_id, &user_id, &target);
    let intent = write_intent(operation, &tenant_id, &user_id, &(locale.as_str(), &draft));
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result: Result<BoolMutationResponse, ApiError> = request(
        CREATE_CATEGORY_GROUP_MUTATION,
        Some(LocaleMutationVariables {
            idempotency_key,
            locale,
            input: draft,
        }),
        token,
        tenant_slug,
    )
    .await;
    if result.is_ok() {
        mark_succeeded(&slot);
    }
    result.map(|response| response.create_catalog_category_attribute_group.unwrap_or(false))
}

pub(crate) async fn set_category_schema_mode(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    draft: SetCategorySchemaModeDraft,
) -> Result<bool, ApiError> {
    let operation = ProductAdminSchemaOperation::SetCategorySchemaMode;
    let slot = retry_slot(operation, &tenant_id, &user_id, draft.category_id.as_str());
    let intent = write_intent(operation, &tenant_id, &user_id, &draft);
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result: Result<BoolMutationResponse, ApiError> = request(
        SET_CATEGORY_SCHEMA_MODE_MUTATION,
        Some(InputVariables {
            idempotency_key,
            input: draft,
        }),
        token,
        tenant_slug,
    )
    .await;
    if result.is_ok() {
        mark_succeeded(&slot);
    }
    result.map(|response| response.set_catalog_category_schema_mode.unwrap_or(false))
}

pub(crate) async fn bind_schema_attribute(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    draft: BindSchemaAttributeDraft,
) -> Result<bool, ApiError> {
    let operation = ProductAdminSchemaOperation::BindSchemaAttribute;
    let target = format!("{}:{}", draft.schema_id, draft.attribute_id);
    let slot = retry_slot(operation, &tenant_id, &user_id, &target);
    let intent = write_intent(operation, &tenant_id, &user_id, &draft);
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result: Result<BoolMutationResponse, ApiError> = request(
        BIND_SCHEMA_ATTRIBUTE_MUTATION,
        Some(InputVariables {
            idempotency_key,
            input: draft,
        }),
        token,
        tenant_slug,
    )
    .await;
    if result.is_ok() {
        mark_succeeded(&slot);
    }
    result.map(|response| response.bind_product_attribute_schema_attribute.unwrap_or(false))
}

pub(crate) async fn bind_category_attribute(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    draft: BindCategoryAttributeDraft,
) -> Result<bool, ApiError> {
    let operation = ProductAdminSchemaOperation::BindCategoryAttribute;
    let target = format!("{}:{}", draft.category_id, draft.attribute_id);
    let slot = retry_slot(operation, &tenant_id, &user_id, &target);
    let intent = write_intent(operation, &tenant_id, &user_id, &draft);
    let idempotency_key = retained_caller_key(&slot, operation, intent);
    let result: Result<BoolMutationResponse, ApiError> = request(
        BIND_CATEGORY_ATTRIBUTE_MUTATION,
        Some(InputVariables {
            idempotency_key,
            input: draft,
        }),
        token,
        tenant_slug,
    )
    .await;
    if result.is_ok() {
        mark_succeeded(&slot);
    }
    result.map(|response| response.bind_catalog_category_attribute.unwrap_or(false))
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
