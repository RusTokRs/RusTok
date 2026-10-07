//! Canonical Product admin transport facade.
//!
//! Every Product admin surface reads the owner through this module. It owns the
//! final read boundary: each primary and category read is wrapped in a
//! correlation-aware [`graphql_error_safety::GraphqlReadContext`] before the
//! private gateway executes it, so backend-controlled text never reaches the UI
//! and the complete typed error is never logged.
//!
//! The catalog list stays native-first: the owner server function is attempted
//! first and the GraphQL document is the fallback. Writes keep their retry and
//! idempotency identity in `transport` and are re-exported here so the mounted
//! surface has one import path.

#[path = "transport/graphql_error_safety.rs"]
mod graphql_error_safety;

use leptos::prelude::*;
use rustok_graphql::GraphqlHttpError;
use rustok_ui_core::UiRouteContext;

use crate::catalog_controls::{build_product_admin_list_input, ProductAdminListInput};
use crate::model::{
    BindCategoryAttributeDraft, BindSchemaAttributeDraft, CatalogCategoryDraft,
    CatalogCategoryList, CategoryAttributeGroupDraft, ProductAdminBootstrap,
    ProductAttributeDraft, ProductAttributeList, ProductAttributeOptionDraft,
    ProductAttributeSchemaDraft, ProductAttributeSchemaGroupDraft, ProductAttributeSchemaList,
    ProductAttributeValueItem, ProductAttributeValuePatchDraft, ProductCatalogSearchOptions,
    ProductDetail, ProductDraft, ProductEffectiveForm, ProductList, ProductPricingDetail,
    SetCategorySchemaModeDraft, ShippingProfileList,
};
use crate::transport as legacy;

use graphql_error_safety::GraphqlFallbackMutationContext;
use graphql_error_safety::GraphqlMutationContext;
use graphql_error_safety::GraphqlReadContext;

pub(crate) type ApiError = GraphqlHttpError;

const PRODUCT_ADMIN_CATALOG_OPTIONS_OPERATION: &str = "fetch_catalog_search_options";
const PRODUCT_ADMIN_CATALOG_OPTIONS_PUBLIC_MESSAGE: &str =
    "Product catalog search options are temporarily unavailable";

/// Bounded private diagnostics for the public catalog search-options wrapper.
///
/// The compatibility executor in `transport` returns the captured GraphQL error
/// as a `String`; that value is used only to derive presence and character
/// length. The captured text is not logged and never becomes the public error.
struct CatalogSearchOptionsErrorContext {
    correlation_id: String,
    token_present: bool,
    tenant_slug_length: Option<usize>,
    locale_length: usize,
}

impl CatalogSearchOptionsErrorContext {
    fn new(token: Option<&str>, tenant_slug: Option<&str>, locale: &str) -> Self {
        Self {
            correlation_id: format!(
                "product-admin-catalog-options:{PRODUCT_ADMIN_CATALOG_OPTIONS_OPERATION}:{}",
                uuid::Uuid::new_v4()
            ),
            token_present: token.is_some(),
            tenant_slug_length: tenant_slug.map(|value| value.chars().count()),
            locale_length: locale.chars().count(),
        }
    }

    fn map_error(&self, raw_error: String) -> String {
        let raw_error_present = !raw_error.is_empty();
        let raw_error_length = raw_error.chars().count();

        tracing::error!(
            raw_error_present,
            raw_error_length,
            owner = "rustok_product.admin",
            owner_operation = PRODUCT_ADMIN_CATALOG_OPTIONS_OPERATION,
            correlation_id = %self.correlation_id,
            token_present = self.token_present,
            tenant_slug_length = ?self.tenant_slug_length,
            locale_length = self.locale_length,
            code = "product.admin_catalog_search_options_graphql_unavailable",
            "product admin catalog search options are unavailable"
        );

        PRODUCT_ADMIN_CATALOG_OPTIONS_PUBLIC_MESSAGE.to_string()
    }
}

/// Route-query controls snapshot for the admin catalog list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProductAdminRouteControls {
    pub category_id: Option<String>,
    pub sort_by: Option<String>,
    pub sort_direction: Option<String>,
    pub attribute_filters: Option<String>,
}

impl ProductAdminRouteControls {
    pub(crate) fn from_route_context(route_context: &UiRouteContext) -> Self {
        Self {
            category_id: route_context
                .query_value("category_id")
                .map(ToString::to_string),
            sort_by: route_context
                .query_value("sort_by")
                .map(ToString::to_string),
            sort_direction: route_context
                .query_value("sort_direction")
                .map(ToString::to_string),
            attribute_filters: route_context
                .query_value("attribute_filters")
                .map(ToString::to_string),
        }
    }
}

