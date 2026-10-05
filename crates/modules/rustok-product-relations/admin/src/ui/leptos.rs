use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_grid::{ColumnFilters, FilterValue, GridPagination, RowSelection};
use rustok_grid_leptos::DataGrid;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    ProductRelationsTransportProfile, build_product_relations_panel_copy, filter_relations,
    relation_grid_columns, selected_transport_profile, validate_target_product_id,
};
use crate::model::{
    CreateProductRelationDraft, ProductRelationItem, ProductRelationsAdminCommand,
    ProductRelationsAdminFilters,
};
use crate::transport::{
    ProductRelationsTransportContext, execute_product_relations_command, load_product_relations,
};

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
pub fn ProductRelationsPanel(
    #[prop(optional)] locale: Option<String>,
    product_id: String,
    #[prop(optional)] token: Option<Signal<Option<String>>>,
    #[prop(optional)] tenant: Option<Signal<Option<String>>>,
    busy: ReadSignal<bool>,
    set_busy: WriteSignal<bool>,
    set_error: WriteSignal<Option<String>>,
) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let effective_locale = locale.clone().or_else(|| route_context.locale.clone());
    let copy = std::sync::Arc::new(build_product_relations_panel_copy(effective_locale.as_deref()));
    let is_ru = effective_locale.as_deref().map(|l| l.starts_with("ru")).unwrap_or(false);
    let columns = relation_grid_columns(effective_locale.as_deref());

    let (selected_type, set_selected_type) = signal("cross_sell".to_string());
    let (show_add, set_show_add) = signal(false);
    let (target_product_id, set_target_product_id) = signal(String::new());
    let (position, set_position) = signal(0_i32);
    let (reload_seq, set_reload_seq) = signal(0_usize);

    let search = RwSignal::new(String::new());
    let filters = RwSignal::new(ColumnFilters::new());
    let selection = RwSignal::new(RowSelection::new());
    let pagination = RwSignal::new(GridPagination::new(1, 25, 0));

    let profile = selected_transport_profile(option_env!("RUSTOK_UI_TRANSPORT_PROFILE"));
    let token_val = token.map(|t| t.get()).unwrap_or_default();
    let tenant_val = tenant.map(|t| t.get()).unwrap_or_default();
    let transport = match profile {
        ProductRelationsTransportProfile::Native => ProductRelationsTransportContext::native(),
        ProductRelationsTransportProfile::Graphql => {
            ProductRelationsTransportContext::graphql(token_val, tenant_val)
        }
    };

    let p_id = product_id.clone();
    let transport_for_loader = transport.clone();
    let relations_resource = local_resource(
        move || (selected_type.get(), reload_seq.get()),
        move |(rel_type, _)| {
            let context = transport_for_loader.clone();
            let prod_id = p_id.clone();
            async move {
                load_product_relations(
                    context,
                    ProductRelationsAdminFilters {
                        product_id: Some(prod_id),
                        relation_type: Some(rel_type),
                    },
                )
                .await
            }
        },
    );

    let filtered_relations = Memo::new({
        let relations_resource = relations_resource.clone();
        move |_| {
            let relations = relations_resource.get().and_then(Result::ok).unwrap_or_default();
            let s_val = search.get();
            let col_filters = filters.get();
            let target_filter = col_filters.get("target_product_id").and_then(|f| match f {
                FilterValue::Text(s) if !s.trim().is_empty() => Some(s.as_str()),
                _ => None,
            });
            let query = if !s_val.trim().is_empty() {
                Some(s_val.as_str())
            } else {
                target_filter
            };
            let list = filter_relations(&relations, query, None);
            pagination.update(|p| p.total = list.len() as u64);
            list
        }
    });

    let paged_relations = Memo::new(move |_| {
        let list = filtered_relations.get();
        let p = pagination.get();
        let start = (p.page.saturating_sub(1)) * p.page_size;
        list.into_iter().skip(start).take(p.page_size).collect::<Vec<_>>()
    });

    let on_add_submit = {
        let transport = transport.clone();
        let prod_id = product_id.clone();
        move |ev: leptos::ev::SubmitEvent| {
            ev.prevent_default();
            let target_id_val = target_product_id.get_untracked().trim().to_string();

            if let Err(e) = validate_target_product_id(&target_id_val) {
                set_error.set(Some(e.to_string()));
                return;
            }
            if target_id_val == prod_id {
                set_error.set(Some("A product cannot be related to itself.".to_string()));
                return;
            }

            let draft = CreateProductRelationDraft {
                product_id: prod_id.clone(),
                related_product_id: target_id_val,
                relation_type: selected_type.get_untracked(),
                position: Some(position.get_untracked()),
                metadata: None,
            };

            set_busy.set(true);
            set_error.set(None);
            let transport = transport.clone();
            spawn_local(async move {
                match execute_product_relations_command(
                    transport,
                    uuid::Uuid::new_v4().to_string(),
                    ProductRelationsAdminCommand::Add { draft },
                )
                .await
                {
                    Ok(res) if res.is_success() => {
                        set_target_product_id.set(String::new());
                        set_position.set(0);
                        set_show_add.set(false);
                        set_reload_seq.update(|v| *v += 1);
                    }
                    Ok(_) => set_error.set(Some("Failed to add relation.".to_string())),
                    Err(err) => set_error.set(Some(err.to_string())),
                }
                set_busy.set(false);
            });
        }
    };

    let on_remove = {
        let transport = transport.clone();
        move |rel_id: String| {
            set_busy.set(true);
            set_error.set(None);
            let transport = transport.clone();
            spawn_local(async move {
                match execute_product_relations_command(
                    transport,
                    uuid::Uuid::new_v4().to_string(),
                    ProductRelationsAdminCommand::Remove { id: rel_id },
                )
                .await
                {
                    Ok(res) if res.is_success() => set_reload_seq.update(|v| *v += 1),
                    Ok(_) => set_error.set(Some("Failed to remove relation.".to_string())),
                    Err(err) => set_error.set(Some(err.to_string())),
                }
                set_busy.set(false);
            });
        }
    };

    let on_remove_selected = {
        let transport = transport.clone();
        move |_| {
            let ids = selection.get().to_vec();
            if ids.is_empty() {
                return;
            }
            set_busy.set(true);
            set_error.set(None);
            let transport = transport.clone();
            spawn_local(async move {
                let mut failed = 0;
                for id in ids {
                    let res = execute_product_relations_command(
                        transport.clone(),
                        uuid::Uuid::new_v4().to_string(),
                        ProductRelationsAdminCommand::Remove { id },
                    )
                    .await;
                    if res.is_err() || !res.unwrap().is_success() {
                        failed += 1;
                    }
                }
                selection.update(|s| s.clear());
                if failed > 0 {
                    set_error.set(Some(format!("Failed to remove {failed} relation(s).")));
                }
                set_reload_seq.update(|v| *v += 1);
                set_busy.set(false);
            });
        }
    };

    let on_move = {
        let transport = transport.clone();
        let prod_id = product_id.clone();
        move |items: Vec<ProductRelationItem>, index: usize, delta: isize| {
            let new_index = index as isize + delta;
            if new_index < 0 || new_index >= items.len() as isize {
                return;
            }
            let new_index = new_index as usize;
            let mut ordered = items;
            ordered.swap(index, new_index);
            let ordered_ids: Vec<String> = ordered.iter().map(|item| item.id.clone()).collect();
            let rel_type = selected_type.get_untracked();
            let prod_id = prod_id.clone();

            set_busy.set(true);
            set_error.set(None);
            let transport = transport.clone();
            spawn_local(async move {
                match execute_product_relations_command(
                    transport,
                    uuid::Uuid::new_v4().to_string(),
                    ProductRelationsAdminCommand::Reorder {
                        product_id: prod_id,
                        relation_type: rel_type,
                        ordered_ids,
                    },
                )
                .await
                {
                    Ok(res) if res.is_success() => set_reload_seq.update(|v| *v += 1),
                    Ok(_) => set_error.set(Some("Failed to reorder relations.".to_string())),
                    Err(err) => set_error.set(Some(err.to_string())),
                }
                set_busy.set(false);
            });
        }
    };

    let cell_on_move = on_move.clone();
    let cell_on_remove = on_remove.clone();
    let cell_copy_move_up = copy.move_up.clone();
    let cell_copy_move_down = copy.move_down.clone();
    let cell_copy_remove = copy.remove.clone();

    let cell_renderer = Callback::new(move |(item, col_id): (ProductRelationItem, String)| {
        match col_id.as_str() {
            "position" => {
                view! {
                    <span class="font-mono text-xs text-muted-foreground">{item.position}</span>
                }
                .into_any()
            }
            "target_product_id" => {
                let target_id = item.related_product_id.clone();
                view! {
                    <code class="text-xs font-mono px-1.5 py-0.5 rounded bg-muted text-foreground">
                        {target_id}
                    </code>
                }
                .into_any()
            }
            "actions" => {
                let item_id = item.id.clone();
                let all_items = relations_resource.get().and_then(Result::ok).unwrap_or_default();
                let idx = all_items.iter().position(|r| r.id == item_id).unwrap_or(0);
                let is_first = idx == 0;
                let is_last = idx + 1 >= all_items.len();
                let on_move_up = cell_on_move.clone();
                let on_move_down = cell_on_move.clone();
                let on_del = cell_on_remove.clone();
                let move_up_title = cell_copy_move_up.clone();
                let move_down_title = cell_copy_move_down.clone();
                let remove_label = cell_copy_remove.clone();
                let items_for_up = all_items.clone();
                let items_for_down = all_items.clone();

                view! {
                    <div class="flex items-center justify-end gap-1">
                        <button
                            type="button"
                            class="rounded p-1 text-xs text-muted-foreground hover:bg-gray-100 dark:hover:bg-gray-800 hover:text-foreground disabled:opacity-30"
                            disabled=move || is_first || busy.get()
                            title=move_up_title
                            on:click=move |_| on_move_up(items_for_up.clone(), idx, -1)
                        >
                            "↑"
                        </button>
                        <button
                            type="button"
                            class="rounded p-1 text-xs text-muted-foreground hover:bg-gray-100 dark:hover:bg-gray-800 hover:text-foreground disabled:opacity-30"
                            disabled=move || is_last || busy.get()
                            title=move_down_title
                            on:click=move |_| on_move_down(items_for_down.clone(), idx, 1)
                        >
                            "↓"
                        </button>
                        <button
                            type="button"
                            class="rounded px-2 py-0.5 text-xs text-rose-600 hover:bg-rose-50 border border-rose-200 dark:border-rose-900/40 transition disabled:opacity-50 ml-2"
                            disabled=move || busy.get()
                            on:click={
                                let item_id = item_id.clone();
                                move |_| on_del(item_id.clone())
                            }
                        >
                            {remove_label}
                        </button>
                    </div>
                }
                .into_any()
            }
            _ => ().into_any(),
        }
    });

    view! {
        <section class="rounded-3xl border border-border bg-card p-6 shadow-sm dark:border-gray-800 dark:bg-gray-900">
            <div class="flex items-center justify-between gap-3">
                <div>
                    <h3 class="text-lg font-semibold text-card-foreground dark:text-white">{copy.title.clone()}</h3>
                    <p class="text-sm text-muted-foreground dark:text-gray-400">{copy.subtitle.clone()}</p>
                </div>
                <button
                    type="button"
                    class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50 dark:border-gray-700 dark:text-gray-200"
                    disabled=move || busy.get()
                    on:click=move |_| set_show_add.update(|v| *v = !*v)
                >
                    {copy.add.clone()}
                </button>
            </div>

            <div class="mt-4 flex flex-wrap gap-2 border-b border-border pb-3 dark:border-gray-800">
                {
                    let types = [
                        ("cross_sell", copy.tab_cross_sell.clone()),
                        ("up_sell", copy.tab_up_sell.clone()),
                        ("related", copy.tab_related.clone()),
                        ("accessory", copy.tab_accessory.clone()),
                        ("alternative", copy.tab_alternative.clone()),
                    ];
                    types.into_iter().map(|(rel_key, label)| {
                        let is_active = move || selected_type.get() == rel_key;
                        let rel_key_str = rel_key.to_string();
                        view! {
                            <button
                                type="button"
                                class=move || {
                                    if is_active() {
                                        "rounded-lg bg-blue-600 px-3 py-1.5 text-xs font-medium text-white shadow-sm transition"
                                    } else {
                                        "rounded-lg border border-border px-3 py-1.5 text-xs font-medium text-muted-foreground transition hover:bg-gray-100 dark:hover:bg-gray-800 hover:text-foreground dark:border-gray-700"
                                    }
                                }
                                on:click=move |_| {
                                    set_selected_type.set(rel_key_str.clone());
                                    selection.update(|s| s.clear());
                                    pagination.update(|p| p.page = 1);
                                }
                            >
                                {label}
                            </button>
                        }
                    }).collect_view()
                }
            </div>

            {
                let copy_for_form = copy.clone();
                view! {
                    <Show when=move || show_add.get()>
                        <form class="mt-4 rounded-2xl border border-border/70 bg-background/50 p-4 space-y-3 dark:border-gray-700 dark:bg-gray-800/40" on:submit={
                            let on_submit = on_add_submit.clone();
                            move |ev| on_submit(ev)
                        }>
                            <div class="grid gap-3 md:grid-cols-2">
                                <div>
                                    <label class="block text-xs font-medium text-muted-foreground mb-1 dark:text-gray-300">{copy_for_form.target_product_id.clone()}</label>
                                    <input
                                        class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-blue-500 font-mono text-xs dark:bg-gray-800 dark:border-gray-700 dark:text-white"
                                        placeholder="Product UUID"
                                        prop:value=move || target_product_id.get()
                                        on:input=move |ev| set_target_product_id.set(event_target_value(&ev))
                                    />
                                </div>
                                <div>
                                    <label class="block text-xs font-medium text-muted-foreground mb-1 dark:text-gray-300">{copy_for_form.position.clone()}</label>
                                    <input
                                        type="number"
                                        class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-blue-500 text-xs dark:bg-gray-800 dark:border-gray-700 dark:text-white"
                                        prop:value=move || position.get().to_string()
                                        on:input=move |ev| {
                                            if let Ok(val) = event_target_value(&ev).parse::<i32>() {
                                                set_position.set(val);
                                            }
                                        }
                                    />
                                </div>
                            </div>
                            <div class="flex justify-end gap-2">
                                <button
                                    type="button"
                                    class="rounded-lg px-3 py-1.5 text-xs text-muted-foreground hover:bg-accent transition dark:text-gray-400"
                                    on:click=move |_| set_show_add.set(false)
                                >
                                    "Cancel"
                                </button>
                                <button
                                    type="submit"
                                    class="rounded-lg bg-blue-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-blue-700 transition disabled:opacity-50"
                                    disabled=move || busy.get()
                                >
                                    {copy_for_form.add.clone()}
                                </button>
                            </div>
                        </form>
                    </Show>
                }
            }

            <div class="mt-4 space-y-4">
                // Search input
                <div class="flex flex-col sm:flex-row gap-4">
                    <div class="flex-1">
                        <input
                            type="text"
                            placeholder={if is_ru { "Поиск по ID товара..." } else { "Search by product ID..." }}
                            class="w-full px-3 py-2 border rounded-xl shadow-sm focus:ring-primary focus:border-primary text-sm bg-background border-border text-foreground placeholder:text-muted-foreground"
                            prop:value=move || search.get()
                            on:input=move |ev| search.set(event_target_value(&ev))
                        />
                    </div>
                </div>

                // Selection toolbar
                <Show when=move || !selection.get().is_empty()>
                    <div class="flex items-center justify-between gap-3 px-4 py-2.5 rounded-xl border border-primary/20 bg-primary/5 text-sm">
                        <div class="flex items-center gap-2">
                            <span class="font-medium text-foreground">
                                {move || format!("{} {} {}", selection.get().count(), if is_ru { "выбрано" } else { "selected" }, if is_ru { "связей" } else { "relations" })}
                            </span>
                        </div>
                        <div class="flex items-center gap-2">
                            <button
                                type="button"
                                class="h-7 px-2.5 rounded-lg text-xs font-medium text-destructive hover:bg-destructive/10 transition border border-destructive/30 disabled:opacity-50"
                                disabled=move || busy.get()
                                on:click={
                                    let on_remove_selected = on_remove_selected.clone();
                                    move |ev| on_remove_selected(ev)
                                }
                            >
                                {if is_ru { "Удалить выбранные" } else { "Remove selected" }}
                            </button>
                            <button
                                type="button"
                                class="h-7 px-2.5 rounded-lg text-xs text-muted-foreground hover:text-foreground transition border border-border bg-background"
                                on:click=move |_| selection.update(|s| s.clear())
                            >
                                {if is_ru { "Снять выбор" } else { "Clear" }}
                            </button>
                        </div>
                    </div>
                </Show>

                <DataGrid
                    columns=columns.clone()
                    data=Signal::derive(move || paged_relations.get())
                    key_fn=|item: &ProductRelationItem| item.id.clone()
                    cell_renderer=cell_renderer
                    is_loading=Signal::derive(move || busy.get() || relations_resource.get().is_none())
                    empty_message=copy.empty.clone()
                    selection=selection
                    pagination=pagination
                    filters=filters
                    on_filter_change=Callback::new(move |f| filters.set(f))
                    on_row_click=Callback::new(|_| ())
                />
            </div>
        </section>
    }.into_any()
}

