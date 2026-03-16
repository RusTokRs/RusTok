use leptos::prelude::*;
use leptos_auth::hooks::use_current_user;
use leptos_router::components::A;
use leptos_router::hooks::use_location;

use crate::app::modules::{components_for_slot, AdminSlot};
use crate::app::providers::enabled_modules::use_enabled_modules;
use crate::{t_string, use_i18n};

#[component]
pub fn Sidebar() -> impl IntoView {
    let i18n = use_i18n();
    let current_user = use_current_user();
    let enabled_modules = use_enabled_modules();

    let module_nav_items = Signal::derive(move || {
        let enabled = enabled_modules.get();
        components_for_slot(AdminSlot::NavItem, Some(&enabled))
    });

    view! {
        <aside class="w-64 bg-sidebar border-r border-sidebar-border h-screen flex flex-col shrink-0">
            <div class="p-5 border-b border-sidebar-border">
                <A href="/dashboard" attr:class="flex items-center gap-2">
                    <div class="flex h-8 w-8 items-center justify-center rounded-lg bg-primary">
                        <span class="text-primary-foreground font-bold text-sm">"R"</span>
                    </div>
                    <span class="text-sm font-semibold text-sidebar-foreground">{t_string!(i18n, app.brand.title)}</span>
                </A>
            </div>

            <nav class="flex-1 px-3 py-4 overflow-y-auto">
                <NavGroupLabel label=t_string!(i18n, app.nav.group.overview).to_string() />
                <NavLink href="/dashboard" icon="grid" label=t_string!(i18n, app.nav.dashboard).to_string() />

                {move || {
                    let role = current_user.get()
                        .map(|u| u.role.to_uppercase())
                        .unwrap_or_default();
                    let is_admin = role == "ADMIN" || role == "SUPER_ADMIN";
                    if is_admin {
                        view! {
                            <div class="pt-3">
                                <NavGroupLabel label=t_string!(i18n, app.nav.group.management).to_string() />
                                <NavLink href="/users" icon="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z" label=t_string!(i18n, app.nav.users).to_string() />
                                <NavLink href="/modules" icon="M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4" label=t_string!(i18n, app.nav.modules).to_string() />
                                <NavLink href="/apps" icon="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1" label=t_string!(i18n, app.nav.apps).to_string() />

                                <Show when=move || !module_nav_items.get().is_empty()>
                                    <div class="pt-3">
                                        <NavGroupLabel label=t_string!(i18n, app.nav.modulePlugins).to_string() />
                                        {move || module_nav_items.get().into_iter().map(|item| (item.render)()).collect_view()}
                                    </div>
                                </Show>
                            </div>
                        }.into_any()
                    } else {
                        ().into_any()
                    }
                }}

                <div class="pt-3">
                    <NavGroupLabel label=t_string!(i18n, app.nav.group.account).to_string() />
                    <NavLink href="/profile" icon="user" label=t_string!(i18n, app.nav.profile).to_string() />
                    <NavLink href="/security" icon="lock" label=t_string!(i18n, app.nav.security).to_string() />
                </div>
            </nav>

            <div class="border-t border-border p-4">
                <div class="flex items-center gap-3 px-2">
                    <div class="flex h-9 w-9 items-center justify-center rounded-full bg-primary/10 text-sm font-bold text-primary">
                        {move || current_user.get().and_then(|u| u.name.as_ref().and_then(|n| n.chars().next())).unwrap_or('?')}
                    </div>
                    <div class="grid flex-1 min-w-0 text-left text-sm leading-tight">
                        <span class="truncate font-semibold text-sidebar-foreground text-xs">
                            {move || current_user.get().and_then(|u| u.name.clone()).unwrap_or_else(|| t_string!(i18n, app.menu.defaultUser).to_string())}
                        </span>
                        <span class="truncate text-xs text-sidebar-foreground/60">
                            {move || current_user.get().map(|u| u.email.clone()).unwrap_or_default()}
                        </span>
                    </div>
                </div>
            </div>
        </aside>
    }
}

#[component]
fn NavGroupLabel(label: String) -> impl IntoView {
    view! {
        <p class="mb-2 px-3 text-xs font-semibold uppercase tracking-wider text-muted-foreground/70">
            {label}
        </p>
    }
}

#[component]
fn NavLink(href: &'static str, icon: &'static str, label: String) -> impl IntoView {
    let location = use_location();
    let is_active = move || {
        let path = location.pathname.get();
        if href == "/" {
            path == "/" || path == "/dashboard"
        } else {
            path.starts_with(href)
        }
    };

    view! {
        <A
            href=href
            attr:class=move || format!(
                "flex items-center gap-3 rounded-lg px-3 py-2 text-sm font-medium transition-all hover:bg-accent hover:text-accent-foreground {}",
                if is_active() { "bg-accent text-accent-foreground shadow-sm" } else { "text-muted-foreground" }
            )
        >
            <NavIcon d=icon />
            {label}
        </A>
    }
}

#[component]
fn NavIcon(d: &'static str) -> impl IntoView {
    view! {
        <svg class="h-4 w-4 shrink-0 transition-colors" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d=d />
        </svg>
    }
}
