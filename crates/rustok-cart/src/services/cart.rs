mod checkout;
pub mod helpers;
mod promotions;
pub mod types;

pub use types::{
    CartLineItemPricingUpdate, CartPricingAdjustmentUpdate, CartPromotionKind,
    CartPromotionPreview, DeliveryGroupKey, DeliveryGroupSnapshot,
};

use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};
use tracing::instrument;
use uuid::Uuid;
use validator::Validate;

use rustok_core::generate_id;
use rustok_tax::{TaxCalculationPort, in_process_tax_calculation_port};

use crate::dto::{
    AddCartLineItemInput, CartResponse, CreateCartInput, SetCartAdjustmentInput,
    UpdateCartContextInput,
};
use crate::entities;
use crate::error::{CartError, CartResult};

use helpers::*;

#[derive(Clone)]
pub struct CartService {
    db: DatabaseConnection,
    tax_calculation_port: Arc<dyn TaxCalculationPort>,
}

impl CartService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            tax_calculation_port: in_process_tax_calculation_port(),
        }
    }

    /// Overrides the owner-managed tax provider with an explicitly composed port.
    pub fn with_tax_calculation_port(
        mut self,
        tax_calculation_port: Arc<dyn TaxCalculationPort>,
    ) -> Self {
        self.tax_calculation_port = tax_calculation_port;
        self
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id))]
    pub async fn create_cart(
        &self,
        tenant_id: Uuid,
        input: CreateCartInput,
    ) -> CartResult<CartResponse> {
        self.create_cart_with_channel(tenant_id, input, None, None)
            .await
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, channel_id = ?channel_id, channel_slug = ?channel_slug))]
    pub async fn create_cart_with_channel(
        &self,
        tenant_id: Uuid,
        input: CreateCartInput,
        channel_id: Option<Uuid>,
        channel_slug: Option<String>,
    ) -> CartResult<CartResponse> {
        input
            .validate()
            .map_err(|error| CartError::Validation(error.to_string()))?;

        let currency_code = input.currency_code.trim().to_ascii_uppercase();
        if currency_code.len() != 3 {
            return Err(CartError::Validation(
                "currency_code must be a 3-letter code".to_string(),
            ));
        }
        let country_code = input
            .country_code
            .as_deref()
            .map(normalize_country_code)
            .transpose()?;
        let locale_code = input
            .locale_code
            .as_deref()
            .map(normalize_locale_code)
            .transpose()?;
        let channel_slug = channel_slug
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let cart_id = generate_id();
        let now = Utc::now();

        entities::cart::ActiveModel {
            id: Set(cart_id),
            tenant_id: Set(tenant_id),
            channel_id: Set(channel_id),
            channel_slug: Set(channel_slug),
            customer_id: Set(input.customer_id),
            email: Set(input.email),
            region_id: Set(input.region_id),
            country_code: Set(country_code),
            locale_code: Set(locale_code),
            selected_shipping_option_id: Set(input.selected_shipping_option_id),
            status: Set(STATUS_ACTIVE.to_string()),
            currency_code: Set(currency_code),
            shipping_total: Set(Decimal::ZERO),
            total_amount: Set(Decimal::ZERO),
            tax_total: Set(Decimal::ZERO),
            metadata: Set(input.metadata),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
            completed_at: Set(None),
        }
        .insert(&self.db)
        .await?;

        self.get_cart(tenant_id, cart_id).await
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id, cart_id = %cart_id))]
    pub async fn get_cart(&self, tenant_id: Uuid, cart_id: Uuid) -> CartResult<CartResponse> {
        let cart = load_cart(&self.db, tenant_id, cart_id).await?;
        build_response(&self.db, cart).await
    }

    pub async fn add_line_item(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        input: AddCartLineItemInput,
    ) -> CartResult<CartResponse> {
        self.add_line_item_with_pricing_adjustment(tenant_id, cart_id, input, None)
            .await
    }

    pub async fn add_line_item_with_pricing_adjustment(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        input: AddCartLineItemInput,
        pricing_adjustment: Option<CartPricingAdjustmentUpdate>,
    ) -> CartResult<CartResponse> {
        input
            .validate()
            .map_err(|error| CartError::Validation(error.to_string()))?;
        if input.unit_price < Decimal::ZERO {
            return Err(CartError::Validation(
                "unit_price cannot be negative".to_string(),
            ));
        }

        let txn = self.db.begin().await?;
        let cart = load_cart_in_tx(&txn, tenant_id, cart_id).await?;
        ensure_active(&cart.status, "add_line_item")?;
        let now = Utc::now();
        let metadata = sanitize_line_item_metadata(input.metadata);
        let locale = match cart
            .locale_code
            .as_deref()
            .and_then(rustok_api::normalize_locale_tag)
        {
            Some(locale) => locale,
            None => load_tenant_default_locale(&txn, tenant_id).await?,
        };
        let line_item_id = generate_id();

        entities::cart_line_item::ActiveModel {
            id: Set(line_item_id),
            cart_id: Set(cart_id),
            product_id: Set(input.product_id),
            variant_id: Set(input.variant_id),
            shipping_profile_slug: Set(normalize_shipping_profile_slug(
                input.shipping_profile_slug.as_deref(),
            )),
            sku: Set(input.sku),
            quantity: Set(input.quantity),
            unit_price: Set(input.unit_price),
            total_price: Set(input.unit_price * Decimal::from(input.quantity)),
            currency_code: Set(cart.currency_code.clone()),
            metadata: Set(metadata.clone()),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;
        entities::cart_line_item_translation::ActiveModel {
            id: Set(generate_id()),
            cart_line_item_id: Set(line_item_id),
            locale: Set(locale),
            title: Set(input.title),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&txn)
        .await?;

        if let (Some(product_id), Some(variant_id)) = (input.product_id, input.variant_id) {
            let seller_id = metadata
                .get("seller_id")
                .and_then(|v| v.as_str())
                .or_else(|| {
                    metadata
                        .get("seller")
                        .and_then(|v| v.get("id"))
                        .and_then(|v| v.as_str())
                })
                .and_then(|s| Uuid::parse_str(s).ok());
            if let Some(seller_id) = seller_id {
                use rust_decimal::prelude::ToPrimitive;
                let unit_amount = (input.unit_price * Decimal::from(100))
                    .to_i64()
                    .unwrap_or_default();
                let subtotal_amount = unit_amount * i64::from(input.quantity);
                entities::cart_line_item_marketplace_snapshot::ActiveModel {
                    cart_line_item_id: Set(line_item_id),
                    seller_id: Set(seller_id),
                    listing_id: Set(variant_id),
                    master_product_id: Set(product_id),
                    master_variant_id: Set(variant_id),
                    listing_terms_version: Set(1),
                    currency_code: Set(cart.currency_code.clone().to_uppercase()),
                    currency_exponent: Set(2),
                    unit_amount: Set(unit_amount),
                    subtotal_amount: Set(subtotal_amount),
                    discount_amount: Set(0),
                    tax_amount: Set(0),
                    total_amount: Set(subtotal_amount),
                    pricing_reference: Set(None),
                    inventory_reference: Set(None),
                    fulfillment_profile_slug: Set(Some(normalize_shipping_profile_slug(
                        input.shipping_profile_slug.as_deref(),
                    ))),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(&txn)
                .await?;
            }
        }
        replace_pricing_adjustments(
            &txn,
            cart_id,
            cart.currency_code.as_str(),
            vec![(line_item_id, pricing_adjustment)],
        )
        .await?;

        recalculate_totals(&txn, self.tax_calculation_port.as_ref(), cart).await?;
        reconcile_cart_shipping_state(&txn, cart_id).await?;
        txn.commit().await?;
        self.get_cart(tenant_id, cart_id).await
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, cart_id = %cart_id))]
    pub async fn update_context(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        input: UpdateCartContextInput,
    ) -> CartResult<CartResponse> {
        input
            .validate()
            .map_err(|error| CartError::Validation(error.to_string()))?;

        let txn = self.db.begin().await?;
        let cart = load_cart_in_tx(&txn, tenant_id, cart_id).await?;
        if cart.status != STATUS_ACTIVE && cart.status != STATUS_CHECKING_OUT {
            return Err(CartError::InvalidTransition {
                from: cart.status,
                to: "update_context".to_string(),
            });
        }
        let shipping_patch_input = input.clone();

        let country_code = input
            .country_code
            .as_deref()
            .map(normalize_country_code)
            .transpose()?;
        let locale_code = input
            .locale_code
            .as_deref()
            .map(normalize_locale_code)
            .transpose()?;

        let mut active: entities::cart::ActiveModel = cart.clone().into();
        active.email = Set(input.email);
        active.region_id = Set(input.region_id);
        active.country_code = Set(country_code);
        active.locale_code = Set(locale_code);
        active.selected_shipping_option_id = Set(input.selected_shipping_option_id);
        active.updated_at = Set(Utc::now().into());
        active.update(&txn).await?;
        apply_shipping_selection_patch(&txn, &cart, &shipping_patch_input).await?;

        txn.commit().await?;
        self.get_cart(tenant_id, cart_id).await
    }

    pub async fn set_adjustments(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        adjustments: Vec<SetCartAdjustmentInput>,
    ) -> CartResult<CartResponse> {
        for adjustment in &adjustments {
            adjustment
                .validate()
                .map_err(|error| CartError::Validation(error.to_string()))?;
            if adjustment.amount <= Decimal::ZERO {
                return Err(CartError::Validation(
                    "adjustment amount must be greater than zero".to_string(),
                ));
            }
        }

        let txn = self.db.begin().await?;
        let cart = load_cart_in_tx(&txn, tenant_id, cart_id).await?;
        ensure_active(&cart.status, "set_adjustments")?;

        let line_items = entities::cart_line_item::Entity::find()
            .filter(entities::cart_line_item::Column::CartId.eq(cart_id))
            .all(&txn)
            .await?;
        let line_item_ids = line_items
            .iter()
            .map(|item| item.id)
            .collect::<BTreeSet<_>>();
        let subtotal_amount = subtotal_amount(&line_items);
        let adjustment_total = adjustments
            .iter()
            .fold(Decimal::ZERO, |acc, adjustment| acc + adjustment.amount);
        if adjustment_total > subtotal_amount {
            return Err(CartError::Validation(
                "adjustment total cannot exceed cart subtotal".to_string(),
            ));
        }

        for adjustment in &adjustments {
            if let Some(line_item_id) = adjustment.line_item_id {
                if !line_item_ids.contains(&line_item_id) {
                    return Err(CartError::Validation(format!(
                        "cart line item {line_item_id} does not belong to cart {cart_id}"
                    )));
                }
            }
        }

        entities::cart_adjustment::Entity::delete_many()
            .filter(entities::cart_adjustment::Column::CartId.eq(cart_id))
            .exec(&txn)
            .await?;

        let now = Utc::now();
        for adjustment in adjustments {
            entities::cart_adjustment::ActiveModel {
                id: Set(generate_id()),
                cart_id: Set(cart_id),
                cart_line_item_id: Set(adjustment.line_item_id),
                source_type: Set(normalize_adjustment_source_type(&adjustment.source_type)?),
                source_id: Set(normalize_adjustment_source_id(
                    adjustment.source_id.as_deref(),
                )),
                amount: Set(adjustment.amount),
                currency_code: Set(cart.currency_code.clone()),
                metadata: Set(sanitize_adjustment_metadata(adjustment.metadata)),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(&txn)
            .await?;
        }

        recalculate_totals(&txn, self.tax_calculation_port.as_ref(), cart).await?;
        txn.commit().await?;
        self.get_cart(tenant_id, cart_id).await
    }

    pub async fn update_line_item_quantity(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        line_item_id: Uuid,
        quantity: i32,
    ) -> CartResult<CartResponse> {
        if quantity < 1 {
            return Err(CartError::Validation(
                "quantity must be at least 1".to_string(),
            ));
        }

        let txn = self.db.begin().await?;
        let cart = load_cart_in_tx(&txn, tenant_id, cart_id).await?;
        ensure_active(&cart.status, "update_line_item_quantity")?;

        let line_item = entities::cart_line_item::Entity::find_by_id(line_item_id)
            .filter(entities::cart_line_item::Column::CartId.eq(cart_id))
            .one(&txn)
            .await?
            .ok_or(CartError::CartLineItemNotFound(line_item_id))?;

        let mut active: entities::cart_line_item::ActiveModel = line_item.into();
        let now = Utc::now();
        let unit_price = active.unit_price.clone().take().unwrap_or(Decimal::ZERO);
        active.quantity = Set(quantity);
        active.total_price = Set(unit_price * Decimal::from(quantity));
        active.updated_at = Set(now.into());
        active.update(&txn).await?;

        recalculate_totals(&txn, self.tax_calculation_port.as_ref(), cart).await?;
        reconcile_cart_shipping_state(&txn, cart_id).await?;
        txn.commit().await?;
        self.get_cart(tenant_id, cart_id).await
    }

    pub async fn update_line_item_pricing(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        line_item_id: Uuid,
        quantity: i32,
        unit_price: Decimal,
        pricing_adjustment: Option<CartPricingAdjustmentUpdate>,
    ) -> CartResult<CartResponse> {
        if quantity < 1 {
            return Err(CartError::Validation(
                "quantity must be at least 1".to_string(),
            ));
        }

        let txn = self.db.begin().await?;
        let cart = load_cart_in_tx(&txn, tenant_id, cart_id).await?;
        ensure_active(&cart.status, "update_line_item_pricing")?;

        let line_item = entities::cart_line_item::Entity::find_by_id(line_item_id)
            .filter(entities::cart_line_item::Column::CartId.eq(cart_id))
            .one(&txn)
            .await?
            .ok_or(CartError::CartLineItemNotFound(line_item_id))?;

        let mut active: entities::cart_line_item::ActiveModel = line_item.into();
        let now = Utc::now();
        active.unit_price = Set(unit_price);
        active.quantity = Set(quantity);
        active.total_price = Set(unit_price * Decimal::from(quantity));
        active.updated_at = Set(now.into());
        active.update(&txn).await?;
        replace_pricing_adjustments(
            &txn,
            cart.id,
            cart.currency_code.as_str(),
            vec![(line_item_id, pricing_adjustment)],
        )
        .await?;

        recalculate_totals(&txn, self.tax_calculation_port.as_ref(), cart).await?;
        reconcile_cart_shipping_state(&txn, cart_id).await?;
        txn.commit().await?;
        self.get_cart(tenant_id, cart_id).await
    }

    pub async fn reprice_line_items(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        updates: Vec<CartLineItemPricingUpdate>,
    ) -> CartResult<CartResponse> {
        if updates.is_empty() {
            return self.get_cart(tenant_id, cart_id).await;
        }

        let updates_map: HashMap<Uuid, CartLineItemPricingUpdate> = updates
            .into_iter()
            .map(|update| (update.line_item_id, update))
            .collect();
        let txn = self.db.begin().await?;
        let cart = load_cart_in_tx(&txn, tenant_id, cart_id).await?;
        ensure_active(&cart.status, "reprice_line_items")?;

        let line_items = entities::cart_line_item::Entity::find()
            .filter(entities::cart_line_item::Column::CartId.eq(cart_id))
            .all(&txn)
            .await?;

        let now = Utc::now();
        let mut pricing_adjustments = Vec::new();
        for line_item in line_items {
            if let Some(update) = updates_map.get(&line_item.id) {
                let line_item_id = line_item.id;
                let quantity = line_item.quantity;
                let mut active: entities::cart_line_item::ActiveModel = line_item.into();
                active.unit_price = Set(update.unit_price);
                active.total_price = Set(update.unit_price * Decimal::from(quantity));
                active.updated_at = Set(now.into());
                active.update(&txn).await?;
                pricing_adjustments.push((line_item_id, update.pricing_adjustment.clone()));
            }
        }
        replace_pricing_adjustments(
            &txn,
            cart.id,
            cart.currency_code.as_str(),
            pricing_adjustments,
        )
        .await?;

        recalculate_totals(&txn, self.tax_calculation_port.as_ref(), cart).await?;
        reconcile_cart_shipping_state(&txn, cart_id).await?;
        txn.commit().await?;
        self.get_cart(tenant_id, cart_id).await
    }

    pub async fn remove_line_item(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        line_item_id: Uuid,
    ) -> CartResult<CartResponse> {
        let txn = self.db.begin().await?;
        let cart = load_cart_in_tx(&txn, tenant_id, cart_id).await?;
        ensure_active(&cart.status, "remove_line_item")?;

        let line_item = entities::cart_line_item::Entity::find_by_id(line_item_id)
            .filter(entities::cart_line_item::Column::CartId.eq(cart_id))
            .one(&txn)
            .await?
            .ok_or(CartError::CartLineItemNotFound(line_item_id))?;
        entities::cart_adjustment::Entity::delete_many()
            .filter(entities::cart_adjustment::Column::CartLineItemId.eq(line_item_id))
            .exec(&txn)
            .await?;
        entities::cart_tax_line::Entity::delete_many()
            .filter(entities::cart_tax_line::Column::CartLineItemId.eq(line_item_id))
            .exec(&txn)
            .await?;
        entities::cart_line_item_translation::Entity::delete_many()
            .filter(entities::cart_line_item_translation::Column::CartLineItemId.eq(line_item_id))
            .exec(&txn)
            .await?;
        let active: entities::cart_line_item::ActiveModel = line_item.into();
        active.delete(&txn).await?;

        recalculate_totals(&txn, self.tax_calculation_port.as_ref(), cart).await?;
        reconcile_cart_shipping_state(&txn, cart_id).await?;
        txn.commit().await?;
        self.get_cart(tenant_id, cart_id).await
    }
}
