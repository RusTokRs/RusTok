use rustok_graphql::GraphqlHttpError;
use uuid::Uuid;

const PRODUCT_ADMIN_GRAPHQL_OWNER: &str = "rustok_product.admin";
const PRODUCT_ADMIN_GRAPHQL_BOUNDARY: &str = "product_admin_primary_graphql_reads";
const PRODUCT_ADMIN_CATEGORY_GRAPHQL_BOUNDARY: &str =
    "product_admin_category_graphql_reads";
const PRODUCT_ADMIN_HTTP_PUBLIC_MESSAGE: &str =
    "Product admin service is temporarily unavailable";
const PRODUCT_ADMIN_GRAPHQL_PUBLIC_MESSAGE: &str =
    "Product admin request could not be completed";

pub(super) struct GraphqlReadContext {
    operation: &'static str,
    boundary: &'static str,
    correlation_id: String,
    token_present: bool,
    tenant_slug_length: Option<usize>,
    tenant_id_length: Option<usize>,
    resource_id_length: Option<usize>,
    category_id_length: Option<usize>,
    locale_length: Option<usize>,
    search_length: Option<usize>,
    status_length: Option<usize>,
    currency_code_length: Option<usize>,
    native_fallback_attempted: bool,
}

impl GraphqlReadContext {
    pub(super) fn for_bootstrap(token: Option<&str>, tenant_slug: Option<&str>) -> Self {
        Self::new("fetch_bootstrap", token, tenant_slug)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn for_products(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        locale: Option<&str>,
        search: Option<&str>,
        status: Option<&str>,
    ) -> Self {
        let mut context = Self::new("fetch_products", token, tenant_slug);
        context.tenant_id_length = Some(tenant_id.chars().count());
        context.locale_length = text_length(locale);
        context.search_length = text_length(search);
        context.status_length = text_length(status);
        context.native_fallback_attempted = true;
        context
    }

    pub(super) fn for_product(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        resource_id: &str,
        locale: Option<&str>,
    ) -> Self {
        let mut context = Self::new("fetch_product", token, tenant_slug);
        context.tenant_id_length = Some(tenant_id.chars().count());
        context.resource_id_length = Some(resource_id.chars().count());
        context.locale_length = text_length(locale);
        context
    }

    pub(super) fn for_product_pricing(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        resource_id: &str,
        locale: Option<&str>,
        currency_code: Option<&str>,
    ) -> Self {
        let mut context = Self::new("fetch_product_pricing", token, tenant_slug);
        context.tenant_id_length = Some(tenant_id.chars().count());
        context.resource_id_length = Some(resource_id.chars().count());
        context.locale_length = text_length(locale);
        context.currency_code_length = text_length(currency_code);
        context
    }

    pub(super) fn for_shipping_profiles(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
    ) -> Self {
        let mut context = Self::new("fetch_shipping_profiles", token, tenant_slug);
        context.tenant_id_length = Some(tenant_id.chars().count());
        context
    }

    pub(super) fn for_product_attributes(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        locale: &str,
    ) -> Self {
        Self::for_category_tenant_locale(
            "fetch_product_attributes",
            token,
            tenant_slug,
            tenant_id,
            locale,
        )
    }

    pub(super) fn for_catalog_categories(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        locale: &str,
    ) -> Self {
        Self::for_category_tenant_locale(
            "fetch_catalog_categories",
            token,
            tenant_slug,
            tenant_id,
            locale,
        )
    }

    pub(super) fn for_attribute_schemas(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        locale: &str,
    ) -> Self {
        Self::for_category_tenant_locale(
            "fetch_attribute_schemas",
            token,
            tenant_slug,
            tenant_id,
            locale,
        )
    }

    pub(super) fn for_effective_product_form(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        product_id: Option<&str>,
        category_id: Option<&str>,
        locale: &str,
    ) -> Self {
        let mut context = Self::for_category_tenant_locale(
            "fetch_effective_product_form",
            token,
            tenant_slug,
            tenant_id,
            locale,
        );
        context.resource_id_length = text_length(product_id);
        context.category_id_length = text_length(category_id);
        context
    }

    pub(super) fn for_product_attribute_values(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        product_id: &str,
        locale: &str,
    ) -> Self {
        let mut context = Self::for_category_tenant_locale(
            "fetch_product_attribute_values",
            token,
            tenant_slug,
            tenant_id,
            locale,
        );
        context.resource_id_length = Some(product_id.chars().count());
        context
    }

    fn for_category_tenant_locale(
        operation: &'static str,
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        locale: &str,
    ) -> Self {
        let mut context = Self::new(operation, token, tenant_slug);
        context.boundary = PRODUCT_ADMIN_CATEGORY_GRAPHQL_BOUNDARY;
        context.tenant_id_length = Some(tenant_id.chars().count());
        context.locale_length = Some(locale.chars().count());
        context.native_fallback_attempted = true;
        context
    }

    fn new(operation: &'static str, token: Option<&str>, tenant_slug: Option<&str>) -> Self {
        Self {
            operation,
            boundary: PRODUCT_ADMIN_GRAPHQL_BOUNDARY,
            correlation_id: format!("product-admin-graphql:{operation}:{}", Uuid::new_v4()),
            token_present: token.is_some(),
            tenant_slug_length: text_length(tenant_slug),
            tenant_id_length: None,
            resource_id_length: None,
            category_id_length: None,
            locale_length: None,
            search_length: None,
            status_length: None,
            currency_code_length: None,
            native_fallback_attempted: false,
        }
    }

    pub(super) fn map_error(&self, error: GraphqlHttpError) -> GraphqlHttpError {
        let (error_kind, code, public_error, technical_failure) = match &error {
            GraphqlHttpError::Network => (
                "network",
                "product.admin_graphql_network_unavailable",
                GraphqlHttpError::Network,
                true,
            ),
            GraphqlHttpError::Http(_) => (
                "http",
                "product.admin_graphql_http_unavailable",
                GraphqlHttpError::Http(PRODUCT_ADMIN_HTTP_PUBLIC_MESSAGE.to_string()),
                true,
            ),
            GraphqlHttpError::Unauthorized => (
                "unauthorized",
                "product.admin_graphql_authentication_required",
                GraphqlHttpError::Unauthorized,
                false,
            ),
            GraphqlHttpError::Graphql(_) => (
                "graphql",
                "product.admin_graphql_request_rejected",
                GraphqlHttpError::Graphql(PRODUCT_ADMIN_GRAPHQL_PUBLIC_MESSAGE.to_string()),
                false,
            ),
        };
        let error_payload_length = match &error {
            GraphqlHttpError::Http(value) | GraphqlHttpError::Graphql(value) => {
                Some(value.chars().count())
            }
            GraphqlHttpError::Network | GraphqlHttpError::Unauthorized => None,
        };
        let error_payload_present = error_payload_length.is_some_and(|length| length > 0);

        if technical_failure {
            tracing::error!(
                error_payload_present,
                error_payload_length = ?error_payload_length,
                owner = PRODUCT_ADMIN_GRAPHQL_OWNER,
                owner_operation = self.operation,
                correlation_id = %self.correlation_id,
                token_present = self.token_present,
                tenant_slug_present = self.tenant_slug_length.is_some(),
                tenant_slug_length = ?self.tenant_slug_length,
                tenant_id_present = self.tenant_id_length.is_some(),
                tenant_id_length = ?self.tenant_id_length,
                resource_id_present = self.resource_id_length.is_some(),
                resource_id_length = ?self.resource_id_length,
                category_id_present = self.category_id_length.is_some(),
                category_id_length = ?self.category_id_length,
                locale_present = self.locale_length.is_some(),
                locale_length = ?self.locale_length,
                search_present = self.search_length.is_some(),
                search_length = ?self.search_length,
                status_present = self.status_length.is_some(),
                status_length = ?self.status_length,
                currency_code_present = self.currency_code_length.is_some(),
                currency_code_length = ?self.currency_code_length,
                native_fallback_attempted = self.native_fallback_attempted,
                error_kind,
                code,
                boundary = self.boundary,
                "product admin GraphQL read failed"
            );
        } else {
            tracing::warn!(
                error_payload_present,
                error_payload_length = ?error_payload_length,
                owner = PRODUCT_ADMIN_GRAPHQL_OWNER,
                owner_operation = self.operation,
                correlation_id = %self.correlation_id,
                token_present = self.token_present,
                tenant_slug_present = self.tenant_slug_length.is_some(),
                tenant_slug_length = ?self.tenant_slug_length,
                tenant_id_present = self.tenant_id_length.is_some(),
                tenant_id_length = ?self.tenant_id_length,
                resource_id_present = self.resource_id_length.is_some(),
                resource_id_length = ?self.resource_id_length,
                category_id_present = self.category_id_length.is_some(),
                category_id_length = ?self.category_id_length,
                locale_present = self.locale_length.is_some(),
                locale_length = ?self.locale_length,
                search_present = self.search_length.is_some(),
                search_length = ?self.search_length,
                status_present = self.status_length.is_some(),
                status_length = ?self.status_length,
                currency_code_present = self.currency_code_length.is_some(),
                currency_code_length = ?self.currency_code_length,
                native_fallback_attempted = self.native_fallback_attempted,
                error_kind,
                code,
                boundary = self.boundary,
                "product admin GraphQL read was rejected"
            );
        }

        public_error
    }
}

fn text_length(value: Option<&str>) -> Option<usize> {
    value.map(|value| value.chars().count())
}
