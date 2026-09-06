use rustok_api::{
    AuthContext, OptionalAuthContext, PortActor, PortContext, PortError, PortErrorKind,
    RequestContext, TenantContext,
};
use rustok_cart::{
    CartStorefrontReadRequest, PrepareCartCheckoutSnapshotRequest,
    bind_in_process_atomic_cart_checkout_with_pricing, in_process_cart_checkout_port,
    in_process_cart_storefront_port,
};
use rustok_customer::{CustomerUserProjectionRequest, in_process_customer_read_port};
use rustok_payment::providers::PaymentProviderRegistry;
use std::{sync::Arc, time::Duration};
use thiserror::Error;
use uuid::Uuid;

use crate::storefront_checkout_runtime::{
    StorefrontCheckoutCompletionCommand, StorefrontCheckoutRuntime,
};

const STOREFRONT_STAGED_CHECKOUT_OWNER: &str = "rustok_commerce.recovering_staged_checkout";
const STOREFRONT_STAGED_CHECKOUT_CART_OWNER: &str = "rustok_cart";
const STOREFRONT_STAGED_CHECKOUT_CUSTOMER_OWNER: &str = "rustok_customer";
const STOREFRONT_STAGED_CHECKOUT_BOUNDARY: &str = "commerce_storefront_staged_checkout_runtime";

#[derive(Debug, Error)]
pub enum StorefrontStagedCheckoutRuntimeError {
    #[error("checkout request is invalid")]
    Validation(String),
    #[error("checkout cart is not accessible")]
    CartAccess,
    #[error("authentication required for customer-owned cart")]
    AuthenticationRequired,
    #[error("checkout dependency is temporarily unavailable")]
    TemporarilyUnavailable,
    #[error("checkout could not be completed")]
    CheckoutFailed,
    #[error("checkout compensation is pending")]
    CompensationPending,
    #[error("checkout requires reconciliation")]
    ReconciliationRequired,
}

impl StorefrontStagedCheckoutRuntimeError {
    pub const fn public_code(&self) -> &'static str {
        match self {
            Self::Validation(_) => "checkout_operation_invalid",
            Self::CartAccess => "checkout_cart_not_accessible",
            Self::AuthenticationRequired => "checkout_authentication_required",
            Self::TemporarilyUnavailable => "checkout_temporarily_unavailable",
            Self::CheckoutFailed => "checkout_failed",
            Self::CompensationPending => "checkout_compensation_pending",
            Self::ReconciliationRequired => "checkout_reconciliation_required",
        }
    }

    pub const fn public_message(&self) -> &'static str {
        match self {
            Self::Validation(_) => "Checkout request is invalid",
            Self::CartAccess => "Checkout cart was not found or is not accessible",
            Self::AuthenticationRequired => "Authentication is required for customer-owned carts",
            Self::TemporarilyUnavailable => "Checkout is temporarily unavailable",
            Self::CheckoutFailed => "Checkout could not be completed",
            Self::CompensationPending => "Checkout failed and compensation will be retried",
            Self::ReconciliationRequired => {
                "Checkout requires reconciliation before it can continue"
            }
        }
    }

    pub const fn retryable(&self) -> bool {
        matches!(
            self,
            Self::TemporarilyUnavailable | Self::CompensationPending
        )
    }
}

/// Backward-compatible storefront command wrapper using the resolver-scoped Product provider.
pub async fn complete_storefront_checkout(
    runtime: &StorefrontCheckoutRuntime,
    payment_provider_registry: PaymentProviderRegistry,
    tenant: &TenantContext,
    request_context: &RequestContext,
    auth: OptionalAuthContext,
    idempotency_key: impl Into<String>,
    command: StorefrontCheckoutCompletionCommand,
) -> Result<crate::dto::CompleteCheckoutResponse, StorefrontStagedCheckoutRuntimeError> {
    complete_storefront_checkout_input(
        runtime,
        payment_provider_registry,
        tenant.id,
        request_context,
        auth.0,
        idempotency_key,
        crate::dto::CompleteCheckoutInput {
            cart_id: command.cart_id,
            shipping_option_id: None,
            shipping_selections: None,
            region_id: None,
            country_code: None,
            locale: Some(request_context.locale.clone()),
            create_fulfillment: command.create_fulfillment,
            metadata: command.metadata,
        },
    )
    .await
}

