use leptos::prelude::*;
use leptos_auth::hooks::use_current_user;
use leptos_router::components::A;

use crate::{t_string, use_i18n};

#[component]
pub fn Sidebar() -> impl IntoView {
    let i18n = use_i18n();
    let current_user = use_current_user();

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
                <NavGroupLabel label=move || t_string!(i18n, app.nav.group.overview).to_string() />
                <NavLink href="/dashboard" icon="grid">
                    {move || t_string!(i18n, app.nav.dashboard)}
                </NavLink>

                {move || {
                    let role = current_user.get()
                        .map(|u| u.role.to_uppercase())
                        .unwrap_or_default();
                    let is_admin = role == "ADMIN" || role == "SUPER_ADMIN";
                    if is_admin {
                        view! {
                            <div class="pt-3">
                                <NavGroupLabel label=move || t_string!(i18n, app.nav.group.management).to_string() />
                                <NavLink href="/users" icon="users">
                                    {move || t_string!(i18n, app.nav.users)}
                                </NavLink>
                                <NavLink href="/modules" icon="package">
                                    {move || t_string!(i18n, app.nav.modules)}
                                </NavLink>
                            </div>
                        }.into_any()
                    } else {
                        ().into_any()
                    }
                }}

                <div class="pt-3">
                    <NavGroupLabel label=move || t_string!(i18n, app.nav.group.account).to_string() />
                    <NavLink href="/profile" icon="user">
                        {move || t_string!(i18n, app.nav.profile)}
                    </NavLink>
                    <NavLink href="/security" icon="lock">
                        {move || t_string!(i18n, app.nav.security)}
                    </NavLink>
                </div>
            </nav>

            <div class="p-4 border-t border-sidebar-border">
                <div class="flex items-center gap-2">
                    <div class="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-primary/10">
                        <span class="text-primary text-sm font-semibold">
                            {move || {
                                current_user.get()
                                    .and_then(|u| u.name.clone().or(Some(u.email.clone())))
                                    .and_then(|n| n.chars().next())
                                    .map(|c| c.to_uppercase().to_string())
                                    .unwrap_or_else(|| "U".to_string())
                            }}
                        </span>
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
fn NavGroupLabel(label: impl Fn() -> String + Send + Sync + 'static) -> impl IntoView {
    view! {
        <div class="px-3 pb-1 text-xs font-semibold text-sidebar-foreground/50 uppercase tracking-wider">
            {label}
        </div>
    }
}

#[component]
fn NavLink(href: &'static str, icon: &'static str, children: Children) -> impl IntoView {
    view! {
        <A
            href=href
            attr:class="flex items-center gap-3 px-3 py-2 text-sm font-medium text-sidebar-foreground rounded-lg hover:bg-sidebar-accent hover:text-sidebar-accent-foreground transition-colors mb-1"
        >
            <NavIcon icon=icon />
            <span>{children()}</span>
        </A>
    }
}

#[component]
fn NavIcon(icon: &'static str) -> impl IntoView {
    let svg_path = match icon {
        "grid" => "M4 4h6v6H4V4zm0 10h6v6H4v-6zm10-10h6v6h-6V4zm0 10h6v6h-6v-6z",
        "users" => "M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2M23 21v-2a4 4 0 0 0-3-3.87M16 3.13a4 4 0 0 1 0 7.75M9 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8z",
        "user" => "M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2M12 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8z",
        "lock" => "M19 11H5a2 2 0 0 0-2 2v7a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7a2 2 0 0 0-2-2zM7 11V7a5 5 0 0 1 10 0v4",
        "package" => "M16.5 9.4l-9-5.19M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16zM3.27 6.96L12 12.01l8.73-5.05M12 22.08V12",
        _ => "M12 12m-10 0a10 10 0 1 0 20 0a10 10 0 1 0-20 0",
    };

    view! {
        <svg
            class="h-4 w-4 shrink-0"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
        >
            <path d=svg_path />
        </svg>
    }
}
