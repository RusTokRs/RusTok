use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set, TransactionTrait};
use uuid::Uuid;
use validator::Validate;

use crate::dto::{CartMarketplaceLineSnapshot, MarketplaceCartLineSnapshotInput};
use crate::entities::{cart_line_item, cart_line_item_marketplace_snapshot};
use crate::error::{CartError, CartResult};

use super::cart::helpers::{ensure_active, load_cart_in_tx, normalize_shipping_profile_slug};

#[derive(Clone)]
pub struct CartMarketplaceSnapshotService {
    db: DatabaseConnection,
}

impl CartMarketplaceSnapshotService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn bind_line_snapshot(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
        cart_line_item_id: Uuid,
        input: MarketplaceCartLineSnapshotInput,
    ) -> CartResult<CartMarketplaceLineSnapshot> {
        input
            .validate()
            .map_err(|error| CartError::Validation(error.to_string()))?;
        validate_snapshot_input(&input)?;

        if let Some(existing) = cart_line_item_marketplace_snapshot::Entity::find_by_id(
            cart_line_item_id,
        )
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
        let line = cart_line_item::Entity::find_by_id(cart_line_item_id)
            .filter(cart_line_item::Column::CartId.eq(cart_id))
            .one(&transaction)
            .await?
            .ok_or(CartError::CartLineItemNotFound(cart_line_item_id))?;
        validate_line_binding(&line, &input)?;

        let now = Utc::now().fixed_offset();
        let insert = cart_line_item_marketplace_snapshot::ActiveModel {
            cart_line_item_id: Set(cart_line_item_id),
            seller_id: Set(input.seller_id),
            listing_id: Set(input.listing_id),
            master_product_id: Set(input.master_product_id),
            master_variant_id: Set(input.master_variant_id),
            listing_terms_version: Set(input.listing_terms_version),
            unit_amount: Set(input.unit_amount),
            subtotal_amount: Set(input.subtotal_amount),
            discount_amount: Set(input.discount_amount),
            tax_amount: Set(input.tax_amount),
            total_amount: Set(input.total_amount),
            pricing_reference: Set(normalize_reference(input.pricing_reference)),
            inventory_reference: Set(normalize_reference(input.inventory_reference)),
            fulfillment_profile_slug: Set(input
                .fulfillment_profile_slug
                .as_deref()
                .map(|value| normalize_shipping_profile_slug(Some(value)))),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&transaction)
        .await;

        let model = match insert {
            Ok(model) => model,
            Err(error) => {
                transaction.rollback().await?;
                if let Some(existing) = cart_line_item_marketplace_snapshot::Entity::find_by_id(
                    cart_line_item_id,
                )
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
                return Err(error.into());
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

fn validate_snapshot_input(input: &MarketplaceCartLineSnapshotInput) -> CartResult<()> {
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
    let expected_subtotal = input
        .unit_amount
        .checked_mul(i64::from(1))
        .ok_or_else(|| CartError::Validation("marketplace subtotal overflow".to_string()))?;
    if input.subtotal_amount < expected_subtotal {
        return Err(CartError::Validation(
            "marketplace subtotal must not be lower than unit amount".to_string(),
        ));
    }
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
    Ok(())
}

fn validate_line_binding(
    line: &cart_line_item::Model,
    input: &MarketplaceCartLineSnapshotInput,
) -> CartResult<()> {
    if line.product_id != Some(input.master_product_id) {
        return Err(CartError::Validation(format!(
            "cart line {} product does not match marketplace snapshot",
            line.id
        )));
    }
    if line.variant_id != Some(input.master_variant_id) {
        return Err(CartError::Validation(format!(
            "cart line {} variant does not match marketplace snapshot",
            line.id
        )));
    }
    let expected_subtotal = input
        .unit_amount
        .checked_mul(i64::from(line.quantity))
        .ok_or_else(|| CartError::Validation("marketplace subtotal overflow".to_string()))?;
    if input.subtotal_amount != expected_subtotal {
        return Err(CartError::Validation(format!(
            "marketplace subtotal {} does not match unit amount {} x quantity {}",
            input.subtotal_amount, input.unit_amount, line.quantity
        )));
    }
    if let Some(profile) = input.fulfillment_profile_slug.as_deref() {
        let profile = normalize_shipping_profile_slug(Some(profile));
        if profile != line.shipping_profile_slug {
            return Err(CartError::Validation(format!(
                "cart line {} shipping profile does not match marketplace snapshot",
                line.id
            )));
        }
    }
    Ok(())
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
        && existing.unit_amount == input.unit_amount
        && existing.subtotal_amount == input.subtotal_amount
        && existing.discount_amount == input.discount_amount
        && existing.tax_amount == input.tax_amount
        && existing.total_amount == input.total_amount
        && existing.pricing_reference == normalize_reference(input.pricing_reference.clone())
        && existing.inventory_reference == normalize_reference(input.inventory_reference.clone())
        && existing.fulfillment_profile_slug
            == input
                .fulfillment_profile_slug
                .as_deref()
                .map(|value| normalize_shipping_profile_slug(Some(value)))
}

fn map_snapshot(
    model: cart_line_item_marketplace_snapshot::Model,
) -> CartMarketplaceLineSnapshot {
    CartMarketplaceLineSnapshot {
        cart_line_item_id: model.cart_line_item_id,
        seller_id: model.seller_id,
        listing_id: model.listing_id,
        master_product_id: model.master_product_id,
        master_variant_id: model.master_variant_id,
        listing_terms_version: model.listing_terms_version,
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