#[path = "transport.rs"]
mod legacy;
#[path = "transport/admin_catalog_graphql.rs"]
mod admin_catalog_graphql;
#[path = "transport/admin_catalog_native.rs"]
mod admin_catalog_native;
#[path = "transport/graphql_error_safety.rs"]
mod graphql_error_safety;
#[path = "transport/graphql_fallback_mutation_error_safety.rs"]
mod graphql_fallback_mutation_error_safety;
#[path = "transport/graphql_fallback_mutations.rs"]
mod graphql_fallback_mutations;

use crate::catalog_controls::{ProductAdminListInput, build_product_admin_list_input};
use crate::model::{
    CatalogCategoryList, ProductAdminBootstrap, ProductAttributeList, ProductAttributeSchemaList,
    ProductAttributeValueItem, ProductCatalogSearchOptions, ProductDetail, ProductDraft,
    ProductEffectiveForm, ProductList, ProductPricingDetail, ShippingProfileList,
};
use rustok_graphql::GraphqlHttpError;

const PRODUCT_ADMIN_CATALOG_OPTIONS_OWNER: &str = "rustok_product.admin";
const PRODUCT_ADMIN_CATALOG_OPTIONS_OPERATION: &str = "fetch_catalog_search_options";
const PRODUCT_ADMIN_CATALOG_OPTIONS_BOUNDARY: &str =
    "product_admin_catalog_search_options_graphql_fallback";
const PRODUCT_ADMIN_CATALOG_OPTIONS_PUBLIC_MESSAGE: &str =
    "Product catalog search options are temporarily unavailable";

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
            owner = PRODUCT_ADMIN_CATALOG_OPTIONS_OWNER,
            owner_operation = PRODUCT_ADMIN_CATALOG_OPTIONS_OPERATION,
            correlation_id = %self.correlation_id,
            token_present = self.token_present,
            tenant_slug_present = self.tenant_slug_length.is_some(),
            tenant_slug_length = ?self.tenant_slug_length,
            locale_length = self.locale_length,
            code = "product.admin_catalog_search_options_graphql_unavailable",
            boundary = PRODUCT_ADMIN_CATALOG_OPTIONS_BOUNDARY,
            "product admin catalog search options GraphQL fallback failed"
        );

        PRODUCT_ADMIN_CATALOG_OPTIONS_PUBLIC_MESSAGE.to_string()
    }
}

pub(crate) async fn fetch_bootstrap(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<ProductAdminBootstrap, GraphqlHttpError> {
    let context = graphql_error_safety::GraphqlReadContext::for_bootstrap(
        token.as_deref(),
        tenant_slug.as_deref(),
    );
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
    search: Option<String>,
    status: Option<String>,
) -> Result<ProductList, GraphqlHttpError> {
    let route_controls = leptos::prelude::use_context::<ProductAdminListInput>().unwrap_or_default();
    let attribute_filters = if route_controls.attribute_filters.is_empty() {
        None
    } else {
        Some(route_controls.attribute_filters.join(";"))
    };
    let controls = build_product_admin_list_input(
        search,
        status,
        route_controls.category_id,
        route_controls.sort_by,
        route_controls.sort_direction,
        attribute_filters,
    );
    let native_controls = controls.clone();
    match admin_catalog_native::fetch_products(
        tenant_id.clone(),
        locale.clone(),
        native_controls,
    )
    .await
    {
        Ok(value) => Ok(value),
        Err(_) => {
            let context = graphql_error_safety::GraphqlReadContext::for_products(
                token.as_deref(),
                tenant_slug.as_deref(),
                tenant_id.as_str(),
                locale.as_deref(),
                controls.search.as_deref(),
                controls.status.as_deref(),
            );
            admin_catalog_graphql::fetch_products(
                token,
                tenant_slug,
                tenant_id,
                locale,
                controls,
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
) -> Result<Option<ProductDetail>, GraphqlHttpError> {
    let context = graphql_error_safety::GraphqlReadContext::for_product(
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
) -> Result<Option<ProductPricingDetail>, GraphqlHttpError> {
    let context = graphql_error_safety::GraphqlReadContext::for_product_pricing(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        id.as_str(),
        locale.as_deref(),
        currency_code.as_deref(),
    );
    legacy::fetch_product_pricing(
        token,
        tenant_slug,
        tenant_id,
        id,
        locale,
        currency_code,
    )
    .await
    .map_err(|error| context.map_error(error))
}

pub(crate) async fn fetch_shipping_profiles(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
) -> Result<ShippingProfileList, GraphqlHttpError> {
    let context = graphql_error_safety::GraphqlReadContext::for_shipping_profiles(
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
) -> Result<ProductAttributeList, GraphqlHttpError> {
    let context = graphql_error_safety::GraphqlReadContext::for_product_attributes(
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
) -> Result<CatalogCategoryList, GraphqlHttpError> {
    let context = graphql_error_safety::GraphqlReadContext::for_catalog_categories(
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
) -> Result<ProductAttributeSchemaList, GraphqlHttpError> {
    let context = graphql_error_safety::GraphqlReadContext::for_attribute_schemas(
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
) -> Result<Option<ProductEffectiveForm>, GraphqlHttpError> {
    let context = graphql_error_safety::GraphqlReadContext::for_effective_product_form(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        product_id.as_deref(),
        category_id.as_deref(),
        locale.as_str(),
    );
    legacy::fetch_effective_product_form(
        token,
        tenant_slug,
        tenant_id,
        product_id,
        category_id,
        locale,
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
) -> Result<Vec<ProductAttributeValueItem>, GraphqlHttpError> {
    let context = graphql_error_safety::GraphqlReadContext::for_product_attribute_values(
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
) -> Result<ProductDetail, GraphqlHttpError> {
    let context = graphql_error_safety::GraphqlMutationContext::for_create_product(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        user_id.as_str(),
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
) -> Result<ProductDetail, GraphqlHttpError> {
    let context = graphql_error_safety::GraphqlMutationContext::for_update_product(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        user_id.as_str(),
        id.as_str(),
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
) -> Result<ProductDetail, GraphqlHttpError> {
    let context = graphql_error_safety::GraphqlMutationContext::for_change_product_status(
        token.as_deref(),
        tenant_slug.as_deref(),
        tenant_id.as_str(),
        user_id.as_str(),
        id.as_str(),
        status,
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
) -> Result<bool, GraphqlHttpError> {
    let context = graphql_error_safety::GraphqlMutationContext::for_delete_product(
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
