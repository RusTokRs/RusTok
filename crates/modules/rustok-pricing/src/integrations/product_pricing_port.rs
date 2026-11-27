//! Adapter implementing `rustok_product::ProductPricingPort` for the Product catalog.
//!
//! Provides transactional price authoring/cleanup and catalog price projections without
//! exposing private Pricing persistence entities or table structures to Product.

use async_trait::async_trait;
use rustok_commerce_foundation::CommerceError as PricingError;
use rustok_product::CommerceError as ProductError;
use rustok_product::StorefrontProductListPrice;
use rustok_product::ports::pricing::{
    CatalogBasePriceSnapshot, InitialPrice, PricingConnection, ProductPricingPort,
};
use sea_orm::{ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

use crate::entities::price;
use crate::services::bootstrap::{BootstrapService, catalog_base_scope};

const PRIMARY_CATALOG_CURRENCY_CODE: &str = "USD";

/// Product Admin treats the first catalog base price as its primary display/edit row.
/// Database row order is undefined, so return a deterministic order with USD first.
fn sort_catalog_base_prices(models: &mut [price::Model]) {
    models.sort_by(|left, right| {
        let left_is_secondary = !left
            .currency_code
            .eq_ignore_ascii_case(PRIMARY_CATALOG_CURRENCY_CODE);
        let right_is_secondary = !right
            .currency_code
            .eq_ignore_ascii_case(PRIMARY_CATALOG_CURRENCY_CODE);

        left.variant_id
            .cmp(&right.variant_id)
            .then_with(|| left_is_secondary.cmp(&right_is_secondary))
            .then_with(|| left.currency_code.cmp(&right.currency_code))
    });
}

fn map_catalog_scope_error(error: PricingError) -> ProductError {
    match error {
        PricingError::Database(error) => ProductError::Database(error),
        PricingError::VariantNotFound(id) => ProductError::VariantNotFound(id),
        PricingError::Validation(message) | PricingError::InvalidPrice(message) => {
            ProductError::Validation(message)
        }
        error => ProductError::Core(error.into()),
    }
}

/// Reject invalid authored money as a domain refusal before bootstrap can
/// turn it into a storage error. The bootstrap independently checks the same
/// owner rule for its direct callers.
fn validate_catalog_price_precision(prices: &[InitialPrice]) -> Result<(), ProductError> {
    for price in prices {
        crate::services::pricing::types::validate_price_precision(
            price.amount,
            price.compare_at_amount,
            &price.currency_code,
        )
        .map_err(map_catalog_scope_error)?;
    }
    Ok(())
}

fn canonicalize_initial_price_channel_scope(
    price: &mut InitialPrice,
    channel_scope: Option<(Uuid, String)>,
) {
    price.channel_id = channel_scope.as_ref().map(|(channel_id, _)| *channel_id);
    price.channel_slug = channel_scope.map(|(_, channel_slug)| channel_slug);
}

/// Adapter providing ProductPricingPort backed by the native Pricing database.
pub struct PricingProductPricingPortAdapter {
    _db: DatabaseConnection,
}

impl PricingProductPricingPortAdapter {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { _db: db }
    }
}

