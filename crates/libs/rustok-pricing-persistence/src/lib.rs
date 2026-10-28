//! # RusToK Pricing Persistence
//!
//! Pricing-owned ORM entities and transaction-aware persistence bootstrap.
//! Holds SeaORM entities for prices, price lists, and multilingual translations.

use rust_decimal::Decimal;
use sea_orm::{ColumnTrait, Condition, ConnectionTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

pub mod entities;

#[cfg(test)]
mod contract_tests;

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

    /// Replace only the catalog's unscoped base prices. Price lists, channels,
    /// regions and quantity tiers belong to Pricing and survive this operation.
    /// The caller must hold the Product/Variant owner lock in its transaction.
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

fn catalog_base_scope() -> Condition {
    Condition::all()
        .add(price::Column::PriceListId.is_null())
        .add(price::Column::ChannelId.is_null())
        .add(price::Column::ChannelSlug.is_null())
        .add(price::Column::RegionId.is_null())
        .add(price::Column::MinQuantity.is_null())
        .add(price::Column::MaxQuantity.is_null())
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

    async fn test_price_db() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let backend = db.get_database_backend();
        let table = sea_orm::Schema::new(backend).create_table_from_entity(price::Entity);
        db.execute(&table).await.unwrap();
        db
    }

    fn initial_price(variant_id: Uuid, amount: i64) -> InitialPrice {
        InitialPrice {
            variant_id,
            channel_id: None,
            channel_slug: None,
            currency_code: "USD".into(),
            amount: Decimal::new(amount, 2),
            compare_at_amount: None,
        }
    }

    #[tokio::test]
    async fn catalog_base_replace_and_clear_preserve_every_context_and_other_variant() {
        use sea_orm::{ActiveModelTrait, TransactionTrait};

        let db = test_price_db().await;
        let variant_id = rustok_core::generate_id();
        let other_variant_id = rustok_core::generate_id();
        BootstrapService::create_initial_prices_in_tx(
            &db,
            vec![
                initial_price(variant_id, 1000),
                initial_price(other_variant_id, 500),
            ],
        )
        .await
        .unwrap();
        let base = BootstrapService::load_catalog_base_prices_for_variants(&db, &[variant_id])
            .await
            .unwrap()
            .remove(0);
        let mut contextual = Vec::new();
        for context in 0..6 {
            let mut model: price::ActiveModel = base.clone().into();
            model.id = Set(rustok_core::generate_id());
            match context {
                0 => model.price_list_id = Set(Some(rustok_core::generate_id())),
                1 => model.channel_id = Set(Some(rustok_core::generate_id())),
                2 => model.channel_slug = Set(Some("web".into())),
                3 => model.region_id = Set(Some(rustok_core::generate_id())),
                4 => model.min_quantity = Set(Some(1)),
                _ => model.max_quantity = Set(Some(10)),
            }
            contextual.push(model.insert(&db).await.unwrap());
        }
        let txn = db.begin().await.unwrap();
        BootstrapService::replace_catalog_base_prices_in_tx(
            &txn,
            variant_id,
            vec![initial_price(variant_id, 1250)],
        )
        .await
        .unwrap();
        txn.commit().await.unwrap();
        let base_prices =
            BootstrapService::load_catalog_base_prices_for_variants(&db, &[variant_id])
                .await
                .unwrap();
        assert_eq!(base_prices.len(), 1);
        assert_eq!(base_prices[0].amount, Decimal::new(1250, 2));

        let txn = db.begin().await.unwrap();
        BootstrapService::replace_catalog_base_prices_in_tx(&txn, variant_id, vec![])
            .await
            .unwrap();
        txn.commit().await.unwrap();
        assert!(
            BootstrapService::load_catalog_base_prices_for_variants(&db, &[variant_id])
                .await
                .unwrap()
                .is_empty()
        );
        let remaining = BootstrapService::load_prices_for_variants(&db, &[variant_id])
            .await
            .unwrap();
        assert_eq!(remaining.len(), contextual.len());
        for expected in contextual {
            assert!(
                remaining.contains(&expected),
                "contextual price was changed or deleted"
            );
        }
        let other =
            BootstrapService::load_catalog_base_prices_for_variants(&db, &[other_variant_id])
                .await
                .unwrap();
        assert_eq!(other.len(), 1);
        assert_eq!(other[0].amount, Decimal::new(500, 2));
    }

    #[tokio::test]
    async fn catalog_replace_refuses_context_and_foreign_variant_before_deleting() {
        let db = test_price_db().await;
        let variant_id = rustok_core::generate_id();
        BootstrapService::create_initial_prices_in_tx(&db, vec![initial_price(variant_id, 1000)])
            .await
            .unwrap();
        for invalid in 0..4 {
            let mut input = initial_price(variant_id, 1250);
            match invalid {
                0 => input.channel_slug = Some("web".into()),
                1 => input.channel_id = Some(rustok_core::generate_id()),
                2 => input.variant_id = rustok_core::generate_id(),
                _ => input.amount = Decimal::new(-1, 0),
            }
            assert!(
                BootstrapService::replace_catalog_base_prices_in_tx(&db, variant_id, vec![input])
                    .await
                    .is_err()
            );
            let prices =
                BootstrapService::load_catalog_base_prices_for_variants(&db, &[variant_id])
                    .await
                    .unwrap();
            assert_eq!(prices.len(), 1);
            assert_eq!(prices[0].amount, Decimal::new(1000, 2));
        }
    }

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

    #[tokio::test]
    async fn test_bootstrap_service_negative_compare_at_amount_guard() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let prices = vec![InitialPrice {
            variant_id: Uuid::new_v4(),
            channel_id: None,
            channel_slug: None,
            currency_code: "USD".to_string(),
            amount: Decimal::new(1000, 2),
            compare_at_amount: Some(Decimal::new(-50, 2)),
        }];
        let err = BootstrapService::create_initial_prices_in_tx(&db, prices)
            .await
            .expect_err("negative compare_at_amount must be rejected");
        assert!(
            err.to_string()
                .contains("compare_at_amount must not be negative")
        );
    }

    #[tokio::test]
    async fn test_bootstrap_service_invalid_currency_code_guard() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        for invalid in ["", "US", "USDD", "U$D", "123", "   "] {
            let prices = vec![InitialPrice {
                variant_id: Uuid::new_v4(),
                channel_id: None,
                channel_slug: None,
                currency_code: invalid.to_string(),
                amount: Decimal::new(1000, 2),
                compare_at_amount: None,
            }];
            let err = BootstrapService::create_initial_prices_in_tx(&db, prices)
                .await
                .unwrap_err();
            assert!(
                err.to_string()
                    .contains("currency_code must be a 3-letter ASCII alphabetic code"),
                "expected rejection for '{invalid}', got: {err}"
            );
        }
    }

    #[tokio::test]
    async fn test_bootstrap_service_normalizes_currency_and_slug() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let schema = sea_orm::Schema::new(sea_orm::DbBackend::Sqlite);
        let stmt = schema.create_table_from_entity(price::Entity);
        db.execute(&stmt).await.unwrap();

        let variant_id = Uuid::new_v4();
        let prices = vec![InitialPrice {
            variant_id,
            channel_id: None,
            channel_slug: Some("  DEFAULT-STORE  ".to_string()),
            currency_code: " rub ".to_string(),
            amount: Decimal::new(1250, 2),
            compare_at_amount: Some(Decimal::new(1500, 2)),
        }];

        BootstrapService::create_initial_prices_in_tx(&db, prices)
            .await
            .expect("should successfully insert initial prices");

        let loaded = BootstrapService::load_prices_for_variants(&db, &[variant_id])
            .await
            .expect("should load prices");

        assert_eq!(loaded.len(), 1);
        let p = &loaded[0];
        assert_eq!(p.currency_code, "RUB");
        assert_eq!(p.channel_slug, Some("default-store".to_string()));
        assert_eq!(p.amount, Decimal::new(1250, 2));
        assert_eq!(p.compare_at_amount, Some(Decimal::new(1500, 2)));
        assert_eq!(p.legacy_amount, Some(1250));
        assert_eq!(p.legacy_compare_at_amount, Some(1500));
    }

    #[tokio::test]
    async fn test_bootstrap_service_empty_slices_no_op() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        BootstrapService::create_initial_prices_in_tx(&db, vec![])
            .await
            .unwrap();
        let loaded = BootstrapService::load_prices_for_variants(&db, &[])
            .await
            .unwrap();
        assert!(loaded.is_empty());
        BootstrapService::delete_prices_for_variants_in_tx(&db, &[])
            .await
            .unwrap();
    }
}
