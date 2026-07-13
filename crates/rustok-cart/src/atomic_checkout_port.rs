use async_trait::async_trait;
use rustok_api::{
    normalize_locale_tag, PortActor, PortCallPolicy, PortContext, PortError,
    PLATFORM_FALLBACK_LOCALE,
};
use sea_orm::DatabaseConnection;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use uuid::Uuid;

use crate::{
    in_process_cart_checkout_snapshot_port, CartCheckoutContextUpdateRequest,
    CartCheckoutLifecycleRequest, CartCheckoutPort, CartCheckoutSnapshotPort,
    CartCheckoutSnapshotRequest, CartError, CartResponse, CartService, CartStatus,
    PrepareCartCheckoutSnapshotRequest, PreparedCartCheckoutSnapshot,
};

type PreparedState = Arc<Mutex<Option<PreparedCartCheckoutSnapshot>>>;

/// Request-scoped adapter that preserves the existing `CartCheckoutPort`
/// protocol while deferring persistence until the durable orchestration claim.
pub struct AtomicCartCheckoutPort {
    service: Arc<CartService>,
    snapshot_port: Arc<dyn CartCheckoutSnapshotPort>,
    prepare_request: PrepareCartCheckoutSnapshotRequest,
    prepared_state: PreparedState,
}

/// Handle retained by the checkout journal wrapper. It performs the owner
/// prepare command after the operation lease is claimed and before any order or
/// provider side effect is allowed.
#[derive(Clone)]
pub struct AtomicCartCheckoutHandle {
    service: Arc<CartService>,
    snapshot_port: Arc<dyn CartCheckoutSnapshotPort>,
    prepare_request: PrepareCartCheckoutSnapshotRequest,
    prepared_state: PreparedState,
}

pub struct AtomicCartCheckoutBinding {
    pub port: Arc<dyn CartCheckoutPort>,
    pub handle: AtomicCartCheckoutHandle,
}

impl AtomicCartCheckoutPort {
    pub fn new(db: DatabaseConnection, prepare_request: PrepareCartCheckoutSnapshotRequest) -> Self {
        let service = Arc::new(CartService::new(db.clone()));
        Self {
            service,
            snapshot_port: in_process_cart_checkout_snapshot_port(db),
            prepare_request,
            prepared_state: Arc::new(Mutex::new(None)),
        }
    }

    fn ensure_cart_id(&self, cart_id: Uuid) -> Result<(), PortError> {
        ensure_bound_cart_id(self.prepare_request.cart_id, cart_id)
    }

    fn prepared_for_orchestration(&self) -> Result<Option<CartResponse>, PortError> {
        stored_snapshot(&self.prepared_state).map(|snapshot| {
            snapshot.map(|snapshot| {
                let mut cart = snapshot.cart;
                cart.status = CartStatus::Active.as_str().to_string();
                cart
            })
        })
    }
}

impl AtomicCartCheckoutHandle {
    pub fn cart_id(&self) -> Uuid {
        self.prepare_request.cart_id
    }

    /// Prepares the cart once for this request. A retry may adopt an existing
    /// `checking_out` or `completed` state only when the durable operation has
    /// already executed at least once.
    pub async fn prepare(
        &self,
        tenant_id: Uuid,
        allow_existing_lock: bool,
    ) -> Result<PreparedCartCheckoutSnapshot, PortError> {
        if let Some(snapshot) = stored_snapshot(&self.prepared_state)? {
            return Ok(snapshot);
        }

        let cart = match self
            .service
            .prepare_checkout(tenant_id, self.prepare_request.clone())
            .await
        {
            Ok(cart) => cart,
            Err(CartError::InvalidTransition { from, .. })
                if allow_existing_lock
                    && matches!(
                        from.as_str(),
                        status if status == CartStatus::CheckingOut.as_str()
                            || status == CartStatus::Completed.as_str()
                    ) =>
            {
                self.service
                    .get_cart(tenant_id, self.prepare_request.cart_id)
                    .await
                    .map_err(cart_error_to_port_error)?
            }
            Err(error) => return Err(cart_error_to_port_error(error)),
        };

        if !matches!(
            cart.status.as_str(),
            status if status == CartStatus::CheckingOut.as_str()
                || status == CartStatus::Completed.as_str()
        ) {
            return Err(PortError::conflict(
                "cart.checkout_not_locked",
                format!(
                    "cart {} is `{}` after checkout preparation",
                    cart.id, cart.status
                ),
            ));
        }

        let snapshot = self
            .snapshot_port
            .prepare_checkout_snapshot(
                snapshot_port_context(tenant_id, &self.prepare_request),
                self.prepare_request.clone(),
            )
            .await?;
        if cart.status == CartStatus::CheckingOut.as_str() {
            store_snapshot(&self.prepared_state, snapshot.clone())?;
        }
        Ok(snapshot)
    }
}

pub fn bind_in_process_atomic_cart_checkout(
    db: DatabaseConnection,
    prepare_request: PrepareCartCheckoutSnapshotRequest,
) -> AtomicCartCheckoutBinding {
    let service = Arc::new(CartService::new(db.clone()));
    let snapshot_port = in_process_cart_checkout_snapshot_port(db);
    let prepared_state = Arc::new(Mutex::new(None));
    let port = Arc::new(AtomicCartCheckoutPort {
        service: service.clone(),
        snapshot_port: snapshot_port.clone(),
        prepare_request: prepare_request.clone(),
        prepared_state: prepared_state.clone(),
    });
    let handle = AtomicCartCheckoutHandle {
        service,
        snapshot_port,
        prepare_request,
        prepared_state,
    };

    AtomicCartCheckoutBinding { port, handle }
}

