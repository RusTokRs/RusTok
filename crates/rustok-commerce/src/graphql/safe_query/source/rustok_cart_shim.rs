use std::sync::Arc;

use ::rustok_api::{PortActorKind, PortContext, PortError, PortErrorKind};
use ::rustok_cart::CartStorefrontPort as OwnerCartStorefrontPort;
pub(crate) use ::rustok_cart::CartStorefrontReadRequest;
use ::sea_orm::DatabaseConnection;
use ::uuid::Uuid;

use super::super::query_error_boundary::{BoundaryError, QueryGraphqlMessage};

const GRAPHQL_QUERY_CART_BOUNDARY: &str = "commerce_graphql_query_cart";

struct CartQueryDiagnosticError;

impl std::fmt::Debug for CartQueryDiagnosticError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("redacted")
    }
}

fn actor_kind_name(kind: &PortActorKind) -> &'static str {
    match kind {
        PortActorKind::User => "user",
        PortActorKind::Service => "service",
        PortActorKind::System => "system",
    }
}

fn identity_shape(value: &str) -> &'static str {
    if value.is_empty() {
        return "empty";
    }
    match Uuid::parse_str(value) {
        Ok(value) if value.is_nil() => "uuid_nil",
        Ok(_) => "uuid_non_nil",
        Err(_) => "opaque",
    }
}

fn text_shape(value: &str) -> &'static str {
    if value.is_empty() { "empty" } else { "present" }
}

fn optional_text_shape(value: Option<&str>) -> &'static str {
    match value {
        None => "absent",
        Some(value) if value.is_empty() => "empty",
        Some(_) => "present",
    }
}

fn uuid_shape(value: &Uuid) -> &'static str {
    if value.is_nil() {
        "uuid_nil"
    } else {
        "uuid_non_nil"
    }
}

#[derive(Clone, Debug)]
struct CartQueryDiagnosticContext {
    tenant_id_shape: &'static str,
    actor_kind: &'static str,
    actor_id_shape: &'static str,
    claim_count: usize,
    role_count: usize,
    channel_shape: &'static str,
    locale_shape: &'static str,
    correlation_id: String,
    causation_id_shape: &'static str,
    traceparent_shape: &'static str,
    deadline_ms: Option<u64>,
    cart_id_shape: &'static str,
}

impl CartQueryDiagnosticContext {
    fn new(context: &PortContext, cart_id: &Uuid) -> Self {
        Self {
            tenant_id_shape: identity_shape(context.tenant_id.as_str()),
            actor_kind: actor_kind_name(&context.actor.kind),
            actor_id_shape: identity_shape(context.actor.id.as_str()),
            claim_count: context.claims.len(),
            role_count: context.roles.len(),
            channel_shape: optional_text_shape(context.channel.as_deref()),
            locale_shape: text_shape(context.locale.as_str()),
            correlation_id: context.correlation_id.clone(),
            causation_id_shape: optional_text_shape(context.causation_id.as_deref()),
            traceparent_shape: optional_text_shape(context.traceparent.as_deref()),
            deadline_ms: context.deadline_ms,
            cart_id_shape: uuid_shape(cart_id),
        }
    }
}

pub(crate) struct CartGraphqlMessage {
    error: PortError,
    context: CartQueryDiagnosticContext,
}