/// Host-composed storefront command wrapper.
///
/// Native and remote-selected transports use this entrypoint so Product execution is chosen by
/// `ProductCatalogReadRuntime` rather than reconstructed inside Commerce.
pub async fn complete_storefront_checkout_with_product_port(
    runtime: &StorefrontCheckoutRuntime,
    payment_provider_registry: PaymentProviderRegistry,
    product_catalog_read_port: Arc<dyn rustok_product::ProductCatalogReadPort>,
    tenant: &TenantContext,
    request_context: &RequestContext,
    auth: OptionalAuthContext,
    idempotency_key: impl Into<String>,
    command: StorefrontCheckoutCompletionCommand,
) -> Result<crate::dto::CompleteCheckoutResponse, StorefrontStagedCheckoutRuntimeError> {
    complete_storefront_checkout_input_with_product_port(
        runtime,
        payment_provider_registry,
        product_catalog_read_port,
        tenant.id,
        request_context,
        auth.0,
        idempotency_key,
        crate::dto::CompleteCheckoutInput {
            cart_id: command.cart_id,
            shipping_option_id: None,
            shipping_selections: None,
            region_id: None,
            country_code: None,
            locale: Some(request_context.locale.clone()),
            create_fulfillment: command.create_fulfillment,
            metadata: command.metadata,
        },
    )
    .await
}

/// Resolver-scoped compatibility wrapper.
///
/// Mounted GraphQL schemas receive the host-selected Product runtime through the Commerce resolver
/// scope. Directly embedded schemas retain an explicit in-process fallback.
pub async fn complete_storefront_checkout_input(
    runtime: &StorefrontCheckoutRuntime,
    payment_provider_registry: PaymentProviderRegistry,
    tenant_id: Uuid,
    request_context: &RequestContext,
    auth: Option<AuthContext>,
    idempotency_key: impl Into<String>,
    checkout_input: crate::dto::CompleteCheckoutInput,
) -> Result<crate::dto::CompleteCheckoutResponse, StorefrontStagedCheckoutRuntimeError> {
    let product_catalog_read_port =
        crate::graphql_runtime::product_catalog_read_runtime_for_current_graphql_scope(
            runtime.db_clone(),
            runtime.event_bus(),
        )
        .read_port();
    complete_storefront_checkout_input_with_product_port(
        runtime,
        payment_provider_registry,
        product_catalog_read_port,
        tenant_id,
        request_context,
        auth,
        idempotency_key,
        checkout_input,
    )
    .await
}