pub fn in_process_atomic_cart_checkout_port(
    db: DatabaseConnection,
    prepare_request: PrepareCartCheckoutSnapshotRequest,
) -> Arc<dyn CartCheckoutPort> {
    bind_in_process_atomic_cart_checkout(db, prepare_request).port
}

#[async_trait]
impl CartCheckoutPort for AtomicCartCheckoutPort {
    async fn read_cart_checkout_snapshot(
        &self,
        context: PortContext,
        request: CartCheckoutSnapshotRequest,
    ) -> Result<CartResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        self.ensure_cart_id(request.cart_id)?;
        if let Some(cart) = self.prepared_for_orchestration()? {
            return Ok(cart);
        }
        self.service
            .get_cart(parse_port_tenant_id(&context)?, request.cart_id)
            .await
            .map_err(cart_error_to_port_error)
    }

    async fn update_cart_checkout_context(
        &self,
        context: PortContext,
        request: CartCheckoutContextUpdateRequest,
    ) -> Result<CartResponse, PortError> {
        context.require_write_semantics()?;
        self.ensure_cart_id(request.cart_id)?;
        if let Some(cart) = self.prepared_for_orchestration()? {
            return Ok(cart);
        }
        self.snapshot_port
            .prepare_checkout_snapshot(context, self.prepare_request.clone())
            .await
            .map(|snapshot| snapshot.cart)
    }

    async fn begin_cart_checkout(
        &self,
        context: PortContext,
        request: CartCheckoutLifecycleRequest,
    ) -> Result<CartResponse, PortError> {
        context.require_write_semantics()?;
        self.ensure_cart_id(request.cart_id)?;
        if let Some(snapshot) = stored_snapshot(&self.prepared_state)? {
            return Ok(snapshot.cart);
        }
        self.service
            .prepare_checkout(
                parse_port_tenant_id(&context)?,
                self.prepare_request.clone(),
            )
            .await
            .map_err(cart_error_to_port_error)
    }

    async fn release_cart_checkout(
        &self,
        context: PortContext,
        request: CartCheckoutLifecycleRequest,
    ) -> Result<CartResponse, PortError> {
        context.require_write_semantics()?;
        self.ensure_cart_id(request.cart_id)?;
        self.service
            .release_checkout(parse_port_tenant_id(&context)?, request.cart_id)
            .await
            .map_err(cart_error_to_port_error)
    }

    async fn complete_cart_checkout(
        &self,
        context: PortContext,
        request: CartCheckoutLifecycleRequest,
    ) -> Result<CartResponse, PortError> {
        context.require_write_semantics()?;
        self.ensure_cart_id(request.cart_id)?;
        self.service
            .complete_cart(parse_port_tenant_id(&context)?, request.cart_id)
            .await
            .map_err(cart_error_to_port_error)
    }
}

fn snapshot_port_context(
    tenant_id: Uuid,
    request: &PrepareCartCheckoutSnapshotRequest,
) -> PortContext {
    let locale = request
        .locale_code
        .as_deref()
        .and_then(normalize_locale_tag)
        .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service("rustok-cart.atomic-checkout"),
        locale,
        format!("cart:{}:prepared-checkout-snapshot", request.cart_id),
    )
    .with_deadline(Duration::from_secs(2))
}

fn ensure_bound_cart_id(bound_cart_id: Uuid, cart_id: Uuid) -> Result<(), PortError> {
    if cart_id == bound_cart_id {
        Ok(())
    } else {
        Err(PortError::validation(
            "cart.checkout_adapter_cart_mismatch",
            format!("checkout adapter is bound to cart {bound_cart_id}, not {cart_id}"),
        ))
    }
}

fn stored_snapshot(state: &PreparedState) -> Result<Option<PreparedCartCheckoutSnapshot>, PortError> {
    state
        .lock()
        .map(|snapshot| snapshot.clone())
        .map_err(|_| {
            PortError::invariant_violation(
                "cart.checkout_prepared_state_poisoned",
                "prepared checkout state is unavailable",
            )
        })
}

fn store_snapshot(
    state: &PreparedState,
    snapshot: PreparedCartCheckoutSnapshot,
) -> Result<(), PortError> {
    let mut state = state.lock().map_err(|_| {
        PortError::invariant_violation(
            "cart.checkout_prepared_state_poisoned",
            "prepared checkout state is unavailable",
        )
    })?;
    *state = Some(snapshot);
    Ok(())
}

fn parse_port_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.tenant_id.trim()).map_err(|_| {
        PortError::validation(
            "cart.invalid_tenant_id",
            "cart checkout requires a UUID tenant_id",
        )
    })
}

fn cart_error_to_port_error(error: CartError) -> PortError {
    match error {
        CartError::Validation(message) => {
            PortError::validation("cart.checkout_validation", message)
        }
        CartError::CartNotFound(cart_id) => {
            PortError::not_found("cart.not_found", format!("cart {cart_id} not found"))
        }
        CartError::CartLineItemNotFound(line_item_id) => PortError::not_found(
            "cart.line_item_not_found",
            format!("cart line item {line_item_id} not found"),
        ),
        CartError::InvalidTransition { from, to } => PortError::conflict(
            "cart.invalid_transition",
            format!("invalid cart status transition: {from} -> {to}"),
        ),
        CartError::Database(_) => PortError::unavailable(
            "cart.checkout_storage_unavailable",
            "cart checkout storage is unavailable",
        ),
        CartError::TaxBoundary {
            kind,
            code,
            message,
            retryable,
        } => PortError::new(kind, code, message, retryable),
    }
}
