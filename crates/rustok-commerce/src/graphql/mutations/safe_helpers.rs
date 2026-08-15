use async_graphql::{ErrorExtensions, Result};
use rustok_api::{
    AuthContext, PortActor, PortActorKind, PortContext, PortError, PortErrorKind, RequestContext,
};
use rustok_cart::CartStorefrontPort;
use rustok_customer::{CustomerUserProjectionRequest, in_process_customer_read_port};
use rustok_pricing::{PriceResolutionContext, PricingReadPort};
use uuid::Uuid;

use super::super::types::AddStorefrontCartLineItemInput;
pub(crate) use super::legacy_helpers::*;

const STOREFRONT_CART_HELPER_BOUNDARY: &str = "commerce_graphql_storefront_cart_helper";
const STOREFRONT_CUSTOMER_OWNER: &str = "rustok_customer";
const STOREFRONT_CUSTOMER_OWNER_OPERATION: &str = "read_customer_projection_by_user";

#[derive(Clone, Copy)]
struct StorefrontCustomerDiagnosticContext {
    tenant_id_shape: &'static str,
    actor_kind: &'static str,
    actor_id_shape: &'static str,
    claim_count: usize,
    role_count: usize,
    channel_shape: &'static str,
    locale_shape: &'static str,
    correlation_id_shape: &'static str,
    causation_id_shape: &'static str,
    traceparent_shape: &'static str,
    idempotency_key_shape: &'static str,
    deadline_ms: Option<u64>,
}

impl From<&PortContext> for StorefrontCustomerDiagnosticContext {
    fn from(context: &PortContext) -> Self {
        Self {
            tenant_id_shape: identity_text_shape(context.tenant_id.as_str()),
            actor_kind: actor_kind_name(&context.actor.kind),
            actor_id_shape: identity_text_shape(context.actor.id.as_str()),
            claim_count: context.claims.len(),
            role_count: context.roles.len(),
            channel_shape: optional_text_shape(context.channel.as_deref()),
            locale_shape: text_shape(context.locale.as_str()),
            correlation_id_shape: text_shape(context.correlation_id.as_str()),
            causation_id_shape: optional_text_shape(context.causation_id.as_deref()),
            traceparent_shape: optional_text_shape(context.traceparent.as_deref()),
            idempotency_key_shape: optional_text_shape(context.idempotency_key.as_deref()),
            deadline_ms: context.deadline_ms,
        }
    }
}

struct StorefrontCustomerDiagnosticError;

impl std::fmt::Debug for StorefrontCustomerDiagnosticError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("redacted")
    }
}

struct StorefrontCartPortDiagnosticError {
    code: String,
    kind: PortErrorKind,
    retryable: bool,
    message_shape: &'static str,
    message_len: usize,
}

impl std::fmt::Debug for StorefrontCartPortDiagnosticError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("redacted")
    }
}

struct StorefrontLegacyGraphqlDiagnosticError;

impl From<async_graphql::Error> for StorefrontLegacyGraphqlDiagnosticError {
    fn from(_error: async_graphql::Error) -> Self {
        Self
    }
}

impl std::fmt::Debug for StorefrontLegacyGraphqlDiagnosticError {
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

fn identity_text_shape(value: &str) -> &'static str {
    if value.is_empty() {
        return "empty";
    }
    match Uuid::parse_str(value) {
        Ok(value) if value.is_nil() => "uuid_nil",
        Ok(_) => "uuid_non_nil",
        Err(_) => "opaque",
    }
}

fn uuid_shape(value: Uuid) -> &'static str {
    if value.is_nil() { "nil" } else { "non_nil" }
}

fn optional_uuid_shape(value: Option<Uuid>) -> &'static str {
    match value {
        None => "absent",
        Some(value) if value.is_nil() => "present_nil",
        Some(_) => "present_non_nil",
    }
}

fn text_shape(value: &str) -> &'static str {
    if value.is_empty() { "empty" } else { "present" }
}

