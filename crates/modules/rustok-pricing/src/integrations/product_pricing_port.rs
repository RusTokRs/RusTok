//! Adapter implementing `rustok_product::ProductPricingPort` for the Product catalog.
//!
//! Provides transactional price authoring/cleanup and catalog price projections without
//! exposing private Pricing persistence entities or table structures to Product.

use async_trait::async_trait;
use rustok_product::ports::pricing::{
    CatalogBasePriceSnapshot, InitialPrice, PricingConnection, ProductPricingPort,
};
use rustok_product::StorefrontProductListPrice;
use sea_orm::{ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter};
use std::collections::HashMap;
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
        prices: Vec<InitialPrice>,
    ) -> Result<(), rustok_product::CommerceError> {
        BootstrapService::create_initial_prices_in_tx(txn, prices)
            .await
            .map_err(rustok_product::CommerceError::from)
    }

    async fn replace_catalog_base_prices_in_tx(
        &self,
        txn: &DatabaseTransaction,
        variant_id: Uuid,
        prices: Vec<InitialPrice>,
    ) -> Result<(), rustok_product::CommerceError> {
        BootstrapService::replace_catalog_base_prices_in_tx(txn, variant_id, prices)
            .await
            .map_err(rustok_product::CommerceError::from)
    }

    async fn delete_prices_for_variants_in_tx(
        &self,
        txn: &DatabaseTransaction,
        variant_ids: &[Uuid],
    ) -> Result<(), rustok_product::CommerceError> {
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
            if price.price_list_id.is_some() || price.min_quantity.is_some() {
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
        .filter(|price| price.channel_slug.is_none())
        .collect::<Vec<_>>();
    if !channel_neutral.is_empty() {
        return channel_neutral;
    }

    candidates
}

/// Creates an in-process adapter implementing `ProductPricingPort`.
pub fn in_process_product_pricing_port(
    db: DatabaseConnection,
) -> Arc<dyn ProductPricingPort> {
    Arc::new(PricingProductPricingPortAdapter::new(db))
}
