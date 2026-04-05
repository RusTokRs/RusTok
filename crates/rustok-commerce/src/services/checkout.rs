use thiserror::Error;
use tracing::instrument;
use uuid::Uuid;
use validator::Validate;

use rustok_cart::error::CartError;
use rustok_fulfillment::error::FulfillmentError;
use rustok_order::error::OrderError;
use rustok_outbox::TransactionalEventBus;
use rustok_payment::error::PaymentError;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::collections::BTreeSet;

use crate::dto::{
    AuthorizePaymentInput, CancelPaymentInput, CompleteCheckoutInput, CompleteCheckoutResponse,
    CreateFulfillmentInput, CreateOrderInput, CreateOrderLineItemInput,
    CreatePaymentCollectionInput, ResolveStoreContextInput,
};
use crate::entities::{product, product_variant};
use crate::storefront_channel::{
    is_metadata_visible_for_public_channel, load_available_inventory_for_variant_in_public_channel,
    normalize_public_channel_slug,
};
use crate::storefront_shipping::{
    is_shipping_option_compatible_with_profiles, load_current_shipping_profile_slug_for_line_item,
};
use crate::{
    CartService, FulfillmentService, OrderService, PaymentService, StoreContextService,
    UpdateCartContextInput,
};

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
    cart_service: CartService,
    order_service: OrderService,
    payment_service: PaymentService,
    fulfillment_service: FulfillmentService,
    context_service: StoreContextService,
}