fn optional_text_shape(value: Option<&str>) -> &'static str {
    match value {
        None => "absent",
        Some("") => "empty",
        Some(_) => "present",
    }
}

fn public_graphql_error(
    message: &'static str,
    code: &'static str,
    retryable: bool,
) -> async_graphql::Error {
    async_graphql::Error::new(message).extend_with(|_, extensions| {
        extensions.set("code", code);
        extensions.set("retryable", retryable);
    })
}

fn storefront_customer_port_context(tenant_id: Uuid, user_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::user(user_id.to_string()),
        "en",
        format!("storefront-customer:{user_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2))
}

fn customer_port_graphql_error(
    context: &PortContext,
    consumer_operation: &'static str,
    error: PortError,
) -> async_graphql::Error {
    let (message, code, retryable) = match &error.kind {
        PortErrorKind::Validation => (
            "Customer request is invalid",
            "CUSTOMER_REQUEST_INVALID",
            false,
        ),
        PortErrorKind::NotFound => ("Customer was not found", "CUSTOMER_NOT_FOUND", false),
        PortErrorKind::Conflict => (
            "Customer state conflicts with the requested operation",
            "CUSTOMER_STATE_CONFLICT",
            false,
        ),
        PortErrorKind::Forbidden => (
            "Customer operation is not permitted",
            "CUSTOMER_ACCESS_DENIED",
            false,
        ),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
            "Customer information is temporarily unavailable",
            "CUSTOMER_TEMPORARILY_UNAVAILABLE",
            true,
        ),
        PortErrorKind::InvariantViolation => (
            "Customer operation could not be completed safely",
            "CUSTOMER_OPERATION_FAILED",
            false,
        ),
    };

    let technical = matches!(
        &error.kind,
        PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation
    );
    let diagnostic_context = StorefrontCustomerDiagnosticContext::from(context);
    let owner_code = error.code.clone();
    let owner_kind = error.kind.clone();
    let owner_retryable = error.retryable;
    let owner_message_shape = text_shape(error.message.as_str());
    let owner_message_len = error.message.len();
    let error = StorefrontCustomerDiagnosticError;

    if technical {
        tracing::error!(
            error = ?error,
            owner = STOREFRONT_CUSTOMER_OWNER,
            owner_operation = STOREFRONT_CUSTOMER_OWNER_OPERATION,
            consumer_operation,
            tenant_id_shape = diagnostic_context.tenant_id_shape,
            actor_kind = diagnostic_context.actor_kind,
            actor_id_shape = diagnostic_context.actor_id_shape,
            claim_count = diagnostic_context.claim_count,
            role_count = diagnostic_context.role_count,
            channel_shape = diagnostic_context.channel_shape,
            locale_shape = diagnostic_context.locale_shape,
            correlation_id_shape = diagnostic_context.correlation_id_shape,
            causation_id_shape = diagnostic_context.causation_id_shape,
            traceparent_shape = diagnostic_context.traceparent_shape,
            idempotency_key_shape = diagnostic_context.idempotency_key_shape,
            deadline_ms = ?diagnostic_context.deadline_ms,
            owner_code = %owner_code,
            owner_message_shape,
            owner_message_len,
            owner_kind = ?owner_kind,
            owner_retryable,
            public_code = code,
            public_retryable = retryable,
            boundary = STOREFRONT_CART_HELPER_BOUNDARY,
            "commerce GraphQL storefront customer owner port failed"
        );
    } else {
        tracing::warn!(
            error = ?error,
            owner = STOREFRONT_CUSTOMER_OWNER,
            owner_operation = STOREFRONT_CUSTOMER_OWNER_OPERATION,
            consumer_operation,
            tenant_id_shape = diagnostic_context.tenant_id_shape,
            actor_kind = diagnostic_context.actor_kind,
            actor_id_shape = diagnostic_context.actor_id_shape,
            claim_count = diagnostic_context.claim_count,
            role_count = diagnostic_context.role_count,
            channel_shape = diagnostic_context.channel_shape,
            locale_shape = diagnostic_context.locale_shape,
            correlation_id_shape = diagnostic_context.correlation_id_shape,
            causation_id_shape = diagnostic_context.causation_id_shape,
            traceparent_shape = diagnostic_context.traceparent_shape,
            idempotency_key_shape = diagnostic_context.idempotency_key_shape,
            deadline_ms = ?diagnostic_context.deadline_ms,
            owner_code = %owner_code,
            owner_message_shape,
            owner_message_len,
            owner_kind = ?owner_kind,
            owner_retryable,
            public_code = code,
            public_retryable = retryable,
            boundary = STOREFRONT_CART_HELPER_BOUNDARY,
            "commerce GraphQL storefront customer owner port was rejected"
        );
    }

    public_graphql_error(message, code, retryable)
}

