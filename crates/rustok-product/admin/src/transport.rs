mod graphql_adapter;
mod native_server_adapter;

use crate::model::{
    BindCategoryAttributeDraft, BindSchemaAttributeDraft, CatalogCategoryDraft,
    CatalogCategoryList, CategoryAttributeGroupDraft, ProductAdminBootstrap, ProductAttributeDraft,
    ProductAttributeList, ProductAttributeOptionDraft, ProductAttributeSchemaDraft,
    ProductAttributeSchemaGroupDraft, ProductAttributeSchemaList, ProductAttributeValueItem,
    ProductAttributeValuePatchDraft, ProductCatalogSearchOption, ProductCatalogSearchOptions,
    ProductDetail, ProductDraft, ProductEffectiveForm, ProductList, ProductPricingDetail,
    SetCategorySchemaModeDraft, ShippingProfileList,
};
use graphql_adapter::ApiError;

pub(crate) async fn fetch_bootstrap(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<ProductAdminBootstrap, ApiError> {
    graphql_adapter::fetch_bootstrap(token, tenant_slug).await
}

pub(crate) async fn fetch_products(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: Option<String>,
    search: Option<String>,
    status: Option<String>,
) -> Result<ProductList, ApiError> {
    graphql_adapter::fetch_products(token, tenant_slug, tenant_id, locale, search, status).await
}

pub(crate) async fn fetch_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
    locale: Option<String>,
) -> Result<Option<ProductDetail>, ApiError> {
    graphql_adapter::fetch_product(token, tenant_slug, tenant_id, id, locale).await
}

pub(crate) async fn fetch_product_pricing(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
    locale: Option<String>,
    currency_code: Option<String>,
) -> Result<Option<ProductPricingDetail>, ApiError> {
    graphql_adapter::fetch_product_pricing(token, tenant_slug, tenant_id, id, locale, currency_code)
        .await
}

pub(crate) async fn fetch_shipping_profiles(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
) -> Result<ShippingProfileList, ApiError> {
    graphql_adapter::fetch_shipping_profiles(token, tenant_slug, tenant_id).await
}

pub(crate) async fn fetch_product_attributes(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: String,
) -> Result<ProductAttributeList, ApiError> {
    match native_server_adapter::fetch_product_attributes(tenant_id.clone(), locale.clone()).await {
        Ok(value) => Ok(value),
        Err(_) => {
            graphql_adapter::fetch_product_attributes(token, tenant_slug, tenant_id, locale).await
        }
    }
}

pub(crate) async fn fetch_catalog_categories(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: String,
) -> Result<CatalogCategoryList, ApiError> {
    match native_server_adapter::fetch_catalog_categories(tenant_id.clone(), locale.clone()).await {
        Ok(value) => Ok(value),
        Err(_) => {
            graphql_adapter::fetch_catalog_categories(token, tenant_slug, tenant_id, locale).await
        }
    }
}

pub async fn fetch_catalog_search_options(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
) -> Result<ProductCatalogSearchOptions, String> {
    if let Ok(options) = native_server_adapter::fetch_catalog_search_options(locale.clone()).await {
        return Ok(options);
    }

    let bootstrap = graphql_adapter::fetch_bootstrap(token.clone(), tenant_slug.clone())
        .await
        .map_err(|err| err.to_string())?;
    let tenant_id = bootstrap.current_tenant.id;
    let categories = graphql_adapter::fetch_catalog_categories(
        token.clone(),
        tenant_slug.clone(),
        tenant_id.clone(),
        locale.clone(),
    )
    .await
    .map_err(|err| err.to_string())?;
    let attributes =
        graphql_adapter::fetch_product_attributes(token, tenant_slug, tenant_id, locale)
            .await
            .map_err(|err| err.to_string())?;

    Ok(ProductCatalogSearchOptions {
        category_options: categories
            .items
            .into_iter()
            .map(|category| ProductCatalogSearchOption {
                value: category.id,
                label: first_non_empty([category.path, category.name, category.code]),
            })
            .collect(),
        attribute_options: attributes
            .items
            .into_iter()
            .filter(|attribute| attribute.is_filterable || attribute.is_sortable)
            .map(|attribute| {
                let label = first_non_empty([attribute.label, attribute.code.clone()]);
                ProductCatalogSearchOption {
                    value: attribute.code.clone(),
                    label: format!("{label} ({})", attribute.code),
                }
            })
            .collect(),
    })
}

