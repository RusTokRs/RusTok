use rustok_api::{OptionalAuthContext, PortActor, PortContext, RequestContext, TenantContext};
use rustok_cart::{
    CartCheckoutSnapshotRequest, CartStorefrontReadRequest, in_process_cart_checkout_port,
    in_process_cart_storefront_port,
};
use rustok_customer::{CustomerUserProjectionRequest, in_process_customer_read_port};
use rustok_fulfillment::ShippingOptionReadPort;
use rustok_outbox::TransactionalEventBus;
use rustok_pricing::in_process_pricing_read_port;
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone)]
pub struct StorefrontCheckoutRuntime {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    shipping_option_read_port: Arc<dyn ShippingOptionReadPort>,
    product_catalog_read_runtime: rustok_product::ProductCatalogReadRuntime,
}

impl StorefrontCheckoutRuntime {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self::with_shipping_option_read_port(
            db.clone(),
            event_bus,
            rustok_fulfillment::in_process_shipping_option_read_port(db),
        )
    }

    pub fn with_shipping_option_read_port(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        shipping_option_read_port: Arc<dyn ShippingOptionReadPort>,
    ) -> Self {
        let product_catalog_read_runtime =
            rustok_product::ProductCatalogReadRuntime::in_process(db.clone(), event_bus.clone());
        Self {
            db,
            event_bus,
            shipping_option_read_port,
            product_catalog_read_runtime,
        }
    }

    pub fn with_product_catalog_read_runtime(
        mut self,
        runtime: rustok_product::ProductCatalogReadRuntime,
    ) -> Self {
        self.product_catalog_read_runtime = runtime;
        self
    }

    pub fn product_catalog_read_runtime(&self) -> rustok_product::ProductCatalogReadRuntime {
        self.product_catalog_read_runtime.clone()
    }

    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    pub fn db_clone(&self) -> DatabaseConnection {
        self.db.clone()
    }

    pub fn event_bus(&self) -> TransactionalEventBus {
        self.event_bus.clone()
    }

    fn shipping_option_read_port(&self) -> &dyn ShippingOptionReadPort {
        self.shipping_option_read_port.as_ref()
    }
}

#[derive(Clone, Debug)]
pub struct StorefrontPaymentCollectionCommand {
    pub cart_id: Uuid,
    pub metadata: Value,
}

#[derive(Clone, Debug)]
pub struct StorefrontShippingSelectionUpdateInput {
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
    pub selected_shipping_option_id: Option<Uuid>,
}

#[derive(Clone, Debug)]
pub struct StorefrontShippingSelectionCommand {
    pub cart_id: Uuid,
    pub shipping_selections: Vec<StorefrontShippingSelectionUpdateInput>,
}

#[derive(Clone, Debug)]
pub struct StorefrontCheckoutCompletionCommand {
    pub cart_id: Uuid,
    pub create_fulfillment: bool,
    pub metadata: Value,
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct StorefrontCheckoutRuntimeError {
    message: String,
    owner_retryable: Option<bool>,
}

impl StorefrontCheckoutRuntimeError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            owner_retryable: None,
        }
    }

    pub(super) fn from_port_error(error: rustok_api::PortError) -> Self {
        Self {
            message: format!("{error:?}"),
            owner_retryable: Some(error.retryable),
        }
    }

    pub(super) fn owner_retryable(&self) -> Option<bool> {
        self.owner_retryable
    }
}

pub async fn read_storefront_payment_collection(
    runtime: &StorefrontCheckoutRuntime,
    tenant: &TenantContext,
    auth: OptionalAuthContext,
    cart_id: Uuid,
) -> Result<Option<rustok_payment::dto::PaymentCollectionResponse>, StorefrontCheckoutRuntimeError>
{
    let cart = in_process_cart_checkout_port(runtime.db_clone())
        .read_cart_checkout_snapshot(
            storefront_cart_port_context(tenant.id, cart_id),
            CartCheckoutSnapshotRequest {
                cart_id,
                locale: None,
            },
        )
        .await
        .map_err(runtime_error)?;
    let storefront_customer_id =
        resolve_storefront_customer_id(runtime.db_clone(), tenant.id, auth.0).await?;
    ensure_storefront_cart_access(&cart, storefront_customer_id)?;

    rustok_payment::PaymentService::new(runtime.db_clone())
        .find_reusable_collection_by_cart(tenant.id, cart.id)
        .await
        .map_err(runtime_error)
}

