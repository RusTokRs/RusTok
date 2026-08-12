mod legacy {
    include!("storefront_checkout_runtime.rs");
}

pub use legacy::{
    StorefrontCheckoutCompletionCommand, StorefrontCheckoutRuntime,
    StorefrontPaymentCollectionCommand, StorefrontShippingSelectionCommand,
    StorefrontShippingSelectionUpdateInput,
};

const STOREFRONT_RUNTIME_BOUNDARY: &str = "commerce_storefront_checkout_runtime";

#[derive(Clone, Copy)]
struct MountedRuntimePublicPolicy {
    message: &'static str,
    code: &'static str,
    retryable: bool,
}

const PAYMENT_COLLECTION_READ_POLICY: MountedRuntimePublicPolicy = MountedRuntimePublicPolicy {
    message: "Storefront payment collection is temporarily unavailable",
    code: "STOREFRONT_PAYMENT_COLLECTION_UNAVAILABLE",
    retryable: true,
};

const REFUND_SUMMARY_READ_POLICY: MountedRuntimePublicPolicy = MountedRuntimePublicPolicy {
    message: "Storefront refund summary is temporarily unavailable",
    code: "STOREFRONT_REFUND_SUMMARY_UNAVAILABLE",
    retryable: true,
};

const PAYMENT_COLLECTION_CREATE_POLICY: MountedRuntimePublicPolicy = MountedRuntimePublicPolicy {
    message: "Storefront payment collection is temporarily unavailable",
    code: "STOREFRONT_PAYMENT_COLLECTION_CREATE_FAILED",
    retryable: true,
};

const SHIPPING_SELECTION_POLICY: MountedRuntimePublicPolicy = MountedRuntimePublicPolicy {
    message: "Shipping selection is temporarily unavailable",
    code: "STOREFRONT_SHIPPING_SELECTION_FAILED",
    retryable: true,
};

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct StorefrontCheckoutRuntimeError {
    message: &'static str,
    code: &'static str,
    retryable: bool,
}

impl StorefrontCheckoutRuntimeError {
    fn from_policy(policy: MountedRuntimePublicPolicy) -> Self {
        Self {
            message: policy.message,
            code: policy.code,
            retryable: policy.retryable,
        }
    }

    pub const fn public_message(&self) -> &'static str {
        self.message
    }

    pub const fn public_code(&self) -> &'static str {
        self.code
    }

    pub const fn retryable(&self) -> bool {
        self.retryable
    }
}

struct MountedRuntimeDiagnosticError;

impl std::fmt::Debug for MountedRuntimeDiagnosticError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("redacted")
    }
}

#[derive(Clone, Copy)]
struct MountedRuntimeErrorContext {
    tenant_id_non_nil: bool,
    auth_present: bool,
    resource_id_non_nil: bool,
    request_context_present: bool,
    channel_id_present: bool,
    channel_id_non_nil: Option<bool>,
    channel_slug_present: bool,
    channel_slug_length: Option<usize>,
    locale_present: bool,
    locale_length: Option<usize>,
    operation: &'static str,
}

impl MountedRuntimeErrorContext {
    fn new(
        tenant_id: uuid::Uuid,
        auth_present: bool,
        resource_id: uuid::Uuid,
        request_context: Option<&rustok_api::RequestContext>,
        operation: &'static str,
    ) -> Self {
        Self {
            tenant_id_non_nil: !tenant_id.is_nil(),
            auth_present,
            resource_id_non_nil: !resource_id.is_nil(),
            request_context_present: request_context.is_some(),
            channel_id_present: request_context.is_some_and(|context| context.channel_id.is_some()),
            channel_id_non_nil: request_context
                .and_then(|context| context.channel_id)
                .map(|value| !value.is_nil()),
            channel_slug_present: request_context
                .is_some_and(|context| context.channel_slug.is_some()),
            channel_slug_length: request_context
                .and_then(|context| context.channel_slug.as_ref())
                .map(|value| value.chars().count()),
            locale_present: request_context
                .is_some_and(|context| !context.locale.trim().is_empty()),
            locale_length: request_context.map(|context| context.locale.chars().count()),
            operation,
        }
    }
}

fn map_legacy_runtime_error(
    context: MountedRuntimeErrorContext,
    policy: MountedRuntimePublicPolicy,
    _error: legacy::StorefrontCheckoutRuntimeError,
) -> StorefrontCheckoutRuntimeError {
    let diagnostic_error = MountedRuntimeDiagnosticError;
    let legacy_error_type = std::any::type_name::<legacy::StorefrontCheckoutRuntimeError>();

    tracing::error!(
        error = ?diagnostic_error,
        legacy_error_type,
        owner = "rustok_commerce.storefront_checkout_runtime",
        tenant_id_non_nil = context.tenant_id_non_nil,
        auth_present = context.auth_present,
        resource_id_non_nil = context.resource_id_non_nil,
        request_context_present = context.request_context_present,
        channel_id_present = context.channel_id_present,
        channel_id_non_nil = ?context.channel_id_non_nil,
        channel_slug_present = context.channel_slug_present,
        channel_slug_length = ?context.channel_slug_length,
        locale_present = context.locale_present,
        locale_length = ?context.locale_length,
        operation = context.operation,
        public_code = policy.code,
        public_retryable = policy.retryable,
        boundary = STOREFRONT_RUNTIME_BOUNDARY,
        "mounted storefront checkout runtime dependency failed"
    );

    StorefrontCheckoutRuntimeError::from_policy(policy)
}

