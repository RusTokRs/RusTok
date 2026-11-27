//! Adapter implementing `rustok_product::ProductPricingPort` for the Product catalog.
//!
//! Provides transactional price authoring/cleanup and catalog price projections without
//! exposing private Pricing persistence entities or table structures to Product.
use rust_decimal::Decimal;
use rustok_core::events::ValidateEvent;
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;

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

fn price_event_amount(amount: Decimal, currency: &str) -> Result<i64, ProductError> {
    crate::services::pricing::types::validate_price_precision(amount, None, currency)
        .map_err(map_catalog_scope_error)?;
    rustok_core::money::to_minor_units_exact(amount, currency).map_err(|_| {
        ProductError::Validation(
            "Price amount cannot be represented exactly in currency minor units".to_owned(),
        )
    })
}

fn price_event_predecessor(saved: &price::Model) -> Result<i64, ProductError> {
    let currency = saved.currency_code.trim().to_ascii_uppercase();
    if saved.currency_code != currency {
        return Err(ProductError::Conflict(
            "Stored base price currency is not canonical".to_owned(),
        ));
    }
    price_event_amount(saved.amount, &currency)
}

fn price_event_successor(
    saved: &price::Model,
    authored: &InitialPrice,
) -> Result<i64, ProductError> {
    let currency = authored.currency_code.trim().to_ascii_uppercase();
    if saved.currency_code != currency
        || saved.amount != authored.amount
        || saved.compare_at_amount != authored.compare_at_amount
    {
        return Err(ProductError::Validation(
            "Persisted price differs from the authored price".to_owned(),
        ));
    }
    validate_catalog_price_precision(std::slice::from_ref(authored))?;
    price_event_amount(saved.amount, &saved.currency_code)
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
    event_bus: TransactionalEventBus,
}

impl PricingProductPricingPortAdapter {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { _db: db, event_bus }
    }

    async fn publish_price_updated_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        product_id: Uuid,
        currency: String,
        old_amount: Option<i64>,
        new_amount: i64,
    ) -> Result<(), ProductError> {
        let event = DomainEvent::PriceUpdated {
            variant_id,
            product_id,
            currency,
            old_amount,
            new_amount,
        };
        event
            .validate()
            .map_err(|error| ProductError::Validation(format!("Invalid price event: {error}")))?;
        let _event_id = self
            .event_bus
            .publish_in_tx_with_envelope_id(txn, tenant_id, Some(actor_id), event)
            .await?;
        Ok(())
    }
}

