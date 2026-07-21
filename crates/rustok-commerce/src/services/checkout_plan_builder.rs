use rustok_api::{PLATFORM_FALLBACK_LOCALE, PortActor, PortContext, normalize_locale_tag};
use rustok_cart::{
    CartMarketplaceLineSnapshot, CartResponse, ListMarketplaceCartLineSnapshotsRequest,
    MarketplaceCartSnapshotReadPort, PreparedCartCheckoutSnapshot,
    in_process_marketplace_cart_snapshot_read_port,
};
use rustok_commerce_foundation::entities::product::ProductStatus;
use rustok_fulfillment::FulfillmentService;
use rustok_inventory::{InventoryAvailabilityRequest, InventoryReservationPort};
use rustok_product::{
    ProductCatalogReadPort, ProductProjectionRequest, VariantProductProjectionRequest,
};
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};
use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
    time::Duration,
};
use uuid::Uuid;
use validator::Validate;

use crate::dto::{
    CompleteCheckoutInput, CreateOrderAdjustmentInput, CreateOrderInput, CreateOrderLineItemInput,
    CreateOrderTaxLineInput, ResolveStoreContextInput,
};
use crate::storefront_channel::{
    is_metadata_visible_for_public_channel, normalize_public_channel_slug,
};
use crate::storefront_shipping::{
    effective_shipping_profile_slug, is_shipping_option_compatible_with_profiles,
};

use super::{
    CheckoutError, CheckoutFulfillmentPlan, CheckoutFulfillmentPlanItem,
    CheckoutMarketplaceLineSnapshot, CheckoutOrderPlanPayload, CheckoutResult, StoreContextService,
};

pub struct CheckoutPlanBuilder {
    db: DatabaseConnection,
    inventory_availability_port: Arc<dyn InventoryReservationPort>,
    product_catalog_read_port: Arc<dyn ProductCatalogReadPort>,
    marketplace_snapshot_read_port: Arc<dyn MarketplaceCartSnapshotReadPort>,
    fulfillment_service: FulfillmentService,
    context_service: StoreContextService,
}

impl CheckoutPlanBuilder {
    pub fn new(
        db: DatabaseConnection,
        region_read_port: Arc<dyn rustok_region::RegionReadPort>,
        inventory_availability_port: Arc<dyn InventoryReservationPort>,
        product_catalog_read_port: Arc<dyn ProductCatalogReadPort>,
    ) -> Self {
        Self {
            db: db.clone(),
            inventory_availability_port,
            product_catalog_read_port,
            marketplace_snapshot_read_port: in_process_marketplace_cart_snapshot_read_port(
                db.clone(),
            ),
            fulfillment_service: FulfillmentService::new(db.clone()),
            context_service: StoreContextService::new(db, region_read_port),
        }
    }

    pub fn with_marketplace_snapshot_read_port(
        mut self,
        marketplace_snapshot_read_port: Arc<dyn MarketplaceCartSnapshotReadPort>,
    ) -> Self {
        self.marketplace_snapshot_read_port = marketplace_snapshot_read_port;
        self
    }