pub async fn read_storefront_order_refunds(
    runtime: &StorefrontCheckoutRuntime,
    tenant: &TenantContext,
    request_context: &RequestContext,
    auth: OptionalAuthContext,
    order_id: Uuid,
) -> Result<(Vec<rustok_payment::dto::RefundResponse>, u64), StorefrontCheckoutRuntimeError> {
    let auth = auth.0.ok_or_else(|| {
        StorefrontCheckoutRuntimeError::new("Authentication required to access order refunds")
    })?;
    let customer = in_process_customer_read_port(runtime.db_clone())
        .read_customer_projection_by_user(
            storefront_customer_port_context(tenant.id, auth.user_id),
            CustomerUserProjectionRequest {
                user_id: auth.user_id,
            },
        )
        .await
        .map_err(runtime_error)?;
    let order = match rustok_order::OrderService::new(runtime.db_clone(), runtime.event_bus())
        .get_order_with_locale_fallback(
            tenant.id,
            order_id,
            request_context.locale.as_str(),
            Some(tenant.default_locale.as_str()),
        )
        .await
    {
        Ok(order) => order,
        Err(rustok_order::OrderError::OrderNotFound(_)) => return Ok((Vec::new(), 0)),
        Err(error) => return Err(runtime_error(error)),
    };
    if order.customer_id != Some(customer.id) {
        return Err(StorefrontCheckoutRuntimeError::new(
            "Order does not belong to the current storefront customer",
        ));
    }

    rustok_payment::PaymentService::new(runtime.db_clone())
        .list_refunds(
            tenant.id,
            rustok_payment::dto::ListRefundsInput {
                page: 1,
                per_page: 50,
                payment_collection_id: None,
                order_id: Some(order_id),
                status: None,
            },
        )
        .await
        .map_err(runtime_error)
}

pub async fn create_storefront_payment_collection(
    runtime: &StorefrontCheckoutRuntime,
    tenant: &TenantContext,
    request_context: &RequestContext,
    auth: OptionalAuthContext,
    command: StorefrontPaymentCollectionCommand,
) -> Result<rustok_payment::dto::PaymentCollectionResponse, StorefrontCheckoutRuntimeError> {
    let cart_storefront_port = in_process_cart_storefront_port(runtime.db_clone());
    let cart = cart_storefront_port
        .read_storefront_cart(
            storefront_cart_storefront_port_context(
                tenant.id,
                command.cart_id,
                Some(request_context),
                "read",
                false,
            ),
            CartStorefrontReadRequest {
                cart_id: command.cart_id,
            },
        )
        .await
        .map_err(runtime_error)?;
    let auth_context = auth.0.clone();
    let storefront_customer_id =
        resolve_storefront_customer_id(runtime.db_clone(), tenant.id, auth.0).await?;
    ensure_storefront_cart_access(&cart, storefront_customer_id)?;
    let mut pricing_context = storefront_cart_storefront_port_context(
        tenant.id,
        cart.id,
        Some(request_context),
        "reprice",
        true,
    );
    if let Some(auth) = &auth_context {
        pricing_context.actor = PortActor::user(auth.user_id.to_string());
        pricing_context.claims = auth
            .scopes
            .iter()
            .chain(auth.permissions.iter())
            .cloned()
            .collect();
    }
    let input = rustok_cart::UpdateCartContextInput {
        email: cart.email.clone(),
        region_id: cart.region_id,
        country_code: cart.country_code.clone(),
        locale_code: cart.locale_code.clone(),
        selected_shipping_option_id: cart.selected_shipping_option_id,
        shipping_selections: None,
    };
    let pricing = in_process_pricing_read_port(
        runtime.db_clone(),
        runtime.event_bus(),
        runtime.product_catalog_read_runtime(),
    );
    let command = crate::prepare_storefront_cart_context(
        pricing.as_ref(),
        pricing_context.clone(),
        &cart,
        input,
    )
    .await
    .map_err(port_runtime_error)?;
    let cart = cart_storefront_port
        .update_storefront_context(pricing_context, command)
        .await
        .map_err(port_runtime_error)?;

    let service = rustok_payment::PaymentService::new(runtime.db_clone());
    if let Some(existing) = service
        .find_reusable_collection_by_cart(tenant.id, cart.id)
        .await
        .map_err(runtime_error)?
    {
        return Ok(existing);
    }

    let context = crate::StoreContextService::new(
        runtime.db_clone(),
        std::sync::Arc::new(rustok_region::RegionService::new(runtime.db_clone())),
    )
    .resolve_context(
        tenant.id,
        crate::dto::ResolveStoreContextInput {
            region_id: cart.region_id,
            country_code: cart.country_code.clone(),
            locale: Some(resolve_requested_locale(
                cart.locale_code.clone(),
                Some(request_context.locale.as_str()),
                tenant.default_locale.as_str(),
            )),
            currency_code: Some(cart.currency_code.clone()),
        },
    )
    .await
    .map_err(runtime_error)?;

    service
        .create_collection(
            tenant.id,
            rustok_payment::dto::CreatePaymentCollectionInput {
                cart_id: Some(cart.id),
                order_id: None,
                customer_id: cart.customer_id,
                currency_code: cart.currency_code.clone(),
                amount: cart.total_amount,
                metadata: merge_metadata(command.metadata, cart_context_metadata(&cart, &context)),
            },
        )
        .await
        .map_err(runtime_error)
}