#[async_trait]
impl ProductPricingPort for PricingProductPricingPortAdapter {
    async fn create_initial_prices_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Uuid,
        mut prices: Vec<InitialPrice>,
    ) -> Result<(), ProductError> {
        let mut variant_products = HashMap::new();
        let mut identities = HashSet::new();
        for price in &mut prices {
            if !variant_products.contains_key(&price.variant_id) {
                let variant = crate::services::pricing::scope::load_variant_for_update_in_tx(
                    txn,
                    tenant_id,
                    price.variant_id,
                )
                .await
                .map_err(map_catalog_scope_error)?;
                variant_products.insert(price.variant_id, variant.product_id);
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
                return Err(ProductError::Validation(
                    "Duplicate initial price for the same variant, channel and currency".to_owned(),
                ));
            }
        }

        validate_catalog_price_precision(&prices)?;
        let authored_prices = prices.clone();
        BootstrapService::create_initial_prices_in_tx(txn, prices)
            .await
            .map_err(ProductError::from)?;

        if authored_prices.is_empty() {
            return Ok(());
        }

        let variant_ids = variant_products.keys().copied().collect::<Vec<_>>();
        let mut saved_prices = BootstrapService::load_prices_for_variants(txn, &variant_ids)
            .await
            .map_err(ProductError::from)?;
        saved_prices.sort_by(|left, right| {
            left.variant_id
                .cmp(&right.variant_id)
                .then_with(|| left.channel_id.cmp(&right.channel_id))
                .then_with(|| left.channel_slug.cmp(&right.channel_slug))
                .then_with(|| left.currency_code.cmp(&right.currency_code))
        });
        if saved_prices.len() != authored_prices.len() {
            return Err(ProductError::Conflict(
                "Persisted initial prices could not be matched to their inputs".to_owned(),
            ));
        }

        let mut seen = HashSet::new();
        for saved in saved_prices {
            if saved.price_list_id.is_some()
                || saved.region_id.is_some()
                || saved.min_quantity.is_some()
                || saved.max_quantity.is_some()
            {
                return Err(ProductError::Conflict(
                    "Unexpected contextual price found during initial price verification"
                        .to_owned(),
                ));
            }
            let authored = authored_prices
                .iter()
                .find(|candidate| {
                    candidate.variant_id == saved.variant_id
                        && candidate.channel_id == saved.channel_id
                        && candidate.channel_slug.as_deref() == saved.channel_slug.as_deref()
                        && candidate
                            .currency_code
                            .trim()
                            .eq_ignore_ascii_case(&saved.currency_code)
                })
                .ok_or_else(|| {
                    ProductError::Conflict(
                        "Persisted initial price identity differs from its input".to_owned(),
                    )
                })?;
            let identity = (
                saved.variant_id,
                saved.channel_id,
                saved.channel_slug.clone(),
                saved.currency_code.clone(),
            );
            if !seen.insert(identity) {
                return Err(ProductError::Conflict(
                    "Persisted initial price identity is ambiguous".to_owned(),
                ));
            }
            let product_id = variant_products
                .get(&saved.variant_id)
                .copied()
                .ok_or_else(|| {
                    ProductError::Conflict(
                        "Initial price variant lost its tenant-owned Product binding".to_owned(),
                    )
                })?;
            let new_amount = price_event_successor(&saved, authored)?;
            self.publish_price_updated_in_tx(
                txn,
                tenant_id,
                actor_id,
                saved.variant_id,
                product_id,
                saved.currency_code,
                None,
                new_amount,
            )
            .await?;
        }

        if seen.len() != authored_prices.len() {
            return Err(ProductError::Conflict(
                "Not every initial price produced a persisted successor".to_owned(),
            ));
        }
        Ok(())
    }

    async fn replace_catalog_base_prices_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        prices: Vec<InitialPrice>,
    ) -> Result<(), ProductError> {
        let variant = crate::services::pricing::scope::load_variant_for_update_in_tx(
            txn, tenant_id, variant_id,
        )
        .await
        .map_err(map_catalog_scope_error)?;
        if prices.iter().any(|price| {
            price.variant_id != variant_id
                || price.channel_id.is_some()
                || price.channel_slug.is_some()
        }) {
            return Err(ProductError::Validation(
                "Catalog base-price replacement accepts only unscoped prices for its Variant"
                    .to_owned(),
            ));
        }
        validate_catalog_price_precision(&prices)?;

        let mut old_amounts = HashMap::new();
        if !prices.is_empty() {
            let existing =
                BootstrapService::load_catalog_base_prices_for_variants(txn, &[variant_id])
                    .await
                    .map_err(ProductError::from)?;
            for saved in existing {
                let currency = saved.currency_code.clone();
                let amount = price_event_predecessor(&saved)?;
                if old_amounts.insert(currency, amount).is_some() {
                    return Err(ProductError::Conflict(
                        "Ambiguous stored base prices prevent catalog replacement".to_owned(),
                    ));
                }
            }
        }

        let mut authored_by_currency = HashMap::new();
        for authored in prices.iter().cloned() {
            let currency = authored.currency_code.trim().to_ascii_uppercase();
            if authored_by_currency.insert(currency, authored).is_some() {
                return Err(ProductError::Validation(
                    "Duplicate catalog base-price currency".to_owned(),
                ));
            }
        }

        BootstrapService::replace_catalog_base_prices_in_tx(txn, variant_id, prices)
            .await
            .map_err(ProductError::from)?;

        let mut saved_prices =
            BootstrapService::load_catalog_base_prices_for_variants(txn, &[variant_id])
                .await
                .map_err(ProductError::from)?;
        saved_prices.sort_by(|left, right| left.currency_code.cmp(&right.currency_code));
        if saved_prices.len() != authored_by_currency.len() {
            return Err(ProductError::Conflict(
                "Persisted catalog base prices could not be matched to their inputs".to_owned(),
            ));
        }

        for saved in saved_prices {
            let currency = saved.currency_code.clone();
            let authored = authored_by_currency.remove(&currency).ok_or_else(|| {
                ProductError::Conflict(
                    "Persisted catalog base-price currency differs from its input".to_owned(),
                )
            })?;
            let new_amount = price_event_successor(&saved, &authored)?;
            let old_amount = old_amounts.remove(&currency);
            self.publish_price_updated_in_tx(
                txn,
                tenant_id,
                actor_id,
                variant_id,
                variant.product_id,
                currency,
                old_amount,
                new_amount,
            )
            .await?;
        }

        if !authored_by_currency.is_empty() {
            return Err(ProductError::Conflict(
                "Not every catalog base price produced a persisted successor".to_owned(),
            ));
        }
        Ok(())
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
pub fn in_process_product_pricing_port(
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
) -> Arc<dyn ProductPricingPort> {
    Arc::new(PricingProductPricingPortAdapter::new(db, event_bus))
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
    #[test]
    fn price_event_amount_requires_exact_currency_minor_units() {
        assert_eq!(
            price_event_amount(Decimal::new(1234, 2), "USD").unwrap(),
            1234
        );
        assert!(price_event_amount(Decimal::new(12345, 3), "USD").is_err());
    }

    #[test]
    fn price_event_successor_uses_the_persisted_exact_amount() {
        let authored = initial_price(Uuid::from_u128(100));
        let saved = price::Model {
            amount: authored.amount,
            compare_at_amount: authored.compare_at_amount,
            ..candidate(1, None, None, None, None, None, 0)
        };

        assert_eq!(price_event_successor(&saved, &authored).unwrap(), 1000);

        let mismatched = price::Model {
            amount: Decimal::new(1100, 2),
            ..saved
        };
        assert!(price_event_successor(&mismatched, &authored).is_err());
    }
}
