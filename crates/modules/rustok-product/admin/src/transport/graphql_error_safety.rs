use rustok_graphql::GraphqlHttpError;
use uuid::Uuid;

pub(crate) const SELECTED_PRODUCT_ADMIN_TRANSPORT: &str =
    if cfg!(any(feature = "ssr", feature = "hydrate")) {
        "native"
    } else {
        "graphql"
    };

const SELECTED_PRODUCT_ADMIN_READ_BOUNDARY: &str =
    if cfg!(any(feature = "ssr", feature = "hydrate")) {
        "product_admin_native_reads"
    } else {
        "product_admin_graphql_reads"
    };

const SELECTED_PRODUCT_ADMIN_MUTATION_BOUNDARY: &str =
    if cfg!(any(feature = "ssr", feature = "hydrate")) {
        "product_admin_native_mutations"
    } else {
        "product_admin_graphql_mutations"
    };

pub(crate) struct GraphqlReadContext {
    operation: &'static str,
    boundary: &'static str,
    correlation_id: String,
    token_present: bool,
    tenant_slug_length: Option<usize>,
    tenant_id_length: Option<usize>,
    resource_id_length: Option<usize>,
    category_id_length: Option<usize>,
    vendor_length: Option<usize>,
    attribute_filter_count: Option<usize>,
    locale_length: Option<usize>,
    search_length: Option<usize>,
    status_length: Option<usize>,
    currency_code_length: Option<usize>,
    selected_transport: &'static str,
}

const PRODUCT_ADMIN_GRAPHQL_OWNER: &str = "rustok_product.admin";
const PRODUCT_ADMIN_GRAPHQL_BOUNDARY: &str = "product_admin_primary_graphql_reads";
const PRODUCT_ADMIN_HTTP_PUBLIC_MESSAGE: &str = "Product admin service is temporarily unavailable";
const PRODUCT_ADMIN_GRAPHQL_PUBLIC_MESSAGE: &str = "Product admin request could not be completed";