pub async fn select_storefront_shipping_option(
    runtime: &StorefrontCheckoutRuntime,
    tenant: &TenantContext,
    request_context: Option<&RequestContext>,
    auth: OptionalAuthContext,
    command: StorefrontShippingSelectionCommand,
) -> Result<(), StorefrontCheckoutRuntimeError> {
    let request = request_context.ok_or_else(|| {
        StorefrontCheckoutRuntimeError::new(
            "Shipping changes require the admitted storefront request",
        )
    })?;
    if tenant.id.is_nil()
        || request.tenant_id != tenant.id
        || auth
            .0
            .as_ref()
            .is_some_and(|auth| auth.tenant_id != tenant.id || auth.user_id.is_nil())
        || request
            .user_id
            .is_some_and(|id| auth.0.as_ref().map(|auth| auth.user_id) != Some(id))
    {
        return Err(StorefrontCheckoutRuntimeError::new(
            "Shipping request context is unavailable",
        ));
    }
    crate::admit_storefront_buyer_request(runtime.db(), tenant.id, request)
        .await
        .map_err(port_runtime_error)?;
    let mut context = storefront_cart_storefront_port_context(
        tenant.id,
        command.cart_id,
        Some(request),
        "update-context",
        true,
    );
    if let Some(auth) = &auth.0 {
        context.actor = PortActor::user(auth.user_id.to_string());
        context.claims = auth
            .scopes
            .iter()
            .chain(auth.permissions.iter())
            .cloned()
            .collect();
    }
    let cart_storefront_port = in_process_cart_storefront_port(runtime.db_clone());
    let cart = cart_storefront_port
        .read_storefront_cart(
            context.clone(),
            CartStorefrontReadRequest {
                cart_id: command.cart_id,
            },
        )
        .await
        .map_err(runtime_error)?;
    let storefront_customer_id =
        resolve_storefront_customer_id(runtime.db_clone(), tenant.id, auth.0).await?;
    ensure_storefront_cart_access(&cart, storefront_customer_id)?;
    let shipping_selections = command
        .shipping_selections
        .into_iter()
        .map(|selection| rustok_cart::CartShippingSelectionInput {
            shipping_profile_slug: selection.shipping_profile_slug,
            seller_id: selection.seller_id,
            seller_scope: None,
            selected_shipping_option_id: selection.selected_shipping_option_id,
        })
        .collect::<Vec<_>>();
    let input = rustok_cart::UpdateCartContextInput {
        email: cart.email.clone(),
        region_id: cart.region_id,
        country_code: cart.country_code.clone(),
        locale_code: cart.locale_code.clone(),
        selected_shipping_option_id: None,
        shipping_selections: Some(shipping_selections),
    };
    let target = cart
        .project_context(&input)
        .map_err(|_| StorefrontCheckoutRuntimeError::new("Shipping context is invalid"))?;
    let public_channel_slug =
        crate::storefront_channel::resolve_cart_channel(runtime.db(), &context, &target)
            .await
            .map_err(port_runtime_error)?;
    for selection in input.shipping_selections.iter().flatten() {
        if selection.selected_shipping_option_id.is_none() {
            continue;
        }
        crate::storefront_shipping::validate_storefront_shipping_option_selection(
            &target,
            selection,
            target.currency_code.as_str(),
            public_channel_slug.as_deref(),
            target
                .locale_code
                .as_deref()
                .or(Some(request.locale.as_str())),
            Some(tenant.default_locale.as_str()),
            &context,
            runtime.shipping_option_read_port(),
        )
        .await
        .map_err(|_| {
            StorefrontCheckoutRuntimeError::new("storefront shipping selection validation failed")
        })?;
    }
    let pricing = in_process_pricing_read_port(
        runtime.db_clone(),
        runtime.event_bus(),
        runtime.product_catalog_read_runtime(),
    );
    let command =
        crate::prepare_storefront_cart_context(pricing.as_ref(), context.clone(), &cart, input)
            .await
            .map_err(port_runtime_error)?;
    cart_storefront_port
        .update_storefront_context(context, command)
        .await
        .map_err(runtime_error)?;
    Ok(())
}

