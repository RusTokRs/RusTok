use crate::model::{PageDetail, PageListItem, PublishedLanding};

pub fn selected_page_title(page: &PageDetail, default_title: String) -> String {
    page.translation
        .as_ref()
        .and_then(|translation| translation.title.clone())
        .unwrap_or(default_title)
}

pub fn selected_page_slug(page: &PageDetail, default_slug: String) -> String {
    page.translation
        .as_ref()
        .and_then(|translation| translation.slug.clone())
        .unwrap_or(default_slug)
}

pub fn selected_page_effective_locale(page: &PageDetail, default_locale: String) -> String {
    page.effective_locale.clone().unwrap_or(default_locale)
}

/// The content of the selected page: its published landing, when it has one.
///
/// This is the only content source. A page without a published landing has nothing to render;
/// the authoring document is not part of [`PageDetail`].
pub fn selected_page_landing(page: &PageDetail) -> Option<PublishedLanding> {
    page.landing.clone()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedPageEmptyState {
    pub title: String,
    pub body: String,
}

pub fn selected_page_empty_state(title: String, body: String) -> SelectedPageEmptyState {
    SelectedPageEmptyState { title, body }
}

pub fn count_label(template: &str, count: u64) -> String {
    template.replace("{count}", &count.to_string())
}

pub fn open_link_label(prefix: &str, slug: &str) -> String {
    format!("{} {}", prefix, slug)
}

pub fn page_link_href(module_route_base: &str, slug: &str) -> String {
    format!("{module_route_base}?slug={slug}")
}

pub fn page_status_label(status: &str) -> &str {
    status
}

pub fn label_value_pair(label: &str, value: &str) -> String {
    format!("{}: {}", label, value)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedPagesEmptyState {
    pub body: String,
}

pub fn published_pages_empty_state(body: String) -> PublishedPagesEmptyState {
    PublishedPagesEmptyState { body }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedPagesHeaderView {
    pub title: String,
    pub total_label: String,
}

pub fn published_pages_header_view(
    title: String,
    total_template: &str,
    total: u64,
) -> PublishedPagesHeaderView {
    PublishedPagesHeaderView {
        title,
        total_label: count_label(total_template, total),
    }
}

pub fn load_error_message(label: &str, error: &str) -> String {
    format!("{}: {}", label, error)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorefrontPageListItemView {
    pub title: String,
    pub slug: String,
    pub href: String,
    pub status: String,
    pub open_label: String,
    pub template_label: String,
}

pub fn storefront_page_list_item_view(
    page: PageListItem,
    module_route_base: &str,
    missing_slug: String,
    untitled_title: String,
    open_prefix: &str,
    template_label: &str,
) -> StorefrontPageListItemView {
    let title = page.title.unwrap_or(untitled_title);
    let slug = page.slug.unwrap_or(missing_slug);
    let href = page_link_href(module_route_base, slug.as_str());
    let status = page_status_label(page.status.as_str()).to_string();
    let open_label = open_link_label(open_prefix, slug.as_str());
    let template_label = label_value_pair(template_label, page.template.as_str());

    StorefrontPageListItemView {
        title,
        slug,
        href,
        status,
        open_label,
        template_label,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PageDetail, PageTranslation, PublishedLanding};

    #[test]
    fn storefront_link_and_status_helpers_are_core_owned() {
        assert_eq!(page_link_href("/pages", "home"), "/pages?slug=home");
        assert_eq!(page_status_label("published"), "published");
    }

    #[test]
    fn storefront_page_list_item_view_applies_core_owned_fallbacks() {
        let view = storefront_page_list_item_view(
            PageListItem {
                id: "page_1".to_string(),
                title: None,
                slug: None,
                status: "published".to_string(),
                template: "default".to_string(),
            },
            "/pages",
            "missing-slug".to_string(),
            "Untitled page".to_string(),
            "Open",
            "template",
        );

        assert_eq!(view.title, "Untitled page");
        assert_eq!(view.slug, "missing-slug");
        assert_eq!(view.href, "/pages?slug=missing-slug");
        assert_eq!(view.status, "published");
        assert_eq!(view.open_label, "Open missing-slug");
        assert_eq!(view.template_label, "template: default");
    }

    #[test]
    fn selected_page_empty_state_is_core_owned() {
        let state = selected_page_empty_state(
            "Requested page is not published yet".to_string(),
            "Choose a page from the list below".to_string(),
        );

        assert_eq!(state.title, "Requested page is not published yet");
        assert_eq!(state.body, "Choose a page from the list below");
    }

    #[test]
    fn selected_page_content_is_the_published_landing_only() {
        let landing = PublishedLanding {
            locale: "en".to_string(),
            html: "<h1>Landing</h1>".to_string(),
            css: "h1 { color: red; }".to_string(),
            artifact_hash: "hash".to_string(),
        };
        let mut page = PageDetail {
            effective_locale: Some("en".to_string()),
            translation: Some(PageTranslation {
                locale: "en".to_string(),
                title: Some("Landing".to_string()),
                slug: Some("landing".to_string()),
                meta_title: None,
                meta_description: None,
            }),
            landing: Some(landing.clone()),
        };
        assert_eq!(selected_page_landing(&page), Some(landing));

        page.landing = None;
        assert_eq!(selected_page_landing(&page), None);
    }
}
