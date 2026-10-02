mod core;

use self::core::build_header_links;
use crate::shared::ui::UiButton;
use leptos::prelude::*;

#[component]
pub fn Header(
    locale: String,
    nav_home: String,
    nav_catalog: String,
    nav_about: String,
    nav_contact: String,
    nav_language: String,
    cta_primary: String,
    navigation_views: Vec<AnyView>,
    #[prop(optional)] action_views: Vec<AnyView>,
) -> impl IntoView {
    let links = build_header_links(locale.as_str());
    let navigation = if navigation_views.is_empty() {
        view! {
            <nav class="hidden md:flex items-center gap-1" aria-label="Primary navigation">
                <a class="rounded-xl px-3.5 py-1.5 text-sm font-medium text-muted-foreground hover:text-foreground hover:bg-secondary/70 transition-all duration-200" href="#home">{nav_home}</a>
                <a class="rounded-xl px-3.5 py-1.5 text-sm font-medium text-muted-foreground hover:text-foreground hover:bg-secondary/70 transition-all duration-200" href="#catalog">{nav_catalog}</a>
                <a class="rounded-xl px-3.5 py-1.5 text-sm font-medium text-muted-foreground hover:text-foreground hover:bg-secondary/70 transition-all duration-200" href="#about">{nav_about}</a>
                <a class="rounded-xl px-3.5 py-1.5 text-sm font-medium text-muted-foreground hover:text-foreground hover:bg-secondary/70 transition-all duration-200" href="#contact">{nav_contact}</a>
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
        <header class="sticky top-0 z-50 w-full border-b border-border/70 bg-background/85 backdrop-blur-xl shadow-xs transition-all">
            <div class="container-app flex h-16 w-full items-center justify-between px-4 sm:px-6">
                <div class="flex items-center gap-8">
                    <a class="group flex items-center gap-2.5 transition-transform duration-200 hover:scale-[1.02]" href=links.home_href>
                        <div class="flex h-9 w-9 items-center justify-center rounded-xl bg-gradient-to-tr from-primary to-orange-500 text-white shadow-md shadow-primary/25 transition-all duration-300 group-hover:shadow-primary/40 group-hover:rotate-3">
                            <svg class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
                                <polygon points="12 2 2 7 12 12 22 7 12 2" />
                                <polyline points="2 17 12 22 22 17" />
                                <polyline points="2 12 12 17 22 12" />
                            </svg>
                        </div>
                        <div class="flex items-center gap-1.5">
                            <span class="text-xl font-extrabold tracking-tight text-foreground">
                                "Rus"<span class="text-primary">"ToK"</span>
                            </span>
                            <span class="rounded-md bg-primary/10 border border-primary/20 px-1.5 py-0.5 text-[10px] font-bold text-primary tracking-wide">
                                "STORE"
                            </span>
                        </div>
                    </a>
                    {navigation}
                </div>

                <div class="flex items-center gap-3">
                    <div class="contents" data-storefront-header-actions="true">
                        {action_views}
                    </div>

                    // Language Selector with Flags
                    <div class="relative">
                        <details class="group">
                            <summary class="inline-flex items-center gap-1.5 rounded-xl border border-border/80 bg-secondary/60 px-3 py-1.5 text-xs font-semibold text-foreground cursor-pointer hover:bg-secondary hover:border-primary/40 transition-all list-none shadow-xs">
                                <svg class="h-3.5 w-3.5 text-muted-foreground group-hover:text-primary transition-colors" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                    <circle cx="12" cy="12" r="10" />
                                    <line x1="2" y1="12" x2="22" y2="12" />
                                    <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
                                </svg>
                                <span>{nav_language}</span>
                                <svg class="h-3 w-3 text-muted-foreground transition-transform group-open:rotate-180" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                    <polyline points="6 9 12 15 18 9" />
                                </svg>
                            </summary>
                            <ul class="absolute right-0 mt-2 w-36 rounded-xl border border-border/80 bg-popover/95 p-1.5 shadow-xl backdrop-blur-xl z-50">
                                <li>
                                    <a class="flex items-center gap-2 rounded-lg px-2.5 py-1.5 text-xs font-medium text-popover-foreground hover:bg-accent hover:text-accent-foreground transition-colors" href=links.english_href.clone()>
                                        <span>"🇺🇸"</span>
                                        <span>"English"</span>
                                    </a>
                                </li>
                                <li>
                                    <a class="flex items-center gap-2 rounded-lg px-2.5 py-1.5 text-xs font-medium text-popover-foreground hover:bg-accent hover:text-accent-foreground transition-colors" href=links.russian_href.clone()>
                                        <span>"🇷🇺"</span>
                                        <span>"Русский"</span>
                                    </a>
                                </li>
                            </ul>
                        </details>
                    </div>

                    // Primary Header Action
                    <a href="#catalog" class="inline-flex">
                        <UiButton class="px-4 py-2 text-xs font-semibold rounded-xl bg-primary text-primary-foreground shadow-sm shadow-primary/20 hover:shadow-md hover:shadow-primary/30 transition-all flex items-center gap-1.5">
                            <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                <path d="M6 2L3 6v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6l-3-4z" />
                                <line x1="3" y1="6" x2="21" y2="6" />
                                <path d="M16 10a4 4 0 0 1-8 0" />
                            </svg>
                            <span>{cta_primary}</span>
                        </UiButton>
                    </a>
                </div>
            </div>
        </header>
    }
}