#[component]
pub fn ProductRelationsAdmin() -> impl IntoView {
    let (product_id, set_product_id) = signal(String::new());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);

    view! {
        <div class="product-relations-admin p-6 max-w-7xl mx-auto space-y-6">
            <div class="border-b border-gray-200 pb-5 dark:border-gray-800">
                <h1 class="text-2xl font-bold tracking-tight text-gray-900 dark:text-white">
                    "Product Relations"
                </h1>
                <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
                    "Manage cross-sells, up-sells, accessories, and product merchandising associations."
                </p>
            </div>

            {move || error.get().map(|err| view! {
                <div class="rounded-xl border border-red-200 bg-red-50 p-4 text-sm text-red-700 dark:border-red-900/50 dark:bg-red-950/50 dark:text-red-300">
                    {err}
                </div>
            })}

            <div class="bg-white dark:bg-gray-900 p-4 rounded-xl border border-gray-200 dark:border-gray-800">
                <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                    "Product ID (UUID)"
                </label>
                <input
                    type="text"
                    class="w-full max-w-md px-3 py-2 border rounded-lg font-mono text-sm dark:bg-gray-800 dark:border-gray-700 dark:text-white"
                    placeholder="Enter Product UUID to manage relations"
                    prop:value=move || product_id.get()
                    on:input=move |ev| set_product_id.set(event_target_value(&ev))
                />
            </div>

            {move || {
                let pid = product_id.get();
                if pid.trim().is_empty() {
                    view! {
                        <div class="p-8 text-center text-gray-500 dark:text-gray-400 text-sm">
                            "Please specify a Product UUID above to manage its relations."
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <ProductRelationsPanel
                            product_id=pid
                            busy=busy
                            set_busy=set_busy
                            set_error=set_error
                        />
                    }.into_any()
                }
            }}
        </div>
    }
}
