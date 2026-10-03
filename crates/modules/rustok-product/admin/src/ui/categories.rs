use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use rustok_ui_core::UiRouteContext;

use crate::core::{build_category_tree, flatten_category_tree, slugify};
use crate::model::{CatalogCategoryDraft, CatalogCategorySummary};
use crate::transport;

#[component]
pub fn CategoriesPage() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let is_ru = locale.as_deref() == Some("ru");
    let base_route = route_context.admin_module_route_base("product");
    let token = use_token();
    let tenant = use_tenant();

    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (is_busy, set_is_busy) = signal(false);
    let (error_msg, set_error_msg) = signal(Option::<String>::None);
    let (success_msg, set_success_msg) = signal(Option::<String>::None);

    // Form fields for creating/editing category
    let (editing_id, set_editing_id) = signal(Option::<String>::None);
    let (cat_name, set_cat_name) = signal(String::new());
    let (cat_slug, set_cat_slug) = signal(String::new());
    let (cat_code, set_cat_code) = signal(String::new());
    let (cat_parent_id, set_cat_parent_id) = signal(String::new());
    let (cat_kind, set_cat_kind) = signal("physical".to_string());
    let (cat_description, set_cat_description) = signal(String::new());

    // Load categories
    let res_locale = locale.clone();
    let categories_resource = LocalResource::new(move || {
        let tok = token.get();
        let ten = tenant.get();
        let loc = res_locale.clone().unwrap_or_default();
        let _ = refresh_nonce.get();
        async move {
            let bootstrap = transport::fetch_bootstrap(tok.clone(), ten.clone())
                .await
                .map_err(|e| e.to_string())?;
            let res = transport::fetch_catalog_categories(
                tok,
                ten,
                bootstrap.current_tenant.id,
                loc,
            )
            .await
            .map_err(|e| e.to_string())?;
            Ok::<Vec<CatalogCategorySummary>, String>(res.items)
        }
    });

    // Auto-generate slug and code from name
    let on_name_input = move |val: String| {
        let slug = slugify(&val);
        set_cat_name.set(val);
        if editing_id.get_untracked().is_none() {
            set_cat_slug.set(slug.clone());
            set_cat_code.set(slug.to_uppercase().replace('-', "_"));
        }
    };

    let reset_form = move || {
        set_editing_id.set(None);
        set_cat_name.set(String::new());
        set_cat_slug.set(String::new());
        set_cat_code.set(String::new());
        set_cat_parent_id.set(String::new());
        set_cat_kind.set("physical".to_string());
        set_cat_description.set(String::new());
        set_error_msg.set(None);
        set_success_msg.set(None);
    };

    // Save category
    let on_save_category = move |_| {
        let name = cat_name.get_untracked().trim().to_string();
        if name.is_empty() {
            set_error_msg.set(Some(if is_ru { "Введите название категории" } else { "Enter category name" }.to_string()));
            return;
        }

        let mut slug = cat_slug.get_untracked().trim().to_string();
        if slug.is_empty() {
            slug = slugify(&name);
        }

        let mut code = cat_code.get_untracked().trim().to_string();
        if code.is_empty() {
            code = slug.to_uppercase().replace('-', "_");
        }

        let parent_id_val = cat_parent_id.get_untracked().trim().to_string();
        let parent_id = if parent_id_val.is_empty() { None } else { Some(parent_id_val) };

        let kind = cat_kind.get_untracked();
        let desc = cat_description.get_untracked().trim().to_string();
        let description = if desc.is_empty() { None } else { Some(desc) };

        set_is_busy.set(true);
        set_error_msg.set(None);
        set_success_msg.set(None);

        let tok = token.get_untracked();
        let ten = tenant.get_untracked();
        let loc = locale.clone().unwrap_or_default();

        let draft = CatalogCategoryDraft {
            parent_id,
            code,
            slug,
            kind,
            name,
            description,
        };

        spawn_local(async move {
            let Ok(bootstrap) = transport::fetch_bootstrap(tok.clone(), ten.clone()).await else {
                set_is_busy.set(false);
                set_error_msg.set(Some("Failed to authenticate bootstrap".to_string()));
                return;
            };

            let res = transport::create_catalog_category(
                tok,
                ten,
                bootstrap.current_tenant.id,
                bootstrap.me.id,
                loc,
                draft,
            )
            .await;

            set_is_busy.set(false);
            match res {
                Ok(_) => {
                    set_success_msg.set(Some(if is_ru { "Категория успешно сохранена" } else { "Category saved successfully" }.to_string()));
                    reset_form();
                    set_refresh_nonce.update(|n| *n += 1);
                }
                Err(err) => set_error_msg.set(Some(err.to_string())),
            }
        });
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
                            <span>{if is_ru { "Категории каталога" } else { "Product Categories" }}</span>
                            <span class="text-xs font-normal px-2 py-0.5 rounded-full bg-primary/10 text-primary border border-primary/20">
                                {move || categories_resource.get().and_then(Result::ok).map(|c| c.len()).unwrap_or(0)}
                            </span>
                        </h1>
                        <p class="text-xs text-muted-foreground mt-0.5">
                            {if is_ru {
                                "Иерархическая структура каталога, физические и виртуальные категории"
                            } else {
                                "Hierarchical catalog taxonomy with physical and virtual category modes"
                            }}
                        </p>
                    </div>
                </div>

                <div class="flex items-center gap-2">
                    <button
                        type="button"
                        class="h-9 px-3 rounded-xl border border-border bg-background text-xs font-medium text-foreground hover:bg-accent transition"
                        on:click=move |_| set_refresh_nonce.update(|n| *n += 1)
                    >
                        "↻ " {if is_ru { "Обновить" } else { "Refresh" }}
                    </button>
                    <button
                        type="button"
                        class="h-9 px-3.5 rounded-xl bg-primary text-primary-foreground text-xs font-semibold hover:bg-primary/90 transition shadow-sm"
                        on:click=move |_| reset_form()
                    >
                        "+ " {if is_ru { "Новая категория" } else { "New Category" }}
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

            // Main Two-Column Layout
            <div class="grid grid-cols-1 lg:grid-cols-3 gap-6 items-start">
                // Left Column: Hierarchical Category Tree (2 cols)
                <div class="lg:col-span-2 bg-card rounded-2xl border border-border p-5 shadow-sm space-y-4">
                    <h2 class="text-sm font-semibold text-foreground border-b border-border pb-2.5">
                        {if is_ru { "Дерево категорий" } else { "Category Taxonomy Tree" }}
                    </h2>

                    <div class="rounded-xl border border-border overflow-hidden">
                        <table class="w-full text-xs text-left">
                            <thead class="bg-muted/50 text-[11px] font-semibold text-muted-foreground uppercase tracking-wider border-b border-border">
                                <tr>
                                    <th class="px-4 py-2.5">{if is_ru { "Категория" } else { "Category" }}</th>
                                    <th class="px-3 py-2.5">{if is_ru { "Код" } else { "Code" }}</th>
                                    <th class="px-3 py-2.5">{if is_ru { "Тип" } else { "Kind" }}</th>
                                    <th class="px-3 py-2.5 text-right">{if is_ru { "Действия" } else { "Actions" }}</th>
                                </tr>
                            </thead>
                            <tbody class="divide-y divide-border/60">
                                {move || {
                                    let cats = categories_resource.get().and_then(Result::ok).unwrap_or_default();
                                    if cats.is_empty() {
                                        return view! {
                                            <tr>
                                                <td colspan="4" class="px-4 py-8 text-center text-muted-foreground italic">
                                                    {if is_ru { "Категории ещё не созданы" } else { "No categories found" }}
                                                </td>
                                            </tr>
                                        }.into_any();
                                    }

                                    let tree = build_category_tree(&cats);
                                    let flattened = flatten_category_tree(&tree);

                                    flattened.into_iter().map(|(cat, depth)| {
                                        let cat_id = cat.id.clone();
                                        let cat_id_sub = cat.id.clone();
                                        let cat_name_val = cat.name.clone();
                                        let cat_slug_val = cat.slug.clone();
                                        let cat_code_val = cat.code.clone();
                                        let cat_kind_val = cat.kind.clone();
                                        let is_virt = cat.kind.to_lowercase() == "virtual";

                                        view! {
                                            <tr class="hover:bg-accent/40 transition-colors">
                                                <td class="px-4 py-2.5">
                                                    <div class="flex items-center gap-1.5" style=format!("padding-left: {}px", depth * 20)>
                                                        <span class="text-muted-foreground/60 text-xs">
                                                            {if depth == 0 { "📁" } else { "↳ 📄" }}
                                                        </span>
                                                        <span class="font-medium text-foreground">{cat.name}</span>
                                                        <span class="text-[10px] font-mono text-muted-foreground/70">
                                                            {format!("({})", cat.slug)}
                                                        </span>
                                                    </div>
                                                </td>
                                                <td class="px-3 py-2.5 font-mono text-[11px] text-muted-foreground">
                                                    {cat.code}
                                                </td>
                                                <td class="px-3 py-2.5">
                                                    <span class=if is_virt {
                                                        "inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-semibold bg-purple-500/10 text-purple-600 dark:text-purple-400 border border-purple-500/20"
                                                    } else {
                                                        "inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-semibold bg-blue-500/10 text-blue-600 dark:text-blue-400 border border-blue-500/20"
                                                    }>
                                                        {if is_virt { "Virtual" } else { "Physical" }}
                                                    </span>
                                                </td>
                                                <td class="px-3 py-2.5 text-right">
                                                    <div class="flex items-center justify-end gap-1.5">
                                                        <button
                                                            type="button"
                                                            class="h-6 px-2 rounded text-[11px] font-medium bg-secondary text-secondary-foreground hover:bg-accent"
                                                            title=if is_ru { "Добавить подкатегорию" } else { "Add subcategory" }
                                                            on:click=move |_| {
                                                                reset_form();
                                                                set_cat_parent_id.set(cat_id_sub.clone());
                                                            }
                                                        >
                                                            "+ ↳"
                                                        </button>
                                                        <button
                                                            type="button"
                                                            class="h-6 px-2 rounded text-[11px] font-medium border border-border text-foreground hover:bg-accent"
                                                            title=if is_ru { "Создать категорию на основе этой" } else { "Create category from this template" }
                                                            on:click=move |_| {
                                                                set_editing_id.set(Some(cat_id.clone()));
                                                                set_cat_name.set(format!("{} (copy)", cat_name_val));
                                                                set_cat_slug.set(format!("{}-copy", cat_slug_val));
                                                                set_cat_code.set(format!("{}_COPY", cat_code_val));
                                                                set_cat_kind.set(cat_kind_val.clone());
                                                            }
                                                        >
                                                            {if is_ru { "Копия" } else { "Clone" }}
                                                        </button>
                                                    </div>
                                                </td>
                                            </tr>
                                        }
                                    }).collect_view().into_any()
                                }}
                            </tbody>
                        </table>
                    </div>
                </div>

                // Right Column: Add / Edit Form (1 col)
                <div class="bg-card rounded-2xl border border-border p-5 shadow-sm space-y-4">
                    <div class="flex items-center justify-between border-b border-border pb-2.5">
                        <h2 class="text-sm font-semibold text-foreground">
                            {move || if editing_id.get().is_some() {
                                if is_ru { "Категория на основе шаблона" } else { "Category from Template" }
                            } else {
                                if is_ru { "Новая категория" } else { "Add Category" }
                            }}
                        </h2>
                        <Show when=move || editing_id.get().is_some()>
                            <button
                                type="button"
                                class="text-xs text-muted-foreground hover:text-foreground"
                                on:click=move |_| reset_form()
                            >
                                {if is_ru { "Сбросить" } else { "Cancel" }}
                            </button>
                        </Show>
                    </div>

                    <div class="space-y-1.5">
                        <label class="text-xs font-medium text-foreground">
                            {if is_ru { "Название категории *" } else { "Category Name *" }}
                        </label>
                        <input
                            type="text"
                            placeholder=if is_ru { "Например, Одежда" } else { "e.g. Apparel" }
                            prop:value=move || cat_name.get()
                            on:input=move |ev| on_name_input(event_target_value(&ev))
                            class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground font-medium outline-none focus:border-primary"
                        />
                    </div>

                    <div class="space-y-1.5">
                        <label class="text-xs font-medium text-foreground">
                            {if is_ru { "Slug (URL идентификатор) *" } else { "Slug *" }}
                        </label>
                        <input
                            type="text"
                            placeholder="apparel"
                            prop:value=move || cat_slug.get()
                            on:input=move |ev| set_cat_slug.set(event_target_value(&ev))
                            class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground font-mono outline-none focus:border-primary"
                        />
                    </div>

                    <div class="space-y-1.5">
                        <label class="text-xs font-medium text-foreground">
                            {if is_ru { "Код категории *" } else { "Code *" }}
                        </label>
                        <input
                            type="text"
                            placeholder="APPAREL"
                            prop:value=move || cat_code.get()
                            on:input=move |ev| set_cat_code.set(event_target_value(&ev))
                            class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground font-mono outline-none focus:border-primary"
                        />
                    </div>

                    <div class="space-y-1.5">
                        <label class="text-xs font-medium text-foreground">
                            {if is_ru { "Родительская категория" } else { "Parent Category" }}
                        </label>
                        <select
                            prop:value=move || cat_parent_id.get()
                            on:change=move |ev| set_cat_parent_id.set(event_target_value(&ev))
                            class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground outline-none focus:border-primary"
                        >
                            <option value="">{if is_ru { "— Верхний уровень (Корень) —" } else { "— Root Level (No Parent) —" }}</option>
                            {move || {
                                let cats = categories_resource.get().and_then(Result::ok).unwrap_or_default();
                                let tree = build_category_tree(&cats);
                                let flattened = flatten_category_tree(&tree);
                                flattened.into_iter().map(|(c, depth)| {
                                    let indent = "— ".repeat(depth);
                                    view! {
                                        <option value=c.id.clone()>
                                            {format!("{}{}", indent, c.name)}
                                        </option>
                                    }
                                }).collect_view()
                            }}
                        </select>
                    </div>

                    <div class="space-y-1.5">
                        <label class="text-xs font-medium text-foreground">
                            {if is_ru { "Тип категории" } else { "Category Kind" }}
                        </label>
                        <select
                            prop:value=move || cat_kind.get()
                            on:change=move |ev| set_cat_kind.set(event_target_value(&ev))
                            class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground outline-none focus:border-primary"
                        >
                            <option value="physical">{if is_ru { "Физическая (Физические товары)" } else { "Physical (Physical Goods)" }}</option>
                            <option value="virtual">{if is_ru { "Виртуальная (Цифровые услуги / Смарт-правила)" } else { "Virtual (Digital / Smart Rules)" }}</option>
                        </select>
                    </div>

                    <div class="space-y-1.5">
                        <label class="text-xs font-medium text-foreground">
                            {if is_ru { "Описание" } else { "Description" }}
                        </label>
                        <textarea
                            rows="3"
                            placeholder=if is_ru { "Описание категории для каталога и SEO..." } else { "Category description for catalog and SEO..." }
                            prop:value=move || cat_description.get()
                            on:input=move |ev| set_cat_description.set(event_target_value(&ev))
                            class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground outline-none focus:border-primary resize-y"
                        />
                    </div>

                    <div class="pt-2">
                        <button
                            type="button"
                            class="w-full h-9 rounded-xl bg-primary text-primary-foreground text-xs font-semibold hover:bg-primary/90 transition shadow-sm disabled:opacity-50"
                            disabled=move || is_busy.get()
                            on:click=on_save_category
                        >
                            {move || if is_busy.get() {
                                if is_ru { "Сохранение..." } else { "Saving..." }
                            } else {
                                if is_ru { "Сохранить категорию" } else { "Save Category" }
                            }}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    }
}
