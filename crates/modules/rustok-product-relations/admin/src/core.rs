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
