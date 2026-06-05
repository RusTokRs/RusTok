use crate::model::{InventoryProductDetail, InventoryVariant};

pub(crate) const DEFAULT_PRODUCT_PAGE: u64 = 1;
pub(crate) const DEFAULT_PRODUCT_PAGE_SIZE: u64 = 24;

#[derive(Clone, Debug)]
pub(crate) struct InventoryProductsRequest {
    pub token: Option<String>,
    pub tenant_slug: Option<String>,
    pub tenant_id: String,
    pub locale: Option<String>,
    pub search: Option<String>,
    pub status: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InventoryProductsFilter {
    pub locale: Option<String>,
    pub search: Option<String>,
    pub status: Option<String>,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct InventoryProductRequest {
    pub token: Option<String>,
    pub tenant_slug: Option<String>,
    pub tenant_id: String,
    pub id: String,
    pub locale: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InventoryProductSelector {
    pub id: String,
    pub locale: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct InventorySetQuantityRequest {
    pub tenant_id: String,
    pub variant_id: String,
    pub quantity: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InventorySetQuantityInput {
    pub tenant_id: String,
    pub variant_id: String,
    pub quantity: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InventoryHealthState {
    Backorder,
    OutOfStock,
    LowStock,
    Healthy,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct InventoryHealthCounts {
    pub low_stock: usize,
    pub backorder: usize,
    pub out_of_stock: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct InventorySummary {
    pub variant_count: usize,
    pub total_quantity: i32,
    pub low_stock: usize,
    pub backorder: usize,
    pub out_of_stock: usize,
    pub healthy: usize,
}

pub(crate) fn normalized_products_filter(
    locale: Option<String>,
    search: Option<String>,
    status: Option<String>,
) -> InventoryProductsFilter {
    InventoryProductsFilter {
        locale: normalize_locale_filter(locale),
        search: normalize_search_filter(search),
        status: normalize_status_filter(status),
        page: DEFAULT_PRODUCT_PAGE,
        per_page: DEFAULT_PRODUCT_PAGE_SIZE,
    }
}

pub(crate) fn normalized_product_selector(
    id: String,
    locale: Option<String>,
) -> InventoryProductSelector {
    InventoryProductSelector {
        id,
        locale: normalize_locale_filter(locale),
    }
}

pub(crate) fn normalized_set_quantity_input(
    tenant_id: String,
    variant_id: String,
    quantity: i32,
) -> InventorySetQuantityInput {
    InventorySetQuantityInput {
        tenant_id: tenant_id.trim().to_string(),
        variant_id: variant_id.trim().to_string(),
        quantity,
    }
}

pub(crate) fn apply_variant_quantity_update(
    detail: &mut InventoryProductDetail,
    variant_id: &str,
    new_quantity: i32,
) -> bool {
    let Some(variant) = detail
        .variants
        .iter_mut()
        .find(|variant| variant.id == variant_id)
    else {
        return false;
    };

    variant.inventory_quantity = new_quantity;
    variant.in_stock = new_quantity > 0;
    true
}

pub(crate) fn parse_set_quantity(value: &str) -> Result<i32, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("quantity is required".to_string());
    }

    trimmed
        .parse::<i32>()
        .map_err(|_| "quantity must be a signed integer".to_string())
}

pub(crate) fn normalize_optional_trimmed(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

pub(crate) fn normalize_locale_filter(locale: Option<String>) -> Option<String> {
    normalize_optional_trimmed(locale)
}

pub(crate) fn normalize_search_filter(search: Option<String>) -> Option<String> {
    normalize_optional_trimmed(search)
}

pub(crate) fn normalize_status_filter(status: Option<String>) -> Option<String> {
    normalize_optional_trimmed(status).map(|value| value.to_ascii_uppercase())
}

pub(crate) fn summarize_inventory(variants: &[InventoryVariant]) -> InventorySummary {
    let health_counts = summarize_inventory_health_counts(variants);
    let non_healthy_total = health_counts.non_healthy_total();
    let healthy_total = variants.len().saturating_sub(non_healthy_total);

    debug_assert_eq!(
        non_healthy_total + healthy_total,
        variants.len(),
        "inventory health partition must cover every variant exactly once"
    );

    InventorySummary {
        variant_count: variants.len(),
        total_quantity: variants
            .iter()
            .map(|variant| variant.inventory_quantity)
            .sum(),
        low_stock: health_counts.low_stock,
        backorder: health_counts.backorder,
        out_of_stock: health_counts.out_of_stock,
        healthy: healthy_total,
    }
}

impl InventoryHealthCounts {
    pub(crate) fn non_healthy_total(self) -> usize {
        self.low_stock + self.backorder + self.out_of_stock
    }
}

pub(crate) fn summarize_inventory_health_counts(
    variants: &[InventoryVariant],
) -> InventoryHealthCounts {
    variants
        .iter()
        .fold(InventoryHealthCounts::default(), |mut counts, variant| {
            match inventory_health_state(variant) {
                InventoryHealthState::LowStock => counts.low_stock += 1,
                InventoryHealthState::Backorder => counts.backorder += 1,
                InventoryHealthState::OutOfStock => counts.out_of_stock += 1,
                InventoryHealthState::Healthy => {}
            }
            counts
        })
}

pub(crate) fn inventory_health_state(variant: &InventoryVariant) -> InventoryHealthState {
    if variant.inventory_policy.eq_ignore_ascii_case("continue") {
        InventoryHealthState::Backorder
    } else if !variant.in_stock {
        InventoryHealthState::OutOfStock
    } else if variant.inventory_quantity <= LOW_STOCK_THRESHOLD {
        InventoryHealthState::LowStock
    } else {
        InventoryHealthState::Healthy
    }
}

pub(crate) const LOW_STOCK_THRESHOLD: i32 = 5;

#[cfg(test)]
mod tests {
    use super::*;

    fn variant(in_stock: bool, policy: &str, quantity: i32) -> InventoryVariant {
        InventoryVariant {
            id: format!("variant-{quantity}-{policy}"),
            sku: Some(format!("SKU-{quantity}")),
            barcode: None,
            shipping_profile_slug: None,
            title: format!("Variant {quantity}"),
            option1: None,
            option2: None,
            option3: None,
            prices: Vec::new(),
            inventory_quantity: quantity,
            inventory_policy: policy.to_string(),
            in_stock,
        }
    }

    fn detail_with_variants(variants: Vec<InventoryVariant>) -> InventoryProductDetail {
        InventoryProductDetail {
            id: "product-1".to_string(),
            status: "ACTIVE".to_string(),
            vendor: None,
            product_type: None,
            shipping_profile_slug: None,
            created_at: "2026-06-05T00:00:00Z".to_string(),
            updated_at: "2026-06-05T00:00:00Z".to_string(),
            published_at: None,
            translations: Vec::new(),
            variants,
        }
    }

    #[test]
    fn normalized_set_quantity_input_trims_route_identifiers_without_changing_quantity() {
        let input = normalized_set_quantity_input(
            " tenant-id ".to_string(),
            " variant-id ".to_string(),
            -3,
        );

        assert_eq!(input.tenant_id, "tenant-id");
        assert_eq!(input.variant_id, "variant-id");
        assert_eq!(input.quantity, -3);
    }

    #[test]
    fn parse_set_quantity_accepts_signed_integer_with_whitespace() {
        assert_eq!(parse_set_quantity(" 42 "), Ok(42));
        assert_eq!(parse_set_quantity("-3"), Ok(-3));
    }

    #[test]
    fn parse_set_quantity_rejects_blank_or_non_integer_values() {
        assert!(parse_set_quantity("   ").is_err());
        assert!(parse_set_quantity("1.5").is_err());
        assert!(parse_set_quantity("many").is_err());
    }

    #[test]
    fn apply_variant_quantity_update_updates_quantity_and_stock_flag() {
        let mut detail = detail_with_variants(vec![variant(true, "deny", 2)]);

        assert!(apply_variant_quantity_update(
            &mut detail,
            "variant-2-deny",
            0
        ));
        assert_eq!(detail.variants[0].inventory_quantity, 0);
        assert!(!detail.variants[0].in_stock);

        assert!(apply_variant_quantity_update(
            &mut detail,
            "variant-2-deny",
            7
        ));
        assert_eq!(detail.variants[0].inventory_quantity, 7);
        assert!(detail.variants[0].in_stock);
    }

    #[test]
    fn apply_variant_quantity_update_returns_false_for_unknown_variant() {
        let mut detail = detail_with_variants(vec![variant(true, "deny", 2)]);

        assert!(!apply_variant_quantity_update(
            &mut detail,
            "missing-variant",
            9
        ));
        assert_eq!(detail.variants[0].inventory_quantity, 2);
    }

    #[test]
    fn normalize_optional_trimmed_keeps_non_blank_and_drops_blank_values() {
        assert_eq!(
            normalize_optional_trimmed(Some("  value  ".to_string())),
            Some("value".to_string())
        );
        assert_eq!(normalize_optional_trimmed(Some("   ".to_string())), None);
        assert_eq!(normalize_optional_trimmed(None), None);
    }

    #[test]
    fn normalized_products_filter_trims_status_locale_and_search() {
        assert_eq!(
            normalized_products_filter(
                Some("  de-DE  ".to_string()),
                Some("  winter jacket  ".to_string()),
                Some(" active ".to_string()),
            ),
            InventoryProductsFilter {
                locale: Some("de-DE".to_string()),
                search: Some("winter jacket".to_string()),
                status: Some("ACTIVE".to_string()),
                page: DEFAULT_PRODUCT_PAGE,
                per_page: DEFAULT_PRODUCT_PAGE_SIZE,
            }
        );
    }

    #[test]
    fn normalized_products_filter_drops_blank_values() {
        assert_eq!(
            normalized_products_filter(
                Some("   ".to_string()),
                Some("   ".to_string()),
                Some("   ".to_string())
            ),
            InventoryProductsFilter {
                locale: None,
                search: None,
                status: None,
                page: DEFAULT_PRODUCT_PAGE,
                per_page: DEFAULT_PRODUCT_PAGE_SIZE,
            }
        );
    }

    #[test]
    fn normalized_product_selector_trims_locale_without_rewriting_id() {
        assert_eq!(
            normalized_product_selector(" product-1 ".to_string(), Some("  en-US  ".to_string())),
            InventoryProductSelector {
                id: " product-1 ".to_string(),
                locale: Some("en-US".to_string()),
            }
        );
    }

    #[test]
    fn summary_keeps_low_stock_out_of_stock_and_backorder_disjoint() {
        let variants = vec![
            variant(true, "deny", LOW_STOCK_THRESHOLD),
            variant(false, "deny", 10),
            variant(false, "continue", 0),
            variant(true, "continue", 20),
        ];

        let summary = summarize_inventory(&variants);
        assert_eq!(summary.variant_count, 4);
        assert_eq!(summary.low_stock, 1);
        assert_eq!(summary.out_of_stock, 1);
        assert_eq!(summary.backorder, 2);
        assert_eq!(summary.healthy, 0);
    }

    #[test]
    fn health_state_treats_backorder_policy_case_insensitively() {
        assert_eq!(
            inventory_health_state(&variant(false, "CONTINUE", 0)),
            InventoryHealthState::Backorder
        );
    }

    #[test]
    fn summary_partition_is_complete_including_healthy_bucket() {
        let variants = vec![
            variant(true, "deny", LOW_STOCK_THRESHOLD + 3),
            variant(true, "deny", LOW_STOCK_THRESHOLD),
            variant(false, "deny", 3),
            variant(true, "continue", 0),
        ];

        let summary = summarize_inventory(&variants);
        assert_eq!(
            summary.healthy + summary.low_stock + summary.out_of_stock + summary.backorder,
            summary.variant_count
        );
    }
}