#[async_trait]
impl ProductPricingPort for PricingProductPricingPortAdapter {
    async fn create_initial_prices_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        prices: Vec<InitialPrice>,
    ) -> Result<(), rustok_product::CommerceError> {
        validate_catalog_price_precision(&prices)?;
        let mut prices = prices;
        let mut validated_variants = HashSet::new();
        let mut identities = HashSet::new();
        for price in &mut prices {
            if validated_variants.insert(price.variant_id) {
                crate::services::pricing::scope::load_variant_for_update_in_tx(
                    txn,
                    tenant_id,
                    price.variant_id,
                )
                .await
                .map_err(map_catalog_scope_error)?;
            }
            let channel_scope = crate::services::pricing::scope::resolve_channel_scope_in_tx(
                txn,
                tenant_id,
                price.channel_id,
                price.channel_slug.as_deref(),
            )
            .await
            .map_err(map_catalog_scope_error)?;
            canonicalize_initial_price_channel_scope(price, channel_scope);
            let identity = (
                price.variant_id,
                price.channel_id,
                price.currency_code.trim().to_ascii_uppercase(),
            );
            if !identities.insert(identity) {
                return Err(rustok_product::CommerceError::Validation(
                    "Duplicate initial price for the same variant, channel and currency".to_owned(),
                ));
            }
        }

        BootstrapService::create_initial_prices_in_tx(txn, prices)
            .await
            .map_err(rustok_product::CommerceError::from)
    }

    async fn replace_catalog_base_prices_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        variant_id: Uuid,
        prices: Vec<InitialPrice>,
    ) -> Result<(), rustok_product::CommerceError> {
        validate_catalog_price_precision(&prices)?;
        crate::services::pricing::scope::load_variant_for_update_in_tx(txn, tenant_id, variant_id)
            .await
            .map_err(map_catalog_scope_error)?;
        BootstrapService::replace_catalog_base_prices_in_tx(txn, variant_id, prices)
            .await
            .map_err(rustok_product::CommerceError::from)
    }

    async fn delete_prices_for_variants_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        variant_ids: &[Uuid],
    ) -> Result<(), rustok_product::CommerceError> {
        let mut validated_variants = HashSet::new();
        for variant_id in variant_ids {
            if validated_variants.insert(*variant_id) {
                crate::services::pricing::scope::load_variant_for_update_in_tx(
                    txn,
                    tenant_id,
                    *variant_id,
                )
                .await
                .map_err(map_catalog_scope_error)?;
            }
        }
        BootstrapService::delete_prices_for_variants_in_tx(txn, variant_ids)
            .await
            .map_err(rustok_product::CommerceError::from)
    }

    async fn load_catalog_base_prices_for_variants(
        &self,
        conn: PricingConnection<'_>,
        variant_ids: &[Uuid],
    ) -> Result<Vec<CatalogBasePriceSnapshot>, rustok_product::CommerceError> {
        if variant_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut models = match conn {
            PricingConnection::Connection(c) => {
                price::Entity::find()
                    .filter(price::Column::VariantId.is_in(variant_ids.iter().copied()))
                    .filter(catalog_base_scope())
                    .all(c)
                    .await?
            }
            PricingConnection::Transaction(tx) => {
                price::Entity::find()
                    .filter(price::Column::VariantId.is_in(variant_ids.iter().copied()))
                    .filter(catalog_base_scope())
                    .all(tx)
                    .await?
            }
        };

        sort_catalog_base_prices(&mut models);

        Ok(models
            .into_iter()
            .map(|model| CatalogBasePriceSnapshot {
                id: model.id,
                variant_id: model.variant_id,
                currency_code: model.currency_code,
                amount: model.amount,
                compare_at_amount: model.compare_at_amount,
            })
            .collect())
    }

    async fn load_storefront_list_prices(
        &self,
        conn: PricingConnection<'_>,
        variant_to_product: &HashMap<Uuid, Uuid>,
        variant_ids: &[Uuid],
        currency_code: Option<&str>,
        public_channel_slug: Option<&str>,
    ) -> Result<HashMap<Uuid, StorefrontProductListPrice>, rustok_product::CommerceError> {
        if variant_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let prices = match conn {
            PricingConnection::Connection(c) => {
                price::Entity::find()
                    .filter(price::Column::VariantId.is_in(variant_ids.iter().copied()))
                    .all(c)
                    .await?
            }
            PricingConnection::Transaction(tx) => {
                price::Entity::find()
                    .filter(price::Column::VariantId.is_in(variant_ids.iter().copied()))
                    .all(tx)
                    .await?
            }
        };

        let mut candidates_by_product = HashMap::<Uuid, Vec<&price::Model>>::new();
        for price in &prices {
            if !is_storefront_list_price_candidate(price) {
                continue;
            }
            let Some(product_id) = variant_to_product.get(&price.variant_id) else {
                continue;
            };
            candidates_by_product
                .entry(*product_id)
                .or_default()
                .push(price);
        }

        let mut result = HashMap::new();
        for (product_id, candidates) in candidates_by_product {
            if let Some(price) =
                select_storefront_list_price(candidates, currency_code, public_channel_slug)
            {
                let on_sale = price
                    .compare_at_amount
                    .is_some_and(|compare_at| compare_at > price.amount);
                result.insert(
                    product_id,
                    StorefrontProductListPrice {
                        currency_code: price.currency_code.to_ascii_uppercase(),
                        amount: price.amount,
                        compare_at_amount: price.compare_at_amount,
                        on_sale,
                    },
                );
            }
        }

        Ok(result)
    }
}

