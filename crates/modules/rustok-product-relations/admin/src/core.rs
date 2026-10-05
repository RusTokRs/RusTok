use rustok_grid::{ColumnAlign, GridColumnDef, GridFilterType};
use rustok_ui_core::normalize_ui_text;

use crate::model::{ProductRelationItem, ProductRelationsPanelCopy};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProductRelationsTransportProfile {
    Native,
    Graphql,
}

impl ProductRelationsTransportProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Graphql => "graphql",
        }
    }
}

pub fn selected_transport_profile(value: Option<&str>) -> ProductRelationsTransportProfile {
    match normalize_ui_text(value.unwrap_or_default())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "graphql" => ProductRelationsTransportProfile::Graphql,
        _ => ProductRelationsTransportProfile::Native,
    }
}

pub fn build_product_relations_panel_copy(locale: Option<&str>) -> ProductRelationsPanelCopy {
    use crate::i18n::t;

    ProductRelationsPanelCopy {
        title: t(locale, "relations.title", "Product Relations"),
        subtitle: t(
            locale,
            "relations.subtitle",
            "Cross-sells, up-sells, accessories, and merchandising associations.",
        ),
        tab_cross_sell: t(locale, "relations.tabCrossSell", "Cross-sell"),
        tab_up_sell: t(locale, "relations.tabUpSell", "Up-sell"),
        tab_related: t(locale, "relations.tabRelated", "Related"),
        tab_accessory: t(locale, "relations.tabAccessory", "Accessories"),
        tab_alternative: t(locale, "relations.tabAlternative", "Alternatives"),
        add: t(locale, "relations.add", "Add relation"),
        target_product_id: t(
            locale,
            "relations.targetProductId",
            "Target Product ID (UUID)",
        ),
        position: t(locale, "relations.position", "Position"),
        empty: t(
            locale,
            "relations.empty",
            "No relations configured for this type.",
        ),
        remove: t(locale, "relations.remove", "Remove"),
        move_up: t(locale, "relations.moveUp", "Move up"),
        move_down: t(locale, "relations.moveDown", "Move down"),
    }
}

pub fn relation_grid_columns(locale: Option<&str>) -> Vec<GridColumnDef> {
    use crate::i18n::t;

    let is_ru = locale.map(|l| l.starts_with("ru")).unwrap_or(false);

    vec![
        GridColumnDef::new("position", t(locale, "relations.position", "Position"))
            .width(100)
            .align(ColumnAlign::Left),
        GridColumnDef::new(
            "target_product_id",
            t(locale, "relations.targetProductId", "Target Product ID (UUID)"),
        )
        .align(ColumnAlign::Left)
        .filter(GridFilterType::Text {
            placeholder: Some(if is_ru {
                "Поиск по ID товара...".to_string()
            } else {
                "Filter product ID...".to_string()
            }),
        }),
        GridColumnDef::new("actions", t(locale, "relations.actions", "Actions"))
            .width(140)
            .align(ColumnAlign::Right)
            .not_sortable(),
    ]
}

pub fn matches_relation_filter(
    item: &ProductRelationItem,
    query: Option<&str>,
    relation_type: Option<&str>,
) -> bool {
    let matches_query = match normalize_ui_text(query.unwrap_or_default()) {
        Some(q) => {
            let q_lower = q.to_ascii_lowercase();
            item.related_product_id.to_ascii_lowercase().contains(&q_lower)
                || item.id.to_ascii_lowercase().contains(&q_lower)
                || item.product_id.to_ascii_lowercase().contains(&q_lower)
        }
        None => true,
    };

    let matches_type = match normalize_ui_text(relation_type.unwrap_or_default()) {
        Some(t) => item.relation_type.eq_ignore_ascii_case(&t),
        None => true,
    };

    matches_query && matches_type
}

