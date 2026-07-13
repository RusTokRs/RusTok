use thiserror::Error;
use tracing::instrument;
use uuid::Uuid;
use validator::Validate;

use rustok_api::{PLATFORM_FALLBACK_LOCALE, normalize_locale_tag};
use rustok_api::{PortActor, PortContext, PortError, PortErrorKind};
use rustok_cart::error::CartError;
use rustok_cart::{
    CartCheckoutContextUpdateRequest, CartCheckoutLifecycleRequest, CartCheckoutPort,
    CartCheckoutSnapshotRequest,
};
use rustok_fulfillment::error::FulfillmentError;
use rustok_fulfillment::providers::FulfillmentProviderRegistry;
use rustok_inventory::{InventoryAvailabilityRequest, InventoryReservationPort};
use rustok_order::error::OrderError;
use rustok_outbox::TransactionalEventBus;
use rustok_payment::PaymentService;
use rustok_payment::error::PaymentError;
use rustok_payment::providers::{PaymentProviderOperationRequest, PaymentProviderRegistry};
use rustok_product::{
    ProductCatalogReadPort, ProductProjectionRequest, VariantProductProjectionRequest,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use std::{collections::BTreeSet, sync::Arc, time::Duration};

use crate::StoreContextService;
use crate::dto::{
    AuthorizePaymentInput, CancelPaymentInput, CompleteCheckoutInput, CompleteCheckoutResponse,
    CreateFulfillmentInput, CreateOrderAdjustmentInput, CreateOrderInput, CreateOrderLineItemInput,
    CreateOrderTaxLineInput, CreatePaymentCollectionInput, ResolveStoreContextInput,
    UpdateCartContextInput,
};
use crate::storefront_channel::{
    is_metadata_visible_for_public_channel, normalize_public_channel_slug,
};
use crate::storefront_shipping::{
    effective_shipping_profile_slug, is_shipping_option_compatible_with_profiles,
};
use rustok_commerce_foundation::entities::product::ProductStatus;
use rustok_fulfillment::FulfillmentService;
use rustok_order::OrderService;

const MANUAL_PROVIDER_ID: &str = "manual";

#[derive(Debug, Error)]
pub enum CheckoutError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("cart {0} cannot be checked out in its current state")]
    CartNotReady(Uuid),
    #[error("checkout for cart {0} is already in progress")]
    CheckoutInProgress(Uuid),
    #[error("cart {0} has no line items")]
    EmptyCart(Uuid),
    #[error(
        "checkout boundary `{stage}` failed with `{code}` ({kind:?}, retryable={retryable}): {message}"
    )]
    BoundaryFailure {
        stage: &'static str,
        kind: PortErrorKind,
        code: String,
        message: String,
        retryable: bool,
    },
    #[error("checkout failed at stage `{stage}`: {source}")]
    StageFailure {
        stage: &'static str,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

pub type CheckoutResult<T> = Result<T, CheckoutError>;

pub struct CheckoutService {
    db: DatabaseConnection,
    cart_checkout_port: Arc<dyn CartCheckoutPort>,
    inventory_reservation_port: Arc<dyn InventoryReservationPort>,
    product_catalog_read_port: Arc<dyn ProductCatalogReadPort>,
    order_service: OrderService,
    payment_service: PaymentService,
    payment_provider_registry: PaymentProviderRegistry,
    fulfillment_service: FulfillmentService,
    context_service: StoreContextService,
}

impl CheckoutService {
    pub fn new(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        region_read_port: Arc<dyn rustok_region::RegionReadPort>,
        cart_checkout_port: Arc<dyn CartCheckoutPort>,
        inventory_reservation_port: Arc<dyn InventoryReservationPort>,
        product_catalog_read_port: Arc<dyn ProductCatalogReadPort>,
    ) -> Self {
        Self {
            db: db.clone(),
            cart_checkout_port,
            inventory_reservation_port,
            product_catalog_read_port,
            order_service: OrderService::new(db.clone(), event_bus),
            payment_service: PaymentService::new(db.clone()),
            payment_provider_registry: PaymentProviderRegistry::with_manual_provider(),
            fulfillment_service: FulfillmentService::new(db.clone()),
            context_service: StoreContextService::new(db, region_read_port),
        }
    }

    /// Override provider registries assembled by runtime composition.
    ///
    /// Payment side effects remain synchronous checkout dependencies. Fulfillment
    /// providers are accepted for API compatibility, but label execution is owned by
    /// the durable paid-order listener and recovery worker after payment is committed.
    pub fn with_provider_registries(
        mut self,
        payment_provider_registry: PaymentProviderRegistry,
        _fulfillment_provider_registry: FulfillmentProviderRegistry,
    ) -> Self {
        self.payment_provider_registry = payment_provider_registry;
        self
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, actor_id = %actor_id))]
    pub async fn complete_checkout(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: CompleteCheckoutInput,
    ) -> CheckoutResult<CompleteCheckoutResponse> {
        input
            .validate()
            .map_err(|error| CheckoutError::Validation(error.to_string()))?;

        let mut cart = self
            .cart_checkout_port
            .read_cart_checkout_snapshot(
                checkout_cart_port_context(
                    tenant_id,
                    actor_id,
                    input.cart_id,
                    input.locale.as_deref(),
                    None,
                    "read",
                    false,
                ),
                CartCheckoutSnapshotRequest {
                    cart_id: input.cart_id,
                    locale: input.locale.clone(),
                },
            )
            .await
            .map_err(|error| checkout_port_error("read_cart_checkout_snapshot", error))?;
        if input.shipping_selections.is_some() || input.shipping_option_id.is_some() {
            cart = self
                .cart_checkout_port
                .update_cart_checkout_context(
                    checkout_cart_port_context(
                        tenant_id,
                        actor_id,
                        cart.id,
                        cart.locale_code.as_deref(),
                        cart.channel_slug.as_deref(),
                        "update_context",
                        true,
                    ),
                    CartCheckoutContextUpdateRequest {
                        cart_id: cart.id,
                        input: UpdateCartContextInput {
                            email: cart.email.clone(),
                            region_id: cart.region_id,
                            country_code: cart.country_code.clone(),
                            locale_code: cart.locale_code.clone(),
                            selected_shipping_option_id: input.shipping_option_id,
                            shipping_selections: input.shipping_selections.clone(),
                        },
                    },
                )
                .await
                .map_err(|error| checkout_port_error("update_cart_checkout_context", error))?;
        }
        if cart.status == "completed" {
            if let Some(response) = self
                .recover_existing_checkout(tenant_id, actor_id, cart.clone())
                .await?
            {
                return Ok(response);
            }
            return Err(CheckoutError::CartNotReady(cart.id));
        }
        if cart.status == "checking_out" {
            if let Some(response) = self
                .recover_existing_checkout(tenant_id, actor_id, cart.clone())
                .await?
            {
                return Ok(response);
            }
            return Err(CheckoutError::CheckoutInProgress(input.cart_id));
        }
        if cart.status != "active" {
            return Err(CheckoutError::CartNotReady(cart.id));
        }
        if cart.line_items.is_empty() {
            return Err(CheckoutError::EmptyCart(cart.id));
        }
        let cart = self
            .cart_checkout_port
            .begin_cart_checkout(
                checkout_cart_port_context(
                    tenant_id,
                    actor_id,
                    cart.id,
                    cart.locale_code.as_deref(),
                    cart.channel_slug.as_deref(),
                    "begin",
                    true,
                ),
                CartCheckoutLifecycleRequest { cart_id: cart.id },
            )
            .await
            .map_err(|error| checkout_port_error("begin_cart_checkout", error))?;
        if let Err(error) = self
            .validate_cart_inventory(tenant_id, actor_id, &cart)
            .await
        {
            let _ = self.release_cart_checkout(tenant_id, actor_id, &cart).await;
            return Err(error);
        }
        let context = match self
            .context_service
            .resolve_context(
                tenant_id,
                ResolveStoreContextInput {
                    region_id: cart.region_id.or(input.region_id),
                    country_code: cart.country_code.clone().or(input.country_code.clone()),
                    locale: cart.locale_code.clone().or(input.locale.clone()),
                    currency_code: Some(cart.currency_code.clone()),
                },
            )
            .await
        {
            Ok(context) => context,
            Err(error) => {
                let _ = self.release_cart_checkout(tenant_id, actor_id, &cart).await;
                return Err(stage_error("resolve_context")(error));
            }
        };
        if let Err(error) = self
            .validate_delivery_groups(
                tenant_id,
                &cart,
                context.locale.as_str(),
                Some(context.default_locale.as_str()),
            )
            .await
        {
            let _ = self.release_cart_checkout(tenant_id, actor_id, &cart).await;
            return Err(error);
        }
        let order_metadata = merge_checkout_metadata(
            input.metadata.clone(),
            checkout_cart_context_metadata(&cart, &context),
        );
        let checkout_result: CheckoutResult<CompleteCheckoutResponse> = async {
            let mut order = self
                .order_service
                .create_order_with_channel(
                    tenant_id,
                    actor_id,
                    CreateOrderInput {
                        customer_id: cart.customer_id,
                        currency_code: cart.currency_code.clone(),
                        shipping_total: cart.shipping_total,
                        line_items: cart
                            .line_items
                            .iter()
                            .map(|item| CreateOrderLineItemInput {
                                product_id: item.product_id,
                                variant_id: item.variant_id,
                                shipping_profile_slug: item.shipping_profile_slug.clone(),
                                seller_id: item.seller_id.clone(),
                                sku: item.sku.clone(),
                                title: item.title.clone(),
                                quantity: item.quantity,
                                unit_price: item.unit_price,
                                metadata: merge_checkout_metadata(
                                    item.metadata.clone(),
                                    checkout_order_line_item_metadata(item.id),
                                ),
                            })
                            .collect(),
                        adjustments: checkout_order_adjustments(&cart),
                        tax_lines: checkout_order_tax_lines(&cart),
                        metadata: order_metadata.clone(),
                    },
                    cart.channel_id,
                    cart.channel_slug.clone(),
                )
                .await
                .map_err(stage_error("create_order"))?;

            if let Err(error) = self
                .order_service
                .confirm_order(tenant_id, actor_id, order.id)
                .await
            {
                self.compensate_order(tenant_id, actor_id, order.id, "confirm_order_failed")
                    .await;
                return Err(stage_error("confirm_order")(error));
            } else {
                order = self
                    .order_service
                    .get_order_with_locale_fallback(
                        tenant_id,
                        order.id,
                        context.locale.as_str(),
                        Some(context.default_locale.as_str()),
                    )
                    .await
                    .map_err(stage_error("reload_order"))?;
            }

            let payment_collection = match self
                .payment_service
                .find_reusable_collection_by_cart(tenant_id, cart.id)
                .await
            {
                Ok(Some(existing)) => match self
                    .payment_service
                    .attach_order_to_collection(
                        tenant_id,
                        existing.id,
                        order.id,
                        input.metadata.clone(),
                    )
                    .await
                {
                    Ok(collection) => collection,
                    Err(error) => {
                        self.compensate_order(
                            tenant_id,
                            actor_id,
                            order.id,
                            "payment_collection_failed",
                        )
                        .await;
                        return Err(stage_error("attach_payment_collection")(error));
                    }
                },
                Ok(None) => match self
                    .payment_service
                    .create_collection(
                        tenant_id,
                        CreatePaymentCollectionInput {
                            cart_id: Some(cart.id),
                            order_id: Some(order.id),
                            customer_id: cart.customer_id,
                            currency_code: cart.currency_code.clone(),
                            amount: cart.total_amount,
                            metadata: input.metadata.clone(),
                        },
                    )
                    .await
                {
                    Ok(collection) => collection,
                    Err(error) => {
                        self.compensate_order(
                            tenant_id,
                            actor_id,
                            order.id,
                            "payment_collection_failed",
                        )
                        .await;
                        return Err(stage_error("create_payment_collection")(error));
                    }
                },
                Err(error) => {
                    self.compensate_order(
                        tenant_id,
                        actor_id,
                        order.id,
                        "payment_collection_failed",
                    )
                    .await;
                    return Err(stage_error("load_payment_collection")(error));
                }
            };

            let authorized_payment = match payment_collection.status.as_str() {
                "pending" => {
                    let provider_id = payment_collection
                        .provider_id
                        .clone()
                        .unwrap_or_else(|| MANUAL_PROVIDER_ID.to_string());
                    let provider_result = match self
                        .payment_provider_registry
                        .execute_authorize(
                            provider_id.as_str(),
                            PaymentProviderOperationRequest {
                                tenant_id,
                                collection_id: payment_collection.id,
                                amount: cart.total_amount,
                                currency_code: cart.currency_code.clone(),
                                idempotency_key: Some(format!(
                                    "checkout:{}:authorize:{}",
                                    cart.id, payment_collection.id
                                )),
                                metadata: input.metadata.clone(),
                            },
                        )
                        .await
                    {
                        Ok(result) => result,
                        Err(error) => {
                            self.compensate_payment_and_order(
                                tenant_id,
                                actor_id,
                                payment_collection.id,
                                order.id,
                                "payment_provider_authorization_failed",
                            )
                            .await;
                            return Err(stage_error("execute_authorize_payment_provider")(error));
                        }
                    };
                    match self
                        .payment_service
                        .authorize_collection(
                            tenant_id,
                            payment_collection.id,
                            AuthorizePaymentInput {
                                provider_id: Some(provider_result.provider_id),
                                provider_payment_id: provider_result.external_reference,
                                amount: Some(provider_result.authorized_amount),
                                metadata: provider_result.metadata,
                            },
                        )
                        .await
                    {
                        Ok(collection) => collection,
                        Err(error) => {
                            self.compensate_payment_and_order(
                                tenant_id,
                                actor_id,
                                payment_collection.id,
                                order.id,
                                "payment_authorization_failed",
                            )
                            .await;
                            return Err(stage_error("authorize_payment")(error));
                        }
                    }
                }
                "authorized" | "captured" => payment_collection.clone(),
                status => {
                    self.compensate_payment_and_order(
                        tenant_id,
                        actor_id,
                        payment_collection.id,
                        order.id,
                        "payment_authorization_failed",
                    )
                    .await;
                    return Err(stage_error("authorize_payment")(
                        PaymentError::InvalidTransition {
                            from: status.to_string(),
                            to: "authorized".to_string(),
                        },
                    ));
                }
            };

            let fulfillments = if input.create_fulfillment {
                match self
                    .create_fulfillments_for_delivery_groups(
                        tenant_id,
                        &order,
                        cart.customer_id,
                        &cart,
                        input.metadata.clone(),
                    )
                    .await
                {
                    Ok(fulfillments) => fulfillments,
                    Err(error) => {
                        self.compensate_payment_and_order(
                            tenant_id,
                            actor_id,
                            authorized_payment.id,
                            order.id,
                            "fulfillment_creation_failed",
                        )
                        .await;
                        return Err(error);
                    }
                }
            } else {
                Vec::new()
            };

            let captured_payment = match authorized_payment.status.as_str() {
                "authorized" => {
                    let provider_id = authorized_payment
                        .provider_id
                        .clone()
                        .unwrap_or_else(|| MANUAL_PROVIDER_ID.to_string());
                    let provider_result = match self
                        .payment_provider_registry
                        .execute_capture(
                            provider_id.as_str(),
                            PaymentProviderOperationRequest {
                                tenant_id,
                                collection_id: authorized_payment.id,
                                amount: cart.total_amount,
                                currency_code: cart.currency_code.clone(),
                                idempotency_key: Some(format!(
                                    "checkout:{}:capture:{}",
                                    cart.id, authorized_payment.id
                                )),
                                metadata: input.metadata.clone(),
                            },
                        )
                        .await
                    {
                        Ok(result) => result,
                        Err(error) => {
                            self.compensate_payment_and_order(
                                tenant_id,
                                actor_id,
                                authorized_payment.id,
                                order.id,
                                "payment_provider_capture_failed",
                            )
                            .await;
                            return Err(stage_error("execute_capture_payment_provider")(error));
                        }
                    };
                    match self
                        .payment_service
                        .capture_collection(
                            tenant_id,
                            authorized_payment.id,
                            rustok_payment::dto::CapturePaymentInput {
                                amount: Some(provider_result.captured_amount),
                                metadata: provider_result.metadata,
                            },
                        )
                        .await
                    {
                        Ok(collection) => collection,
                        Err(error) => {
                            self.compensate_payment_and_order(
                                tenant_id,
                                actor_id,
                                authorized_payment.id,
                                order.id,
                                "payment_capture_failed",
                            )
                            .await;
                            return Err(stage_error("capture_payment")(error));
                        }
                    }
                }
                "captured" => authorized_payment,
                status => {
                    self.compensate_payment_and_order(
                        tenant_id,
                        actor_id,
                        authorized_payment.id,
                        order.id,
                        "payment_capture_failed",
                    )
                    .await;
                    return Err(stage_error("capture_payment")(
                        PaymentError::InvalidTransition {
                            from: status.to_string(),
                            to: "captured".to_string(),
                        },
                    ));
                }
            };
            let payment_reference = captured_payment
                .payments
                .last()
                .map(|payment| payment.provider_payment_id.clone())
                .unwrap_or_else(|| format!("manual_{}", order.id));
            let payment_method = captured_payment
                .provider_id
                .clone()
                .unwrap_or_else(|| MANUAL_PROVIDER_ID.to_string());

            let order = self
                .order_service
                .mark_paid(
                    tenant_id,
                    actor_id,
                    order.id,
                    payment_reference,
                    payment_method,
                )
                .await
                .map_err(stage_error("mark_order_paid"))?;

            let cart = self
                .cart_checkout_port
                .complete_cart_checkout(
                    checkout_cart_port_context(
                        tenant_id,
                        actor_id,
                        cart.id,
                        cart.locale_code.as_deref(),
                        cart.channel_slug.as_deref(),
                        "complete",
                        true,
                    ),
                    CartCheckoutLifecycleRequest { cart_id: cart.id },
                )
                .await
                .map_err(|error| checkout_port_error("complete_cart_checkout", error))?;

            Ok(CompleteCheckoutResponse {
                cart,
                order,
                payment_collection: captured_payment,
                fulfillment: fulfillment_shim(&fulfillments),
                fulfillments,
                context,
            })
        }
        .await;

        if should_release_checkout_lock(&checkout_result) {
            let _ = self.release_cart_checkout(tenant_id, actor_id, &cart).await;
        }

        checkout_result
    }

    async fn validate_cart_inventory(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        cart: &rustok_cart::dto::CartResponse,
    ) -> CheckoutResult<()> {
        let public_channel_slug = normalize_public_channel_slug(cart.channel_slug.as_deref());
        for line_item in &cart.line_items {
            let Some(variant_id) = line_item.variant_id else {
                continue;
            };

            let product = match line_item.product_id {
                Some(product_id) => {
                    self.product_catalog_read_port
                        .read_product_projection(
                            checkout_product_port_context(
                                tenant_id,
                                actor_id,
                                cart.id,
                                cart.locale_code.as_deref(),
                                public_channel_slug.as_deref(),
                            ),
                            ProductProjectionRequest {
                                product_id,
                                locale: cart.locale_code.clone(),
                                fallback_locale: None,
                            },
                        )
                        .await
                }
                None => {
                    self.product_catalog_read_port
                        .read_variant_product_projection(
                            checkout_product_port_context(
                                tenant_id,
                                actor_id,
                                cart.id,
                                cart.locale_code.as_deref(),
                                public_channel_slug.as_deref(),
                            ),
                            VariantProductProjectionRequest {
                                variant_id,
                                locale: cart.locale_code.clone(),
                                fallback_locale: None,
                            },
                        )
                        .await
                }
            }
            .map_err(|error| checkout_port_error("read_checkout_product_projection", error))?;
            let variant = product
                .variants
                .iter()
                .find(|variant| variant.id == variant_id)
                .ok_or_else(|| {
                    CheckoutError::Validation(format!(
                        "Variant {} is no longer available for checkout",
                        variant_id
                    ))
                })?;

            if product.status != ProductStatus::Active
                || product.published_at.is_none()
                || !is_metadata_visible_for_public_channel(
                    &product.metadata,
                    public_channel_slug.as_deref(),
                )
            {
                return Err(CheckoutError::Validation(format!(
                    "Product {} is not available for the cart channel",
                    product.id
                )));
            }
            let current_shipping_profile_slug = effective_shipping_profile_slug(
                product.shipping_profile_slug.as_deref(),
                &product.metadata,
                variant.shipping_profile_slug.as_deref(),
            );
            if current_shipping_profile_slug != line_item.shipping_profile_slug {
                return Err(CheckoutError::Validation(format!(
                    "Line item {} uses stale shipping profile snapshot {} (current: {})",
                    line_item.id, line_item.shipping_profile_slug, current_shipping_profile_slug
                )));
            }

            let availability = self
                .inventory_reservation_port
                .check_availability(
                    checkout_inventory_port_context(
                        tenant_id,
                        actor_id,
                        cart.id,
                        cart.locale_code.as_deref(),
                        public_channel_slug.as_deref(),
                    ),
                    InventoryAvailabilityRequest {
                        variant_id: variant.id,
                        requested_quantity: line_item.quantity,
                        channel_slug: public_channel_slug.clone(),
                    },
                )
                .await
                .map_err(|error| checkout_port_error("check_inventory_availability", error))?;
            if !availability.available {
                return Err(CheckoutError::Validation(format!(
                    "Variant {} does not have enough available inventory for the cart channel",
                    variant.id
                )));
            }
        }

        Ok(())
    }

    async fn release_cart_checkout(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        cart: &rustok_cart::dto::CartResponse,
    ) -> Result<(), CheckoutError> {
        self.cart_checkout_port
            .release_cart_checkout(
                checkout_cart_port_context(
                    tenant_id,
                    actor_id,
                    cart.id,
                    cart.locale_code.as_deref(),
                    cart.channel_slug.as_deref(),
                    "release",
                    true,
                ),
                CartCheckoutLifecycleRequest { cart_id: cart.id },
            )
            .await
            .map(|_| ())
            .map_err(|error| checkout_port_error("release_cart_checkout", error))
    }

    async fn recover_existing_checkout(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        cart: rustok_cart::dto::CartResponse,
    ) -> CheckoutResult<Option<CompleteCheckoutResponse>> {
        let Some(payment_collection) = self
            .payment_service
            .find_latest_collection_by_cart(tenant_id, cart.id)
            .await
            .map_err(stage_error("load_payment_collection"))?
        else {
            return Ok(None);
        };
        let Some(order_id) = payment_collection.order_id else {
            return Ok(None);
        };

        let order_locale = match cart.locale_code.as_deref() {
            Some(locale) => locale.to_string(),
            None => load_tenant_default_locale(&self.db, tenant_id).await?,
        };
        let order = self
            .order_service
            .get_order_with_locale_fallback(tenant_id, order_id, order_locale.as_str(), None)
            .await
            .map_err(stage_error("load_order"))?;
        let is_completed_checkout =
            payment_collection.status == "captured" && order.status == "paid";
        if !is_completed_checkout {
            return Ok(None);
        }

        let cart = if cart.status == "checking_out" {
            self.cart_checkout_port
                .complete_cart_checkout(
                    checkout_cart_port_context(
                        tenant_id,
                        actor_id,
                        cart.id,
                        cart.locale_code.as_deref(),
                        cart.channel_slug.as_deref(),
                        "recover_complete",
                        true,
                    ),
                    CartCheckoutLifecycleRequest { cart_id: cart.id },
                )
                .await
                .map_err(|error| checkout_port_error("complete_recovered_cart_checkout", error))?
        } else {
            cart
        };
        let fulfillments = self
            .fulfillment_service
            .list_by_order(tenant_id, order.id)
            .await
            .map_err(stage_error("load_fulfillments"))?;
        let context = self
            .context_service
            .resolve_context(
                tenant_id,
                ResolveStoreContextInput {
                    region_id: cart.region_id,
                    country_code: cart.country_code.clone(),
                    locale: cart.locale_code.clone(),
                    currency_code: Some(cart.currency_code.clone()),
                },
            )
            .await
            .map_err(stage_error("resolve_context"))?;

        Ok(Some(CompleteCheckoutResponse {
            cart,
            order,
            payment_collection,
            fulfillment: fulfillment_shim(&fulfillments),
            fulfillments,
            context,
        }))
    }

    async fn validate_delivery_groups(
        &self,
        tenant_id: Uuid,
        cart: &rustok_cart::dto::CartResponse,
        requested_locale: &str,
        tenant_default_locale: Option<&str>,
    ) -> CheckoutResult<()> {
        let public_channel_slug = normalize_public_channel_slug(cart.channel_slug.as_deref());

        for delivery_group in &cart.delivery_groups {
            let Some(selected_shipping_option_id) = delivery_group.selected_shipping_option_id
            else {
                return Err(CheckoutError::Validation(format!(
                    "Delivery group {} does not have a selected shipping option",
                    delivery_group.shipping_profile_slug
                )));
            };
            let option = self
                .fulfillment_service
                .get_shipping_option(
                    tenant_id,
                    selected_shipping_option_id,
                    Some(requested_locale),
                    tenant_default_locale,
                )
                .await
                .map_err(stage_error("load_shipping_option"))?;
            if !option
                .currency_code
                .eq_ignore_ascii_case(&cart.currency_code)
            {
                return Err(CheckoutError::Validation(format!(
                    "Shipping option {} uses currency {}, expected {}",
                    option.id, option.currency_code, cart.currency_code
                )));
            }
            if !is_metadata_visible_for_public_channel(
                &option.metadata,
                public_channel_slug.as_deref(),
            ) {
                return Err(CheckoutError::Validation(format!(
                    "Shipping option {} is not available for the cart channel",
                    option.id
                )));
            }
            let required_shipping_profiles =
                BTreeSet::from([delivery_group.shipping_profile_slug.clone()]);
            if !is_shipping_option_compatible_with_profiles(&option, &required_shipping_profiles) {
                return Err(CheckoutError::Validation(format!(
                    "Shipping option {} is not compatible with delivery group {}",
                    option.id, delivery_group.shipping_profile_slug
                )));
            }
        }

        Ok(())
    }

    async fn create_fulfillments_for_delivery_groups(
        &self,
        tenant_id: Uuid,
        order: &crate::dto::OrderResponse,
        customer_id: Option<Uuid>,
        cart: &rustok_cart::dto::CartResponse,
        metadata: serde_json::Value,
    ) -> CheckoutResult<Vec<rustok_fulfillment::dto::FulfillmentResponse>> {
        let mut fulfillments = Vec::with_capacity(cart.delivery_groups.len());

        for delivery_group in &cart.delivery_groups {
            let items = fulfillment_items_for_delivery_group(order, delivery_group)?;
            let selected_shipping_option_id = delivery_group.selected_shipping_option_id;
            let group_metadata = merge_checkout_metadata(
                metadata.clone(),
                serde_json::json!({
                    "delivery_group": {
                        "shipping_profile_slug": delivery_group.shipping_profile_slug,
                        "seller_id": delivery_group.seller_id,
                        "line_item_ids": delivery_group.line_item_ids,
                    }
                }),
            );
            let fulfillment = self
                .fulfillment_service
                .create_fulfillment(
                    tenant_id,
                    CreateFulfillmentInput {
                        order_id: order.id,
                        shipping_option_id: selected_shipping_option_id,
                        customer_id,
                        carrier: None,
                        tracking_number: None,
                        items: Some(items),
                        metadata: group_metadata,
                    },
                )
                .await
                .map_err(stage_error("create_fulfillment"))?;

            fulfillments.push(fulfillment);
        }

        Ok(fulfillments)
    }

    async fn compensate_order(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        order_id: Uuid,
        reason: &str,
    ) {
        let _ = self
            .order_service
            .cancel_order(tenant_id, actor_id, order_id, Some(reason.to_string()))
            .await;
    }

    async fn compensate_payment_and_order(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        payment_collection_id: Uuid,
        order_id: Uuid,
        reason: &str,
    ) {
        let _ = self
            .payment_service
            .cancel_collection(
                tenant_id,
                payment_collection_id,
                CancelPaymentInput {
                    reason: Some(reason.to_string()),
                    metadata: serde_json::json!({ "compensated": true }),
                },
            )
            .await;
        let _ = self
            .order_service
            .cancel_order(tenant_id, actor_id, order_id, Some(reason.to_string()))
            .await;
    }
}

