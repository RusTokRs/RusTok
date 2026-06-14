use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortContext, PortError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Transport-neutral owner boundary for pricing read projections.
#[async_trait]
pub trait PricingReadPort: Send + Sync {
    async fn resolve_product_price(
        &self,
        context: PortContext,
        request: ResolveProductPriceRequest,
    ) -> Result<ResolvedProductPriceSnapshot, PortError>;

    async fn read_price_list_projection(
        &self,
        context: PortContext,
        request: PriceListProjectionRequest,
    ) -> Result<PriceListProjectionSnapshot, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolveProductPriceRequest {
    pub product_id: Uuid,
    pub variant_id: Option<Uuid>,
    pub region_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub currency_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PriceListProjectionRequest {
    pub price_list_id: Uuid,
    pub locale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResolvedProductPriceSnapshot {
    pub product_id: Uuid,
    pub variant_id: Option<Uuid>,
    pub currency_code: String,
    pub amount: Decimal,
    pub price_list_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PriceListProjectionSnapshot {
    pub price_list_id: Uuid,
    pub title: String,
    pub currency_code: String,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
}
