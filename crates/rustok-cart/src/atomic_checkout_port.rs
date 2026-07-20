use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{
    PLATFORM_FALLBACK_LOCALE, PortActor, PortCallPolicy, PortContext, PortError, PortErrorKind,
    normalize_locale_tag,
};
use sea_orm::DatabaseConnection;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use uuid::Uuid;

use crate::{
    CartCheckoutContextUpdateRequest, CartCheckoutLifecycleRequest, CartCheckoutPort,
    CartCheckoutSnapshotPort, CartCheckoutSnapshotRequest, CartError, CartPricingAdjustmentUpdate,
    CartResponse, CartService, CartStatus, PrepareCartCheckoutSnapshotRequest,
    PreparedCartCheckoutSnapshot, in_process_cart_checkout_snapshot_port,
};

const CHECKOUT_PRICING_CHANGED_PREFIX: &str = "checkout pricing snapshot changed:";

type PreparedState = Arc<Mutex<Option<PreparedCartCheckoutSnapshot>>>;

#[derive(Clone, Debug)]
pub struct CartCheckoutLineItemPricingUpdate {
    pub line_item_id: Uuid,
    pub variant_id: Uuid,
    pub quantity: i32,
    pub unit_price: Decimal,
    pub pricing_adjustment: Option<CartPricingAdjustmentUpdate>,
}

#[derive(Clone, Debug)]
pub struct CartCheckoutPricingPlan {
    pub currency_code: String,
    pub effective_region_id: Option<Uuid>,
    pub cart_channel_id: Option<Uuid>,
    pub cart_channel_slug: Option<String>,
    pub line_items: Vec<CartCheckoutLineItemPricingUpdate>,
}

/// Consumer-owned read boundary used by the cart checkout owner after the
/// durable operation lease exists but before the cart compare-and-set lock.
#[async_trait]
pub trait AtomicCartCheckoutPricingResolver: Send + Sync {
    async fn resolve_checkout_pricing(
        &self,
        tenant_id: Uuid,
        cart: &CartResponse,
        request: &PrepareCartCheckoutSnapshotRequest,
    ) -> Result<CartCheckoutPricingPlan, PortError>;
}

/// Request-scoped adapter that preserves the existing `CartCheckoutPort`
/// protocol while deferring persistence until the durable orchestration claim.
pub struct AtomicCartCheckoutPort {
    service: Arc<CartService>,
    snapshot_port: Arc<dyn CartCheckoutSnapshotPort>,
    prepare_request: PrepareCartCheckoutSnapshotRequest,
    pricing_resolver: Option<Arc<dyn AtomicCartCheckoutPricingResolver>>,
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
    pricing_resolver: Option<Arc<dyn AtomicCartCheckoutPricingResolver>>,
    prepared_state: PreparedState,
}

pub struct AtomicCartCheckoutBinding {
    pub port: Arc<dyn CartCheckoutPort>,
    pub handle: AtomicCartCheckoutHandle,
}