fn checkout_inventory_port_context(
    tenant_id: Uuid,
    actor_id: Uuid,
    cart_id: Uuid,
    locale: Option<&str>,
    channel_slug: Option<&str>,
) -> PortContext {
    let locale = locale
        .and_then(normalize_locale_tag)
        .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        locale,
        format!("checkout:{cart_id}:inventory"),
    )
    .with_deadline(Duration::from_secs(2));

    match channel_slug {
        Some(channel_slug) => context.with_channel(channel_slug),
        None => context,
    }
}

fn checkout_product_port_context(
    tenant_id: Uuid,
    actor_id: Uuid,
    cart_id: Uuid,
    locale: Option<&str>,
    channel_slug: Option<&str>,
) -> PortContext {
    let locale = locale
        .and_then(normalize_locale_tag)
        .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        locale,
        format!("checkout:{cart_id}:product"),
    )
    .with_deadline(Duration::from_secs(2));

    match channel_slug {
        Some(channel_slug) => context.with_channel(channel_slug),
        None => context,
    }
}

fn checkout_cart_port_context(
    tenant_id: Uuid,
    actor_id: Uuid,
    cart_id: Uuid,
    locale: Option<&str>,
    channel_slug: Option<&str>,
    operation: &str,
    write: bool,
) -> PortContext {
    let locale = locale
        .and_then(normalize_locale_tag)
        .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        locale,
        format!("checkout:{cart_id}:cart:{operation}"),
    )
    .with_deadline(Duration::from_secs(2));
    let context = match channel_slug {
        Some(channel_slug) => context.with_channel(channel_slug),
        None => context,
    };

    if write {
        context.with_idempotency_key(format!("checkout:{cart_id}:cart:{operation}"))
    } else {
        context
    }
}