pub async fn read_storefront_payment_collection(
    runtime: &StorefrontCheckoutRuntime,
    tenant: &rustok_api::TenantContext,
    auth: rustok_api::OptionalAuthContext,
    cart_id: uuid::Uuid,
) -> Result<Option<rustok_payment::dto::PaymentCollectionResponse>, StorefrontCheckoutRuntimeError>
{
    let error_context = MountedRuntimeErrorContext::new(
        tenant.id,
        auth.0.is_some(),
        cart_id,
        None,
        "read_storefront_payment_collection",
    );

    legacy::read_storefront_payment_collection(runtime, tenant, auth, cart_id)
        .await
        .map_err(|error| {
            map_legacy_runtime_error(error_context, PAYMENT_COLLECTION_READ_POLICY, error)
        })
}

pub async fn read_storefront_order_refunds(
    runtime: &StorefrontCheckoutRuntime,
    tenant: &rustok_api::TenantContext,
    request_context: &rustok_api::RequestContext,
    auth: rustok_api::OptionalAuthContext,
    order_id: uuid::Uuid,
) -> Result<(Vec<rustok_payment::dto::RefundResponse>, u64), StorefrontCheckoutRuntimeError> {
    let error_context = MountedRuntimeErrorContext::new(
        tenant.id,
        auth.0.is_some(),
        order_id,
        Some(request_context),
        "read_storefront_order_refunds",
    );

    legacy::read_storefront_order_refunds(runtime, tenant, request_context, auth, order_id)
        .await
        .map_err(|error| map_legacy_runtime_error(error_context, REFUND_SUMMARY_READ_POLICY, error))
}

pub async fn create_storefront_payment_collection(
    runtime: &StorefrontCheckoutRuntime,
    tenant: &rustok_api::TenantContext,
    request_context: &rustok_api::RequestContext,
    auth: rustok_api::OptionalAuthContext,
    command: StorefrontPaymentCollectionCommand,
) -> Result<rustok_payment::dto::PaymentCollectionResponse, StorefrontCheckoutRuntimeError> {
    let error_context = MountedRuntimeErrorContext::new(
        tenant.id,
        auth.0.is_some(),
        command.cart_id,
        Some(request_context),
        "create_storefront_payment_collection",
    );

    legacy::create_storefront_payment_collection(runtime, tenant, request_context, auth, command)
        .await
        .map_err(|error| {
            map_legacy_runtime_error(error_context, PAYMENT_COLLECTION_CREATE_POLICY, error)
        })
}

pub async fn select_storefront_shipping_option(
    runtime: &StorefrontCheckoutRuntime,
    tenant: &rustok_api::TenantContext,
    request_context: Option<&rustok_api::RequestContext>,
    auth: rustok_api::OptionalAuthContext,
    command: StorefrontShippingSelectionCommand,
) -> Result<(), StorefrontCheckoutRuntimeError> {
    let error_context = MountedRuntimeErrorContext::new(
        tenant.id,
        auth.0.is_some(),
        command.cart_id,
        request_context,
        "select_storefront_shipping_option",
    );

    legacy::select_storefront_shipping_option(runtime, tenant, request_context, auth, command)
        .await
        .map_err(|error| map_legacy_runtime_error(error_context, SHIPPING_SELECTION_POLICY, error))
}

/// Mounted storefront completion boundary.
///
/// The legacy runtime remains available only as an internal compatibility
/// submodule for its non-checkout helpers. Checkout completion itself always
/// enters the durable staged owner-port pipeline with an explicit provider
/// registry and caller-supplied idempotency identity.
pub async fn complete_storefront_checkout(
    runtime: &StorefrontCheckoutRuntime,
    payment_provider_registry: rustok_payment::providers::PaymentProviderRegistry,
    tenant: &rustok_api::TenantContext,
    request_context: &rustok_api::RequestContext,
    auth: rustok_api::OptionalAuthContext,
    idempotency_key: impl Into<String>,
    command: StorefrontCheckoutCompletionCommand,
) -> Result<
    crate::dto::CompleteCheckoutResponse,
    crate::services::storefront_staged_checkout_runtime::StorefrontStagedCheckoutRuntimeError,
> {
    crate::services::storefront_staged_checkout_runtime::complete_storefront_checkout(
        runtime,
        payment_provider_registry,
        tenant,
        request_context,
        auth,
        idempotency_key,
        command,
    )
    .await
}