impl AtomicCartCheckoutPort {
    pub fn new(
        db: DatabaseConnection,
        prepare_request: PrepareCartCheckoutSnapshotRequest,
    ) -> Self {
        let service = Arc::new(CartService::new(db.clone()));
        Self {
            service,
            snapshot_port: in_process_cart_checkout_snapshot_port(db),
            prepare_request,
            pricing_resolver: None,
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

    /// Prepares the cart once for this request. Pricing is resolved only while
    /// the cart is still active. A retry adopts an existing locked/completed
    /// cart without consulting current price lists.
    pub async fn prepare(
        &self,
        tenant_id: Uuid,
        allow_existing_lock: bool,
    ) -> Result<PreparedCartCheckoutSnapshot, PortError> {
        if let Some(snapshot) = stored_snapshot(&self.prepared_state)? {
            return Ok(snapshot);
        }

        let cart = prepare_bound_cart(
            self.service.as_ref(),
            self.pricing_resolver.as_ref(),
            &self.prepare_request,
            tenant_id,
            allow_existing_lock,
        )
        .await?;

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
    bind_atomic_cart_checkout(db, prepare_request, None)
}

pub fn bind_in_process_atomic_cart_checkout_with_pricing(
    db: DatabaseConnection,
    prepare_request: PrepareCartCheckoutSnapshotRequest,
    pricing_resolver: Arc<dyn AtomicCartCheckoutPricingResolver>,
) -> AtomicCartCheckoutBinding {
    bind_atomic_cart_checkout(db, prepare_request, Some(pricing_resolver))
}

fn bind_atomic_cart_checkout(
    db: DatabaseConnection,
    prepare_request: PrepareCartCheckoutSnapshotRequest,
    pricing_resolver: Option<Arc<dyn AtomicCartCheckoutPricingResolver>>,
) -> AtomicCartCheckoutBinding {
    let service = Arc::new(CartService::new(db.clone()));
    let snapshot_port = in_process_cart_checkout_snapshot_port(db);
    let prepared_state = Arc::new(Mutex::new(None));
    let port = Arc::new(AtomicCartCheckoutPort {
        service: service.clone(),
        snapshot_port: snapshot_port.clone(),
        prepare_request: prepare_request.clone(),
        pricing_resolver: pricing_resolver.clone(),
        prepared_state: prepared_state.clone(),
    });
    let handle = AtomicCartCheckoutHandle {
        service,
        snapshot_port,
        prepare_request,
        pricing_resolver,
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
        prepare_bound_cart(
            self.service.as_ref(),
            self.pricing_resolver.as_ref(),
            &self.prepare_request,
            parse_port_tenant_id(&context)?,
            false,
        )
        .await
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

async fn prepare_bound_cart(
    service: &CartService,
    pricing_resolver: Option<&Arc<dyn AtomicCartCheckoutPricingResolver>>,
    prepare_request: &PrepareCartCheckoutSnapshotRequest,
    tenant_id: Uuid,
    allow_existing_lock: bool,
) -> Result<CartResponse, PortError> {
    let current = service
        .get_cart(tenant_id, prepare_request.cart_id)
        .await
        .map_err(cart_error_to_port_error)?;

    if allow_existing_lock
        && matches!(
            current.status.as_str(),
            status if status == CartStatus::CheckingOut.as_str()
                || status == CartStatus::Completed.as_str()
        )
    {
        return Ok(current);
    }

    let pricing_plan = if current.status == CartStatus::Active.as_str() {
        match pricing_resolver {
            Some(resolver) => Some(
                resolver
                    .resolve_checkout_pricing(tenant_id, &current, prepare_request)
                    .await?,
            ),
            None => None,
        }
    } else {
        None
    };

    match service
        .prepare_checkout_with_pricing(tenant_id, prepare_request.clone(), pricing_plan)
        .await
    {
        Ok(cart) => Ok(cart),
        Err(CartError::InvalidTransition { from, .. })
            if allow_existing_lock
                && matches!(
                    from.as_str(),
                    status if status == CartStatus::CheckingOut.as_str()
                        || status == CartStatus::Completed.as_str()
                ) =>
        {
            service
                .get_cart(tenant_id, prepare_request.cart_id)
                .await
                .map_err(cart_error_to_port_error)
        }
        Err(error) => Err(cart_error_to_port_error(error)),
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

fn stored_snapshot(
    state: &PreparedState,
) -> Result<Option<PreparedCartCheckoutSnapshot>, PortError> {
    state.lock().map(|snapshot| snapshot.clone()).map_err(|_| {
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
        CartError::Validation(message) if message.starts_with(CHECKOUT_PRICING_CHANGED_PREFIX) => {
            PortError::new(
                PortErrorKind::Conflict,
                "cart.checkout_pricing_changed",
                message,
                true,
            )
        }
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
