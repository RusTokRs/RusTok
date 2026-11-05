use rustok_ui_core::normalize_ui_text;

pub const DEFAULT_POST_SLUG: &str = "latest";
pub const DEFAULT_ROUTE_SEGMENT: &str = "blog";
pub const SELECTED_POST_QUERY_KEY: &str = "slug";
pub const POSTS_CURSOR_QUERY_KEY: &str = "postsCursor";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlogStorefrontRouteState {
    pub selected_slug: String,
    pub selected_slug_query_key: &'static str,
    pub route_segment: String,
    pub tag: Option<String>,
    pub category_id: Option<String>,
}

impl BlogStorefrontRouteState {
    pub fn with_filters(mut self, tag: Option<String>, category_id: Option<String>) -> Self {
        self.tag = tag.as_deref().and_then(normalize_ui_text);
        self.category_id = category_id.as_deref().and_then(normalize_ui_text);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlogStorefrontFetchRequest {
    pub post_slug: String,
    pub locale: Option<String>,
    pub tag: Option<String>,
    pub category_id: Option<String>,
    /// Opaque keyset cursor (`next_cursor` of the previous posts page).
    pub posts_after: Option<String>,
}

pub fn build_storefront_route_state(
    route_slug: Option<String>,
    route_segment: Option<String>,
) -> BlogStorefrontRouteState {
    BlogStorefrontRouteState {
        selected_slug: selected_slug_or_default(route_slug, DEFAULT_POST_SLUG),
        selected_slug_query_key: SELECTED_POST_QUERY_KEY,
        route_segment: route_segment_or_default(route_segment, DEFAULT_ROUTE_SEGMENT),
        tag: None,
        category_id: None,
    }
}

pub fn build_storefront_fetch_request(
    route_state: &BlogStorefrontRouteState,
    locale: Option<String>,
) -> BlogStorefrontFetchRequest {
    BlogStorefrontFetchRequest {
        post_slug: selected_slug_or_default(
            Some(route_state.selected_slug.clone()),
            DEFAULT_POST_SLUG,
        ),
        locale: locale.as_deref().and_then(normalize_ui_text),
        tag: route_state.tag.clone(),
        category_id: route_state.category_id.clone(),
        posts_after: None,
    }
}

pub fn selected_slug_or_default(value: Option<String>, default_slug: &str) -> String {
    value
        .as_deref()
        .and_then(normalize_ui_text)
        .unwrap_or_else(|| default_slug.to_string())
}

pub fn route_segment_or_default(value: Option<String>, default_segment: &str) -> String {
    value
        .as_deref()
        .and_then(normalize_ui_text)
        .unwrap_or_else(|| default_segment.to_string())
}

pub fn module_href(base: &str, slug: &str) -> String {
    format!("{base}?slug={slug}")
}

pub fn open_link_label(label: &str, slug: &str) -> String {
    format!("{label} {slug}")
}

pub struct PostLinkView {
    pub href: String,
    pub open_label: String,
}

pub fn post_link(base: &str, slug: &str, open_label: &str) -> (String, String) {
    (
        module_href(base, slug),
        self::open_link_label(open_label, slug),
    )
}

pub fn post_link_typed_view(base: &str, slug: &str, open_label: &str) -> PostLinkView {
    let (href, open_label) = post_link(base, slug, open_label);
    PostLinkView { href, open_label }
}

/// Preserve the selected post and other active filters while changing one filter.
/// Cursor state is deliberately excluded: a cursor belongs to the old filter set.
/// RSS route is mounted by the Leptos host, not by the generic module page.
pub fn feed_href(locale: Option<&str>) -> String {
    match locale.map(str::trim).filter(|value| !value.is_empty()) {
        Some(locale) => format!("/{locale}/blog/feed.xml"),
        None => "/blog/feed.xml".to_string(),
    }
}

pub fn filter_query_href(
    slug: Option<&str>,
    tag: Option<&str>,
    category: Option<&str>,
    search: Option<&str>,
) -> String {
    let mut pairs = Vec::new();
    for (key, value) in [
        ("slug", slug),
        ("tag", tag),
        ("category", category),
        ("q", search),
    ] {
        if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
            pairs.push(format!("{key}={}", percent_encode(value)));
        }
    }
    format!("?{}", pairs.join("&"))
}

pub fn percent_encode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(b as char);
            }
            _ => {
                result.push_str(&format!("%{:02X}", b));
            }
        }
    }
    result
}