fn checkout_port_error(stage: &'static str, error: PortError) -> CheckoutError {
    CheckoutError::BoundaryFailure {
        stage,
        kind: error.kind,
        code: error.code,
        message: error.message,
        retryable: error.retryable,
    }
}

async fn load_tenant_default_locale<C>(conn: &C, tenant_id: Uuid) -> CheckoutResult<String>
where
    C: ConnectionTrait,
{
    let row = conn
        .query_one(Statement::from_sql_and_values(
            conn.get_database_backend(),
            "SELECT default_locale FROM tenants WHERE id = ?",
            vec![tenant_id.into()],
        ))
        .await
        .map_err(stage_error("load_tenant_default_locale"))?;

    Ok(row
        .and_then(|row| row.try_get::<String>("", "default_locale").ok())
        .and_then(|locale| normalize_locale_tag(&locale))
        .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string()))
}

fn stage_error<E>(stage: &'static str) -> impl FnOnce(E) -> CheckoutError
where
    E: std::error::Error + Send + Sync + 'static,
{
    move |source| CheckoutError::StageFailure {
        stage,
        source: Box::new(source),
    }
}

fn should_release_checkout_lock(result: &CheckoutResult<CompleteCheckoutResponse>) -> bool {
    match result {
        Err(CheckoutError::StageFailure { stage, .. }) => {
            !matches!(*stage, "mark_order_paid" | "complete_cart")
        }
        Err(_) => true,
        Ok(_) => false,
    }
}

fn merge_checkout_metadata(base: serde_json::Value, patch: serde_json::Value) -> serde_json::Value {
    match (base, patch) {
        (serde_json::Value::Object(mut base), serde_json::Value::Object(patch)) => {
            for (key, value) in patch {
                base.insert(key, value);
            }
            serde_json::Value::Object(base)
        }
        (_, patch) => patch,
    }
}

fn checkout_cart_context_metadata(
    cart: &rustok_cart::dto::CartResponse,
    context: &crate::dto::StoreContextResponse,
) -> serde_json::Value {
    serde_json::json!({
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

fn checkout_order_line_item_metadata(cart_line_item_id: Uuid) -> serde_json::Value {
    serde_json::json!({
        "checkout": {
            "cart_line_item_id": cart_line_item_id,
        }
    })
}

fn checkout_order_adjustments(
    cart: &rustok_cart::dto::CartResponse,
) -> Vec<CreateOrderAdjustmentInput> {
    cart.adjustments
        .iter()
        .map(|adjustment| CreateOrderAdjustmentInput {
            line_item_index: adjustment.line_item_id.and_then(|line_item_id| {
                cart.line_items
                    .iter()
                    .position(|item| item.id == line_item_id)
            }),
            source_type: adjustment.source_type.clone(),
            source_id: adjustment.source_id.clone(),
            amount: adjustment.amount,
            metadata: adjustment.metadata.clone(),
        })
        .collect()
}

fn checkout_order_tax_lines(cart: &rustok_cart::dto::CartResponse) -> Vec<CreateOrderTaxLineInput> {
    cart.tax_lines
        .iter()
        .map(|line| CreateOrderTaxLineInput {
            line_item_index: line.line_item_id.and_then(|line_item_id| {
                cart.line_items
                    .iter()
                    .position(|item| item.id == line_item_id)
            }),
            shipping_option_id: line.shipping_option_id,
            description: line.description.clone(),
            provider_id: line.provider_id.clone(),
            rate: line.rate,
            amount: line.amount,
            currency_code: line.currency_code.clone(),
            metadata: line.metadata.clone(),
        })
        .collect()
}

fn cart_line_item_id_from_order_line_item(
    item: &crate::dto::OrderLineItemResponse,
) -> Option<Uuid> {
    item.metadata
        .get("checkout")
        .and_then(|checkout| checkout.get("cart_line_item_id"))
        .and_then(serde_json::Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
}

fn fulfillment_items_for_delivery_group(
    order: &crate::dto::OrderResponse,
    delivery_group: &rustok_cart::dto::CartDeliveryGroupResponse,
) -> CheckoutResult<Vec<crate::dto::CreateFulfillmentItemInput>> {
    let mut items = Vec::with_capacity(delivery_group.line_item_ids.len());

    for cart_line_item_id in &delivery_group.line_item_ids {
        let order_line_item = order
            .line_items
            .iter()
            .find(|item| cart_line_item_id_from_order_line_item(item) == Some(*cart_line_item_id))
            .ok_or_else(|| {
                CheckoutError::Validation(format!(
                    "order line item for cart line item {cart_line_item_id} is missing from delivery group projection"
                ))
            })?;

        items.push(crate::dto::CreateFulfillmentItemInput {
            order_line_item_id: order_line_item.id,
            quantity: order_line_item.quantity,
            metadata: serde_json::json!({
                "source_cart_line_item_id": cart_line_item_id,
                "shipping_profile_slug": delivery_group.shipping_profile_slug,
                "seller_id": delivery_group.seller_id,
            }),
        });
    }

    Ok(items)
}

fn fulfillment_shim(
    fulfillments: &[rustok_fulfillment::dto::FulfillmentResponse],
) -> Option<rustok_fulfillment::dto::FulfillmentResponse> {
    if fulfillments.len() == 1 {
        fulfillments.first().cloned()
    } else {
        None
    }
}

impl From<CartError> for CheckoutError {
    fn from(source: CartError) -> Self {
        stage_error("cart")(source)
    }
}

impl From<OrderError> for CheckoutError {
    fn from(source: OrderError) -> Self {
        stage_error("order")(source)
    }
}

impl From<PaymentError> for CheckoutError {
    fn from(source: PaymentError) -> Self {
        stage_error("payment")(source)
    }
}

impl From<FulfillmentError> for CheckoutError {
    fn from(source: FulfillmentError) -> Self {
        stage_error("fulfillment")(source)
    }
}
