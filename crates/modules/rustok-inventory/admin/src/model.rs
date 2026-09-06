use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CurrentTenant {
    pub id: String,
    pub slug: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InventoryAdminBootstrap {
    #[serde(rename = "currentTenant")]
    pub current_tenant: CurrentTenant,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InventoryProductList {
    pub items: Vec<InventoryProductListItem>,
    pub total: u64,
    pub page: u64,
    #[serde(rename = "perPage")]
    pub per_page: u64,
    #[serde(rename = "hasNext")]
    pub has_next: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InventoryProductListItem {
    pub id: String,
    pub status: String,
    pub title: String,
    pub handle: String,
    pub vendor: Option<String>,
    #[serde(rename = "productType")]
    pub product_type: Option<String>,
    #[serde(rename = "shippingProfileSlug")]
    pub shipping_profile_slug: Option<String>,
    pub tags: Vec<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "publishedAt")]
    pub published_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InventoryProductDetail {
    pub id: String,
    pub status: String,
    pub vendor: Option<String>,
    #[serde(rename = "productType")]
    pub product_type: Option<String>,
    #[serde(rename = "shippingProfileSlug")]
    pub shipping_profile_slug: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "publishedAt")]
    pub published_at: Option<String>,
    pub translations: Vec<InventoryProductTranslation>,
    pub variants: Vec<InventoryVariant>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InventoryProductTranslation {
    pub locale: String,
    pub title: String,
    pub handle: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InventoryVariant {
    pub id: String,
    pub sku: Option<String>,
    pub barcode: Option<String>,
    #[serde(rename = "shippingProfileSlug")]
    pub shipping_profile_slug: Option<String>,
    pub title: String,
    pub option1: Option<String>,
    pub option2: Option<String>,
    pub option3: Option<String>,
    pub prices: Vec<InventoryPrice>,
    #[serde(rename = "inventoryQuantity")]
    pub inventory_quantity: i32,
    #[serde(rename = "inventoryPolicy")]
    pub inventory_policy: String,
    #[serde(rename = "inStock")]
    pub in_stock: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InventoryPrice {
    #[serde(rename = "currencyCode")]
    pub currency_code: String,
    pub amount: String,
    #[serde(rename = "compareAtAmount")]
    pub compare_at_amount: Option<String>,
    #[serde(rename = "onSale")]
    pub on_sale: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InventoryQuantityWriteResult {
    pub quantity: i32,
    #[serde(rename = "inStock")]
    pub in_stock: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InventoryReservationWriteResult {
    #[serde(rename = "reservedQuantity")]
    pub reserved_quantity: i32,
    #[serde(rename = "availableQuantity")]
    pub available_quantity: i32,
    #[serde(rename = "inStock")]
    pub in_stock: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InventoryAvailabilityCheckResult {
    pub available: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InventoryReservationReleaseWriteResult {
    #[serde(rename = "releasedQuantity")]
    pub released_quantity: i32,
    #[serde(rename = "availableQuantity")]
    pub available_quantity: i32,
    #[serde(rename = "inStock")]
    pub in_stock: bool,
}

#[cfg(test)]
mod tests {
    use super::{
        InventoryAvailabilityCheckResult, InventoryProductDetail, InventoryProductList,
        InventoryQuantityWriteResult, InventoryReservationReleaseWriteResult,
        InventoryReservationWriteResult,
    };

    #[test]
    fn quantity_write_result_keeps_native_endpoint_wire_shape() {
        let value = serde_json::json!({
            "quantity": 3,
            "inStock": false
        });

        let result: InventoryQuantityWriteResult = serde_json::from_value(value.clone())
            .expect("inventory quantity write result compatibility snapshot should deserialize");

        assert_eq!(result.quantity, 3);
        assert!(!result.in_stock);

        let serialized = serde_json::to_value(&result)
            .expect("inventory quantity write result compatibility snapshot should serialize");
        assert_eq!(serialized, value);
    }

    #[test]
    fn availability_check_result_keeps_native_endpoint_wire_shape() {
        let value = serde_json::json!({
            "available": true
        });

        let result: InventoryAvailabilityCheckResult = serde_json::from_value(value.clone())
            .expect(
                "inventory availability check result compatibility snapshot should deserialize",
            );

        assert!(result.available);

        let serialized = serde_json::to_value(&result)
            .expect("inventory availability check result compatibility snapshot should serialize");
        assert_eq!(serialized, value);
    }

    #[test]
    fn reservation_release_write_result_keeps_native_endpoint_wire_shape() {
        let value = serde_json::json!({
            "releasedQuantity": 2,
            "availableQuantity": 10,
            "inStock": true
        });

        let result: InventoryReservationReleaseWriteResult = serde_json::from_value(value.clone())
            .expect(
            "inventory reservation release write result compatibility snapshot should deserialize",
        );

        assert_eq!(result.released_quantity, 2);
        assert_eq!(result.available_quantity, 10);
        assert!(result.in_stock);

        let serialized = serde_json::to_value(&result).expect(
            "inventory reservation release write result compatibility snapshot should serialize",
        );
        assert_eq!(serialized, value);
    }

    #[test]
    fn reservation_write_result_keeps_native_endpoint_wire_shape() {
        let value = serde_json::json!({
            "reservedQuantity": 2,
            "availableQuantity": 8,
            "inStock": true
        });

        let result: InventoryReservationWriteResult = serde_json::from_value(value.clone())
            .expect("inventory reservation write result compatibility snapshot should deserialize");

        assert_eq!(result.reserved_quantity, 2);
        assert_eq!(result.available_quantity, 8);
        assert!(result.in_stock);

        let serialized = serde_json::to_value(&result)
            .expect("inventory reservation write result compatibility snapshot should serialize");
        assert_eq!(serialized, value);
    }

    #[test]
    fn product_list_read_model_keeps_commerce_graphql_compatibility_shape() {
        let value = serde_json::json!({
            "items": [
                {
                    "id": "product-1",
                    "status": "ACTIVE",
                    "title": "Winter jacket",
                    "handle": "winter-jacket",
                    "vendor": "Acme",
                    "productType": "Outerwear",
                    "shippingProfileSlug": "default",
                    "tags": ["winter", "sale"],
                    "createdAt": "2026-06-01T00:00:00Z",
                    "publishedAt": "2026-06-02T00:00:00Z"
                }
            ],
            "total": 1,
            "page": 1,
            "perPage": 24,
            "hasNext": false
        });

        let list: InventoryProductList = serde_json::from_value(value.clone())
            .expect("inventory product list compatibility snapshot should deserialize");

        assert_eq!(list.total, 1);
        assert_eq!(list.page, 1);
        assert_eq!(list.per_page, 24);
        assert!(!list.has_next);
        let item = list
            .items
            .first()
            .expect("snapshot should contain one item");
        assert_eq!(item.id, "product-1");
        assert_eq!(item.title, "Winter jacket");
        assert_eq!(item.handle, "winter-jacket");
        assert_eq!(item.shipping_profile_slug.as_deref(), Some("default"));

        let serialized = serde_json::to_value(&list)
            .expect("inventory product list compatibility snapshot should serialize");
        assert_eq!(serialized, value);
    }

    #[test]
    fn product_detail_read_model_keeps_inventory_variant_and_translation_shape() {
        let value = serde_json::json!({
            "id": "product-1",
            "status": "ACTIVE",
            "vendor": "Acme",
            "productType": "Outerwear",
            "shippingProfileSlug": "default",
            "createdAt": "2026-06-01T00:00:00Z",
            "updatedAt": "2026-06-03T00:00:00Z",
            "publishedAt": null,
            "translations": [
                {
                    "locale": "en-US",
                    "title": "Winter jacket",
                    "handle": "winter-jacket",
                    "description": "Warm jacket"
                }
            ],
            "variants": [
                {
                    "id": "variant-1",
                    "sku": "SKU-1",
                    "barcode": null,
                    "shippingProfileSlug": "bulky",
                    "title": "Blue / M",
                    "option1": "Blue",
                    "option2": "M",
                    "option3": null,
                    "prices": [
                        {
                            "currencyCode": "USD",
                            "amount": "99.00",
                            "compareAtAmount": "129.00",
                            "onSale": true
                        }
                    ],
                    "inventoryQuantity": 7,
                    "inventoryPolicy": "DENY",
                    "inStock": true
                }
            ]
        });

        let detail: InventoryProductDetail = serde_json::from_value(value.clone())
            .expect("inventory product detail compatibility snapshot should deserialize");

        assert_eq!(detail.id, "product-1");
        assert_eq!(detail.translations[0].locale, "en-US");
        assert_eq!(detail.translations[0].handle, "winter-jacket");
        let variant = detail
            .variants
            .first()
            .expect("snapshot should contain one variant");
        assert_eq!(variant.id, "variant-1");
        assert_eq!(variant.inventory_quantity, 7);
        assert_eq!(variant.inventory_policy, "DENY");
        assert!(variant.in_stock);
        assert_eq!(variant.prices[0].currency_code, "USD");
        assert!(variant.prices[0].on_sale);

        let serialized = serde_json::to_value(&detail)
            .expect("inventory product detail compatibility snapshot should serialize");
        assert_eq!(serialized, value);
    }
}
