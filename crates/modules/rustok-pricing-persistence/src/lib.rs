use rust_decimal::Decimal;
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
                legacy_amount: Set(legacy_amount_units(price.amount)),
                legacy_compare_at_amount: Set(price
                    .compare_at_amount
                    .and_then(legacy_amount_units)),
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

/// Exponent of the retained two-decimal pricing mirror columns.
pub const LEGACY_AMOUNT_EXPONENT: u8 = 2;

/// Converts a major-unit amount into the two-decimal units retained by the
/// `legacy_amount` / `legacy_compare_at_amount` mirror columns.
///
/// The conversion itself belongs to the canonical money owner (`rustok_core::money`, see
/// ECOM-MONEY-02 in `docs/audits/ecommerce-deep-review-2026-10-07.md`); this helper only names the
/// pricing-owned column precision and keeps the mirror `NULL` when the value cannot be represented
/// in two decimals.
pub fn legacy_amount_units(amount: Decimal) -> Option<i64> {
    rustok_core::money::to_fixed_point_units(amount, LEGACY_AMOUNT_EXPONENT).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Database;

    #[test]
    fn test_legacy_amount_units() {
        assert_eq!(legacy_amount_units(Decimal::new(1050, 2)), Some(1050));
        assert_eq!(legacy_amount_units(Decimal::new(0, 2)), Some(0));
        assert_eq!(legacy_amount_units(Decimal::new(999999, 2)), Some(999999));
        assert_eq!(legacy_amount_units(Decimal::new(-525, 2)), Some(-525));
        // Rounds to 2 decimal places: 10.555 -> 10.56 -> 1056
        assert_eq!(legacy_amount_units(Decimal::new(10555, 3)), Some(1056));
        // Overflows i64 representation
        assert_eq!(legacy_amount_units(Decimal::MAX), None);
    }

    #[tokio::test]
    async fn test_bootstrap_service_nil_variant_guard() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let prices = vec![InitialPrice {
            variant_id: Uuid::nil(),
            channel_id: None,
            channel_slug: None,
            currency_code: "USD".to_string(),
            amount: Decimal::new(1000, 2),
            compare_at_amount: None,
        }];
        let err = BootstrapService::create_initial_prices_in_tx(&db, prices)
            .await
            .expect_err("nil variant_id must be rejected");
        assert!(
            err.to_string()
                .contains("variant_id must not be a nil UUID")
        );
    }

    #[tokio::test]
    async fn test_bootstrap_service_nil_channel_guard() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let prices = vec![InitialPrice {
            variant_id: Uuid::new_v4(),
            channel_id: Some(Uuid::nil()),
            channel_slug: None,
            currency_code: "USD".to_string(),
            amount: Decimal::new(1000, 2),
            compare_at_amount: None,
        }];
        let err = BootstrapService::create_initial_prices_in_tx(&db, prices)
            .await
            .expect_err("nil channel_id must be rejected");
        assert!(
            err.to_string()
                .contains("channel_id must not be a nil UUID")
        );
    }

    #[tokio::test]
    async fn test_bootstrap_service_negative_amount_guard() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let prices = vec![InitialPrice {
            variant_id: Uuid::new_v4(),
            channel_id: None,
            channel_slug: None,
            currency_code: "USD".to_string(),
            amount: Decimal::new(-100, 2),
            compare_at_amount: None,
        }];
        let err = BootstrapService::create_initial_prices_in_tx(&db, prices)
            .await
            .expect_err("negative amount must be rejected");
        assert!(err.to_string().contains("amount must not be negative"));
    }

    #[tokio::test]
    async fn test_bootstrap_service_load_and_delete_nil_variant_guards() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let err = BootstrapService::load_prices_for_variants(&db, &[Uuid::nil()])
            .await
            .expect_err("nil variant_id must be rejected");
        assert!(
            err.to_string()
                .contains("variant_ids must not contain a nil UUID")
        );

        let err =
            BootstrapService::delete_prices_for_variants_in_tx(&db, &[Uuid::new_v4(), Uuid::nil()])
                .await
                .expect_err("nil variant_id in slice must be rejected");
        assert!(
            err.to_string()
                .contains("variant_ids must not contain a nil UUID")
        );
    }
}