/// Completes one storefront checkout through the durable staged owner-port pipeline using the
/// Product port selected by host composition.
pub async fn complete_storefront_checkout_input_with_product_port(
    runtime: &StorefrontCheckoutRuntime,
    payment_provider_registry: PaymentProviderRegistry,
    product_catalog_read_port: Arc<dyn rustok_product::ProductCatalogReadPort>,
    tenant_id: Uuid,
    request_context: &RequestContext,
    auth: Option<AuthContext>,
    idempotency_key: impl Into<String>,
    mut checkout_input: crate::dto::CompleteCheckoutInput,
) -> Result<crate::dto::CompleteCheckoutResponse, StorefrontStagedCheckoutRuntimeError> {
    let idempotency_key = idempotency_key.into().trim().to_string();
    if idempotency_key.is_empty() || idempotency_key.len() > 191 {
        return Err(StorefrontStagedCheckoutRuntimeError::Validation(
            "idempotency key must contain 1 to 191 bytes".to_string(),
        ));
    }
    if checkout_input.cart_id.is_nil() {
        return Err(StorefrontStagedCheckoutRuntimeError::Validation(
            "cart_id must be a non-nil UUID".to_string(),
        ));
    }
    let cart_id = checkout_input.cart_id;
    let cart_storefront_port = in_process_cart_storefront_port(runtime.db_clone());
    let cart_port_context = cart_context(
        tenant_id,
        cart_id,
        request_context,
        auth.as_ref(),
        &idempotency_key,
    );
    let cart = cart_storefront_port
        .read_storefront_cart(
            cart_port_context.clone(),
            CartStorefrontReadRequest { cart_id },
        )
        .await
        .map_err(|error| {
            map_owner_port_error(
                &cart_port_context,
                STOREFRONT_STAGED_CHECKOUT_CART_OWNER,
                "read_storefront_cart",
                error,
                StorefrontStagedCheckoutRuntimeError::CartAccess,
            )
        })?;

    if checkout_input
        .locale
        .as_deref()
        .map(str::trim)
        .is_none_or(str::is_empty)
    {
        checkout_input.locale = cart
            .locale_code
            .clone()
            .or_else(|| Some(request_context.locale.clone()));
    }
    if checkout_input.country_code.is_none() {
        checkout_input.country_code = cart.country_code.clone();
    }
    if checkout_input.region_id.is_none() {
        checkout_input.region_id = cart.region_id;
    }
    let customer_id = resolve_customer_id(runtime, tenant_id, auth.as_ref()).await?;
    if cart.customer_id.is_some() && cart.customer_id != customer_id {
        if auth.is_none() {
            return Err(StorefrontStagedCheckoutRuntimeError::AuthenticationRequired);
        }
        return Err(StorefrontStagedCheckoutRuntimeError::CartAccess);
    }
    let actor_id = auth
        .as_ref()
        .map(|auth| auth.user_id)
        .unwrap_or_else(Uuid::nil);

    let event_bus = runtime.event_bus();
    let pricing_resolver = Arc::new(crate::StorefrontCheckoutPricingResolver::new(
        runtime.db_clone(),
        event_bus.clone(),
        request_context.channel_id,
        request_context.channel_slug.clone(),
    ));
    let atomic_cart = bind_in_process_atomic_cart_checkout_with_pricing(
        runtime.db_clone(),
        PrepareCartCheckoutSnapshotRequest {
            cart_id,
            input: rustok_cart::UpdateCartContextInput {
                email: None,
                region_id: checkout_input.region_id,
                country_code: checkout_input.country_code.clone(),
                locale_code: checkout_input.locale.clone(),
                selected_shipping_option_id: checkout_input.shipping_option_id,
                shipping_selections: checkout_input.shipping_selections.clone(),
            },
        },
        pricing_resolver,
    );
    let inventory_availability = rustok_inventory::in_process_inventory_reservation_port(
        runtime.db_clone(),
        event_bus.clone(),
    );
    let reservation_port =
        rustok_inventory::in_process_inventory_reservation_identity_port(runtime.db_clone());
    let plan_builder = crate::CheckoutPlanBuilder::new(
        runtime.db_clone(),
        Arc::new(rustok_region::RegionService::new(runtime.db_clone())),
        inventory_availability,
        product_catalog_read_port,
    );
    let pipeline = crate::CheckoutStagePipeline::new(
        runtime.db_clone(),
        event_bus.clone(),
        reservation_port.clone(),
        atomic_cart.port.clone(),
    );
    #[cfg(feature = "marketplace-financial")]
    let pipeline = {
        let marketplace_allocation_service = Arc::new(
            rustok_marketplace_allocation::MarketplaceAllocationService::new(runtime.db_clone()),
        );
        let marketplace_commission_service = Arc::new(
            rustok_marketplace_commission::MarketplaceCommissionService::new(
                runtime.db_clone(),
                marketplace_allocation_service.clone(),
            ),
        );
        let marketplace_ledger_service =
            Arc::new(rustok_marketplace_ledger::MarketplaceLedgerService::new(
                runtime.db_clone(),
                marketplace_commission_service.clone(),
            ));
        pipeline
            .with_marketplace_allocation_port(marketplace_allocation_service)
            .with_marketplace_commission_port(marketplace_commission_service)
            .with_marketplace_ledger_port(marketplace_ledger_service)
    };
    let pipeline = pipeline.with_payment_provider_registry(payment_provider_registry.clone());
    let staged = crate::StagedCheckoutService::new(
        plan_builder,
        pipeline,
        atomic_cart.handle,
        runtime.db_clone(),
    );
    let compensation = crate::CheckoutCompensationService::new(
        runtime.db_clone(),
        event_bus,
        reservation_port,
        in_process_cart_checkout_port(runtime.db_clone()),
    )
    .with_payment_provider_registry(payment_provider_registry);

    crate::RecoveringStagedCheckoutService::new(staged, compensation)
        .complete_checkout(tenant_id, actor_id, idempotency_key, checkout_input)
        .await
        .map_err(|error| map_checkout_error(&cart_port_context, cart_id, actor_id, error))
}

async fn resolve_customer_id(
    runtime: &StorefrontCheckoutRuntime,
    tenant_id: Uuid,
    auth: Option<&AuthContext>,
) -> Result<Option<Uuid>, StorefrontStagedCheckoutRuntimeError> {
    let Some(auth) = auth else {
        return Ok(None);
    };
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(auth.user_id.to_string()),
        rustok_api::PLATFORM_FALLBACK_LOCALE,
        format!("storefront-checkout:customer:{}", auth.user_id),
    )
    .with_deadline(Duration::from_secs(2));
    match in_process_customer_read_port(runtime.db_clone())
        .read_customer_projection_by_user(
            context.clone(),
            CustomerUserProjectionRequest {
                user_id: auth.user_id,
            },
        )
        .await
    {
        Ok(customer) => Ok(Some(customer.id)),
        Err(error) if error.code == "customer.customer_by_user_not_found" => Ok(None),
        Err(error) => Err(map_owner_port_error(
            &context,
            STOREFRONT_STAGED_CHECKOUT_CUSTOMER_OWNER,
            "read_customer_projection_by_user",
            error,
            StorefrontStagedCheckoutRuntimeError::CartAccess,
        )),
    }
}

