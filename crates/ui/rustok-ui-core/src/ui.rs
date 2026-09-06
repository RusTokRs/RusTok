use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UiRouteContext {
    pub locale: Option<String>,
    pub route_segment: Option<String>,
    pub subpath: Option<String>,
    pub query: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiRouteQueryUpdate {
    Clear,
    Replace(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiRouteQueryIntent {
    Push { key: &'static str, value: String },
    Replace { key: &'static str, value: String },
    Clear { key: &'static str },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiRouteQueryWrite {
    pub updates: Vec<(&'static str, Option<String>)>,
    pub replace: bool,
}

impl UiRouteQueryUpdate {
    pub fn into_query_value(self) -> Option<String> {
        match self {
            Self::Clear => None,
            Self::Replace(value) => Some(value),
        }
    }
}

impl UiRouteQueryIntent {
    pub fn push(key: &'static str, value: impl Into<String>) -> Self {
        Self::Push {
            key,
            value: value.into(),
        }
    }

    pub fn replace(key: &'static str, value: impl Into<String>) -> Self {
        Self::Replace {
            key,
            value: value.into(),
        }
    }

    pub fn clear(key: &'static str) -> Self {
        Self::Clear { key }
    }

    pub fn into_write(self) -> UiRouteQueryWrite {
        match self {
            Self::Push { key, value } => UiRouteQueryWrite {
                updates: vec![(key, Some(value))],
                replace: false,
            },
            Self::Replace { key, value } => UiRouteQueryWrite {
                updates: vec![(key, Some(value))],
                replace: true,
            },
            Self::Clear { key } => UiRouteQueryWrite {
                updates: vec![(key, None)],
                replace: true,
            },
        }
    }
}

pub fn normalize_ui_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn normalize_optional_ui_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| normalize_ui_text(value.as_str()))
}

pub fn normalize_required_ui_text(value: String) -> String {
    value.trim().to_string()
}

pub fn parse_ui_csv(value: &str) -> Vec<String> {
    value.split(',').filter_map(normalize_ui_text).collect()
}

pub fn route_query_update_for_text(value: &str) -> UiRouteQueryUpdate {
    if value.trim().is_empty() {
        UiRouteQueryUpdate::Clear
    } else {
        UiRouteQueryUpdate::Replace(value.to_string())
    }
}

pub fn ui_busy_key(action: &str) -> String {
    action.trim().to_string()
}

pub fn ui_busy_key_with_id(action: &str, item_id: &str) -> String {
    format!("{}:{}", action.trim(), item_id.trim())
}

pub fn ui_optional_busy_key_with_id(action: &str, item_id: Option<&str>) -> String {
    match item_id.and_then(normalize_ui_text) {
        Some(item_id) => ui_busy_key_with_id(action, item_id.as_str()),
        None => ui_busy_key(action),
    }
}

pub fn ui_busy_key_matches_action(busy_key: Option<&str>, action: &str) -> bool {
    let action = action.trim();
    let prefix = format!("{action}:");
    busy_key
        .map(|key| key == action || key.starts_with(prefix.as_str()))
        .unwrap_or(false)
}

pub fn ui_busy_key_last_segment_matches(busy_key: Option<&str>, item_id: &str) -> bool {
    let Some(item_id) = normalize_ui_text(item_id) else {
        return false;
    };

    busy_key
        .and_then(|value| value.rsplit(':').next())
        .map(|busy_item_id| busy_item_id == item_id)
        .unwrap_or(false)
}

pub fn ui_scoped_busy_key(scope: &str, action: &str, item_id: Option<&str>) -> String {
    let scope = scope.trim();
    let action = action.trim();
    match item_id.and_then(normalize_ui_text) {
        Some(item_id) => format!("{scope}:{action}:{item_id}"),
        None => format!("{scope}:{action}"),
    }
}

impl UiRouteContext {
    pub fn query_value(&self, key: &str) -> Option<&str> {
        self.query.get(key).map(String::as_str)
    }

    pub fn module_route_base(&self, route_segment: &str) -> String {
        let route_segment = route_segment.trim_matches('/');
        match self
            .locale
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(locale) if route_segment.is_empty() => format!("/{locale}/modules"),
            Some(locale) => format!("/{locale}/modules/{route_segment}"),
            None if route_segment.is_empty() => "/modules".to_string(),
            None => format!("/modules/{route_segment}"),
        }
    }

    pub fn subpath(&self) -> Option<&str> {
        self.subpath.as_deref()
    }

    pub fn subpath_matches(&self, prefix: &str) -> bool {
        self.subpath()
            .map(|subpath| subpath == prefix || subpath.starts_with(&format!("{prefix}/")))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        UiRouteContext, UiRouteQueryIntent, UiRouteQueryUpdate, normalize_optional_ui_text,
        normalize_required_ui_text, normalize_ui_text, parse_ui_csv, route_query_update_for_text,
        ui_busy_key, ui_busy_key_last_segment_matches, ui_busy_key_matches_action,
        ui_busy_key_with_id, ui_optional_busy_key_with_id, ui_scoped_busy_key,
    };

    #[test]
    fn module_route_base_uses_locale_prefix_when_present() {
        let route_context = UiRouteContext {
            locale: Some("fr".to_string()),
            ..Default::default()
        };

        assert_eq!(route_context.module_route_base("blog"), "/fr/modules/blog");
    }

    #[test]
    fn module_route_base_falls_back_to_legacy_path_without_locale() {
        let route_context = UiRouteContext::default();

        assert_eq!(route_context.module_route_base("pages"), "/modules/pages");
    }

    #[test]
    fn shared_ui_input_helpers_normalize_text_and_csv() {
        assert_eq!(
            normalize_ui_text("  catalog  "),
            Some("catalog".to_string())
        );
        assert_eq!(normalize_ui_text("   "), None);
        assert_eq!(
            parse_ui_csv(" product, blog ,, pages "),
            vec![
                "product".to_string(),
                "blog".to_string(),
                "pages".to_string()
            ]
        );
        assert_eq!(
            normalize_optional_ui_text(Some("  storefront  ".to_string())),
            Some("storefront".to_string())
        );
        assert_eq!(normalize_optional_ui_text(Some("   ".to_string())), None);
        assert_eq!(
            normalize_required_ui_text("  cart-1  ".to_string()),
            "cart-1".to_string()
        );
    }

    #[test]
    fn route_query_update_for_text_preserves_replacement_value() {
        assert_eq!(
            route_query_update_for_text("   "),
            UiRouteQueryUpdate::Clear
        );
        assert_eq!(
            route_query_update_for_text("  botas "),
            UiRouteQueryUpdate::Replace("  botas ".to_string())
        );
        assert_eq!(
            UiRouteQueryUpdate::Replace("value".to_string()).into_query_value(),
            Some("value".to_string())
        );
        assert_eq!(UiRouteQueryUpdate::Clear.into_query_value(), None);
    }

    #[test]
    fn route_query_intent_encodes_push_replace_and_clear_writes() {
        assert_eq!(
            UiRouteQueryIntent::push("product_id", "product-1").into_write(),
            super::UiRouteQueryWrite {
                updates: vec![("product_id", Some("product-1".to_string()))],
                replace: false,
            }
        );
        assert_eq!(
            UiRouteQueryIntent::replace("product_id", "product-2").into_write(),
            super::UiRouteQueryWrite {
                updates: vec![("product_id", Some("product-2".to_string()))],
                replace: true,
            }
        );
        assert_eq!(
            UiRouteQueryIntent::clear("product_id").into_write(),
            super::UiRouteQueryWrite {
                updates: vec![("product_id", None)],
                replace: true,
            }
        );
    }

    #[test]
    fn shared_busy_key_helpers_encode_common_ui_operations() {
        assert_eq!(ui_busy_key(" upload "), "upload");
        assert_eq!(ui_busy_key_with_id(" edit ", " item-1 "), "edit:item-1");
        assert_eq!(
            ui_optional_busy_key_with_id("save", Some(" item-2 ")),
            "save:item-2"
        );
        assert_eq!(ui_optional_busy_key_with_id("save", Some("   ")), "save");
        assert!(ui_busy_key_matches_action(Some("save:item-2"), "save"));
        assert!(ui_busy_key_matches_action(Some("save"), "save"));
        assert!(!ui_busy_key_matches_action(Some("publish:item-2"), "save"));
        assert!(ui_busy_key_last_segment_matches(
            Some("category:edit:item-2"),
            " item-2 "
        ));
        assert!(!ui_busy_key_last_segment_matches(
            Some("category:edit:item-20"),
            " item-2 "
        ));
        assert_eq!(
            ui_scoped_busy_key(" category ", " edit ", Some(" item-3 ")),
            "category:edit:item-3"
        );
        assert_eq!(
            ui_scoped_busy_key("category", "save", None),
            "category:save"
        );
    }
}