pub fn filter_relations(
    items: &[ProductRelationItem],
    query: Option<&str>,
    relation_type: Option<&str>,
) -> Vec<ProductRelationItem> {
    items
        .iter()
        .filter(|item| matches_relation_filter(item, query, relation_type))
        .cloned()
        .collect()
}

pub fn validate_target_product_id(id: &str) -> Result<(), &'static str> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return Err("Target product ID cannot be empty");
    }
    if uuid::Uuid::parse_str(trimmed).is_err() {
        return Err("Target product ID must be a valid UUID");
    }
    Ok(())
}

pub fn is_valid_relation_type(rtype: &str) -> bool {
    matches!(
        rtype,
        "cross_sell" | "up_sell" | "related" | "accessory" | "alternative"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_resolution_bilingual() {
        let en = build_product_relations_panel_copy(Some("en"));
        assert_eq!(en.title, "Product Relations");
        assert_eq!(en.tab_cross_sell, "Cross-sell");

        let ru = build_product_relations_panel_copy(Some("ru"));
        assert_eq!(ru.title, "Связанные товары");
        assert_eq!(ru.tab_cross_sell, "Сопутствующие");
    }

    #[test]
    fn target_id_validation() {
        assert!(validate_target_product_id("a1a2a3a4-b1b2-c1c2-d1d2-d3d4d5d6d7d8").is_ok());
        assert!(validate_target_product_id("").is_err());
        assert!(validate_target_product_id("not-a-uuid").is_err());
    }

    #[test]
    fn relation_types() {
        assert!(is_valid_relation_type("cross_sell"));
        assert!(is_valid_relation_type("up_sell"));
        assert!(is_valid_relation_type("related"));
        assert!(is_valid_relation_type("accessory"));
        assert!(is_valid_relation_type("alternative"));
        assert!(!is_valid_relation_type("invalid_type"));
    }

    #[test]
    fn test_relation_grid_columns() {
        let cols_en = relation_grid_columns(Some("en"));
        assert_eq!(cols_en.len(), 3);
        assert_eq!(cols_en[0].id.as_str(), "position");
        assert_eq!(cols_en[0].title, "Position");
        assert!(cols_en[0].sortable);
        assert_eq!(cols_en[1].id.as_str(), "target_product_id");
        assert_eq!(cols_en[1].title, "Target Product ID (UUID)");
        assert_eq!(cols_en[2].id.as_str(), "actions");
        assert_eq!(cols_en[2].title, "Actions");
        assert!(!cols_en[2].sortable);

        let cols_ru = relation_grid_columns(Some("ru"));
        assert_eq!(cols_ru[0].title, "Позиция");
        assert_eq!(cols_ru[1].title, "ID связанного товара (UUID)");
        assert_eq!(cols_ru[2].title, "Действия");
    }

    #[test]
    fn test_filter_relations() {
        let items = vec![
            ProductRelationItem {
                id: "rel-1".to_string(),
                product_id: "prod-1".to_string(),
                related_product_id: "prod-2".to_string(),
                relation_type: "cross_sell".to_string(),
                position: 1,
                metadata: serde_json::Value::Null,
                created_at: String::new(),
                updated_at: String::new(),
            },
            ProductRelationItem {
                id: "rel-2".to_string(),
                product_id: "prod-1".to_string(),
                related_product_id: "prod-3".to_string(),
                relation_type: "up_sell".to_string(),
                position: 2,
                metadata: serde_json::Value::Null,
                created_at: String::new(),
                updated_at: String::new(),
            },
        ];

        let res = filter_relations(&items, Some("prod-2"), None);
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "rel-1");

        let res_type = filter_relations(&items, None, Some("up_sell"));
        assert_eq!(res_type.len(), 1);
        assert_eq!(res_type[0].id, "rel-2");

        let res_empty = filter_relations(&items, Some("nonexistent"), None);
        assert_eq!(res_empty.len(), 0);

        let res_all = filter_relations(&items, None, None);
        assert_eq!(res_all.len(), 2);
    }
}
