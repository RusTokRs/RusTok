use std::sync::Arc;

use chrono::Utc;
use rust_decimal::{Decimal, prelude::ToPrimitive};
use rustok_core::generate_id;
use rustok_tax::{TaxCalculationPort, in_process_tax_calculation_port};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
use serde_json::Value;
use uuid::Uuid;
use validator::Validate;

use crate::dto::{
    AddMarketplaceCartLineItemInput, AddMarketplaceCartLineItemResponse,
    CartMarketplaceLineSnapshot, MarketplaceCartLineSnapshotInput,
};
use crate::entities::{
    cart_line_item, cart_line_item_marketplace_snapshot, cart_line_item_translation,
};
use crate::error::{CartError, CartResult};

use super::cart::helpers::{
    build_response, ensure_active, load_cart, load_cart_in_tx, load_tenant_default_locale,
    normalize_shipping_profile_slug, recalculate_totals, reconcile_cart_shipping_state,
    sanitize_line_item_metadata,
};

#[derive(Clone)]
pub struct CartMarketplaceSnapshotService {
    db: DatabaseConnection,
    tax_calculation_port: Arc<dyn TaxCalculationPort>,
}

impl CartMarketplaceSnapshotService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            tax_calculation_port: in_process_tax_calculation_port(),
        }
    }

    pub fn with_tax_calculation_port(
        mut self,
        tax_calculation_port: Arc<dyn TaxCalculationPort>,
    ) -> Self {
        self.tax_calculation_port = tax_calculation_port;
        self
    }

    pub async fn add_marketplace_line_item(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        input: AddMarketplaceCartLineItemInput,
    ) -> CartResult<AddMarketplaceCartLineItemResponse> {
        input
            .validate()
            .map_err(|error| CartError::Validation(error.to_string()))?;
        if input.line_item.unit_price < Decimal::ZERO {
            return Err(CartError::Validation(
                "unit_price cannot be negative".to_string(),
            ));
        }
        let mut snapshot = normalize_snapshot_input(input.marketplace)?;
        validate_line_input_identity(&input.line_item, &snapshot)?;
        validate_snapshot_amounts(&snapshot, input.line_item.quantity)?;
        validate_decimal_unit_price(input.line_item.unit_price, &snapshot)?;

        let transaction = self.db.begin().await?;
        let cart = load_cart_in_tx(&transaction, tenant_id, cart_id).await?;
        ensure_active(&cart.status, "add_marketplace_line_item")?;
        validate_cart_currency(&cart.currency_code, &snapshot)?;

        let locale = match cart
            .locale_code
            .as_deref()
            .and_then(rustok_api::normalize_locale_tag)
        {
            Some(locale) => locale,
            None => load_tenant_default_locale(&transaction, tenant_id).await?,
        };
        let shipping_profile_slug =
            normalize_shipping_profile_slug(input.line_item.shipping_profile_slug.as_deref());
        if let Some(profile) = snapshot.fulfillment_profile_slug.as_deref() {
            if profile != shipping_profile_slug {
                return Err(CartError::Validation(
                    "marketplace snapshot fulfillment profile does not match cart line".to_string(),
                ));
            }
        } else {
            snapshot.fulfillment_profile_slug = Some(shipping_profile_slug.clone());
        }

        let line_item_id = generate_id();
        let now = Utc::now().fixed_offset();
        cart_line_item::ActiveModel {
            id: Set(line_item_id),
            cart_id: Set(cart_id),
            product_id: Set(input.line_item.product_id),
            variant_id: Set(input.line_item.variant_id),
            shipping_profile_slug: Set(shipping_profile_slug),
            sku: Set(input.line_item.sku),
            quantity: Set(input.line_item.quantity),
            unit_price: Set(input.line_item.unit_price),
            total_price: Set(input.line_item.unit_price * Decimal::from(input.line_item.quantity)),
            currency_code: Set(cart.currency_code.clone()),
            metadata: Set(compatibility_metadata(
                input.line_item.metadata,
                snapshot.seller_id,
            )),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&transaction)
        .await?;
        cart_line_item_translation::ActiveModel {
            id: Set(generate_id()),
            cart_line_item_id: Set(line_item_id),
            locale: Set(locale),
            title: Set(input.line_item.title),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&transaction)
        .await?;
        let snapshot_model = insert_snapshot(&transaction, line_item_id, snapshot).await?;

        recalculate_totals(&transaction, self.tax_calculation_port.as_ref(), cart).await?;
        reconcile_cart_shipping_state(&transaction, cart_id).await?;
        transaction.commit().await?;

        let cart = load_cart(&self.db, tenant_id, cart_id).await?;
        Ok(AddMarketplaceCartLineItemResponse {
            cart: build_response(&self.db, cart).await?,
            snapshot: map_snapshot(snapshot_model),
        })
    }

    pub async fn bind_line_snapshot(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        cart_line_item_id: Uuid,
        input: MarketplaceCartLineSnapshotInput,
    ) -> CartResult<CartMarketplaceLineSnapshot> {
        let mut input = normalize_snapshot_input(input)?;

        if let Some(existing) =
            cart_line_item_marketplace_snapshot::Entity::find_by_id(cart_line_item_id)
                .one(&self.db)
                .await?
        {
            let existing = map_snapshot(existing);
            if snapshot_matches(&existing, &input) {
                return Ok(existing);
            }
            return Err(CartError::Validation(format!(
                "cart marketplace snapshot for line {cart_line_item_id} is already bound to another immutable identity"
            )));
        }

        let transaction = self.db.begin().await?;
        let cart = load_cart_in_tx(&transaction, tenant_id, cart_id).await?;
        ensure_active(&cart.status, "bind_marketplace_snapshot")?;
        validate_cart_currency(&cart.currency_code, &input)?;
        let line = cart_line_item::Entity::find_by_id(cart_line_item_id)
            .filter(cart_line_item::Column::CartId.eq(cart_id))
            .one(&transaction)
            .await?
            .ok_or(CartError::CartLineItemNotFound(cart_line_item_id))?;
        if input.fulfillment_profile_slug.is_none() {
            input.fulfillment_profile_slug = Some(line.shipping_profile_slug.clone());
        }
        validate_line_binding(&line, &input)?;

        let insert = insert_snapshot(&transaction, cart_line_item_id, input.clone()).await;
        let model = match insert {
            Ok(model) => model,
            Err(error) => {
                transaction.rollback().await?;
                if let Some(existing) =
                    cart_line_item_marketplace_snapshot::Entity::find_by_id(cart_line_item_id)
                        .one(&self.db)
                        .await?
                {
                    let existing = map_snapshot(existing);
                    if snapshot_matches(&existing, &input) {
                        return Ok(existing);
                    }
                    return Err(CartError::Validation(format!(
                        "cart marketplace snapshot for line {cart_line_item_id} was concurrently bound to another immutable identity"
                    )));
                }
                return Err(error);
            }
        };
        transaction.commit().await?;
        Ok(map_snapshot(model))
    }

    pub async fn list_cart_snapshots(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
    ) -> CartResult<Vec<CartMarketplaceLineSnapshot>> {
        let transaction = self.db.begin().await?;
        load_cart_in_tx(&transaction, tenant_id, cart_id).await?;
        let line_ids = cart_line_item::Entity::find()
            .filter(cart_line_item::Column::CartId.eq(cart_id))
            .all(&transaction)
            .await?
            .into_iter()
            .map(|line| line.id)
            .collect::<Vec<_>>();
        if line_ids.is_empty() {
            transaction.commit().await?;
            return Ok(Vec::new());
        }
        let snapshots = cart_line_item_marketplace_snapshot::Entity::find()
            .filter(cart_line_item_marketplace_snapshot::Column::CartLineItemId.is_in(line_ids))
            .all(&transaction)
            .await?
            .into_iter()
            .map(map_snapshot)
            .collect();
        transaction.commit().await?;
        Ok(snapshots)
    }
}

