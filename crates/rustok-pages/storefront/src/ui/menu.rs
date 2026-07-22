use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

use crate::model::{StorefrontMenu, StorefrontMenuItem, StorefrontMenuLocation};
use crate::transport;

#[derive(Clone, Copy)]
enum MenuPresentation {
    Header,
    Footer,
}

#[component]
pub fn PagesHeaderMenu() -> impl IntoView {
    view! {
        <ActiveMenu
            location=StorefrontMenuLocation::Header
            presentation=MenuPresentation::Header
        />
    }
}

#[component]
pub fn PagesFooterMenu() -> impl IntoView {
    view! {
        <ActiveMenu
            location=StorefrontMenuLocation::Footer
            presentation=MenuPresentation::Footer
        />
    }
}

#[component]
fn ActiveMenu(
    location: StorefrontMenuLocation,
    presentation: MenuPresentation,
) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let menu_resource = Resource::new_blocking(
        move || locale.clone(),
        move |locale| async move { transport::fetch_active_menu(location, locale).await },
    );

    view! {
        <Suspense fallback=|| ()>
            {move || {
                let menu_resource = menu_resource;
                Suspend::new(async move {
                    match menu_resource.await {
                        Ok(Some(menu)) => render_menu(menu, presentation),
                        Ok(None) | Err(_) => view! { <span class="hidden"></span> }.into_any(),
                    }
                })
            }}
        </Suspense>
    }
}

fn render_menu(menu: StorefrontMenu, presentation: MenuPresentation) -> AnyView {
    match presentation {
        MenuPresentation::Header => render_header_menu(menu),
        MenuPresentation::Footer => render_footer_menu(menu),
    }
}

fn render_header_menu(menu: StorefrontMenu) -> AnyView {
    let label = menu.name;
    view! {
        <nav class="hidden lg:flex items-center" aria-label=label>
            <ul class="flex items-center gap-6">
                {menu.items.into_iter().map(render_header_item).collect_view()}
            </ul>
        </nav>
    }
    .into_any()
}

fn render_header_item(item: StorefrontMenuItem) -> AnyView {
    if item.children.is_empty() {
        return view! {
            <li>
                <a class="text-sm text-muted-foreground hover:text-foreground transition-colors" href=item.url>
                    {item.title}
                </a>
            </li>
        }
        .into_any();
    }

    view! {
        <li class="relative">
            <details class="group">
                <summary class="cursor-pointer list-none text-sm text-muted-foreground hover:text-foreground transition-colors">
                    {item.title}
                </summary>
                <ul class="absolute left-0 z-50 mt-2 min-w-48 space-y-1 rounded-md border border-border bg-popover p-2 shadow-md">
                    {item.children.into_iter().map(render_header_child).collect_view()}
                </ul>
            </details>
        </li>
    }
    .into_any()
}

fn render_header_child(item: StorefrontMenuItem) -> AnyView {
    let nested = (!item.children.is_empty()).then(|| {
        view! {
            <ul class="mt-1 border-l border-border pl-3">
                {item.children.into_iter().map(render_header_child).collect_view()}
            </ul>
        }
    });
    view! {
        <li>
            <a class="block rounded px-3 py-2 text-sm text-popover-foreground hover:bg-accent hover:text-accent-foreground" href=item.url>
                {item.title}
            </a>
            {nested}
        </li>
    }
    .into_any()
}

fn render_footer_menu(menu: StorefrontMenu) -> AnyView {
    let label = menu.name;
    view! {
        <nav aria-label=label>
            <ul class="flex flex-wrap justify-center gap-x-6 gap-y-3">
                {menu.items.into_iter().map(render_footer_item).collect_view()}
            </ul>
        </nav>
    }
    .into_any()
}

fn render_footer_item(item: StorefrontMenuItem) -> AnyView {
    let nested = (!item.children.is_empty()).then(|| {
        view! {
            <ul class="mt-2 space-y-1">
                {item.children.into_iter().map(render_footer_item).collect_view()}
            </ul>
        }
    });
    view! {
        <li>
            <a class="text-sm text-muted-foreground hover:text-foreground transition-colors" href=item.url>
                {item.title}
            </a>
            {nested}
        </li>
    }
    .into_any()
}
