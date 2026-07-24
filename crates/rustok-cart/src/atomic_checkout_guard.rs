use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{
    PLATFORM_FALLBACK_LOCALE, PortActor, PortContext, PortError, PortErrorKind,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::atomic_checkout_port::{self as legacy, AtomicCartCheckoutPricingResolver};
use crate::checkout_snapshot::{
    PrepareCartCheckoutSnapshotRequest, PreparedCartCheckoutSnapshot,
};
use crate::{
    CartCheckoutContextUpdateRequest, CartCheckoutLifecycleRequest, CartCheckoutPort,
    CartCheckoutSnapshotRequest, CartResponse,
};

const READ_ATOMIC_CART_CHECKOUT_SNAPSHOT_OPERATION: &str =
    "read_atomic_cart_checkout_snapshot";
const UPDATE_ATOMIC_CART_CHECKOUT_CONTEXT_OPERATION: &str =
    "update_atomic_cart_checkout_context";
const BEGIN_ATOMIC_CART_CHECKOUT_OPERATION: &str = "begin_atomic_cart_checkout";
const RELEASE_ATOMIC_CART_CHECKOUT_OPERATION: &str = "release_atomic_cart_checkout";
const COMPLETE_ATOMIC_CART_CHECKOUT_OPERATION: &str = "complete_atomic_cart_checkout";
const PREPARE_ATOMIC_CART_CHECKOUT_OPERATION: &str = "prepare_atomic_cart_checkout";

struct GuardedAtomicCartCheckoutPort {
    inner: Arc<dyn CartCheckoutPort>,
}

#[derive(Clone)]
pub struct AtomicCartCheckoutHandle {
    inner: legacy::AtomicCartCheckoutHandle,
}

pub struct AtomicCartCheckoutBinding {
    pub port: Arc<dyn CartCheckoutPort>,
    pub handle: AtomicCartCheckoutHandle,
}

impl AtomicCartCheckoutHandle {
    pub fn cart_id(&self) -> Uuid {
        self.inner.cart_id()
    }

    pub async fn prepare(
        &self,
        tenant_id: Uuid,
        allow_existing_lock: bool,
    ) -> Result<PreparedCartCheckoutSnapshot, PortError> {
        let context = atomic_handle_context(tenant_id, self.cart_id());
        self.inner
            .prepare(tenant_id, allow_existing_lock)
            .await
            .map_err(|error| {
                map_atomic_checkout_error(
                    &context,
                    PREPARE_ATOMIC_CART_CHECKOUT_OPERATION,
                    error,
                )
            })
    }
}

pub fn bind_in_process_atomic_cart_checkout(
    db: DatabaseConnection,
    prepare_request: PrepareCartCheckoutSnapshotRequest,
) -> AtomicCartCheckoutBinding {
    wrap_binding(legacy::bind_in_process_atomic_cart_checkout(
        db,
        prepare_request,
    ))
}

pub fn bind_in_process_atomic_cart_checkout_with_pricing(
    db: DatabaseConnection,
    prepare_request: PrepareCartCheckoutSnapshotRequest,
    pricing_resolver: Arc<dyn AtomicCartCheckoutPricingResolver>,
) -> AtomicCartCheckoutBinding {
    wrap_binding(legacy::bind_in_process_atomic_cart_checkout_with_pricing(
        db,
        prepare_request,
        pricing_resolver,
    ))
}

pub fn in_process_atomic_cart_checkout_port(
    db: DatabaseConnection,
    prepare_request: PrepareCartCheckoutSnapshotRequest,
) -> Arc<dyn CartCheckoutPort> {
    bind_in_process_atomic_cart_checkout(db, prepare_request).port
}

fn wrap_binding(binding: legacy::AtomicCartCheckoutBinding) -> AtomicCartCheckoutBinding {
    AtomicCartCheckoutBinding {
        port: Arc::new(GuardedAtomicCartCheckoutPort {
            inner: binding.port,
        }),
        handle: AtomicCartCheckoutHandle {
            inner: binding.handle,
        },
    }
}

#[async_trait]
impl CartCheckoutPort for GuardedAtomicCartCheckoutPort {
    async fn read_cart_checkout_snapshot(
        &self,
        context: PortContext,
        request: CartCheckoutSnapshotRequest,
    ) -> Result<CartResponse, PortError> {
        let error_context = context.clone();
        self.inner
            .read_cart_checkout_snapshot(context, request)
            .await
            .map_err(|error| {
                map_atomic_checkout_error(
                    &error_context,
                    READ_ATOMIC_CART_CHECKOUT_SNAPSHOT_OPERATION,
                    error,
                )
            })
    }

    async fn update_cart_checkout_context(
        &self,
        context: PortContext,
        request: CartCheckoutContextUpdateRequest,
    ) -> Result<CartResponse, PortError> {
        let error_context = context.clone();
        self.inner
            .update_cart_checkout_context(context, request)
            .await
            .map_err(|error| {
                map_atomic_checkout_error(
                    &error_context,
                    UPDATE_ATOMIC_CART_CHECKOUT_CONTEXT_OPERATION,
                    error,
                )
            })
    }

    async fn begin_cart_checkout(
        &self,
        context: PortContext,
        request: CartCheckoutLifecycleRequest,
    ) -> Result<CartResponse, PortError> {
        let error_context = context.clone();
        self.inner
            .begin_cart_checkout(context, request)
            .await
            .map_err(|error| {
                map_atomic_checkout_error(
                    &error_context,
                    BEGIN_ATOMIC_CART_CHECKOUT_OPERATION,
                    error,
                )
            })
    }

    async fn release_cart_checkout(
        &self,
        context: PortContext,
        request: CartCheckoutLifecycleRequest,
    ) -> Result<CartResponse, PortError> {
        let error_context = context.clone();
        self.inner
            .release_cart_checkout(context, request)
            .await
            .map_err(|error| {
                map_atomic_checkout_error(
                    &error_context,
                    RELEASE_ATOMIC_CART_CHECKOUT_OPERATION,
                    error,
                )
            })
    }

    async fn complete_cart_checkout(
        &self,
        context: PortContext,
        request: CartCheckoutLifecycleRequest,
    ) -> Result<CartResponse, PortError> {
        let error_context = context.clone();
        self.inner
            .complete_cart_checkout(context, request)
            .await
            .map_err(|error| {
                map_atomic_checkout_error(
                    &error_context,
                    COMPLETE_ATOMIC_CART_CHECKOUT_OPERATION,
                    error,
                )
            })
    }
}

fn atomic_handle_context(tenant_id: Uuid, cart_id: Uuid) -> PortContext {
    let key = format!("cart:{cart_id}:atomic-checkout-handle");
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service("rustok-cart.atomic-checkout-guard"),
        PLATFORM_FALLBACK_LOCALE,
        key.clone(),
    )
    .with_idempotency_key(key)
}

