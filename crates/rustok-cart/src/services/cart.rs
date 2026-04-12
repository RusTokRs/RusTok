use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, Set, Statement, TransactionTrait,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use tracing::instrument;
use uuid::Uuid;
use validator::Validate;

use rustok_commerce_foundation::entities::region;
use rustok_core::{generate_id, normalize_locale_tag, PLATFORM_FALLBACK_LOCALE};
use rustok_fulfillment::entities::shipping_option;

use crate::dto::{
    AddCartLineItemInput, CartAdjustmentResponse, CartDeliveryGroupResponse, CartLineItemResponse,
    CartResponse, CartTaxLineResponse, CreateCartInput, SetCartAdjustmentInput,
    UpdateCartContextInput,
};
use crate::entities;
use crate::error::{CartError, CartResult};

const STATUS_ACTIVE: &str = "active";
const STATUS_CHECKING_OUT: &str = "checking_out";
const STATUS_COMPLETED: &str = "completed";
const STATUS_ABANDONED: &str = "abandoned";
const DEFAULT_SHIPPING_PROFILE_SLUG: &str = "default";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct DeliveryGroupKey {
    shipping_profile_slug: String,
    seller_id: Option<String>,
    seller_scope: Option<String>,
}

#[derive(Clone, Debug)]
struct DeliveryGroupSnapshot {
    key: DeliveryGroupKey,
}

pub struct CartService {
    db: DatabaseConnection,
}

#[derive(Clone, Debug)]
pub struct CartLineItemPricingUpdate {
    pub line_item_id: Uuid,
    pub unit_price: Decimal,
    pub pricing_adjustment: Option<CartPricingAdjustmentUpdate>,
}

#[derive(Clone, Debug)]
pub struct CartPricingAdjustmentUpdate {
    pub source_id: Option<String>,
    pub amount: Decimal,
    pub metadata: Value,
}

const PRICING_ADJUSTMENT_SOURCE_TYPE: &str = "pricing";

