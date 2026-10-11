use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_grid::{ColumnFilters, GridPagination, RowSelection};
use rustok_grid_leptos::DataGrid;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    BrandAdminTransportProfile, brand_grid_columns, build_brand_admin_shell, filter_brands,
    selected_transport_profile, validate_brand_name, validate_brand_slug,
};
use crate::i18n::{normalize_admin_locale, t};
use crate::model::{
    BrandAdminCommand, BrandAdminCreateDraft, BrandAdminFilters, BrandAdminListItem,
    BrandAdminUpdateDraft,
};
use crate::transport::{BrandAdminTransportContext, execute_brand_command, load_brand_directory};

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
pub fn BrandAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = normalize_admin_locale(route_context.locale.as_deref());
    let profile = selected_transport_profile(option_env!("RUSTOK_UI_TRANSPORT_PROFILE"));
    let shell = build_brand_admin_shell(Some(locale), profile);
    let transport = transport_context(profile);

    let refresh_nonce = RwSignal::new(0_u64);
    let search = RwSignal::new(String::new());
    let is_active_filter = RwSignal::new(Option::<bool>::None);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(Option::<String>::None);
    let notice = RwSignal::new(Option::<String>::None);

    let show_create_modal = RwSignal::new(false);
    let edit_brand_id = RwSignal::new(Option::<String>::None);

    let draft_slug = RwSignal::new(String::new());
    let draft_name = RwSignal::new(String::new());
    let draft_description = RwSignal::new(String::new());
    let draft_website_url = RwSignal::new(String::new());
    let draft_logo_url = RwSignal::new(String::new());
    let draft_is_active = RwSignal::new(true);
    let draft_sort_order = RwSignal::new(0_i32);

    let directory_transport = transport.clone();
    let directory = local_resource(
        move || (refresh_nonce.get(), search.get(), is_active_filter.get()),
        move |(_, search_term, active)| {
            let context = directory_transport.clone();
            async move {
                load_brand_directory(
                    context,
                    BrandAdminFilters {
                        search: if search_term.trim().is_empty() {
                            None
                        } else {
                            Some(search_term.trim().to_string())
                        },
                        is_active: active,
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
        draft_website_url.set(String::new());
        draft_logo_url.set(String::new());
        draft_is_active.set(true);
        draft_sort_order.set(0);
        show_create_modal.set(false);
        edit_brand_id.set(None);
    };

    let on_submit_create = {
        let transport = transport.clone();
        move |_| {
            let slug = draft_slug.get();
            let name = draft_name.get();

            if let Err(e) = validate_brand_name(&name) {
                error.set(Some(e.to_string()));
                return;
            }
            if let Err(e) = validate_brand_slug(&slug) {
                error.set(Some(e.to_string()));
                return;
            }

            busy.set(true);
            error.set(None);
            let transport = transport.clone();
            let draft = BrandAdminCreateDraft {
                slug: slug.trim().to_string(),
                name: name.trim().to_string(),
                description: optional_text(draft_description.get()),
                website_url: optional_text(draft_website_url.get()),
                logo_url: optional_text(draft_logo_url.get()),
                is_active: draft_is_active.get(),
                sort_order: draft_sort_order.get(),
            };

            spawn_local(async move {
                let idempotency_key = format!("brand-create-{}", uuid::Uuid::new_v4());
                let result = execute_brand_command(
                    transport,
                    idempotency_key,
                    BrandAdminCommand::Create { draft },
                )
                .await;

                busy.set(false);
                match result {
                    Ok(_) => {
                        notice.set(Some(t(
                            Some(locale),
                            "brand.notice-created",
                            "Brand created successfully",
                        )));
                        reset_form();
                        refresh_nonce.update(|n| *n += 1);
                    }
                    Err(e) => {
                        error.set(Some(format!("{e}")));
                    }
                }
            });
        }
    };

    let on_submit_update = {
        let transport = transport.clone();
        move |id: String| {
            let slug = draft_slug.get();
            let name = draft_name.get();

            if let Err(e) = validate_brand_name(&name) {
                error.set(Some(e.to_string()));
                return;
            }
            if let Err(e) = validate_brand_slug(&slug) {
                error.set(Some(e.to_string()));
                return;
            }

            busy.set(true);
            error.set(None);
            let transport = transport.clone();
            let draft = BrandAdminUpdateDraft {
                slug: Some(slug.trim().to_string()),
                name: Some(name.trim().to_string()),
                description: optional_text(draft_description.get()),
                website_url: optional_text(draft_website_url.get()),
                logo_url: optional_text(draft_logo_url.get()),
                is_active: Some(draft_is_active.get()),
                sort_order: Some(draft_sort_order.get()),
            };

            spawn_local(async move {
                let idempotency_key = format!("brand-update-{}", uuid::Uuid::new_v4());
                let result = execute_brand_command(
                    transport,
                    idempotency_key,
                    BrandAdminCommand::Update { id, draft },
                )
                .await;

                busy.set(false);
                match result {
                    Ok(_) => {
                        notice.set(Some(t(
                            Some(locale),
                            "brand.notice-updated",
                            "Brand updated successfully",
                        )));
                        reset_form();
                        refresh_nonce.update(|n| *n += 1);
                    }
                    Err(e) => {
                        error.set(Some(format!("{e}")));
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
                let idempotency_key = format!("brand-delete-{}", uuid::Uuid::new_v4());
                let result = execute_brand_command(
                    transport,
                    idempotency_key,
                    BrandAdminCommand::Delete { id },
                )
                .await;

                busy.set(false);
                match result {
                    Ok(_) => {
                        notice.set(Some(t(
                            Some(locale),
                            "brand.notice-deleted",
                            "Brand deleted successfully",
                        )));
                        refresh_nonce.update(|n| *n += 1);
                    }
                    Err(e) => {
                        error.set(Some(format!("{e}")));
                    }
                }
            });
        }
    };

    let edit_brand = Callback::new(move |b: BrandAdminListItem| {
        draft_slug.set(b.slug.clone());
        draft_name.set(b.name.clone());
        draft_description.set(b.description.clone().unwrap_or_default());
        draft_website_url.set(b.website_url.clone().unwrap_or_default());
        draft_logo_url.set(b.logo_url.clone().unwrap_or_default());
        draft_is_active.set(b.is_active);
        draft_sort_order.set(b.sort_order);
        edit_brand_id.set(Some(b.id.clone()));
    });

    let is_ru = locale.starts_with("ru");
    let columns = brand_grid_columns(Some(locale));
    let filters = RwSignal::new(ColumnFilters::new());
    let selection = RwSignal::new(RowSelection::new());
    let pagination = RwSignal::new(GridPagination::new(1, 10, 0));

    let filtered_brands = Memo::new(move |_| {
        let raw = directory
            .get()
            .and_then(Result::ok)
            .map(|data| data.items)
            .unwrap_or_default();
        let current_filters = filters.get();
        let search_term = search.get();
        let list = filter_brands(
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

    let on_filters_change = Callback::new(move |new_filters: ColumnFilters| {
        filters.set(new_filters);
    });

    let on_row_click = Callback::new(move |item: BrandAdminListItem| {
        edit_brand.run(item);
    });

    let cell_locale = locale;
    let cell_edit_brand = edit_brand;
    let cell_on_delete = on_delete.clone();

    let cell_renderer = Callback::new(move |(item, col_id): (BrandAdminListItem, String)| {
        match col_id.as_str() {
            "name" => {
                let logo = item.logo_url.clone();
                let name = item.name.clone();
                let desc = item.description.clone();
                let initial = name.chars().next().unwrap_or('B').to_string();
                view! {
                    <div class="flex items-center gap-3 py-1 min-w-0">
                        {if let Some(logo) = logo {
                            view! {
                                <img src=logo alt=name.clone() class="w-8 h-8 rounded object-contain bg-gray-100 dark:bg-gray-700 p-0.5 shrink-0" />
                            }.into_any()
                        } else {
                            view! {
                                <div class="w-8 h-8 rounded bg-indigo-50 dark:bg-indigo-900/30 text-indigo-600 dark:text-indigo-400 flex items-center justify-center font-bold text-xs uppercase shrink-0">
                                    {initial}
                                </div>
                            }.into_any()
                        }}
                        <div class="flex flex-col min-w-0">
                            <span class="font-medium text-foreground truncate">{name}</span>
                            {desc.map(|d| view! {
                                <span class="text-xs text-muted-foreground truncate">{d}</span>
                            })}
                        </div>
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
            "website" => {
                if let Some(url) = item.website_url.clone() {
                    let display_url = url.clone();
                    view! {
                        <a
                            href=url
                            target="_blank"
                            rel="noopener noreferrer"
                            class="text-xs text-indigo-600 dark:text-indigo-400 hover:underline truncate inline-block max-w-[160px]"
                            on:click=move |ev| ev.stop_propagation()
                        >
                            {display_url}
                        </a>
                    }
                    .into_any()
                } else {
                    view! { <span class="text-muted-foreground">-</span> }.into_any()
                }
            }
            "products_count" => {
                let count = item.products_count;
                view! {
                    <span class="inline-flex px-2 py-0.5 rounded-full text-xs font-semibold bg-muted text-foreground">
                        {count.to_string()}
                    </span>
                }
                .into_any()
            }
            "status" => {
                let is_active = item.is_active;
                let active_label = t(Some(cell_locale), "brand.active", "Active");
                let inactive_label = t(Some(cell_locale), "brand.inactive", "Inactive");
                view! {
                    <span class=if is_active {
                        "inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-green-100 text-green-800 dark:bg-green-900/40 dark:text-green-300"
                    } else {
                        "inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-300"
                    }>
                        {if is_active { active_label } else { inactive_label }}
                    </span>
                }
                .into_any()
            }
            "updated_at" => {
                let updated = item
                    .updated_at
                    .split('T')
                    .next()
                    .unwrap_or(item.updated_at.as_str())
                    .to_string();
                view! {
                    <span class="text-xs text-muted-foreground font-mono">
                        {updated}
                    </span>
                }
                .into_any()
            }
            "actions" => {
                let brand_for_edit = item.clone();
                let brand_id = item.id.clone();
                let on_delete_brand = cell_on_delete.clone();
                let on_edit = cell_edit_brand;
                let edit_label = t(Some(cell_locale), "brand.action-edit", "Edit");
                let delete_label = t(Some(cell_locale), "brand.delete", "Delete");
                view! {
                    <div class="flex items-center justify-end gap-2">
                        <button
                            type="button"
                            class="text-indigo-600 hover:text-indigo-900 dark:text-indigo-400 dark:hover:text-indigo-300 text-xs font-medium px-2 py-1 rounded hover:bg-indigo-50 dark:hover:bg-indigo-950/50"
                            on:click=move |ev| {
                                ev.stop_propagation();
                                on_edit.run(brand_for_edit.clone());
                            }
                        >
                            {edit_label}
                        </button>
                        <button
                            type="button"
                            class="text-red-600 hover:text-red-900 dark:text-red-400 dark:hover:text-red-300 text-xs font-medium px-2 py-1 rounded hover:bg-rose-50 dark:hover:bg-rose-950/50"
                            on:click={
                                let id = brand_id.clone();
                                let on_delete_brand = on_delete_brand.clone();
                                move |ev| {
                                    ev.stop_propagation();
                                    on_delete_brand(id.clone());
                                }
                            }
                        >
                            {delete_label}
                        </button>
                    </div>
                }
                .into_any()
            }
            _ => ().into_any(),
        }
    });

    view! {
        <div class="brand-admin-container p-6 space-y-6">
            // Header
            <div class="flex flex-col md:flex-row md:items-center md:justify-between gap-4 border-b pb-4">
                <div>
                    <span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-indigo-100 text-indigo-800 dark:bg-indigo-900/40 dark:text-indigo-300 mb-1">
                        {t(Some(locale), "brand.badge", "Brand Catalog")}
                    </span>
                    <h1 class="text-2xl font-bold tracking-tight text-gray-900 dark:text-gray-100">
                        {shell.title}
                    </h1>
                    <p class="text-sm text-gray-500 dark:text-gray-400">
                        {shell.subtitle}
                    </p>
                </div>
                <div class="flex items-center gap-3">
                    <button
                        type="button"
                        class="inline-flex items-center px-4 py-2 border border-transparent rounded-md shadow-sm text-sm font-medium text-white bg-indigo-600 hover:bg-indigo-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-indigo-500"
                        on:click=move |_| {
                            reset_form();
                            show_create_modal.set(true);
                        }
                    >
                        {t(Some(locale), "brand.action-create", "+ New Brand")}
                    </button>
                </div>
            </div>

            // Notifications / Error Banners
            {move || error.get().map(|err| {
                view! {
                    <div class="rounded-md bg-red-50 p-4 border border-red-200 dark:bg-red-950/50 dark:border-red-800">
                        <div class="flex justify-between">
                            <p class="text-sm text-red-800 dark:text-red-200">{err}</p>
                            <button
                                type="button"
                                class="text-red-600 hover:text-red-800 dark:text-red-400 font-bold"
                                on:click=move |_| error.set(None)
                            >
                                "×"
                            </button>
                        </div>
                    </div>
                }
            })}

            {move || notice.get().map(|msg| {
                view! {
                    <div class="rounded-md bg-green-50 p-4 border border-green-200 dark:bg-green-950/50 dark:border-green-800">
                        <div class="flex justify-between">
                            <p class="text-sm text-green-800 dark:text-green-200">{msg}</p>
                            <button
                                type="button"
                                class="text-green-600 hover:text-green-800 dark:text-green-400 font-bold"
                                on:click=move |_| notice.set(None)
                            >
                                "×"
                            </button>
                        </div>
                    </div>
                }
            })}

            // Filters & Search Bar
            <div class="flex flex-col sm:flex-row gap-4">
                <div class="flex-1">
                    <input
                        type="text"
                        placeholder=t(Some(locale), "brand.filter-searchPlaceholder", "Search by name or slug...")
                        class="w-full px-3 py-2 border rounded-md shadow-sm focus:ring-indigo-500 focus:border-indigo-500 text-sm bg-white dark:bg-gray-800 dark:border-gray-700"
                        prop:value=move || search.get()
                        on:input=move |ev| search.set(event_target_value(&ev))
                    />
                </div>
                <div class="flex gap-2">
                    <button
                        type="button"
                        class=move || {
                            let active = is_active_filter.get().is_none();
                            if active {
                                "px-3 py-2 text-sm font-medium rounded-md bg-gray-200 dark:bg-gray-700 text-gray-900 dark:text-white"
                            } else {
                                "px-3 py-2 text-sm font-medium rounded-md border text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800"
                            }
                        }
                        on:click=move |_| is_active_filter.set(None)
                    >
                        {t(Some(locale), "brand.filter-all", "All")}
                    </button>
                    <button
                        type="button"
                        class=move || {
                            let active = is_active_filter.get() == Some(true);
                            if active {
                                "px-3 py-2 text-sm font-medium rounded-md bg-green-100 text-green-800 dark:bg-green-900/40 dark:text-green-300 font-semibold"
                            } else {
                                "px-3 py-2 text-sm font-medium rounded-md border text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800"
                            }
                        }
                        on:click=move |_| is_active_filter.set(Some(true))
                    >
                        {t(Some(locale), "brand.active", "Active")}
                    </button>
                    <button
                        type="button"
                        class=move || {
                            let active = is_active_filter.get() == Some(false);
                            if active {
                                "px-3 py-2 text-sm font-medium rounded-md bg-gray-200 text-gray-800 dark:bg-gray-700 dark:text-gray-300 font-semibold"
                            } else {
                                "px-3 py-2 text-sm font-medium rounded-md border text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800"
                            }
                        }
                        on:click=move |_| is_active_filter.set(Some(false))
                    >
                        {t(Some(locale), "brand.inactive", "Inactive")}
                    </button>
                </div>
            </div>

            // Create / Edit Modal or Inline Form
            {move || {
                let is_editing = edit_brand_id.get().is_some();
                let is_open = show_create_modal.get() || is_editing;

                if !is_open {
                    return None;
                }

                let current_id = edit_brand_id.get();
                let title = if is_editing {
                    t(Some(locale), "brand.edit", "Edit Brand")
                } else {
                    t(Some(locale), "brand.action.createTitle", "Create New Brand")
                };

                let on_save = {
                    let on_submit_create = on_submit_create.clone();
                    let on_submit_update = on_submit_update.clone();
                    move |_| {
                        if let Some(id) = current_id.clone() {
                            on_submit_update(id);
                        } else {
                            on_submit_create(());
                        }
                    }
                };

                Some(view! {
                    <div class="border rounded-lg p-5 bg-gray-50 dark:bg-gray-900 border-indigo-200 dark:border-indigo-900/50 space-y-4 shadow-sm">
                        <div class="flex justify-between items-center border-b pb-2">
                            <h3 class="font-semibold text-lg text-gray-900 dark:text-gray-100">{title}</h3>
                            <button
                                type="button"
                                class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 text-xl font-bold"
                                on:click=move |_| reset_form()
                            >
                                "×"
                            </button>
                        </div>
                        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                            <div>
                                <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                    {t(Some(locale), "brand.form-nameRequired", "Name *")}
                                </label>
                                <input
                                    type="text"
                                    class="w-full px-3 py-2 border rounded-md text-sm bg-white dark:bg-gray-800 dark:border-gray-700"
                                    placeholder="Sony, Apple, Nike..."
                                    prop:value=move || draft_name.get()
                                    on:input=move |ev| {
                                        let val = event_target_value(&ev);
                                        draft_name.set(val.clone());
                                        if edit_brand_id.get().is_none() && draft_slug.get().is_empty() {
                                            draft_slug.set(val.to_lowercase().replace(' ', "-"));
                                        }
                                    }
                                />
                            </div>
                            <div>
                                <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                    {t(Some(locale), "brand.form-slugRequired", "Slug *")}
                                </label>
                                <input
                                    type="text"
                                    class="w-full px-3 py-2 border rounded-md text-sm bg-white dark:bg-gray-800 dark:border-gray-700"
                                    placeholder="sony, apple, nike..."
                                    prop:value=move || draft_slug.get()
                                    on:input=move |ev| draft_slug.set(event_target_value(&ev))
                                />
                            </div>
                            <div>
                                <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                    {t(Some(locale), "brand.form-websiteUrl", "Website URL")}
                                </label>
                                <input
                                    type="url"
                                    class="w-full px-3 py-2 border rounded-md text-sm bg-white dark:bg-gray-800 dark:border-gray-700"
                                    placeholder="https://..."
                                    prop:value=move || draft_website_url.get()
                                    on:input=move |ev| draft_website_url.set(event_target_value(&ev))
                                />
                            </div>
                            <div>
                                <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                    {t(Some(locale), "brand.logo", "Logo URL")}
                                </label>
                                <input
                                    type="url"
                                    class="w-full px-3 py-2 border rounded-md text-sm bg-white dark:bg-gray-800 dark:border-gray-700"
                                    placeholder="https://.../logo.png"
                                    prop:value=move || draft_logo_url.get()
                                    on:input=move |ev| draft_logo_url.set(event_target_value(&ev))
                                />
                            </div>
                            <div class="md:col-span-2">
                                <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                    {t(Some(locale), "brand.description", "Description")}
                                </label>
                                <textarea
                                    rows="2"
                                    class="w-full px-3 py-2 border rounded-md text-sm bg-white dark:bg-gray-800 dark:border-gray-700"
                                    prop:value=move || draft_description.get()
                                    on:input=move |ev| draft_description.set(event_target_value(&ev))
                                ></textarea>
                            </div>
                            <div class="flex items-center gap-4 md:col-span-2">
                                <label class="flex items-center gap-2 text-sm text-gray-700 dark:text-gray-300">
                                    <input
                                        type="checkbox"
                                        class="rounded border-gray-300 text-indigo-600 focus:ring-indigo-500"
                                        prop:checked=move || draft_is_active.get()
                                        on:change=move |ev| draft_is_active.set(event_target_checked(&ev))
                                    />
                                    {t(Some(locale), "brand.form-activeInCatalog", "Active in catalog")}
                                </label>
                                <div class="flex items-center gap-2 text-sm">
                                    <span class="text-xs text-gray-600 dark:text-gray-400">
                                        {t(Some(locale), "brand.form-sortOrder", "Sort order:")}
                                    </span>
                                    <input
                                        type="number"
                                        class="w-20 px-2 py-1 border rounded text-sm bg-white dark:bg-gray-800"
                                        prop:value=move || draft_sort_order.get().to_string()
                                        on:input=move |ev| {
                                            if let Ok(num) = event_target_value(&ev).parse::<i32>() {
                                                draft_sort_order.set(num);
                                            }
                                        }
                                    />
                                </div>
                            </div>
                        </div>
                        <div class="flex justify-end gap-3 pt-3 border-t">
                            <button
                                type="button"
                                class="px-4 py-2 border rounded-md text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800"
                                on:click=move |_| reset_form()
                            >
                                {t(Some(locale), "brand.cancel", "Cancel")}
                            </button>
                            <button
                                type="button"
                                class="px-4 py-2 rounded-md text-sm font-medium text-white bg-indigo-600 hover:bg-indigo-700 disabled:opacity-50"
                                disabled=move || busy.get()
                                on:click=on_save
                            >
                                {if busy.get() {
                                    t(Some(locale), "brand.action-saving", "Saving...")
                                } else {
                                    t(Some(locale), "brand.save", "Save")
                                }}
                            </button>
                        </div>
                    </div>
                })
            }}

            // Selection toolbar
            <Show when=move || !selection.get().is_empty()>
                <div class="flex items-center justify-between gap-3 px-4 py-2.5 rounded-xl border border-indigo-200 bg-indigo-50/50 dark:border-indigo-900/50 dark:bg-indigo-950/20 text-sm">
                    <div class="flex items-center gap-2">
                        <span class="font-medium text-foreground">
                            {move || format!("{} {} {}", selection.get().count(), if is_ru { "выбрано" } else { "selected" }, if is_ru { "брендов" } else { "brands" })}
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

            // Brands Directory Table
            <div class="border rounded-xl overflow-hidden bg-card border-border shadow-sm p-4">
                <Suspense fallback=move || view! {
                    <div class="p-8 text-center text-muted-foreground">
                        {t(Some(locale), "brand.loadingList", "Loading brands...")}
                    </div>
                }>
                    {move || match directory.get() {
                        None => view! {
                            <div class="p-8 text-center text-muted-foreground">
                                {t(Some(locale), "brand.loading", "Loading...")}
                            </div>
                        }.into_any(),
                        Some(Err(err)) => view! {
                            <div class="p-8 text-center text-rose-500">
                                <p class="font-semibold">{t(Some(locale), "brand.error-load", "Failed to load brands")}</p>
                                <p class="text-sm mt-1">{err.to_string()}</p>
                            </div>
                        }.into_any(),
                        Some(Ok(_)) => {
                            let empty_msg = t(Some(locale), "brand.empty", "No brands found").to_string();
                            view! {
                                <div class="space-y-4">
                                    <DataGrid
                                        columns=columns.clone()
                                        data=Signal::derive(move || filtered_brands.get())
                                        key_fn=|item: &BrandAdminListItem| item.id.clone()
                                        cell_renderer=cell_renderer
                                        is_loading=Signal::derive(move || busy.get() || directory.get().is_none())
                                        empty_message=empty_msg
                                        selection=selection
                                        pagination=pagination
                                        filters=filters
                                        on_filter_change=on_filters_change
                                        on_row_click=on_row_click
                                    />
                                </div>
                            }.into_any()
                        }
                    }}
                </Suspense>
            </div>
        </div>
    }
}

fn optional_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn transport_context(profile: BrandAdminTransportProfile) -> BrandAdminTransportContext {
    match profile {
        BrandAdminTransportProfile::Native => BrandAdminTransportContext::native(),
        BrandAdminTransportProfile::Graphql => BrandAdminTransportContext::graphql(None, None),
    }
}
