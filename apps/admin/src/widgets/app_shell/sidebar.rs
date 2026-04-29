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
        <aside class="hidden min-h-svh w-64 shrink-0 flex-col border-r border-sidebar-border bg-sidebar text-sidebar-foreground md:flex">
            <div class="flex h-16 items-center px-2">
                <A href="/dashboard" attr:class="flex w-full items-center gap-2 rounded-lg px-2 py-2 text-left transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-foreground">
                    <div class="flex h-8 w-8 items-center justify-center rounded-lg bg-sidebar-primary text-sidebar-primary-foreground">
                        <span class="text-sm font-semibold">"R"</span>
                    </div>
                    <div class="grid flex-1 text-left text-sm leading-tight">
                        <span class="truncate font-semibold">{move || t_string!(i18n, app.brand.title).to_string()}</span>
                        <span class="truncate text-xs text-sidebar-foreground/60">"Admin"</span>
                    </div>
                </A>
            </div>

            <nav class="flex flex-1 flex-col gap-1 overflow-y-auto px-2 py-2">
                <NavGroupLabel label=move || t_string!(i18n, app.nav.group.overview).to_string() />
                <NavLink href="/dashboard" icon="grid" label=move || t_string!(i18n, app.nav.dashboard).to_string() />

                {move || {
                    let role = current_user.get()
                        .map(|u| u.role.to_uppercase())
                        .unwrap_or_default();
                    let is_admin = role == "ADMIN" || role == "SUPER_ADMIN";
                    if is_admin {
                        view! {
                            <div class="pt-3">
                                <NavGroupLabel label=move || t_string!(i18n, app.nav.group.management).to_string() />
                                <NavLink href="/users" icon="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z" label=move || t_string!(i18n, app.nav.users).to_string() />
                                <NavLink href="/roles" icon="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" label="Roles & Permissions" />
                                <NavLink href="/modules" icon="M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4" label=move || t_string!(i18n, app.nav.modules).to_string() />
                                <NavLink href="/modules/search" icon="M21 21l-4.35-4.35m1.85-5.15a7 7 0 11-14 0 7 7 0 0114 0z" label=move || t_string!(i18n, app.nav.search).to_string() />
                                <NavLink href="/apps" icon="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1" label=move || t_string!(i18n, app.nav.apps).to_string() />
                                <NavLink href="/ai" icon="M9.5 3h5M12 3v3m-7 6a7 7 0 1114 0c0 1.947-.794 3.709-2.076 4.977L18 21h-2.5l-1.154-1.154A6.965 6.965 0 0112 20a6.965 6.965 0 01-2.346-.154L8.5 21H6l1.076-4.023A6.965 6.965 0 015 12z" label="AI" />
                                <NavLink href="/email" icon="M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z" label="Email Settings" />
                                <NavLink href="/cache" icon="M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4" label="Cache" />
                                <NavLink href="/events" icon="M13 10V3L4 14h7v7l9-11h-7z" label=move || t_string!(i18n, events.title).to_string() />
                                <NavLink href="/install" icon="M12 3v18m7-7-7 7-7-7M5 3h14" label="Installer" />

                                <Show when=move || !module_nav_items.get().is_empty()>
                                    <div class="pt-3">
                                        <NavGroupLabel label=move || t_string!(i18n, app.nav.modulePlugins).to_string() />
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
                    <NavGroupLabel label=move || t_string!(i18n, app.nav.group.account).to_string() />
                    <NavLink href="/profile" icon="user" label=move || t_string!(i18n, app.nav.profile).to_string() />
                    <NavLink href="/security" icon="lock" label=move || t_string!(i18n, app.nav.security).to_string() />
                </div>
            </nav>

            <div class="border-t border-sidebar-border p-2">
                <div class="flex items-center gap-3 rounded-lg px-2 py-2 text-sm transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-foreground">
                    <div class="flex h-8 w-8 items-center justify-center rounded-lg bg-sidebar-accent text-sm font-semibold text-sidebar-accent-foreground">
                        {move || current_user.get().and_then(|u| u.name.as_ref().and_then(|n| n.chars().next())).unwrap_or('?')}
                    </div>
                    <div class="grid flex-1 min-w-0 text-left text-sm leading-tight">
                        <span class="truncate font-semibold">
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
fn NavGroupLabel(#[prop(into)] label: TextProp) -> impl IntoView {
    view! {
        <p class="mt-2 px-2 py-1 text-xs font-medium text-sidebar-foreground/70 first:mt-0">
            {move || label.get()}
        </p>
    }
}

#[component]
fn NavLink(href: &'static str, icon: &'static str, #[prop(into)] label: TextProp) -> impl IntoView {
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
                "flex h-8 items-center gap-2 rounded-md px-2 text-sm transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-foreground {}",
                if is_active() { "bg-sidebar-accent text-sidebar-accent-foreground font-medium" } else { "text-sidebar-foreground/80" }
            )
        >
            <NavIcon d=icon />
            {move || label.get()}
        </A>
    }
}

#[component]
fn NavIcon(d: &'static str) -> impl IntoView {
    let path = match d {
        "grid" => "M3 3h7v7H3V3zm11 0h7v7h-7V3zM3 14h7v7H3v-7zm11 0h7v7h-7v-7z",
        "user" => "M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2M12 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8z",
        "lock" => "M7 11V7a5 5 0 0 1 10 0v4M5 11h14v10H5V11z",
        value => value,
    };

    view! {
        <svg class="h-4 w-4 shrink-0 transition-colors" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d=path />
        </svg>
    }
}