fn cart_context(
    tenant_id: Uuid,
    cart_id: Uuid,
    request_context: &RequestContext,
    auth: Option<&AuthContext>,
    idempotency_key: &str,
) -> PortContext {
    let actor = auth
        .map(|auth| PortActor::user(auth.user_id.to_string()))
        .unwrap_or_else(|| PortActor::service("storefront-checkout"));
    let mut context = PortContext::new(
        tenant_id.to_string(),
        actor,
        request_context.locale.clone(),
        format!("storefront-checkout:cart:{cart_id}"),
    )
    .with_idempotency_key(idempotency_key)
    .with_deadline(Duration::from_secs(2));
    if let Some(channel) = request_context.channel_slug.as_deref() {
        context = context.with_channel(channel.to_string());
    }
    context
}

fn map_owner_port_error(
    context: &PortContext,
    owner: &'static str,
    operation: &'static str,
    error: PortError,
    fallback: StorefrontStagedCheckoutRuntimeError,
) -> StorefrontStagedCheckoutRuntimeError {
    let public = match &error.kind {
        PortErrorKind::Unavailable | PortErrorKind::Timeout => {
            StorefrontStagedCheckoutRuntimeError::TemporarilyUnavailable
        }
        _ => fallback,
    };
    match &error.kind {
        PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation => {
            tracing::error!(
                error = ?error,
                owner,
                operation,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                actor = ?context.actor,
                channel = ?context.channel,
                locale = %context.locale,
                causation_id = ?context.causation_id,
                traceparent = ?context.traceparent,
                idempotency_key = ?context.idempotency_key,
                deadline_ms = ?context.deadline_ms,
                internal_code = %error.code,
                internal_message = %error.message,
                error_kind = ?error.kind,
                owner_retryable = error.retryable,
                public_code = public.public_code(),
                public_retryable = public.retryable(),
                boundary = STOREFRONT_STAGED_CHECKOUT_BOUNDARY,
                "storefront checkout owner port failed"
            );
        }
        _ => {
            tracing::warn!(
                error = ?error,
                owner,
                operation,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                actor = ?context.actor,
                channel = ?context.channel,
                locale = %context.locale,
                causation_id = ?context.causation_id,
                traceparent = ?context.traceparent,
                idempotency_key = ?context.idempotency_key,
                deadline_ms = ?context.deadline_ms,
                internal_code = %error.code,
                internal_message = %error.message,
                error_kind = ?error.kind,
                owner_retryable = error.retryable,
                public_code = public.public_code(),
                public_retryable = public.retryable(),
                boundary = STOREFRONT_STAGED_CHECKOUT_BOUNDARY,
                "storefront checkout owner port was rejected"
            );
        }
    }
    public
}

fn map_checkout_error(
    context: &PortContext,
    cart_id: Uuid,
    actor_id: Uuid,
    error: crate::RecoveringStagedCheckoutError,
) -> StorefrontStagedCheckoutRuntimeError {
    let (public, error_kind) = match &error {
        crate::RecoveringStagedCheckoutError::StagedAndCompensation {
            compensation: crate::CheckoutCompensationError::ManualReconciliation(_),
            ..
        } => (
            StorefrontStagedCheckoutRuntimeError::ReconciliationRequired,
            "manual_reconciliation",
        ),
        crate::RecoveringStagedCheckoutError::StagedAndCompensation { .. } => (
            StorefrontStagedCheckoutRuntimeError::CompensationPending,
            "compensation_pending",
        ),
        crate::RecoveringStagedCheckoutError::StagedAndJournal { .. } => (
            StorefrontStagedCheckoutRuntimeError::TemporarilyUnavailable,
            "staged_and_journal",
        ),
        crate::RecoveringStagedCheckoutError::Journal(_) => (
            StorefrontStagedCheckoutRuntimeError::TemporarilyUnavailable,
            "journal",
        ),
        crate::RecoveringStagedCheckoutError::Staged(_) => (
            StorefrontStagedCheckoutRuntimeError::CheckoutFailed,
            "staged",
        ),
    };
    tracing::error!(
        error = ?error,
        owner = STOREFRONT_STAGED_CHECKOUT_OWNER,
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        channel = ?context.channel,
        actor_id = %actor_id,
        cart_id = %cart_id,
        operation = "complete_storefront_checkout",
        error_kind,
        public_code = public.public_code(),
        retryable = public.retryable(),
        boundary = STOREFRONT_STAGED_CHECKOUT_BOUNDARY,
        "storefront staged checkout failed"
    );
    public
}