fn first_non_empty(values: impl IntoIterator<Item = String>) -> String {
    values
        .into_iter()
        .find(|value| !value.trim().is_empty())
        .unwrap_or_default()
}

pub(crate) async fn fetch_attribute_schemas(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: String,
) -> Result<ProductAttributeSchemaList, ApiError> {
    match native_server_adapter::fetch_attribute_schemas(tenant_id.clone(), locale.clone()).await {
        Ok(value) => Ok(value),
        Err(_) => {
            graphql_adapter::fetch_attribute_schemas(token, tenant_slug, tenant_id, locale).await
        }
    }
}

pub(crate) async fn fetch_effective_product_form(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    product_id: Option<String>,
    category_id: Option<String>,
    locale: String,
) -> Result<Option<ProductEffectiveForm>, ApiError> {
    match native_server_adapter::fetch_effective_product_form(
        tenant_id.clone(),
        product_id.clone(),
        category_id.clone(),
        locale.clone(),
    )
    .await
    {
        Ok(value) => Ok(value),
        Err(_) => {
            graphql_adapter::fetch_effective_product_form(
                token,
                tenant_slug,
                tenant_id,
                product_id,
                category_id,
                locale,
            )
            .await
        }
    }
}

pub(crate) async fn fetch_product_attribute_values(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    product_id: String,
    locale: String,
) -> Result<Vec<ProductAttributeValueItem>, ApiError> {
    match native_server_adapter::fetch_product_attribute_values(
        tenant_id.clone(),
        product_id.clone(),
        locale.clone(),
    )
    .await
    {
        Ok(value) => Ok(value),
        Err(_) => {
            graphql_adapter::fetch_product_attribute_values(
                token,
                tenant_slug,
                tenant_id,
                product_id,
                locale,
            )
            .await
        }
    }
}

pub(crate) async fn create_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    draft: ProductDraft,
) -> Result<ProductDetail, ApiError> {
    graphql_adapter::create_product(token, tenant_slug, tenant_id, user_id, draft).await
}

pub(crate) async fn create_product_attribute(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    locale: String,
    draft: ProductAttributeDraft,
) -> Result<bool, ApiError> {
    match native_server_adapter::create_product_attribute(
        tenant_id.clone(),
        locale.clone(),
        draft.clone(),
    )
    .await
    {
        Ok(value) => Ok(value),
        Err(_) => {
            graphql_adapter::create_product_attribute(
                token,
                tenant_slug,
                tenant_id,
                user_id,
                locale,
                draft,
            )
            .await
        }
    }
}

pub(crate) async fn create_product_attribute_option(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    locale: String,
    draft: ProductAttributeOptionDraft,
) -> Result<bool, ApiError> {
    match native_server_adapter::create_product_attribute_option(
        tenant_id.clone(),
        locale.clone(),
        draft.clone(),
    )
    .await
    {
        Ok(value) => Ok(value),
        Err(_) => {
            graphql_adapter::create_product_attribute_option(
                token,
                tenant_slug,
                tenant_id,
                user_id,
                locale,
                draft,
            )
            .await
        }
    }
}

pub(crate) async fn create_catalog_category(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    locale: String,
    draft: CatalogCategoryDraft,
) -> Result<bool, ApiError> {
    match native_server_adapter::create_catalog_category(
        tenant_id.clone(),
        locale.clone(),
        draft.clone(),
    )
    .await
    {
        Ok(value) => Ok(value),
        Err(_) => {
            graphql_adapter::create_catalog_category(
                token,
                tenant_slug,
                tenant_id,
                user_id,
                locale,
                draft,
            )
            .await
        }
    }
}

pub(crate) async fn create_attribute_schema(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    locale: String,
    draft: ProductAttributeSchemaDraft,
) -> Result<bool, ApiError> {
    match native_server_adapter::create_attribute_schema(
        tenant_id.clone(),
        locale.clone(),
        draft.clone(),
    )
    .await
    {
        Ok(value) => Ok(value),
        Err(_) => {
            graphql_adapter::create_attribute_schema(
                token,
                tenant_slug,
                tenant_id,
                user_id,
                locale,
                draft,
            )
            .await
        }
    }
}