impl CartService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
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
        let cart = self.load_cart(tenant_id, cart_id).await?;
        self.build_response(cart).await
    }

    pub async fn add_line_item(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        input: AddCartLineItemInput,
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
        let cart = self.load_cart_in_tx(&txn, tenant_id, cart_id).await?;
        ensure_active(&cart.status, "add_line_item")?;
        let now = Utc::now();
        let metadata = sanitize_line_item_metadata(input.metadata);
        let locale = match cart.locale_code.as_deref().and_then(normalize_locale_tag) {
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
            metadata: Set(metadata),
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

        self.recalculate_totals(&txn, cart).await?;
        self.reconcile_cart_shipping_state(&txn, cart_id).await?;
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
        let cart = self.load_cart_in_tx(&txn, tenant_id, cart_id).await?;
        ensure_active(&cart.status, "update_context")?;
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
        self.apply_shipping_selection_patch(&txn, &cart, &shipping_patch_input)
            .await?;

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
        let cart = self.load_cart_in_tx(&txn, tenant_id, cart_id).await?;
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

        self.recalculate_totals(&txn, cart).await?;
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
        let cart = self.load_cart_in_tx(&txn, tenant_id, cart_id).await?;
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

        self.recalculate_totals(&txn, cart).await?;
        self.reconcile_cart_shipping_state(&txn, cart_id).await?;
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
        let cart = self.load_cart_in_tx(&txn, tenant_id, cart_id).await?;
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
        self.replace_pricing_adjustments(
            &txn,
            cart.id,
            cart.currency_code.as_str(),
            vec![(line_item_id, pricing_adjustment)],
        )
        .await?;

        self.recalculate_totals(&txn, cart).await?;
        self.reconcile_cart_shipping_state(&txn, cart_id).await?;
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
        let cart = self.load_cart_in_tx(&txn, tenant_id, cart_id).await?;
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
        self.replace_pricing_adjustments(
            &txn,
            cart.id,
            cart.currency_code.as_str(),
            pricing_adjustments,
        )
        .await?;

        self.recalculate_totals(&txn, cart).await?;
        self.reconcile_cart_shipping_state(&txn, cart_id).await?;
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
        let cart = self.load_cart_in_tx(&txn, tenant_id, cart_id).await?;
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
        let active: entities::cart_line_item::ActiveModel = line_item.into();
        active.delete(&txn).await?;

        self.recalculate_totals(&txn, cart).await?;
        self.reconcile_cart_shipping_state(&txn, cart_id).await?;
        txn.commit().await?;
        self.get_cart(tenant_id, cart_id).await
    }

    pub async fn complete_cart(&self, tenant_id: Uuid, cart_id: Uuid) -> CartResult<CartResponse> {
        self.transition_cart_from_any(
            tenant_id,
            cart_id,
            &[STATUS_ACTIVE, STATUS_CHECKING_OUT],
            STATUS_COMPLETED,
            true,
        )
        .await
    }

    pub async fn abandon_cart(&self, tenant_id: Uuid, cart_id: Uuid) -> CartResult<CartResponse> {
        self.transition_cart(tenant_id, cart_id, STATUS_ACTIVE, STATUS_ABANDONED, false)
            .await
    }

    pub async fn begin_checkout(&self, tenant_id: Uuid, cart_id: Uuid) -> CartResult<CartResponse> {
        self.transition_cart(
            tenant_id,
            cart_id,
            STATUS_ACTIVE,
            STATUS_CHECKING_OUT,
            false,
        )
        .await
    }

    pub async fn release_checkout(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
    ) -> CartResult<CartResponse> {
        self.transition_cart(
            tenant_id,
            cart_id,
            STATUS_CHECKING_OUT,
            STATUS_ACTIVE,
            false,
        )
        .await
    }

    async fn transition_cart(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        expected_from: &str,
        next_status: &str,
        mark_completed: bool,
    ) -> CartResult<CartResponse> {
        let txn = self.db.begin().await?;
        let cart = self.load_cart_in_tx(&txn, tenant_id, cart_id).await?;
        if cart.status != expected_from {
            return Err(CartError::InvalidTransition {
                from: cart.status,
                to: next_status.to_string(),
            });
        }

        let mut active: entities::cart::ActiveModel = cart.into();
        let now = Utc::now();
        active.status = Set(next_status.to_string());
        active.updated_at = Set(now.into());
        active.completed_at = Set(if mark_completed {
            Some(now.into())
        } else {
            None
        });
        active.update(&txn).await?;
        txn.commit().await?;
        self.get_cart(tenant_id, cart_id).await
    }

    async fn transition_cart_from_any(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        expected_from: &[&str],
        next_status: &str,
        mark_completed: bool,
    ) -> CartResult<CartResponse> {
        let txn = self.db.begin().await?;
        let cart = self.load_cart_in_tx(&txn, tenant_id, cart_id).await?;
        if !expected_from.contains(&cart.status.as_str()) {
            return Err(CartError::InvalidTransition {
                from: cart.status,
                to: next_status.to_string(),
            });
        }

        let mut active: entities::cart::ActiveModel = cart.into();
        let now = Utc::now();
        active.status = Set(next_status.to_string());
        active.updated_at = Set(now.into());
        active.completed_at = Set(if mark_completed {
            Some(now.into())
        } else {
            None
        });
        active.update(&txn).await?;
        txn.commit().await?;
        self.get_cart(tenant_id, cart_id).await
    }

    async fn recalculate_totals<C>(&self, conn: &C, cart: entities::cart::Model) -> CartResult<()>
    where
        C: sea_orm::ConnectionTrait,
    {
        let line_items = entities::cart_line_item::Entity::find()
            .filter(entities::cart_line_item::Column::CartId.eq(cart.id))
            .all(conn)
            .await?;
        let adjustments = entities::cart_adjustment::Entity::find()
            .filter(entities::cart_adjustment::Column::CartId.eq(cart.id))
            .all(conn)
            .await?;
        let shipping_selections = entities::cart_shipping_selection::Entity::find()
            .filter(entities::cart_shipping_selection::Column::CartId.eq(cart.id))
            .all(conn)
            .await?;
        let (tax_total, tax_included) = self
            .recalculate_tax_lines(conn, &cart, &line_items, &shipping_selections)
            .await?;
        let subtotal = subtotal_amount(&line_items);
        let adjusted_total = net_total(subtotal, adjustment_total(&adjustments));
        let total_amount = if tax_included {
            adjusted_total
        } else {
            adjusted_total + tax_total
        };

        let mut active: entities::cart::ActiveModel = cart.into();
        active.total_amount = Set(total_amount);
        active.tax_total = Set(tax_total);
        active.updated_at = Set(Utc::now().into());
        active.update(conn).await?;
        Ok(())
    }

    async fn replace_pricing_adjustments<C>(
        &self,
        conn: &C,
        cart_id: Uuid,
        currency_code: &str,
        updates: Vec<(Uuid, Option<CartPricingAdjustmentUpdate>)>,
    ) -> CartResult<()>
    where
        C: sea_orm::ConnectionTrait,
    {
        if updates.is_empty() {
            return Ok(());
        }

        let line_item_ids = updates.iter().map(|(line_item_id, _)| *line_item_id).collect::<Vec<_>>();
        entities::cart_adjustment::Entity::delete_many()
            .filter(entities::cart_adjustment::Column::CartId.eq(cart_id))
            .filter(entities::cart_adjustment::Column::SourceType.eq(PRICING_ADJUSTMENT_SOURCE_TYPE))
            .filter(entities::cart_adjustment::Column::CartLineItemId.is_in(line_item_ids))
            .exec(conn)
            .await?;

        let now = Utc::now();
        for (line_item_id, adjustment) in updates {
            let Some(adjustment) = adjustment else {
                continue;
            };
            if adjustment.amount <= Decimal::ZERO {
                continue;
            }

            entities::cart_adjustment::ActiveModel {
                id: Set(generate_id()),
                cart_id: Set(cart_id),
                cart_line_item_id: Set(Some(line_item_id)),
                source_type: Set(PRICING_ADJUSTMENT_SOURCE_TYPE.to_string()),
                source_id: Set(normalize_adjustment_source_id(adjustment.source_id.as_deref())),
                amount: Set(adjustment.amount),
                currency_code: Set(currency_code.to_ascii_uppercase()),
                metadata: Set(sanitize_adjustment_metadata(adjustment.metadata)),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(conn)
            .await?;
        }

        Ok(())
    }

    async fn load_cart(&self, tenant_id: Uuid, cart_id: Uuid) -> CartResult<entities::cart::Model> {
        self.load_cart_in_tx(&self.db, tenant_id, cart_id).await
    }

    async fn load_cart_in_tx<C>(
        &self,
        conn: &C,
        tenant_id: Uuid,
        cart_id: Uuid,
    ) -> CartResult<entities::cart::Model>
    where
        C: sea_orm::ConnectionTrait,
    {
        entities::cart::Entity::find_by_id(cart_id)
            .filter(entities::cart::Column::TenantId.eq(tenant_id))
            .one(conn)
            .await?
            .ok_or(CartError::CartNotFound(cart_id))
    }

    async fn build_response(&self, cart: entities::cart::Model) -> CartResult<CartResponse> {
        let line_items = entities::cart_line_item::Entity::find()
            .filter(entities::cart_line_item::Column::CartId.eq(cart.id))
            .order_by_asc(entities::cart_line_item::Column::CreatedAt)
            .all(&self.db)
            .await?;
        let tenant_default_locale = load_tenant_default_locale(&self.db, cart.tenant_id).await?;
        let title_map = load_line_item_titles(
            &self.db,
            &line_items,
            cart.locale_code.as_deref(),
            Some(tenant_default_locale.as_str()),
        )
        .await?;
        let adjustments = entities::cart_adjustment::Entity::find()
            .filter(entities::cart_adjustment::Column::CartId.eq(cart.id))
            .order_by_asc(entities::cart_adjustment::Column::CreatedAt)
            .all(&self.db)
            .await?;
        let tax_lines = entities::cart_tax_line::Entity::find()
            .filter(entities::cart_tax_line::Column::CartId.eq(cart.id))
            .order_by_asc(entities::cart_tax_line::Column::CreatedAt)
            .all(&self.db)
            .await?;
        let shipping_selections = entities::cart_shipping_selection::Entity::find()
            .filter(entities::cart_shipping_selection::Column::CartId.eq(cart.id))
            .all(&self.db)
            .await?;
        let subtotal_amount = subtotal_amount(&line_items);
        let adjustment_total = adjustment_total(&adjustments);
        let total_amount = cart.total_amount;
        let delivery_group_snapshots = collect_delivery_group_snapshots(&line_items);
        let selection_map =
            selection_map_from_records(&delivery_group_snapshots, shipping_selections.into_iter());
        let delivery_groups = build_delivery_groups(&line_items, &selection_map);
        let selected_shipping_option_id = match delivery_groups.len() {
            0 => cart.selected_shipping_option_id,
            1 => delivery_groups[0].selected_shipping_option_id,
            _ => None,
        };

        Ok(CartResponse {
            id: cart.id,
            tenant_id: cart.tenant_id,
            channel_id: cart.channel_id,
            channel_slug: cart.channel_slug,
            customer_id: cart.customer_id,
            email: cart.email,
            region_id: cart.region_id,
            country_code: cart.country_code,
            locale_code: cart.locale_code,
            selected_shipping_option_id,
            status: cart.status,
            currency_code: cart.currency_code,
            subtotal_amount,
            adjustment_total,
            total_amount,
            tax_total: cart.tax_total,
            metadata: cart.metadata,
            created_at: cart.created_at.with_timezone(&Utc),
            updated_at: cart.updated_at.with_timezone(&Utc),
            completed_at: cart.completed_at.map(|value| value.with_timezone(&Utc)),
            line_items: line_items
                .into_iter()
                .map(|item| {
                    let seller_id = seller_id_from_metadata(&item.metadata);
                    let seller_scope = seller_scope_from_metadata(&item.metadata);
                    CartLineItemResponse {
                        id: item.id,
                        cart_id: item.cart_id,
                        product_id: item.product_id,
                        variant_id: item.variant_id,
                        shipping_profile_slug: item.shipping_profile_slug,
                        seller_id,
                        seller_scope,
                        sku: item.sku,
                        title: title_map.get(&item.id).cloned().unwrap_or_default(),
                        quantity: item.quantity,
                        unit_price: item.unit_price,
                        total_price: item.total_price,
                        currency_code: item.currency_code,
                        metadata: item.metadata,
                        created_at: item.created_at.with_timezone(&Utc),
                        updated_at: item.updated_at.with_timezone(&Utc),
                    }
                })
                .collect(),
            adjustments: adjustments
                .into_iter()
                .map(|adjustment| CartAdjustmentResponse {
                    id: adjustment.id,
                    cart_id: adjustment.cart_id,
                    line_item_id: adjustment.cart_line_item_id,
                    source_type: adjustment.source_type,
                    source_id: adjustment.source_id,
                    amount: adjustment.amount,
                    currency_code: adjustment.currency_code,
                    metadata: adjustment.metadata,
                    created_at: adjustment.created_at.with_timezone(&Utc),
                    updated_at: adjustment.updated_at.with_timezone(&Utc),
                })
                .collect(),
            tax_lines: tax_lines
                .into_iter()
                .map(|line| CartTaxLineResponse {
                    id: line.id,
                    cart_id: line.cart_id,
                    line_item_id: line.cart_line_item_id,
                    shipping_option_id: line.shipping_option_id,
                    description: line.description,
                    rate: line.rate,
                    amount: line.amount,
                    currency_code: line.currency_code,
                    metadata: line.metadata,
                    created_at: line.created_at.with_timezone(&Utc),
                    updated_at: line.updated_at.with_timezone(&Utc),
                })
                .collect(),
            delivery_groups,
        })
    }

    async fn recalculate_tax_lines<C>(
        &self,
        conn: &C,
        cart: &entities::cart::Model,
        line_items: &[entities::cart_line_item::Model],
        shipping_selections: &[entities::cart_shipping_selection::Model],
    ) -> CartResult<(Decimal, bool)>
    where
        C: sea_orm::ConnectionTrait,
    {
        entities::cart_tax_line::Entity::delete_many()
            .filter(entities::cart_tax_line::Column::CartId.eq(cart.id))
            .exec(conn)
            .await?;

        let Some(region_id) = cart.region_id else {
            return Ok((Decimal::ZERO, false));
        };

        let region = region::Entity::find_by_id(region_id)
            .filter(region::Column::TenantId.eq(cart.tenant_id))
            .one(conn)
            .await?
            .ok_or(CartError::Validation(
                "Region not found for cart".to_string(),
            ))?;
        let tax_rate = region.tax_rate;
        if tax_rate <= Decimal::ZERO {
            return Ok((Decimal::ZERO, region.tax_included));
        }

        let mut tax_lines = Vec::new();
        let mut tax_total = Decimal::ZERO;
        let now = Utc::now();
        for item in line_items {
            if item.total_price <= Decimal::ZERO {
                continue;
            }
            let tax_amount = calculate_tax_amount(item.total_price, tax_rate, region.tax_included);
            if tax_amount <= Decimal::ZERO {
                continue;
            }
            tax_total += tax_amount;
            tax_lines.push(entities::cart_tax_line::ActiveModel {
                id: Set(generate_id()),
                cart_id: Set(cart.id),
                cart_line_item_id: Set(Some(item.id)),
                shipping_option_id: Set(None),
                description: Set(Some("line_item".to_string())),
                rate: Set(tax_rate),
                amount: Set(tax_amount),
                currency_code: Set(cart.currency_code.clone()),
                metadata: Set(serde_json::json!({ "tax_included": region.tax_included })),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            });
        }

        for selection in shipping_selections {
            let Some(shipping_option_id) = selection.selected_shipping_option_id else {
                continue;
            };
            let option = shipping_option::Entity::find_by_id(shipping_option_id)
                .filter(shipping_option::Column::TenantId.eq(cart.tenant_id))
                .one(conn)
                .await?;
            let Some(option) = option else {
                continue;
            };
            if option.currency_code != cart.currency_code {
                continue;
            }
            if option.amount <= Decimal::ZERO {
                continue;
            }
            let tax_amount = calculate_tax_amount(option.amount, tax_rate, region.tax_included);
            if tax_amount <= Decimal::ZERO {
                continue;
            }
            tax_total += tax_amount;
            tax_lines.push(entities::cart_tax_line::ActiveModel {
                id: Set(generate_id()),
                cart_id: Set(cart.id),
                cart_line_item_id: Set(None),
                shipping_option_id: Set(Some(option.id)),
                description: Set(Some("shipping".to_string())),
                rate: Set(tax_rate),
                amount: Set(tax_amount),
                currency_code: Set(cart.currency_code.clone()),
                metadata: Set(serde_json::json!({
                    "tax_included": region.tax_included,
                })),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            });
        }

        if !tax_lines.is_empty() {
            entities::cart_tax_line::Entity::insert_many(tax_lines)
                .exec(conn)
                .await?;
        }

        Ok((tax_total, region.tax_included))
    }

    async fn apply_shipping_selection_patch<C>(
        &self,
        conn: &C,
        cart: &entities::cart::Model,
        input: &UpdateCartContextInput,
    ) -> CartResult<()>
    where
        C: sea_orm::ConnectionTrait,
    {
        let line_items = entities::cart_line_item::Entity::find()
            .filter(entities::cart_line_item::Column::CartId.eq(cart.id))
            .all(conn)
            .await?;
        let available_group_snapshots = collect_delivery_group_snapshots(&line_items);
        let existing = entities::cart_shipping_selection::Entity::find()
            .filter(entities::cart_shipping_selection::Column::CartId.eq(cart.id))
            .all(conn)
            .await?;
        let mut desired =
            selection_map_from_records(&available_group_snapshots, existing.into_iter());

        if let Some(shipping_selections) = &input.shipping_selections {
            desired.clear();
            for selection in shipping_selections {
                let normalized =
                    normalize_shipping_profile_slug(Some(selection.shipping_profile_slug.as_str()));
                let normalized_seller_id = normalize_seller_id(selection.seller_id.as_deref());
                let normalized_seller_scope =
                    normalize_seller_scope(selection.seller_scope.as_deref());
                let matching_keys = matching_delivery_group_keys(
                    &available_group_snapshots,
                    normalized.as_str(),
                    normalized_seller_id.as_deref(),
                    normalized_seller_scope.as_deref(),
                );
                for key in matching_keys {
                    desired.insert(key, selection.selected_shipping_option_id);
                }
            }
        } else if available_group_snapshots.len() <= 1 {
            if let Some(group) = available_group_snapshots.iter().next() {
                desired.insert(group.key.clone(), input.selected_shipping_option_id);
            } else {
                desired.clear();
            }
        } else if input.selected_shipping_option_id != cart.selected_shipping_option_id
            && input.selected_shipping_option_id.is_some()
        {
            return Err(CartError::Validation(
                "selected_shipping_option_id can only be used for carts with a single delivery group"
                    .to_string(),
            ));
        }

        self.store_shipping_selections(conn, cart.id, desired)
            .await?;
        self.reconcile_cart_shipping_state(conn, cart.id).await
    }

    async fn store_shipping_selections<C>(
        &self,
        conn: &C,
        cart_id: Uuid,
        desired: BTreeMap<DeliveryGroupKey, Option<Uuid>>,
    ) -> CartResult<()>
    where
        C: sea_orm::ConnectionTrait,
    {
        let existing = entities::cart_shipping_selection::Entity::find()
            .filter(entities::cart_shipping_selection::Column::CartId.eq(cart_id))
            .all(conn)
            .await?;
        let existing_map = existing
            .into_iter()
            .map(|selection| {
                (
                    DeliveryGroupKey {
                        shipping_profile_slug: selection.shipping_profile_slug.clone(),
                        seller_id: normalize_seller_id(selection.seller_id.as_deref()),
                        seller_scope: normalize_seller_scope(selection.seller_scope.as_deref()),
                    },
                    selection,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let now = Utc::now();

        for (group_key, selected_shipping_option_id) in &desired {
            if let Some(current) = existing_map.get(group_key) {
                let mut active: entities::cart_shipping_selection::ActiveModel =
                    current.clone().into();
                active.selected_shipping_option_id = Set(*selected_shipping_option_id);
                active.updated_at = Set(now.into());
                active.update(conn).await?;
            } else {
                entities::cart_shipping_selection::ActiveModel {
                    id: Set(generate_id()),
                    cart_id: Set(cart_id),
                    shipping_profile_slug: Set(group_key.shipping_profile_slug.clone()),
                    seller_id: Set(group_key.seller_id.clone()),
                    seller_scope: Set(group_key.seller_scope.clone()),
                    selected_shipping_option_id: Set(*selected_shipping_option_id),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(conn)
                .await?;
            }
        }

        for (group_key, current) in existing_map {
            if !desired.contains_key(&group_key) {
                let active: entities::cart_shipping_selection::ActiveModel = current.into();
                active.delete(conn).await?;
            }
        }

        Ok(())
    }

    async fn reconcile_cart_shipping_state<C>(&self, conn: &C, cart_id: Uuid) -> CartResult<()>
    where
        C: sea_orm::ConnectionTrait,
    {
        let cart = entities::cart::Entity::find_by_id(cart_id)
            .one(conn)
            .await?
            .ok_or(CartError::CartNotFound(cart_id))?;
        let line_items = entities::cart_line_item::Entity::find()
            .filter(entities::cart_line_item::Column::CartId.eq(cart_id))
            .order_by_asc(entities::cart_line_item::Column::CreatedAt)
            .all(conn)
            .await?;
        let delivery_group_snapshots = collect_delivery_group_snapshots(&line_items);
        let mut desired = entities::cart_shipping_selection::Entity::find()
            .filter(entities::cart_shipping_selection::Column::CartId.eq(cart_id))
            .all(conn)
            .await
            .map(|records| {
                selection_map_from_records(&delivery_group_snapshots, records.into_iter())
            })?;

        if delivery_group_snapshots.len() == 1
            && desired.is_empty()
            && cart.selected_shipping_option_id.is_some()
            && !line_items.is_empty()
        {
            if let Some(group) = delivery_group_snapshots.iter().next() {
                desired.insert(group.key.clone(), cart.selected_shipping_option_id);
            }
        }

        self.store_shipping_selections(conn, cart_id, desired.clone())
            .await?;

        let legacy_selected_shipping_option_id = match delivery_group_snapshots.len() {
            0 => cart.selected_shipping_option_id,
            1 => delivery_group_snapshots
                .iter()
                .next()
                .and_then(|group| desired.get(&group.key).copied().flatten()),
            _ => None,
        };
        let mut active: entities::cart::ActiveModel = cart.into();
        active.selected_shipping_option_id = Set(legacy_selected_shipping_option_id);
        active.updated_at = Set(Utc::now().into());
        active.update(conn).await?;
        Ok(())
    }
}

fn ensure_active(status: &str, action: &str) -> CartResult<()> {
    if status == STATUS_ACTIVE {
        Ok(())
    } else {
        Err(CartError::InvalidTransition {
            from: status.to_string(),
            to: action.to_string(),
        })
    }
}

fn normalize_country_code(value: &str) -> CartResult<String> {
    let normalized = value.trim().to_ascii_uppercase();
    if normalized.len() == 2 && normalized.chars().all(|ch| ch.is_ascii_alphabetic()) {
        Ok(normalized)
    } else {
        Err(CartError::Validation(format!(
            "country_code `{value}` is invalid"
        )))
    }
}

fn normalize_locale_code(value: &str) -> CartResult<String> {
    let normalized = value.trim().replace('_', "-").to_ascii_lowercase();
    if (2..=10).contains(&normalized.len()) {
        Ok(normalized)
    } else {
        Err(CartError::Validation(format!(
            "locale_code `{value}` is invalid"
        )))
    }
}

fn normalize_shipping_profile_slug(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_else(|| DEFAULT_SHIPPING_PROFILE_SLUG.to_string())
}

fn normalize_seller_scope(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

fn normalize_seller_id(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_owned())
}

fn normalize_adjustment_source_type(value: &str) -> CartResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() || normalized.len() > 64 {
        return Err(CartError::Validation(
            "adjustment source_type must be 1-64 characters".to_string(),
        ));
    }
    Ok(normalized)
}

fn normalize_adjustment_source_id(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_owned())
}

fn subtotal_amount(line_items: &[entities::cart_line_item::Model]) -> Decimal {
    line_items
        .iter()
        .fold(Decimal::ZERO, |acc, item| acc + item.total_price)
}

fn adjustment_total(adjustments: &[entities::cart_adjustment::Model]) -> Decimal {
    adjustments
        .iter()
        .fold(Decimal::ZERO, |acc, adjustment| acc + adjustment.amount)
}

fn calculate_tax_amount(amount: Decimal, rate: Decimal, tax_included: bool) -> Decimal {
    if rate <= Decimal::ZERO {
        return Decimal::ZERO;
    }

    if tax_included {
        let divisor = rate + Decimal::from(100);
        if divisor <= Decimal::ZERO {
            Decimal::ZERO
        } else {
            amount * rate / divisor
        }
    } else {
        amount * rate / Decimal::from(100)
    }
}

fn net_total(subtotal_amount: Decimal, adjustment_total: Decimal) -> Decimal {
    if adjustment_total > subtotal_amount {
        Decimal::ZERO
    } else {
        subtotal_amount - adjustment_total
    }
}

fn seller_id_from_metadata(metadata: &Value) -> Option<String> {
    metadata
        .get("seller")
        .and_then(|seller| seller.get("id"))
        .and_then(Value::as_str)
        .and_then(|value| normalize_seller_id(Some(value)))
        .or_else(|| {
            metadata
                .get("seller_id")
                .and_then(Value::as_str)
                .and_then(|value| normalize_seller_id(Some(value)))
        })
}

fn seller_scope_from_metadata(metadata: &Value) -> Option<String> {
    metadata
        .get("seller")
        .and_then(|seller| seller.get("scope"))
        .and_then(Value::as_str)
        .and_then(|value| normalize_seller_scope(Some(value)))
        .or_else(|| {
            metadata
                .get("seller_scope")
                .and_then(Value::as_str)
                .and_then(|value| normalize_seller_scope(Some(value)))
        })
}

fn delivery_group_snapshot_for_line_item(
    item: &entities::cart_line_item::Model,
) -> DeliveryGroupSnapshot {
    DeliveryGroupSnapshot {
        key: DeliveryGroupKey {
            shipping_profile_slug: normalize_shipping_profile_slug(Some(
                item.shipping_profile_slug.as_str(),
            )),
            seller_id: seller_id_from_metadata(&item.metadata),
            seller_scope: seller_scope_from_metadata(&item.metadata),
        },
    }
}

fn collect_delivery_group_snapshots(
    line_items: &[entities::cart_line_item::Model],
) -> BTreeSet<DeliveryGroupSnapshot> {
    line_items
        .iter()
        .map(delivery_group_snapshot_for_line_item)
        .collect()
}

impl PartialEq for DeliveryGroupSnapshot {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for DeliveryGroupSnapshot {}

impl PartialOrd for DeliveryGroupSnapshot {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DeliveryGroupSnapshot {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.key.cmp(&other.key)
    }
}

fn matching_delivery_group_keys(
    available_groups: &BTreeSet<DeliveryGroupSnapshot>,
    shipping_profile_slug: &str,
    seller_id: Option<&str>,
    seller_scope: Option<&str>,
) -> Vec<DeliveryGroupKey> {
    available_groups
        .iter()
        .filter(|group| {
            if group.key.shipping_profile_slug != shipping_profile_slug {
                return false;
            }

            if let Some(seller_id) = seller_id {
                return group.key.seller_id.as_deref() == Some(seller_id);
            }

            match seller_scope {
                Some(seller_scope) => {
                    group.key.seller_id.is_none()
                        && group.key.seller_scope.as_deref() == Some(seller_scope)
                }
                None => group.key.seller_id.is_none(),
            }
        })
        .map(|group| group.key.clone())
        .collect()
}

fn selection_map_from_records<I>(
    available_groups: &BTreeSet<DeliveryGroupSnapshot>,
    records: I,
) -> BTreeMap<DeliveryGroupKey, Option<Uuid>>
where
    I: IntoIterator<Item = entities::cart_shipping_selection::Model>,
{
    let mut desired = BTreeMap::new();
    let mut legacy_records = Vec::new();

    for record in records {
        let seller_id = normalize_seller_id(record.seller_id.as_deref());
        let seller_scope = normalize_seller_scope(record.seller_scope.as_deref());
        if seller_id.is_some() || seller_scope.is_some() {
            for key in matching_delivery_group_keys(
                available_groups,
                record.shipping_profile_slug.as_str(),
                seller_id.as_deref(),
                seller_scope.as_deref(),
            ) {
                desired.insert(key, record.selected_shipping_option_id);
            }
        } else {
            legacy_records.push(record);
        }
    }

    for record in legacy_records {
        for key in matching_delivery_group_keys(
            available_groups,
            record.shipping_profile_slug.as_str(),
            None,
            None,
        ) {
            desired
                .entry(key)
                .or_insert(record.selected_shipping_option_id);
        }
    }

    desired
}

fn build_delivery_groups(
    line_items: &[entities::cart_line_item::Model],
    selection_map: &BTreeMap<DeliveryGroupKey, Option<Uuid>>,
) -> Vec<CartDeliveryGroupResponse> {
    let mut groups = BTreeMap::<DeliveryGroupKey, Vec<Uuid>>::new();
    for item in line_items {
        let snapshot = delivery_group_snapshot_for_line_item(item);
        groups
            .entry(snapshot.key)
            .and_modify(|line_item_ids| line_item_ids.push(item.id))
            .or_insert_with(|| vec![item.id]);
    }

    groups
        .into_iter()
        .map(|(group_key, line_item_ids)| CartDeliveryGroupResponse {
            selected_shipping_option_id: selection_map.get(&group_key).copied().flatten(),
            shipping_profile_slug: group_key.shipping_profile_slug,
            seller_id: group_key.seller_id,
            seller_scope: group_key.seller_scope,
            line_item_ids,
            available_shipping_options: Vec::new(),
        })
        .collect()
}

fn sanitize_line_item_metadata(metadata: Value) -> Value {
    let mut metadata = match metadata {
        Value::Object(object) => object,
        value => return value,
    };

    metadata.remove("seller_label");

    if let Some(Value::Object(mut seller)) = metadata.remove("seller") {
        seller.remove("label");
        metadata.insert("seller".to_string(), Value::Object(seller));
    }

    Value::Object(metadata)
}

fn sanitize_adjustment_metadata(metadata: Value) -> Value {
    let mut metadata = match metadata {
        Value::Object(object) => object,
        value => return value,
    };

    metadata.remove("label");
    metadata.remove("display_label");
    metadata.remove("localized_label");

    Value::Object(metadata)
}

async fn load_line_item_titles<C>(
    conn: &C,
    line_items: &[entities::cart_line_item::Model],
    preferred_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> CartResult<HashMap<Uuid, String>>
where
    C: sea_orm::ConnectionTrait,
{
    let mut titles = HashMap::new();
    if line_items.is_empty() {
        return Ok(titles);
    }

    let preferred_locale = preferred_locale.and_then(normalize_locale_tag);
    let fallback_locale = tenant_default_locale.and_then(normalize_locale_tag);
    let line_item_ids = line_items.iter().map(|item| item.id).collect::<Vec<_>>();
    let rows = entities::cart_line_item_translation::Entity::find()
        .filter(
            entities::cart_line_item_translation::Column::CartLineItemId
                .is_in(line_item_ids.clone()),
        )
        .all(conn)
        .await?;

    let mut rows_by_item = HashMap::<Uuid, Vec<entities::cart_line_item_translation::Model>>::new();
    for row in rows {
        rows_by_item.entry(row.cart_line_item_id).or_default().push(row);
    }

    for line_item in line_items {
        if let Some(title) = rows_by_item
            .remove(&line_item.id)
            .and_then(|rows| {
                select_cart_line_item_title(
                    &rows,
                    preferred_locale.as_deref(),
                    fallback_locale.as_deref(),
                )
            })
        {
            titles.insert(line_item.id, title);
        }
    }

    Ok(titles)
}

fn select_cart_line_item_title(
    rows: &[entities::cart_line_item_translation::Model],
    preferred_locale: Option<&str>,
    fallback_locale: Option<&str>,
) -> Option<String> {
    let preferred_locale = preferred_locale.and_then(normalize_locale_tag);
    let fallback_locale = fallback_locale.and_then(normalize_locale_tag);

    preferred_locale
        .as_deref()
        .and_then(|preferred_locale| {
            rows.iter().find(|row| {
                normalize_locale_tag(&row.locale).as_deref() == Some(preferred_locale)
            })
        })
        .or_else(|| {
            fallback_locale.as_deref().and_then(|fallback_locale| {
                rows.iter().find(|row| {
                    normalize_locale_tag(&row.locale).as_deref() == Some(fallback_locale)
                })
            })
        })
        .or_else(|| rows.first())
        .map(|row| row.title.clone())
}

async fn load_tenant_default_locale<C>(conn: &C, tenant_id: Uuid) -> CartResult<String>
where
    C: ConnectionTrait,
{
    let row = conn
        .query_one(Statement::from_sql_and_values(
            conn.get_database_backend(),
            "SELECT default_locale FROM tenants WHERE id = ?",
            vec![tenant_id.into()],
        ))
        .await?;

    let default_locale = row
        .and_then(|row| row.try_get::<String>("", "default_locale").ok())
        .as_deref()
        .map(normalize_locale_code)
        .transpose()?
        .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());

    Ok(default_locale)
}