/// Resolves the catalog list input for the mounted list page.
///
/// The page provides its normalized controls through context; when the page
/// relies on the URL instead, the route query is normalized into the same
/// [`ProductAdminListInput`] contract.
pub(crate) fn product_admin_list_input_from_route() -> ProductAdminListInput {
    if let Some(provided) = use_context::<ProductAdminListInput>() {
        return provided;
    }
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let route_controls = ProductAdminRouteControls::from_route_context(&route_context);
    build_product_admin_list_input(
        None,
        None,
        route_controls.category_id,
        route_controls.sort_by,
        route_controls.sort_direction,
        route_controls.attribute_filters,
    )
}

pub(crate) async fn fetch_bootstrap(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<ProductAdminBootstrap, ApiError> {
    let context = GraphqlReadContext::for_bootstrap(token.as_deref(), tenant_slug.as_deref());
    legacy::fetch_bootstrap(token, tenant_slug)
        .await
        .map_err(|error| context.map_error(error))
}

pub async fn fetch_catalog_search_options(
    token: Option<String>,
    tenant_slug: Option<String>,
    locale: String,
) -> Result<ProductCatalogSearchOptions, String> {
    let context = CatalogSearchOptionsErrorContext::new(
        token.as_deref(),
        tenant_slug.as_deref(),
        locale.as_str(),
    );
    legacy::fetch_catalog_search_options(token, tenant_slug, locale)
        .await
        .map_err(|error| context.map_error(error))
}

pub(crate) async fn fetch_products(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: Option<String>,
    controls: ProductAdminListInput,
) -> Result<ProductList, ApiError> {
    match legacy::admin_catalog_native::fetch_products(
        tenant_id.clone(),
        locale.clone(),
        controls.clone(),
    )
    .await
    {
        Ok(value) => Ok(value),
        Err(_) => {
            let context = GraphqlReadContext::for_products(
                token.as_deref(),
                tenant_slug.as_deref(),
                tenant_id.as_str(),
                locale.as_deref(),
                controls.search.as_deref(),
                controls.status.as_deref(),
            );
            legacy::admin_catalog_graphql::fetch_products(
                token, tenant_slug, tenant_id, locale, controls,
            )
            .await
            .map_err(|error| context.map_error(error))
        }
    }
}

pub(crate) async fn fetch_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
    locale: Option<String>,
) -> Result<Option<ProductDetail>, ApiError> {
    let context = GraphqlReadContext::for_product(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        id.as_str(),
        locale.as_deref(),
    );
    legacy::fetch_product(token, tenant_slug, tenant_id, id, locale)
        .await
        .map_err(|error| context.map_error(error))
}

pub(crate) async fn fetch_product_pricing(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
    locale: Option<String>,
    currency_code: Option<String>,
) -> Result<Option<ProductPricingDetail>, ApiError> {
    let context = GraphqlReadContext::for_product_pricing(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        id.as_str(),
        locale.as_deref(),
        currency_code.as_deref(),
    );
    legacy::fetch_product_pricing(token, tenant_slug, tenant_id, id, locale, currency_code)
        .await
        .map_err(|error| context.map_error(error))
}

pub(crate) async fn fetch_shipping_profiles(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
) -> Result<ShippingProfileList, ApiError> {
    let context = GraphqlReadContext::for_shipping_profiles(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
    );
    legacy::fetch_shipping_profiles(token, tenant_slug, tenant_id)
        .await
        .map_err(|error| context.map_error(error))
}

pub(crate) async fn fetch_product_attributes(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: String,
) -> Result<ProductAttributeList, ApiError> {
    let context = GraphqlReadContext::for_product_attributes(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        locale.as_str(),
    );
    legacy::fetch_product_attributes(token, tenant_slug, tenant_id, locale)
        .await
        .map_err(|failure| context.map_error(failure))
}

pub(crate) async fn fetch_catalog_categories(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: String,
) -> Result<CatalogCategoryList, ApiError> {
    let context = GraphqlReadContext::for_catalog_categories(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        locale.as_str(),
    );
    legacy::fetch_catalog_categories(token, tenant_slug, tenant_id, locale)
        .await
        .map_err(|failure| context.map_error(failure))
}

pub(crate) async fn fetch_attribute_schemas(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: String,
) -> Result<ProductAttributeSchemaList, ApiError> {
    let context = GraphqlReadContext::for_attribute_schemas(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        locale.as_str(),
    );
    legacy::fetch_attribute_schemas(token, tenant_slug, tenant_id, locale)
        .await
        .map_err(|failure| context.map_error(failure))
}

