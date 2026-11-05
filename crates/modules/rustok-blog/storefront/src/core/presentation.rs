use super::routing::post_link_typed_view;
use crate::i18n::t;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostStatusTone {
    Published,
    Archived,
    Default,
}

pub fn post_status_tone(status: &str) -> PostStatusTone {
    let normalized = status.trim();
    if normalized.eq_ignore_ascii_case("published") {
        PostStatusTone::Published
    } else if normalized.eq_ignore_ascii_case("archived") {
        PostStatusTone::Archived
    } else {
        PostStatusTone::Default
    }
}

pub fn status_badge_css(status: &str) -> &'static str {
    match post_status_tone(status) {
        PostStatusTone::Published => {
            "inline-flex rounded-full border border-emerald-300/50 bg-emerald-50 px-2.5 py-0.5 text-xs font-medium uppercase tracking-[0.18em] text-emerald-800 dark:border-emerald-700/40 dark:bg-emerald-900/25 dark:text-emerald-300"
        }
        PostStatusTone::Archived => {
            "inline-flex rounded-full border border-border bg-muted px-2.5 py-0.5 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground"
        }
        PostStatusTone::Default => {
            "inline-flex rounded-full border border-primary/30 bg-primary/10 px-2.5 py-0.5 text-xs font-medium uppercase tracking-[0.18em] text-primary"
        }
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

pub struct StatusBadgeView {
    pub label: String,
    pub badge_css: &'static str,
}

pub fn status_badge_typed_view(status: String, unknown_label: &str) -> StatusBadgeView {
    let (label, badge_css) = status_badge_view(status, unknown_label);
    StatusBadgeView { label, badge_css }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlogStorefrontShellViewModel {
    pub badge: String,
    pub title: String,
    pub subtitle: String,
    pub load_error: String,
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

pub fn fallback_text(value: Option<String>, fallback: &str) -> String {
    value.unwrap_or_else(|| fallback.to_string())
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

pub fn count_label(total: u64, suffix: &str) -> String {
    format!("{total} {suffix}")
}

pub fn error_with_context(context: &str, error: &str) -> String {
    format!("{context}: {error}")
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

pub struct SelectedPostEmptyStateView {
    pub title: String,
    pub body: String,
}

pub struct PublishedPostsEmptyStateView {
    pub message: String,
}

pub enum PublishedPostsReadyView<T> {
    Items(Vec<T>),
    Empty(PublishedPostsEmptyStateView),
}

pub fn selected_post_empty_state_view(title: String, body: String) -> (String, String) {
    (title, body)
}

pub fn selected_post_empty_state_typed_view(
    title: String,
    body: String,
) -> SelectedPostEmptyStateView {
    let (title, body) = selected_post_empty_state_view(title, body);
    SelectedPostEmptyStateView { title, body }
}

pub fn published_posts_empty_state_message(message: String) -> String {
    message
}

pub fn published_posts_empty_state_view(message: String) -> (String,) {
    (published_posts_empty_state_message(message),)
}

pub fn published_posts_empty_state_typed_view(message: String) -> PublishedPostsEmptyStateView {
    let (message,) = published_posts_empty_state_view(message);
    PublishedPostsEmptyStateView { message }
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

pub fn label_value_pair(label: &str, value: &str) -> String {
    format!("{label}: {value}")
}

pub fn meta_separator() -> &'static str {
    "·"
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

pub fn selected_post_tag_items(tags: Vec<String>) -> Option<Vec<String>> {
    if has_items(tags.as_slice()) {
        Some(tags)
    } else {
        None
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

pub fn list_post_excerpt(post_excerpt: Option<String>, fallback: &str) -> String {
    fallback_excerpt(post_excerpt, fallback)
}

pub fn list_post_locale_meta(locale_label: &str, effective_locale: &str) -> String {
    label_value_pair(locale_label, effective_locale)
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

/// Extracts a draft preview token from a URL fragment (e.g. `#token=…` or `token=…`).
/// Returns `None` if the fragment does not contain a non-empty `token` parameter.
#[allow(dead_code)]
pub fn parse_preview_token_from_fragment(fragment: &str) -> Option<String> {
    let raw = fragment.trim().strip_prefix('#').unwrap_or(fragment.trim());
    if raw.is_empty() {
        return None;
    }
    for part in raw.split('&') {
        let mut key_val = part.splitn(2, '=');
        let key = key_val.next().map(str::trim)?;
        let val = key_val.next().map(str::trim);
        if key == "token"
            && let Some(val) = val
        {
            let token = val.trim();
            if !token.is_empty() && token.len() <= 128 {
                return Some(token.to_string());
            }
        }
    }
    None
}

/// Localized view model copy for the draft preview screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DraftPreviewCopy {
    pub badge: String,
    pub banner_body: String,
    pub title: String,
    pub subtitle: String,
    pub token_placeholder: String,
    pub button_open: String,
    pub button_loading: String,
    pub error_not_found: String,
    pub error_load: String,
}

/// Builds the localized view model copy for the draft preview screen.
pub fn build_draft_preview_copy(locale: Option<&str>) -> DraftPreviewCopy {
    DraftPreviewCopy {
        badge: t(locale, "blog.preview.badge", "Draft preview"),
        banner_body: t(
            locale,
            "blog.preview.banner.body",
            "This article is not published yet. The link stops working once the post is published, archived or the token expires.",
        ),
        title: t(locale, "blog.preview.title", "Draft preview"),
        subtitle: t(
            locale,
            "blog.preview.subtitle",
            "Paste a preview token, or open the link containing the #token=… fragment.",
        ),
        token_placeholder: t(locale, "blog.preview.token.placeholder", "token"),
        button_open: t(locale, "blog.preview.button.open", "Open preview"),
        button_loading: t(locale, "blog.preview.button.loading", "Loading…"),
        error_not_found: t(
            locale,
            "blog.preview.error.not.found",
            "The token is invalid, expired or has been revoked.",
        ),
        error_load: t(
            locale,
            "blog.preview.error.load",
            "Failed to load the draft.",
        ),
    }
}

/// Compute category choices and related items once, outside render bindings.
pub fn showcase_posts(
    data: &crate::model::StorefrontBlogData,
    searching: bool,
) -> (
    Option<String>,
    std::collections::BTreeMap<String, String>,
    Vec<crate::model::BlogPostListItem>,
) {
    let selected_slug = data
        .selected_post
        .as_ref()
        .and_then(|post| post.slug.clone());
    let mut categories = std::collections::BTreeMap::new();
    for post in &data.posts.items {
        if let (Some(id), Some(name)) = (&post.category_id, &post.category_name) {
            categories.entry(id.clone()).or_insert_with(|| name.clone());
        }
    }
    if let Some(post) = &data.selected_post {
        if let (Some(id), Some(name)) = (&post.category_id, &post.category_name) {
            categories.entry(id.clone()).or_insert_with(|| name.clone());
        }
    }
    let posts = data
        .posts
        .items
        .iter()
        .filter(|post| searching || selected_slug.is_none() || post.slug != selected_slug)
        .cloned()
        .collect();
    (selected_slug, categories, posts)
}
