use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_ui_core::UiRouteContext;

use crate::model::{
    ProductAttributeDraft, ProductAttributeOptionDraft, ProductAttributeSchemaDraft,
    ProductAttributeSchemaSummary, ProductAttributeSummary,
};
use crate::transport;

#[component]
pub fn AttributesPage() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale_val = route_context.locale.clone();
    let is_ru = locale_val.as_deref() == Some("ru");
    let locale_store = StoredValue::new(locale_val);
    let base_route = route_context.admin_module_route_base("product");
    let token = use_token();
    let tenant = use_tenant();

    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (is_busy, set_is_busy) = signal(false);
    let (error_msg, set_error_msg) = signal(Option::<String>::None);
    let (success_msg, set_success_msg) = signal(Option::<String>::None);

    // Filter/Tab state
    let (active_tab, set_active_tab) = signal("attributes".to_string());
    let (search_query, set_search_query) = signal(String::new());

    // Attribute Form fields
    let (attr_label, set_attr_label) = signal(String::new());
    let (attr_code, set_attr_code) = signal(String::new());
    let (attr_value_type, set_attr_value_type) = signal("text".to_string());
    let (attr_help_text, set_attr_help_text) = signal(String::new());
    let (attr_is_localized, set_attr_is_localized) = signal(false);
    let (attr_is_filterable, set_attr_is_filterable) = signal(true);
    let (attr_is_searchable, set_attr_is_searchable) = signal(true);
    let (attr_is_sortable, set_attr_is_sortable) = signal(false);
    let (attr_show_on_storefront, set_attr_show_on_storefront) = signal(true);

    // Option Dialog/Subform fields
    let (selected_attr_for_opt, set_selected_attr_for_opt) =
        signal(Option::<ProductAttributeSummary>::None);
    let (opt_label, set_opt_label) = signal(String::new());
    let (opt_code, set_opt_code) = signal(String::new());

    // Schema Form fields
    let (schema_name, set_schema_name) = signal(String::new());
    let (schema_code, set_schema_code) = signal(String::new());
    let (schema_desc, set_schema_desc) = signal(String::new());

    // Load attributes resource
    let attributes_resource = LocalResource::new(move || {
        let tok = token.get();
        let ten = tenant.get();
        let loc = locale_store.get_value().unwrap_or_default();
        let _ = refresh_nonce.get();
        async move {
            let bootstrap = transport::fetch_bootstrap(tok.clone(), ten.clone())
                .await
                .map_err(|e| e.to_string())?;
            let res = transport::fetch_product_attributes(
                tok,
                ten,
                bootstrap.current_tenant.id,
                loc,
            )
            .await
            .map_err(|e| e.to_string())?;
            Ok::<Vec<ProductAttributeSummary>, String>(res.items)
        }
    });

    // Load schemas resource
    let schemas_resource = LocalResource::new(move || {
        let tok = token.get();
        let ten = tenant.get();
        let loc = locale_store.get_value().unwrap_or_default();
        let _ = refresh_nonce.get();
        async move {
            let bootstrap = transport::fetch_bootstrap(tok.clone(), ten.clone())
                .await
                .map_err(|e| e.to_string())?;
            let res = transport::fetch_attribute_schemas(
                tok,
                ten,
                bootstrap.current_tenant.id,
                loc,
            )
            .await
            .map_err(|e| e.to_string())?;
            Ok::<Vec<ProductAttributeSchemaSummary>, String>(res.items)
        }
    });

    // Auto-generate code from label
    let on_label_input = move |val: String| {
        let generated_code = val
            .to_lowercase()
            .trim()
            .replace(|c: char| !c.is_alphanumeric() && c != '_', "_")
            .trim_matches('_')
            .to_string();
        set_attr_label.set(val);
        set_attr_code.set(generated_code);
    };

    let reset_attr_form = move || {
        set_attr_label.set(String::new());
        set_attr_code.set(String::new());
        set_attr_value_type.set("text".to_string());
        set_attr_help_text.set(String::new());
        set_attr_is_localized.set(false);
        set_attr_is_filterable.set(true);
        set_attr_is_searchable.set(true);
        set_attr_is_sortable.set(false);
        set_attr_show_on_storefront.set(true);
        set_error_msg.set(None);
        set_success_msg.set(None);
    };

    view! {
        <div class="flex flex-col gap-6 w-full max-w-6xl mx-auto pb-12 animate-in fade-in duration-150">
            // Header
            <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4 bg-card rounded-2xl border border-border p-4 shadow-sm">
                <div class="flex items-center gap-3">
                    <a
                        href=base_route.clone()
                        class="inline-flex items-center justify-center w-8 h-8 rounded-lg border border-border bg-background text-foreground hover:bg-accent transition text-sm"
                        title=if is_ru { "Назад к каталогу" } else { "Back to catalog" }
                    >
                        "←"
                    </a>
                    <div>
                        <h1 class="text-xl font-bold tracking-tight text-foreground flex items-center gap-2">
                            <span>{if is_ru { "Характеристики и схемы" } else { "Attributes & Schemas" }}</span>
                            <span class="text-xs font-normal px-2 py-0.5 rounded-full bg-primary/10 text-primary border border-primary/20">
                                {move || attributes_resource.get().and_then(Result::ok).map(|a| a.len()).unwrap_or(0)}
                            </span>
                        </h1>
                        <p class="text-xs text-muted-foreground mt-0.5">
                            {if is_ru {
                                "Типизированные характеристики каталога, словари опций и схемы категорий"
                            } else {
                                "Typed product attributes, option dictionaries and category schema templates"
                            }}
                        </p>
                    </div>
                </div>

                <div class="flex items-center gap-2">
                    <div class="flex rounded-xl border border-border p-1 bg-muted/40">
                        <button
                            type="button"
                            class=move || {
                                if active_tab.get() == "attributes" {
                                    "px-3 py-1 text-xs font-semibold rounded-lg bg-background text-foreground shadow-sm transition"
                                } else {
                                    "px-3 py-1 text-xs font-medium text-muted-foreground hover:text-foreground transition"
                                }
                            }
                            on:click=move |_| set_active_tab.set("attributes".to_string())
                        >
                            {if is_ru { "Атрибуты" } else { "Attributes" }}
                        </button>
                        <button
                            type="button"
                            class=move || {
                                if active_tab.get() == "schemas" {
                                    "px-3 py-1 text-xs font-semibold rounded-lg bg-background text-foreground shadow-sm transition"
                                } else {
                                    "px-3 py-1 text-xs font-medium text-muted-foreground hover:text-foreground transition"
                                }
                            }
                            on:click=move |_| set_active_tab.set("schemas".to_string())
                        >
                            {if is_ru { "Схемы" } else { "Schemas" }}
                        </button>
                    </div>
                    <a
                        href=format!("{base_route}/categories")
                        class="h-9 px-3 rounded-xl border border-border bg-background text-xs font-medium text-foreground hover:bg-accent transition inline-flex items-center"
                    >
                        {if is_ru { "📁 Категории" } else { "📁 Categories" }}
                    </a>
                    <button
                        type="button"
                        class="h-9 px-3 rounded-xl border border-border bg-background text-xs font-medium text-foreground hover:bg-accent transition"
                        on:click=move |_| set_refresh_nonce.update(|n| *n += 1)
                    >
                        "↻"
                    </button>
                </div>
            </div>

            // Alerts
            <Show when=move || error_msg.get().is_some()>
                <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-xs text-destructive flex items-center justify-between">
                    <span>{move || error_msg.get().unwrap_or_default()}</span>
                    <button type="button" class="text-xs font-bold" on:click=move |_| set_error_msg.set(None)>"✕"</button>
                </div>
            </Show>
            <Show when=move || success_msg.get().is_some()>
                <div class="rounded-xl border border-emerald-500/30 bg-emerald-500/10 px-4 py-3 text-xs text-emerald-600 dark:text-emerald-400 flex items-center justify-between">
                    <span>{move || success_msg.get().unwrap_or_default()}</span>
                    <button type="button" class="text-xs font-bold" on:click=move |_| set_success_msg.set(None)>"✕"</button>
                </div>
            </Show>

            // Option Dialog Drawer
            <Show when=move || selected_attr_for_opt.get().is_some()>
                <div class="bg-card rounded-2xl border-2 border-primary/30 p-5 shadow-sm space-y-4">
                    <div class="flex items-center justify-between border-b border-border pb-2.5">
                        <div class="flex items-center gap-2">
                            <span class="text-xs font-bold uppercase tracking-wider text-primary">
                                {if is_ru { "Добавление опции" } else { "Add Dictionary Option" }}
                            </span>
                            <span class="text-xs text-muted-foreground">
                                "→ " {move || selected_attr_for_opt.get().map(|a| a.label).unwrap_or_default()}
                            </span>
                        </div>
                        <button
                            type="button"
                            class="text-xs text-muted-foreground hover:text-foreground"
                            on:click=move |_| set_selected_attr_for_opt.set(None)
                        >
                            {if is_ru { "Отмена" } else { "Cancel" }}
                        </button>
                    </div>

                    <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
                        <div class="space-y-1.5">
                            <label class="text-xs font-medium text-foreground">
                                {if is_ru { "Название опции *" } else { "Option Label *" }}
                            </label>
                            <input
                                type="text"
                                placeholder=if is_ru { "Например, Красный, XL" } else { "e.g. Red, XL" }
                                prop:value=move || opt_label.get()
                                on:input=move |ev| {
                                    let v = event_target_value(&ev);
                                    let c = v.to_lowercase().trim().replace(|ch: char| !ch.is_alphanumeric(), "_");
                                    set_opt_label.set(v);
                                    set_opt_code.set(c);
                                }
                                class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground font-medium outline-none focus:border-primary"
                            />
                        </div>
                        <div class="space-y-1.5">
                            <label class="text-xs font-medium text-foreground">
                                {if is_ru { "Код опции *" } else { "Option Code *" }}
                            </label>
                            <input
                                type="text"
                                placeholder="red, xl"
                                prop:value=move || opt_code.get()
                                on:input=move |ev| set_opt_code.set(event_target_value(&ev))
                                class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground font-mono outline-none focus:border-primary"
                            />
                        </div>
                    </div>

                    <div class="flex justify-end pt-1">
                        <button
                            type="button"
                            class="h-9 px-4 rounded-xl bg-primary text-primary-foreground text-xs font-semibold hover:bg-primary/90 transition shadow-sm disabled:opacity-50"
                            disabled=move || is_busy.get()
                            on:click=move |_| {
                                let Some(attr) = selected_attr_for_opt.get_untracked() else { return; };
                                let label = opt_label.get_untracked().trim().to_string();
                                let code = opt_code.get_untracked().trim().to_string();
                                if label.is_empty() || code.is_empty() {
                                    set_error_msg.set(Some(if is_ru { "Заполните название и код опции" } else { "Fill in option label and code" }.to_string()));
                                    return;
                                }
                                set_is_busy.set(true);
                                set_error_msg.set(None);
                                set_success_msg.set(None);
                                let tok = token.get_untracked();
                                let ten = tenant.get_untracked();
                                let loc = locale_store.get_value().unwrap_or_default();
                                let draft = ProductAttributeOptionDraft {
                                    attribute_id: attr.id,
                                    code,
                                    label,
                                    position: 0,
                                };
                                spawn_local(async move {
                                    let Ok(bootstrap) = transport::fetch_bootstrap(tok.clone(), ten.clone()).await else {
                                        set_is_busy.set(false);
                                        set_error_msg.set(Some("Failed to authenticate bootstrap".to_string()));
                                        return;
                                    };
                                    let res = transport::create_product_attribute_option(tok, ten, bootstrap.current_tenant.id, bootstrap.me.id, loc, draft).await;
                                    set_is_busy.set(false);
                                    match res {
                                        Ok(_) => {
                                            set_success_msg.set(Some(if is_ru { "Опция словаря успешно добавлена" } else { "Option added successfully" }.to_string()));
                                            set_opt_label.set(String::new());
                                            set_opt_code.set(String::new());
                                            set_selected_attr_for_opt.set(None);
                                            set_refresh_nonce.update(|n| *n += 1);
                                        }
                                        Err(err) => set_error_msg.set(Some(err.to_string())),
                                    }
                                });
                            }
                        >
                            {if is_ru { "Сохранить опцию" } else { "Save Option" }}
                        </button>
                    </div>
                </div>
            </Show>

            // Main Content: View switcher based on active_tab
            {move || {
                if active_tab.get() == "schemas" {
                    // Schemas Tab View
                    view! {
                        <div class="grid grid-cols-1 lg:grid-cols-3 gap-6 items-start">
                            <div class="lg:col-span-2 bg-card rounded-2xl border border-border p-5 shadow-sm space-y-4">
                                <h2 class="text-sm font-semibold text-foreground border-b border-border pb-2.5">
                                    {if is_ru { "Шаблоны схем категорий" } else { "Category Schema Templates" }}
                                </h2>
                                <div class="rounded-xl border border-border overflow-hidden">
                                    <table class="w-full text-xs text-left">
                                        <thead class="bg-muted/50 text-[11px] font-semibold text-muted-foreground uppercase tracking-wider border-b border-border">
                                            <tr>
                                                <th class="px-4 py-2.5">{if is_ru { "Название схемы" } else { "Schema Name" }}</th>
                                                <th class="px-3 py-2.5">{if is_ru { "Код" } else { "Code" }}</th>
                                            </tr>
                                        </thead>
                                        <tbody class="divide-y divide-border/60">
                                            {move || {
                                                let schemas = schemas_resource.get().and_then(Result::ok).unwrap_or_default();
                                                if schemas.is_empty() {
                                                    return view! {
                                                        <tr>
                                                            <td colspan="2" class="px-4 py-8 text-center text-muted-foreground italic">
                                                                {if is_ru { "Схемы ещё не созданы" } else { "No attribute schemas defined" }}
                                                            </td>
                                                        </tr>
                                                    }.into_any();
                                                }

                                                schemas.into_iter().map(|s| {
                                                    view! {
                                                        <tr class="hover:bg-accent/40 transition-colors">
                                                            <td class="px-4 py-2.5 font-medium text-foreground">{s.name}</td>
                                                            <td class="px-3 py-2.5 font-mono text-[11px] text-muted-foreground">{s.code}</td>
                                                        </tr>
                                                    }
                                                }).collect_view().into_any()
                                            }}
                                        </tbody>
                                    </table>
                                </div>
                            </div>

                            // Add Schema Column
                            <div class="bg-card rounded-2xl border border-border p-5 shadow-sm space-y-4">
                                <h2 class="text-sm font-semibold text-foreground border-b border-border pb-2.5">
                                    {if is_ru { "Новая схема" } else { "New Schema" }}
                                </h2>
                                <div class="space-y-1.5">
                                    <label class="text-xs font-medium text-foreground">{if is_ru { "Название схемы *" } else { "Schema Name *" }}</label>
                                    <input
                                        type="text"
                                        placeholder=if is_ru { "Характеристики одежды" } else { "Apparel Specs" }
                                        prop:value=move || schema_name.get()
                                        on:input=move |ev| {
                                            let v = event_target_value(&ev);
                                            let c = v.to_lowercase().trim().replace(|ch: char| !ch.is_alphanumeric(), "_");
                                            set_schema_name.set(v);
                                            set_schema_code.set(c);
                                        }
                                        class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground font-medium outline-none focus:border-primary"
                                    />
                                </div>
                                <div class="space-y-1.5">
                                    <label class="text-xs font-medium text-foreground">{if is_ru { "Код схемы *" } else { "Schema Code *" }}</label>
                                    <input
                                        type="text"
                                        placeholder="apparel_specs"
                                        prop:value=move || schema_code.get()
                                        on:input=move |ev| set_schema_code.set(event_target_value(&ev))
                                        class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground font-mono outline-none focus:border-primary"
                                    />
                                </div>
                                <div class="space-y-1.5">
                                    <label class="text-xs font-medium text-foreground">{if is_ru { "Описание" } else { "Description" }}</label>
                                    <input
                                        type="text"
                                        placeholder=if is_ru { "Набор полей для одежды и обуви" } else { "Field bundle for apparel" }
                                        prop:value=move || schema_desc.get()
                                        on:input=move |ev| set_schema_desc.set(event_target_value(&ev))
                                        class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground outline-none focus:border-primary"
                                    />
                                </div>
                                <button
                                    type="button"
                                    class="w-full h-9 rounded-xl bg-primary text-primary-foreground text-xs font-semibold hover:bg-primary/90 transition shadow-sm disabled:opacity-50"
                                    disabled=move || is_busy.get()
                                    on:click=move |_| {
                                        let name = schema_name.get_untracked().trim().to_string();
                                        let code = schema_code.get_untracked().trim().to_string();
                                        if name.is_empty() || code.is_empty() {
                                            set_error_msg.set(Some(if is_ru { "Заполните название и код схемы" } else { "Fill in schema name and code" }.to_string()));
                                            return;
                                        }
                                        let desc_val = schema_desc.get_untracked().trim().to_string();
                                        let description = if desc_val.is_empty() { None } else { Some(desc_val) };
                                        set_is_busy.set(true);
                                        set_error_msg.set(None);
                                        set_success_msg.set(None);
                                        let tok = token.get_untracked();
                                        let ten = tenant.get_untracked();
                                        let loc = locale_store.get_value().unwrap_or_default();
                                        let draft = ProductAttributeSchemaDraft { code, name, description };
                                        spawn_local(async move {
                                            let Ok(bootstrap) = transport::fetch_bootstrap(tok.clone(), ten.clone()).await else {
                                                set_is_busy.set(false);
                                                set_error_msg.set(Some("Failed to authenticate bootstrap".to_string()));
                                                return;
                                            };
                                            let res = transport::create_attribute_schema(tok, ten, bootstrap.current_tenant.id, bootstrap.me.id, loc, draft).await;
                                            set_is_busy.set(false);
                                            match res {
                                                Ok(_) => {
                                                    set_success_msg.set(Some(if is_ru { "Схема атрибутов успешно создана" } else { "Schema created successfully" }.to_string()));
                                                    set_schema_name.set(String::new());
                                                    set_schema_code.set(String::new());
                                                    set_schema_desc.set(String::new());
                                                    set_refresh_nonce.update(|n| *n += 1);
                                                }
                                                Err(err) => set_error_msg.set(Some(err.to_string())),
                                            }
                                        });
                                    }
                                >
                                    {if is_ru { "Создать схему" } else { "Create Schema" }}
                                </button>
                            </div>
                        </div>
                    }.into_any()
                } else {
                    // Attributes Tab View
                    view! {
                        <div class="grid grid-cols-1 lg:grid-cols-3 gap-6 items-start">
                            // Left Column: Attributes Table (2 cols)
                            <div class="lg:col-span-2 bg-card rounded-2xl border border-border p-5 shadow-sm space-y-4">
                                <div class="flex items-center justify-between border-b border-border pb-2.5">
                                    <h2 class="text-sm font-semibold text-foreground">
                                        {if is_ru { "Список характеристик" } else { "Attributes Directory" }}
                                    </h2>
                                    <input
                                        type="text"
                                        placeholder=if is_ru { "Поиск по названию или коду..." } else { "Filter by name or code..." }
                                        prop:value=move || search_query.get()
                                        on:input=move |ev| set_search_query.set(event_target_value(&ev))
                                        class="text-xs rounded-lg border border-border bg-background px-2.5 py-1 text-foreground outline-none focus:border-primary w-48 font-normal"
                                    />
                                </div>

                                <div class="rounded-xl border border-border overflow-hidden">
                                    <table class="w-full text-xs text-left">
                                        <thead class="bg-muted/50 text-[11px] font-semibold text-muted-foreground uppercase tracking-wider border-b border-border">
                                            <tr>
                                                <th class="px-4 py-2.5">{if is_ru { "Название" } else { "Label" }}</th>
                                                <th class="px-3 py-2.5">{if is_ru { "Код" } else { "Code" }}</th>
                                                <th class="px-3 py-2.5">{if is_ru { "Тип" } else { "Type" }}</th>
                                                <th class="px-3 py-2.5 text-center">{if is_ru { "Фильтр" } else { "Filter" }}</th>
                                                <th class="px-3 py-2.5 text-right">{if is_ru { "Опции" } else { "Actions" }}</th>
                                            </tr>
                                        </thead>
                                        <tbody class="divide-y divide-border/60">
                                            {move || {
                                                let all_attrs = attributes_resource.get().and_then(Result::ok).unwrap_or_default();
                                                let q = search_query.get().to_lowercase().trim().to_string();
                                                let attrs: Vec<_> = all_attrs.into_iter().filter(|a| {
                                                    if q.is_empty() { return true; }
                                                    a.label.to_lowercase().contains(&q) || a.code.to_lowercase().contains(&q)
                                                }).collect();

                                                if attrs.is_empty() {
                                                    return view! {
                                                        <tr>
                                                            <td colspan="5" class="px-4 py-8 text-center text-muted-foreground italic">
                                                                {if is_ru { "Характеристики не найдены" } else { "No attributes found" }}
                                                            </td>
                                                        </tr>
                                                    }.into_any();
                                                }

                                                attrs.into_iter().map(|attr| {
                                                    let is_option = attr.value_type == "option" || attr.value_type == "multi_option";
                                                    let attr_clone = attr.clone();

                                                    view! {
                                                        <tr class="hover:bg-accent/40 transition-colors">
                                                            <td class="px-4 py-2.5 font-medium text-foreground">{attr.label}</td>
                                                            <td class="px-3 py-2.5 font-mono text-[11px] text-muted-foreground">{attr.code}</td>
                                                            <td class="px-3 py-2.5">
                                                                <span class="inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-mono bg-muted text-muted-foreground">
                                                                    {attr.value_type}
                                                                </span>
                                                            </td>
                                                            <td class="px-3 py-2.5 text-center">
                                                                {if attr.is_filterable {
                                                                    view! { <span class="text-emerald-500 font-bold">"✓"</span> }.into_any()
                                                                } else {
                                                                    view! { <span class="text-muted-foreground/40">"—"</span> }.into_any()
                                                                }}
                                                            </td>
                                                            <td class="px-3 py-2.5 text-right">
                                                                {if is_option {
                                                                    view! {
                                                                        <button
                                                                            type="button"
                                                                            class="h-6 px-2 rounded text-[11px] font-medium bg-primary/10 text-primary hover:bg-primary/20 transition"
                                                                            on:click=move |_| {
                                                                                set_selected_attr_for_opt.set(Some(attr_clone.clone()));
                                                                                set_opt_label.set(String::new());
                                                                                set_opt_code.set(String::new());
                                                                            }
                                                                        >
                                                                            "+ " {if is_ru { "Опция" } else { "Option" }}
                                                                        </button>
                                                                    }.into_any()
                                                                } else {
                                                                    view! { <span class="text-muted-foreground text-[10px]">"—"</span> }.into_any()
                                                                }}
                                                            </td>
                                                        </tr>
                                                    }
                                                }).collect_view().into_any()
                                            }}
                                        </tbody>
                                    </table>
                                </div>
                            </div>

                            // Right Column: Add Attribute Form (1 col)
                            <div class="bg-card rounded-2xl border border-border p-5 shadow-sm space-y-4">
                                <div class="flex items-center justify-between border-b border-border pb-2.5">
                                    <h2 class="text-sm font-semibold text-foreground">
                                        {if is_ru { "Новая характеристика" } else { "Add Attribute" }}
                                    </h2>
                                    <button
                                        type="button"
                                        class="text-xs text-muted-foreground hover:text-foreground"
                                        on:click=move |_| reset_attr_form()
                                    >
                                        {if is_ru { "Сбросить" } else { "Reset" }}
                                    </button>
                                </div>

                                <div class="space-y-1.5">
                                    <label class="text-xs font-medium text-foreground">
                                        {if is_ru { "Название характеристики *" } else { "Attribute Label *" }}
                                    </label>
                                    <input
                                        type="text"
                                        placeholder=if is_ru { "Например, Цвет, Размер, Материал" } else { "e.g. Color, Size, Material" }
                                        prop:value=move || attr_label.get()
                                        on:input=move |ev| on_label_input(event_target_value(&ev))
                                        class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground font-medium outline-none focus:border-primary"
                                    />
                                </div>

                                <div class="space-y-1.5">
                                    <label class="text-xs font-medium text-foreground">
                                        {if is_ru { "Код характеристики *" } else { "Code *" }}
                                    </label>
                                    <input
                                        type="text"
                                        placeholder="color, size, material"
                                        prop:value=move || attr_code.get()
                                        on:input=move |ev| set_attr_code.set(event_target_value(&ev))
                                        class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground font-mono outline-none focus:border-primary"
                                    />
                                </div>

                                <div class="space-y-1.5">
                                    <label class="text-xs font-medium text-foreground">
                                        {if is_ru { "Тип значения" } else { "Value Type" }}
                                    </label>
                                    <select
                                        prop:value=move || attr_value_type.get()
                                        on:change=move |ev| set_attr_value_type.set(event_target_value(&ev))
                                        class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground outline-none focus:border-primary"
                                    >
                                        <option value="text">{if is_ru { "Текст (Строка)" } else { "Text (Single line)" }}</option>
                                        <option value="rich_text">{if is_ru { "Форматированный текст" } else { "Rich Text" }}</option>
                                        <option value="integer">{if is_ru { "Целое число" } else { "Integer" }}</option>
                                        <option value="decimal">{if is_ru { "Дробное число" } else { "Decimal" }}</option>
                                        <option value="boolean">{if is_ru { "Логический (Да/Нет)" } else { "Boolean (Yes/No)" }}</option>
                                        <option value="option">{if is_ru { "Один выбор из словаря" } else { "Single Option (Select)" }}</option>
                                        <option value="multi_option">{if is_ru { "Множественный выбор" } else { "Multi Option (Checklist)" }}</option>
                                        <option value="date">{if is_ru { "Дата" } else { "Date" }}</option>
                                        <option value="datetime">{if is_ru { "Дата и время" } else { "Date & Time" }}</option>
                                        <option value="json">{if is_ru { "JSON структура" } else { "JSON (Structured)" }}</option>
                                    </select>
                                </div>

                                <div class="space-y-1.5">
                                    <label class="text-xs font-medium text-foreground">
                                        {if is_ru { "Подсказка для оператора" } else { "Help Text" }}
                                    </label>
                                    <input
                                        type="text"
                                        placeholder=if is_ru { "Краткая подсказка..." } else { "Optional tooltip..." }
                                        prop:value=move || attr_help_text.get()
                                        on:input=move |ev| set_attr_help_text.set(event_target_value(&ev))
                                        class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground outline-none focus:border-primary"
                                    />
                                </div>

                                <div class="rounded-xl border border-border p-3 space-y-2.5 bg-muted/20">
                                    <label class="flex items-center gap-2 cursor-pointer">
                                        <input
                                            type="checkbox"
                                            prop:checked=move || attr_is_filterable.get()
                                            on:change=move |ev| set_attr_is_filterable.set(event_target_checked(&ev))
                                            class="rounded border-border text-primary"
                                        />
                                        <span class="text-xs text-foreground font-medium">
                                            {if is_ru { "Фильтрация в каталоге" } else { "Filterable (Facets)" }}
                                        </span>
                                    </label>

                                    <label class="flex items-center gap-2 cursor-pointer">
                                        <input
                                            type="checkbox"
                                            prop:checked=move || attr_is_localized.get()
                                            on:change=move |ev| set_attr_is_localized.set(event_target_checked(&ev))
                                            class="rounded border-border text-primary"
                                        />
                                        <span class="text-xs text-foreground font-medium">
                                            {if is_ru { "Локализованные значения" } else { "Multilingual values" }}
                                        </span>
                                    </label>

                                    <label class="flex items-center gap-2 cursor-pointer">
                                        <input
                                            type="checkbox"
                                            prop:checked=move || attr_is_searchable.get()
                                            on:change=move |ev| set_attr_is_searchable.set(event_target_checked(&ev))
                                            class="rounded border-border text-primary"
                                        />
                                        <span class="text-xs text-foreground font-medium">
                                            {if is_ru { "Индексация в поиске" } else { "Searchable in index" }}
                                        </span>
                                    </label>

                                    <label class="flex items-center gap-2 cursor-pointer">
                                        <input
                                            type="checkbox"
                                            prop:checked=move || attr_show_on_storefront.get()
                                            on:change=move |ev| set_attr_show_on_storefront.set(event_target_checked(&ev))
                                            class="rounded border-border text-primary"
                                        />
                                        <span class="text-xs text-foreground font-medium">
                                            {if is_ru { "Показывать на витрине" } else { "Show on storefront specs" }}
                                        </span>
                                    </label>
                                </div>

                                <div class="pt-2">
                                    <button
                                        type="button"
                                        class="w-full h-9 rounded-xl bg-primary text-primary-foreground text-xs font-semibold hover:bg-primary/90 transition shadow-sm disabled:opacity-50"
                                        disabled=move || is_busy.get()
                                        on:click=move |_| {
                                            let label = attr_label.get_untracked().trim().to_string();
                                            if label.is_empty() {
                                                set_error_msg.set(Some(if is_ru { "Введите название характеристики" } else { "Enter attribute label" }.to_string()));
                                                return;
                                            }
                                            let code = attr_code.get_untracked().trim().to_string();
                                            if code.is_empty() {
                                                set_error_msg.set(Some(if is_ru { "Введите код характеристики" } else { "Enter attribute code" }.to_string()));
                                                return;
                                            }
                                            let help_text_val = attr_help_text.get_untracked().trim().to_string();
                                            let help_text = if help_text_val.is_empty() { None } else { Some(help_text_val) };
                                            set_is_busy.set(true);
                                            set_error_msg.set(None);
                                            set_success_msg.set(None);
                                            let tok = token.get_untracked();
                                            let ten = tenant.get_untracked();
                                            let loc = locale_store.get_value().unwrap_or_default();
                                            let draft = ProductAttributeDraft {
                                                code,
                                                value_type: attr_value_type.get_untracked(),
                                                label,
                                                help_text,
                                                is_localized: attr_is_localized.get_untracked(),
                                                is_filterable: attr_is_filterable.get_untracked(),
                                                is_searchable: attr_is_searchable.get_untracked(),
                                                is_sortable: attr_is_sortable.get_untracked(),
                                                show_on_storefront: attr_show_on_storefront.get_untracked(),
                                            };
                                            spawn_local(async move {
                                                let Ok(bootstrap) = transport::fetch_bootstrap(tok.clone(), ten.clone()).await else {
                                                    set_is_busy.set(false);
                                                    set_error_msg.set(Some("Failed to authenticate bootstrap".to_string()));
                                                    return;
                                                };
                                                let res = transport::create_product_attribute(tok, ten, bootstrap.current_tenant.id, bootstrap.me.id, loc, draft).await;
                                                set_is_busy.set(false);
                                                match res {
                                                    Ok(_) => {
                                                        set_success_msg.set(Some(if is_ru { "Характеристика успешно создана" } else { "Attribute created successfully" }.to_string()));
                                                        reset_attr_form();
                                                        set_refresh_nonce.update(|n| *n += 1);
                                                    }
                                                    Err(err) => set_error_msg.set(Some(err.to_string())),
                                                }
                                            });
                                        }
                                    >
                                        {move || if is_busy.get() {
                                            if is_ru { "Сохранение..." } else { "Saving..." }
                                        } else {
                                            if is_ru { "Создать характеристику" } else { "Create Attribute" }
                                        }}
                                    </button>
                                </div>
                            </div>
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}
