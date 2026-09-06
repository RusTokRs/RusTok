use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

pub mod entities;

use entities::price;

#[derive(Clone, Debug)]
pub struct InitialPrice {
    pub variant_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub currency_code: String,
    pub amount: Decimal,
    pub compare_at_amount: Option<Decimal>,
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

        let models = prices
            .into_iter()
            .map(|price| price::ActiveModel {
                id: Set(rustok_core::generate_id()),
                variant_id: Set(price.variant_id),
                price_list_id: Set(None),
                channel_id: Set(price.channel_id),
                channel_slug: Set(price.channel_slug),
                currency_code: Set(price.currency_code),
                region_id: Set(None),
                amount: Set(price.amount),
                compare_at_amount: Set(price.compare_at_amount),
                legacy_amount: Set(decimal_to_cents(price.amount)),
                legacy_compare_at_amount: Set(price.compare_at_amount.and_then(decimal_to_cents)),
                min_quantity: Set(None),
                max_quantity: Set(None),
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

        price::Entity::find()
            .filter(price::Column::VariantId.is_in(variant_ids.iter().copied()))
            .all(conn)
            .await
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

        price::Entity::delete_many()
            .filter(price::Column::VariantId.is_in(variant_ids.iter().copied()))
            .exec(conn)
            .await?;
        Ok(())
    }
}

fn decimal_to_cents(amount: Decimal) -> Option<i64> {
    (amount * Decimal::from(100)).round().to_i64()
}
