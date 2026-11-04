//! Product-owned port boundary for pricing collaboration.
//!
//! Provides transport-neutral and persistence-neutral contracts for variant price
//! authoring, transactional cleanup, and storefront/catalog price projections.
//! No Pricing tables or SeaORM entities are imported here.

use async_trait::async_trait;
use rust_decimal::Decimal;
use sea_orm::{DatabaseConnection, DatabaseTransaction};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{CommerceError, dto::StorefrontProductListPrice};

/// Initial price input for variant bootstrapping.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InitialPrice {
    pub variant_id: Uuid,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub currency_code: String,
    pub amount: Decimal,
    pub compare_at_amount: Option<Decimal>,
}

/// Bounded snapshot of an unscoped base price for catalog projections.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogBasePriceSnapshot {
    pub id: Uuid,
    pub variant_id: Uuid,
    pub currency_code: String,
    pub amount: Decimal,
    pub compare_at_amount: Option<Decimal>,
}

/// Either an open transaction or a database connection pool.
#[derive(Copy, Clone)]
pub enum PricingConnection<'a> {
    Connection(&'a DatabaseConnection),
    Transaction(&'a DatabaseTransaction),
}

impl<'a> From<&'a DatabaseConnection> for PricingConnection<'a> {
    fn from(c: &'a DatabaseConnection) -> Self {
        PricingConnection::Connection(c)
    }
}

impl<'a> From<&'a DatabaseTransaction> for PricingConnection<'a> {
    fn from(tx: &'a DatabaseTransaction) -> Self {
        PricingConnection::Transaction(tx)
    }
}

/// Transactional and projection port for variant prices.
#[async_trait]
pub trait ProductPricingPort: Send + Sync {
    /// Authors initial variant prices in the active product write transaction.
    async fn create_initial_prices_in_tx(
        &self,
        txn: &DatabaseTransaction,
        prices: Vec<InitialPrice>,
    ) -> Result<(), CommerceError>;

    /// Replaces variant unscoped base prices in the active product write transaction.
    async fn replace_catalog_base_prices_in_tx(
        &self,
        txn: &DatabaseTransaction,
        variant_id: Uuid,
        prices: Vec<InitialPrice>,
    ) -> Result<(), CommerceError>;

    /// Deletes all prices for the specified variants in the active product write transaction.
    async fn delete_prices_for_variants_in_tx(
        &self,
        txn: &DatabaseTransaction,
        variant_ids: &[Uuid],
    ) -> Result<(), CommerceError>;

    /// Loads unscoped base prices for variants for admin or storefront catalog detail projections.
    async fn load_catalog_base_prices_for_variants(
        &self,
        conn: PricingConnection<'_>,
        variant_ids: &[Uuid],
    ) -> Result<Vec<CatalogBasePriceSnapshot>, CommerceError>;

    /// Resolves storefront minimum "from" prices for a product listing.
    async fn load_storefront_list_prices(
        &self,
        conn: PricingConnection<'_>,
        variant_to_product: &HashMap<Uuid, Uuid>,
        variant_ids: &[Uuid],
        currency_code: Option<&str>,
        public_channel_slug: Option<&str>,
    ) -> Result<HashMap<Uuid, StorefrontProductListPrice>, CommerceError>;
}
