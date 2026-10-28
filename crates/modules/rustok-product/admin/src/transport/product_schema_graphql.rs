//! GraphQL transport for Product schema-authoring writes.
//!
//! The mounted Commerce GraphQL boundary requires a non-null `idempotencyKey` on every schema
//! mutation. This module is the only place that builds those documents. Each public function
//! receives the caller key minted once per logical invocation by the legacy transport (see
//! `schema_retry_identity::run_keyed_schema_write`), forwards it as `$idempotencyKey`, and treats
//! a missing or `false` owner confirmation as a protocol error so the key stays retained.

use rustok_graphql::{GraphqlHttpError, GraphqlRequest, execute as execute_graphql, graphql_url};
use serde::{Deserialize, Serialize};

use crate::model::{
    BindCategoryAttributeDraft, BindSchemaAttributeDraft, CatalogCategoryDraft,
    CategoryAttributeGroupDraft, ProductAttributeDraft, ProductAttributeOptionDraft,
    ProductAttributeSchemaDraft, ProductAttributeSchemaGroupDraft, ProductAttributeValueItem,
    ProductAttributeValuePatchDraft, SetCategorySchemaModeDraft,
};
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

/// Wire envelope shared by every schema mutation: the caller key plus the operation payload.
///
/// Tenant and actor scope travel with the request credentials (token and tenant slug), not as
/// variables: the mounted schema mutations declare no `tenantId`/`userId` arguments. The caller
/// rebuilds those values only for the retry slot (`schema_slot`) and intent (`schema_intent`).
#[derive(Debug, Serialize)]
struct SchemaWriteVariables<T> {
    #[serde(rename = "idempotencyKey")]
    idempotency_key: String,
    #[serde(flatten)]
    extra: T,
}

#[derive(Debug, Serialize)]
struct LocaleInputVariables<T> {
    locale: String,
    input: T,
}

#[derive(Debug, Serialize)]
struct InputVariables<T> {
    input: T,
}

#[derive(Debug, Serialize)]
struct SaveAttributeValuesVariables {
    #[serde(rename = "productId")]
    product_id: String,
    locale: String,
    patches: Vec<ProductAttributeValuePatchDraft>,
}

#[derive(Debug, Serialize)]
struct ClearDetachedAttributeValuesVariables {
    #[serde(rename = "productId")]
    product_id: String,
    locale: String,
    #[serde(rename = "attributeIds")]
    attribute_ids: Vec<String>,
}

/// Maps the owner's boolean mutation result to a confirmed success.
///
/// `null` or `false` without a GraphQL error is a protocol violation: the owner did not confirm
/// the write, so the caller key must stay retained for an explicit retry.
fn confirmed_schema_write(value: Option<bool>) -> Result<bool, GraphqlHttpError> {
    match value {
        Some(true) => Ok(true),
        Some(false) | None => Err(GraphqlHttpError::Graphql(
            "Product schema write was not confirmed by the owner".to_string(),
        )),
    }
}

async fn request<V, T>(
    query: &str,
    variables: V,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, GraphqlHttpError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, Some(variables)),
        token,
        tenant_slug,
        None,
    )
    .await
}

