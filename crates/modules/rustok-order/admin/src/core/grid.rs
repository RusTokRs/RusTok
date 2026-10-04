use rustok_grid::{
    ColumnAlign, ColumnFilters, FilterOption, FilterValue, GridColumnDef, GridFilterType,
};

use crate::model::OrderListItem;

/// Returns the column definitions for the orders DataGrid, localized according to the current UI locale.
pub fn order_grid_columns(locale: Option<&str>) -> Vec<GridColumnDef> {
    let is_ru = locale == Some("ru");
    vec![
        GridColumnDef::checkbox(),
        GridColumnDef::new("id", if is_ru { "Заказ" } else { "Order" })
            .width(130)
            .min_width(100)
            .align(ColumnAlign::Left)
            .filter(GridFilterType::Text {
                placeholder: Some(if is_ru {
                    "Поиск по ID...".to_string()
                } else {
                    "Search ID...".to_string()
                }),
            }),
        GridColumnDef::new("created_at", if is_ru { "Дата" } else { "Date" })
            .width(140)
            .align(ColumnAlign::Left)
            .filter(GridFilterType::DateRange {
                from_placeholder: Some(if is_ru { "С".to_string() } else { "From".to_string() }),
                to_placeholder: Some(if is_ru { "По".to_string() } else { "To".to_string() }),
            }),
        GridColumnDef::new("customer", if is_ru { "Клиент" } else { "Customer" })
            .width(180)
            .min_width(140)
            .align(ColumnAlign::Left)
            .filter(GridFilterType::Text {
                placeholder: Some(if is_ru {
                    "Фильтр клиента...".to_string()
                } else {
                    "Filter customer...".to_string()
                }),
            }),
        GridColumnDef::new("status", if is_ru { "Статус" } else { "Status" })
            .width(130)
            .align(ColumnAlign::Center)
            .filter(GridFilterType::Select {
                options: vec![
                    FilterOption {
                        value: "".to_string(),
                        label: if is_ru {
                            "Все статусы".to_string()
                        } else {
                            "All statuses".to_string()
                        },
                    },
                    FilterOption {
                        value: "pending".to_string(),
                        label: if is_ru {
                            "Ожидает".to_string()
                        } else {
                            "Pending".to_string()
                        },
                    },
                    FilterOption {
                        value: "confirmed".to_string(),
                        label: if is_ru {
                            "Подтвержден".to_string()
                        } else {
                            "Confirmed".to_string()
                        },
                    },
                    FilterOption {
                        value: "paid".to_string(),
                        label: if is_ru {
                            "Оплачен".to_string()
                        } else {
                            "Paid".to_string()
                        },
                    },
                    FilterOption {
                        value: "shipped".to_string(),
                        label: if is_ru {
                            "Отправлен".to_string()
                        } else {
                            "Shipped".to_string()
                        },
                    },
                    FilterOption {
                        value: "delivered".to_string(),
                        label: if is_ru {
                            "Доставлен".to_string()
                        } else {
                            "Delivered".to_string()
                        },
                    },
                    FilterOption {
                        value: "cancelled".to_string(),
                        label: if is_ru {
                            "Отменен".to_string()
                        } else {
                            "Cancelled".to_string()
                        },
                    },
                ],
                placeholder: Some(if is_ru {
                    "Все статусы".to_string()
                } else {
                    "All statuses".to_string()
                }),
            }),
        GridColumnDef::new("items", if is_ru { "Состав" } else { "Items" })
            .width(220)
            .align(ColumnAlign::Left)
            .not_sortable()
            .not_filterable(),
        GridColumnDef::new("total", if is_ru { "Сумма" } else { "Total" })
            .width(120)
            .align(ColumnAlign::Right),
        GridColumnDef::new("actions", "")
            .width(90)
            .align(ColumnAlign::Center)
            .not_sortable()
            .not_resizable()
            .not_filterable(),
    ]
}

