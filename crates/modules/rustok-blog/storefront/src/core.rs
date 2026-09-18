use rustok_ui_core::normalize_ui_text;

use crate::i18n::t;

pub const DEFAULT_POST_SLUG: &str = "latest";
pub const DEFAULT_ROUTE_SEGMENT: &str = "blog";
pub const SELECTED_POST_QUERY_KEY: &str = "slug";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlogStorefrontRouteState {
    pub selected_slug: String,
    pub selected_slug_query_key: &'static str,
    pub route_segment: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlogStorefrontShellViewModel {
    pub badge: String,
    pub title: String,
    pub subtitle: String,
    pub load_error: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlogStorefrontFetchRequest {
    pub post_slug: String,
    pub locale: Option<String>,
}

pub fn build_storefront_route_state(
    route_slug: Option<String>,
    route_segment: Option<String>,
) -> BlogStorefrontRouteState {
    BlogStorefrontRouteState {
        selected_slug: selected_slug_or_default(route_slug, DEFAULT_POST_SLUG),
        selected_slug_query_key: SELECTED_POST_QUERY_KEY,
        route_segment: route_segment_or_default(route_segment, DEFAULT_ROUTE_SEGMENT),
    }
}

pub fn build_storefront_shell_view_model(locale: Option<&str>) -> BlogStorefrontShellViewModel {
    BlogStorefrontShellViewModel {
        badge: t(locale, "blog.badge", "blog"),
        title: t(
            locale,
            "blog.title",
            "Stories published from the module package",
        ),
        subtitle: t(
            locale,
            "blog.subtitle",
            "This storefront surface reads blog data through GraphQL with no host-specific blog wiring.",
        ),
        load_error: t(
            locale,
            "blog.error.load",
            "Failed to load blog storefront data",
        ),
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
    }
}

pub fn fallback_text(value: Option<String>, fallback: &str) -> String {
    value.unwrap_or_else(|| fallback.to_string())
}

pub fn count_label(total: u64, suffix: &str) -> String {
    format!("{total} {suffix}")
}

pub fn published_posts_total_label(total: u64, suffix: &str) -> String {
    count_label(total, suffix)
}

pub fn published_posts_header_view(
    title: String,
    total: u64,
    total_suffix: &str,
) -> (String, String) {
    (title, published_posts_total_label(total, total_suffix))
}

pub fn selected_post_empty_state_view(title: String, body: String) -> (String, String) {
    (title, body)
}

pub struct SelectedPostMetaView {
    pub slug_meta: String,
    pub locale_meta: String,
    pub published_meta: String,
    pub separator: &'static str,
}

pub struct SelectedPostTagsView {
    pub items: Vec<String>,
}

pub struct SelectedPostContentView {
    pub excerpt: String,
    pub body: String,
}

pub struct SelectedPostStatusView {
    pub status: String,
    pub unknown_label: String,
}

pub struct SelectedPostHeaderView {
    pub title: String,
    pub meta: SelectedPostMetaView,
    pub status: SelectedPostStatusView,
}

pub struct PublishedPostCardView {
    pub status: String,
    pub excerpt: String,
    pub href: String,
    pub open_label: String,
    pub locale_meta: String,
}

pub struct PublishedPostsHeaderView {
    pub title: String,
    pub total_label: String,
}

pub struct SelectedPostEmptyStateView {
    pub title: String,
    pub body: String,
}

pub struct PublishedPostsEmptyStateView {
    pub message: String,
}

pub struct StatusBadgeView {
    pub label: String,
    pub badge_css: &'static str,
}

pub struct PostLinkView {
    pub href: String,
    pub open_label: String,
}

pub enum PublishedPostsReadyView<T> {
    Items(Vec<T>),
    Empty(PublishedPostsEmptyStateView),
}

pub fn published_posts_header_typed_view(
    title: String,
    total: u64,
    total_suffix: &str,
) -> PublishedPostsHeaderView {
    let (title, total_label) = published_posts_header_view(title, total, total_suffix);
    PublishedPostsHeaderView { title, total_label }
}

pub fn selected_post_empty_state_typed_view(
    title: String,
    body: String,
) -> SelectedPostEmptyStateView {
    let (title, body) = selected_post_empty_state_view(title, body);
    SelectedPostEmptyStateView { title, body }
}

pub fn published_posts_empty_state_typed_view(message: String) -> PublishedPostsEmptyStateView {
    let (message,) = published_posts_empty_state_view(message);
    PublishedPostsEmptyStateView { message }
}

pub fn published_posts_ready_typed_view<T>(
    items: Vec<T>,
    empty_message: String,
) -> PublishedPostsReadyView<T> {
    match published_posts_ready_items(items, empty_message) {
        Ok(items) => PublishedPostsReadyView::Items(items),
        Err(message) => {
            PublishedPostsReadyView::Empty(published_posts_empty_state_typed_view(message))
        }
    }
}

pub fn selected_post_meta_view(
    slug_label: &str,
    slug: &str,
    locale_label: &str,
    effective_locale: &str,
    published_label: &str,
    published_at: &str,
) -> SelectedPostMetaView {
    let (slug_meta, locale_meta, published_meta, separator) = selected_post_meta_row(
        slug_label,
        slug,
        locale_label,
        effective_locale,
        published_label,
        published_at,
    );
    SelectedPostMetaView {
        slug_meta,
        locale_meta,
        published_meta,
        separator,
    }
}

pub fn selected_post_tags_view(tags: Vec<String>) -> Option<SelectedPostTagsView> {
    selected_post_tag_items(tags).map(|items| SelectedPostTagsView { items })
}

pub fn selected_post_content_view(excerpt: String, body: String) -> SelectedPostContentView {
    SelectedPostContentView { excerpt, body }
}

pub fn selected_post_status_view(status: String, unknown_label: String) -> SelectedPostStatusView {
    SelectedPostStatusView {
        status,
        unknown_label,
    }
}

pub fn selected_post_header_view(
    title: String,
    meta: SelectedPostMetaView,
    status: SelectedPostStatusView,
) -> SelectedPostHeaderView {
    SelectedPostHeaderView {
        title,
        meta,
        status,
    }
}

pub fn open_link_label(label: &str, slug: &str) -> String {
    format!("{label} {slug}")
}

pub fn label_value_pair(label: &str, value: &str) -> String {
    format!("{label}: {value}")
}

pub fn post_meta_pairs(
    slug_label: &str,
    slug: &str,
    locale_label: &str,
    effective_locale: &str,
    published_label: &str,
    published_at: &str,
) -> [String; 3] {
    [
        label_value_pair(slug_label, slug),
        label_value_pair(locale_label, effective_locale),
        label_value_pair(published_label, published_at),
    ]
}

pub fn meta_separator() -> &'static str {
    "·"
}

pub fn selected_post_meta_row(
    slug_label: &str,
    slug: &str,
    locale_label: &str,
    effective_locale: &str,
    published_label: &str,
    published_at: &str,
) -> (String, String, String, &'static str) {
    let [slug_meta, locale_meta, published_meta] = post_meta_pairs(
        slug_label,
        slug,
        locale_label,
        effective_locale,
        published_label,
        published_at,
    );
    (slug_meta, locale_meta, published_meta, meta_separator())
}

pub fn list_post_excerpt(post_excerpt: Option<String>, fallback: &str) -> String {
    fallback_excerpt(post_excerpt, fallback)
}

pub fn error_with_context(context: &str, error: &str) -> String {
    format!("{context}: {error}")
}

pub fn module_href(base: &str, slug: &str) -> String {
    format!("{base}?slug={slug}")
}

pub fn post_link(base: &str, slug: &str, open_label: &str) -> (String, String) {
    (module_href(base, slug), open_link_label(open_label, slug))
}

pub fn post_link_typed_view(base: &str, slug: &str, open_label: &str) -> PostLinkView {
    let (href, open_label) = post_link(base, slug, open_label);
    PostLinkView { href, open_label }
}

pub fn list_post_summary(
    slug: Option<String>,
    missing_slug_fallback: &str,
    excerpt: Option<String>,
    excerpt_fallback: &str,
    module_route_base: &str,
    open_label: &str,
) -> (String, String, String) {
    let resolved_slug = fallback_slug(slug, missing_slug_fallback);
    let resolved_excerpt = list_post_excerpt(excerpt, excerpt_fallback);
    let link_view = post_link_typed_view(module_route_base, resolved_slug.as_str(), open_label);
    (resolved_excerpt, link_view.href, link_view.open_label)
}

pub fn list_post_locale_meta(locale_label: &str, effective_locale: &str) -> String {
    label_value_pair(locale_label, effective_locale)
}

pub struct PublishedPostCardInput<'a> {
    pub slug: Option<String>,
    pub missing_slug_fallback: &'a str,
    pub excerpt: Option<String>,
    pub excerpt_fallback: &'a str,
    pub module_route_base: &'a str,
    pub open_label: &'a str,
    pub locale_label: &'a str,
    pub effective_locale: &'a str,
    pub status: String,
}

