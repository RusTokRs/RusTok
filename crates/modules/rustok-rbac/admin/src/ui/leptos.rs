use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_grid::{ColumnFilters, GridPagination, RowSelection};
use rustok_grid_leptos::DataGrid;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    RbacPermissionRowViewModel, build_rbac_admin_overview_view_model, build_rbac_permission_rows,
    filter_rbac_permission_rows, filter_rbac_roles, format_rbac_admin_bootstrap_error,
    rbac_permission_grid_columns, rbac_role_grid_columns,
};
use crate::i18n::t;
use crate::model::RbacRoleInfo;
use crate::transport;

fn local_resource<S, Fut, T>(
    source: impl Fn() -> S + 'static,
    fetcher: impl Fn(S) -> Fut + 'static,
) -> LocalResource<T>
where
    S: 'static,
    Fut: std::future::Future<Output = T> + 'static,
    T: 'static,
{
    LocalResource::new(move || fetcher(source()))
}

#[component]
pub fn RbacAdmin() -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;

    let bootstrap = local_resource(
        move || (token.get(), tenant.get()),
        move |_| async move { transport::fetch_bootstrap().await },
    );

    view! {
        <div class="space-y-6">
            <header class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-2">
                    <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                        {t(locale.as_deref(), "rbac.badge", "rbac")}
                    </span>
                    <h1 class="text-2xl font-semibold text-card-foreground">
                        {t(locale.as_deref(), "rbac.title", "RBAC Runtime & Access Control")}
                    </h1>
                    <p class="max-w-3xl text-sm text-muted-foreground">
                        {t(locale.as_deref(), "rbac.subtitle", "Module-owned overview for live permission snapshot, role registry, and modular access catalog.")}
                    </p>
                </div>
            </header>

            <Suspense fallback=move || view! { <div class="h-32 animate-pulse rounded-2xl bg-muted"></div> }>
                {move || {
                    let loc = locale.clone();
                    bootstrap.get().map(move |result| match result {
                        Ok(data) => {
                            let loc_str = loc.as_deref();
                            let view_model = build_rbac_admin_overview_view_model(loc_str, data.clone());
                            let granted_permissions = view_model.granted_permissions;
                            let permission_rows = build_rbac_permission_rows(&data.module_permissions, &data.roles);

                            view! {
                                <section class="grid gap-4 lg:grid-cols-3">
                                    {view_model
                                        .info_cards
                                        .into_iter()
                                        .map(|card| view! { <InfoCard label=card.label value=card.value /> })
                                        .collect_view()}
                                </section>

                                <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                                    <div class="flex items-center justify-between gap-4">
                                        <div>
                                            <h2 class="text-lg font-semibold text-card-foreground">
                                                {granted_permissions.title}
                                            </h2>
                                            <p class="text-sm text-muted-foreground">
                                                {granted_permissions.subtitle}
                                            </p>
                                        </div>
                                        <div class="text-sm text-muted-foreground">
                                            {granted_permissions.count_label}
                                        </div>
                                    </div>
                                    <div class="mt-4 flex flex-wrap gap-2">
                                        {granted_permissions
                                            .permissions
                                            .into_iter()
                                            .map(|permission| view! {
                                                <span class="rounded-full border border-border bg-background px-3 py-1 text-xs text-muted-foreground">
                                                    {permission}
                                                </span>
                                            })
                                            .collect_view()}
                                    </div>
                                </section>

                                <RbacRolesGridSection roles=data.roles locale=loc.clone() />

                                <RbacPermissionsGridSection rows=permission_rows locale=loc.clone() />
                            }
                            .into_any()
                        }
                        Err(err) => view! {
                            <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-5 py-4 text-sm text-destructive">
                                {format_rbac_admin_bootstrap_error(loc.as_deref(), err)}
                            </div>
                        }
                        .into_any(),
                    })
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn InfoCard(label: String, value: String) -> impl IntoView {
    view! {
        <div class="rounded-2xl border border-border bg-card p-6 shadow-sm">
            <div class="text-sm text-muted-foreground">{label}</div>
            <div class="mt-2 text-lg font-semibold text-card-foreground break-all">{value}</div>
        </div>
    }
}

#[component]
fn RbacRolesGridSection(roles: Vec<RbacRoleInfo>, locale: Option<String>) -> impl IntoView {
    let loc_str = locale.as_deref();
    let is_ru = loc_str.map(|l| l.starts_with("ru")).unwrap_or(false);
    let columns = rbac_role_grid_columns(loc_str);

    let (search, set_search) = signal(String::new());
    let selection = RwSignal::new(RowSelection::new());
    let pagination = RwSignal::new(GridPagination::new(1, 10, roles.len() as u64));
    let filters = RwSignal::new(ColumnFilters::new());

    let stored_roles = StoredValue::new(roles);

    let filtered_roles = Memo::new(move |_| {
        let q = search.get();
        let all = stored_roles.get_value();
        let list = filter_rbac_roles(&all, &q);
        pagination.update(|p| p.total = list.len() as u64);
        list
    });

    let cell_renderer = Callback::new(move |(item, col_id): (RbacRoleInfo, String)| {
        match col_id.as_str() {
            "name" => view! {
                <span class="font-medium text-card-foreground">{item.display_name}</span>
            }
            .into_any(),
            "slug" => view! {
                <span class="font-mono text-xs text-muted-foreground">{item.slug}</span>
            }
            .into_any(),
            "permissions_count" => {
                let count = item.permissions.len();
                view! {
                    <span class="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-muted text-foreground">
                        {count}
                    </span>
                }
                .into_any()
            }
            "preview" => {
                let perms = item.permissions;
                view! {
                    <div class="flex flex-wrap gap-1 max-h-16 overflow-y-auto py-1">
                        {perms.into_iter().take(6).map(|p| view! {
                            <span class="rounded border border-border bg-background px-1.5 py-0.5 text-[10px] font-mono text-muted-foreground">
                                {p}
                            </span>
                        }).collect_view()}
                    </div>
                }
                .into_any()
            }
            _ => ().into_any(),
        }
    });

    let empty_msg = if is_ru {
        "Роли не найдены".to_string()
    } else {
        "No roles found".to_string()
    };

    view! {
        <section class="rounded-2xl border border-border bg-card p-6 shadow-sm space-y-4">
            <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
                <div>
                    <h2 class="text-lg font-semibold text-card-foreground">
                        {if is_ru { "Таблица ролей" } else { "Roles Table" }}
                    </h2>
                    <p class="text-sm text-muted-foreground">
                        {if is_ru { "Встроенные роли платформы и связанные разрешения" } else { "Platform roles and associated permission scopes" }}
                    </p>
                </div>
                <input
                    type="text"
                    placeholder=if is_ru { "Поиск ролей..." } else { "Search roles..." }
                    class="w-full sm:w-64 px-3 py-1.5 border border-border rounded-lg text-sm bg-background text-foreground"
                    prop:value=move || search.get()
                    on:input=move |ev| set_search.set(event_target_value(&ev))
                />
            </div>

            <DataGrid
                columns=columns
                data=filtered_roles.into()
                key_fn=|item: &RbacRoleInfo| item.slug.clone()
                cell_renderer=cell_renderer
                empty_message=empty_msg
                selection=selection
                pagination=pagination
                filters=filters
            />
        </section>
    }
}

#[component]
fn RbacPermissionsGridSection(
    rows: Vec<RbacPermissionRowViewModel>,
    locale: Option<String>,
) -> impl IntoView {
    let loc_str = locale.as_deref();
    let is_ru = loc_str.map(|l| l.starts_with("ru")).unwrap_or(false);
    let columns = rbac_permission_grid_columns(loc_str);

    let mut available_modules: Vec<String> = rows.iter().map(|r| r.module_slug.clone()).collect();
    available_modules.sort();
    available_modules.dedup();

    let (search, set_search) = signal(String::new());
    let (selected_module, set_selected_module) = signal(String::from("all"));
    let selection = RwSignal::new(RowSelection::new());
    let pagination = RwSignal::new(GridPagination::new(1, 25, rows.len() as u64));
    let filters = RwSignal::new(ColumnFilters::new());

    let stored_rows = StoredValue::new(rows);

    let filtered_rows = Memo::new(move |_| {
        let q = search.get();
        let m = selected_module.get();
        let all = stored_rows.get_value();
        let list = filter_rbac_permission_rows(&all, &q, Some(m.as_str()));
        pagination.update(|p| p.total = list.len() as u64);
        list
    });

    let cell_renderer = Callback::new(
        move |(item, col_id): (RbacPermissionRowViewModel, String)| match col_id.as_str() {
            "module" => view! {
                <span class="inline-flex items-center px-2 py-0.5 rounded text-xs font-mono font-medium border border-border bg-muted/40 text-foreground">
                    {item.module_slug}
                </span>
            }
            .into_any(),
            "permission" => view! {
                <span class="font-mono text-xs font-semibold text-foreground">
                    {item.permission}
                </span>
            }
            .into_any(),
            "roles" => {
                let roles = item.roles;
                view! {
                    <div class="flex flex-wrap gap-1">
                        {roles
                            .into_iter()
                            .map(|r| {
                                view! {
                                    <span class="rounded border border-border bg-background px-2 py-0.5 text-xs text-muted-foreground">
                                        {r}
                                    </span>
                                }
                            })
                            .collect_view()}
                    </div>
                }
                .into_any()
            }
            _ => ().into_any(),
        },
    );

    let empty_msg = if is_ru {
        "Разрешения не найдены".to_string()
    } else {
        "No permissions found".to_string()
    };

    view! {
        <section class="rounded-2xl border border-border bg-card p-6 shadow-sm space-y-4">
            <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
                <div>
                    <h2 class="text-lg font-semibold text-card-foreground">
                        {if is_ru { "Каталог разрешений" } else { "Permissions Catalog" }}
                    </h2>
                    <p class="text-sm text-muted-foreground">
                        {if is_ru { "Все разрешения платформы, сгруппированные по модулям" } else { "All platform permissions indexed by module ownership" }}
                    </p>
                </div>
                <div class="flex flex-col sm:flex-row items-stretch sm:items-center gap-2">
                    <select
                        class="px-3 py-1.5 border border-border rounded-lg text-sm bg-background text-foreground"
                        prop:value=move || selected_module.get()
                        on:change=move |ev| set_selected_module.set(event_target_value(&ev))
                    >
                        <option value="all">{if is_ru { "Все модули" } else { "All Modules" }}</option>
                        {available_modules
                            .into_iter()
                            .map(|m| {
                                let label = m.clone();
                                view! { <option value=m>{label}</option> }
                            })
                            .collect_view()}
                    </select>
                    <input
                        type="text"
                        placeholder=if is_ru { "Поиск прав..." } else { "Search permissions..." }
                        class="w-full sm:w-64 px-3 py-1.5 border border-border rounded-lg text-sm bg-background text-foreground"
                        prop:value=move || search.get()
                        on:input=move |ev| set_search.set(event_target_value(&ev))
                    />
                </div>
            </div>

            <DataGrid
                columns=columns
                data=filtered_rows.into()
                key_fn=|item: &RbacPermissionRowViewModel| item.permission.clone()
                cell_renderer=cell_renderer
                empty_message=empty_msg
                selection=selection
                pagination=pagination
                filters=filters
            />
        </section>
    }
}