impl GraphqlReadContext {
    pub(crate) fn for_bootstrap(token: Option<&str>, tenant_slug: Option<&str>) -> Self {
        Self::new("fetch_bootstrap", token, tenant_slug)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn for_products(
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
        context.boundary = SELECTED_PRODUCT_ADMIN_READ_BOUNDARY;
        context.selected_transport = SELECTED_PRODUCT_ADMIN_TRANSPORT;
        context
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn for_admin_catalog_facets(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        locale: Option<&str>,
        search: Option<&str>,
        status: Option<&str>,
        vendor: Option<&str>,
        category_id: Option<&str>,
        attribute_filter_count: usize,
    ) -> Self {
        let mut context = Self::new("admin_catalog_facets", token, tenant_slug);
        context.tenant_id_length = Some(tenant_id.chars().count());
        context.locale_length = text_length(locale);
        context.search_length = text_length(search);
        context.status_length = text_length(status);
        context.vendor_length = text_length(vendor);
        context.category_id_length = text_length(category_id);
        context.attribute_filter_count = Some(attribute_filter_count);
        context.boundary = SELECTED_PRODUCT_ADMIN_READ_BOUNDARY;
        context.selected_transport = SELECTED_PRODUCT_ADMIN_TRANSPORT;
        context
    }

    pub(crate) fn for_product(
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

    #[expect(
        dead_code,
        reason = "reserved for future admin product pricing context"
    )]
    pub(crate) fn for_product_pricing(
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

    pub(crate) fn for_shipping_profiles(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
    ) -> Self {
        let mut context = Self::new("fetch_shipping_profiles", token, tenant_slug);
        context.tenant_id_length = Some(tenant_id.chars().count());
        context
    }

    pub(crate) fn for_product_attributes(
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

    pub(crate) fn for_attribute_schemas(
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

    pub(crate) fn for_catalog_categories(
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

    pub(crate) fn for_effective_product_form(
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

    pub(crate) fn for_product_attribute_values(
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
        context.boundary = SELECTED_PRODUCT_ADMIN_READ_BOUNDARY;
        context.selected_transport = SELECTED_PRODUCT_ADMIN_TRANSPORT;
        context.tenant_id_length = Some(tenant_id.chars().count());
        context.locale_length = Some(locale.chars().count());
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
            vendor_length: None,
            attribute_filter_count: None,
            selected_transport: "graphql",
        }
    }

    pub(crate) fn map_error(&self, error: GraphqlHttpError) -> GraphqlHttpError {
        let (error_kind, code, public_error, technical_failure) = match &error {
            GraphqlHttpError::Network => (
                "network",
                "product.admin_transport_network_unavailable",
                GraphqlHttpError::Network,
                true,
            ),
            GraphqlHttpError::Http(_) => (
                "http",
                "product.admin_transport_http_unavailable",
                GraphqlHttpError::Http(PRODUCT_ADMIN_HTTP_PUBLIC_MESSAGE.to_string()),
                true,
            ),
            GraphqlHttpError::Unauthorized => (
                "unauthorized",
                "product.admin_transport_authentication_required",
                GraphqlHttpError::Unauthorized,
                false,
            ),
            GraphqlHttpError::Graphql(_) => (
                "graphql",
                "product.admin_transport_request_rejected",
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
                correlation_id_present = !self.correlation_id.is_empty(),
                correlation_id_length = self.correlation_id.chars().count(),
                token_present = self.token_present,
                tenant_slug_present = self.tenant_slug_length.is_some(),
                tenant_slug_length = ?self.tenant_slug_length,
                tenant_id_present = self.tenant_id_length.is_some(),
                tenant_id_length = ?self.tenant_id_length,
                resource_id_present = self.resource_id_length.is_some(),
                resource_id_length = ?self.resource_id_length,
                category_id_present = self.category_id_length.is_some(),
                category_id_length = ?self.category_id_length,
                vendor_present = self.vendor_length.is_some(),
                vendor_length = ?self.vendor_length,
                attribute_filter_count = ?self.attribute_filter_count,
                locale_present = self.locale_length.is_some(),
                locale_length = ?self.locale_length,
                search_present = self.search_length.is_some(),
                search_length = ?self.search_length,
                status_present = self.status_length.is_some(),
                status_length = ?self.status_length,
                currency_code_present = self.currency_code_length.is_some(),
                currency_code_length = ?self.currency_code_length,
                selected_transport = self.selected_transport,
                error_kind,
                code,
                boundary = self.boundary,
                "product admin read failed"
            );
        } else {
            tracing::warn!(
                error_payload_present,
                error_payload_length = ?error_payload_length,
                owner = PRODUCT_ADMIN_GRAPHQL_OWNER,
                owner_operation = self.operation,
                correlation_id_present = !self.correlation_id.is_empty(),
                correlation_id_length = self.correlation_id.chars().count(),
                token_present = self.token_present,
                tenant_slug_present = self.tenant_slug_length.is_some(),
                tenant_slug_length = ?self.tenant_slug_length,
                tenant_id_present = self.tenant_id_length.is_some(),
                tenant_id_length = ?self.tenant_id_length,
                resource_id_present = self.resource_id_length.is_some(),
                resource_id_length = ?self.resource_id_length,
                category_id_present = self.category_id_length.is_some(),
                category_id_length = ?self.category_id_length,
                vendor_present = self.vendor_length.is_some(),
                vendor_length = ?self.vendor_length,
                attribute_filter_count = ?self.attribute_filter_count,
                locale_present = self.locale_length.is_some(),
                locale_length = ?self.locale_length,
                search_present = self.search_length.is_some(),
                search_length = ?self.search_length,
                status_present = self.status_length.is_some(),
                status_length = ?self.status_length,
                currency_code_present = self.currency_code_length.is_some(),
                currency_code_length = ?self.currency_code_length,
                selected_transport = self.selected_transport,
                error_kind,
                code,
                boundary = self.boundary,
                "product admin read was rejected"
            );
        }

        public_error
    }
}

/// Bounded private diagnostics for the primary Product Admin lifecycle writes.
///
/// The primary mutations are classified exactly like the primary reads: the
/// public envelope stays static and the captured payload is reduced to presence
/// and character length before it reaches structured tracing. The complete typed
/// error is not logged by this boundary.
pub(crate) struct GraphqlMutationContext {
    operation: &'static str,
    correlation_id: String,
    token_present: bool,
    tenant_slug_length: Option<usize>,
    tenant_id_length: usize,
    actor_id_length: usize,
    resource_id_length: Option<usize>,
    status_length: Option<usize>,
    draft_present: bool,
    selected_transport: &'static str,
}

impl GraphqlMutationContext {
    pub(crate) fn for_create_product(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        actor_id: &str,
        draft_present: bool,
    ) -> Self {
        let mut context = Self::new("create_product", token, tenant_slug, tenant_id, actor_id);
        context.draft_present = draft_present;
        context
    }

    pub(crate) fn for_update_product(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        actor_id: &str,
        resource_id: &str,
        draft_present: bool,
    ) -> Self {
        let mut context = Self::new("update_product", token, tenant_slug, tenant_id, actor_id);
        context.resource_id_length = Some(resource_id.chars().count());
        context.draft_present = draft_present;
        context
    }

    pub(crate) fn for_change_product_status(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        actor_id: &str,
        resource_id: &str,
        status: Option<&str>,
    ) -> Self {
        let mut context = Self::new(
            "change_product_status",
            token,
            tenant_slug,
            tenant_id,
            actor_id,
        );
        context.resource_id_length = Some(resource_id.chars().count());
        context.status_length = text_length(status);
        context
    }

    pub(crate) fn for_delete_product(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        actor_id: &str,
        resource_id: &str,
    ) -> Self {
        let mut context = Self::new("delete_product", token, tenant_slug, tenant_id, actor_id);
        context.resource_id_length = Some(resource_id.chars().count());
        context
    }

    fn new(
        operation: &'static str,
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        actor_id: &str,
    ) -> Self {
        Self {
            operation,
            correlation_id: format!("product-admin-mutation:{operation}:{}", Uuid::new_v4()),
            token_present: token.is_some(),
            tenant_slug_length: text_length(tenant_slug),
            tenant_id_length: tenant_id.chars().count(),
            actor_id_length: actor_id.chars().count(),
            resource_id_length: None,
            status_length: None,
            draft_present: false,
            selected_transport: SELECTED_PRODUCT_ADMIN_TRANSPORT,
        }
    }

    pub(crate) fn map_error(&self, error: GraphqlHttpError) -> GraphqlHttpError {
        let (error_kind, code, public_error, technical_failure) = match &error {
            GraphqlHttpError::Network => (
                "network",
                "product.admin_transport_network_unavailable",
                GraphqlHttpError::Network,
                true,
            ),
            GraphqlHttpError::Http(_) => (
                "http",
                "product.admin_transport_http_unavailable",
                GraphqlHttpError::Http(
                    "Product admin service is temporarily unavailable".to_string(),
                ),
                true,
            ),
            GraphqlHttpError::Unauthorized => (
                "unauthorized",
                "product.admin_transport_authentication_required",
                GraphqlHttpError::Unauthorized,
                false,
            ),
            GraphqlHttpError::Graphql(_) => (
                "graphql",
                "product.admin_transport_request_rejected",
                GraphqlHttpError::Graphql(
                    "Product admin request could not be completed".to_string(),
                ),
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
                correlation_id_present = !self.correlation_id.is_empty(),
                correlation_id_length = self.correlation_id.chars().count(),
                token_present = self.token_present,
                tenant_slug_present = self.tenant_slug_length.is_some(),
                tenant_slug_length = ?self.tenant_slug_length,
                tenant_id_length = self.tenant_id_length,
                actor_id_length = self.actor_id_length,
                resource_id_present = self.resource_id_length.is_some(),
                resource_id_length = ?self.resource_id_length,
                status_present = self.status_length.is_some(),
                status_length = ?self.status_length,
                draft_present = self.draft_present,
                error_kind,
                code,
                selected_transport = self.selected_transport,
                boundary = SELECTED_PRODUCT_ADMIN_MUTATION_BOUNDARY,
                "product admin mutation failed"
            );
        } else {
            tracing::warn!(
                error_payload_present,
                error_payload_length = ?error_payload_length,
                owner = PRODUCT_ADMIN_GRAPHQL_OWNER,
                owner_operation = self.operation,
                correlation_id_present = !self.correlation_id.is_empty(),
                correlation_id_length = self.correlation_id.chars().count(),
                token_present = self.token_present,
                tenant_slug_present = self.tenant_slug_length.is_some(),
                tenant_slug_length = ?self.tenant_slug_length,
                tenant_id_length = self.tenant_id_length,
                actor_id_length = self.actor_id_length,
                resource_id_present = self.resource_id_length.is_some(),
                resource_id_length = ?self.resource_id_length,
                status_present = self.status_length.is_some(),
                status_length = ?self.status_length,
                draft_present = self.draft_present,
                error_kind,
                code,
                selected_transport = self.selected_transport,
                boundary = SELECTED_PRODUCT_ADMIN_MUTATION_BOUNDARY,
                "product admin mutation was rejected"
            );
        }

        public_error
    }
}

fn text_length(value: Option<&str>) -> Option<usize> {
    value.map(|value| value.chars().count())
}
