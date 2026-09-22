use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    BrandAdminTransportProfile, build_brand_admin_shell, selected_transport_profile,
    validate_brand_name, validate_brand_slug,
};
use crate::i18n::normalize_admin_locale;
use crate::model::{
    BrandAdminCommand, BrandAdminCreateDraft, BrandAdminFilters,
    BrandAdminUpdateDraft,
};
use crate::transport::{
    BrandAdminTransportContext, execute_brand_command, load_brand_directory,
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
pub fn BrandAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = normalize_admin_locale(route_context.locale.as_deref());
    let russian = locale == "ru";
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
                        notice.set(Some(if russian {
                            "Бренд успешно создан".to_string()
                        } else {
                            "Brand created successfully".to_string()
                        }));
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
                        notice.set(Some(if russian {
                            "Бренд успешно обновлён".to_string()
                        } else {
                            "Brand updated successfully".to_string()
                        }));
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
                        notice.set(Some(if russian {
                            "Бренд успешно удалён".to_string()
                        } else {
                            "Brand deleted successfully".to_string()
                        }));
                        refresh_nonce.update(|n| *n += 1);
                    }
                    Err(e) => {
                        error.set(Some(format!("{e}")));
                    }
                }
            });
        }
    };

    view! {
        <div class="brand-admin-container p-6 space-y-6">
            // Header
            <div class="flex flex-col md:flex-row md:items-center md:justify-between gap-4 border-b pb-4">
                <div>
                    <span class="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-indigo-100 text-indigo-800 dark:bg-indigo-900/40 dark:text-indigo-300 mb-1">
                        {if russian { "Каталог брендов" } else { "Brand Catalog" }}
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
                        {if russian { "+ Новый бренд" } else { "+ New Brand" }}
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
                        placeholder=if russian { "Поиск по названию или ЧПУ..." } else { "Search by name or slug..." }
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
                        {if russian { "Все" } else { "All" }}
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
                        {if russian { "Активные" } else { "Active" }}
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
                        {if russian { "Неактивные" } else { "Inactive" }}
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
                    if russian { "Редактирование бренда" } else { "Edit Brand" }
                } else if russian {
                    "Создание нового бренда"
                } else {
                    "Create New Brand"
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
                                    {if russian { "Название *" } else { "Name *" }}
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
                                    {if russian { "ЧПУ (slug) *" } else { "Slug *" }}
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
                                    {if russian { "Веб-сайт" } else { "Website URL" }}
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
                                    {if russian { "URL логотипа" } else { "Logo URL" }}
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
                                    {if russian { "Описание" } else { "Description" }}
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
                                    {if russian { "Активен в каталоге" } else { "Active in catalog" }}
                                </label>
                                <div class="flex items-center gap-2 text-sm">
                                    <span class="text-xs text-gray-600 dark:text-gray-400">
                                        {if russian { "Сортировка:" } else { "Sort order:" }}
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
                                {if russian { "Отмена" } else { "Cancel" }}
                            </button>
                            <button
                                type="button"
                                class="px-4 py-2 rounded-md text-sm font-medium text-white bg-indigo-600 hover:bg-indigo-700 disabled:opacity-50"
                                disabled=move || busy.get()
                                on:click=on_save
                            >
                                {if busy.get() {
                                    if russian { "Сохранение..." } else { "Saving..." }
                                } else if russian {
                                    "Сохранить"
                                } else {
                                    "Save"
                                }}
                            </button>
                        </div>
                    </div>
                })
            }}

            // Brands Directory Table
            <div class="border rounded-lg overflow-hidden bg-white dark:bg-gray-800 shadow-sm">
                <Suspense fallback=move || view! {
                    <div class="p-8 text-center text-gray-500 dark:text-gray-400">
                        {if russian { "Загрузка списка брендов..." } else { "Loading brands..." }}
                    </div>
                }>
                    {move || match directory.get() {
                        None => view! {
                            <div class="p-8 text-center text-gray-500 dark:text-gray-400">
                                {if russian { "Загрузка..." } else { "Loading..." }}
                            </div>
                        }.into_any(),
                        Some(Err(err)) => view! {
                            <div class="p-8 text-center text-red-500">
                                <p class="font-semibold">{if russian { "Ошибка загрузки брендов" } else { "Failed to load brands" }}</p>
                                <p class="text-sm mt-1">{err.to_string()}</p>
                            </div>
                        }.into_any(),
                        Some(Ok(data)) => {
                            if data.items.is_empty() {
                                return view! {
                                    <div class="p-8 text-center text-gray-500 dark:text-gray-400">
                                        <p class="font-medium text-base">
                                            {if russian { "Бренды не найдены" } else { "No brands found" }}
                                        </p>
                                        <p class="text-xs mt-1 text-gray-400">
                                            {if russian { "Создайте первый бренд с помощью кнопки выше" } else { "Create your first brand using the button above" }}
                                        </p>
                                    </div>
                                }.into_any();
                            }

                            let items = data.items.clone();
                            view! {
                                <table class="min-w-full divide-y divide-gray-200 dark:divide-gray-700 text-left text-sm">
                                    <thead class="bg-gray-50 dark:bg-gray-900/50 text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider">
                                        <tr>
                                            <th class="px-6 py-3">{if russian { "Бренд" } else { "Brand" }}</th>
                                            <th class="px-6 py-3">{if russian { "ЧПУ" } else { "Slug" }}</th>
                                            <th class="px-6 py-3">{if russian { "Веб-сайт" } else { "Website" }}</th>
                                            <th class="px-6 py-3">{if russian { "Товары" } else { "Products" }}</th>
                                            <th class="px-6 py-3">{if russian { "Статус" } else { "Status" }}</th>
                                            <th class="px-6 py-3 text-right">{if russian { "Действия" } else { "Actions" }}</th>
                                        </tr>
                                    </thead>
                                    <tbody class="divide-y divide-gray-200 dark:divide-gray-700">
                                        {items.into_iter().map(|brand| {
                                            let brand_id = brand.id.clone();
                                            let brand_for_edit = brand.clone();
                                            let on_delete_brand = on_delete.clone();

                                            view! {
                                                <tr class="hover:bg-gray-50 dark:hover:bg-gray-750">
                                                    <td class="px-6 py-4 flex items-center gap-3">
                                                        {if let Some(logo) = brand.logo_url {
                                                            view! {
                                                                <img src=logo alt=brand.name.clone() class="w-8 h-8 rounded object-contain bg-gray-100 dark:bg-gray-700 p-0.5" />
                                                            }.into_any()
                                                        } else {
                                                            view! {
                                                                <div class="w-8 h-8 rounded bg-indigo-50 dark:bg-indigo-900/30 text-indigo-600 dark:text-indigo-400 flex items-center justify-center font-bold text-xs">
                                                                    {brand.name.chars().next().unwrap_or('B').to_string()}
                                                                </div>
                                                            }.into_any()
                                                        }}
                                                        <div>
                                                            <span class="font-medium text-gray-900 dark:text-gray-100">{brand.name}</span>
                                                            {if let Some(desc) = brand.description {
                                                                view! {
                                                                    <p class="text-xs text-gray-400 truncate max-w-xs">{desc}</p>
                                                                }.into_any()
                                                            } else {
                                                                view! { <span></span> }.into_any()
                                                            }}
                                                        </div>
                                                    </td>
                                                    <td class="px-6 py-4 text-xs font-mono text-gray-500 dark:text-gray-400">
                                                        {brand.slug}
                                                    </td>
                                                    <td class="px-6 py-4 text-xs">
                                                        {if let Some(url) = brand.website_url {
                                                            let display_url = url.clone();
                                                            view! {
                                                                <a href=url target="_blank" rel="noopener noreferrer" class="text-indigo-600 dark:text-indigo-400 hover:underline">
                                                                    {display_url}
                                                                </a>
                                                            }.into_any()
                                                        } else {
                                                            view! { <span class="text-gray-400">"-"</span> }.into_any()
                                                        }}
                                                    </td>
                                                    <td class="px-6 py-4 text-xs font-semibold">
                                                        <span class="px-2 py-0.5 rounded-full bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300">
                                                            {brand.products_count.to_string()}
                                                        </span>
                                                    </td>
                                                    <td class="px-6 py-4 text-xs">
                                                        {if brand.is_active {
                                                            view! {
                                                                <span class="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-green-100 text-green-800 dark:bg-green-900/40 dark:text-green-300">
                                                                    {if russian { "Активен" } else { "Active" }}
                                                                </span>
                                                            }.into_any()
                                                        } else {
                                                            view! {
                                                                <span class="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-gray-100 text-gray-800 dark:bg-gray-700 dark:text-gray-300">
                                                                    {if russian { "Неактивен" } else { "Inactive" }}
                                                                </span>
                                                            }.into_any()
                                                        }}
                                                    </td>
                                                    <td class="px-6 py-4 text-right text-xs font-medium space-x-2">
                                                        <button
                                                            type="button"
                                                            class="text-indigo-600 hover:text-indigo-900 dark:text-indigo-400 dark:hover:text-indigo-300"
                                                            on:click={
                                                                let b = brand_for_edit.clone();
                                                                move |_| {
                                                                    draft_slug.set(b.slug.clone());
                                                                    draft_name.set(b.name.clone());
                                                                    draft_description.set(b.description.clone().unwrap_or_default());
                                                                    draft_website_url.set(b.website_url.clone().unwrap_or_default());
                                                                    draft_logo_url.set(b.logo_url.clone().unwrap_or_default());
                                                                    draft_is_active.set(b.is_active);
                                                                    draft_sort_order.set(b.sort_order);
                                                                    edit_brand_id.set(Some(b.id.clone()));
                                                                }
                                                            }
                                                        >
                                                            {if russian { "Изменить" } else { "Edit" }}
                                                        </button>
                                                        <button
                                                            type="button"
                                                            class="text-red-600 hover:text-red-900 dark:text-red-400 dark:hover:text-red-300"
                                                            on:click={
                                                                let id = brand_id.clone();
                                                                move |_| {
                                                                    on_delete_brand(id.clone());
                                                                }
                                                            }
                                                        >
                                                            {if russian { "Удалить" } else { "Delete" }}
                                                        </button>
                                                    </td>
                                                </tr>
                                            }
                                        }).collect_view()}
                                    </tbody>
                                </table>
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
