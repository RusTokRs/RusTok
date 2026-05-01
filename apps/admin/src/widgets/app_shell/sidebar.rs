use std::collections::BTreeMap;

use leptos::prelude::*;
use leptos_auth::hooks::{use_current_user, use_tenant};
use leptos_router::components::A;
use leptos_router::hooks::{use_location, use_query_map};

use crate::app::modules::module_navigation_entries;
use crate::app::providers::enabled_modules::use_enabled_modules;
use crate::{t_string, use_i18n};

#[derive(Clone)]
struct NavChild {
    href: String,
    label: String,
}

#[derive(Clone)]
struct ModuleNavGroup {
    key: &'static str,
    items: Vec<ModuleNavItem>,
}

#[derive(Clone)]
struct ModuleNavItem {
    label: String,
    order: usize,
    children: Vec<NavChild>,
}

#[component]
pub fn Sidebar(#[prop(into)] sidebar_open: Signal<bool>) -> impl IntoView {
    let i18n = use_i18n();
    let current_user = use_current_user();
    let tenant = use_tenant();
    let enabled_modules = use_enabled_modules();

    let module_nav_groups = Signal::derive(move || {
        let enabled = enabled_modules.get();
        let mut grouped = BTreeMap::<&'static str, Vec<ModuleNavItem>>::new();

        for entry in module_navigation_entries()
            .iter()
            .filter(|entry| enabled.contains(entry.module_slug))
        {
            let mut children = vec![NavChild {
                href: format!("/modules/{}", entry.route_segment),
                label: t_string!(i18n, app.nav.overview).to_string(),
            }];

            children.extend(entry.child_pages.iter().map(|child| NavChild {
                href: format!("/modules/{}/{}", entry.route_segment, child.subpath),
                label: child.nav_label.to_string(),
            }));

            if entry.has_settings {
                children.push(NavChild {
                    href: format!("/modules?module_slug={}", entry.module_slug),
                    label: format!("{} {}", entry.nav_label, t_string!(i18n, app.nav.settings)),
                });
            }

            grouped
                .entry(entry.nav_group)
                .or_default()
                .push(ModuleNavItem {
                    label: entry.nav_label.to_string(),
                    order: entry.nav_order,
                    children,
                });
        }

        let mut groups = grouped
            .into_iter()
            .map(|(key, mut items)| {
                items.sort_by(|left, right| {
                    left.order
                        .cmp(&right.order)
                        .then_with(|| left.label.cmp(&right.label))
                });
                ModuleNavGroup { key, items }
            })
            .collect::<Vec<_>>();

        groups.sort_by(|left, right| {
            module_group_order(left.key)
                .cmp(&module_group_order(right.key))
                .then_with(|| left.key.cmp(right.key))
        });
        groups
    });

    view! {
        <aside class=move || {
            format!(
                "hidden h-svh min-h-0 shrink-0 flex-col border-r border-sidebar-border bg-sidebar text-sidebar-foreground transition-[width] duration-200 ease-linear md:flex {}",
                if sidebar_open.get() { "w-64" } else { "w-14" }
            )
        }>
            <div class="flex h-16 items-center px-2">
                <A href="/dashboard" attr:class=move || {
                    format!(
                        "flex w-full items-center gap-2 rounded-lg px-2 py-2 text-left transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-foreground {}",
                        if sidebar_open.get() { "" } else { "justify-center" }
                    )
                }>
                    <div class="flex h-8 w-8 items-center justify-center rounded-lg bg-sidebar-primary text-sidebar-primary-foreground">
                        <span class="text-sm font-semibold">"R"</span>
                    </div>
                    <Show when=move || sidebar_open.get()>
                        <div class="grid flex-1 text-left text-sm leading-tight">
                            <span class="truncate font-semibold">
                                {move || {
                                    tenant
                                        .get()
                                        .filter(|value| !value.trim().is_empty())
                                        .unwrap_or_else(|| t_string!(i18n, app.brand.title).to_string())
                                }}
                            </span>
                            <span class="truncate text-xs text-sidebar-foreground/60">
                                {move || current_user.get().map(|u| u.role).unwrap_or_else(|| "Workspace".to_string())}
                            </span>
                        </div>
                    </Show>
                </A>
            </div>

            <nav class=move || {
                format!(
                    "admin-sidebar-scrollbar flex min-h-0 flex-1 flex-col gap-1 overflow-x-hidden overscroll-contain px-2 py-2 {}",
                    if sidebar_open.get() { "overflow-y-auto" } else { "overflow-hidden" }
                )
            }>
                <Show when=move || sidebar_open.get()>
                    <NavGroupLabel label=move || t_string!(i18n, app.nav.group.overview).to_string() />
                </Show>
                <NavLink sidebar_open=sidebar_open href="/dashboard" icon="grid" label=move || t_string!(i18n, app.nav.dashboard).to_string() />

                {move || {
                    let role = current_user
                        .get()
                        .map(|u| u.role.to_uppercase())
                        .unwrap_or_default();
                    let is_admin = role == "ADMIN" || role == "SUPER_ADMIN";
                    if !is_admin {
                        return ().into_any();
                    }

                    view! {
                        <div class="pt-3">
                            <Show when=move || sidebar_open.get()>
                                <NavGroupLabel label=move || t_string!(i18n, app.nav.group.management).to_string() />
                            </Show>
                            <NavContainer
                                sidebar_open=sidebar_open
                                label=move || t_string!(i18n, app.nav.group.access).to_string()
                                icon="users"
                                children=vec![
                                    NavChild { href: "/users".to_string(), label: t_string!(i18n, app.nav.users).to_string() },
                                    NavChild { href: "/roles".to_string(), label: t_string!(i18n, app.nav.roles).to_string() },
                                ]
                            />
                            <NavContainer
                                sidebar_open=sidebar_open
                                label=move || t_string!(i18n, app.nav.group.platform).to_string()
                                icon="box"
                                children=vec![
                                    NavChild { href: "/modules".to_string(), label: t_string!(i18n, app.nav.modules).to_string() },
                                    NavChild { href: "/apps".to_string(), label: t_string!(i18n, app.nav.apps).to_string() },
                                    NavChild { href: "/install".to_string(), label: t_string!(i18n, app.nav.installer).to_string() },
                                ]
                            />
                            <NavContainer
                                sidebar_open=sidebar_open
                                label=move || t_string!(i18n, app.nav.group.operations).to_string()
                                icon="activity"
                                children=vec![
                                    NavChild { href: "/ai".to_string(), label: t_string!(i18n, app.nav.ai).to_string() },
                                    NavChild { href: "/email".to_string(), label: t_string!(i18n, app.nav.email).to_string() },
                                    NavChild { href: "/cache".to_string(), label: t_string!(i18n, app.nav.cache).to_string() },
                                    NavChild { href: "/events".to_string(), label: t_string!(i18n, events.title).to_string() },
                                ]
                            />

                            <Show when=move || !module_nav_groups.get().is_empty()>
                                <div class="pt-3">
                                    <Show when=move || sidebar_open.get()>
                                        <NavGroupLabel label=move || t_string!(i18n, app.nav.modulePlugins).to_string() />
                                    </Show>
                                    {move || {
                                        module_nav_groups
                                            .get()
                                            .into_iter()
                                            .map(|group| {
                                                let label = module_group_label(group.key);
                                                view! {
                                                    <Show when=move || sidebar_open.get()>
                                                        <NavGroupLabel label=label.clone() />
                                                    </Show>
                                                    <div class=move || {
                                                        if sidebar_open.get() {
                                                            "space-y-1"
                                                        } else {
                                                            "space-y-1"
                                                        }
                                                    }>
                                                        {group.items
                                                            .into_iter()
                                                            .map(|item| {
                                                                view! {
                                                                    <NavContainer
                                                                        sidebar_open=sidebar_open
                                                                        label=item.label
                                                                        icon=module_group_icon(group.key)
                                                                        children=item.children
                                                                    />
                                                                }
                                                            })
                                                            .collect_view()}
                                                    </div>
                                                }
                                            })
                                            .collect_view()
                                    }}
                                </div>
                            </Show>
                        </div>
                    }
                    .into_any()
                }}

                <div class="pt-3">
                    <Show when=move || sidebar_open.get()>
                        <NavGroupLabel label=move || t_string!(i18n, app.nav.group.account).to_string() />
                    </Show>
                    <NavLink sidebar_open=sidebar_open href="/profile" icon="user" label=move || t_string!(i18n, app.nav.profile).to_string() />
                    <NavLink sidebar_open=sidebar_open href="/security" icon="lock" label=move || t_string!(i18n, app.nav.security).to_string() />
                </div>
            </nav>

            <div class="border-t border-sidebar-border p-2">
                <div class=move || {
                    format!(
                        "flex items-center gap-3 rounded-lg px-2 py-2 text-sm transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-foreground {}",
                        if sidebar_open.get() { "" } else { "justify-center" }
                    )
                }>
                    <div class="flex h-8 w-8 items-center justify-center rounded-lg bg-sidebar-accent text-sm font-semibold text-sidebar-accent-foreground">
                        {move || current_user.get().and_then(|u| u.name.as_ref().and_then(|n| n.chars().next())).unwrap_or('?')}
                    </div>
                    <Show when=move || sidebar_open.get()>
                        <div class="grid min-w-0 flex-1 text-left text-sm leading-tight">
                            <span class="truncate font-semibold">
                                {move || current_user.get().and_then(|u| u.name.clone()).unwrap_or_else(|| t_string!(i18n, app.menu.defaultUser).to_string())}
                            </span>
                            <span class="truncate text-xs text-sidebar-foreground/60">
                                {move || current_user.get().map(|u| u.email.clone()).unwrap_or_default()}
                            </span>
                        </div>
                    </Show>
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
fn NavContainer(
    #[prop(into)] sidebar_open: Signal<bool>,
    #[prop(into)] label: TextProp,
    icon: &'static str,
    children: Vec<NavChild>,
) -> impl IntoView {
    let location = use_location();
    let query = use_query_map();
    let expanded = RwSignal::new(false);
    let children = StoredValue::new(children);
    let label_for_title = label.clone();
    let label_for_button = label.clone();

    let is_active = Memo::new(move |_| {
        let path = location.pathname.get();
        let module_query = query.get().get("module_slug");
        children
            .get_value()
            .iter()
            .any(|child| href_is_active(&path, module_query.as_deref(), &child.href))
    });
    let is_open = Memo::new(move |_| sidebar_open.get() && (expanded.get() || is_active.get()));

    view! {
        <div class="space-y-1">
            <button
                type="button"
                title=move || label_for_title.get()
                class=move || format!(
                    "flex h-8 w-full items-center gap-2 rounded-md px-2 text-sm transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-foreground {} {}",
                    if sidebar_open.get() { "" } else { "justify-center" },
                    if is_active.get() { "bg-sidebar-accent text-sidebar-accent-foreground font-medium" } else { "text-sidebar-foreground/80" }
                )
                on:click=move |_| set_expanded(expanded, !expanded.get())
            >
                <NavIcon d=icon />
                <span class=move || if sidebar_open.get() { "truncate" } else { "hidden" }>
                    {move || label_for_button.get()}
                </span>
                <span class=move || {
                    format!(
                        "ml-auto hidden h-4 w-4 items-center justify-center transition-transform {} {}",
                        if sidebar_open.get() { "inline-flex" } else { "" },
                        if is_open.get() { "rotate-90" } else { "" }
                    )
                }>
                    <NavIcon d="chevron" />
                </span>
            </button>
            <Show when=move || is_open.get()>
                <div class="ml-4 space-y-1 border-l border-sidebar-border/70 pl-2">
                    {children
                        .get_value()
                        .into_iter()
                        .map(|child| {
                            view! {
                                <NavLink
                                    sidebar_open=Signal::derive(move || true)
                                    href=child.href
                                    icon="dot"
                                    label=child.label
                                />
                            }
                        })
                        .collect_view()}
                </div>
            </Show>
        </div>
    }
}

#[component]
fn NavLink(
    #[prop(into)] sidebar_open: Signal<bool>,
    #[prop(into)] href: String,
    icon: &'static str,
    #[prop(into)] label: TextProp,
) -> impl IntoView {
    let location = use_location();
    let query = use_query_map();
    let label_text = label.clone();
    let label_for_title = label.clone();
    let href_for_active = href.clone();
    let is_active = move || {
        let path = location.pathname.get();
        let module_query = query.get().get("module_slug");
        href_is_active(&path, module_query.as_deref(), &href_for_active)
    };

    view! {
        <A
            href=href
            attr:title=move || label_for_title.get()
            attr:class=move || format!(
                "flex h-8 items-center gap-2 rounded-md px-2 text-sm transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-foreground {} {}",
                if sidebar_open.get() { "" } else { "justify-center" },
                if is_active() { "bg-sidebar-accent text-sidebar-accent-foreground font-medium" } else { "text-sidebar-foreground/80" }
            )
        >
            <NavIcon d=icon />
            <span class=move || if sidebar_open.get() { "truncate" } else { "hidden" }>
                {move || label_text.get()}
            </span>
        </A>
    }
}

#[component]
fn NavIcon(d: &'static str) -> impl IntoView {
    let path = match d {
        "activity" => "M13 10V3L4 14h7v7l9-11h-7z",
        "box" => "M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4",
        "chevron" => "M9 5l7 7-7 7",
        "content" => "M4 6h16M4 12h16M4 18h10",
        "commerce" => "M6 6h15l-1.5 9h-12L6 6zM6 6 5 3H3m6 18a1 1 0 1 0 0-2 1 1 0 0 0 0 2zm9 0a1 1 0 1 0 0-2 1 1 0 0 0 0 2z",
        "dot" => "M12 12h.01",
        "grid" => "M3 3h7v7H3V3zm11 0h7v7h-7V3zM3 14h7v7H3v-7zm11 0h7v7h-7v-7z",
        "list" => "M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01",
        "lock" => "M7 11V7a5 5 0 0 1 10 0v4M5 11h14v10H5V11z",
        "runtime" => "M4 17l6-6-4-4m8 0h6m-6 5h6m-6 5h6",
        "settings" => "M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8zm8.94 3a8.1 8.1 0 0 0-.56-1.35l1.07-1.07-2.03-2.03-1.07 1.07A8.1 8.1 0 0 0 17 7.06V5.5h-2.87l-.38-1.5h-3.5l-.38 1.5H7v1.56c-.47.15-.92.34-1.35.56L4.58 6.55 2.55 8.58l1.07 1.07c-.22.43-.41.88-.56 1.35H1.5v2.87l1.56.38c.15.47.34.92.56 1.35l-1.07 1.07 2.03 2.03 1.07-1.07c.43.22.88.41 1.35.56v1.56h2.87l.38 1.5h3.5l.38-1.5H17v-1.56c.47-.15.92-.34 1.35-.56l1.07 1.07 2.03-2.03-1.07-1.07c.22-.43.41-.88.56-1.35l1.56-.38V11h-1.56z",
        "user" => "M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2M12 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8z",
        "users" => "M17 20h5v-2a3 3 0 0 0-5.36-1.86M17 20H7m10 0v-2c0-.66-.13-1.28-.36-1.86M7 20H2v-2a3 3 0 0 1 5.36-1.86M15 7a3 3 0 1 1-6 0 3 3 0 0 1 6 0z",
        value => value,
    };

    view! {
        <svg class="h-4 w-4 shrink-0 transition-colors" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d=path />
        </svg>
    }
}

fn set_expanded(expanded: RwSignal<bool>, next: bool) {
    expanded.set(next);
}

fn href_is_active(path: &str, module_query: Option<&str>, href: &str) -> bool {
    if let Some(module_slug) = href.strip_prefix("/modules?module_slug=") {
        return path == "/modules" && module_query == Some(module_slug);
    }

    if href == "/dashboard" {
        return path == "/dashboard" || path == "/";
    }

    if href == "/modules" {
        return path == "/modules" && module_query.is_none();
    }

    path == href || path.starts_with(&format!("{}/", href.trim_end_matches('/')))
}

fn module_group_order(group: &str) -> usize {
    match group {
        "Content" => 10,
        "Commerce" => 20,
        "Runtime" => 30,
        "Governance" => 40,
        "Automation" => 50,
        _ => 90,
    }
}

fn module_group_label(group: &str) -> String {
    let i18n = use_i18n();

    match group {
        "Content" => t_string!(i18n, app.nav.group.content).to_string(),
        "Commerce" => t_string!(i18n, app.nav.group.commerce).to_string(),
        "Runtime" => t_string!(i18n, app.nav.group.runtime).to_string(),
        "Governance" => t_string!(i18n, app.nav.group.governance).to_string(),
        "Automation" => t_string!(i18n, app.nav.group.automation).to_string(),
        _ => t_string!(i18n, app.nav.group.other).to_string(),
    }
}

fn module_group_icon(group: &str) -> &'static str {
    match group {
        "Content" => "content",
        "Commerce" => "commerce",
        "Runtime" => "runtime",
        "Governance" => "lock",
        "Automation" => "activity",
        _ => "box",
    }
}
