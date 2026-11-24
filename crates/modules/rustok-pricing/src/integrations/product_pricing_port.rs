//! Adapter implementing `rustok_product::ProductPricingPort` for the Product catalog.
//!
//! Provides transactional price authoring/cleanup and catalog price projections without
//! exposing private Pricing persistence entities or table structures to Product.

use async_trait::async_trait;
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
        let mut validated_variants = HashSet::new();
        let mut identities = HashSet::new();
        for price in &prices {
            if validated_variants.insert(price.variant_id) {
                crate::services::pricing::scope::load_variant_for_update_in_tx(
                    txn,
                    tenant_id,
                    price.variant_id,
                )
                .await
                .map_err(rustok_product::CommerceError::from)?;
            }
            let channel_id = crate::services::pricing::scope::resolve_channel_id_in_tx(
                txn,
                tenant_id,
                price.channel_id,
                price.channel_slug.as_deref(),
            )
            .await
            .map_err(rustok_product::CommerceError::from)?;
            let identity = (
                price.variant_id,
                channel_id,
                price.currency_code.trim().to_ascii_uppercase(),
            );
            if !identities.insert(identity) {
                return Err(rustok_product::CommerceError::Validation(
                    "Duplicate initial price for the same variant, channel and currency"
                        .to_owned(),
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
        crate::services::pricing::scope::load_variant_for_update_in_tx(txn, tenant_id, variant_id)
            .await
            .map_err(rustok_product::CommerceError::from)?;
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
                .map_err(rustok_product::CommerceError::from)?;
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

        let models = match conn {
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
            if let Some(code) = currency_code
                && !price.currency_code.eq_ignore_ascii_case(code)
            {
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
            let selected = select_storefront_list_price_candidates(candidates, public_channel_slug);
            if let Some(price) = selected
                .into_iter()
                .min_by(|left, right| left.amount.cmp(&right.amount))
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

fn select_storefront_list_price_candidates<'a>(
    candidates: Vec<&'a price::Model>,
    public_channel_slug: Option<&str>,
) -> Vec<&'a price::Model> {
    if let Some(channel_slug) = public_channel_slug {
        let channel_scoped = candidates
            .iter()
            .copied()
            .filter(|price| {
                price
                    .channel_slug
                    .as_deref()
                    .is_some_and(|slug| slug.eq_ignore_ascii_case(channel_slug))
            })
            .collect::<Vec<_>>();
        if !channel_scoped.is_empty() {
            return channel_scoped;
        }
    }

    let channel_neutral = candidates
        .iter()
        .copied()
        .filter(|price| price.channel_id.is_none() && price.channel_slug.is_none())
        .collect::<Vec<_>>();
    if !channel_neutral.is_empty() {
        return channel_neutral;
    }

    candidates
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

    #[test]
    fn channel_id_only_price_is_not_used_as_channel_neutral_fallback() {
        let channel_only = candidate(
            1,
            Some(Uuid::from_u128(2)),
            None,
            None,
            None,
            None,
            5,
        );
        let neutral = candidate(3, None, None, None, None, None, 10);

        let selected = select_storefront_list_price_candidates(
            vec![&channel_only, &neutral],
            None,
        );

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

        let selected = select_storefront_list_price_candidates(
            vec![&matching, &other, &neutral],
            Some("web"),
        );

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].id, matching.id);
    }

    #[test]
    fn region_and_quantity_tiers_are_not_list_price_candidates_without_context() {
        let base = candidate(1, None, None, None, None, None, 10);
        let regional = candidate(
            2,
            None,
            None,
            Some(Uuid::from_u128(3)),
            None,
            None,
            8,
        );
        let min_tier = candidate(4, None, None, None, Some(2), None, 7);
        let max_tier = candidate(5, None, None, None, None, Some(4), 6);

        assert!(is_storefront_list_price_candidate(&base));
        assert!(!is_storefront_list_price_candidate(&regional));
        assert!(!is_storefront_list_price_candidate(&min_tier));
        assert!(!is_storefront_list_price_candidate(&max_tier));
    }
}