pub fn published_post_card_view(input: PublishedPostCardInput<'_>) -> PublishedPostCardView {
    let (resolved_excerpt, href, resolved_open_label) = list_post_summary(
        input.slug,
        input.missing_slug_fallback,
        input.excerpt,
        input.excerpt_fallback,
        input.module_route_base,
        input.open_label,
    );
    PublishedPostCardView {
        status: input.status,
        excerpt: resolved_excerpt,
        href,
        open_label: resolved_open_label,
        locale_meta: list_post_locale_meta(input.locale_label, input.effective_locale),
    }
}

pub fn fallback_slug(value: Option<String>, fallback: &str) -> String {
    fallback_text(value, fallback)
}

pub fn fallback_excerpt(value: Option<String>, fallback: &str) -> String {
    fallback_text(value, fallback)
}

pub fn selected_post_fallback_fields(
    slug: Option<String>,
    slug_fallback: &str,
    excerpt: Option<String>,
    excerpt_fallback: &str,
    published_at: Option<String>,
    published_at_fallback: &str,
) -> (String, String, String) {
    (
        fallback_slug(slug, slug_fallback),
        fallback_excerpt(excerpt, excerpt_fallback),
        fallback_text(published_at, published_at_fallback),
    )
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

pub fn status_badge_css(status: &str) -> &'static str {
    let status = status.trim();

    if status.eq_ignore_ascii_case("published") {
        "inline-flex rounded-full border border-emerald-300/50 bg-emerald-50 px-2.5 py-0.5 text-xs font-medium uppercase tracking-[0.18em] text-emerald-800 dark:border-emerald-700/40 dark:bg-emerald-900/25 dark:text-emerald-300"
    } else if status.eq_ignore_ascii_case("archived") {
        "inline-flex rounded-full border border-border bg-muted px-2.5 py-0.5 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground"
    } else {
        "inline-flex rounded-full border border-primary/30 bg-primary/10 px-2.5 py-0.5 text-xs font-medium uppercase tracking-[0.18em] text-primary"
    }
}

