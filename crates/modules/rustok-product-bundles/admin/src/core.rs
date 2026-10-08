use rustok_grid::{
    ColumnAlign, ColumnFilters, FilterOption, FilterValue, GridColumnDef, GridFilterType,
};
use rustok_ui_core::normalize_ui_text;

use crate::model::{BundleAdminListItem, BundleAdminShell};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BundleAdminTransportProfile {
    Native,
    Graphql,
}

impl BundleAdminTransportProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Graphql => "graphql",
        }
    }
}

pub fn selected_transport_profile(value: Option<&str>) -> BundleAdminTransportProfile {
    match normalize_ui_text(value.unwrap_or_default())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "graphql" => BundleAdminTransportProfile::Graphql,
        _ => BundleAdminTransportProfile::Native,
    }
}

pub fn build_bundle_admin_shell(
    locale: Option<&str>,
    profile: BundleAdminTransportProfile,
) -> BundleAdminShell {
    use crate::i18n::t;

    BundleAdminShell {
        title: t(locale, "bundle.title", "Product Bundles"),
        subtitle: t(
            locale,
            "bundle.shell.subtitle",
            "Manage product bundles, kits, and package discounts",
        ),
        empty_state: t(
            locale,
            "bundle.shell.emptyState",
            "Bundle transport is not mounted in this host yet",
        ),
        transport_profile: profile.as_str().to_string(),
    }
}

pub fn validate_bundle_slug(slug: &str) -> Result<(), &'static str> {
    let trimmed = slug.trim();
    if trimmed.is_empty() {
        return Err("Slug cannot be empty");
    }
    if trimmed.len() > 100 {
        return Err("Slug cannot exceed 100 characters");
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("Slug can only contain alphanumeric characters, hyphens, and underscores");
    }
    Ok(())
}

pub fn validate_bundle_name(name: &str) -> Result<(), &'static str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Name cannot be empty");
    }
    if trimmed.len() > 255 {
        return Err("Name cannot exceed 255 characters");
    }
    Ok(())
}

pub fn validate_bundle_discount(
    discount_type: &str,
    discount_value: &str,
) -> Result<(), &'static str> {
    if discount_type == "none" || discount_type.is_empty() {
        return Ok(());
    }

    let val = discount_value
        .trim()
        .parse::<f64>()
        .map_err(|_| "Discount value must be a number")?;
    if val < 0.0 {
        return Err("Discount value cannot be negative");
    }

    if discount_type == "percentage" && val > 100.0 {
        return Err("Percentage discount cannot exceed 100%");
    }

    Ok(())
}

pub fn bundle_grid_columns(locale: Option<&str>) -> Vec<GridColumnDef> {
    let is_ru = locale.map(|l| l.starts_with("ru")).unwrap_or(false);
    vec![
        GridColumnDef::new("name", if is_ru { "Название" } else { "Name" })
            .width(220)
            .align(ColumnAlign::Left)
            .filter(GridFilterType::Text {
                placeholder: Some(if is_ru {
                    "Фильтр названия...".to_string()
                } else {
                    "Filter name...".to_string()
                }),
            }),
        GridColumnDef::new("slug", if is_ru { "Slug" } else { "Slug" })
            .width(160)
            .align(ColumnAlign::Left),
        GridColumnDef::new("type", if is_ru { "Тип" } else { "Type" })
            .width(130)
            .align(ColumnAlign::Center)
            .filter(GridFilterType::Select {
                options: vec![
                    FilterOption {
                        value: "fixed".to_string(),
                        label: if is_ru {
                            "Фиксированный".to_string()
                        } else {
                            "Fixed".to_string()
                        },
                    },
                    FilterOption {
                        value: "custom".to_string(),
                        label: if is_ru {
                            "Настраиваемый".to_string()
                        } else {
                            "Custom".to_string()
                        },
                    },
                ],
                placeholder: Some(if is_ru {
                    "Все типы".to_string()
                } else {
                    "All types".to_string()
                }),
            }),
        GridColumnDef::new("discount", if is_ru { "Скидка" } else { "Discount" })
            .width(140)
            .align(ColumnAlign::Right),
        GridColumnDef::new("items", if is_ru { "Товары" } else { "Items" })
            .width(90)
            .align(ColumnAlign::Right),
        GridColumnDef::new("status", if is_ru { "Статус" } else { "Status" })
            .width(120)
            .align(ColumnAlign::Center)
            .filter(GridFilterType::Select {
                options: vec![
                    FilterOption {
                        value: "active".to_string(),
                        label: if is_ru {
                            "Активен".to_string()
                        } else {
                            "Active".to_string()
                        },
                    },
                    FilterOption {
                        value: "draft".to_string(),
                        label: if is_ru {
                            "Черновик".to_string()
                        } else {
                            "Draft".to_string()
                        },
                    },
                    FilterOption {
                        value: "archived".to_string(),
                        label: if is_ru {
                            "В архиве".to_string()
                        } else {
                            "Archived".to_string()
                        },
                    },
                ],
                placeholder: Some(if is_ru {
                    "Все статусы".to_string()
                } else {
                    "All statuses".to_string()
                }),
            }),
        GridColumnDef::new("actions", if is_ru { "Действия" } else { "Actions" })
            .width(140)
            .align(ColumnAlign::Right)
            .not_sortable(),
    ]
}