async fn insert_snapshot<C: sea_orm::ConnectionTrait>(
    connection: &C,
    cart_line_item_id: Uuid,
    input: MarketplaceCartLineSnapshotInput,
) -> CartResult<cart_line_item_marketplace_snapshot::Model> {
    let now = Utc::now().fixed_offset();
    Ok(cart_line_item_marketplace_snapshot::ActiveModel {
        cart_line_item_id: Set(cart_line_item_id),
        seller_id: Set(input.seller_id),
        listing_id: Set(input.listing_id),
        master_product_id: Set(input.master_product_id),
        master_variant_id: Set(input.master_variant_id),
        listing_terms_version: Set(input.listing_terms_version),
        currency_code: Set(input.currency_code),
        currency_exponent: Set(input.currency_exponent),
        unit_amount: Set(input.unit_amount),
        subtotal_amount: Set(input.subtotal_amount),
        discount_amount: Set(input.discount_amount),
        tax_amount: Set(input.tax_amount),
        total_amount: Set(input.total_amount),
        pricing_reference: Set(input.pricing_reference),
        inventory_reference: Set(input.inventory_reference),
        fulfillment_profile_slug: Set(input.fulfillment_profile_slug),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(connection)
    .await?)
}

fn normalize_snapshot_input(
    mut input: MarketplaceCartLineSnapshotInput,
) -> CartResult<MarketplaceCartLineSnapshotInput> {
    input
        .validate()
        .map_err(|error| CartError::Validation(error.to_string()))?;
    for (value, field) in [
        (input.seller_id, "seller_id"),
        (input.listing_id, "listing_id"),
        (input.master_product_id, "master_product_id"),
        (input.master_variant_id, "master_variant_id"),
    ] {
        if value.is_nil() {
            return Err(CartError::Validation(format!(
                "marketplace snapshot {field} must not be nil"
            )));
        }
    }
    input.currency_code = input.currency_code.trim().to_ascii_uppercase();
    if input.currency_code.len() != 3
        || !input
            .currency_code
            .bytes()
            .all(|byte| byte.is_ascii_uppercase())
    {
        return Err(CartError::Validation(
            "marketplace snapshot currency_code must be a 3-letter code".to_string(),
        ));
    }
    input.pricing_reference = normalize_reference(input.pricing_reference);
    input.inventory_reference = normalize_reference(input.inventory_reference);
    input.fulfillment_profile_slug = input
        .fulfillment_profile_slug
        .as_deref()
        .map(|value| normalize_shipping_profile_slug(Some(value)));
    if input.discount_amount > input.subtotal_amount {
        return Err(CartError::Validation(
            "marketplace discount must not exceed subtotal".to_string(),
        ));
    }
    let expected_total = input
        .subtotal_amount
        .checked_sub(input.discount_amount)
        .and_then(|value| value.checked_add(input.tax_amount))
        .ok_or_else(|| CartError::Validation("marketplace total overflow".to_string()))?;
    if input.total_amount != expected_total {
        return Err(CartError::Validation(format!(
            "marketplace total must equal subtotal - discount + tax ({expected_total})"
        )));
    }
    Ok(input)
}

fn validate_line_input_identity(
    line: &crate::dto::AddCartLineItemInput,
    snapshot: &MarketplaceCartLineSnapshotInput,
) -> CartResult<()> {
    if line.product_id != Some(snapshot.master_product_id)
        || line.variant_id != Some(snapshot.master_variant_id)
    {
        return Err(CartError::Validation(
            "marketplace cart line product and variant must match the typed snapshot".to_string(),
        ));
    }
    Ok(())
}

fn validate_cart_currency(
    cart_currency_code: &str,
    input: &MarketplaceCartLineSnapshotInput,
) -> CartResult<()> {
    if !cart_currency_code.eq_ignore_ascii_case(&input.currency_code) {
        return Err(CartError::Validation(format!(
            "marketplace snapshot currency {} does not match cart currency {}",
            input.currency_code, cart_currency_code
        )));
    }
    Ok(())
}

fn validate_snapshot_amounts(
    input: &MarketplaceCartLineSnapshotInput,
    quantity: i32,
) -> CartResult<()> {
    let expected_subtotal = input
        .unit_amount
        .checked_mul(i64::from(quantity))
        .ok_or_else(|| CartError::Validation("marketplace subtotal overflow".to_string()))?;
    if input.subtotal_amount != expected_subtotal {
        return Err(CartError::Validation(format!(
            "marketplace subtotal {} does not match unit amount {} x quantity {}",
            input.subtotal_amount, input.unit_amount, quantity
        )));
    }
    Ok(())
}

fn validate_decimal_unit_price(
    unit_price: Decimal,
    input: &MarketplaceCartLineSnapshotInput,
) -> CartResult<()> {
    let exponent = u32::try_from(input.currency_exponent)
        .map_err(|_| CartError::Validation("currency exponent must be non-negative".to_string()))?;
    let factor = 10_i64
        .checked_pow(exponent)
        .ok_or_else(|| CartError::Validation("currency exponent is too large".to_string()))?;
    let scaled = unit_price
        .checked_mul(Decimal::from(factor))
        .ok_or_else(|| CartError::Validation("marketplace unit price overflow".to_string()))?;
    if !scaled.fract().is_zero() {
        return Err(CartError::Validation(format!(
            "unit price {unit_price} cannot be represented exactly with currency exponent {exponent}"
        )));
    }
    let minor_units = scaled.to_i64().ok_or_else(|| {
        CartError::Validation("marketplace unit price exceeds minor-unit range".to_string())
    })?;
    if minor_units != input.unit_amount {
        return Err(CartError::Validation(format!(
            "marketplace unit amount {} does not match cart unit price {}",
            input.unit_amount, unit_price
        )));
    }
    Ok(())
}

fn validate_line_binding(
    line: &cart_line_item::Model,
    input: &MarketplaceCartLineSnapshotInput,
) -> CartResult<()> {
    if line.product_id != Some(input.master_product_id)
        || line.variant_id != Some(input.master_variant_id)
    {
        return Err(CartError::Validation(format!(
            "cart line {} identity does not match marketplace snapshot",
            line.id
        )));
    }
    validate_snapshot_amounts(input, line.quantity)?;
    validate_decimal_unit_price(line.unit_price, input)?;
    if input.fulfillment_profile_slug.as_deref() != Some(line.shipping_profile_slug.as_str()) {
        return Err(CartError::Validation(format!(
            "cart line {} shipping profile does not match marketplace snapshot",
            line.id
        )));
    }
    Ok(())
}

fn compatibility_metadata(metadata: Value, seller_id: Uuid) -> Value {
    let metadata = sanitize_line_item_metadata(metadata);
    let mut metadata = match metadata {
        Value::Object(metadata) => metadata,
        _ => serde_json::Map::new(),
    };
    metadata.remove("marketplace");
    metadata.remove("seller");
    metadata.insert(
        "seller_id".to_string(),
        Value::String(seller_id.to_string()),
    );
    Value::Object(metadata)
}

fn normalize_reference(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn snapshot_matches(
    existing: &CartMarketplaceLineSnapshot,
    input: &MarketplaceCartLineSnapshotInput,
) -> bool {
    existing.seller_id == input.seller_id
        && existing.listing_id == input.listing_id
        && existing.master_product_id == input.master_product_id
        && existing.master_variant_id == input.master_variant_id
        && existing.listing_terms_version == input.listing_terms_version
        && existing.currency_code == input.currency_code
        && existing.currency_exponent == input.currency_exponent
        && existing.unit_amount == input.unit_amount
        && existing.subtotal_amount == input.subtotal_amount
        && existing.discount_amount == input.discount_amount
        && existing.tax_amount == input.tax_amount
        && existing.total_amount == input.total_amount
        && existing.pricing_reference == input.pricing_reference
        && existing.inventory_reference == input.inventory_reference
        && existing.fulfillment_profile_slug == input.fulfillment_profile_slug
}

fn map_snapshot(model: cart_line_item_marketplace_snapshot::Model) -> CartMarketplaceLineSnapshot {
    CartMarketplaceLineSnapshot {
        cart_line_item_id: model.cart_line_item_id,
        seller_id: model.seller_id,
        listing_id: model.listing_id,
        master_product_id: model.master_product_id,
        master_variant_id: model.master_variant_id,
        listing_terms_version: model.listing_terms_version,
        currency_code: model.currency_code,
        currency_exponent: model.currency_exponent,
        unit_amount: model.unit_amount,
        subtotal_amount: model.subtotal_amount,
        discount_amount: model.discount_amount,
        tax_amount: model.tax_amount,
        total_amount: model.total_amount,
        pricing_reference: model.pricing_reference,
        inventory_reference: model.inventory_reference,
        fulfillment_profile_slug: model.fulfillment_profile_slug,
    }
}
