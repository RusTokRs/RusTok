use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    BundleAdminTransportProfile, build_bundle_admin_shell, selected_transport_profile,
    validate_bundle_discount, validate_bundle_name, validate_bundle_slug,
};
use crate::i18n::normalize_admin_locale;
use crate::model::{
    BundleAdminCommand, BundleAdminCreateDraft, BundleAdminFilters, BundleAdminUpdateDraft,
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
pub fn BundleAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = normalize_admin_locale(route_context.locale.as_deref());
    let russian = locale == "ru";
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
        move || (refresh_nonce.get(), search.get(), status_filter.get(), type_filter.get()),
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
                        notice.set(Some(if russian {
                            "Комплект успешно создан.".to_string()
                        } else {
                            "Bundle created successfully.".to_string()
                        }));
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
                        notice.set(Some(if russian {
                            "Комплект успешно обновлён.".to_string()
                        } else {
                            "Bundle updated successfully.".to_string()
                        }));
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
                        notice.set(Some(if russian {
                            "Комплект успешно удалён.".to_string()
                        } else {
                            "Bundle deleted successfully.".to_string()
                        }));
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
                        {if russian { "Комплекты товаров" } else { "Product Bundles" }}
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
                        {if russian { "Новый комплект" } else { "New Bundle" }}
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
            })}

            // Filters Section
            <div class="flex flex-col sm:flex-row gap-4 items-center justify-between bg-white dark:bg-gray-900 p-4 rounded-xl border border-gray-200 dark:border-gray-800">
                <div class="relative w-full sm:w-80">
                    <div class="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none text-gray-400">
                        <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"/>
                        </svg>
                    </div>
                    <input
                        type="text"
                        class="block w-full pl-10 pr-3 py-2 border border-gray-300 dark:border-gray-700 rounded-lg bg-white dark:bg-gray-800 text-gray-900 dark:text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500 text-sm"
                        placeholder=if russian { "Поиск по названию или slug..." } else { "Search by name or slug..." }
                        prop:value=move || search.get()
                        on:input=move |ev| search.set(event_target_value(&ev))
                    />
                </div>

                <div class="flex items-center gap-2 w-full sm:w-auto overflow-x-auto">
                    <button
                        type="button"
                        class=move || format!(
                            "px-3 py-1.5 rounded-lg text-xs font-medium transition-colors {}",
                            if status_filter.get().is_none() {
                                "bg-blue-600 text-white"
                            } else {
                                "bg-gray-100 text-gray-700 hover:bg-gray-200 dark:bg-gray-800 dark:text-gray-300"
                            }
                        )
                        on:click=move |_| status_filter.set(None)
                    >
                        {if russian { "Все" } else { "All" }}
                    </button>
                    <button
                        type="button"
                        class=move || format!(
                            "px-3 py-1.5 rounded-lg text-xs font-medium transition-colors {}",
                            if status_filter.get().as_deref() == Some("active") {
                                "bg-green-600 text-white"
                            } else {
                                "bg-gray-100 text-gray-700 hover:bg-gray-200 dark:bg-gray-800 dark:text-gray-300"
                            }
                        )
                        on:click=move |_| status_filter.set(Some("active".to_string()))
                    >
                        {if russian { "Активные" } else { "Active" }}
                    </button>
                    <button
                        type="button"
                        class=move || format!(
                            "px-3 py-1.5 rounded-lg text-xs font-medium transition-colors {}",
                            if status_filter.get().as_deref() == Some("draft") {
                                "bg-amber-600 text-white"
                            } else {
                                "bg-gray-100 text-gray-700 hover:bg-gray-200 dark:bg-gray-800 dark:text-gray-300"
                            }
                        )
                        on:click=move |_| status_filter.set(Some("draft".to_string()))
                    >
                        {if russian { "Черновики" } else { "Draft" }}
                    </button>
                </div>
            </div>

            // Directory Table
            <div class="bg-white dark:bg-gray-900 rounded-xl border border-gray-200 dark:border-gray-800 overflow-hidden shadow-sm">
                <Suspense fallback=move || view! {
                    <div class="p-8 text-center text-gray-500 dark:text-gray-400 text-sm">
                        {if russian { "Загрузка комплектов..." } else { "Loading bundles..." }}
                    </div>
                }>
                    {move || {
                        directory.get().map(|res| match res {
                            Ok(dir) if dir.items.is_empty() => view! {
                                <div class="p-12 text-center">
                                    <svg class="mx-auto h-12 w-12 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4"/>
                                    </svg>
                                    <h3 class="mt-2 text-sm font-medium text-gray-900 dark:text-white">
                                        {if russian { "Комплекты не найдены" } else { "No bundles found" }}
                                    </h3>
                                    <p class="mt-1 text-sm text-gray-500 dark:text-gray-400">
                                        {if russian { "Создайте первый комплект товаров для вашего каталога." } else { "Create the first product bundle for your catalog." }}
                                    </p>
                                </div>
                            }.into_any(),
                            Ok(dir) => view! {
                                <div class="overflow-x-auto">
                                    <table class="min-w-full divide-y divide-gray-200 dark:divide-gray-800 text-left text-sm">
                                        <thead class="bg-gray-50 dark:bg-gray-800/50 text-gray-600 dark:text-gray-400 font-medium">
                                            <tr>
                                                <th class="px-6 py-3">{if russian { "Название" } else { "Name" }}</th>
                                                <th class="px-6 py-3">{if russian { "Slug" } else { "Slug" }}</th>
                                                <th class="px-6 py-3">{if russian { "Тип" } else { "Type" }}</th>
                                                <th class="px-6 py-3">{if russian { "Скидка" } else { "Discount" }}</th>
                                                <th class="px-6 py-3">{if russian { "Позиций" } else { "Items" }}</th>
                                                <th class="px-6 py-3">{if russian { "Статус" } else { "Status" }}</th>
                                                <th class="px-6 py-3 text-right">{if russian { "Действия" } else { "Actions" }}</th>
                                            </tr>
                                        </thead>
                                        <tbody class="divide-y divide-gray-200 dark:divide-gray-800">
                                            {dir.items.into_iter().map(|item| {
                                                let id_for_edit = item.id.clone();
                                                let id_for_del = item.id.clone();
                                                let item_clone = item.clone();
                                                let is_active = item.status == "active";

                                                view! {
                                                    <tr class="hover:bg-gray-50/50 dark:hover:bg-gray-800/40 transition-colors">
                                                        <td class="px-6 py-4 font-medium text-gray-900 dark:text-white">
                                                            {item.name}
                                                        </td>
                                                        <td class="px-6 py-4 text-gray-500 dark:text-gray-400 font-mono text-xs">
                                                            {item.slug}
                                                        </td>
                                                        <td class="px-6 py-4">
                                                            <span class="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-gray-100 text-gray-800 dark:bg-gray-800 dark:text-gray-300">
                                                                {item.bundle_type}
                                                            </span>
                                                        </td>
                                                        <td class="px-6 py-4 text-gray-600 dark:text-gray-300 text-xs">
                                                            {if item.discount_type == "none" {
                                                                "-".to_string()
                                                            } else {
                                                                format!("{} ({})", item.discount_value, item.discount_type)
                                                            }}
                                                        </td>
                                                        <td class="px-6 py-4 text-gray-500 dark:text-gray-400">
                                                            {item.items_count}
                                                        </td>
                                                        <td class="px-6 py-4">
                                                            <span class=format!(
                                                                "inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium {}",
                                                                if is_active {
                                                                    "bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400"
                                                                } else {
                                                                    "bg-gray-100 text-gray-800 dark:bg-gray-800 dark:text-gray-400"
                                                                }
                                                            )>
                                                                {item.status}
                                                            </span>
                                                        </td>
                                                        <td class="px-6 py-4 text-right space-x-2">
                                                            <button
                                                                type="button"
                                                                class="text-blue-600 hover:text-blue-800 dark:text-blue-400 text-xs font-medium"
                                                                on:click=move |_| {
                                                                    draft_slug.set(item_clone.slug.clone());
                                                                    draft_name.set(item_clone.name.clone());
                                                                    draft_description.set(item_clone.description.clone().unwrap_or_default());
                                                                    draft_bundle_type.set(item_clone.bundle_type.clone());
                                                                    draft_status.set(item_clone.status.clone());
                                                                    draft_discount_type.set(item_clone.discount_type.clone());
                                                                    draft_discount_value.set(item_clone.discount_value.clone());
                                                                    edit_bundle_id.set(Some(id_for_edit.clone()));
                                                                }
                                                            >
                                                                {if russian { "Ред." } else { "Edit" }}
                                                            </button>
                                                            <button
                                                                type="button"
                                                                class="text-red-600 hover:text-red-800 dark:text-red-400 text-xs font-medium"
                                                                on:click=move |_| delete_confirm_id.set(Some(id_for_del.clone()))
                                                            >
                                                                {if russian { "Удалить" } else { "Delete" }}
                                                            </button>
                                                        </td>
                                                    </tr>
                                                }
                                            }).collect_view()}
                                        </tbody>
                                    </table>
                                </div>
                            }.into_any(),
                            Err(e) => view! {
                                <div class="p-8 text-center text-red-600 dark:text-red-400 text-sm">
                                    {format!("Error: {e}")}
                                </div>
                            }.into_any(),
                        })
                    }}
                </Suspense>
            </div>

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
                                {if russian { "Создать комплект" } else { "Create Bundle" }}
                            </h2>
                            <button type="button" class="text-gray-400 hover:text-gray-600" on:click=move |_| show_create_modal.set(false)>
                                "×"
                            </button>
                        </div>

                        <div class="space-y-3 text-sm">
                            <div>
                                <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                    {if russian { "Название *" } else { "Name *" }}
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
                                    {if russian { "ЧПУ (slug) *" } else { "Slug *" }}
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
                                        {if russian { "Тип" } else { "Type" }}
                                    </label>
                                    <select
                                        class="w-full px-3 py-2 border rounded-lg dark:bg-gray-800 dark:border-gray-700 text-xs"
                                        on:change=move |ev| draft_bundle_type.set(event_target_value(&ev))
                                    >
                                        <option value="fixed" selected=move || draft_bundle_type.get() == "fixed">
                                            {if russian { "Фиксированный" } else { "Fixed" }}
                                        </option>
                                        <option value="flexible" selected=move || draft_bundle_type.get() == "flexible">
                                            {if russian { "Настраиваемый" } else { "Flexible" }}
                                        </option>
                                    </select>
                                </div>

                                <div>
                                    <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                        {if russian { "Статус" } else { "Status" }}
                                    </label>
                                    <select
                                        class="w-full px-3 py-2 border rounded-lg dark:bg-gray-800 dark:border-gray-700 text-xs"
                                        on:change=move |ev| draft_status.set(event_target_value(&ev))
                                    >
                                        <option value="active" selected=move || draft_status.get() == "active">
                                            {if russian { "Активен" } else { "Active" }}
                                        </option>
                                        <option value="draft" selected=move || draft_status.get() == "draft">
                                            {if russian { "Черновик" } else { "Draft" }}
                                        </option>
                                        <option value="archived" selected=move || draft_status.get() == "archived">
                                            {if russian { "В архиве" } else { "Archived" }}
                                        </option>
                                    </select>
                                </div>
                            </div>

                            <div class="grid grid-cols-2 gap-3">
                                <div>
                                    <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                        {if russian { "Тип скидки" } else { "Discount Type" }}
                                    </label>
                                    <select
                                        class="w-full px-3 py-2 border rounded-lg dark:bg-gray-800 dark:border-gray-700 text-xs"
                                        on:change=move |ev| draft_discount_type.set(event_target_value(&ev))
                                    >
                                        <option value="none" selected=move || draft_discount_type.get() == "none">
                                            {if russian { "Без скидки" } else { "None" }}
                                        </option>
                                        <option value="percentage" selected=move || draft_discount_type.get() == "percentage">
                                            {if russian { "Процентная (%)" } else { "Percentage (%)" }}
                                        </option>
                                        <option value="fixed_amount" selected=move || draft_discount_type.get() == "fixed_amount">
                                            {if russian { "Фикс. сумма" } else { "Fixed Amount" }}
                                        </option>
                                    </select>
                                </div>

                                <div>
                                    <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                        {if russian { "Размер скидки" } else { "Discount Value" }}
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
                                    {if russian { "Описание" } else { "Description" }}
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
                                {if russian { "Отмена" } else { "Cancel" }}
                            </button>
                            <button
                                type="button"
                                class="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-lg hover:bg-blue-700"
                                on:click=on_create
                            >
                                {if russian { "Создать" } else { "Create" }}
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
                                    {if russian { "Редактировать комплект" } else { "Edit Bundle" }}
                                </h2>
                                <button type="button" class="text-gray-400 hover:text-gray-600" on:click=move |_| edit_bundle_id.set(None)>
                                    "×"
                                </button>
                            </div>

                            <div class="space-y-3 text-sm">
                                <div>
                                    <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                        {if russian { "Название *" } else { "Name *" }}
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
                                        {if russian { "ЧПУ (slug) *" } else { "Slug *" }}
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
                                            {if russian { "Тип" } else { "Type" }}
                                        </label>
                                        <select
                                            class="w-full px-3 py-2 border rounded-lg dark:bg-gray-800 dark:border-gray-700 text-xs"
                                            on:change=move |ev| draft_bundle_type.set(event_target_value(&ev))
                                        >
                                            <option value="fixed" selected=move || draft_bundle_type.get() == "fixed">
                                                {if russian { "Фиксированный" } else { "Fixed" }}
                                            </option>
                                            <option value="flexible" selected=move || draft_bundle_type.get() == "flexible">
                                                {if russian { "Настраиваемый" } else { "Flexible" }}
                                            </option>
                                        </select>
                                    </div>

                                    <div>
                                        <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                            {if russian { "Статус" } else { "Status" }}
                                        </label>
                                        <select
                                            class="w-full px-3 py-2 border rounded-lg dark:bg-gray-800 dark:border-gray-700 text-xs"
                                            on:change=move |ev| draft_status.set(event_target_value(&ev))
                                        >
                                            <option value="active" selected=move || draft_status.get() == "active">
                                                {if russian { "Активен" } else { "Active" }}
                                            </option>
                                            <option value="draft" selected=move || draft_status.get() == "draft">
                                                {if russian { "Черновик" } else { "Draft" }}
                                            </option>
                                            <option value="archived" selected=move || draft_status.get() == "archived">
                                                {if russian { "В архиве" } else { "Archived" }}
                                            </option>
                                        </select>
                                    </div>
                                </div>

                                <div class="grid grid-cols-2 gap-3">
                                    <div>
                                        <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                            {if russian { "Тип скидки" } else { "Discount Type" }}
                                        </label>
                                        <select
                                            class="w-full px-3 py-2 border rounded-lg dark:bg-gray-800 dark:border-gray-700 text-xs"
                                            on:change=move |ev| draft_discount_type.set(event_target_value(&ev))
                                        >
                                            <option value="none" selected=move || draft_discount_type.get() == "none">
                                                {if russian { "Без скидки" } else { "None" }}
                                            </option>
                                            <option value="percentage" selected=move || draft_discount_type.get() == "percentage">
                                                {if russian { "Процентная (%)" } else { "Percentage (%)" }}
                                            </option>
                                            <option value="fixed_amount" selected=move || draft_discount_type.get() == "fixed_amount">
                                                {if russian { "Фикс. сумма" } else { "Fixed Amount" }}
                                            </option>
                                        </select>
                                    </div>

                                    <div>
                                        <label class="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                            {if russian { "Размер скидки" } else { "Discount Value" }}
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
                                        {if russian { "Описание" } else { "Description" }}
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
                                    {if russian { "Отмена" } else { "Cancel" }}
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
                                    {if russian { "Сохранить" } else { "Save" }}
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
                                {if russian { "Удаление комплекта" } else { "Delete Bundle" }}
                            </h3>
                            <p class="text-sm text-gray-500 dark:text-gray-400">
                                {if russian { "Вы уверены, что хотите удалить этот комплект? Это действие необратимо." } else { "Are you sure you want to delete this bundle? This action cannot be undone." }}
                            </p>
                            <div class="flex justify-end gap-3 pt-2">
                                <button
                                    type="button"
                                    class="px-4 py-2 text-sm border rounded-lg dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800"
                                    on:click=move |_| delete_confirm_id.set(None)
                                >
                                    {if russian { "Отмена" } else { "Cancel" }}
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
                                    {if russian { "Удалить" } else { "Delete" }}
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
        BundleAdminTransportProfile::Graphql => {
            BundleAdminTransportContext::graphql(None, None)
        }
    }
}
