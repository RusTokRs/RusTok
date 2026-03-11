use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_location;

use crate::features::auth::UserMenu;
use crate::shared::ui::LanguageToggle;
use crate::{t_string, use_i18n};

#[derive(Clone, Copy, PartialEq)]
struct Breadcrumb {
    label_key: &'static str,
    href: Option<&'static str>,
}

#[component]
pub fn Header() -> impl IntoView {
    let i18n = use_i18n();
    let location = use_location();

    let breadcrumbs = Memo::new(move |_| resolve_breadcrumbs(&location.pathname.get()));
    let title_key = Memo::new(move |_| resolve_title_key(&location.pathname.get()));

    Effect::new(move |_| {
        let title = format!(
            "{} — {}",
            t_string!(i18n, app.brand.title),
            resolve_label(i18n, title_key.get())
        );
        set_document_title(&title);
    });

    view! {
        <header class="h-14 bg-background border-b border-border flex items-center justify-between px-6 shrink-0">
            <div class="flex items-center gap-2 text-sm text-muted-foreground">
                <span class="font-medium text-foreground">{t_string!(i18n, app.brand.title)}</span>
                <span>"/"</span>
                {{
                    let crumbs = breadcrumbs.get();
                    let last_index = crumbs.len().saturating_sub(1);
                    crumbs
                        .into_iter()
                        .enumerate()
                        .map(|(index, crumb)| {
                            let label_key = crumb.label_key;
                            let is_last = index == last_index;
                            let content = if let Some(href) = crumb.href {
                                view! { <A href=href attr:class="hover:text-foreground transition-colors">{move || resolve_label(i18n, label_key)}</A> }.into_any()
                            } else {
                                view! { <span class="text-foreground">{move || resolve_label(i18n, label_key)}</span> }.into_any()
                            };
                            view! {
                                <div class="flex items-center gap-2">
                                    {content}
                                    <Show when=move || !is_last>
                                        <span>"/"</span>
                                    </Show>
                                </div>
                            }
                        })
                        .collect_view()
                }}
            </div>

            <div class="flex items-center gap-3">
                <LanguageToggle />
                <UserMenu />
            </div>
        </header>
    }
}

/// Resolve a navigation label key to its translation.
/// Uses compile-time checked keys via t_string! for known routes.
fn resolve_label(i18n: leptos_i18n::I18nContext<crate::i18n::Locale>, key: &str) -> String {
    let s: &str = match key {
        "app.nav.dashboard" => t_string!(i18n, app.nav.dashboard),
        "app.nav.users" => t_string!(i18n, app.nav.users),
        "app.nav.profile" => t_string!(i18n, app.nav.profile),
        "app.nav.security" => t_string!(i18n, app.nav.security),
        "app.nav.modules" => t_string!(i18n, app.nav.modules),
        "users.detail.title" => t_string!(i18n, users.detail.title),
        _ => key,
    };
    s.to_string()
}

fn resolve_breadcrumbs(pathname: &str) -> Vec<Breadcrumb> {
    match pathname {
        "/" | "/dashboard" => vec![Breadcrumb {
            label_key: "app.nav.dashboard",
            href: Some("/dashboard"),
        }],
        "/users" => vec![Breadcrumb {
            label_key: "app.nav.users",
            href: Some("/users"),
        }],
        "/profile" => vec![Breadcrumb {
            label_key: "app.nav.profile",
            href: Some("/profile"),
        }],
        "/security" => vec![Breadcrumb {
            label_key: "app.nav.security",
            href: Some("/security"),
        }],
        "/modules" => vec![Breadcrumb {
            label_key: "app.nav.modules",
            href: Some("/modules"),
        }],
        _ if pathname.starts_with("/users/") => vec![
            Breadcrumb {
                label_key: "app.nav.users",
                href: Some("/users"),
            },
            Breadcrumb {
                label_key: "users.detail.title",
                href: None,
            },
        ],
        _ => vec![Breadcrumb {
            label_key: "app.nav.dashboard",
            href: Some("/dashboard"),
        }],
    }
}

fn resolve_title_key(pathname: &str) -> &'static str {
    match pathname {
        "/" | "/dashboard" => "app.nav.dashboard",
        "/users" => "app.nav.users",
        "/profile" => "app.nav.profile",
        "/security" => "app.nav.security",
        "/modules" => "app.nav.modules",
        _ if pathname.starts_with("/users/") => "users.detail.title",
        _ => "app.nav.dashboard",
    }
}

fn set_document_title(_title: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                document.set_title(_title);
            }
        }
    }
}
