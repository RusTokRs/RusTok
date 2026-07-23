#[path = "storefront_checkout_runtime.rs"]
mod legacy;

pub use legacy::{
    StorefrontCheckoutCompletionCommand, StorefrontCheckoutRuntime,
    StorefrontCheckoutRuntimeError, StorefrontPaymentCollectionCommand,
    StorefrontShippingSelectionCommand, StorefrontShippingSelectionUpdateInput,
    create_storefront_payment_collection, read_storefront_order_refunds,
    read_storefront_payment_collection, select_storefront_shipping_option,
};

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