async fn resolve_storefront_customer_id(
    db: sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    auth: Option<rustok_api::AuthContext>,
) -> Result<Option<Uuid>, StorefrontCheckoutRuntimeError> {
    let Some(auth) = auth else {
        return Ok(None);
    };

    match in_process_customer_read_port(db)
        .read_customer_projection_by_user(
            storefront_customer_port_context(tenant_id, auth.user_id),
            CustomerUserProjectionRequest {
                user_id: auth.user_id,
            },
        )
        .await
    {
        Ok(customer) => Ok(Some(customer.id)),
        Err(error) if error.code == "customer.customer_by_user_not_found" => Ok(None),
        Err(error) => Err(runtime_error(error.message)),
    }
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

fn storefront_cart_port_context(tenant_id: Uuid, cart_id: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service("rustok-commerce.storefront"),
        "en",
        format!("storefront-cart:{cart_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2))
}

fn storefront_cart_storefront_port_context(
    tenant_id: Uuid,
    cart_id: Uuid,
    request_context: Option<&RequestContext>,
    operation: &str,
    is_write: bool,
) -> PortContext {
    let operation_key = format!("storefront-cart:{operation}:{cart_id}");
    let correlation_id = request_context
        .map(|context| context.correlation_id.clone())
        .unwrap_or_else(|| operation_key.clone());
    let locale = request_context
        .map(|context| context.locale.as_str())
        .unwrap_or(rustok_api::PLATFORM_FALLBACK_LOCALE);
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::service("rustok-commerce.storefront"),
        locale,
        correlation_id.clone(),
    )
    .with_causation_id(operation_key.clone())
    .with_deadline(std::time::Duration::from_secs(2));
    let context = request_context
        .and_then(|request| request.channel_slug.as_deref())
        .map(|channel| context.clone().with_channel(channel))
        .unwrap_or(context);
    if is_write {
        context.with_idempotency_key(operation_key)
    } else {
        context
    }
}

fn ensure_storefront_cart_access(
    cart: &rustok_cart::dto::CartResponse,
    storefront_customer_id: Option<Uuid>,
) -> Result<(), StorefrontCheckoutRuntimeError> {
    if let Some(owner_customer_id) = cart.customer_id {
        match storefront_customer_id {
            Some(customer_id) if customer_id == owner_customer_id => Ok(()),
            Some(_) => Err(StorefrontCheckoutRuntimeError::new(
                "Cart does not belong to the current storefront customer",
            )),
            None => Err(StorefrontCheckoutRuntimeError::new(
                "Authentication required to access this cart",
            )),
        }
    } else {
        Ok(())
    }
}

fn merge_metadata(current: Value, patch: Value) -> Value {
    match (current, patch) {
        (Value::Object(mut current), Value::Object(patch)) => {
            for (key, value) in patch {
                current.insert(key, value);
            }
            Value::Object(current)
        }
        (_, patch) => patch,
    }
}

fn cart_context_metadata(
    cart: &rustok_cart::dto::CartResponse,
    context: &crate::dto::StoreContextResponse,
) -> Value {
    json!({
        "cart_context": {
            "region_id": cart.region_id,
            "country_code": cart.country_code,
            "locale": context.locale,
            "currency_code": context.currency_code,
            "selected_shipping_option_id": cart.selected_shipping_option_id,
            "email": cart.email,
        }
    })
}

fn resolve_requested_locale(
    requested: Option<String>,
    request_context_locale: Option<&str>,
    tenant_default_locale: &str,
) -> String {
    normalize_optional(requested)
        .or_else(|| {
            request_context_locale.and_then(|value| normalize_optional(Some(value.to_string())))
        })
        .or_else(|| normalize_optional(Some(tenant_default_locale.to_string())))
        .unwrap_or_default()
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn normalize_public_channel_slug(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

fn runtime_error(error: impl std::fmt::Debug) -> StorefrontCheckoutRuntimeError {
    StorefrontCheckoutRuntimeError::new(format!("{error:?}"))
}

fn port_runtime_error(error: rustok_api::PortError) -> StorefrontCheckoutRuntimeError {
    StorefrontCheckoutRuntimeError::from_port_error(error)
}
