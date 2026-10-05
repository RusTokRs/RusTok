use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_grid::{ColumnFilters, FilterValue, GridPagination, RowSelection};
use rustok_grid_leptos::DataGrid;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    BundleAdminTransportProfile, build_bundle_admin_shell, bundle_grid_columns, filter_bundles,
    selected_transport_profile, validate_bundle_discount, validate_bundle_name,
    validate_bundle_slug,
};
use crate::i18n::{normalize_admin_locale, t};
use crate::model::{
    BundleAdminCommand, BundleAdminCreateDraft, BundleAdminFilters, BundleAdminListItem,
    BundleAdminUpdateDraft,
};
use crate::transport::{
    BundleAdminTransportContext, execute_bundle_command, load_bundle_directory,
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
pub fn ProductBundlesAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = normalize_admin_locale(route_context.locale.as_deref());
    let profile = selected_transport_profile(option_env!("RUSTOK_UI_TRANSPORT_PROFILE"));
    let shell = build_bundle_admin_shell(Some(locale), profile);
    let transport = transport_context(profile);

    let refresh_nonce = RwSignal::new(0_u64);
    let search = RwSignal::new(String::new());
    let status_filter = RwSignal::new(Option::<String>::None);
    let type_filter = RwSignal::new(Option::<String>::None);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(Option::<String>::None);
    let notice = RwSignal::new(Option::<String>::None);

    let show_create_modal = RwSignal::new(false);
    let edit_bundle_id = RwSignal::new(Option::<String>::None);
    let delete_confirm_id = RwSignal::new(Option::<String>::None);

    let draft_slug = RwSignal::new(String::new());
    let draft_name = RwSignal::new(String::new());
    let draft_description = RwSignal::new(String::new());
    let draft_bundle_type = RwSignal::new("fixed".to_string());
    let draft_status = RwSignal::new("active".to_string());
    let draft_discount_type = RwSignal::new("none".to_string());
    let draft_discount_value = RwSignal::new("0".to_string());

    let directory_transport = transport.clone();
    let directory = local_resource(
        move || {
            (
                refresh_nonce.get(),
                search.get(),
                status_filter.get(),
                type_filter.get(),
            )
        },
        move |(_, search_term, status, btype)| {
            let context = directory_transport.clone();
            async move {
                load_bundle_directory(
                    context,
                    BundleAdminFilters {
                        search: if search_term.trim().is_empty() {
                            None
                        } else {
                            Some(search_term.trim().to_string())
                        },
                        status,
                        bundle_type: btype,
                        page: 1,
                        per_page: 100,
                    },
                )
                .await
            }
        },
    );

    let reset_form = move || {
        draft_slug.set(String::new());
        draft_name.set(String::new());
        draft_description.set(String::new());
        draft_bundle_type.set("fixed".to_string());
        draft_status.set("active".to_string());
        draft_discount_type.set("none".to_string());
        draft_discount_value.set("0".to_string());
        show_create_modal.set(false);
        edit_bundle_id.set(None);
        delete_confirm_id.set(None);
    };

    let on_submit_create = {
        let transport = transport.clone();
        move |_| {
            let slug = draft_slug.get();
            let name = draft_name.get();
            let dtype = draft_discount_type.get();
            let dval = draft_discount_value.get();

            if let Err(e) = validate_bundle_name(&name) {
                error.set(Some(e.to_string()));
                return;
            }
            if let Err(e) = validate_bundle_slug(&slug) {
                error.set(Some(e.to_string()));
                return;
            }
            if let Err(e) = validate_bundle_discount(&dtype, &dval) {
                error.set(Some(e.to_string()));
                return;
            }

            busy.set(true);
            error.set(None);
            let transport = transport.clone();
            let draft = BundleAdminCreateDraft {
                slug,
                name,
                description: if draft_description.get().trim().is_empty() {
                    None
                } else {
                    Some(draft_description.get().trim().to_string())
                },
                bundle_type: draft_bundle_type.get(),
                status: draft_status.get(),
                discount_type: dtype,
                discount_value: dval,
            };

            spawn_local(async move {
                let res = execute_bundle_command(
                    transport,
                    uuid::Uuid::new_v4().to_string(),
                    BundleAdminCommand::Create { draft },
                )
                .await;

                busy.set(false);
                match res {
                    Ok(out) if out.is_success() => {
                        reset_form();
                        notice.set(Some(t(
                            Some(locale),
                            "bundle.notice-created",
                            "Bundle created successfully.",
                        )));
                        refresh_nonce.update(|n| *n += 1);
                    }
                    Ok(out) => {
                        error.set(Some(out.error_summary()));
                    }
                    Err(e) => {
                        error.set(Some(e.to_string()));
                    }
                }
            });
        }
    };

    let on_submit_update = {
        let transport = transport.clone();
        move |id: String| {
            let name = draft_name.get();
            let slug = draft_slug.get();
            let dtype = draft_discount_type.get();
            let dval = draft_discount_value.get();

            if let Err(e) = validate_bundle_name(&name) {
                error.set(Some(e.to_string()));
                return;
            }
            if let Err(e) = validate_bundle_slug(&slug) {
                error.set(Some(e.to_string()));
                return;
            }
            if let Err(e) = validate_bundle_discount(&dtype, &dval) {
                error.set(Some(e.to_string()));
                return;
            }

            busy.set(true);
            error.set(None);
            let transport = transport.clone();
            let draft = BundleAdminUpdateDraft {
                slug: Some(slug),
                name: Some(name),
                description: Some(draft_description.get().trim().to_string()),
                bundle_type: Some(draft_bundle_type.get()),
                status: Some(draft_status.get()),
                discount_type: Some(dtype),
                discount_value: Some(dval),
            };

            spawn_local(async move {
                let res = execute_bundle_command(
                    transport,
                    uuid::Uuid::new_v4().to_string(),
                    BundleAdminCommand::Update { id, draft },
                )
                .await;

                busy.set(false);
                match res {
                    Ok(out) if out.is_success() => {
                        reset_form();
                        notice.set(Some(t(
                            Some(locale),
                            "bundle.notice-updated",
                            "Bundle updated successfully.",
                        )));
                        refresh_nonce.update(|n| *n += 1);
                    }
                    Ok(out) => {
                        error.set(Some(out.error_summary()));
                    }
                    Err(e) => {
                        error.set(Some(e.to_string()));
                    }
                }
            });
        }
    };

    let on_delete = {
        let transport = transport.clone();
        move |id: String| {
            busy.set(true);
            error.set(None);
            let transport = transport.clone();

            spawn_local(async move {
                let res = execute_bundle_command(
                    transport,
                    uuid::Uuid::new_v4().to_string(),
                    BundleAdminCommand::Delete { id },
                )
                .await;

                busy.set(false);
                match res {
                    Ok(out) if out.is_success() => {
                        delete_confirm_id.set(None);
                        notice.set(Some(t(
                            Some(locale),
                            "bundle.notice-deleted",
                            "Bundle deleted successfully.",
                        )));
                        refresh_nonce.update(|n| *n += 1);
                    }
                    Ok(out) => {
                        error.set(Some(out.error_summary()));
                    }
                    Err(e) => {
                        error.set(Some(e.to_string()));
                    }
                }
            });
        }
    };

    view! {
        <div class="bundle-admin p-6 max-w-7xl mx-auto space-y-6">
            // Header Section
            <div class="flex flex-col md:flex-row md:items-center md:justify-between gap-4 border-b border-gray-200 pb-5 dark:border-gray-800">
                <div>
                    <div class="inline-flex items-center gap-2 px-2.5 py-0.5 rounded-full text-xs font-medium bg-blue-100 text-blue-800 dark:bg-blue-900/40 dark:text-blue-300 mb-2">
                        {t(Some(locale), "bundle.badge", "Product Bundles")}
                    </div>
                    <h1 class="text-2xl font-bold tracking-tight text-gray-900 dark:text-white">
                        {shell.title}
                    </h1>
                    <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
                        {shell.subtitle}
                    </p>
                </div>
                <div class="flex items-center gap-3">
                    <button
                        type="button"
                        class="inline-flex items-center justify-center px-4 py-2 border border-transparent rounded-lg shadow-sm text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 transition-colors"
                        on:click=move |_| {
                            reset_form();
                            show_create_modal.set(true);
                        }
                    >
                        <svg class="-ml-1 mr-2 h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6v6m0 0v6m0-6h6m-6 0H6"/>
                        </svg>
                        {t(Some(locale), "bundle.create", "New Bundle")}
                    </button>
                </div>
            </div>

            // Notifications
            {move || notice.get().map(|msg| view! {
                <div class="p-4 rounded-lg bg-green-50 border border-green-200 text-green-800 dark:bg-green-900/20 dark:border-green-800/40 dark:text-green-300 text-sm flex items-center justify-between">
                    <span>{msg}</span>
                    <button type="button" class="text-green-600 hover:text-green-800 dark:text-green-400" on:click=move |_| notice.set(None)>
                        "×"
                    </button>
                </div>
            })}

            {move || error.get().map(|msg| view! {
                <div class="p-4 rounded-lg bg-red-50 border border-red-200 text-red-800 dark:bg-red-900/20 dark:border-red-800/40 dark:text-red-300 text-sm flex items-center justify-between">
                    <span>{msg}</span>
                    <button type="button" class="text-red-600 hover:text-red-800 dark:text-red-400" on:click=move |_| error.set(None)>
                        "×"
                    </button>
                </div>
            })}            // Grid and table state
            {
                let is_ru = locale.starts_with("ru");
                let columns = bundle_grid_columns(Some(locale));
                let filters = RwSignal::new(ColumnFilters::new());
                let selection = RwSignal::new(RowSelection::new());
                let pagination = RwSignal::new(GridPagination::new(1, 10, 0));

                let filtered_bundles = Memo::new(move |_| {
                    let raw = directory
                        .get()
                        .and_then(Result::ok)
                        .map(|data| data.items)
                        .unwrap_or_default();
                    let current_filters = filters.get();
                    let search_term = search.get();
                    let list = filter_bundles(
                        &raw,
                        &current_filters,
                        if search_term.trim().is_empty() {
                            None
                        } else {
                            Some(search_term.trim())
                        },
                    );
                    pagination.update(|p| p.total = list.len() as u64);
                    list
                });

                let paged_bundles = Memo::new(move |_| {
                    let list = filtered_bundles.get();
                    let p = pagination.get();
                    let start = (p.page.saturating_sub(1)) * p.page_size;
                    list.into_iter().skip(start).take(p.page_size).collect::<Vec<_>>()
                });

                let on_filters_change = Callback::new(move |new_filters: ColumnFilters| {
                    if let Some(FilterValue::Select(s)) = new_filters.get("status") {
                        status_filter.set(Some(s.clone()));
                    } else {
                        status_filter.set(None);
                    }
                    if let Some(FilterValue::Select(t_val)) = new_filters.get("type") {
                        type_filter.set(Some(t_val.clone()));
                    } else {
                        type_filter.set(None);
                    }
                    filters.set(new_filters);
                });

                let cell_locale = locale;
                let cell_draft_slug = draft_slug;
                let cell_draft_name = draft_name;
                let cell_draft_description = draft_description;
                let cell_draft_bundle_type = draft_bundle_type;
                let cell_draft_status = draft_status;
                let cell_draft_discount_type = draft_discount_type;
                let cell_draft_discount_value = draft_discount_value;
                let cell_edit_bundle_id = edit_bundle_id;
                let cell_delete_confirm_id = delete_confirm_id;

                let cell_renderer = Callback::new(move |(item, col_id): (BundleAdminListItem, String)| {
                    match col_id.as_str() {
                        "name" => {
                            let name = item.name.clone();
                            let desc = item.description.clone();
                            view! {
                                <div class="flex flex-col py-1">
                                    <span class="font-medium text-foreground">{name}</span>
                                    {desc.map(|d| view! {
                                        <span class="text-xs text-muted-foreground truncate max-w-xs">{d}</span>
                                    })}
                                </div>
                            }
                            .into_any()
                        }
                        "slug" => {
                            let slug = item.slug.clone();
                            view! {
                                <code class="text-xs font-mono px-1.5 py-0.5 rounded bg-muted text-muted-foreground">
                                    {slug}
                                </code>
                            }
                            .into_any()
                        }
                        "type" => {
                            let b_type = item.bundle_type.clone();
                            view! {
                                <span class="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-muted text-foreground">
                                    {b_type}
                                </span>
                            }
                            .into_any()
                        }
                        "discount" => {
                            let text = if item.discount_type == "none" {
                                "-".to_string()
                            } else {
                                format!("{} ({})", item.discount_value, item.discount_type)
                            };
                            view! {
                                <span class="text-xs text-muted-foreground font-mono">{text}</span>
                            }
                            .into_any()
                        }
                        "items" => {
                            let count = item.items_count;
                            view! {
                                <span class="text-sm text-foreground font-mono">{count}</span>
                            }
                            .into_any()
                        }
                        "status" => {
                            let is_active = item.status == "active";
                            let status_label = item.status.clone();
                            view! {
                                <span class=format!(
                                    "inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium {}",
                                    if is_active {
                                        "bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400"
                                    } else {
                                        "bg-gray-100 text-gray-800 dark:bg-gray-800 dark:text-gray-400"
                                    }
                                )>
                                    {status_label}
                                </span>
                            }
                            .into_any()
                        }
                        "actions" => {
                            let item_clone = item.clone();
                            let id_for_del = item.id.clone();
                            let edit_label = t(Some(cell_locale), "bundle.action-edit", "Edit");
                            let del_label = t(Some(cell_locale), "bundle.delete", "Delete");
                            view! {
                                <div class="flex items-center justify-end gap-2 py-1">
                                    <button
                                        type="button"
                                        class="text-indigo-600 hover:text-indigo-900 dark:text-indigo-400 dark:hover:text-indigo-300 text-xs font-medium px-2 py-1 rounded hover:bg-indigo-50 dark:hover:bg-indigo-950/50"
                                        on:click=move |_| {
                                            cell_draft_slug.set(item_clone.slug.clone());
                                            cell_draft_name.set(item_clone.name.clone());
                                            cell_draft_description.set(item_clone.description.clone().unwrap_or_default());
                                            cell_draft_bundle_type.set(item_clone.bundle_type.clone());
                                            cell_draft_status.set(item_clone.status.clone());
                                            cell_draft_discount_type.set(item_clone.discount_type.clone());
                                            cell_draft_discount_value.set(item_clone.discount_value.clone());
                                            cell_edit_bundle_id.set(Some(item_clone.id.clone()));
                                        }
                                    >
                                        {edit_label}
                                    </button>
                                    <button
                                        type="button"
                                        class="text-red-600 hover:text-red-900 dark:text-red-400 dark:hover:text-red-300 text-xs font-medium px-2 py-1 rounded hover:bg-rose-50 dark:hover:bg-rose-950/50"
                                        on:click=move |_| cell_delete_confirm_id.set(Some(id_for_del.clone()))
                                    >
                                        {del_label}
                                    </button>
                                </div>
                            }
                            .into_any()
                        }
                        _ => ().into_any(),
                    }
                });

                view! {
                    <div class="space-y-4">
                        // Search input
                        <div class="flex flex-col sm:flex-row gap-4">
                            <div class="flex-1">
                                <input
                                    type="text"
                                    placeholder=t(Some(locale), "bundle.filter-searchPlaceholder", "Search by name or slug...")
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
                                        {move || format!("{} {} {}", selection.get().count(), if is_ru { "выбрано" } else { "selected" }, if is_ru { "комплектов" } else { "bundles" })}
                                    </span>
                                </div>
                                <button
                                    type="button"
                                    class="h-6 px-2.5 rounded-lg text-xs text-muted-foreground hover:text-foreground transition border border-border bg-background"
                                    on:click=move |_| selection.update(|s| s.clear())
                                >
                                    {if is_ru { "Снять выбор" } else { "Clear" }}
                                </button>
                            </div>
                        </Show>

                        // Directory Table with DataGrid
                        <div class="bg-card rounded-xl border border-border overflow-hidden shadow-sm p-4">
                            <Suspense fallback=move || view! {
                                <div class="p-8 text-center text-muted-foreground text-sm">
                                    {t(Some(locale), "bundle.loadingList", "Loading bundles...")}
                                </div>
                            }>
                                {move || match directory.get() {
                                    None => view! {
                                        <div class="p-8 text-center text-muted-foreground text-sm">
                                            {t(Some(locale), "bundle.loadingList", "Loading bundles...")}
                                        </div>
                                    }.into_any(),
                                    Some(Err(e)) => view! {
                                        <div class="p-8 text-center text-destructive text-sm">
                                            {format!("Error: {e}")}
                                        </div>
                                    }.into_any(),
                                    Some(Ok(_)) => {
                                        let empty_msg = t(Some(locale), "bundle.empty", "No bundles found").to_string();
                                        view! {
                                            <DataGrid
                                                columns=columns.clone()
                                                data=Signal::derive(move || paged_bundles.get())
                                                key_fn=|item: &BundleAdminListItem| item.id.clone()
                                                cell_renderer=cell_renderer
                                                is_loading=Signal::derive(move || busy.get() || directory.get().is_none())
                                                empty_message=empty_msg
                                                selection=selection
                                                pagination=pagination
                                                filters=filters
                                                on_filter_change=on_filters_change
                                                on_row_click=Callback::new(|_| ())
                                            />
                                        }.into_any()
                                    }
                                }}
                            </Suspense>
                        </div>
                    </div>
                }
            }

            // Create Modal
            {
                let on_create = on_submit_create.clone();
                move || show_create_modal.get().then(|| {
                    let on_create = on_create.clone();
                    view! {
                <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/50 backdrop-blur-sm">
                    <div class="bg-white dark:bg-gray-900 rounded-2xl shadow-xl max-w-lg w-full p-6 space-y-4 border border-gray-200 dark:border-gray-800">
                        <div class="flex items-center justify-between">
                            <h2 class="text-lg font-bold text-gray-900 dark:text-white">
                                {t(Some(locale), "bundle.action-createTitle", "Create Bundle")}
                            </h2>
                            <button type="button" class="text-gray-400 hover:text-gray-600" on:click=move |_| show_create_modal.set(false)>
                                "×"
                            </button>
                        </div>

                        <div class="space-y-3 text-sm">
                            <div>
                                <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                    {t(Some(locale), "bundle.form-nameRequired", "Name *")}
                                </label>
                                <input
                                    type="text"
                                    class="w-full px-3 py-2 border rounded-lg dark:bg-gray-800 dark:border-gray-700"
                                    prop:value=move || draft_name.get()
                                    on:input=move |ev| draft_name.set(event_target_value(&ev))
                                />
                            </div>

                            <div>
                                <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                    {t(Some(locale), "bundle.form-slugRequired", "Slug *")}
                                </label>
                                <input
                                    type="text"
                                    class="w-full px-3 py-2 border rounded-lg dark:bg-gray-800 dark:border-gray-700 font-mono text-xs"
                                    prop:value=move || draft_slug.get()
                                    on:input=move |ev| draft_slug.set(event_target_value(&ev))
                                />
                            </div>

                            <div class="grid grid-cols-2 gap-3">
                                <div>
                                    <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                        {t(Some(locale), "bundle.type", "Type")}
                                    </label>
                                    <select
                                        class="w-full px-3 py-2 border rounded-lg dark:bg-gray-800 dark:border-gray-700 text-xs"
                                        on:change=move |ev| draft_bundle_type.set(event_target_value(&ev))
                                    >
                                        <option value="fixed" selected=move || draft_bundle_type.get() == "fixed">
                                            {t(Some(locale), "bundle.typeFixed", "Fixed")}
                                        </option>
                                        <option value="flexible" selected=move || draft_bundle_type.get() == "flexible">
                                            {t(Some(locale), "bundle.typeFlexible", "Flexible")}
                                        </option>
                                    </select>
                                </div>

                                <div>
                                    <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                        {t(Some(locale), "bundle.status", "Status")}
                                    </label>
                                    <select
                                        class="w-full px-3 py-2 border rounded-lg dark:bg-gray-800 dark:border-gray-700 text-xs"
                                        on:change=move |ev| draft_status.set(event_target_value(&ev))
                                    >
                                        <option value="active" selected=move || draft_status.get() == "active">
                                            {t(Some(locale), "bundle.statusActive", "Active")}
                                        </option>
                                        <option value="draft" selected=move || draft_status.get() == "draft">
                                            {t(Some(locale), "bundle.statusDraft", "Draft")}
                                        </option>
                                        <option value="archived" selected=move || draft_status.get() == "archived">
                                            {t(Some(locale), "bundle.statusArchived", "Archived")}
                                        </option>
                                    </select>
                                </div>
                            </div>

                            <div class="grid grid-cols-2 gap-3">
                                <div>
                                    <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                        {t(Some(locale), "bundle.discountType", "Discount Type")}
                                    </label>
                                    <select
                                        class="w-full px-3 py-2 border rounded-lg dark:bg-gray-800 dark:border-gray-700 text-xs"
                                        on:change=move |ev| draft_discount_type.set(event_target_value(&ev))
                                    >
                                        <option value="none" selected=move || draft_discount_type.get() == "none">
                                            {t(Some(locale), "bundle.discountTypeNone", "None")}
                                        </option>
                                        <option value="percentage" selected=move || draft_discount_type.get() == "percentage">
                                            {t(Some(locale), "bundle.discountTypePercentage", "Percentage (%)")}
                                        </option>
                                        <option value="fixed_amount" selected=move || draft_discount_type.get() == "fixed_amount">
                                            {t(Some(locale), "bundle.discountTypeFixedAmount", "Fixed Amount")}
                                        </option>
                                    </select>
                                </div>

                                <div>
                                    <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                        {t(Some(locale), "bundle.discountValue", "Discount Value")}
                                    </label>
                                    <input
                                        type="text"
                                        class="w-full px-3 py-2 border rounded-lg dark:bg-gray-800 dark:border-gray-700 text-xs"
                                        prop:value=move || draft_discount_value.get()
                                        on:input=move |ev| draft_discount_value.set(event_target_value(&ev))
                                    />
                                </div>
                            </div>

                            <div>
                                <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                    {t(Some(locale), "bundle.description", "Description")}
                                </label>
                                <textarea
                                    class="w-full px-3 py-2 border rounded-lg dark:bg-gray-800 dark:border-gray-700"
                                    rows="2"
                                    prop:value=move || draft_description.get()
                                    on:input=move |ev| draft_description.set(event_target_value(&ev))
                                />
                            </div>
                        </div>

                        <div class="flex justify-end gap-3 pt-3 border-t dark:border-gray-800">
                            <button
                                type="button"
                                class="px-4 py-2 text-sm border rounded-lg dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800"
                                on:click=move |_| show_create_modal.set(false)
                            >
                                {t(Some(locale), "bundle.cancel", "Cancel")}
                            </button>
                            <button
                                type="button"
                                class="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-lg hover:bg-blue-700"
                                on:click=on_create
                            >
                                {t(Some(locale), "bundle.action-create", "Create")}
                            </button>
                        </div>
                    </div>
                </div>
                    }
                })
            }

            // Edit Modal
            {move || edit_bundle_id.get().map(|id| {
                let id_clone = id.clone();
                let on_save = on_submit_update.clone();

                view! {
                    <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/50 backdrop-blur-sm">
                        <div class="bg-white dark:bg-gray-900 rounded-2xl shadow-xl max-w-lg w-full p-6 space-y-4 border border-gray-200 dark:border-gray-800">
                            <div class="flex items-center justify-between">
                                <h2 class="text-lg font-bold text-gray-900 dark:text-white">
                                    {t(Some(locale), "bundle.edit", "Edit Bundle")}
                                </h2>
                                <button type="button" class="text-gray-400 hover:text-gray-600" on:click=move |_| edit_bundle_id.set(None)>
                                    "×"
                                </button>
                            </div>

                            <div class="space-y-3 text-sm">
                                <div>
                                    <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                        {t(Some(locale), "bundle.form-nameRequired", "Name *")}
                                    </label>
                                    <input
                                        type="text"
                                        class="w-full px-3 py-2 border rounded-lg dark:bg-gray-800 dark:border-gray-700"
                                        prop:value=move || draft_name.get()
                                        on:input=move |ev| draft_name.set(event_target_value(&ev))
                                    />
                                </div>

                                <div>
                                    <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                        {t(Some(locale), "bundle.form-slugRequired", "Slug *")}
                                    </label>
                                    <input
                                        type="text"
                                        class="w-full px-3 py-2 border rounded-lg dark:bg-gray-800 dark:border-gray-700 font-mono text-xs"
                                        prop:value=move || draft_slug.get()
                                        on:input=move |ev| draft_slug.set(event_target_value(&ev))
                                    />
                                </div>

                                <div class="grid grid-cols-2 gap-3">
                                    <div>
                                        <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                            {t(Some(locale), "bundle.type", "Type")}
                                        </label>
                                        <select
                                            class="w-full px-3 py-2 border rounded-lg dark:bg-gray-800 dark:border-gray-700 text-xs"
                                            on:change=move |ev| draft_bundle_type.set(event_target_value(&ev))
                                        >
                                            <option value="fixed" selected=move || draft_bundle_type.get() == "fixed">
                                                {t(Some(locale), "bundle.typeFixed", "Fixed")}
                                            </option>
                                            <option value="flexible" selected=move || draft_bundle_type.get() == "flexible">
                                                {t(Some(locale), "bundle.typeFlexible", "Flexible")}
                                            </option>
                                        </select>
                                    </div>

                                    <div>
                                        <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                            {t(Some(locale), "bundle.status", "Status")}
                                        </label>
                                        <select
                                            class="w-full px-3 py-2 border rounded-lg dark:bg-gray-800 dark:border-gray-700 text-xs"
                                            on:change=move |ev| draft_status.set(event_target_value(&ev))
                                        >
                                            <option value="active" selected=move || draft_status.get() == "active">
                                                {t(Some(locale), "bundle.statusActive", "Active")}
                                            </option>
                                            <option value="draft" selected=move || draft_status.get() == "draft">
                                                {t(Some(locale), "bundle.statusDraft", "Draft")}
                                            </option>
                                            <option value="archived" selected=move || draft_status.get() == "archived">
                                                {t(Some(locale), "bundle.statusArchived", "Archived")}
                                            </option>
                                        </select>
                                    </div>
                                </div>

                                <div class="grid grid-cols-2 gap-3">
                                    <div>
                                        <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                            {t(Some(locale), "bundle.discountType", "Discount Type")}
                                        </label>
                                        <select
                                            class="w-full px-3 py-2 border rounded-lg dark:bg-gray-800 dark:border-gray-700 text-xs"
                                            on:change=move |ev| draft_discount_type.set(event_target_value(&ev))
                                        >
                                            <option value="none" selected=move || draft_discount_type.get() == "none">
                                                {t(Some(locale), "bundle.discountTypeNone", "None")}
                                            </option>
                                            <option value="percentage" selected=move || draft_discount_type.get() == "percentage">
                                                {t(Some(locale), "bundle.discountTypePercentage", "Percentage (%)")}
                                            </option>
                                            <option value="fixed_amount" selected=move || draft_discount_type.get() == "fixed_amount">
                                                {t(Some(locale), "bundle.discountTypeFixedAmount", "Fixed Amount")}
                                            </option>
                                        </select>
                                    </div>

                                    <div>
                                        <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                            {t(Some(locale), "bundle.discountValue", "Discount Value")}
                                        </label>
                                        <input
                                            type="text"
                                            class="w-full px-3 py-2 border rounded-lg dark:bg-gray-800 dark:border-gray-700 text-xs"
                                            prop:value=move || draft_discount_value.get()
                                            on:input=move |ev| draft_discount_value.set(event_target_value(&ev))
                                        />
                                    </div>
                                </div>

                                <div>
                                    <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                        {t(Some(locale), "bundle.description", "Description")}
                                    </label>
                                    <textarea
                                        class="w-full px-3 py-2 border rounded-lg dark:bg-gray-800 dark:border-gray-700"
                                        rows="2"
                                        prop:value=move || draft_description.get()
                                        on:input=move |ev| draft_description.set(event_target_value(&ev))
                                    />
                                </div>
                            </div>

                            <div class="flex justify-end gap-3 pt-3 border-t dark:border-gray-800">
                                <button
                                    type="button"
                                    class="px-4 py-2 text-sm border rounded-lg dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800"
                                    on:click=move |_| edit_bundle_id.set(None)
                                >
                                    {t(Some(locale), "bundle.cancel", "Cancel")}
                                </button>
                                <button
                                    type="button"
                                    class="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-lg hover:bg-blue-700"
                                    on:click={
                                        let on_save = on_save.clone();
                                        let id_clone = id_clone.clone();
                                        move |_| on_save(id_clone.clone())
                                    }
                                >
                                    {t(Some(locale), "bundle.save", "Save")}
                                </button>
                            </div>
                        </div>
                    </div>
                }
            })}

            // Delete Confirm Modal
            {move || delete_confirm_id.get().map(|id| {
                let id_clone = id.clone();
                let on_del = on_delete.clone();

                view! {
                    <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/50 backdrop-blur-sm">
                        <div class="bg-white dark:bg-gray-900 rounded-2xl shadow-xl max-w-sm w-full p-6 space-y-4 border border-gray-200 dark:border-gray-800">
                            <h3 class="text-base font-bold text-gray-900 dark:text-white">
                                {t(Some(locale), "bundle.delete-title", "Delete Bundle")}
                            </h3>
                            <p class="text-sm text-gray-500 dark:text-gray-400">
                                {t(Some(locale), "bundle.delete-confirmBody", "Are you sure you want to delete this bundle? This action cannot be undone.")}
                            </p>
                            <div class="flex justify-end gap-3 pt-2">
                                <button
                                    type="button"
                                    class="px-4 py-2 text-sm border rounded-lg dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800"
                                    on:click=move |_| delete_confirm_id.set(None)
                                >
                                    {t(Some(locale), "bundle.cancel", "Cancel")}
                                </button>
                                <button
                                    type="button"
                                    class="px-4 py-2 text-sm font-medium text-white bg-red-600 rounded-lg hover:bg-red-700"
                                    on:click={
                                        let on_del = on_del.clone();
                                        let id_clone = id_clone.clone();
                                        move |_| on_del(id_clone.clone())
                                    }
                                >
                                    {t(Some(locale), "bundle.delete", "Delete")}
                                </button>
                            </div>
                        </div>
                    </div>
                }
            })}
        </div>
    }
}

fn transport_context(profile: BundleAdminTransportProfile) -> BundleAdminTransportContext {
    match profile {
        BundleAdminTransportProfile::Native => BundleAdminTransportContext::native(),
        BundleAdminTransportProfile::Graphql => BundleAdminTransportContext::graphql(None, None),
    }
}
