use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortContext, PortError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Transport-neutral owner boundary for checkout shipping selection.
#[async_trait]
pub trait ShippingSelectionPort: Send + Sync {
    async fn list_seller_shipping_options(
        &self,
        context: PortContext,
        request: ListSellerShippingOptionsRequest,
    ) -> Result<SellerShippingOptionsSnapshot, PortError>;

    async fn select_shipping_option(
        &self,
        context: PortContext,
        request: SelectShippingOptionPortRequest,
    ) -> Result<SelectedShippingOptionSnapshot, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListSellerShippingOptionsRequest {
    pub cart_id: Uuid,
    pub seller_id: Option<String>,
    pub shipping_profile_slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SelectShippingOptionPortRequest {
    pub cart_id: Uuid,
    pub seller_id: Option<String>,
    pub shipping_option_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SellerShippingOptionsSnapshot {
    pub cart_id: Uuid,
    pub seller_id: Option<String>,
    pub options: Vec<ShippingOptionProjection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShippingOptionProjection {
    pub id: Uuid,
    pub provider_id: String,
    pub name: String,
    pub currency_code: String,
    pub amount: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelectedShippingOptionSnapshot {
    pub cart_id: Uuid,
    pub seller_id: Option<String>,
    pub option: ShippingOptionProjection,
}