impl CheckoutService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            db: db.clone(),
            cart_service: CartService::new(db.clone()),
            order_service: OrderService::new(db.clone(), event_bus),
            payment_service: PaymentService::new(db.clone()),
            fulfillment_service: FulfillmentService::new(db.clone()),
            context_service: StoreContextService::new(db),
        }
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
            .cart_service
            .get_cart(tenant_id, input.cart_id)
            .await
            .map_err(stage_error("load_cart"))?;
        if input.shipping_selections.is_some() || input.shipping_option_id.is_some() {
            cart = self
                .cart_service
                .update_context(
                    tenant_id,
                    cart.id,
                    UpdateCartContextInput {
                        email: cart.email.clone(),
                        region_id: cart.region_id,
                        country_code: cart.country_code.clone(),
                        locale_code: cart.locale_code.clone(),
                        selected_shipping_option_id: input.shipping_option_id,
                        shipping_selections: input.shipping_selections.clone(),
                    },
                )
                .await
                .map_err(stage_error("update_cart_shipping"))?;
        }
        if cart.status == "completed" {
            if let Some(response) = self
                .recover_existing_checkout(tenant_id, cart.clone())
                .await?
            {
                return Ok(response);
            }
            return Err(CheckoutError::CartNotReady(cart.id));
        }
        if cart.status == "checking_out" {
            if let Some(response) = self
                .recover_existing_checkout(tenant_id, cart.clone())
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
            .cart_service
            .begin_checkout(tenant_id, cart.id)
            .await
            .map_err(stage_error("begin_checkout"))?;
        if let Err(error) = self.validate_cart_inventory(tenant_id, &cart).await {
            let _ = self.cart_service.release_checkout(tenant_id, cart.id).await;
            return Err(error);
        }
        if let Err(error) = self.validate_delivery_groups(tenant_id, &cart).await {
            let _ = self.cart_service.release_checkout(tenant_id, cart.id).await;
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
                let _ = self.cart_service.release_checkout(tenant_id, cart.id).await;
                return Err(stage_error("resolve_context")(error));
            }
        };
        let checkout_result: CheckoutResult<CompleteCheckoutResponse> = async {
            let mut order = self
                .order_service
                .create_order_with_channel(
                    tenant_id,
                    actor_id,
                    CreateOrderInput {
                        customer_id: cart.customer_id,
                        currency_code: cart.currency_code.clone(),
                        line_items: cart
                            .line_items
                            .iter()
                            .map(|item| CreateOrderLineItemInput {
                                product_id: item.product_id,
                                variant_id: item.variant_id,
                                shipping_profile_slug: item.shipping_profile_slug.clone(),
                                sku: item.sku.clone(),
                                title: item.title.clone(),
                                quantity: item.quantity,
                                unit_price: item.unit_price,
                                metadata: item.metadata.clone(),
                            })
                            .collect(),
                        metadata: input.metadata.clone(),
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
                    .get_order(tenant_id, order.id)
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
                "pending" => match self
                    .payment_service
                    .authorize_collection(
                        tenant_id,
                        payment_collection.id,
                        AuthorizePaymentInput {
                            provider_id: None,
                            provider_payment_id: None,
                            amount: Some(cart.total_amount),
                            metadata: input.metadata.clone(),
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
                },
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
                        order.id,
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
                "authorized" => match self
                    .payment_service
                    .capture_collection(
                        tenant_id,
                        authorized_payment.id,
                        rustok_payment::dto::CapturePaymentInput {
                            amount: Some(cart.total_amount),
                            metadata: input.metadata.clone(),
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
                },
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
                .cart_service
                .complete_cart(tenant_id, cart.id)
                .await
                .map_err(stage_error("complete_cart"))?;

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
            let _ = self.cart_service.release_checkout(tenant_id, cart.id).await;
        }

        checkout_result
    }

    async fn validate_cart_inventory(
        &self,
        tenant_id: Uuid,
        cart: &rustok_cart::dto::CartResponse,
    ) -> CheckoutResult<()> {
        let public_channel_slug = normalize_public_channel_slug(cart.channel_slug.as_deref());

        for line_item in &cart.line_items {
            let Some(variant_id) = line_item.variant_id else {
                continue;
            };

            let Some(variant) = product_variant::Entity::find_by_id(variant_id)
                .filter(product_variant::Column::TenantId.eq(tenant_id))
                .one(&self.db)
                .await
                .map_err(stage_error("load_variant"))?
            else {
                return Err(CheckoutError::Validation(format!(
                    "Variant {} is no longer available for checkout",
                    variant_id
                )));
            };

            let product_id = line_item.product_id.unwrap_or(variant.product_id);
            let Some(product) = product::Entity::find_by_id(product_id)
                .filter(product::Column::TenantId.eq(tenant_id))
                .one(&self.db)
                .await
                .map_err(stage_error("load_product"))?
            else {
                return Err(CheckoutError::Validation(format!(
                    "Product {} is no longer available for checkout",
                    product_id
                )));
            };
            if product.status != product::ProductStatus::Active
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
            let current_shipping_profile_slug = load_current_shipping_profile_slug_for_line_item(
                &self.db,
                tenant_id,
                Some(product.id),
                Some(variant.id),
            )
            .await
            .map_err(stage_error("load_shipping_profile"))?;
            if current_shipping_profile_slug != line_item.shipping_profile_slug {
                return Err(CheckoutError::Validation(format!(
                    "Line item {} uses stale shipping profile snapshot {} (current: {})",
                    line_item.id, line_item.shipping_profile_slug, current_shipping_profile_slug
                )));
            }

            if variant.inventory_policy == "continue" {
                continue;
            }

            let available_inventory = load_available_inventory_for_variant_in_public_channel(
                &self.db,
                tenant_id,
                variant.id,
                public_channel_slug.as_deref(),
            )
            .await
            .map_err(stage_error("load_inventory"))?;
            if available_inventory < line_item.quantity {
                return Err(CheckoutError::Validation(format!(
                    "Variant {} does not have enough available inventory for the cart channel",
                    variant.id
                )));
            }
        }

        Ok(())
    }

    async fn recover_existing_checkout(
        &self,
        tenant_id: Uuid,
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

        let order = self
            .order_service
            .get_order(tenant_id, order_id)
            .await
            .map_err(stage_error("load_order"))?;
        let is_completed_checkout =
            payment_collection.status == "captured" && order.status == "paid";
        if !is_completed_checkout {
            return Ok(None);
        }

        let cart = if cart.status == "checking_out" {
            self.cart_service
                .complete_cart(tenant_id, cart.id)
                .await
                .map_err(stage_error("finalize_recovered_cart"))?
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
                .get_shipping_option(tenant_id, selected_shipping_option_id)
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
        order_id: Uuid,
        customer_id: Option<Uuid>,
        cart: &rustok_cart::dto::CartResponse,
        metadata: serde_json::Value,
    ) -> CheckoutResult<Vec<rustok_fulfillment::dto::FulfillmentResponse>> {
        let mut fulfillments = Vec::with_capacity(cart.delivery_groups.len());

        for delivery_group in &cart.delivery_groups {
            let selected_shipping_option_id = delivery_group.selected_shipping_option_id;
            let group_metadata = merge_checkout_metadata(
                metadata.clone(),
                serde_json::json!({
                    "delivery_group": {
                        "shipping_profile_slug": delivery_group.shipping_profile_slug,
                        "line_item_ids": delivery_group.line_item_ids,
                    }
                }),
            );
            let fulfillment = self
                .fulfillment_service
                .create_fulfillment(
                    tenant_id,
                    CreateFulfillmentInput {
                        order_id,
                        shipping_option_id: selected_shipping_option_id,
                        customer_id,
                        carrier: None,
                        tracking_number: None,
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
                    metadata: serde_json::json!({ "compensated": true, "reason": reason }),
                },
            )
            .await;
        let _ = self
            .order_service
            .cancel_order(tenant_id, actor_id, order_id, Some(reason.to_string()))
            .await;
    }
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