impl QueryGraphqlMessage for CartGraphqlMessage {
    fn into_query_boundary(self) -> BoundaryError {
        let (message, code, retryable, error_kind, technical) = match &self.error.kind {
            PortErrorKind::Validation => (
                "Cart query is invalid",
                "CART_REQUEST_INVALID",
                false,
                "validation",
                false,
            ),
            PortErrorKind::NotFound => (
                "Cart was not found",
                "CART_RESOURCE_NOT_FOUND",
                false,
                "not_found",
                false,
            ),
            PortErrorKind::Conflict => (
                "Cart state conflicts with this query",
                "CART_STATE_CONFLICT",
                false,
                "conflict",
                false,
            ),
            PortErrorKind::Forbidden => (
                "Cart query is not permitted",
                "CART_ACCESS_DENIED",
                false,
                "forbidden",
                false,
            ),
            PortErrorKind::Unavailable | PortErrorKind::Timeout => (
                "Cart data is temporarily unavailable",
                "CART_TEMPORARILY_UNAVAILABLE",
                true,
                "unavailable",
                true,
            ),
            PortErrorKind::InvariantViolation => (
                "Cart query could not be completed safely",
                "CART_OPERATION_FAILED",
                false,
                "invariant",
                true,
            ),
        };
        let owner_message_shape = text_shape(self.error.message.as_str());
        let owner_message_length = self.error.message.chars().count();
        let diagnostic_error = CartQueryDiagnosticError;
        if technical {
            tracing::error!(
                error = ?diagnostic_error,
                owner = "rustok_cart",
                owner_operation = "read_storefront_cart",
                tenant_id_shape = self.context.tenant_id_shape,
                actor_kind = self.context.actor_kind,
                actor_id_shape = self.context.actor_id_shape,
                claim_count = self.context.claim_count,
                role_count = self.context.role_count,
                channel_shape = self.context.channel_shape,
                locale_shape = self.context.locale_shape,
                correlation_id = %self.context.correlation_id,
                causation_id_shape = self.context.causation_id_shape,
                traceparent_shape = self.context.traceparent_shape,
                deadline_ms = ?self.context.deadline_ms,
                cart_id_shape = self.context.cart_id_shape,
                error_kind,
                owner_code = %self.error.code,
                owner_message_shape,
                owner_message_length,
                owner_retryable = self.error.retryable,
                public_code = code,
                retryable,
                boundary = GRAPHQL_QUERY_CART_BOUNDARY,
                "commerce GraphQL cart query failed"
            );
        } else {
            tracing::warn!(
                error = ?diagnostic_error,
                owner = "rustok_cart",
                owner_operation = "read_storefront_cart",
                tenant_id_shape = self.context.tenant_id_shape,
                actor_kind = self.context.actor_kind,
                actor_id_shape = self.context.actor_id_shape,
                claim_count = self.context.claim_count,
                role_count = self.context.role_count,
                channel_shape = self.context.channel_shape,
                locale_shape = self.context.locale_shape,
                correlation_id = %self.context.correlation_id,
                causation_id_shape = self.context.causation_id_shape,
                traceparent_shape = self.context.traceparent_shape,
                deadline_ms = ?self.context.deadline_ms,
                cart_id_shape = self.context.cart_id_shape,
                error_kind,
                owner_code = %self.error.code,
                owner_message_shape,
                owner_message_length,
                owner_retryable = self.error.retryable,
                public_code = code,
                retryable,
                boundary = GRAPHQL_QUERY_CART_BOUNDARY,
                "commerce GraphQL cart query was rejected"
            );
        }
        BoundaryError::Public {
            message,
            code,
            retryable,
        }
    }
}

impl From<CartGraphqlMessage> for BoundaryError {
    fn from(message: CartGraphqlMessage) -> Self {
        message.into_query_boundary()
    }
}

pub(crate) struct CartQueryPortError {
    pub(crate) code: String,
    pub(crate) message: CartGraphqlMessage,
}

impl CartQueryPortError {
    fn new(error: PortError, context: CartQueryDiagnosticContext) -> Self {
        Self {
            code: error.code.clone(),
            message: CartGraphqlMessage { error, context },
        }
    }
}

/// Compatibility facade for unchanged Commerce GraphQL storefront cart reads.
///
/// The resolver can keep its legacy `error.code` not-found guard and
/// `error.message` conversion while the complete typed `PortError` remains owned by
/// the transport mapper.
pub(crate) struct CartStorefrontQueryPort {
    inner: Arc<dyn OwnerCartStorefrontPort>,
}

pub(crate) fn in_process_cart_storefront_port(db: DatabaseConnection) -> CartStorefrontQueryPort {
    CartStorefrontQueryPort {
        inner: ::rustok_cart::in_process_cart_storefront_port(db),
    }
}

impl CartStorefrontQueryPort {
    pub(crate) async fn read_storefront_cart(
        &self,
        context: PortContext,
        request: CartStorefrontReadRequest,
    ) -> Result<::rustok_cart::CartResponse, CartQueryPortError> {
        let diagnostic_context = CartQueryDiagnosticContext::new(&context, &request.cart_id);
        self.inner
            .read_storefront_cart(context, request)
            .await
            .map_err(|error| CartQueryPortError::new(error, diagnostic_context))
    }
}