pub(crate) async fn fetch_effective_product_form(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    product_id: Option<String>,
    category_id: Option<String>,
    locale: String,
) -> Result<Option<ProductEffectiveForm>, ApiError> {
    let context = GraphqlReadContext::for_effective_product_form(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        product_id.as_deref(),
        category_id.as_deref(),
        locale.as_str(),
    );
    legacy::fetch_effective_product_form(
        token, tenant_slug, tenant_id, product_id, category_id, locale,
    )
    .await
    .map_err(|failure| context.map_error(failure))
}

pub(crate) async fn fetch_product_attribute_values(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    product_id: String,
    locale: String,
) -> Result<Vec<ProductAttributeValueItem>, ApiError> {
    let context = GraphqlReadContext::for_product_attribute_values(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        product_id.as_str(),
        locale.as_str(),
    );
    legacy::fetch_product_attribute_values(token, tenant_slug, tenant_id, product_id, locale)
        .await
        .map_err(|failure| context.map_error(failure))
}

pub(crate) async fn create_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    draft: ProductDraft,
) -> Result<ProductDetail, ApiError> {
    let context = GraphqlMutationContext::for_create_product(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        user_id.as_str(),
        true,
    );
    legacy::create_product(token, tenant_slug, tenant_id, user_id, draft)
        .await
        .map_err(|mutation_error| context.map_error(mutation_error))
}

pub(crate) async fn update_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    id: String,
    draft: ProductDraft,
) -> Result<ProductDetail, ApiError> {
    let context = GraphqlMutationContext::for_update_product(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        user_id.as_str(),
        id.as_str(),
        true,
    );
    legacy::update_product(token, tenant_slug, tenant_id, user_id, id, draft)
        .await
        .map_err(|mutation_error| context.map_error(mutation_error))
}

pub(crate) async fn change_product_status(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    id: String,
    status: &str,
) -> Result<ProductDetail, ApiError> {
    let context = GraphqlMutationContext::for_change_product_status(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        user_id.as_str(),
        id.as_str(),
        Some(status),
    );
    legacy::change_product_status(token, tenant_slug, tenant_id, user_id, id, status)
        .await
        .map_err(|mutation_error| context.map_error(mutation_error))
}

pub(crate) async fn delete_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    id: String,
) -> Result<bool, ApiError> {
    let context = GraphqlMutationContext::for_delete_product(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        user_id.as_str(),
        id.as_str(),
    );
    legacy::delete_product(token, tenant_slug, tenant_id, user_id, id)
        .await
        .map_err(|mutation_error| context.map_error(mutation_error))
}

pub(crate) async fn create_product_attribute(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    locale: String,
    draft: ProductAttributeDraft,
) -> Result<bool, ApiError> {
    let context = GraphqlFallbackMutationContext::for_create_product_attribute(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        user_id.as_str(),
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
) -> Result<bool, ApiError> {
    let context = GraphqlFallbackMutationContext::for_create_product_attribute_option(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        user_id.as_str(),
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
) -> Result<bool, ApiError> {
    let context = GraphqlFallbackMutationContext::for_create_catalog_category(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        user_id.as_str(),
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
) -> Result<bool, ApiError> {
    let context = GraphqlFallbackMutationContext::for_create_attribute_schema(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        user_id.as_str(),
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
) -> Result<bool, ApiError> {
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
) -> Result<bool, ApiError> {
    let context = GraphqlFallbackMutationContext::for_create_product_attribute_schema_group(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        user_id.as_str(),
    );
    legacy::create_product_attribute_schema_group(
        token, tenant_slug, tenant_id, user_id, locale, draft,
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
) -> Result<bool, ApiError> {
    let context = GraphqlFallbackMutationContext::for_create_category_attribute_group(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        user_id.as_str(),
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
) -> Result<bool, ApiError> {
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
) -> Result<bool, ApiError> {
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
) -> Result<Vec<ProductAttributeValueItem>, ApiError> {
    let context = GraphqlFallbackMutationContext::for_save_product_attribute_values(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        user_id.as_str(),
    );
    legacy::save_product_attribute_values(
        token, tenant_slug, tenant_id, user_id, product_id, locale, patches,
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
) -> Result<Vec<ProductAttributeValueItem>, ApiError> {
    let context = GraphqlFallbackMutationContext::for_clear_detached_product_attribute_values(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        user_id.as_str(),
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

/// Image and variant writes keep their retry identity in the private gateway.
pub(crate) use legacy::{
    add_product_image, create_product_variant, delete_product_image, delete_product_variant,
    reorder_product_images, set_variant_axes, update_product_image, update_product_variant,
};