fn map_atomic_checkout_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: PortError,
) -> PortError {
    eprintln!("DEBUG MAP ATOMIC CHECKOUT ERROR: {error:?}");
    tracing::error!(
        error = ?error,
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        operation = owner_operation,
        owner_code = %error.code,
        owner_kind = ?error.kind,
        retryable = error.retryable,
        "atomic cart checkout owner operation failed"
    );

    let PortError {
        kind,
        code,
        retryable,
        ..
    } = error;

    let public_message = match code.as_str() {
        "cart.checkout_pricing_changed" => {
            "cart checkout pricing changed; retry with a fresh cart snapshot"
        }
        "cart.checkout_adapter_cart_mismatch" => {
            "cart checkout adapter does not match the requested cart"
        }
        "cart.invalid_tenant_id" => "cart checkout request context is invalid",
        "cart.checkout_not_locked" => {
            "cart checkout could not acquire the required cart lock"
        }
        "cart.checkout_prepared_state_poisoned" => {
            "cart checkout prepared state is unavailable"
        }
        "cart.checkout_validation" => "cart checkout request is invalid",
        "cart.not_found" => "cart was not found",
        "cart.line_item_not_found" => "cart line item was not found",
        "cart.invalid_transition" => {
            "cart lifecycle transition conflicts with the current state"
        }
        "cart.checkout_storage_unavailable" => {
            "cart checkout storage is temporarily unavailable"
        }
        _ => match &kind {
            PortErrorKind::Validation => "cart checkout request is invalid",
            PortErrorKind::NotFound => "cart checkout resource was not found",
            PortErrorKind::Conflict => "cart checkout conflicts with the current state",
            PortErrorKind::Unavailable | PortErrorKind::Timeout => {
                "cart checkout is temporarily unavailable"
            }
            _ => "cart checkout operation could not be completed safely",
        },
    };

    PortError::new(kind, code, public_message, retryable)
}
