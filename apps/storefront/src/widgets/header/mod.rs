mod core;

use self::core::{
    SearchScope, build_header_links, build_search_target_url, resolve_default_scope,
};
use crate::shared::ui::UiButton;
use leptos::prelude::*;
use rustok_ui_core::UiRouteContext;

#[component]
fn HeaderSearchInput(locale: String) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>();
    let route_segment = route_context
        .as_ref()
        .and_then(|ctx| ctx.route_segment.as_deref());
    let default_scope = resolve_default_scope(route_segment);

    let (active_scope, set_active_scope) = signal(default_scope);
    let (search_query, set_search_query) = signal(String::new());
    let (is_dropdown_open, set_is_dropdown_open) = signal(false);

    let is_ru = locale.eq_ignore_ascii_case("ru");
    let current_locale = locale.clone();

    let on_submit = {
        let current_locale = current_locale.clone();
        move |ev: leptos::ev::SubmitEvent| {
            ev.prevent_default();
            let query = search_query.get();
            let scope = active_scope.get();
            let target = build_search_target_url(&current_locale, &query, scope);
            #[cfg(target_arch = "wasm32")]
            if let Some(window) = leptos::web_sys::window() {
                let _ = window.location().set_href(&target);
            }
        }
    };

    let search_action_url = {
        let normalized = rustok_ui_core::normalize_ui_text(&locale)
            .map(|s| s.to_lowercase())
            .unwrap_or_else(|| "en".to_string());
        format!("/{}/search", normalized)
    };

    view! {
        <div class="relative flex items-center flex-1 max-w-[14rem] sm:max-w-xs md:max-w-sm lg:max-w-md mx-2 sm:mx-4">
            <form
                method="get"
                action=search_action_url
                on:submit=on_submit
                class="relative flex items-center w-full rounded-xl border border-border/70 bg-secondary/40 hover:bg-secondary/60 focus-within:bg-background focus-within:border-primary/50 focus-within:ring-2 focus-within:ring-primary/20 transition-all duration-200 shadow-xs"
            >
                // Hidden input for SSR query parameters
                {move || active_scope.get().source_module().map(|module| view! {
                    <input type="hidden" name="source_modules" value=module />
                })}

                // Scope badge with dropdown trigger
                <div class="relative flex items-center pl-1.5 shrink-0">
                    <button
                        type="button"
                        class="inline-flex items-center gap-1 px-2 py-1 rounded-lg text-xs font-semibold bg-primary/10 text-primary hover:bg-primary/20 transition-colors cursor-pointer"
                        on:click=move |_| set_is_dropdown_open.update(|open| *open = !*open)
                        title=if is_ru { "Выбрать область поиска" } else { "Select search scope" }
                    >
                        <span>{move || active_scope.get().label(is_ru)}</span>
                        <svg class="h-3 w-3 opacity-70" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                            <polyline points="6 9 12 15 18 9" />
                        </svg>
                    </button>

                    // Clear scope button (if not All)
                    {move || (active_scope.get() != SearchScope::All).then(|| view! {
                        <button
                            type="button"
                            class="ml-0.5 p-0.5 text-muted-foreground hover:text-foreground rounded-full transition-colors cursor-pointer"
                            on:click=move |ev| {
                                ev.stop_propagation();
                                set_active_scope.set(SearchScope::All);
                            }
                            title=if is_ru { "Сбросить в поиск везде" } else { "Reset to search all" }
                        >
                            <svg class="h-3 w-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                                <line x1="18" y1="6" x2="6" y2="18" />
                                <line x1="6" y1="6" x2="18" y2="18" />
                            </svg>
                        </button>
                    })}

                    // Dropdown menu for selecting scope
                    {move || is_dropdown_open.get().then(|| view! {
                        <div class="absolute left-0 top-full mt-2 w-48 rounded-xl border border-border/80 bg-popover/95 p-1.5 shadow-xl backdrop-blur-xl z-50 animate-in fade-in zoom-in-95 duration-100">
                            <div class="px-2 py-1 text-[10px] font-bold uppercase tracking-wider text-muted-foreground">
                                {if is_ru { "Область поиска" } else { "Search scope" }}
                            </div>
                            {SearchScope::ALL_SCOPES.into_iter().map(|scope| {
                                let is_active = move || active_scope.get() == scope;
                                view! {
                                    <button
                                        type="button"
                                        class=move || {
                                            if is_active() {
                                                "w-full flex items-center justify-between rounded-lg px-2.5 py-1.5 text-xs font-semibold bg-accent text-accent-foreground cursor-pointer"
                                            } else {
                                                "w-full flex items-center justify-between rounded-lg px-2.5 py-1.5 text-xs font-medium text-popover-foreground hover:bg-secondary/80 transition-colors cursor-pointer"
                                            }
                                        }
                                        on:click=move |_| {
                                            set_active_scope.set(scope);
                                            set_is_dropdown_open.set(false);
                                        }
                                    >
                                        <span>{scope.label(is_ru)}</span>
                                        {move || is_active().then(|| view! {
                                            <svg class="h-3.5 w-3.5 text-primary" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                                                <polyline points="20 6 9 17 4 12" />
                                            </svg>
                                        })}
                                    </button>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    })}
                </div>

                // Text input
                <input
                    type="search"
                    name="q"
                    class="w-full min-w-0 bg-transparent px-2.5 py-1.5 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none"
                    placeholder=move || active_scope.get().placeholder(is_ru)
                    prop:value=move || search_query.get()
                    on:input=move |ev| set_search_query.set(event_target_value(&ev))
                />

                // Search submit button
                <button
                    type="submit"
                    class="pr-2.5 pl-1 text-muted-foreground hover:text-primary transition-colors cursor-pointer shrink-0"
                    aria-label=if is_ru { "Искать" } else { "Search" }
                >
                    <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
                        <circle cx="11" cy="11" r="8" />
                        <line x1="21" y1="21" x2="16.65" y2="16.65" />
                    </svg>
                </button>
            </form>
        </div>
    }
}

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

                <HeaderSearchInput locale=locale.clone() />

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