fn cart_port_source_owner(error: &PortError) -> &'static str {
    match error.code.split_once('.') {
        Some(("cart", _)) => "rustok_cart",
        Some(("pricing", _)) => "rustok_pricing",
        _ => "unknown",
    }
}

pub(crate) fn cart_port_error(error: PortError) -> async_graphql::Error {
    let (message, code, retryable) = match &error.kind {
        PortErrorKind::Validation => ("Cart request is invalid", "CART_REQUEST_INVALID", false),
        PortErrorKind::NotFound => (
            "Cart resource was not found",
            "CART_RESOURCE_NOT_FOUND",
            false,
        ),
        PortErrorKind::Conflict => (
            "Cart operation conflicts with the current state",
            "CART_STATE_CONFLICT",
            false,
        ),
        PortErrorKind::Forbidden => (
            "Cart operation is not permitted",
            "CART_ACCESS_DENIED",
            false,
        ),
        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
            "Cart is temporarily unavailable",
            "CART_TEMPORARILY_UNAVAILABLE",
            true,
        ),
        PortErrorKind::InvariantViolation => (
            "Cart operation could not be completed safely",
            "CART_OPERATION_FAILED",
            false,
        ),
    };

    let source_owner = cart_port_source_owner(&error);
    let message_shape = text_shape(error.message.as_str());
    let message_len = error.message.len();
    let error = StorefrontCartPortDiagnosticError {
        code: error.code,
        kind: error.kind,
        retryable: error.retryable,
        message_shape,
        message_len,
    };

    tracing::error!(
        error = ?error,
        owner = "rustok_commerce.graphql_cart_helper",
        source_owner,
        operation = "storefront_cart_port",
        owner_code = %error.code,
        owner_message_shape = error.message_shape,
        owner_message_len = error.message_len,
        owner_kind = ?error.kind,
        owner_retryable = error.retryable,
        public_code = code,
        public_retryable = retryable,
        boundary = STOREFRONT_CART_HELPER_BOUNDARY,
        "commerce GraphQL storefront cart or pricing owner port failed"
    );

    public_graphql_error(message, code, retryable)
}

pub(crate) async fn resolve_optional_storefront_customer_id(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    auth: Option<&AuthContext>,
) -> Result<Option<Uuid>> {
    let Some(auth) = auth else {
        return Ok(None);
    };

    let customer_context = storefront_customer_port_context(tenant_id, auth.user_id);
    match in_process_customer_read_port(db.clone())
        .read_customer_projection_by_user(
            customer_context.clone(),
            CustomerUserProjectionRequest {
                user_id: auth.user_id,
            },
        )
        .await
    {
        Ok(customer) => Ok(Some(customer.id)),
        Err(error) if error.code == "customer.customer_by_user_not_found" => Ok(None),
        Err(error) => Err(customer_port_graphql_error(
            &customer_context,
            "resolve_optional_storefront_customer_id",
            error,
        )),
    }
}

