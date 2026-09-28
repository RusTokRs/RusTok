use leptos::prelude::*;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_router::hooks::use_navigate;
use rustok_ui_core::UiRouteContext;
use rustok_grid_leptos::prelude::*;

use crate::core::{
    filter_products, item_product_kind, product_grid_columns, ProductKind,
};
use crate::model::ProductListItem;
use crate::transport;

#[component]
pub fn ProductGridPage() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let is_ru = locale.as_deref() == Some("ru");
    let base_route = route_context.module_route_base("product");
    let token = use_token();
    let tenant = use_tenant();

    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (is_busy, set_is_busy) = signal(false);
    let (error_msg, set_error_msg) = signal(Option::<String>::None);
    let (search_query, set_search_query) = signal(String::new());
    let (dropdown_open, set_dropdown_open) = signal(false);

    // Grid state
    let columns = product_grid_columns(locale.as_deref());
    let (filters, set_filters) = signal(ColumnFilters::new());
    let selection = RwSignal::new(RowSelection::new());
    let pagination = RwSignal::new(GridPagination::new(1, 20, 0));

    // Load products resource
    let products_resource = LocalResource::new(move || {
        let tok = token.get();
        let ten = tenant.get();
        let loc = locale.clone();
        let _ = refresh_nonce.get();
        async move {
            let bootstrap = transport::fetch_bootstrap(tok.clone(), ten.clone())
                .await
                .map_err(|e| e.to_string())?;
            let res = transport::fetch_products(
                tok,
                ten,
                bootstrap.current_tenant.id,
                loc,
                None,
                None,
            )
            .await
            .map_err(|e| e.to_string())?;
            Ok::<Vec<ProductListItem>, String>(res.items)
        }
    });

    // Reactive filtered data
    let filtered_data = Memo::new(move |_| {
        let raw = products_resource.get().and_then(Result::ok).unwrap_or_default();
        let query = search_query.get().trim().to_lowercase();
        let current_filters = filters.get();

        let filtered = filter_products(&raw, &current_filters);
        if query.is_empty() {
            filtered
        } else {
            filtered
                .into_iter()
                .filter(|item| {
                    item.title.to_lowercase().contains(&query)
                        || item.handle.to_lowercase().contains(&query)
                        || item.seller_id.as_deref().map(|s| s.to_lowercase().contains(&query)).unwrap_or(false)
                        || item.vendor.as_deref().map(|v| v.to_lowercase().contains(&query)).unwrap_or(false)
                })
                .collect()
        }
    });

    // Bulk actions
    let on_bulk_status = {
        let base_token = token;
        let base_tenant = tenant;
        move |target_status: &'static str| {
            let selected_ids = selection.get().to_vec();
            if selected_ids.is_empty() {
                return;
            }
            set_is_busy.set(true);
            set_error_msg.set(None);

            let tok = base_token.get_untracked();
            let ten = base_tenant.get_untracked();

            leptos::task::spawn_local(async move {
                let Ok(bootstrap) = transport::fetch_bootstrap(tok.clone(), ten.clone()).await else {
                    set_is_busy.set(false);
                    set_error_msg.set(Some("Failed to load bootstrap for status mutation".to_string()));
                    return;
                };

                for id in selected_ids {
                    let _ = transport::change_product_status(
                        tok.clone(),
                        ten.clone(),
                        bootstrap.current_tenant.id.clone(),
                        bootstrap.me.id.clone(),
                        id,
                        target_status,
                    )
                    .await;
                }

                set_is_busy.set(false);
                selection.update(|s| s.clear());
                set_refresh_nonce.update(|n| *n += 1);
            });
        }
    };

    let on_bulk_delete = {
        let base_token = token;
        let base_tenant = tenant;
        move || {
            let selected_ids = selection.get().to_vec();
            if selected_ids.is_empty() {
                return;
            }
            set_is_busy.set(true);
            set_error_msg.set(None);

            let tok = base_token.get_untracked();
            let ten = base_tenant.get_untracked();

            leptos::task::spawn_local(async move {
                let Ok(bootstrap) = transport::fetch_bootstrap(tok.clone(), ten.clone()).await else {
                    set_is_busy.set(false);
                    set_error_msg.set(Some("Failed to load bootstrap for deletion".to_string()));
                    return;
                };

                for id in selected_ids {
                    let _ = transport::delete_product(
                        tok.clone(),
                        ten.clone(),
                        bootstrap.current_tenant.id.clone(),
                        bootstrap.me.id.clone(),
                        id,
                    )
                    .await;
                }

                set_is_busy.set(false);
                selection.update(|s| s.clear());
                set_refresh_nonce.update(|n| *n += 1);
            });
        }
    };

    // Quick single item status change
    let on_quick_status = {
        let base_token = token;
        let base_tenant = tenant;
        move |id: String, next_status: String| {
            set_is_busy.set(true);
            set_error_msg.set(None);

            let tok = base_token.get_untracked();
            let ten = base_tenant.get_untracked();

            leptos::task::spawn_local(async move {
                let Ok(bootstrap) = transport::fetch_bootstrap(tok.clone(), ten.clone()).await else {
                    set_is_busy.set(false);
                    return;
                };

                let _ = transport::change_product_status(
                    tok,
                    ten,
                    bootstrap.current_tenant.id,
                    bootstrap.me.id,
                    id,
                    &next_status,
                )
                .await;

                set_is_busy.set(false);
                set_refresh_nonce.update(|n| *n += 1);
            });
        }
    };

    // Quick single delete
    let on_quick_delete = {
        let base_token = token;
        let base_tenant = tenant;
        move |id: String| {
            set_is_busy.set(true);
            set_error_msg.set(None);

            let tok = base_token.get_untracked();
            let ten = base_tenant.get_untracked();

            leptos::task::spawn_local(async move {
                let Ok(bootstrap) = transport::fetch_bootstrap(tok.clone(), ten.clone()).await else {
                    set_is_busy.set(false);
                    return;
                };

                let _ = transport::delete_product(
                    tok,
                    ten,
                    bootstrap.current_tenant.id,
                    bootstrap.me.id,
                    id,
                )
                .await;

                set_is_busy.set(false);
                set_refresh_nonce.update(|n| *n += 1);
            });
        }
    };

    let base_route_for_cell = base_route.clone();
    let base_route_for_click = base_route.clone();
    let on_quick_status_cb = StoredValue::new(on_quick_status);
    let on_quick_delete_cb = StoredValue::new(on_quick_delete);

    // Cell renderer callback
    let cell_renderer = Callback::new(move |(item, col_id): (ProductListItem, String)| {
        let base_route = base_route_for_cell.clone();
        let edit_href = format!("{base_route}/edit/{}", item.id);
        let on_status = on_quick_status_cb.get_value();
        let on_del = on_quick_delete_cb.get_value();

        match col_id.as_str() {
            "image" => {
                view! {
                    <div class="flex items-center justify-center w-full h-full">
                        <div class="w-9 h-9 rounded-lg bg-muted flex items-center justify-center overflow-hidden border border-border/60">
                            <span class="text-base text-muted-foreground/70">"📦"</span>
                        </div>
                    </div>
                }
                .into_any()
            }
            "title" => {
                let tags = item.tags.clone();
                view! {
                    <div class="flex flex-col gap-0.5 max-w-full">
                        <a
                            href=edit_href.clone()
                            class="font-medium text-foreground hover:text-primary transition-colors truncate text-xs"
                            title=item.title.clone()
                        >
                            {item.title}
                        </a>
                        <div class="flex items-center gap-1.5 flex-wrap">
                            <span class="font-mono text-[10px] text-muted-foreground/70 truncate">
                                {format!("/{}", item.handle)}
                            </span>
                            {tags.into_iter().take(2).map(|tag| {
                                view! {
                                    <span class="inline-flex items-center px-1.5 py-0.2 rounded text-[10px] bg-secondary text-secondary-foreground border border-border/50">
                                        {tag}
                                    </span>
                                }
                            }).collect_view()}
                        </div>
                    </div>
                }
                .into_any()
            }
            "product_type" => {
                let kind = item_product_kind(&item);
                view! {
                    <div class="flex items-center">
                        <span class=kind.badge_class()>
                            {kind.label(locale.as_deref())}
                        </span>
                    </div>
                }
                .into_any()
            }
            "sku" => {
                let display = item
                    .seller_id
                    .as_deref()
                    .or(item.vendor.as_deref())
                    .unwrap_or("—");
                view! {
                    <div class="flex flex-col">
                        <span class="font-mono text-xs text-foreground/90 truncate">{display}</span>
                        {item.vendor.as_ref().map(|v| view! {
                            <span class="text-[10px] text-muted-foreground truncate">{format!("by {}", v)}</span>
                        })}
                    </div>
                }
                .into_any()
            }
            "status" => {
                let status_upper = item.status.to_uppercase();
                let (badge_cls, label) = match status_upper.as_str() {
                    "ACTIVE" => (
                        "bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border-emerald-500/20",
                        if is_ru { "Активен" } else { "Active" },
                    ),
                    "ARCHIVED" => (
                        "bg-slate-500/10 text-slate-600 dark:text-slate-400 border-slate-500/20",
                        if is_ru { "В архиве" } else { "Archived" },
                    ),
                    _ => (
                        "bg-amber-500/10 text-amber-600 dark:text-amber-400 border-amber-500/20",
                        if is_ru { "Черновик" } else { "Draft" },
                    ),
                };
                view! {
                    <span class=format!("inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-semibold border {badge_cls}")>
                        {label}
                    </span>
                }
                .into_any()
            }
            "created_at" => {
                let date_str = item.created_at.split('T').next().unwrap_or(&item.created_at);
                view! {
                    <span class="text-xs text-muted-foreground whitespace-nowrap">
                        {date_str.to_string()}
                    </span>
                }
                .into_any()
            }
            "actions" => {
                let item_id_status = item.id.clone();
                let item_id_del = item.id.clone();
                let current_status = item.status.to_uppercase();
                let next_status = if current_status == "ACTIVE" {
                    "DRAFT".to_string()
                } else {
                    "ACTIVE".to_string()
                };
                let status_icon = if current_status == "ACTIVE" { "⏸" } else { "▶" };
                let status_title = if current_status == "ACTIVE" {
                    if is_ru { "Перевести в черновик" } else { "Set to draft" }
                } else {
                    if is_ru { "Опубликовать" } else { "Publish" }
                };

                view! {
                    <div class="flex items-center justify-end gap-1.5">
                        <a
                            href=edit_href
                            class="inline-flex items-center justify-center h-7 px-2.5 rounded-md text-xs font-medium bg-secondary text-secondary-foreground hover:bg-accent transition"
                            title=if is_ru { "Редактировать" } else { "Edit" }
                        >
                            {if is_ru { "Изменить" } else { "Edit" }}
                        </a>
                        <button
                            type="button"
                            class="inline-flex items-center justify-center w-7 h-7 rounded-md text-xs text-muted-foreground hover:text-foreground hover:bg-accent transition"
                            title=status_title
                            on:click=move |_| on_status(item_id_status.clone(), next_status.clone())
                        >
                            {status_icon}
                        </button>
                        <button
                            type="button"
                            class="inline-flex items-center justify-center w-7 h-7 rounded-md text-xs text-rose-500 hover:text-rose-700 hover:bg-rose-50 dark:hover:bg-rose-950/40 transition"
                            title=if is_ru { "Удалить" } else { "Delete" }
                            on:click=move |_| on_del(item_id_del.clone())
                        >
                            "🗑"
                        </button>
                    </div>
                }
                .into_any()
            }
            _ => ().into_any(),
        }
    });

    let navigate = use_navigate();
    let on_row_click = Callback::new(move |item: ProductListItem| {
        let href = format!("{base_route_for_click}/edit/{}", item.id);
        navigate(&href, Default::default());
    });

    let on_filters_change = Callback::new(move |new_filters: ColumnFilters| {
        set_filters.set(new_filters);
    });

    let new_simple_href = format!("{base_route}/new?type=simple");
    let new_variable_href = format!("{base_route}/new?type=variable");
    let new_bundle_href = format!("{base_route}/new?type=bundle");
    let new_digital_href = format!("{base_route}/new?type=digital");

    view! {
        <div class="flex flex-col gap-5 w-full">
            // Top Action Toolbar
            <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4 bg-card rounded-2xl border border-border p-4 shadow-sm">
                <div>
                    <h1 class="text-xl font-bold tracking-tight text-foreground flex items-center gap-2">
                        <span>{if is_ru { "Каталог товаров" } else { "Product Catalog" }}</span>
                        <span class="text-xs font-normal px-2 py-0.5 rounded-full bg-primary/10 text-primary border border-primary/20">
                            {move || filtered_data.get().len()}
                        </span>
                    </h1>
                    <p class="text-xs text-muted-foreground mt-0.5">
                        {if is_ru {
                            "Управление всеми типами товаров: простые, вариативные, комплекты и цифровые"
                        } else {
                            "Manage all product types: simple, variable, bundles, and digital downloads"
                        }}
                    </p>
                </div>

                <div class="flex items-center gap-2.5 flex-wrap">
                    // Search bar
                    <div class="relative min-w-[200px] sm:min-w-[240px]">
                        <input
                            type="text"
                            placeholder=if is_ru { "Быстрый поиск..." } else { "Quick search..." }
                            prop:value=move || search_query.get()
                            on:input=move |ev| set_search_query.set(event_target_value(&ev))
                            class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground placeholder:text-muted-foreground/60 outline-none focus:border-primary focus:ring-1 focus:ring-primary/20 transition"
                        />
                    </div>

                    // Refresh button
                    <button
                        type="button"
                        class="inline-flex items-center justify-center h-9 px-3 rounded-xl border border-border bg-background text-xs font-medium text-foreground hover:bg-accent transition"
                        title=if is_ru { "Обновить" } else { "Refresh" }
                        on:click=move |_| set_refresh_nonce.update(|n| *n += 1)
                    >
                        "↻"
                    </button>

                    // Prominent Split Add Product Dropdown
                    <div class="relative">
                        <div class="inline-flex rounded-xl shadow-sm">
                            <a
                                href=new_simple_href.clone()
                                class="inline-flex items-center justify-center gap-1.5 h-9 px-3.5 rounded-l-xl bg-primary text-primary-foreground text-xs font-semibold hover:bg-primary/90 transition shadow-sm"
                            >
                                <span>"+"</span>
                                <span>{if is_ru { "Добавить товар" } else { "Add Product" }}</span>
                            </a>
                            <button
                                type="button"
                                class="inline-flex items-center justify-center w-8 h-9 rounded-r-xl bg-primary text-primary-foreground border-l border-primary-foreground/20 hover:bg-primary/90 transition text-xs"
                                on:click=move |_| set_dropdown_open.update(|open| *open = !*open)
                            >
                                "▾"
                            </button>
                        </div>

                        // Dropdown menu
                        <Show when=move || dropdown_open.get()>
                            <div
                                class="absolute right-0 top-11 z-50 w-72 rounded-2xl border border-border bg-popover p-2 shadow-xl animate-in fade-in zoom-in-95 duration-100"
                                on:click=move |_| set_dropdown_open.set(false)
                            >
                                <div class="px-2 py-1 text-[11px] font-semibold text-muted-foreground uppercase tracking-wider">
                                    {if is_ru { "Выберите тип товара" } else { "Select Product Type" }}
                                </div>
                                <div class="flex flex-col gap-1 mt-1">
                                    <a
                                        href=new_simple_href.clone()
                                        class="flex items-start gap-2.5 p-2 rounded-xl hover:bg-accent transition-colors"
                                    >
                                        <span class="text-lg">"📦"</span>
                                        <div class="flex flex-col">
                                            <span class="text-xs font-medium text-foreground">{ProductKind::Simple.label(locale.as_deref())}</span>
                                            <span class="text-[10px] text-muted-foreground">{ProductKind::Simple.description(locale.as_deref())}</span>
                                        </div>
                                    </a>
                                    <a
                                        href=new_variable_href
                                        class="flex items-start gap-2.5 p-2 rounded-xl hover:bg-accent transition-colors"
                                    >
                                        <span class="text-lg">"🎨"</span>
                                        <div class="flex flex-col">
                                            <span class="text-xs font-medium text-foreground">{ProductKind::Variable.label(locale.as_deref())}</span>
                                            <span class="text-[10px] text-muted-foreground">{ProductKind::Variable.description(locale.as_deref())}</span>
                                        </div>
                                    </a>
                                    <a
                                        href=new_bundle_href
                                        class="flex items-start gap-2.5 p-2 rounded-xl hover:bg-accent transition-colors"
                                    >
                                        <span class="text-lg">"🎁"</span>
                                        <div class="flex flex-col">
                                            <span class="text-xs font-medium text-foreground">{ProductKind::Bundle.label(locale.as_deref())}</span>
                                            <span class="text-[10px] text-muted-foreground">{ProductKind::Bundle.description(locale.as_deref())}</span>
                                        </div>
                                    </a>
                                    <a
                                        href=new_digital_href
                                        class="flex items-start gap-2.5 p-2 rounded-xl hover:bg-accent transition-colors"
                                    >
                                        <span class="text-lg">"💾"</span>
                                        <div class="flex flex-col">
                                            <span class="text-xs font-medium text-foreground">{ProductKind::Digital.label(locale.as_deref())}</span>
                                            <span class="text-[10px] text-muted-foreground">{ProductKind::Digital.description(locale.as_deref())}</span>
                                        </div>
                                    </a>
                                </div>
                            </div>
                        </Show>
                    </div>
                </div>
            </div>

            // Error alert
            <Show when=move || error_msg.get().is_some()>
                <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-2.5 text-xs text-destructive flex items-center justify-between">
                    <span>{move || error_msg.get().unwrap_or_default()}</span>
                    <button type="button" class="text-xs font-bold" on:click=move |_| set_error_msg.set(None)>"✕"</button>
                </div>
            </Show>

            // Bulk Actions Bar (Visible when items selected)
            <Show when=move || !selection.get().is_empty()>
                <div class="flex items-center justify-between gap-3 bg-primary/5 border border-primary/20 rounded-xl px-4 py-2.5 animate-in fade-in slide-in-from-top-1 duration-150">
                    <div class="flex items-center gap-2">
                        <span class="w-2 h-2 rounded-full bg-primary animate-pulse" />
                        <span class="text-xs font-semibold text-foreground">
                            {move || format!("{} {}", selection.get().count(), if is_ru { "выбрано" } else { "selected" })}
                        </span>
                    </div>
                    <div class="flex items-center gap-2">
                        <button
                            type="button"
                            class="h-7 px-2.5 rounded-lg border border-border bg-background text-[11px] font-medium text-foreground hover:bg-accent transition"
                            on:click={
                                let bulk_st = on_bulk_status.clone();
                                move |_| bulk_st("ACTIVE")
                            }
                        >
                            {if is_ru { "Активировать" } else { "Set Active" }}
                        </button>
                        <button
                            type="button"
                            class="h-7 px-2.5 rounded-lg border border-border bg-background text-[11px] font-medium text-foreground hover:bg-accent transition"
                            on:click={
                                let bulk_st = on_bulk_status.clone();
                                move |_| bulk_st("DRAFT")
                            }
                        >
                            {if is_ru { "В черновик" } else { "Set Draft" }}
                        </button>
                        <button
                            type="button"
                            class="h-7 px-2.5 rounded-lg border border-border bg-background text-[11px] font-medium text-foreground hover:bg-accent transition"
                            on:click={
                                let bulk_st = on_bulk_status.clone();
                                move |_| bulk_st("ARCHIVED")
                            }
                        >
                            {if is_ru { "В архив" } else { "Archive" }}
                        </button>
                        <button
                            type="button"
                            class="h-7 px-2.5 rounded-lg border border-rose-300 dark:border-rose-900 bg-rose-500/10 text-[11px] font-medium text-rose-600 dark:text-rose-400 hover:bg-rose-500/20 transition"
                            on:click=move |_| on_bulk_delete()
                        >
                            {if is_ru { "Удалить выбранные" } else { "Delete Selected" }}
                        </button>
                        <button
                            type="button"
                            class="h-7 px-2 rounded-lg text-[11px] text-muted-foreground hover:text-foreground transition"
                            on:click=move |_| selection.update(|s| s.clear())
                        >
                            {if is_ru { "Снять выбор" } else { "Clear" }}
                        </button>
                    </div>
                </div>
            </Show>

            // Main DataGrid
            <DataGrid
                columns=columns
                data=Signal::derive(move || filtered_data.get())
                key_fn=|item: &ProductListItem| item.id.clone()
                cell_renderer=cell_renderer
                is_loading=Signal::derive(move || is_busy.get() || products_resource.get().is_none())
                empty_message=if is_ru { "Товары не найдены" } else { "No products found" }.to_string()
                selection=selection
                pagination=pagination
                on_filter_change=on_filters_change
                on_row_click=on_row_click
            />
        </div>
    }
}
