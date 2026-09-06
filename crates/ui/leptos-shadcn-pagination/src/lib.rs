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
    let href = href.unwrap_or_else(|| "#".to_string());

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
    let href = href.unwrap_or_else(|| "#".to_string());
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
    let href = href.unwrap_or_else(|| "#".to_string());
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
