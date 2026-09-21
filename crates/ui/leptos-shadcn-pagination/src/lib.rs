use leptos::prelude::*;

#[component]
pub fn Pagination(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    let class = format!("pagination {class}");
    view! { <nav class=class>{children()}</nav> }
}

#[component]
pub fn PaginationContent(
    #[prop(optional, into)] class: String,
    children: Children,
) -> impl IntoView {
    let class = format!("pagination-content {class}");
    view! { <ul class=class>{children()}</ul> }
}

#[component]
pub fn PaginationItem(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    let class = format!("pagination-item {class}");
    view! { <li class=class>{children()}</li> }
}

#[component]
pub fn PaginationLink(
    #[prop(optional, into)] href: Option<String>,
    #[prop(optional)] active: bool,
    #[prop(optional, into)] class: String,
    children: Children,
) -> impl IntoView {
    let class = if active {
        format!("pagination-link active {class}")
    } else {
        format!("pagination-link {class}")
    };
    let aria_current = active.then_some("page");
    let href = safe_pagination_href(href.unwrap_or_else(|| "#".to_string()));

    view! {
        <a class=class href=href aria-current=aria_current>
            {children()}
        </a>
    }
}

#[component]
pub fn PaginationPrevious(
    #[prop(optional, into)] href: Option<String>,
    #[prop(optional)] disabled: bool,
    #[prop(optional, into)] class: String,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    let class = if disabled {
        format!("pagination-previous disabled {class}")
    } else {
        format!("pagination-previous {class}")
    };
    let href = safe_pagination_href(href.unwrap_or_else(|| "#".to_string()));
    let label = children
        .map(|child| child())
        .unwrap_or_else(|| view! { <span>"Previous"</span> }.into_any());

    view! {
        <a class=class href=href aria-disabled=disabled>
            {label}
        </a>
    }
}

#[component]
pub fn PaginationNext(
    #[prop(optional, into)] href: Option<String>,
    #[prop(optional)] disabled: bool,
    #[prop(optional, into)] class: String,
    #[prop(optional)] children: Option<Children>,
) -> impl IntoView {
    let class = if disabled {
        format!("pagination-next disabled {class}")
    } else {
        format!("pagination-next {class}")
    };
    let href = safe_pagination_href(href.unwrap_or_else(|| "#".to_string()));
    let label = children
        .map(|child| child())
        .unwrap_or_else(|| view! { <span>"Next"</span> }.into_any());

    view! {
        <a class=class href=href aria-disabled=disabled>
            {label}
        </a>
    }
}

#[component]
pub fn PaginationEllipsis(#[prop(optional, into)] class: String) -> impl IntoView {
    let class = format!("pagination-ellipsis {class}");
    view! { <span class=class aria-hidden="true">"…"</span> }
}

fn safe_pagination_href(value: String) -> String {
    let href = value.trim();
    if href.is_empty()
        || href.starts_with("//")
        || href.chars().any(|character| character.is_control())
    {
        return "#".to_string();
    }

    let lower = href.to_ascii_lowercase();
    if let Some(colon) = lower.find(':') {
        let scheme = &lower[..colon];
        if !matches!(scheme, "http" | "https") {
            return "#".to_string();
        }
    } else if !(href.starts_with('/')
        || href.starts_with("./")
        || href.starts_with("../")
        || href.starts_with('?')
        || href.starts_with('#'))
    {
        return "#".to_string();
    }

    href.to_string()
}

#[cfg(test)]
mod tests {
    use super::safe_pagination_href;

    #[test]
    fn pagination_href_allows_internal_navigation() {
        assert_eq!(safe_pagination_href("/catalog?page=2".to_string()), "/catalog?page=2");
        assert_eq!(safe_pagination_href("?page=2".to_string()), "?page=2");
        assert_eq!(safe_pagination_href("#page-2".to_string()), "#page-2");
        assert_eq!(safe_pagination_href("../catalog?page=2".to_string()), "../catalog?page=2");
    }

    #[test]
    fn pagination_href_allows_http_urls() {
        assert_eq!(
            safe_pagination_href("https://example.com/catalog?page=2".to_string()),
            "https://example.com/catalog?page=2"
        );
        assert_eq!(
            safe_pagination_href("http://example.com/catalog?page=2".to_string()),
            "http://example.com/catalog?page=2"
        );
    }

    #[test]
    fn pagination_href_blocks_executable_and_opaque_schemes() {
        for href in [
            "javascript:alert(1)",
            "data:text/html,<script>alert(1)</script>",
            "vbscript:msgbox(1)",
            "file:///etc/passwd",
            "blob:https://example.com/id",
            "//evil.example/path",
            "catalog?page=2\n",
        ] {
            assert_eq!(safe_pagination_href(href.to_string()), "#");
        }
    }
}
