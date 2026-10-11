#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SearchScope {
    Catalog,
    Forum,
    Blog,
    All,
}

impl SearchScope {
    pub const ALL_SCOPES: [SearchScope; 4] = [
        SearchScope::Catalog,
        SearchScope::Forum,
        SearchScope::Blog,
        SearchScope::All,
    ];

    pub fn source_module(&self) -> Option<&'static str> {
        match self {
            Self::Catalog => Some("product"),
            Self::Forum => Some("forum"),
            Self::Blog => Some("blog"),
            Self::All => None,
        }
    }

    pub fn label(&self, is_ru: bool) -> &'static str {
        match (self, is_ru) {
            (Self::Catalog, true) => "Магазин",
            (Self::Catalog, false) => "Store",
            (Self::Forum, true) => "Форум",
            (Self::Forum, false) => "Forum",
            (Self::Blog, true) => "Блог",
            (Self::Blog, false) => "Blog",
            (Self::All, true) => "Везде",
            (Self::All, false) => "All",
        }
    }

    pub fn placeholder(&self, is_ru: bool) -> &'static str {
        match (self, is_ru) {
            (Self::Catalog, true) => "Поиск товаров...",
            (Self::Catalog, false) => "Search products...",
            (Self::Forum, true) => "Поиск по форуму...",
            (Self::Forum, false) => "Search forum...",
            (Self::Blog, true) => "Поиск статей...",
            (Self::Blog, false) => "Search blog...",
            (Self::All, true) => "Поиск по сайту...",
            (Self::All, false) => "Search everything...",
        }
    }
}

pub(super) fn resolve_default_scope(route_segment: Option<&str>) -> SearchScope {
    match route_segment.map(str::trim).filter(|s| !s.is_empty()) {
        Some("forum") => SearchScope::Forum,
        Some("blog") => SearchScope::Blog,
        Some("search") => SearchScope::All,
        Some("products") | Some("catalog") | None => SearchScope::Catalog,
        _ => SearchScope::All,
    }
}

pub(super) fn build_search_target_url(locale: &str, query: &str, scope: SearchScope) -> String {
    let normalized_locale = rustok_ui_core::normalize_ui_text(locale)
        .map(|s| s.to_lowercase())
        .unwrap_or_else(|| "en".to_string());
    let trimmed_query = query.trim();
    if trimmed_query.is_empty() {
        return format!("/{}/search", normalized_locale);
    }
    let encoded_query = serde_urlencoded::to_string([("q", trimmed_query)])
        .unwrap_or_else(|_| format!("q={trimmed_query}"));
    match scope.source_module() {
        Some(module) => format!(
            "/{}/search?{}&source_modules={}",
            normalized_locale, encoded_query, module
        ),
        None => format!("/{}/search?{}", normalized_locale, encoded_query),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct HeaderLinks {
    pub home_href: String,
    pub english_href: String,
    pub russian_href: String,
}

pub(super) fn build_header_links(locale: &str) -> HeaderLinks {
    HeaderLinks {
        home_href: storefront_root_for_locale(locale),
        english_href: storefront_root_for_locale("en"),
        russian_href: storefront_root_for_locale("ru"),
    }
}

fn storefront_root_for_locale(locale: &str) -> String {
    let normalized = rustok_ui_core::normalize_ui_text(locale)
        .map(|s| s.to_lowercase())
        .unwrap_or_else(|| "en".to_string());
    format!("/{}", normalized)
}

#[cfg(test)]
mod tests {
    use super::{
        SearchScope, build_header_links, build_search_target_url, resolve_default_scope,
    };

    #[test]
    fn header_links_normalize_home_and_language_targets() {
        let links = build_header_links(" RU ");

        assert_eq!(links.home_href, "/ru");
        assert_eq!(links.english_href, "/en");
        assert_eq!(links.russian_href, "/ru");
    }

    #[test]
    fn resolve_default_scope_identifies_module_contexts() {
        assert_eq!(resolve_default_scope(None), SearchScope::Catalog);
        assert_eq!(resolve_default_scope(Some("")), SearchScope::Catalog);
        assert_eq!(resolve_default_scope(Some("forum")), SearchScope::Forum);
        assert_eq!(resolve_default_scope(Some("  forum  ")), SearchScope::Forum);
        assert_eq!(resolve_default_scope(Some("blog")), SearchScope::Blog);
        assert_eq!(resolve_default_scope(Some("search")), SearchScope::All);
        assert_eq!(resolve_default_scope(Some("custom")), SearchScope::All);
    }

    #[test]
    fn build_search_target_url_respects_scopes() {
        assert_eq!(
            build_search_target_url("ru", "jacket", SearchScope::Catalog),
            "/ru/search?q=jacket&source_modules=product"
        );
        assert_eq!(
            build_search_target_url("ru", "rust async", SearchScope::Forum),
            "/ru/search?q=rust+async&source_modules=forum"
        );
        assert_eq!(
            build_search_target_url("en", "release notes", SearchScope::Blog),
            "/en/search?q=release+notes&source_modules=blog"
        );
        assert_eq!(
            build_search_target_url("en", "anything", SearchScope::All),
            "/en/search?q=anything"
        );
        assert_eq!(
            build_search_target_url("ru", "   ", SearchScope::Catalog),
            "/ru/search"
        );
    }
}