pub fn matches_bundle_filter(item: &BundleAdminListItem, filters: &ColumnFilters) -> bool {
    for (col_id, filter_val) in filters.iter() {
        match (col_id.as_str(), filter_val) {
            ("name", FilterValue::Text(q)) => {
                let term = q.to_lowercase();
                if !item.name.to_lowercase().contains(&term)
                    && !item.slug.to_lowercase().contains(&term)
                {
                    return false;
                }
            }
            ("type", FilterValue::Select(s)) => {
                if !item.bundle_type.eq_ignore_ascii_case(s) {
                    return false;
                }
            }
            ("status", FilterValue::Select(s)) => {
                if !item.status.eq_ignore_ascii_case(s) {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

pub fn filter_bundles(
    bundles: &[BundleAdminListItem],
    filters: &ColumnFilters,
    search: Option<&str>,
) -> Vec<BundleAdminListItem> {
    let search_term = search
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());

    bundles
        .iter()
        .filter(|item| {
            if let Some(ref term) = search_term {
                let matches_global = item.name.to_lowercase().contains(term)
                    || item.slug.to_lowercase().contains(term)
                    || item
                        .description
                        .as_deref()
                        .map(|d| d.to_lowercase().contains(term))
                        .unwrap_or(false);
                if !matches_global {
                    return false;
                }
            }

            matches_bundle_filter(item, filters)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_selection_is_explicit_without_automatic_fallback() {
        assert_eq!(
            selected_transport_profile(Some("graphql")),
            BundleAdminTransportProfile::Graphql
        );
        assert_eq!(
            selected_transport_profile(Some("native")),
            BundleAdminTransportProfile::Native
        );
        assert_eq!(
            selected_transport_profile(None),
            BundleAdminTransportProfile::Native
        );
    }

    #[test]
    fn slug_validation_rules() {
        assert!(validate_bundle_slug("starter-kit").is_ok());
        assert!(validate_bundle_slug("combo_2026").is_ok());
        assert!(validate_bundle_slug("").is_err());
        assert!(validate_bundle_slug("spaces in slug").is_err());
        assert!(validate_bundle_slug("bad@slug").is_err());
    }

    #[test]
    fn discount_validation_rules() {
        assert!(validate_bundle_discount("none", "").is_ok());
        assert!(validate_bundle_discount("percentage", "15").is_ok());
        assert!(validate_bundle_discount("percentage", "105").is_err());
        assert!(validate_bundle_discount("fixed_amount", "500").is_ok());
        assert!(validate_bundle_discount("fixed_amount", "-10").is_err());
        assert!(validate_bundle_discount("percentage", "not_a_number").is_err());
    }

    #[test]
    fn bundle_grid_columns_localization() {
        let cols_en = bundle_grid_columns(Some("en"));
        assert_eq!(cols_en[0].title, "Name");
        assert_eq!(cols_en[2].title, "Type");
        assert_eq!(cols_en[5].title, "Status");

        let cols_ru = bundle_grid_columns(Some("ru"));
        assert_eq!(cols_ru[0].title, "Название");
        assert_eq!(cols_ru[2].title, "Тип");
        assert_eq!(cols_ru[5].title, "Статус");
    }

    #[test]
    fn filter_bundles_by_search_and_status() {
        let b1 = BundleAdminListItem {
            id: "b1".into(),
            tenant_id: "t1".into(),
            slug: "starter-bundle".into(),
            name: "Starter Kit".into(),
            description: Some("Everything to begin".into()),
            bundle_type: "fixed".into(),
            status: "active".into(),
            discount_type: "percentage".into(),
            discount_value: "10".into(),
            items_count: 3,
            created_at: "2026-01-01".into(),
            updated_at: "2026-01-02".into(),
        };
        let b2 = BundleAdminListItem {
            id: "b2".into(),
            tenant_id: "t1".into(),
            slug: "pro-kit".into(),
            name: "Pro Pack".into(),
            description: None,
            bundle_type: "custom".into(),
            status: "draft".into(),
            discount_type: "none".into(),
            discount_value: "0".into(),
            items_count: 5,
            created_at: "2026-01-03".into(),
            updated_at: "2026-01-04".into(),
        };
        let list = vec![b1.clone(), b2.clone()];

        // Filter by search
        let res = filter_bundles(&list, &ColumnFilters::new(), Some("starter"));
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "b1");

        // Filter by status
        let mut filters = ColumnFilters::new();
        filters.set("status", FilterValue::Select("draft".into()));
        let res = filter_bundles(&list, &filters, None);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "b2");

        // Filter by type
        let mut filters = ColumnFilters::new();
        filters.set("type", FilterValue::Select("fixed".into()));
        let res = filter_bundles(&list, &filters, None);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "b1");
    }
}
