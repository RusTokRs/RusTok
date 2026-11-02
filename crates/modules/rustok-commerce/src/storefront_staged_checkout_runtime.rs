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
pub async fn complete_storefront_checkout_with_product_runtime(
    runtime: &StorefrontCheckoutRuntime,
    payment_provider_registry: PaymentProviderRegistry,
    product_catalog_read_runtime: rustok_product::ProductCatalogReadRuntime,
    tenant: &TenantContext,
    request_context: &RequestContext,
    auth: OptionalAuthContext,
    idempotency_key: impl Into<String>,
    command: StorefrontCheckoutCompletionCommand,
) -> Result<crate::dto::CompleteCheckoutResponse, StorefrontStagedCheckoutRuntimeError> {
    complete_storefront_checkout_input_with_product_runtime(
        runtime,
        payment_provider_registry,
        product_catalog_read_runtime,
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
/// Mounted callers capture the full host-selected Product runtime in StorefrontCheckoutRuntime.
/// Directly embedded callers explicitly construct its in-process baseline.
pub async fn complete_storefront_checkout_input(
    runtime: &StorefrontCheckoutRuntime,
    payment_provider_registry: PaymentProviderRegistry,
    tenant_id: Uuid,
    request_context: &RequestContext,
    auth: Option<AuthContext>,
    idempotency_key: impl Into<String>,
    checkout_input: crate::dto::CompleteCheckoutInput,
) -> Result<crate::dto::CompleteCheckoutResponse, StorefrontStagedCheckoutRuntimeError> {
    let product_catalog_read_runtime = runtime.product_catalog_read_runtime();
    complete_storefront_checkout_input_with_product_runtime(
        runtime,
        payment_provider_registry,
        product_catalog_read_runtime,
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
pub async fn complete_storefront_checkout_input_with_product_runtime(
    runtime: &StorefrontCheckoutRuntime,
    payment_provider_registry: PaymentProviderRegistry,
    product_catalog_read_runtime: rustok_product::ProductCatalogReadRuntime,
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
    if tenant_id.is_nil()
        || request_context.tenant_id != tenant_id
        || auth
            .as_ref()
            .is_some_and(|auth| auth.tenant_id != tenant_id || auth.user_id.is_nil())
        || request_context
            .user_id
            .is_some_and(|id| auth.as_ref().map(|auth| auth.user_id) != Some(id))
    {
        return Err(StorefrontStagedCheckoutRuntimeError::CartAccess);
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
        product_catalog_read_runtime.clone(),
        cart_port_context.clone(),
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
        rustok_region::in_process_region_read_port(runtime.db_clone()),
        inventory_availability,
        product_catalog_read_runtime.read_port(),
    );
    let pipeline = crate::CheckoutStagePipeline::new(
        runtime.db_clone(),
        event_bus.clone(),
        reservation_port.clone(),
        atomic_cart.port.clone(),
    );
    #[cfg(feature = "marketplace-financial")]
    let pipeline = {
        // Marketplace checkout receives typed owner capabilities. Concrete allocation, commission,
        // and ledger services are composed only inside their respective owner factories.
        let allocation_port =
            rustok_marketplace_allocation::in_process_marketplace_allocation_command_port(
                runtime.db_clone(),
            );
        let commission_port =
            rustok_marketplace_commission::in_process_marketplace_commission_command_port(
                runtime.db_clone(),
            );
        let ledger_port = rustok_marketplace_balance::in_process_marketplace_ledger_command_port(
            runtime.db_clone(),
        );
        pipeline
            .with_marketplace_allocation_port(allocation_port)
            .with_marketplace_commission_port(commission_port)
            .with_marketplace_ledger_port(ledger_port)
    };
    let pipeline = pipeline.with_payment_provider_registry(payment_provider_registry.clone());
    let staged = crate::StagedCheckoutService::new(
        plan_builder,
        pipeline,
        atomic_cart.handle,
        runtime.db_clone(),
        event_bus.clone(),
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
        request_context.correlation_id.clone(),
    )
    .with_causation_id(format!("storefront-checkout:cart:{cart_id}"))
    .with_idempotency_key(idempotency_key)
    .with_deadline(Duration::from_secs(2));
    if let Some(auth) = auth {
        context.claims.extend(auth.scopes.iter().cloned());
        context
            .claims
            .extend(auth.permissions.iter().map(ToString::to_string));
    }
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
        PortErrorKind::Unavailable | PortErrorKind::Timeout if error.retryable => {
            StorefrontStagedCheckoutRuntimeError::TemporarilyUnavailable
        }
        PortErrorKind::Unavailable | PortErrorKind::Timeout => {
            StorefrontStagedCheckoutRuntimeError::CheckoutFailed
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
    if let Some((code, retryable)) = error
        .inventory_adoption_failure()
        .and_then(crate::CheckoutInventoryOrderAdoptionError::bounded_failure)
    {
        let public = if retryable {
            StorefrontStagedCheckoutRuntimeError::TemporarilyUnavailable
        } else {
            StorefrontStagedCheckoutRuntimeError::ReconciliationRequired
        };
        // Owner advice survives all wrappers, but is neither a receipt nor permission to
        // compensate or change keys. Never log the nested owner/journal payload here.
        tracing::warn!(
            owner = STOREFRONT_STAGED_CHECKOUT_OWNER,
            operation = "complete_storefront_checkout",
            correlation_id = %context.correlation_id,
            code,
            retryable,
            public_code = public.public_code(),
            boundary = STOREFRONT_STAGED_CHECKOUT_BOUNDARY,
            "storefront inventory adoption requires recovery of the existing checkout identity"
        );
        return public;
    }
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
        crate::RecoveringStagedCheckoutError::Staged(crate::StagedCheckoutError::Checkout(
            crate::CheckoutError::BoundaryFailure {
                retryable: true, ..
            },
        )) => (
            StorefrontStagedCheckoutRuntimeError::TemporarilyUnavailable,
            "staged_owner_retryable",
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

#[cfg(test)]
mod tests {
    use super::*;

    fn request_context() -> RequestContext {
        RequestContext {
            tenant_id: Uuid::from_u128(1),
            user_id: Some(Uuid::from_u128(2)),
            channel_id: Some(Uuid::from_u128(8)),
            channel_slug: Some("page-channel".into()),
            channel_resolution_source: None,
            locale: "fr".into(),
            correlation_id: "original-checkout-request".into(),
        }
    }

    #[test]
    fn checkout_context_keeps_authenticated_caller_and_original_correlation() {
        let request = request_context();
        let auth = AuthContext {
            user_id: request.user_id.unwrap(),
            tenant_id: request.tenant_id,
            session_id: Uuid::from_u128(3),
            permissions: vec![],
            client_id: None,
            scopes: vec!["storefront:cart".into()],
            grant_type: "authorization_code".into(),
        };
        let context = cart_context(
            request.tenant_id,
            Uuid::from_u128(4),
            &request,
            Some(&auth),
            "original-key",
        );
        assert_eq!(context.actor, PortActor::user(auth.user_id.to_string()));
        assert_eq!(context.claims, auth.scopes);
        assert_eq!(context.correlation_id, request.correlation_id);
        assert_eq!(context.locale, request.locale);
        assert_eq!(context.idempotency_key.as_deref(), Some("original-key"));
        assert_eq!(context.deadline_ms, Some(2000));
        assert!(context.causation_id.is_some());
    }

    #[test]
    fn checkout_guest_is_service_not_nil_user() {
        let mut request = request_context();
        request.user_id = None;
        let context = cart_context(
            request.tenant_id,
            Uuid::from_u128(4),
            &request,
            None,
            "original-key",
        );
        assert_eq!(context.actor, PortActor::service("storefront-checkout"));
        assert_eq!(context.correlation_id, request.correlation_id);
        assert!(context.claims.is_empty());
    }

    #[test]
    fn owner_mapping_never_promotes_permanent_unavailability() {
        let request = request_context();
        let context = cart_context(
            request.tenant_id,
            Uuid::from_u128(4),
            &request,
            None,
            "original-key",
        );
        for kind in [PortErrorKind::Unavailable, PortErrorKind::Timeout] {
            for retryable in [false, true] {
                let error = PortError::new(
                    kind.clone(),
                    "pricing.product_identity_capability_unavailable",
                    "private-owner-details",
                    retryable,
                );
                let public = map_owner_port_error(
                    &context,
                    "rustok_pricing",
                    "resolve_product_price",
                    error,
                    StorefrontStagedCheckoutRuntimeError::CartAccess,
                );
                assert_eq!(public.retryable(), retryable);
                assert!(!public.public_message().contains("private-owner-details"));
            }
        }
    }

    #[test]
    fn staged_cart_pricing_retains_owner_retryability_at_final_boundary() {
        let request = request_context();
        let cart_id = Uuid::from_u128(4);
        let context = cart_context(request.tenant_id, cart_id, &request, None, "original-key");
        for retryable in [false, true] {
            let error = crate::RecoveringStagedCheckoutError::Staged(
                crate::StagedCheckoutError::Checkout(crate::CheckoutError::BoundaryFailure {
                    stage: "cart_prepare_checkout",
                    kind: PortErrorKind::Unavailable,
                    code: "pricing.checkout_resolution_unavailable".into(),
                    message: "private-owner-details".into(),
                    retryable,
                }),
            );
            let public = map_checkout_error(&context, cart_id, Uuid::nil(), error);
            assert_eq!(public.retryable(), retryable);
            assert!(!public.public_message().contains("private-owner-details"));
        }
    }

    #[test]
    fn inventory_adoption_preserves_advice_and_reconciliation_through_all_error_wrappers() {
        let request = request_context();
        let cart_id = Uuid::from_u128(4);
        let context = cart_context(request.tenant_id, cart_id, &request, None, "original-key");
        for retryable in [false, true] {
            for wrapper in 0..3 {
                let pipeline = crate::CheckoutStagePipelineError::OrderStage(Box::new(
                    crate::CheckoutOrderStageError::Creation(
                        crate::CheckoutOrderCreationError::Adoption(
                            crate::CheckoutInventoryOrderAdoptionError::Boundary {
                                code: "inventory.adoption_unverified".to_string(),
                                message: "Existing checkout identity requires recovery",
                                retryable,
                            },
                        ),
                    ),
                ));
                let error = match wrapper {
                    0 => crate::RecoveringStagedCheckoutError::Staged(
                        crate::StagedCheckoutError::Pipeline(pipeline),
                    ),
                    1 => crate::RecoveringStagedCheckoutError::Staged(
                        crate::StagedCheckoutError::PipelineAndJournal {
                            pipeline: Box::new(pipeline),
                            journal: crate::CheckoutOperationError::Database(
                                sea_orm::DbErr::Custom("private-journal-diagnostic".to_string()),
                            ),
                        },
                    ),
                    _ => crate::RecoveringStagedCheckoutError::StagedAndJournal {
                        staged: Box::new(crate::StagedCheckoutError::Pipeline(pipeline)),
                        journal: crate::CheckoutOperationError::Database(sea_orm::DbErr::Custom(
                            "private-journal-diagnostic".to_string(),
                        )),
                    },
                };
                let public = map_checkout_error(&context, cart_id, Uuid::nil(), error);
                assert_eq!(public.retryable(), retryable);
                if !retryable {
                    assert!(matches!(
                        public,
                        StorefrontStagedCheckoutRuntimeError::ReconciliationRequired
                    ));
                }
                assert!(
                    !public
                        .public_message()
                        .contains("private-journal-diagnostic")
                );
            }
        }
    }
}
