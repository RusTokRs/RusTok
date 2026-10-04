use rustok_grid::{
    ColumnAlign, ColumnFilters, FilterOption, FilterValue, GridColumnDef, GridFilterType,
};

use crate::model::{
    InventoryProductDetail, InventoryProductListItem, InventoryQuantityWriteResult,
    InventoryReservationReleaseWriteResult, InventoryReservationWriteResult, InventoryVariant,
};

#[derive(Clone, Debug)]
pub(crate) struct InventoryProductsRequest {
    pub tenant_id: String,
    pub locale: Option<String>,
    pub search: Option<String>,
    pub status: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct InventoryProductRequest {
    pub tenant_id: String,
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

#[derive(Clone, Debug)]
pub(crate) struct InventoryAdjustQuantityRequest {
    pub tenant_id: String,
    pub variant_id: String,
    pub adjustment: i32,
}

#[derive(Clone, Debug)]
pub(crate) struct InventoryReserveQuantityRequest {
    pub tenant_id: String,
    pub variant_id: String,
    pub quantity: i32,
}

#[derive(Clone, Debug)]
pub(crate) struct InventoryAvailabilityCheckRequest {
    pub tenant_id: String,
    pub variant_id: String,
    pub requested_quantity: i32,
}

#[derive(Clone, Debug)]
pub(crate) struct InventoryReleaseReservationRequest {
    pub tenant_id: String,
    pub variant_id: String,
    pub quantity: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InventoryAdjustQuantityInput {
    pub tenant_id: String,
    pub variant_id: String,
    pub adjustment: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InventoryReserveQuantityInput {
    pub tenant_id: String,
    pub variant_id: String,
    pub quantity: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InventoryAvailabilityCheckInput {
    pub tenant_id: String,
    pub variant_id: String,
    pub requested_quantity: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InventoryReleaseReservationInput {
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

#[cfg(any(feature = "ssr", test))]
pub(crate) fn normalize_status_filter(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_ascii_uppercase())
        }
    })
}

#[cfg(any(feature = "ssr", test))]
pub(crate) fn normalize_locale_filter(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

#[cfg(any(feature = "ssr", test))]
pub(crate) fn normalize_search_filter(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
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

pub(crate) fn normalized_adjust_quantity_input(
    tenant_id: String,
    variant_id: String,
    adjustment: i32,
) -> InventoryAdjustQuantityInput {
    InventoryAdjustQuantityInput {
        tenant_id: tenant_id.trim().to_string(),
        variant_id: variant_id.trim().to_string(),
        adjustment,
    }
}

pub(crate) fn normalized_reserve_quantity_input(
    tenant_id: String,
    variant_id: String,
    quantity: i32,
) -> InventoryReserveQuantityInput {
    InventoryReserveQuantityInput {
        tenant_id: tenant_id.trim().to_string(),
        variant_id: variant_id.trim().to_string(),
        quantity,
    }
}

pub(crate) fn normalized_availability_check_input(
    tenant_id: String,
    variant_id: String,
    requested_quantity: i32,
) -> InventoryAvailabilityCheckInput {
    InventoryAvailabilityCheckInput {
        tenant_id: tenant_id.trim().to_string(),
        variant_id: variant_id.trim().to_string(),
        requested_quantity,
    }
}

pub(crate) fn normalized_release_reservation_input(
    tenant_id: String,
    variant_id: String,
    quantity: i32,
) -> InventoryReleaseReservationInput {
    InventoryReleaseReservationInput {
        tenant_id: tenant_id.trim().to_string(),
        variant_id: variant_id.trim().to_string(),
        quantity,
    }
}

pub(crate) fn apply_variant_quantity_update(
    detail: &mut InventoryProductDetail,
    variant_id: &str,
    result: InventoryQuantityWriteResult,
) -> bool {
    let Some(variant) = detail
        .variants
        .iter_mut()
        .find(|variant| variant.id == variant_id)
    else {
        return false;
    };

    variant.inventory_quantity = result.quantity;
    variant.in_stock = result.in_stock;
    true
}

pub(crate) fn apply_variant_reservation_update(
    detail: &mut InventoryProductDetail,
    variant_id: &str,
    result: InventoryReservationWriteResult,
) -> bool {
    let Some(variant) = detail
        .variants
        .iter_mut()
        .find(|variant| variant.id == variant_id)
    else {
        return false;
    };

    variant.inventory_quantity = result.available_quantity;
    variant.in_stock = result.in_stock;
    true
}

pub(crate) fn apply_variant_reservation_release_update(
    detail: &mut InventoryProductDetail,
    variant_id: &str,
    result: InventoryReservationReleaseWriteResult,
) -> bool {
    let Some(variant) = detail
        .variants
        .iter_mut()
        .find(|variant| variant.id == variant_id)
    else {
        return false;
    };

    variant.inventory_quantity = result.available_quantity;
    variant.in_stock = result.in_stock;
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

pub(crate) fn parse_reserve_quantity(value: &str) -> Result<i32, String> {
    parse_non_negative_quantity(value, "reserve quantity")
}

pub(crate) fn parse_availability_quantity(value: &str) -> Result<i32, String> {
    parse_non_negative_quantity(value, "availability quantity")
}

fn parse_non_negative_quantity(value: &str, label: &str) -> Result<i32, String> {
    let quantity = parse_set_quantity(value)?;
    if quantity < 0 {
        return Err(format!("{label} must be non-negative"));
    }

    Ok(quantity)
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

pub(crate) fn status_badge(status: &str) -> &'static str {
    match status {
        "ACTIVE" => "border-emerald-200 bg-emerald-50 text-emerald-700",
        "ARCHIVED" => "border-slate-200 bg-slate-100 text-slate-700",
        _ => "border-amber-200 bg-amber-50 text-amber-700",
    }
}

pub fn inventory_grid_columns(locale: Option<&str>) -> Vec<GridColumnDef> {
    let is_ru = locale == Some("ru");
    vec![
        GridColumnDef::checkbox(),
        GridColumnDef::new("title", if is_ru { "Товар" } else { "Product" })
            .width(200)
            .min_width(140)
            .align(ColumnAlign::Left)
            .filter(GridFilterType::Text {
                placeholder: Some(if is_ru {
                    "Поиск по названию...".to_string()
                } else {
                    "Search title...".to_string()
                }),
            }),
        GridColumnDef::new("status", if is_ru { "Статус" } else { "Status" })
            .width(120)
            .align(ColumnAlign::Center)
            .filter(GridFilterType::Select {
                options: vec![
                    FilterOption {
                        value: "".to_string(),
                        label: if is_ru { "Все статусы".to_string() } else { "All statuses".to_string() },
                    },
                    FilterOption {
                        value: "ACTIVE".to_string(),
                        label: if is_ru { "Активен".to_string() } else { "Active".to_string() },
                    },
                    FilterOption {
                        value: "DRAFT".to_string(),
                        label: if is_ru { "Черновик".to_string() } else { "Draft".to_string() },
                    },
                    FilterOption {
                        value: "ARCHIVED".to_string(),
                        label: if is_ru { "В архиве".to_string() } else { "Archived".to_string() },
                    },
                ],
                placeholder: Some(if is_ru { "Все статусы".to_string() } else { "All statuses".to_string() }),
            }),
        GridColumnDef::new("product_type", if is_ru { "Тип" } else { "Type" })
            .width(120)
            .align(ColumnAlign::Left)
            .filter(GridFilterType::Text {
                placeholder: Some(if is_ru {
                    "Тип товара...".to_string()
                } else {
                    "Product type...".to_string()
                }),
            }),
        GridColumnDef::new("vendor", if is_ru { "Поставщик" } else { "Vendor" })
            .width(120)
            .align(ColumnAlign::Left)
            .filter(GridFilterType::Text {
                placeholder: Some(if is_ru {
                    "Поставщик...".to_string()
                } else {
                    "Vendor...".to_string()
                }),
            }),
        GridColumnDef::new("shipping_profile_slug", if is_ru { "Профиль" } else { "Profile" })
            .width(130)
            .align(ColumnAlign::Left)
            .filter(GridFilterType::Text {
                placeholder: Some(if is_ru {
                    "Профиль доставки...".to_string()
                } else {
                    "Shipping profile...".to_string()
                }),
            }),
        GridColumnDef::new("created_at", if is_ru { "Создан" } else { "Created" })
            .width(120)
            .align(ColumnAlign::Left)
            .filter(GridFilterType::DateRange {
                from_placeholder: Some(if is_ru { "С".to_string() } else { "From".to_string() }),
                to_placeholder: Some(if is_ru { "По".to_string() } else { "To".to_string() }),
            }),
        GridColumnDef::new("actions", "")
            .width(80)
            .align(ColumnAlign::Center)
            .not_sortable()
            .not_resizable()
            .not_filterable(),
    ]
}

pub fn matches_inventory_filter(
    item: &InventoryProductListItem,
    col_id: &str,
    filter_val: &FilterValue,
) -> bool {
    match (col_id, filter_val) {
        ("title", FilterValue::Text(query)) => {
            let q = query.trim().to_lowercase();
            q.is_empty()
                || item.title.to_lowercase().contains(&q)
                || item.handle.to_lowercase().contains(&q)
        }
        ("status", FilterValue::Select(val)) => {
            val.is_empty() || item.status.eq_ignore_ascii_case(val)
        }
        ("product_type", FilterValue::Text(query)) => {
            let q = query.trim().to_lowercase();
            q.is_empty()
                || item
                    .product_type
                    .as_deref()
                    .map(|t| t.to_lowercase().contains(&q))
                    .unwrap_or(false)
        }
        ("vendor", FilterValue::Text(query)) => {
            let q = query.trim().to_lowercase();
            q.is_empty()
                || item
                    .vendor
                    .as_deref()
                    .map(|v| v.to_lowercase().contains(&q))
                    .unwrap_or(false)
        }
        ("shipping_profile_slug", FilterValue::Text(query)) => {
            let q = query.trim().to_lowercase();
            q.is_empty()
                || item
                    .shipping_profile_slug
                    .as_deref()
                    .map(|p| p.to_lowercase().contains(&q))
                    .unwrap_or(false)
        }
        ("created_at", FilterValue::DateRange { from, to }) => {
            let item_date = item
                .created_at
                .split('T')
                .next()
                .unwrap_or(&item.created_at);
            if let Some(f) = from {
                if !f.trim().is_empty() && item_date < f.as_str() {
                    return false;
                }
            }
            if let Some(t) = to {
                if !t.trim().is_empty() && item_date > t.as_str() {
                    return false;
                }
            }
            true
        }
        _ => true,
    }
}

pub fn filter_inventory_products(
    items: &[InventoryProductListItem],
    filters: &ColumnFilters,
) -> Vec<InventoryProductListItem> {
    if filters.is_empty() {
        return items.to_vec();
    }
    items
        .iter()
        .filter(|item| {
            for (col_id, filter_val) in filters.iter() {
                if !matches_inventory_filter(item, col_id, filter_val) {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect()
}

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
            combination_identity: None,
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
    fn normalized_adjust_quantity_input_trims_route_identifiers_without_changing_adjustment() {
        let input = normalized_adjust_quantity_input(
            " tenant-id ".to_string(),
            " variant-id ".to_string(),
            -4,
        );

        assert_eq!(input.tenant_id, "tenant-id");
        assert_eq!(input.variant_id, "variant-id");
        assert_eq!(input.adjustment, -4);
    }

    #[test]
    fn normalized_reserve_quantity_input_trims_route_identifiers_without_changing_quantity() {
        let input = normalized_reserve_quantity_input(
            " tenant-id ".to_string(),
            " variant-id ".to_string(),
            3,
        );

        assert_eq!(input.tenant_id, "tenant-id");
        assert_eq!(input.variant_id, "variant-id");
        assert_eq!(input.quantity, 3);
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
    fn normalized_release_reservation_input_trims_context() {
        let input = normalized_release_reservation_input(
            " tenant-id ".to_string(),
            " variant-id ".to_string(),
            2,
        );

        assert_eq!(input.tenant_id, "tenant-id");
        assert_eq!(input.variant_id, "variant-id");
        assert_eq!(input.quantity, 2);
    }

    #[test]
    fn normalized_availability_check_input_trims_context() {
        let input = normalized_availability_check_input(
            " tenant-id ".to_string(),
            " variant-id ".to_string(),
            4,
        );

        assert_eq!(input.tenant_id, "tenant-id");
        assert_eq!(input.variant_id, "variant-id");
        assert_eq!(input.requested_quantity, 4);
    }

    #[test]
    fn parse_reserve_quantity_rejects_negative_values() {
        assert_eq!(parse_reserve_quantity(" 3 "), Ok(3));
        assert_eq!(
            parse_reserve_quantity("-1"),
            Err("reserve quantity must be non-negative".to_string())
        );
    }

    #[test]
    fn parse_availability_quantity_rejects_negative_values_with_domain_label() {
        assert_eq!(parse_availability_quantity(" 4 "), Ok(4));
        assert_eq!(
            parse_availability_quantity("-1"),
            Err("availability quantity must be non-negative".to_string())
        );
    }

    #[test]
    fn apply_variant_quantity_update_updates_quantity_and_stock_flag() {
        let mut detail = detail_with_variants(vec![variant(true, "deny", 2)]);

        assert!(apply_variant_quantity_update(
            &mut detail,
            "variant-2-deny",
            InventoryQuantityWriteResult {
                quantity: 0,
                in_stock: false,
            },
        ));
        assert_eq!(detail.variants[0].inventory_quantity, 0);
        assert!(!detail.variants[0].in_stock);

        assert!(apply_variant_quantity_update(
            &mut detail,
            "variant-2-deny",
            InventoryQuantityWriteResult {
                quantity: 7,
                in_stock: true,
            },
        ));
        assert_eq!(detail.variants[0].inventory_quantity, 7);
        assert!(detail.variants[0].in_stock);
    }

    #[test]
    fn apply_variant_reservation_release_update_uses_available_quantity_and_stock_flag() {
        let mut detail = detail_with_variants(vec![variant(false, "deny", 1)]);

        assert!(apply_variant_reservation_release_update(
            &mut detail,
            "variant-1-deny",
            InventoryReservationReleaseWriteResult {
                released_quantity: 2,
                available_quantity: 7,
                in_stock: true,
            },
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
            InventoryQuantityWriteResult {
                quantity: 9,
                in_stock: true,
            },
        ));
        assert_eq!(detail.variants[0].inventory_quantity, 2);
    }

    #[test]
    fn apply_variant_reservation_update_uses_available_quantity_and_stock_flag() {
        let mut detail = detail_with_variants(vec![variant(true, "deny", 10)]);

        assert!(apply_variant_reservation_update(
            &mut detail,
            "variant-10-deny",
            InventoryReservationWriteResult {
                reserved_quantity: 3,
                available_quantity: 7,
                in_stock: true,
            },
        ));

        assert_eq!(detail.variants[0].inventory_quantity, 7);
        assert!(detail.variants[0].in_stock);
    }

    #[test]
    fn apply_variant_quantity_update_uses_write_result_stock_flag_without_recomputing() {
        let mut detail = detail_with_variants(vec![variant(true, "deny", 2)]);

        assert!(apply_variant_quantity_update(
            &mut detail,
            "variant-2-deny",
            InventoryQuantityWriteResult {
                quantity: 3,
                in_stock: false,
            },
        ));

        assert_eq!(detail.variants[0].inventory_quantity, 3);
        assert!(
            !detail.variants[0].in_stock,
            "optimistic refresh must trust the module-owned write result instead of recomputing quantity > 0"
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

    #[test]
    fn test_status_badge_mapping() {
        assert_eq!(
            status_badge("ACTIVE"),
            "border-emerald-200 bg-emerald-50 text-emerald-700"
        );
        assert_eq!(
            status_badge("ARCHIVED"),
            "border-slate-200 bg-slate-100 text-slate-700"
        );
        assert_eq!(
            status_badge("DRAFT"),
            "border-amber-200 bg-amber-50 text-amber-700"
        );
    }

    #[test]
    fn filter_normalizers_trim_and_handle_empty_inputs() {
        assert_eq!(normalize_status_filter(Some("  ".to_string())), None);
        assert_eq!(
            normalize_status_filter(Some(" active ".to_string())),
            Some("ACTIVE".to_string())
        );

        assert_eq!(normalize_locale_filter(None), None);
        assert_eq!(normalize_locale_filter(Some("   ".to_string())), None);
        assert_eq!(
            normalize_locale_filter(Some(" en-US ".to_string())),
            Some("en-US".to_string())
        );

        assert_eq!(normalize_search_filter(None), None);
        assert_eq!(normalize_search_filter(Some("  ".to_string())), None);
        assert_eq!(
            normalize_search_filter(Some(" widget ".to_string())),
            Some("widget".to_string())
        );
    }

    #[test]
    fn inventory_grid_columns_localization() {
        let cols_en = inventory_grid_columns(Some("en"));
        let cols_ru = inventory_grid_columns(Some("ru"));

        assert_eq!(cols_en.len(), cols_ru.len());
        assert_eq!(cols_en[0].id.as_str(), "__checkbox");
        assert_eq!(cols_en[1].title, "Product");
        assert_eq!(cols_ru[1].title, "Товар");
        assert_eq!(cols_en[2].title, "Status");
        assert_eq!(cols_ru[2].title, "Статус");
    }

    #[test]
    fn filter_inventory_products_by_title_and_handle() {
        let item1 = InventoryProductListItem {
            id: "p1".to_string(),
            status: "ACTIVE".to_string(),
            title: "Winter Parka".to_string(),
            handle: "winter-parka".to_string(),
            vendor: Some("Nordic Wear".to_string()),
            product_type: Some("Jacket".to_string()),
            shipping_profile_slug: Some("heavy".to_string()),
            tags: vec![],
            created_at: "2026-01-10T12:00:00Z".to_string(),
            published_at: None,
        };
        let item2 = InventoryProductListItem {
            id: "p2".to_string(),
            status: "DRAFT".to_string(),
            title: "Summer Tee".to_string(),
            handle: "summer-tee".to_string(),
            vendor: Some("Sunny Apparel".to_string()),
            product_type: Some("Shirt".to_string()),
            shipping_profile_slug: Some("standard".to_string()),
            tags: vec![],
            created_at: "2026-03-15T10:00:00Z".to_string(),
            published_at: None,
        };
        let items = vec![item1.clone(), item2.clone()];

        let mut filters = ColumnFilters::new();
        filters.set("title", FilterValue::Text("parka".to_string()));
        let res = filter_inventory_products(&items, &filters);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "p1");

        let mut status_filter = ColumnFilters::new();
        status_filter.set("status", FilterValue::Select("DRAFT".to_string()));
        let res2 = filter_inventory_products(&items, &status_filter);
        assert_eq!(res2.len(), 1);
        assert_eq!(res2[0].id, "p2");

        let mut type_filter = ColumnFilters::new();
        type_filter.set("product_type", FilterValue::Text("shirt".to_string()));
        let res3 = filter_inventory_products(&items, &type_filter);
        assert_eq!(res3.len(), 1);
        assert_eq!(res3[0].id, "p2");

        let mut vendor_filter = ColumnFilters::new();
        vendor_filter.set("vendor", FilterValue::Text("nordic".to_string()));
        let res4 = filter_inventory_products(&items, &vendor_filter);
        assert_eq!(res4.len(), 1);
        assert_eq!(res4[0].id, "p1");

        let mut date_filter = ColumnFilters::new();
        date_filter.set(
            "created_at",
            FilterValue::DateRange {
                from: Some("2026-02-01".to_string()),
                to: None,
            },
        );
        let res5 = filter_inventory_products(&items, &date_filter);
        assert_eq!(res5.len(), 1);
        assert_eq!(res5[0].id, "p2");
    }
}
