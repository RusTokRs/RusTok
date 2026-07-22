mod core;

use self::core::build_header_links;
use crate::shared::ui::UiButton;
use leptos::prelude::*;

#[component]
pub fn Header(
    locale: String,
    nav_home: &'static str,
    nav_catalog: &'static str,
    nav_about: &'static str,
    nav_contact: &'static str,
    nav_language: &'static str,
    cta_primary: &'static str,
    navigation_views: Vec<AnyView>,
) -> impl IntoView {
    let links = build_header_links(locale.as_str());
    let navigation = if navigation_views.is_empty() {
        view! {
            <nav class="hidden lg:flex items-center gap-6" aria-label="Primary navigation">
                <a class="text-sm text-muted-foreground hover:text-foreground transition-colors" href="#home">{nav_home}</a>
                <a class="text-sm text-muted-foreground hover:text-foreground transition-colors" href="#catalog">{nav_catalog}</a>
                <a class="text-sm text-muted-foreground hover:text-foreground transition-colors" href="#about">{nav_about}</a>
                <a class="text-sm text-muted-foreground hover:text-foreground transition-colors" href="#contact">{nav_contact}</a>
            </nav>
        }
        .into_any()
    } else {
        view! {
            <div class="contents">{navigation_views}</div>
        }
        .into_any()
    };

    view! {
        <header class="sticky top-0 z-40 border-b border-border bg-background/95 backdrop-blur">
            <div class="container-app flex h-14 w-full items-center px-4">
                <div class="flex-1">
                    <a class="text-xl font-bold text-foreground hover:text-primary transition-colors" href=links.home_href>
                        "RusToK"
                    </a>
                </div>
                {navigation}
                <div class="flex items-center gap-3 ml-6">
                    <div class="relative">
                        <details class="group">
                            <summary class="inline-flex items-center gap-1 rounded-md border border-input bg-background px-3 py-1.5 text-sm text-foreground cursor-pointer hover:bg-accent hover:text-accent-foreground transition-colors list-none">
                                {nav_language}
                            </summary>
                            <ul class="absolute right-0 mt-1 w-32 rounded-md border border-border bg-popover p-1 shadow-md z-50">
                                <li>
                                    <a class="block rounded px-3 py-1.5 text-sm text-popover-foreground hover:bg-accent hover:text-accent-foreground transition-colors" href=links.english_href.clone()>
                                        "English"
                                    </a>
                                </li>
                                <li>
                                    <a class="block rounded px-3 py-1.5 text-sm text-popover-foreground hover:bg-accent hover:text-accent-foreground transition-colors" href=links.russian_href.clone()>
                                        "Русский"
                                    </a>
                                </li>
                            </ul>
                        </details>
                    </div>
                    <a href="#catalog">
                        <UiButton class="px-4 py-1.5 text-sm">
                            {cta_primary}
                        </UiButton>
                    </a>
                </div>
            </div>
        </header>
    }
}