pub fn status_label(status: &str, fallback: &str) -> String {
    let normalized = status.trim();
    if normalized.is_empty() {
        fallback.to_string()
    } else {
        normalized.to_string()
    }
}

pub fn has_items<T>(items: &[T]) -> bool {
    !items.is_empty()
}

pub fn status_presentation(status: &str, fallback: &str) -> (String, &'static str) {
    let label = status_label(status, fallback);
    let badge_css = status_badge_css(label.as_str());
    (label, badge_css)
}

pub fn status_badge_view(status: String, unknown_label: &str) -> (String, &'static str) {
    status_presentation(status.as_str(), unknown_label)
}

pub fn status_badge_typed_view(status: String, unknown_label: &str) -> StatusBadgeView {
    let (label, badge_css) = status_badge_view(status, unknown_label);
    StatusBadgeView { label, badge_css }
}

pub fn selected_post_tag_items(tags: Vec<String>) -> Option<Vec<String>> {
    if has_items(tags.as_slice()) {
        Some(tags)
    } else {
        None
    }
}

pub fn published_posts_or_empty_message<T>(
    items: Vec<T>,
    empty_message: String,
) -> Result<Vec<T>, String> {
    if has_items(items.as_slice()) {
        Ok(items)
    } else {
        Err(empty_message)
    }
}

pub fn published_posts_view_state<T>(
    items: Vec<T>,
    empty_message: String,
) -> (Option<Vec<T>>, Option<String>) {
    match published_posts_or_empty_message(items, empty_message) {
        Ok(items) => (Some(items), None),
        Err(message) => (None, Some(message)),
    }
}

pub fn published_posts_items_or_default<T>(items: Option<Vec<T>>) -> Vec<T> {
    items.unwrap_or_default()
}

pub fn published_posts_ready_items<T>(
    items: Vec<T>,
    empty_message: String,
) -> Result<Vec<T>, String> {
    let (items, empty_message_opt) = published_posts_view_state(items, empty_message);
    let ready_items = published_posts_items_or_default(items);
    if has_items(ready_items.as_slice()) {
        Ok(ready_items)
    } else {
        Err(empty_message_opt.unwrap_or_default())
    }
}

pub fn published_posts_empty_state_message(message: String) -> String {
    message
}

pub fn published_posts_empty_state_view(message: String) -> (String,) {
    (published_posts_empty_state_message(message),)
}

#[cfg(test)]
mod tests;
