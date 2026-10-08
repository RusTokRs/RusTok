//! Bounded private diagnostics for the Product Admin GraphQL fallback mutations.
//!
//! These eleven commands keep a native-first server-function path. This context
//! is created before the compatibility executor runs and maps only the final
//! returned error. The public envelope is static; the captured payload is reduced
//! to presence and character length before it reaches structured tracing.

use rustok_graphql::GraphqlHttpError;
use uuid::Uuid;

const PRODUCT_ADMIN_GRAPHQL_OWNER: &str = "rustok_product.admin";
const PRODUCT_ADMIN_FALLBACK_MUTATION_GRAPHQL_BOUNDARY: &str =
    "product_admin_fallback_graphql_mutations";
const PRODUCT_ADMIN_HTTP_PUBLIC_MESSAGE: &str = "Product admin service is temporarily unavailable";
const PRODUCT_ADMIN_GRAPHQL_PUBLIC_MESSAGE: &str = "Product admin request could not be completed";

fn text_length(value: Option<&str>) -> Option<usize> {
    value.map(|value| value.chars().count())
}

pub(super) struct GraphqlFallbackMutationContext {
    operation: &'static str,
    correlation_id: String,
    token_present: bool,
    tenant_slug_length: Option<usize>,
    tenant_id_length: usize,
    actor_id_length: usize,
    resource_id_length: Option<usize>,
    locale_length: Option<usize>,
    item_count: Option<usize>,
    input_present: bool,
    native_fallback_attempted: bool,
}

impl GraphqlFallbackMutationContext {
    pub(super) fn for_create_product_attribute(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        actor_id: &str,
    ) -> Self {
        Self::new(
            "create_product_attribute",
            token,
            tenant_slug,
            tenant_id,
            actor_id,
        )
    }

    pub(super) fn for_create_product_attribute_option(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        actor_id: &str,
    ) -> Self {
        Self::new(
            "create_product_attribute_option",
            token,
            tenant_slug,
            tenant_id,
            actor_id,
        )
    }

    pub(super) fn for_create_catalog_category(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        actor_id: &str,
    ) -> Self {
        Self::new(
            "create_catalog_category",
            token,
            tenant_slug,
            tenant_id,
            actor_id,
        )
    }

    pub(super) fn for_create_attribute_schema(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        actor_id: &str,
    ) -> Self {
        Self::new(
            "create_attribute_schema",
            token,
            tenant_slug,
            tenant_id,
            actor_id,
        )
    }

    pub(super) fn for_set_category_schema_mode(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        actor_id: &str,
    ) -> Self {
        Self::new(
            "set_category_schema_mode",
            token,
            tenant_slug,
            tenant_id,
            actor_id,
        )
    }

    pub(super) fn for_create_product_attribute_schema_group(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        actor_id: &str,
    ) -> Self {
        Self::new(
            "create_product_attribute_schema_group",
            token,
            tenant_slug,
            tenant_id,
            actor_id,
        )
    }

    pub(super) fn for_create_category_attribute_group(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        actor_id: &str,
    ) -> Self {
        Self::new(
            "create_category_attribute_group",
            token,
            tenant_slug,
            tenant_id,
            actor_id,
        )
    }

    pub(super) fn for_bind_schema_attribute(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        actor_id: &str,
    ) -> Self {
        Self::new(
            "bind_schema_attribute",
            token,
            tenant_slug,
            tenant_id,
            actor_id,
        )
    }

    pub(super) fn for_bind_category_attribute(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        actor_id: &str,
    ) -> Self {
        Self::new(
            "bind_category_attribute",
            token,
            tenant_slug,
            tenant_id,
            actor_id,
        )
    }

    pub(super) fn for_save_product_attribute_values(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        actor_id: &str,
    ) -> Self {
        Self::new(
            "save_product_attribute_values",
            token,
            tenant_slug,
            tenant_id,
            actor_id,
        )
    }

    pub(super) fn for_clear_detached_product_attribute_values(
        token: Option<&str>,
        tenant_slug: Option<&str>,
        tenant_id: &str,
        actor_id: &str,
    ) -> Self {
        Self::new(
            "clear_detached_product_attribute_values",
            token,
            tenant_slug,
            tenant_id,
            actor_id,
        )
    }

    /// Adds the request shape known to the wrapper: character lengths and
    /// collection counts only, never the values themselves.
    pub(super) fn with_request_shape(
        mut self,
        locale: Option<&str>,
        resource_id: Option<&str>,
        item_count: Option<usize>,
        input_present: bool,
    ) -> Self {
        self.locale_length = text_length(locale);
        self.resource_id_length = text_length(resource_id);
        self.item_count = item_count;
        self.input_present = input_present;
        self
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
            correlation_id: format!(
                "product-admin-fallback-mutation:{operation}:{}",
                Uuid::new_v4()
            ),
            token_present: token.is_some(),
            tenant_slug_length: text_length(tenant_slug),
            tenant_id_length: tenant_id.chars().count(),
            actor_id_length: actor_id.chars().count(),
            resource_id_length: None,
            locale_length: None,
            item_count: None,
            input_present: false,
            // The wrapper maps only a final error, i.e. the native path failed
            // and the single GraphQL fallback failed as well.
            native_fallback_attempted: true,
        }
    }

    /// Maps the final fallback error to a static public envelope and records
    /// bounded diagnostics. The complete typed error is not logged.
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
                tenant_id_length = self.tenant_id_length,
                actor_id_length = self.actor_id_length,
                resource_id_present = self.resource_id_length.is_some(),
                resource_id_length = ?self.resource_id_length,
                locale_present = self.locale_length.is_some(),
                locale_length = ?self.locale_length,
                item_count = ?self.item_count,
                input_present = self.input_present,
                native_fallback_attempted = self.native_fallback_attempted,
                error_kind,
                code,
                boundary = PRODUCT_ADMIN_FALLBACK_MUTATION_GRAPHQL_BOUNDARY,
                "product admin fallback GraphQL mutation failed"
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
                tenant_id_length = self.tenant_id_length,
                actor_id_length = self.actor_id_length,
                resource_id_present = self.resource_id_length.is_some(),
                resource_id_length = ?self.resource_id_length,
                locale_present = self.locale_length.is_some(),
                locale_length = ?self.locale_length,
                item_count = ?self.item_count,
                input_present = self.input_present,
                native_fallback_attempted = self.native_fallback_attempted,
                error_kind,
                code,
                boundary = PRODUCT_ADMIN_FALLBACK_MUTATION_GRAPHQL_BOUNDARY,
                "product admin fallback GraphQL mutation was rejected"
            );
        }

        public_error
    }
}
