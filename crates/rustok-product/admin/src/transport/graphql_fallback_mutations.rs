use crate::model::{
    BindCategoryAttributeDraft, BindSchemaAttributeDraft, CatalogCategoryDraft,
    CategoryAttributeGroupDraft, ProductAttributeDraft, ProductAttributeOptionDraft,
    ProductAttributeSchemaDraft, ProductAttributeSchemaGroupDraft, ProductAttributeValueItem,
    ProductAttributeValuePatchDraft, SetCategorySchemaModeDraft,
};
use rustok_graphql::GraphqlHttpError;

use super::graphql_fallback_mutation_error_safety::GraphqlFallbackMutationContext;
use super::legacy;

pub(crate) async fn create_product_attribute(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    locale: String,
    draft: ProductAttributeDraft,
) -> Result<bool, GraphqlHttpError> {
    let context = GraphqlFallbackMutationContext::for_create_product_attribute(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        user_id.as_str(),
        locale.as_str(),
    );
    legacy::create_product_attribute(token, tenant_slug, tenant_id, user_id, locale, draft)
        .await
        .map_err(|fallback_mutation_error| context.map_error(fallback_mutation_error))
}

pub(crate) async fn create_product_attribute_option(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    locale: String,
    draft: ProductAttributeOptionDraft,
) -> Result<bool, GraphqlHttpError> {
    let context = GraphqlFallbackMutationContext::for_create_product_attribute_option(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        user_id.as_str(),
        locale.as_str(),
    );
    legacy::create_product_attribute_option(token, tenant_slug, tenant_id, user_id, locale, draft)
        .await
        .map_err(|fallback_mutation_error| context.map_error(fallback_mutation_error))
}

pub(crate) async fn create_catalog_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    locale: String,
    draft: CatalogCategoryDraft,
) -> Result<bool, GraphqlHttpError> {
    let context = GraphqlFallbackMutationContext::for_create_catalog_category(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        user_id.as_str(),
        locale.as_str(),
    );
    legacy::create_catalog_category(token, tenant_slug, tenant_id, user_id, locale, draft)
        .await
        .map_err(|fallback_mutation_error| context.map_error(fallback_mutation_error))
}

pub(crate) async fn create_attribute_schema(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    locale: String,
    draft: ProductAttributeSchemaDraft,
) -> Result<bool, GraphqlHttpError> {
    let context = GraphqlFallbackMutationContext::for_create_attribute_schema(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        user_id.as_str(),
        locale.as_str(),
    );
    legacy::create_attribute_schema(token, tenant_slug, tenant_id, user_id, locale, draft)
        .await
        .map_err(|fallback_mutation_error| context.map_error(fallback_mutation_error))
}

pub(crate) async fn set_category_schema_mode(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    draft: SetCategorySchemaModeDraft,
) -> Result<bool, GraphqlHttpError> {
    let context = GraphqlFallbackMutationContext::for_set_category_schema_mode(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        user_id.as_str(),
    );
    legacy::set_category_schema_mode(token, tenant_slug, tenant_id, user_id, draft)
        .await
        .map_err(|fallback_mutation_error| context.map_error(fallback_mutation_error))
}

pub(crate) async fn create_product_attribute_schema_group(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    locale: String,
    draft: ProductAttributeSchemaGroupDraft,
) -> Result<bool, GraphqlHttpError> {
    let context = GraphqlFallbackMutationContext::for_create_product_attribute_schema_group(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        user_id.as_str(),
        locale.as_str(),
    );
    legacy::create_product_attribute_schema_group(
        token,
        tenant_slug,
        tenant_id,
        user_id,
        locale,
        draft,
    )
    .await
    .map_err(|fallback_mutation_error| context.map_error(fallback_mutation_error))
}

pub(crate) async fn create_category_attribute_group(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    locale: String,
    draft: CategoryAttributeGroupDraft,
) -> Result<bool, GraphqlHttpError> {
    let context = GraphqlFallbackMutationContext::for_create_category_attribute_group(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        user_id.as_str(),
        locale.as_str(),
    );
    legacy::create_category_attribute_group(token, tenant_slug, tenant_id, user_id, locale, draft)
        .await
        .map_err(|fallback_mutation_error| context.map_error(fallback_mutation_error))
}

pub(crate) async fn bind_schema_attribute(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    draft: BindSchemaAttributeDraft,
) -> Result<bool, GraphqlHttpError> {
    let context = GraphqlFallbackMutationContext::for_bind_schema_attribute(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        user_id.as_str(),
    );
    legacy::bind_schema_attribute(token, tenant_slug, tenant_id, user_id, draft)
        .await
        .map_err(|fallback_mutation_error| context.map_error(fallback_mutation_error))
}

pub(crate) async fn bind_category_attribute(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    draft: BindCategoryAttributeDraft,
) -> Result<bool, GraphqlHttpError> {
    let context = GraphqlFallbackMutationContext::for_bind_category_attribute(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        user_id.as_str(),
    );
    legacy::bind_category_attribute(token, tenant_slug, tenant_id, user_id, draft)
        .await
        .map_err(|fallback_mutation_error| context.map_error(fallback_mutation_error))
}

pub(crate) async fn save_product_attribute_values(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    product_id: String,
    locale: String,
    patches: Vec<ProductAttributeValuePatchDraft>,
) -> Result<Vec<ProductAttributeValueItem>, GraphqlHttpError> {
    let context = GraphqlFallbackMutationContext::for_save_product_attribute_values(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        user_id.as_str(),
        product_id.as_str(),
        locale.as_str(),
        patches.len(),
    );
    legacy::save_product_attribute_values(
        token,
        tenant_slug,
        tenant_id,
        user_id,
        product_id,
        locale,
        patches,
    )
    .await
    .map_err(|fallback_mutation_error| context.map_error(fallback_mutation_error))
}

pub(crate) async fn clear_detached_product_attribute_values(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    product_id: String,
    locale: String,
    attribute_ids: Vec<String>,
) -> Result<Vec<ProductAttributeValueItem>, GraphqlHttpError> {
    let context = GraphqlFallbackMutationContext::for_clear_detached_product_attribute_values(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        user_id.as_str(),
        product_id.as_str(),
        locale.as_str(),
        attribute_ids.len(),
    );
    legacy::clear_detached_product_attribute_values(
        token,
        tenant_slug,
        tenant_id,
        user_id,
        product_id,
        locale,
        attribute_ids,
    )
    .await
    .map_err(|fallback_mutation_error| context.map_error(fallback_mutation_error))
}