    pub async fn build(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        operation_id: Uuid,
        input: &CompleteCheckoutInput,
        snapshot: &PreparedCartCheckoutSnapshot,
    ) -> CheckoutResult<CheckoutOrderPlanPayload> {
        input
            .validate()
            .map_err(|error| CheckoutError::Validation(error.to_string()))?;
        let cart = &snapshot.cart;
        if cart.id != input.cart_id || cart.tenant_id != tenant_id {
            return Err(CheckoutError::Validation(
                "prepared cart snapshot does not match the checkout request".to_string(),
            ));
        }
        if cart.status != "checking_out" {
            return Err(CheckoutError::CartNotReady(cart.id));
        }
        if cart.line_items.is_empty() {
            return Err(CheckoutError::EmptyCart(cart.id));
        }

        let marketplace_snapshots = self
            .marketplace_snapshot_read_port
            .list_marketplace_line_snapshots(
                port_context(
                    tenant_id,
                    actor_id,
                    cart,
                    normalize_public_channel_slug(cart.channel_slug.as_deref()).as_deref(),
                    "marketplace-snapshot",
                ),
                ListMarketplaceCartLineSnapshotsRequest { cart_id: cart.id },
            )
            .await
            .map_err(|error| boundary_error("read_marketplace_cart_snapshots", error))?;
        let marketplace_lines = build_marketplace_plan_lines(cart, marketplace_snapshots)?;
        let marketplace_sellers = marketplace_lines
            .iter()
            .map(|line| (line.order_line_index, line.snapshot.seller_id.to_string()))
            .collect::<HashMap<_, _>>();

        self.validate_cart_inventory(tenant_id, actor_id, cart)
            .await?;
        let context = self
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
            .map_err(stage_error("resolve_context"))?;
        self.validate_delivery_groups(
            tenant_id,
            cart,
            context.locale.as_str(),
            Some(context.default_locale.as_str()),
        )
        .await?;

        let checkout_metadata = merge_metadata(
            input.metadata.clone(),
            json!({
                "checkout": {
                    "operation_id": operation_id,
                    "snapshot_hash": snapshot.snapshot_hash.clone(),
                }
            }),
        );
        let order_metadata = merge_metadata(
            checkout_metadata.clone(),
            json!({
                "cart_context": {
                    "region_id": cart.region_id,
                    "country_code": cart.country_code.clone(),
                    "locale": context.locale.clone(),
                    "currency_code": context.currency_code.clone(),
                    "selected_shipping_option_id": cart.selected_shipping_option_id,
                    "email": cart.email.clone(),
                }
            }),
        );
        let fulfillment_plans = if input.create_fulfillment {
            build_fulfillment_plans(cart, input.metadata.clone())?
        } else {
            Vec::new()
        };

        Ok(CheckoutOrderPlanPayload {
            order_input: CreateOrderInput {
                customer_id: cart.customer_id,
                currency_code: cart.currency_code.clone(),
                shipping_total: cart.shipping_total,
                line_items: cart
                    .line_items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| CreateOrderLineItemInput {
                        product_id: item.product_id,
                        variant_id: item.variant_id,
                        shipping_profile_slug: item.shipping_profile_slug.clone(),
                        seller_id: marketplace_sellers
                            .get(&index)
                            .cloned()
                            .or_else(|| item.seller_id.clone()),
                        sku: item.sku.clone(),
                        title: item.title.clone(),
                        quantity: item.quantity,
                        unit_price: item.unit_price,
                        metadata: merge_metadata(
                            strip_marketplace_identity(item.metadata.clone()),
                            json!({
                                "checkout": {
                                    "cart_line_item_id": item.id,
                                }
                            }),
                        ),
                    })
                    .collect(),
                adjustments: order_adjustments(cart),
                tax_lines: order_tax_lines(cart),
                metadata: order_metadata,
            },
            channel_id: cart.channel_id,
            channel_slug: cart.channel_slug.clone(),
            context,
            create_fulfillment: input.create_fulfillment,
            fulfillment_plans,
            marketplace_lines,
            checkout_metadata,
        })
    }

    async fn validate_cart_inventory(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        cart: &CartResponse,
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
                            product_context(
                                tenant_id,
                                actor_id,
                                cart,
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
                            product_context(
                                tenant_id,
                                actor_id,
                                cart,
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
            .map_err(|error| boundary_error("read_checkout_product_projection", error))?;
            let variant = product
                .variants
                .iter()
                .find(|variant| variant.id == variant_id)
                .ok_or_else(|| {
                    CheckoutError::Validation(format!(
                        "Variant {variant_id} is no longer available for checkout"
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
                .inventory_availability_port
                .check_availability(
                    inventory_context(tenant_id, actor_id, cart, public_channel_slug.as_deref()),
                    InventoryAvailabilityRequest {
                        variant_id,
                        requested_quantity: line_item.quantity,
                        channel_slug: public_channel_slug.clone(),
                    },
                )
                .await
                .map_err(|error| boundary_error("check_inventory_availability", error))?;
            if !availability.available {
                return Err(CheckoutError::Validation(format!(
                    "Variant {variant_id} does not have enough available inventory for the cart channel"
                )));
            }
        }
        Ok(())
    }

    async fn validate_delivery_groups(
        &self,
        tenant_id: Uuid,
        cart: &CartResponse,
        requested_locale: &str,
        tenant_default_locale: Option<&str>,
    ) -> CheckoutResult<()> {
        let public_channel_slug = normalize_public_channel_slug(cart.channel_slug.as_deref());
        for delivery_group in &cart.delivery_groups {
            let selected_shipping_option_id =
                delivery_group.selected_shipping_option_id.ok_or_else(|| {
                    CheckoutError::Validation(format!(
                        "Delivery group {} does not have a selected shipping option",
                        delivery_group.shipping_profile_slug
                    ))
                })?;
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
            let required_profiles = BTreeSet::from([delivery_group.shipping_profile_slug.clone()]);
            if !is_shipping_option_compatible_with_profiles(&option, &required_profiles) {
                return Err(CheckoutError::Validation(format!(
                    "Shipping option {} is not compatible with delivery group {}",
                    option.id, delivery_group.shipping_profile_slug
                )));
            }
        }
        Ok(())
    }

    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }
}

fn build_marketplace_plan_lines(
    cart: &CartResponse,
    snapshots: Vec<CartMarketplaceLineSnapshot>,
) -> CheckoutResult<Vec<CheckoutMarketplaceLineSnapshot>> {
    let mut snapshots = snapshots
        .into_iter()
        .map(|snapshot| (snapshot.cart_line_item_id, snapshot))
        .collect::<HashMap<_, _>>();
    let mut result = Vec::new();

    for (order_line_index, line) in cart.line_items.iter().enumerate() {
        let snapshot = snapshots.remove(&line.id);
        let has_legacy_marketplace_identity = line.seller_id.is_some()
            || line.metadata.get("marketplace").is_some()
            || line.metadata.get("seller_id").is_some()
            || line.metadata.get("seller").is_some();
        let Some(snapshot) = snapshot else {
            if has_legacy_marketplace_identity {
                return Err(CheckoutError::Validation(format!(
                    "Cart line {} has marketplace identity but no typed marketplace snapshot",
                    line.id
                )));
            }
            continue;
        };
        let expected_subtotal = i64::from(line.quantity)
            .checked_mul(snapshot.unit_amount)
            .ok_or_else(|| {
                CheckoutError::Validation(format!(
                    "Typed marketplace snapshot subtotal overflow for cart line {}",
                    line.id
                ))
            })?;
        if line.product_id != Some(snapshot.master_product_id)
            || line.variant_id != Some(snapshot.master_variant_id)
            || expected_subtotal != snapshot.subtotal_amount
        {
            return Err(CheckoutError::Validation(format!(
                "Cart line {} no longer matches its typed marketplace snapshot",
                line.id
            )));
        }
        result.push(CheckoutMarketplaceLineSnapshot {
            order_line_index,
            snapshot,
        });
    }

    if let Some(orphan) = snapshots.values().next() {
        return Err(CheckoutError::Validation(format!(
            "Typed marketplace snapshot references missing cart line {}",
            orphan.cart_line_item_id
        )));
    }
    Ok(result)
}

fn strip_marketplace_identity(metadata: Value) -> Value {
    match metadata {
        Value::Object(mut metadata) => {
            metadata.remove("marketplace");
            metadata.remove("seller");
            metadata.remove("seller_id");
            Value::Object(metadata)
        }
        metadata => metadata,
    }
}

fn build_fulfillment_plans(
    cart: &CartResponse,
    metadata: Value,
) -> CheckoutResult<Vec<CheckoutFulfillmentPlan>> {
    cart.delivery_groups
        .iter()
        .map(|group| {
            let items = group
                .line_item_ids
                .iter()
                .map(|cart_line_item_id| {
                    let line = cart
                        .line_items
                        .iter()
                        .find(|line| line.id == *cart_line_item_id)
                        .ok_or_else(|| {
                            CheckoutError::Validation(format!(
                                "delivery group references missing cart line {cart_line_item_id}"
                            ))
                        })?;
                    Ok(CheckoutFulfillmentPlanItem {
                        cart_line_item_id: line.id,
                        quantity: line.quantity,
                        metadata: json!({
                            "source_cart_line_item_id": line.id,
                            "shipping_profile_slug": group.shipping_profile_slug.clone(),
                            "seller_id": group.seller_id.clone(),
                        }),
                    })
                })
                .collect::<CheckoutResult<Vec<_>>>()?;
            Ok(CheckoutFulfillmentPlan {
                shipping_option_id: group.selected_shipping_option_id,
                carrier: None,
                tracking_number: None,
                items,
                metadata: merge_metadata(
                    metadata.clone(),
                    json!({
                        "delivery_group": {
                            "shipping_profile_slug": group.shipping_profile_slug.clone(),
                            "seller_id": group.seller_id.clone(),
                            "line_item_ids": group.line_item_ids.clone(),
                        }
                    }),
                ),
            })
        })
        .collect()
}

fn order_adjustments(cart: &CartResponse) -> Vec<CreateOrderAdjustmentInput> {
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

fn order_tax_lines(cart: &CartResponse) -> Vec<CreateOrderTaxLineInput> {
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

fn merge_metadata(base: Value, patch: Value) -> Value {
    match (base, patch) {
        (Value::Object(mut base), Value::Object(patch)) => {
            for (key, value) in patch {
                base.insert(key, value);
            }
            Value::Object(base)
        }
        (_, patch) => patch,
    }
}

fn product_context(
    tenant_id: Uuid,
    actor_id: Uuid,
    cart: &CartResponse,
    channel_slug: Option<&str>,
) -> PortContext {
    port_context(tenant_id, actor_id, cart, channel_slug, "product")
}

fn inventory_context(
    tenant_id: Uuid,
    actor_id: Uuid,
    cart: &CartResponse,
    channel_slug: Option<&str>,
) -> PortContext {
    port_context(tenant_id, actor_id, cart, channel_slug, "inventory")
}

fn port_context(
    tenant_id: Uuid,
    actor_id: Uuid,
    cart: &CartResponse,
    channel_slug: Option<&str>,
    boundary: &str,
) -> PortContext {
    let locale = cart
        .locale_code
        .as_deref()
        .and_then(normalize_locale_tag)
        .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        locale,
        format!("checkout:{}:{boundary}", cart.id),
    )
    .with_deadline(Duration::from_secs(2));
    match channel_slug {
        Some(channel_slug) => context.with_channel(channel_slug),
        None => context,
    }
}

fn boundary_error(stage: &'static str, error: rustok_api::PortError) -> CheckoutError {
    CheckoutError::BoundaryFailure {
        stage,
        kind: error.kind,
        code: error.code,
        message: error.message,
        retryable: error.retryable,
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