fn is_storefront_list_price_candidate(price: &price::Model) -> bool {
    price.price_list_id.is_none()
        && price.region_id.is_none()
        && price.min_quantity.is_none()
        && price.max_quantity.is_none()
}

fn select_storefront_list_price<'a>(
    candidates: Vec<&'a price::Model>,
    currency_code: Option<&str>,
    public_channel_slug: Option<&str>,
) -> Option<&'a price::Model> {
    let candidates = candidates
        .into_iter()
        .filter(|price| {
            currency_code.is_none_or(|code| price.currency_code.eq_ignore_ascii_case(code))
        })
        .collect::<Vec<_>>();
    let candidates = select_storefront_list_price_candidates(candidates, public_channel_slug);
    if candidates.is_empty() {
        return None;
    }

    // Currency is a comparison dimension, not display metadata. Without an
    // explicit currency, a single "from" price is meaningful only when every
    // eligible row uses the same currency.
    if currency_code.is_none() {
        let currencies = candidates
            .iter()
            .map(|price| price.currency_code.to_ascii_uppercase())
            .collect::<HashSet<_>>();
        if currencies.len() != 1 {
            return None;
        }
    }

    candidates.into_iter().min_by(|left, right| {
        left.amount
            .cmp(&right.amount)
            .then_with(|| left.variant_id.cmp(&right.variant_id))
            .then_with(|| left.id.cmp(&right.id))
    })
}

fn select_storefront_list_price_candidates<'a>(
    candidates: Vec<&'a price::Model>,
    public_channel_slug: Option<&str>,
) -> Vec<&'a price::Model> {
    let matches_channel = |price: &price::Model| {
        public_channel_slug.is_some_and(|channel_slug| {
            price
                .channel_slug
                .as_deref()
                .is_some_and(|slug| slug.eq_ignore_ascii_case(channel_slug))
        })
    };
    // Channel precedence belongs to an individual SKU and currency, not the
    // entire product. A scoped price must not hide another SKU's base price
    // or a base price in a different currency.
    let scoped_keys: HashSet<_> = candidates
        .iter()
        .filter(|price| matches_channel(price))
        .map(|price| (price.variant_id, price.currency_code.to_ascii_uppercase()))
        .collect();

    candidates
        .into_iter()
        .filter(|price| {
            matches_channel(price)
                || (price.channel_id.is_none()
                    && price.channel_slug.is_none()
                    && !scoped_keys
                        .contains(&(price.variant_id, price.currency_code.to_ascii_uppercase())))
        })
        .collect()
}

