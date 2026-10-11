use rustok_ui_core::normalize_ui_text;

use crate::model::BrandAdminShell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrandAdminTransportProfile {
    Native,
    Graphql,
}

impl BrandAdminTransportProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Graphql => "graphql",
        }
    }
}

pub fn selected_transport_profile(value: Option<&str>) -> BrandAdminTransportProfile {
    match normalize_ui_text(value.unwrap_or_default())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "graphql" => BrandAdminTransportProfile::Graphql,
        _ => BrandAdminTransportProfile::Native,
    }
}

pub fn build_brand_admin_shell(
    locale: Option<&str>,
    profile: BrandAdminTransportProfile,
) -> BrandAdminShell {
    use crate::i18n::t;

    BrandAdminShell {
        title: t(locale, "brand.title", "Brands"),
        subtitle: t(
            locale,
            "brand.shell.subtitle",
            "Manage brand catalog, manufacturers, and media presentation",
        ),
        empty_state: t(
            locale,
            "brand.shell.emptyState",
            "Brand transport is not mounted in this host yet",
        ),
        transport_profile: profile.as_str().to_string(),
    }
}

pub fn validate_brand_slug(slug: &str) -> Result<(), &'static str> {
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

pub fn validate_brand_name(name: &str) -> Result<(), &'static str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Name cannot be empty");
    }
    if trimmed.len() > 255 {
        return Err("Name cannot exceed 255 characters");
    }
    Ok(())
}

use crate::model::BrandAdminListItem;
use rustok_grid::{
    ColumnAlign, ColumnFilters, FilterOption, FilterValue, GridColumnDef, GridFilterType,
};

pub fn brand_grid_columns(locale: Option<&str>) -> Vec<GridColumnDef> {
    let is_ru = locale.map(|l| l.starts_with("ru")).unwrap_or(false);

    vec![
        GridColumnDef::new("name", if is_ru { "Бренд" } else { "Brand" })
            .width(240)
            .align(ColumnAlign::Left)
            .filter(GridFilterType::Text {
                placeholder: Some(if is_ru {
                    "Поиск по бренду...".to_string()
                } else {
                    "Search brand...".to_string()
                }),
            }),
        GridColumnDef::new("slug", if is_ru { "Слаг" } else { "Slug" })
            .width(160)
            .align(ColumnAlign::Left)
            .filter(GridFilterType::Text {
                placeholder: Some(if is_ru {
                    "Фильтр слага...".to_string()
                } else {
                    "Filter slug...".to_string()
                }),
            }),
        GridColumnDef::new("website", if is_ru { "Веб-сайт" } else { "Website" })
            .width(180)
            .align(ColumnAlign::Left)
            .not_sortable(),
        GridColumnDef::new("products_count", if is_ru { "Товары" } else { "Products" })
            .width(110)
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
                        value: "inactive".to_string(),
                        label: if is_ru {
                            "Неактивен".to_string()
                        } else {
                            "Inactive".to_string()
                        },
                    },
                ],
                placeholder: Some(if is_ru {
                    "Все статусы".to_string()
                } else {
                    "All statuses".to_string()
                }),
            }),
        GridColumnDef::new("updated_at", if is_ru { "Обновлен" } else { "Updated" })
            .width(140)
            .align(ColumnAlign::Right),
        GridColumnDef::new("actions", if is_ru { "Действия" } else { "Actions" })
            .width(120)
            .align(ColumnAlign::Right)
            .not_sortable(),
    ]
}

pub fn matches_brand_filter(brand: &BrandAdminListItem, filters: &ColumnFilters) -> bool {
    for (col_id, filter_val) in filters.iter() {
        match (col_id.as_str(), filter_val) {
            ("name", FilterValue::Text(q)) => {
                let q_lower = q.to_lowercase();
                if !brand.name.to_lowercase().contains(&q_lower)
                    && !brand.slug.to_lowercase().contains(&q_lower)
                {
                    return false;
                }
            }
            ("slug", FilterValue::Text(q)) => {
                if !brand.slug.to_lowercase().contains(&q.to_lowercase()) {
                    return false;
                }
            }
            ("status", FilterValue::Select(s)) => match s.as_str() {
                "active" if !brand.is_active => {
                    return false;
                }
                "inactive" if brand.is_active => {
                    return false;
                }
                _ => {}
            },
            _ => {}
        }
    }
    true
}

pub fn filter_brands(
    brands: &[BrandAdminListItem],
    filters: &ColumnFilters,
    search: Option<&str>,
) -> Vec<BrandAdminListItem> {
    let search_term = search
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());

    brands
        .iter()
        .filter(|brand| {
            if let Some(ref term) = search_term {
                let matches_global = brand.name.to_lowercase().contains(term)
                    || brand.slug.to_lowercase().contains(term)
                    || brand
                        .description
                        .as_deref()
                        .map(|d| d.to_lowercase().contains(term))
                        .unwrap_or(false)
                    || brand
                        .website_url
                        .as_deref()
                        .map(|w| w.to_lowercase().contains(term))
                        .unwrap_or(false);
                if !matches_global {
                    return false;
                }
            }

            matches_brand_filter(brand, filters)
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
            BrandAdminTransportProfile::Graphql
        );
        assert_eq!(
            selected_transport_profile(Some("native")),
            BrandAdminTransportProfile::Native
        );
    }

    #[test]
    fn slug_validation_rules() {
        assert!(validate_brand_slug("apple").is_ok());
        assert!(validate_brand_slug("sony-electronics").is_ok());
        assert!(validate_brand_slug("samsung_2026").is_ok());
        assert!(validate_brand_slug("").is_err());
        assert!(validate_brand_slug("invalid slug with spaces").is_err());
        assert!(validate_brand_slug("bad@slug").is_err());
    }

    #[test]
    fn name_validation_rules() {
        assert!(validate_brand_name("Sony").is_ok());
        assert!(validate_brand_name("").is_err());
        assert!(validate_brand_name("   ").is_err());
    }

    #[test]
    fn brand_grid_columns_localization() {
        let cols_en = brand_grid_columns(Some("en"));
        assert_eq!(cols_en[0].title, "Brand");
        assert_eq!(cols_en[1].title, "Slug");

        let cols_ru = brand_grid_columns(Some("ru"));
        assert_eq!(cols_ru[0].title, "Бренд");
        assert_eq!(cols_ru[1].title, "Слаг");
    }

    #[test]
    fn filter_brands_by_search_and_status() {
        let b1 = BrandAdminListItem {
            id: "1".to_string(),
            slug: "sony".to_string(),
            name: "Sony Electronics".to_string(),
            is_active: true,
            products_count: 42,
            ..Default::default()
        };
        let b2 = BrandAdminListItem {
            id: "2".to_string(),
            slug: "panasonic".to_string(),
            name: "Panasonic".to_string(),
            is_active: false,
            products_count: 5,
            ..Default::default()
        };
        let list = vec![b1, b2];

        let mut filters = ColumnFilters::new();
        let filtered = filter_brands(&list, &filters, Some("sony"));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].slug, "sony");

        filters.set("status", FilterValue::Select("inactive".to_string()));
        let inactive = filter_brands(&list, &filters, None);
        assert_eq!(inactive.len(), 1);
        assert_eq!(inactive[0].slug, "panasonic");
    }
}
