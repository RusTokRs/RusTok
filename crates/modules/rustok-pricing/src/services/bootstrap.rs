//! Pricing-owned transaction-aware operations for catalog variant lifecycle writes.

use rust_decimal::Decimal;
use sea_orm::{ColumnTrait, Condition, ConnectionTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

use crate::entities::price;
pub use rustok_product::ports::pricing::InitialPrice;

/// Exponent of the retained two-decimal pricing mirror columns.
pub const LEGACY_AMOUNT_EXPONENT: u8 = 2;

/// Converts a major-unit amount into the two-decimal units retained by the
/// `legacy_amount` / `legacy_compare_at_amount` mirror columns.
pub fn legacy_amount_units(amount: Decimal) -> Option<i64> {
    rustok_core::money::to_fixed_point_units(amount, LEGACY_AMOUNT_EXPONENT).ok()
}

/// Canonical unscoped inspection/bootstrap predicate.
pub fn catalog_base_scope() -> Condition {
    Condition::all()
        .add(price::Column::PriceListId.is_null())
        .add(price::Column::ChannelId.is_null())
        .add(price::Column::ChannelSlug.is_null())
        .add(price::Column::RegionId.is_null())
        .add(price::Column::MinQuantity.is_null())
        .add(price::Column::MaxQuantity.is_null())
}

/// Pricing-owned transaction-aware operations required by Product lifecycle writes.
pub struct BootstrapService;

impl BootstrapService {
    pub async fn create_initial_prices_in_tx<C>(
        conn: &C,
        prices: Vec<InitialPrice>,
    ) -> Result<(), sea_orm::DbErr>
    where
        C: ConnectionTrait,
    {
        if prices.is_empty() {
            return Ok(());
        }

        for price in &prices {
            if price.variant_id.is_nil() {
                return Err(sea_orm::DbErr::Custom(
                    "variant_id must not be a nil UUID".into(),
                ));
            }
            if price.channel_id == Some(Uuid::nil()) {
                return Err(sea_orm::DbErr::Custom(
                    "channel_id must not be a nil UUID".into(),
                ));
            }
            if price.amount < Decimal::ZERO {
                return Err(sea_orm::DbErr::Custom("amount must not be negative".into()));
            }
            if let Some(compare_at) = price.compare_at_amount
                && compare_at < Decimal::ZERO
            {
                return Err(sea_orm::DbErr::Custom(
                    "compare_at_amount must not be negative".into(),
                ));
            }
            let trimmed_currency = price.currency_code.trim();
            if trimmed_currency.len() != 3
                || !trimmed_currency.chars().all(|ch| ch.is_ascii_alphabetic())
            {
                return Err(sea_orm::DbErr::Custom(
                    "currency_code must be a 3-letter ASCII alphabetic code".into(),
                ));
            }
        }

        let models = prices
            .into_iter()
            .map(|price| {
                let channel_slug = price.channel_slug.and_then(|slug| {
                    let trimmed = slug.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_ascii_lowercase())
                    }
                });
                price::ActiveModel {
                    id: Set(rustok_core::generate_id()),
                    variant_id: Set(price.variant_id),
                    price_list_id: Set(None),
                    channel_id: Set(price.channel_id),
                    channel_slug: Set(channel_slug),
                    currency_code: Set(price.currency_code.trim().to_ascii_uppercase()),
                    region_id: Set(None),
                    amount: Set(price.amount),
                    compare_at_amount: Set(price.compare_at_amount),
                    legacy_amount: Set(legacy_amount_units(price.amount)),
                    legacy_compare_at_amount: Set(price
                        .compare_at_amount
                        .and_then(legacy_amount_units)),
                    min_quantity: Set(None),
                    max_quantity: Set(None),
                }
            })
            .collect::<Vec<_>>();
        price::Entity::insert_many(models).exec(conn).await?;
        Ok(())
    }

    pub async fn load_prices_for_variants<C>(
        conn: &C,
        variant_ids: &[Uuid],
    ) -> Result<Vec<price::Model>, sea_orm::DbErr>
    where
        C: ConnectionTrait,
    {
        if variant_ids.is_empty() {
            return Ok(Vec::new());
        }

        if variant_ids.iter().any(|id| id.is_nil()) {
            return Err(sea_orm::DbErr::Custom(
                "variant_ids must not contain a nil UUID".into(),
            ));
        }

        price::Entity::find()
            .filter(price::Column::VariantId.is_in(variant_ids.iter().copied()))
            .all(conn)
            .await
    }

    /// Catalog snapshots intentionally omit contextual prices: the catalog DTO has
    /// no context identity and cannot safely round-trip those rows.
    pub async fn load_catalog_base_prices_for_variants<C>(
        conn: &C,
        variant_ids: &[Uuid],
    ) -> Result<Vec<price::Model>, sea_orm::DbErr>
    where
        C: ConnectionTrait,
    {
        if variant_ids.iter().any(|id| id.is_nil()) {
            return Err(sea_orm::DbErr::Custom(
                "variant_ids must not contain a nil UUID".into(),
            ));
        }
        if variant_ids.is_empty() {
            return Ok(Vec::new());
        }
        price::Entity::find()
            .filter(price::Column::VariantId.is_in(variant_ids.iter().copied()))
            .filter(catalog_base_scope())
            .all(conn)
            .await
    }

    /// Replace only the catalog's unscoped base prices.
    pub async fn replace_catalog_base_prices_in_tx<C>(
        conn: &C,
        variant_id: Uuid,
        prices: Vec<InitialPrice>,
    ) -> Result<(), sea_orm::DbErr>
    where
        C: ConnectionTrait,
    {
        if variant_id.is_nil()
            || prices.iter().any(|price| {
                price.variant_id != variant_id
                    || price.channel_id.is_some()
                    || price.channel_slug.is_some()
                    || price.amount < Decimal::ZERO
                    || price
                        .compare_at_amount
                        .is_some_and(|amount| amount < Decimal::ZERO)
            })
        {
            return Err(sea_orm::DbErr::Custom(
                "Catalog price replacement accepts only this variant's unscoped base prices; use Pricing for contextual prices".into(),
            ));
        }
        price::Entity::delete_many()
            .filter(price::Column::VariantId.eq(variant_id))
            .filter(catalog_base_scope())
            .exec(conn)
            .await?;
        Self::create_initial_prices_in_tx(conn, prices).await
    }

    pub async fn delete_prices_for_variants_in_tx<C>(
        conn: &C,
        variant_ids: &[Uuid],
    ) -> Result<(), sea_orm::DbErr>
    where
        C: ConnectionTrait,
    {
        if variant_ids.is_empty() {
            return Ok(());
        }

        if variant_ids.iter().any(|id| id.is_nil()) {
            return Err(sea_orm::DbErr::Custom(
                "variant_ids must not contain a nil UUID".into(),
            ));
        }

        price::Entity::delete_many()
            .filter(price::Column::VariantId.is_in(variant_ids.iter().copied()))
            .exec(conn)
            .await?;
        Ok(())
    }
}
