use rustok_grid::{
    ColumnAlign, ColumnFilters, FilterOption, FilterValue, GridColumnDef, GridFilterType,
};

use crate::model::PricingProductListItem;

pub fn pricing_grid_columns(locale: Option<&str>) -> Vec<GridColumnDef> {
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

pub fn matches_pricing_filter(
    item: &PricingProductListItem,
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

pub fn filter_pricing_products(
    items: &[PricingProductListItem],
    filters: &ColumnFilters,
) -> Vec<PricingProductListItem> {
    if filters.is_empty() {
        return items.to_vec();
    }
    items
        .iter()
        .filter(|item| {
            for (col_id, filter_val) in filters.iter() {
                if !matches_pricing_filter(item, col_id, filter_val) {
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

    #[test]
    fn pricing_grid_columns_localization() {
        let cols_en = pricing_grid_columns(Some("en"));
        let cols_ru = pricing_grid_columns(Some("ru"));

        assert_eq!(cols_en.len(), cols_ru.len());
        assert_eq!(cols_en[0].id.as_str(), "__checkbox");
        assert_eq!(cols_en[1].title, "Product");
        assert_eq!(cols_ru[1].title, "Товар");
        assert_eq!(cols_en[2].title, "Status");
        assert_eq!(cols_ru[2].title, "Статус");
    }

    #[test]
    fn filter_pricing_products_by_title_and_status() {
        let item1 = PricingProductListItem {
            id: "p1".to_string(),
            status: "ACTIVE".to_string(),
            seller_id: None,
            title: "Pro Coffee Maker".to_string(),
            handle: "pro-coffee-maker".to_string(),
            vendor: Some("BrewCorp".to_string()),
            product_type: Some("Appliance".to_string()),
            shipping_profile_slug: Some("fragile".to_string()),
            tags: vec![],
            created_at: "2026-02-10T12:00:00Z".to_string(),
            published_at: None,
        };
        let item2 = PricingProductListItem {
            id: "p2".to_string(),
            status: "DRAFT".to_string(),
            seller_id: None,
            title: "Coffee Beans 1kg".to_string(),
            handle: "coffee-beans-1kg".to_string(),
            vendor: Some("RoastWorks".to_string()),
            product_type: Some("Consumable".to_string()),
            shipping_profile_slug: Some("standard".to_string()),
            tags: vec![],
            created_at: "2026-04-01T10:00:00Z".to_string(),
            published_at: None,
        };
        let items = vec![item1.clone(), item2.clone()];

        let mut filters = ColumnFilters::new();
        filters.set("title", FilterValue::Text("maker".to_string()));
        let res = filter_pricing_products(&items, &filters);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "p1");

        let mut status_filter = ColumnFilters::new();
        status_filter.set("status", FilterValue::Select("DRAFT".to_string()));
        let res2 = filter_pricing_products(&items, &status_filter);
        assert_eq!(res2.len(), 1);
        assert_eq!(res2[0].id, "p2");
    }
}
