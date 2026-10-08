use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorefrontOrderLineItem {
    pub id: String,
    pub product_id: Option<String>,
    pub variant_id: Option<String>,
    pub sku: Option<String>,
    pub title: String,
    pub quantity: i32,
    pub unit_price: String,
    pub total_price: String,
    pub currency_code: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorefrontOrderAdjustment {
    pub id: String,
    pub line_item_id: Option<String>,
    pub source_type: String,
    pub source_id: Option<String>,
    pub amount: String,
    pub currency_code: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorefrontCustomerAddress {
    pub full_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub country_code: Option<String>,
    pub city: Option<String>,
    pub street_address: Option<String>,
    pub postal_code: Option<String>,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorefrontOrder {
    pub id: String,
    pub status: String,
    pub currency_code: String,
    pub subtotal_amount: String,
    pub adjustment_total: String,
    pub shipping_total: String,
    pub total_amount: String,
    pub tax_total: String,
    pub tax_included: bool,
    pub metadata: String,
    pub payment_id: Option<String>,
    pub payment_method: Option<String>,
    pub tracking_number: Option<String>,
    pub carrier: Option<String>,
    pub cancellation_reason: Option<String>,
    pub delivered_signature: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub confirmed_at: Option<String>,
    pub paid_at: Option<String>,
    pub shipped_at: Option<String>,
    pub delivered_at: Option<String>,
    pub cancelled_at: Option<String>,
    pub line_items: Vec<StorefrontOrderLineItem>,
    pub adjustments: Vec<StorefrontOrderAdjustment>,
}

impl StorefrontOrder {
    pub fn parse_delivery_address(&self) -> Option<StorefrontCustomerAddress> {
        if self.metadata.trim().is_empty() {
            return None;
        }

        #[derive(Deserialize)]
        struct RawMetadata {
            #[serde(default, rename = "fullName")]
            full_name: Option<String>,
            #[serde(default)]
            email: Option<String>,
            #[serde(default)]
            phone: Option<String>,
            #[serde(default, rename = "countryCode")]
            country_code: Option<String>,
            #[serde(default)]
            city: Option<String>,
            #[serde(default, rename = "streetAddress")]
            street_address: Option<String>,
            #[serde(default, rename = "postalCode")]
            postal_code: Option<String>,
            #[serde(default)]
            notes: Option<String>,
        }

        let parsed: RawMetadata = serde_json::from_str(&self.metadata).ok()?;
        Some(StorefrontCustomerAddress {
            full_name: parsed.full_name.filter(|s| !s.trim().is_empty()),
            email: parsed.email.filter(|s| !s.trim().is_empty()),
            phone: parsed.phone.filter(|s| !s.trim().is_empty()),
            country_code: parsed.country_code.filter(|s| !s.trim().is_empty()),
            city: parsed.city.filter(|s| !s.trim().is_empty()),
            street_address: parsed.street_address.filter(|s| !s.trim().is_empty()),
            postal_code: parsed.postal_code.filter(|s| !s.trim().is_empty()),
            notes: parsed.notes.filter(|s| !s.trim().is_empty()),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorefrontOrdersResponse {
    pub items: Vec<StorefrontOrder>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub has_next: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_delivery_address_from_metadata() {
        let order = StorefrontOrder {
            id: "ord-1".into(),
            status: "PROCESSING".into(),
            currency_code: "RUB".into(),
            subtotal_amount: "1500.00".into(),
            adjustment_total: "0".into(),
            shipping_total: "300.00".into(),
            total_amount: "1800.00".into(),
            tax_total: "0".into(),
            tax_included: false,
            metadata: serde_json::json!({
                "fullName": "Иван Иванов",
                "phone": "+7 999 123-45-67",
                "city": "Москва",
                "streetAddress": "ул. Тверская, д. 1",
                "postalCode": "125009",
            })
            .to_string(),
            payment_id: None,
            payment_method: Some("card".into()),
            tracking_number: Some("TRK123456".into()),
            carrier: Some("CDEK".into()),
            cancellation_reason: None,
            delivered_signature: None,
            created_at: "2026-10-04T12:00:00Z".into(),
            updated_at: "2026-10-04T12:00:00Z".into(),
            confirmed_at: None,
            paid_at: None,
            shipped_at: None,
            delivered_at: None,
            cancelled_at: None,
            line_items: vec![],
            adjustments: vec![],
        };

        let addr = order.parse_delivery_address().expect("address parsed");
        assert_eq!(addr.full_name.as_deref(), Some("Иван Иванов"));
        assert_eq!(addr.phone.as_deref(), Some("+7 999 123-45-67"));
        assert_eq!(addr.city.as_deref(), Some("Москва"));
        assert_eq!(addr.street_address.as_deref(), Some("ул. Тверская, д. 1"));
        assert_eq!(addr.postal_code.as_deref(), Some("125009"));
    }

    #[test]
    fn handles_empty_or_invalid_metadata() {
        let order = StorefrontOrder {
            id: "ord-2".into(),
            status: "PENDING".into(),
            currency_code: "USD".into(),
            subtotal_amount: "10.00".into(),
            adjustment_total: "0".into(),
            shipping_total: "0".into(),
            total_amount: "10.00".into(),
            tax_total: "0".into(),
            tax_included: false,
            metadata: "not valid json".into(),
            payment_id: None,
            payment_method: None,
            tracking_number: None,
            carrier: None,
            cancellation_reason: None,
            delivered_signature: None,
            created_at: "2026-10-04T12:00:00Z".into(),
            updated_at: "2026-10-04T12:00:00Z".into(),
            confirmed_at: None,
            paid_at: None,
            shipped_at: None,
            delivered_at: None,
            cancelled_at: None,
            line_items: vec![],
            adjustments: vec![],
        };

        assert!(order.parse_delivery_address().is_none());
    }
}
