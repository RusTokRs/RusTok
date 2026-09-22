use rustok_ui_core::normalize_ui_text;

use crate::model::ProductRelationsPanelCopy;

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

pub fn build_product_relations_panel_copy(
    locale: Option<&str>,
) -> ProductRelationsPanelCopy {
    let russian = crate::i18n::normalize_admin_locale(locale) == "ru";

    if russian {
        ProductRelationsPanelCopy {
            title: "Связанные товары".to_string(),
            subtitle: "Сопутствующие товары, апселлы, аксессуары и рекомендации.".to_string(),
            tab_cross_sell: "Сопутствующие".to_string(),
            tab_up_sell: "Апселлы".to_string(),
            tab_related: "Похожие".to_string(),
            tab_accessory: "Аксессуары".to_string(),
            tab_alternative: "Аналоги".to_string(),
            add: "Добавить связь".to_string(),
            target_product_id: "ID связанного товара (UUID)".to_string(),
            position: "Позиция".to_string(),
            empty: "Нет настроенных связей этого типа.".to_string(),
            remove: "Удалить".to_string(),
            move_up: "Вверх".to_string(),
            move_down: "Вниз".to_string(),
        }
    } else {
        ProductRelationsPanelCopy {
            title: "Product Relations".to_string(),
            subtitle: "Cross-sells, up-sells, accessories, and merchandising associations.".to_string(),
            tab_cross_sell: "Cross-sell".to_string(),
            tab_up_sell: "Up-sell".to_string(),
            tab_related: "Related".to_string(),
            tab_accessory: "Accessories".to_string(),
            tab_alternative: "Alternatives".to_string(),
            add: "Add relation".to_string(),
            target_product_id: "Target Product ID (UUID)".to_string(),
            position: "Position".to_string(),
            empty: "No relations configured for this type.".to_string(),
            remove: "Remove".to_string(),
            move_up: "Move up".to_string(),
            move_down: "Move down".to_string(),
        }
    }
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
}
