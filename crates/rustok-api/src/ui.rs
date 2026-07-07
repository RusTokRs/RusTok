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

impl UiRouteQueryUpdate {
    pub fn into_query_value(self) -> Option<String> {
        match self {
            Self::Clear => None,
            Self::Replace(value) => Some(value),
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
        normalize_ui_text, parse_ui_csv, route_query_update_for_text, UiRouteContext,
        UiRouteQueryUpdate,
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
}