/// Creates an in-process adapter implementing `ProductPricingPort`.
pub fn in_process_product_pricing_port(db: DatabaseConnection) -> Arc<dyn ProductPricingPort> {
    Arc::new(PricingProductPricingPortAdapter::new(db))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn candidate(
        id: u128,
        channel_id: Option<Uuid>,
        channel_slug: Option<&str>,
        region_id: Option<Uuid>,
        min_quantity: Option<i32>,
        max_quantity: Option<i32>,
        amount: i64,
    ) -> price::Model {
        price::Model {
            id: Uuid::from_u128(id),
            variant_id: Uuid::from_u128(100),
            price_list_id: None,
            channel_id,
            channel_slug: channel_slug.map(str::to_owned),
            currency_code: "USD".to_owned(),
            region_id,
            amount: Decimal::new(amount, 0),
            compare_at_amount: None,
            legacy_amount: None,
            legacy_compare_at_amount: None,
            min_quantity,
            max_quantity,
        }
    }

    fn initial_price(variant_id: Uuid) -> InitialPrice {
        InitialPrice {
            variant_id,
            channel_id: None,
            channel_slug: None,
            currency_code: "USD".to_owned(),
            amount: Decimal::new(1000, 2),
            compare_at_amount: None,
        }
    }

    #[test]
    fn channel_id_only_initial_price_is_canonicalized_for_storefront_selection() {
        let channel_id = Uuid::from_u128(2);
        let mut price = initial_price(Uuid::from_u128(100));
        price.channel_id = Some(channel_id);

        canonicalize_initial_price_channel_scope(&mut price, Some((channel_id, "web".to_owned())));

        assert_eq!(price.channel_id, Some(channel_id));
        assert_eq!(price.channel_slug.as_deref(), Some("web"));

        let model = candidate(
            1,
            price.channel_id,
            price.channel_slug.as_deref(),
            None,
            None,
            None,
            10,
        );
        let selected = select_storefront_list_price_candidates(vec![&model], Some("web"));
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id, model.id);
    }

    #[test]
    fn channel_slug_only_initial_price_is_canonicalized_with_channel_id() {
        let channel_id = Uuid::from_u128(2);
        let mut price = initial_price(Uuid::from_u128(100));
        price.channel_slug = Some(" WEB ".to_owned());

        canonicalize_initial_price_channel_scope(&mut price, Some((channel_id, "web".to_owned())));

        assert_eq!(price.channel_id, Some(channel_id));
        assert_eq!(price.channel_slug.as_deref(), Some("web"));
    }

    #[test]
    fn unscoped_initial_price_keeps_both_channel_aliases_empty() {
        let mut price = initial_price(Uuid::from_u128(100));
        canonicalize_initial_price_channel_scope(&mut price, None);

        assert_eq!(price.channel_id, None);
        assert_eq!(price.channel_slug, None);
    }

    #[test]
    fn catalog_price_storage_overflow_is_a_product_validation_refusal() {
        let mut price = InitialPrice {
            variant_id: Uuid::from_u128(100),
            channel_id: None,
            channel_slug: None,
            currency_code: "USD".to_owned(),
            amount: Decimal::new(10_000_000_000, 0),
            compare_at_amount: None,
        };
        assert!(matches!(
            validate_catalog_price_precision(std::slice::from_ref(&price)),
            Err(ProductError::Validation(_))
        ));
        price.amount = Decimal::ONE;
        price.compare_at_amount = Some(Decimal::new(10_000_000_000, 0));
        assert!(matches!(
            validate_catalog_price_precision(std::slice::from_ref(&price)),
            Err(ProductError::Validation(_))
        ));
        price.compare_at_amount = None;
        assert!(validate_catalog_price_precision(&[price]).is_ok());
    }

    #[test]
    fn catalog_base_price_order_keeps_usd_primary_and_sorts_remaining_currencies() {
        let usd = candidate(1, None, None, None, None, None, 10);
        let eur = price::Model {
            currency_code: "EUR".to_owned(),
            ..candidate(2, None, None, None, None, None, 9)
        };
        let gbp = price::Model {
            currency_code: "GBP".to_owned(),
            ..candidate(3, None, None, None, None, None, 8)
        };
        let mut models = vec![gbp, eur, usd];

        sort_catalog_base_prices(&mut models);

        assert_eq!(
            models
                .iter()
                .map(|model| model.currency_code.as_str())
                .collect::<Vec<_>>(),
            vec!["USD", "EUR", "GBP"]
        );
    }

    #[test]
    fn channel_id_only_price_is_not_used_as_channel_neutral_fallback() {
        let channel_only = candidate(1, Some(Uuid::from_u128(2)), None, None, None, None, 5);
        let neutral = candidate(3, None, None, None, None, None, 10);

        let selected = select_storefront_list_price_candidates(vec![&channel_only, &neutral], None);

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id, neutral.id);
    }

    #[test]
    fn matching_channel_price_precedes_neutral_and_other_channel_prices() {
        let matching = candidate(
            1,
            Some(Uuid::from_u128(2)),
            Some("web"),
            None,
            None,
            None,
            12,
        );
        let other = candidate(
            2,
            Some(Uuid::from_u128(3)),
            Some("mobile"),
            None,
            None,
            None,
            5,
        );
        let neutral = candidate(3, None, None, None, None, None, 10);

        let selected =
            select_storefront_list_price_candidates(vec![&matching, &other, &neutral], Some("web"));

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id, matching.id);
    }

    #[test]
    fn channel_precedence_is_independent_for_each_sku() {
        let scoped = candidate(
            1,
            Some(Uuid::from_u128(2)),
            Some("web"),
            None,
            None,
            None,
            12,
        );
        let shadowed = candidate(2, None, None, None, None, None, 1);
        let other_sku = price::Model {
            variant_id: Uuid::from_u128(101),
            ..candidate(3, None, None, None, None, None, 8)
        };
        let selected = select_storefront_list_price(
            vec![&scoped, &shadowed, &other_sku],
            Some("USD"),
            Some("web"),
        )
        .expect("other SKU has an eligible base price");
        assert_eq!(selected.id, other_sku.id);
    }

    #[test]
    fn channel_precedence_does_not_hide_another_currency() {
        let scoped = candidate(
            1,
            Some(Uuid::from_u128(2)),
            Some("web"),
            None,
            None,
            None,
            12,
        );
        let eur = price::Model {
            currency_code: "EUR".to_owned(),
            ..candidate(2, None, None, None, None, None, 8)
        };
        assert!(select_storefront_list_price(vec![&scoped, &eur], None, Some("web")).is_none());
        assert_eq!(
            select_storefront_list_price(vec![&scoped, &eur], Some("EUR"), Some("web"))
                .expect("EUR base price")
                .id,
            eur.id,
        );
    }

    #[test]
    fn equal_minimum_prices_have_deterministic_sale_metadata() {
        let first = candidate(1, None, None, None, None, None, 10);
        let second = price::Model {
            compare_at_amount: Some(Decimal::new(20, 0)),
            ..candidate(2, None, None, None, None, None, 10)
        };
        for rows in [vec![&first, &second], vec![&second, &first]] {
            assert_eq!(
                select_storefront_list_price(rows, Some("USD"), None)
                    .expect("equal minimum")
                    .id,
                first.id,
            );
        }
    }

    #[test]
    fn missing_channel_price_does_not_fall_back_to_another_channel() {
        let other_channel = candidate(
            1,
            Some(Uuid::from_u128(3)),
            Some("mobile"),
            None,
            None,
            None,
            5,
        );
        let channel_id_only = candidate(2, Some(Uuid::from_u128(4)), None, None, None, None, 6);

        let selected = select_storefront_list_price_candidates(
            vec![&other_channel, &channel_id_only],
            Some("web"),
        );

        assert!(selected.is_empty());
    }

    #[test]
    fn missing_channel_price_uses_only_a_channel_neutral_fallback() {
        let other_channel = candidate(
            1,
            Some(Uuid::from_u128(3)),
            Some("mobile"),
            None,
            None,
            None,
            5,
        );
        let neutral = candidate(2, None, None, None, None, None, 10);

        let selected =
            select_storefront_list_price_candidates(vec![&other_channel, &neutral], Some("web"));

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id, neutral.id);
    }

    #[test]
    fn channel_scoped_prices_are_not_selected_without_channel_context() {
        let channel_only = candidate(
            1,
            Some(Uuid::from_u128(2)),
            Some("web"),
            None,
            None,
            None,
            5,
        );

        let selected = select_storefront_list_price_candidates(vec![&channel_only], None);

        assert!(selected.is_empty());
    }

    #[test]
    fn ambiguous_currency_is_not_compared_without_an_explicit_currency() {
        let usd = candidate(1, None, None, None, None, None, 5);
        let eur = price::Model {
            currency_code: "EUR".to_owned(),
            ..candidate(2, None, None, None, None, None, 4)
        };

        assert!(
            select_storefront_list_price(vec![&usd, &eur], None, None).is_none(),
            "different currencies must not be ordered by their numeric amount"
        );
        assert_eq!(
            select_storefront_list_price(vec![&usd, &eur], Some("USD"), None)
                .expect("requested currency")
                .id,
            usd.id
        );
    }

    #[test]
    fn one_currency_can_be_minimized_without_an_explicit_currency() {
        let higher = candidate(1, None, None, None, None, None, 12);
        let lower = candidate(2, None, None, None, None, None, 8);

        let selected = select_storefront_list_price(vec![&higher, &lower], None, None)
            .expect("one-currency list price");

        assert_eq!(selected.id, lower.id);
    }

    #[test]
    fn region_and_quantity_tiers_are_not_list_price_candidates_without_context() {
        let base = candidate(1, None, None, None, None, None, 10);
        let regional = candidate(2, None, None, Some(Uuid::from_u128(3)), None, None, 8);
        let min_tier = candidate(4, None, None, None, Some(2), None, 7);
        let max_tier = candidate(5, None, None, None, None, Some(4), 6);

        assert!(is_storefront_list_price_candidate(&base));
        assert!(!is_storefront_list_price_candidate(&regional));
        assert!(!is_storefront_list_price_candidate(&min_tier));
        assert!(!is_storefront_list_price_candidate(&max_tier));
    }
}