pub(crate) async fn set_category_schema_mode(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    draft: SetCategorySchemaModeDraft,
) -> Result<bool, ApiError> {
    match native_server_adapter::set_category_schema_mode(tenant_id.clone(), draft.clone()).await {
        Ok(value) => Ok(value),
        Err(_) => {
            graphql_adapter::set_category_schema_mode(token, tenant_slug, tenant_id, user_id, draft)
                .await
        }
    }
}

pub(crate) async fn create_product_attribute_schema_group(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    locale: String,
    draft: ProductAttributeSchemaGroupDraft,
) -> Result<bool, ApiError> {
    match native_server_adapter::create_product_attribute_schema_group(
        tenant_id.clone(),
        locale.clone(),
        draft.clone(),
    )
    .await
    {
        Ok(value) => Ok(value),
        Err(_) => {
            graphql_adapter::create_product_attribute_schema_group(
                token,
                tenant_slug,
                tenant_id,
                user_id,
                locale,
                draft,
            )
            .await
        }
    }
}

pub(crate) async fn create_category_attribute_group(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    locale: String,
    draft: CategoryAttributeGroupDraft,
) -> Result<bool, ApiError> {
    match native_server_adapter::create_category_attribute_group(
        tenant_id.clone(),
        locale.clone(),
        draft.clone(),
    )
    .await
    {
        Ok(value) => Ok(value),
        Err(_) => {
            graphql_adapter::create_category_attribute_group(
                token,
                tenant_slug,
                tenant_id,
                user_id,
                locale,
                draft,
            )
            .await
        }
    }
}

pub(crate) async fn bind_schema_attribute(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    draft: BindSchemaAttributeDraft,
) -> Result<bool, ApiError> {
    match native_server_adapter::bind_schema_attribute(tenant_id.clone(), draft.clone()).await {
        Ok(value) => Ok(value),
        Err(_) => {
            graphql_adapter::bind_schema_attribute(token, tenant_slug, tenant_id, user_id, draft)
                .await
        }
    }
}

pub(crate) async fn bind_category_attribute(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    draft: BindCategoryAttributeDraft,
) -> Result<bool, ApiError> {
    match native_server_adapter::bind_category_attribute(tenant_id.clone(), draft.clone()).await {
        Ok(value) => Ok(value),
        Err(_) => {
            graphql_adapter::bind_category_attribute(token, tenant_slug, tenant_id, user_id, draft)
                .await
        }
    }
}

pub(crate) async fn save_product_attribute_values(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    product_id: String,
    locale: String,
    patches: Vec<ProductAttributeValuePatchDraft>,
) -> Result<Vec<ProductAttributeValueItem>, ApiError> {
    match native_server_adapter::save_product_attribute_values(
        tenant_id.clone(),
        product_id.clone(),
        locale.clone(),
        patches.clone(),
    )
    .await
    {
        Ok(value) => Ok(value),
        Err(_) => {
            graphql_adapter::save_product_attribute_values(
                token,
                tenant_slug,
                tenant_id,
                user_id,
                product_id,
                locale,
                patches,
            )
            .await
        }
    }
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
    match native_server_adapter::clear_detached_product_attribute_values(
        tenant_id.clone(),
        product_id.clone(),
        locale.clone(),
        attribute_ids.clone(),
    )
    .await
    {
        Ok(values) => Ok(values),
        Err(_) => {
            graphql_adapter::clear_detached_product_attribute_values(
                token,
                tenant_slug,
                tenant_id,
                user_id,
                product_id,
                locale,
                attribute_ids,
            )
            .await
        }
    }
}

pub(crate) async fn update_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    id: String,
    draft: ProductDraft,
) -> Result<ProductDetail, ApiError> {
    graphql_adapter::update_product(token, tenant_slug, tenant_id, user_id, id, draft).await
}

pub(crate) async fn change_product_status(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    id: String,
    status: &str,
) -> Result<ProductDetail, ApiError> {
    graphql_adapter::change_product_status(token, tenant_slug, tenant_id, user_id, id, status).await
}

pub(crate) async fn delete_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    id: String,
) -> Result<bool, ApiError> {
    graphql_adapter::delete_product(token, tenant_slug, tenant_id, user_id, id).await
}