/// Evaluates if a single order satisfies the filter condition on the given column.
pub fn matches_order_filter(
    item: &OrderListItem,
    col_id: &str,
    filter_val: &FilterValue,
) -> bool {
    match (col_id, filter_val) {
        ("id", FilterValue::Text(query)) => {
            let q = query.trim().to_lowercase();
            q.is_empty() || item.id.to_lowercase().contains(&q)
        }
        ("customer", FilterValue::Text(query)) => {
            let q = query.trim().to_lowercase();
            q.is_empty()
                || item
                    .customer_id
                    .as_deref()
                    .map(|c| c.to_lowercase().contains(&q))
                    .unwrap_or(false)
        }
        ("status", FilterValue::Select(val)) => {
            if val.is_empty() {
                true
            } else {
                item.status.eq_ignore_ascii_case(val)
            }
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

/// Filters a list of orders in-memory according to the active column filters.
pub fn filter_orders(items: &[OrderListItem], filters: &ColumnFilters) -> Vec<OrderListItem> {
    if filters.is_empty() {
        return items.to_vec();
    }
    items
        .iter()
        .filter(|item| {
            for (col_id, filter_val) in filters.iter() {
                if !matches_order_filter(item, col_id, filter_val) {
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

    fn create_test_order(id: &str, status: &str, customer_id: Option<&str>, created_at: &str) -> OrderListItem {
        OrderListItem {
            id: id.to_string(),
            customer_id: customer_id.map(String::from),
            status: status.to_string(),
            currency_code: "USD".to_string(),
            total_amount: "150.00".to_string(),
            tracking_number: None,
            carrier: None,
            created_at: created_at.to_string(),
            confirmed_at: None,
            paid_at: None,
            shipped_at: None,
            delivered_at: None,
            cancelled_at: None,
            line_items: vec![],
        }
    }

    #[test]
    fn test_order_grid_columns_localization() {
        let cols_en = order_grid_columns(Some("en"));
        let cols_ru = order_grid_columns(Some("ru"));
        assert_eq!(cols_en.len(), cols_ru.len());
        assert!(cols_en[0].is_checkbox());
        assert_eq!(cols_en[1].title, "Order");
        assert_eq!(cols_ru[1].title, "Заказ");
        assert_eq!(cols_en[4].title, "Status");
        assert_eq!(cols_ru[4].title, "Статус");
    }

    #[test]
    fn test_filter_orders_by_id_and_status() {
        let orders = vec![
            create_test_order("ord-101", "paid", Some("cust-1"), "2026-05-10T12:00:00Z"),
            create_test_order("ord-102", "pending", Some("cust-2"), "2026-05-11T12:00:00Z"),
            create_test_order("ord-201", "delivered", Some("cust-1"), "2026-05-12T12:00:00Z"),
        ];

        let mut filters = ColumnFilters::new();
        filters.set("status", FilterValue::Select("paid".to_string()));
        let filtered = filter_orders(&orders, &filters);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "ord-101");

        let mut id_filter = ColumnFilters::new();
        id_filter.set("id", FilterValue::Text("201".to_string()));
        let filtered_id = filter_orders(&orders, &id_filter);
        assert_eq!(filtered_id.len(), 1);
        assert_eq!(filtered_id[0].id, "ord-201");
    }

    #[test]
    fn test_filter_orders_by_customer_and_date() {
        let orders = vec![
            create_test_order("ord-101", "paid", Some("cust-alpha"), "2026-05-01T12:00:00Z"),
            create_test_order("ord-102", "pending", Some("cust-beta"), "2026-05-15T12:00:00Z"),
            create_test_order("ord-103", "delivered", Some("cust-alpha"), "2026-05-20T12:00:00Z"),
        ];

        let mut filters = ColumnFilters::new();
        filters.set("customer", FilterValue::Text("alpha".to_string()));
        filters.set("created_at", FilterValue::DateRange {
            from: Some("2026-05-10".to_string()),
            to: None,
        });
        let filtered = filter_orders(&orders, &filters);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "ord-103");
    }
}