fn legacy_graphql_error(
    error: async_graphql::Error,
    tenant_id: Uuid,
    resource_id: Option<Uuid>,
    operation: &'static str,
    message: &'static str,
    code: &'static str,
    retryable: bool,
) -> async_graphql::Error {
    let tenant_id_shape = uuid_shape(tenant_id);
    let resource_id_shape = optional_uuid_shape(resource_id);
    let error = StorefrontLegacyGraphqlDiagnosticError::from(error);

    tracing::error!(
        error = ?error,
        owner = "rustok_commerce.graphql_cart_helper",
        tenant_id_shape,
        resource_id_shape,
        operation,
        error_kind = "legacy_graphql_error",
        public_code = code,
        public_retryable = retryable,
        boundary = STOREFRONT_CART_HELPER_BOUNDARY,
        "commerce GraphQL storefront cart helper failed"
    );
    public_graphql_error(message, code, retryable)
}

pub(crate) async fn enrich_storefront_cart(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    request_context: &RequestContext,
    tenant_default_locale: &str,
    cart: crate::dto::CartResponse,
) -> Result<crate::dto::CartResponse> {
    let cart_id = cart.id;
    super::legacy_helpers::enrich_storefront_cart(
        db,
        tenant_id,
        request_context,
        tenant_default_locale,
        cart,
    )
    .await
    .map_err(|error| {
        legacy_graphql_error(
            error,
            tenant_id,
            Some(cart_id),
            "enrich_storefront_cart",
            "Cart shipping details are temporarily unavailable",
            "CART_ENRICHMENT_UNAVAILABLE",
            true,
        )
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn validate_selected_shipping_option(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    cart: &crate::dto::CartResponse,
    selected_shipping_option_id: Option<Uuid>,
    shipping_selections: Option<&[crate::dto::CartShippingSelectionInput]>,
    currency_code: &str,
    public_channel_slug: Option<&str>,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> Result<()> {
    super::legacy_helpers::validate_selected_shipping_option(
        db,
        tenant_id,
        cart,
        selected_shipping_option_id,
        shipping_selections,
        currency_code,
        public_channel_slug,
        requested_locale,
        tenant_default_locale,
    )
    .await
    .map_err(|error| {
        legacy_graphql_error(
            error,
            tenant_id,
            Some(cart.id),
            "validate_selected_shipping_option",
            "Selected shipping option is invalid",
            "SHIPPING_OPTION_INVALID",
            false,
        )
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn resolve_storefront_line_item_input(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    pricing_read_port: &dyn PricingReadPort,
    pricing_port_context: PortContext,
    pricing_context: &PriceResolutionContext,
    locale: &str,
    default_locale: &str,
    public_channel_slug: Option<&str>,
    input: AddStorefrontCartLineItemInput,
) -> Result<ResolvedStorefrontLineItemInput> {
    super::typed_line_item_helpers::resolve_storefront_line_item_input(
        db,
        tenant_id,
        pricing_read_port,
        pricing_port_context,
        pricing_context,
        locale,
        default_locale,
        public_channel_slug,
        input,
    )
    .await
}

pub(crate) async fn reprice_storefront_cart_line_items(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    request_context: &RequestContext,
    event_bus: &rustok_outbox::TransactionalEventBus,
    cart_storefront_port: &dyn CartStorefrontPort,
    cart: crate::dto::CartResponse,
) -> Result<crate::dto::CartResponse> {
    let cart_id = cart.id;
    super::legacy_helpers::reprice_storefront_cart_line_items(
        db,
        tenant_id,
        request_context,
        event_bus,
        cart_storefront_port,
        cart,
    )
    .await
    .map_err(|error| {
        legacy_graphql_error(
            error,
            tenant_id,
            Some(cart_id),
            "reprice_storefront_cart_line_items",
            "Cart pricing could not be refreshed",
            "CART_REPRICE_FAILED",
            true,
        )
    })
}

pub(crate) async fn validate_storefront_line_item_quantity(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    variant_id: Uuid,
    requested_quantity: i32,
    public_channel_slug: Option<&str>,
) -> Result<()> {
    super::typed_line_item_helpers::validate_storefront_line_item_quantity(
        db,
        tenant_id,
        variant_id,
        requested_quantity,
        public_channel_slug,
    )
    .await
}
