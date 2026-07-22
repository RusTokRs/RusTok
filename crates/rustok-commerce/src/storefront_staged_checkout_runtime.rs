use rustok_api::{OptionalAuthContext, PortActor, PortContext, RequestContext, TenantContext};
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

#[derive(Debug, Error)]
pub enum StorefrontStagedCheckoutRuntimeError {
    #[error("checkout request is invalid: {0}")]
    Validation(String),
    #[error("checkout cart is not accessible")]
    CartAccess,
    #[error("checkout could not be completed")]
    CheckoutFailed,
    #[error("checkout requires reconciliation")]
    ReconciliationRequired,
}

pub async fn complete_storefront_checkout(
    runtime: &StorefrontCheckoutRuntime,
    payment_provider_registry: PaymentProviderRegistry,
    tenant: &TenantContext,
    request_context: &RequestContext,
    auth: OptionalAuthContext,
    idempotency_key: impl Into<String>,
    command: StorefrontCheckoutCompletionCommand,
) -> Result<crate::dto::CompleteCheckoutResponse, StorefrontStagedCheckoutRuntimeError> {
    let idempotency_key = idempotency_key.into().trim().to_string();
    if idempotency_key.is_empty() || idempotency_key.len() > 191 {
        return Err(StorefrontStagedCheckoutRuntimeError::Validation(
            "idempotency key must contain 1 to 191 bytes".to_string(),
        ));
    }

    let auth_context = auth.0;
    let cart_storefront_port = in_process_cart_storefront_port(runtime.db_clone());
    let cart = cart_storefront_port
        .read_storefront_cart(
            cart_context(
                tenant.id,
                command.cart_id,
                request_context,
                auth_context.as_ref(),
            ),
            CartStorefrontReadRequest {
                cart_id: command.cart_id,
            },
        )
        .await
        .map_err(|_| StorefrontStagedCheckoutRuntimeError::CartAccess)?;
    let customer_id = resolve_customer_id(runtime, tenant.id, auth_context.as_ref()).await?;
    if cart.customer_id.is_some() && cart.customer_id != customer_id {
        return Err(StorefrontStagedCheckoutRuntimeError::CartAccess);
    }
    let actor_id = auth_context
        .as_ref()
        .map(|auth| auth.user_id)
        .unwrap_or_else(Uuid::nil);

    let checkout_input = crate::dto::CompleteCheckoutInput {
        cart_id: command.cart_id,
        shipping_option_id: None,
        shipping_selections: None,
        region_id: None,
        country_code: None,
        locale: Some(request_context.locale.clone()),
        create_fulfillment: command.create_fulfillment,
        metadata: command.metadata,
    };
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
            cart_id: command.cart_id,
            region_id: None,
            country_code: None,
            locale_code: Some(request_context.locale.clone()),
            selected_shipping_option_id: None,
            shipping_selections: None,
        },
        pricing_resolver,
    );
    let inventory_availability = Arc::new(rustok_inventory::InventoryService::new(
        runtime.db_clone(),
        event_bus.clone(),
    ));
    let reservation_port =
        rustok_inventory::in_process_inventory_reservation_identity_port(runtime.db_clone());
    let plan_builder = crate::CheckoutPlanBuilder::new(
        runtime.db_clone(),
        Arc::new(rustok_region::RegionService::new(runtime.db_clone())),
        inventory_availability,
        Arc::new(rustok_product::CatalogService::new(
            runtime.db_clone(),
            event_bus.clone(),
        )),
    );
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
    let pipeline = crate::CheckoutStagePipeline::new(
        runtime.db_clone(),
        event_bus.clone(),
        reservation_port.clone(),
        atomic_cart.port.clone(),
    )
    .with_marketplace_allocation_port(marketplace_allocation_service)
    .with_marketplace_commission_port(marketplace_commission_service)
    .with_marketplace_ledger_port(marketplace_ledger_service)
    .with_payment_provider_registry(payment_provider_registry.clone());
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
        .complete_checkout(tenant.id, actor_id, idempotency_key, checkout_input)
        .await
        .map_err(map_checkout_error)
}

async fn resolve_customer_id(
    runtime: &StorefrontCheckoutRuntime,
    tenant_id: Uuid,
    auth: Option<&rustok_api::AuthContext>,
) -> Result<Option<Uuid>, StorefrontStagedCheckoutRuntimeError> {
    let Some(auth) = auth else {
        return Ok(None);
    };
    match in_process_customer_read_port(runtime.db_clone())
        .read_customer_projection_by_user(
            PortContext::new(
                tenant_id.to_string(),
                PortActor::user(auth.user_id.to_string()),
                rustok_api::PLATFORM_FALLBACK_LOCALE,
                format!("storefront-checkout:customer:{}", auth.user_id),
            )
            .with_deadline(Duration::from_secs(2)),
            CustomerUserProjectionRequest {
                user_id: auth.user_id,
            },
        )
        .await
    {
        Ok(customer) => Ok(Some(customer.id)),
        Err(error) if error.code == "customer.customer_by_user_not_found" => Ok(None),
        Err(_) => Err(StorefrontStagedCheckoutRuntimeError::CartAccess),
    }
}

fn cart_context(
    tenant_id: Uuid,
    cart_id: Uuid,
    request_context: &RequestContext,
    auth: Option<&rustok_api::AuthContext>,
) -> PortContext {
    let actor = auth
        .map(|auth| PortActor::user(auth.user_id.to_string()))
        .unwrap_or_else(|| PortActor::service("storefront-native-checkout"));
    let mut context = PortContext::new(
        tenant_id.to_string(),
        actor,
        request_context.locale.clone(),
        format!("storefront-native-checkout:cart:{cart_id}"),
    )
    .with_deadline(Duration::from_secs(2));
    if let Some(channel) = request_context.channel_slug.as_deref() {
        context = context.with_channel(channel.to_string());
    }
    context
}

fn map_checkout_error(
    error: crate::RecoveringStagedCheckoutError,
) -> StorefrontStagedCheckoutRuntimeError {
    match error {
        crate::RecoveringStagedCheckoutError::StagedAndCompensation {
            compensation: crate::CheckoutCompensationError::ManualReconciliation(_),
            ..
        } => StorefrontStagedCheckoutRuntimeError::ReconciliationRequired,
        crate::RecoveringStagedCheckoutError::StagedAndJournal { .. }
        | crate::RecoveringStagedCheckoutError::StagedAndCompensation { .. }
        | crate::RecoveringStagedCheckoutError::Staged(_)
        | crate::RecoveringStagedCheckoutError::Journal(_) => {
            StorefrontStagedCheckoutRuntimeError::CheckoutFailed
        }
    }
}