pub(crate) async fn create_product_attribute(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
    draft: ProductAttributeDraft,
    idempotency_key: String,
) -> Result<bool, GraphqlHttpError> {
    let response: BoolMutationResponse = request(
        CREATE_PRODUCT_ATTRIBUTE_MUTATION,
        SchemaWriteVariables {
            idempotency_key,
            extra: LocaleInputVariables {
                locale,
                input: draft,
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    confirmed_schema_write(response.create_product_attribute)
}

pub(crate) async fn create_product_attribute_option(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
    draft: ProductAttributeOptionDraft,
    idempotency_key: String,
) -> Result<bool, GraphqlHttpError> {
    let response: BoolMutationResponse = request(
        CREATE_PRODUCT_ATTRIBUTE_OPTION_MUTATION,
        SchemaWriteVariables {
            idempotency_key,
            extra: LocaleInputVariables {
                locale,
                input: draft,
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    confirmed_schema_write(response.create_product_attribute_option)
}

pub(crate) async fn create_catalog_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
    draft: CatalogCategoryDraft,
    idempotency_key: String,
) -> Result<bool, GraphqlHttpError> {
    let response: BoolMutationResponse = request(
        CREATE_CATALOG_CATEGORY_MUTATION,
        SchemaWriteVariables {
            idempotency_key,
            extra: LocaleInputVariables {
                locale,
                input: draft,
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    confirmed_schema_write(response.create_catalog_category)
}

pub(crate) async fn create_attribute_schema(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
    draft: ProductAttributeSchemaDraft,
    idempotency_key: String,
) -> Result<bool, GraphqlHttpError> {
    let response: BoolMutationResponse = request(
        CREATE_ATTRIBUTE_SCHEMA_MUTATION,
        SchemaWriteVariables {
            idempotency_key,
            extra: LocaleInputVariables {
                locale,
                input: draft,
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    confirmed_schema_write(response.create_product_attribute_schema)
}

pub(crate) async fn create_product_attribute_schema_group(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
    draft: ProductAttributeSchemaGroupDraft,
    idempotency_key: String,
) -> Result<bool, GraphqlHttpError> {
    let response: BoolMutationResponse = request(
        CREATE_SCHEMA_GROUP_MUTATION,
        SchemaWriteVariables {
            idempotency_key,
            extra: LocaleInputVariables {
                locale,
                input: draft,
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    confirmed_schema_write(response.create_product_attribute_schema_group)
}

pub(crate) async fn create_category_attribute_group(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
    draft: CategoryAttributeGroupDraft,
    idempotency_key: String,
) -> Result<bool, GraphqlHttpError> {
    let response: BoolMutationResponse = request(
        CREATE_CATEGORY_GROUP_MUTATION,
        SchemaWriteVariables {
            idempotency_key,
            extra: LocaleInputVariables {
                locale,
                input: draft,
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    confirmed_schema_write(response.create_catalog_category_attribute_group)
}

pub(crate) async fn set_category_schema_mode(
    token: Option<String>,
    tenant_slug: Option<String>,
    draft: SetCategorySchemaModeDraft,
    idempotency_key: String,
) -> Result<bool, GraphqlHttpError> {
    let response: BoolMutationResponse = request(
        SET_CATEGORY_SCHEMA_MODE_MUTATION,
        SchemaWriteVariables {
            idempotency_key,
            extra: InputVariables { input: draft },
        },
        token,
        tenant_slug,
    )
    .await?;
    confirmed_schema_write(response.set_catalog_category_schema_mode)
}

pub(crate) async fn bind_schema_attribute(
    token: Option<String>,
    tenant_slug: Option<String>,
    draft: BindSchemaAttributeDraft,
    idempotency_key: String,
) -> Result<bool, GraphqlHttpError> {
    let response: BoolMutationResponse = request(
        BIND_SCHEMA_ATTRIBUTE_MUTATION,
        SchemaWriteVariables {
            idempotency_key,
            extra: InputVariables { input: draft },
        },
        token,
        tenant_slug,
    )
    .await?;
    confirmed_schema_write(response.bind_product_attribute_schema_attribute)
}

pub(crate) async fn bind_category_attribute(
    token: Option<String>,
    tenant_slug: Option<String>,
    draft: BindCategoryAttributeDraft,
    idempotency_key: String,
) -> Result<bool, GraphqlHttpError> {
    let response: BoolMutationResponse = request(
        BIND_CATEGORY_ATTRIBUTE_MUTATION,
        SchemaWriteVariables {
            idempotency_key,
            extra: InputVariables { input: draft },
        },
        token,
        tenant_slug,
    )
    .await?;
    confirmed_schema_write(response.bind_catalog_category_attribute)
}

pub(crate) async fn save_product_attribute_values(
    token: Option<String>,
    tenant_slug: Option<String>,
    product_id: String,
    locale: String,
    patches: Vec<ProductAttributeValuePatchDraft>,
    idempotency_key: String,
) -> Result<Vec<ProductAttributeValueItem>, GraphqlHttpError> {
    let patches = normalize_product_attribute_patches(patches);
    let response: SaveAttributeValuesResponse = request(
        SAVE_ATTRIBUTE_VALUES_MUTATION,
        SchemaWriteVariables {
            idempotency_key,
            extra: SaveAttributeValuesVariables {
                product_id,
                locale,
                patches,
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok::<Vec<ProductAttributeValueItem>, GraphqlHttpError>(response.save_product_attribute_values)
}

pub(crate) async fn clear_detached_product_attribute_values(
    token: Option<String>,
    tenant_slug: Option<String>,
    product_id: String,
    locale: String,
    attribute_ids: Vec<String>,
    idempotency_key: String,
) -> Result<Vec<ProductAttributeValueItem>, GraphqlHttpError> {
    let response: ClearDetachedAttributeValuesResponse = request(
        CLEAR_DETACHED_ATTRIBUTE_VALUES_MUTATION,
        SchemaWriteVariables {
            idempotency_key,
            extra: ClearDetachedAttributeValuesVariables {
                product_id,
                locale,
                attribute_ids,
            },
        },
        token,
        tenant_slug,
    )
    .await?;
    Ok::<Vec<ProductAttributeValueItem>, GraphqlHttpError>(
        response.clear_detached_product_attribute_values,
    )
}

/// GraphQL enum spelling belongs to the transport, not to the editor model.
/// Textarea/richtext are editor rendering kinds; their typed value is Text.
pub(crate) fn normalize_product_attribute_patches(
    mut patches: Vec<ProductAttributeValuePatchDraft>,
) -> Vec<ProductAttributeValuePatchDraft> {
    for patch in &mut patches {
        patch.kind = match patch.kind.trim().to_ascii_uppercase().as_str() {
            "TEXTAREA" | "RICHTEXT" => "TEXT".into(),
            kind => kind.to_string(),
        };
    }
    patches
}

pub(crate) fn normalize_product_attribute_batches(
    batches: Vec<crate::model::ProductAttributeValueBatchDraft>,
) -> Vec<crate::model::ProductAttributeValueBatchDraft> {
    batches
        .into_iter()
        .map(|mut batch| {
            batch.patches = normalize_product_attribute_patches(batch.patches);
            batch
        })
        .collect()
}

#[cfg(test)]
mod value_input_tests {
    use super::*;

    #[test]
    fn atomic_batches_use_the_same_graphql_enum_admission_as_standalone_patches() {
        let batches = ["text", "textarea", "richtext", "clear"]
            .into_iter()
            .map(|kind| {
                serde_json::from_value(serde_json::json!({
                    "locale": "fr",
                    "patches": [{ "attributeId": "attribute", "kind": kind, "text": "value" }]
                }))
                .unwrap()
            })
            .collect();
        let batches = normalize_product_attribute_batches(batches);
        let kinds = batches
            .iter()
            .map(|batch| batch.patches[0].kind.as_str())
            .collect::<Vec<_>>();
        assert_eq!(kinds, vec!["TEXT", "TEXT", "TEXT", "CLEAR"]);
        assert!(batches.iter().all(|batch| batch.locale == "fr"));
        let wire = serde_json::to_value(&batches[0]).unwrap();
        assert_eq!(wire["patches"][0]["attributeId"], "attribute");
        assert_eq!(wire["patches"][0]["text"], "value");
    }
}
