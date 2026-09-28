use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_router::hooks::use_navigate;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    build_product_image_view_models, build_variant_row_view_models, slugify,
    ProductKind,
};
use crate::model::{
    CatalogCategorySummary, ProductDetail, ProductDraft, ProductImageDraft,
};
use crate::transport;

#[component]
pub fn ProductEditorPage(
    #[prop(optional)] is_new: bool,
    #[prop(optional)] product_id: Option<String>,
    #[prop(optional)] initial_type: Option<String>,
) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let is_ru = locale.as_deref() == Some("ru");
    let base_route = route_context.module_route_base("product");
    let token = use_token();
    let tenant = use_tenant();

    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (is_busy, set_is_busy) = signal(false);
    let (error_msg, set_error_msg) = signal(Option::<String>::None);
    let (success_msg, set_success_msg) = signal(Option::<String>::None);

    // Initial product type from prop or query param
    let start_type = initial_type
        .as_deref()
        .or_else(|| route_context.query.get("type").map(String::as_str))
        .map(ProductKind::parse)
        .unwrap_or(ProductKind::Simple);

    let (active_kind, set_active_kind) = signal(start_type);

    // Form fields
    let (title, set_title) = signal(String::new());
    let (handle, set_handle) = signal(String::new());
    let (description, set_description) = signal(String::new());
    let (status, set_status) = signal("DRAFT".to_string());
    let (vendor, set_vendor) = signal(String::new());
    let (seller_id, set_seller_id) = signal(String::new());
    let (primary_category_id, set_primary_category_id) = signal(String::new());
    let (shipping_profile_slug, set_shipping_profile_slug) = signal(String::new());
    let (tag_input, set_tag_input) = signal(String::new());
    let (tags, set_tags) = signal(Vec::<String>::new());

    // Pricing
    let (currency_code, set_currency_code) = signal("USD".to_string());
    let (amount, set_amount) = signal("0.00".to_string());
    let (compare_at_amount, set_compare_at_amount) = signal(String::new());

    // Simple Product Specific
    let (sku, set_sku) = signal(String::new());
    let (barcode, set_barcode) = signal(String::new());
    let (inventory_quantity, set_inventory_quantity) = signal(0_i32);
    let (inventory_policy, set_inventory_policy) = signal("DENY".to_string());

    // Variable Product Specific
    let (variant_axes_str, set_variant_axes_str) = signal("Size, Color".to_string());

    // Digital Product Specific
    let (digital_file_url, set_digital_file_url) = signal(String::new());
    let (digital_download_limit, set_digital_download_limit) = signal("10".to_string());
    let (digital_expiry_days, set_digital_expiry_days) = signal("365".to_string());

    // Bundle Product Specific
    let (bundle_components_str, set_bundle_components_str) = signal(String::new());

    // SEO
    let (meta_title, set_meta_title) = signal(String::new());
    let (meta_description, set_meta_description) = signal(String::new());

    // Image addition form
    let (new_image_url, set_new_image_url) = signal(String::new());
    let (new_image_alt, set_new_image_alt) = signal(String::new());

    // Product state in edit mode
    let (loaded_product, set_loaded_product) = signal(Option::<ProductDetail>::None);

    // Categories list resource
    let cat_locale = locale.clone();
    let categories_resource = LocalResource::new(move || {
        let tok = token.get();
        let ten = tenant.get();
        let loc = cat_locale.clone().unwrap_or_default();
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

    // Shipping profiles resource
    let shipping_profiles_resource = LocalResource::new(move || {
        let tok = token.get();
        let ten = tenant.get();
        async move {
            let bootstrap = transport::fetch_bootstrap(tok.clone(), ten.clone())
                .await
                .map_err(|e| e.to_string())?;
            let res = transport::fetch_shipping_profiles(
                tok,
                ten,
                bootstrap.current_tenant.id,
            )
            .await
            .map_err(|e| e.to_string())?;
            Ok::<Vec<crate::model::ShippingProfile>, String>(res.items)
        }
    });

    // Load existing product if in Edit mode
    let target_id = product_id.clone();
    let eff_locale = locale.clone();
    Effect::new(move |_| {
        let _ = refresh_nonce.get();
        let Some(pid) = target_id.clone() else {
            return;
        };
        let tok = token.get_untracked();
        let ten = tenant.get_untracked();
        let loc = eff_locale.clone();

        spawn_local(async move {
            let Ok(bootstrap) = transport::fetch_bootstrap(tok.clone(), ten.clone()).await else {
                return;
            };
            if let Ok(Some(detail)) = transport::fetch_product(
                tok,
                ten,
                bootstrap.current_tenant.id,
                pid,
                loc,
            )
            .await
            {
                // Populate signals from loaded product detail
                if let Some(tr) = detail.translations.first() {
                    set_title.set(tr.title.clone());
                    set_handle.set(tr.handle.clone());
                    set_description.set(tr.description.clone().unwrap_or_default());
                    set_meta_title.set(tr.meta_title.clone().unwrap_or_default());
                    set_meta_description.set(tr.meta_description.clone().unwrap_or_default());
                }
                set_status.set(detail.status.clone());
                set_vendor.set(detail.vendor.clone().unwrap_or_default());
                set_seller_id.set(detail.seller_id.clone().unwrap_or_default());
                set_primary_category_id.set(detail.primary_category_id.clone().unwrap_or_default());
                set_shipping_profile_slug.set(detail.shipping_profile_slug.clone().unwrap_or_default());
                set_tags.set(detail.tags.clone());

                if let Some(ref kind_str) = detail.product_type {
                    set_active_kind.set(ProductKind::parse(kind_str));
                }

                // First variant for simple product pricing & stock
                if let Some(variant) = detail.variants.first() {
                    set_sku.set(variant.sku.clone().unwrap_or_default());
                    set_barcode.set(variant.barcode.clone().unwrap_or_default());
                    set_inventory_quantity.set(variant.inventory_quantity);
                    set_inventory_policy.set(variant.inventory_policy.clone());
                    if let Some(price) = variant.prices.first() {
                        set_currency_code.set(price.currency_code.clone());
                        set_amount.set(price.amount.clone());
                        set_compare_at_amount.set(price.compare_at_amount.clone().unwrap_or_default());
                    }
                }

                set_loaded_product.set(Some(detail));
            }
        });
    });

    // Auto-generate slug from title
    let on_generate_slug = move |_| {
        let current_title = title.get_untracked();
        if !current_title.is_empty() {
            set_handle.set(slugify(&current_title));
        }
    };

    // Add tag
    let on_add_tag = move |_| {
        let new_tag = tag_input.get_untracked().trim().to_string();
        if !new_tag.is_empty() {
            set_tags.update(|t| {
                if !t.contains(&new_tag) {
                    t.push(new_tag);
                }
            });
            set_tag_input.set(String::new());
        }
    };

    // Remove tag
    let on_remove_tag = move |tag_to_remove: String| {
        set_tags.update(|t| t.retain(|tag| tag != &tag_to_remove));
    };

    // Image actions
    let on_add_image = {
        let edit_id = product_id.clone();
        let base_token = token;
        let base_tenant = tenant;
        let base_locale = locale.clone();
        move || {
            let url = new_image_url.get_untracked().trim().to_string();
            if url.is_empty() {
                return;
            }
            let alt = new_image_alt.get_untracked().trim().to_string();
            let Some(pid) = edit_id.clone() else {
                return;
            };

            set_is_busy.set(true);
            let tok = base_token.get_untracked();
            let ten = base_tenant.get_untracked();
            let loc = base_locale.clone();

            spawn_local(async move {
                let Ok(bootstrap) = transport::fetch_bootstrap(tok.clone(), ten.clone()).await else {
                    set_is_busy.set(false);
                    return;
                };

                let media_id = if uuid::Uuid::parse_str(&url).is_ok() {
                    url
                } else {
                    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, url.as_bytes()).to_string()
                };

                let draft = ProductImageDraft {
                    media_id,
                    alt_text: if alt.is_empty() { None } else { Some(alt) },
                    position: None,
                    locale: loc.clone(),
                };

                let _ = transport::add_product_image(
                    tok,
                    ten,
                    bootstrap.current_tenant.id,
                    bootstrap.me.id,
                    pid,
                    draft,
                )
                .await;

                set_new_image_url.set(String::new());
                set_new_image_alt.set(String::new());
                set_is_busy.set(false);
                set_refresh_nonce.update(|n| *n += 1);
            });
        }
    };

    let on_delete_image = {
        let edit_id = product_id.clone();
        let base_token = token;
        let base_tenant = tenant;
        move |image_id: String| {
            let Some(pid) = edit_id.clone() else {
                return;
            };
            set_is_busy.set(true);
            let tok = base_token.get_untracked();
            let ten = base_tenant.get_untracked();

            spawn_local(async move {
                let Ok(bootstrap) = transport::fetch_bootstrap(tok.clone(), ten.clone()).await else {
                    set_is_busy.set(false);
                    return;
                };

                let _ = transport::delete_product_image(
                    tok,
                    ten,
                    bootstrap.current_tenant.id,
                    bootstrap.me.id,
                    pid,
                    image_id,
                )
                .await;

                set_is_busy.set(false);
                set_refresh_nonce.update(|n| *n += 1);
            });
        }
    };

    // Save product (Create or Update)
    let is_editing = !is_new && product_id.is_some();
    let current_edit_id = product_id.clone();
    let base_route_for_save = base_route.clone();
    let navigate = use_navigate();
    let save_locale = locale.clone();

    let save_product = move |target_status: Option<&'static str>| {
        let t_val = title.get_untracked().trim().to_string();
        if t_val.is_empty() {
            set_error_msg.set(Some(if is_ru { "Введите название товара" } else { "Please enter product title" }.to_string()));
            return;
        }

        let mut h_val = handle.get_untracked().trim().to_string();
        if h_val.is_empty() {
            h_val = slugify(&t_val);
        }

        let current_status_val = target_status
            .map(|s| s.to_string())
            .unwrap_or_else(|| status.get_untracked());

        set_is_busy.set(true);
        set_error_msg.set(None);
        set_success_msg.set(None);

        let tok = token.get_untracked();
        let ten = tenant.get_untracked();
        let loc = save_locale.clone();
        let base_route = base_route_for_save.clone();
        let edit_id_opt = current_edit_id.clone();
        let kind = active_kind.get_untracked();
        let nav = navigate.clone();

        let mt = meta_title.get_untracked().trim().to_string();
        let md = meta_description.get_untracked().trim().to_string();
        let tg = tags.get_untracked();
        let target_status = current_status_val.clone();

        let draft = ProductDraft {
            locale: loc.unwrap_or_else(|| "en".to_string()),
            title: t_val,
            handle: h_val,
            description: description.get_untracked(),
            seller_id: seller_id.get_untracked(),
            vendor: vendor.get_untracked(),
            product_type: kind.as_str().to_string(),
            shipping_profile_slug: Some(shipping_profile_slug.get_untracked()).filter(|s| !s.is_empty()),
            primary_category_id: Some(primary_category_id.get_untracked()).filter(|s| !s.is_empty()),
            sku: sku.get_untracked(),
            barcode: barcode.get_untracked(),
            currency_code: currency_code.get_untracked(),
            amount: amount.get_untracked(),
            compare_at_amount: compare_at_amount.get_untracked(),
            inventory_quantity: inventory_quantity.get_untracked(),
            publish_now: target_status.to_uppercase() == "ACTIVE",
            status: Some(target_status.clone()),
            meta_title: if mt.is_empty() { None } else { Some(mt) },
            meta_description: if md.is_empty() { None } else { Some(md) },
            tags: tg,
        };

        spawn_local(async move {
            let Ok(bootstrap) = transport::fetch_bootstrap(tok.clone(), ten.clone()).await else {
                set_is_busy.set(false);
                set_error_msg.set(Some("Failed to authenticate bootstrap".to_string()));
                return;
            };

            if let Some(pid) = edit_id_opt {
                let status_to_change = target_status.clone();
                let res = transport::update_product(
                    tok.clone(),
                    ten.clone(),
                    bootstrap.current_tenant.id.clone(),
                    bootstrap.me.id.clone(),
                    pid.clone(),
                    draft,
                )
                .await;

                // Also update status to ensure state transition is persisted
                let _ = transport::change_product_status(
                    tok.clone(),
                    ten.clone(),
                    bootstrap.current_tenant.id.clone(),
                    bootstrap.me.id.clone(),
                    pid.clone(),
                    &status_to_change,
                )
                .await;

                // Update default variant pricing, stock, SKU, and barcode
                if let Some(detail) = loaded_product.get_untracked() {
                    if let Some(variant) = detail.variants.first() {
                        let cur_sku = sku.get_untracked();
                        let cur_bc = barcode.get_untracked();
                        let cur_qty = inventory_quantity.get_untracked();
                        let cur_cur = currency_code.get_untracked();
                        let cur_amt = amount.get_untracked();
                        let cur_comp = compare_at_amount.get_untracked();

                        let v_draft = crate::model::VariantDraft {
                            sku: if cur_sku.trim().is_empty() { None } else { Some(cur_sku) },
                            barcode: if cur_bc.trim().is_empty() { None } else { Some(cur_bc) },
                            shipping_profile_slug: None,
                            axis_values: Vec::new(),
                            prices: vec![crate::model::VariantPriceDraft {
                                currency_code: if cur_cur.trim().is_empty() { "USD".to_string() } else { cur_cur },
                                amount: if cur_amt.trim().is_empty() { "0.00".to_string() } else { cur_amt },
                                compare_at_amount: if cur_comp.trim().is_empty() { None } else { Some(cur_comp) },
                            }],
                            inventory_quantity: Some(cur_qty),
                            inventory_policy: Some(inventory_policy.get_untracked()),
                        };

                        let _ = transport::update_product_variant(
                            tok,
                            ten,
                            bootstrap.current_tenant.id,
                            bootstrap.me.id,
                            variant.id.clone(),
                            v_draft,
                        )
                        .await;
                    }
                }

                set_is_busy.set(false);
                match res {
                    Ok(_) => {
                        set_success_msg.set(Some(if is_ru { "Товар успешно сохранён" } else { "Product saved successfully" }.to_string()));
                        set_refresh_nonce.update(|n| *n += 1);
                    }
                    Err(err) => set_error_msg.set(Some(err.to_string())),
                }
            } else {
                let res = transport::create_product(
                    tok,
                    ten,
                    bootstrap.current_tenant.id,
                    bootstrap.me.id,
                    draft,
                )
                .await;

                set_is_busy.set(false);
                match res {
                    Ok(created) => {
                        let redirect_url = format!("{base_route}/edit/{}", created.id);
                        nav(&redirect_url, Default::default());
                    }
                    Err(err) => set_error_msg.set(Some(err.to_string())),
                }
            }
        });
    };

    let page_title = if is_editing {
        if is_ru { "Редактирование товара" } else { "Edit Product" }
    } else {
        if is_ru { "Создание товара" } else { "New Product" }
    };

    let save_product_cb_draft = save_product.clone();
    let save_product_cb_active = save_product.clone();

    view! {
        <div class="flex flex-col gap-6 w-full max-w-6xl mx-auto pb-12 animate-in fade-in duration-150">
            // Breadcrumb & Top Bar
            <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4 border-b border-border pb-4">
                <div class="flex items-center gap-3">
                    <a
                        href=base_route.clone()
                        class="inline-flex items-center justify-center w-8 h-8 rounded-lg border border-border bg-background text-foreground hover:bg-accent transition text-sm"
                        title=if is_ru { "Назад к каталогу" } else { "Back to catalog" }
                    >
                        "←"
                    </a>
                    <div>
                        <div class="flex items-center gap-2">
                            <span class="text-xs text-muted-foreground">{if is_ru { "Каталог" } else { "Catalog" }}</span>
                            <span class="text-xs text-muted-foreground">"/"</span>
                            <span class="text-xs font-medium text-foreground">{page_title}</span>
                        </div>
                        <h1 class="text-xl font-bold tracking-tight text-foreground flex items-center gap-2.5 mt-0.5">
                            <span>{page_title}</span>
                            <span class=move || active_kind.get().badge_class()>
                                {
                                    let badge_locale = locale.clone();
                                    move || active_kind.get().label(badge_locale.as_deref())
                                }
                            </span>
                        </h1>
                    </div>
                </div>

                <div class="flex items-center gap-2.5 flex-wrap">
                    <a
                        href=base_route.clone()
                        class="h-9 px-3.5 rounded-xl border border-border bg-background text-xs font-medium text-foreground hover:bg-accent transition flex items-center"
                    >
                        {if is_ru { "Отмена" } else { "Cancel" }}
                    </a>
                    <button
                        type="button"
                        class="h-9 px-3.5 rounded-xl border border-border bg-background text-xs font-medium text-foreground hover:bg-accent transition disabled:opacity-50"
                        disabled=move || is_busy.get()
                        on:click=move |_| save_product_cb_draft(Some("DRAFT"))
                    >
                        {if is_ru { "Сохранить черновик" } else { "Save as Draft" }}
                    </button>
                    <button
                        type="button"
                        class="h-9 px-4 rounded-xl bg-primary text-primary-foreground text-xs font-semibold hover:bg-primary/90 transition shadow-sm disabled:opacity-50"
                        disabled=move || is_busy.get()
                        on:click=move |_| save_product_cb_active(Some("ACTIVE"))
                    >
                        {if is_editing {
                            if is_ru { "Сохранить изменения" } else { "Save Changes" }
                        } else {
                            if is_ru { "Опубликовать товар" } else { "Publish Product" }
                        }}
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

            // Product Type Switcher Hero
            <div class="bg-card rounded-2xl border border-border p-4 shadow-sm">
                <div class="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-2">
                    {if is_ru { "Тип товара" } else { "Product Type" }}
                </div>
                <div class="grid grid-cols-2 sm:grid-cols-4 gap-2.5">
                    {[ProductKind::Simple, ProductKind::Variable, ProductKind::Bundle, ProductKind::Digital].into_iter().map({
                        let switcher_locale = locale.clone();
                        move |kind| {
                            let is_active = move || active_kind.get() == kind;
                            let label = kind.label(switcher_locale.as_deref());
                            let desc = kind.description(switcher_locale.as_deref());
                            view! {
                                <button
                                    type="button"
                                    class=move || {
                                        if is_active() {
                                            "flex flex-col items-start p-3 rounded-xl border-2 border-primary bg-primary/5 text-left transition"
                                        } else {
                                            "flex flex-col items-start p-3 rounded-xl border border-border bg-background hover:border-primary/40 text-left transition"
                                        }
                                    }
                                    on:click=move |_| set_active_kind.set(kind)
                                >
                                    <div class="flex items-center gap-2 mb-1">
                                        <span class="text-base">
                                            {match kind {
                                                ProductKind::Simple => "📦",
                                                ProductKind::Variable => "🎨",
                                                ProductKind::Bundle => "🎁",
                                                ProductKind::Digital => "💾",
                                            }}
                                        </span>
                                        <span class="text-xs font-bold text-foreground">{label}</span>
                                    </div>
                                    <span class="text-[11px] text-muted-foreground line-clamp-2 leading-tight">
                                        {desc}
                                    </span>
                                </button>
                            }
                        }
                    }).collect_view()}
                </div>
            </div>

            // Main Two-Column Layout
            <div class="grid grid-cols-1 lg:grid-cols-3 gap-6 items-start">
                // Left Column (2 cols)
                <div class="lg:col-span-2 flex flex-col gap-6">
                    // General Info Card
                    <div class="bg-card rounded-2xl border border-border p-5 shadow-sm space-y-4">
                        <h2 class="text-sm font-semibold text-foreground border-b border-border pb-2.5">
                            {if is_ru { "Основная информация" } else { "General Information" }}
                        </h2>

                        <div class="space-y-1.5">
                            <label class="text-xs font-medium text-foreground">
                                {if is_ru { "Название товара *" } else { "Product Title *" }}
                            </label>
                            <input
                                type="text"
                                placeholder=if is_ru { "Например, Зимняя куртка Oxford..." } else { "e.g. Oxford Winter Jacket..." }
                                prop:value=move || title.get()
                                on:input=move |ev| set_title.set(event_target_value(&ev))
                                class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2.5 text-foreground placeholder:text-muted-foreground/60 outline-none focus:border-primary focus:ring-1 focus:ring-primary/20 transition font-medium"
                            />
                        </div>

                        <div class="space-y-1.5">
                            <div class="flex items-center justify-between">
                                <label class="text-xs font-medium text-foreground">
                                    {if is_ru { "Slug / URL идентификатор" } else { "Handle / Slug" }}
                                </label>
                                <button
                                    type="button"
                                    class="text-[11px] text-primary hover:underline font-medium"
                                    on:click=on_generate_slug
                                >
                                    {if is_ru { "Сгенерировать из названия" } else { "Auto-generate from title" }}
                                </button>
                            </div>
                            <div class="flex items-center rounded-xl border border-border bg-background px-3 py-2 text-xs">
                                <span class="text-muted-foreground font-mono">"/products/"</span>
                                <input
                                    type="text"
                                    placeholder="oxford-winter-jacket"
                                    prop:value=move || handle.get()
                                    on:input=move |ev| set_handle.set(event_target_value(&ev))
                                    class="w-full bg-transparent text-foreground placeholder:text-muted-foreground/60 outline-none font-mono text-xs ml-0.5"
                                />
                            </div>
                        </div>

                        <div class="space-y-1.5">
                            <label class="text-xs font-medium text-foreground">
                                {if is_ru { "Описание товара" } else { "Description" }}
                            </label>
                            <textarea
                                rows="5"
                                placeholder=if is_ru { "Подробное описание товара, состав, особенности..." } else { "Detailed product description, specifications, features..." }
                                prop:value=move || description.get()
                                on:input=move |ev| set_description.set(event_target_value(&ev))
                                class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2.5 text-foreground placeholder:text-muted-foreground/60 outline-none focus:border-primary focus:ring-1 focus:ring-primary/20 transition resize-y"
                            />
                        </div>
                    </div>

                    // Adaptive Type-Specific Section
                    <div class="bg-card rounded-2xl border border-border p-5 shadow-sm space-y-4">
                        <div class="flex items-center justify-between border-b border-border pb-2.5">
                            <h2 class="text-sm font-semibold text-foreground flex items-center gap-2">
                                <span>
                                    {match active_kind.get() {
                                        ProductKind::Simple => if is_ru { "Параметры простого товара" } else { "Simple Product Specifications" },
                                        ProductKind::Variable => if is_ru { "Варианты и матрица опций" } else { "Variants & Options Matrix" },
                                        ProductKind::Bundle => if is_ru { "Состав комплекта" } else { "Bundle Composition" },
                                        ProductKind::Digital => if is_ru { "Настройки цифровой выдачи" } else { "Digital Fulfillment" },
                                    }}
                                </span>
                            </h2>
                            <span class=move || active_kind.get().badge_class()>
                                {move || active_kind.get().label(locale.as_deref())}
                            </span>
                        </div>

                        {move || match active_kind.get() {
                            ProductKind::Simple => view! {
                                <div class="grid grid-cols-1 sm:grid-cols-2 gap-4">
                                    <div class="space-y-1.5">
                                        <label class="text-xs font-medium text-foreground">
                                            {if is_ru { "Артикул (SKU)" } else { "SKU" }}
                                        </label>
                                        <input
                                            type="text"
                                            placeholder="SKU-1001"
                                            prop:value=move || sku.get()
                                            on:input=move |ev| set_sku.set(event_target_value(&ev))
                                            class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground font-mono outline-none focus:border-primary"
                                        />
                                    </div>
                                    <div class="space-y-1.5">
                                        <label class="text-xs font-medium text-foreground">
                                            {if is_ru { "Штрихкод (Barcode)" } else { "Barcode" }}
                                        </label>
                                        <input
                                            type="text"
                                            placeholder="4600000000000"
                                            prop:value=move || barcode.get()
                                            on:input=move |ev| set_barcode.set(event_target_value(&ev))
                                            class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground font-mono outline-none focus:border-primary"
                                        />
                                    </div>
                                    <div class="space-y-1.5">
                                        <label class="text-xs font-medium text-foreground">
                                            {if is_ru { "Количество на складе" } else { "Stock Quantity" }}
                                        </label>
                                        <input
                                            type="number"
                                            min="0"
                                            prop:value=move || inventory_quantity.get()
                                            on:input=move |ev| {
                                                if let Ok(num) = event_target_value(&ev).parse::<i32>() {
                                                    set_inventory_quantity.set(num);
                                                }
                                            }
                                            class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground font-mono outline-none focus:border-primary"
                                        />
                                    </div>
                                    <div class="space-y-1.5">
                                        <label class="text-xs font-medium text-foreground">
                                            {if is_ru { "Политика списания" } else { "Inventory Policy" }}
                                        </label>
                                        <select
                                            prop:value=move || inventory_policy.get()
                                            on:change=move |ev| set_inventory_policy.set(event_target_value(&ev))
                                            class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground outline-none focus:border-primary"
                                        >
                                            <option value="DENY">{if is_ru { "Запретить заказ при отсутствии" } else { "Deny when out of stock" }}</option>
                                            <option value="CONTINUE">{if is_ru { "Разрешить предзаказ" } else { "Allow backorders" }}</option>
                                        </select>
                                    </div>
                                </div>
                            }.into_any(),

                            ProductKind::Variable => view! {
                                <div class="space-y-4">
                                    <div class="space-y-1.5">
                                        <label class="text-xs font-medium text-foreground">
                                            {if is_ru { "Оси вариаций (атрибуты)" } else { "Variation Axes" }}
                                        </label>
                                        <input
                                            type="text"
                                            placeholder="Color, Size, Material"
                                            prop:value=move || variant_axes_str.get()
                                            on:input=move |ev| set_variant_axes_str.set(event_target_value(&ev))
                                            class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground outline-none focus:border-primary"
                                        />
                                        <p class="text-[11px] text-muted-foreground">
                                            {if is_ru {
                                                "Укажите оси через запятую для генерации комбинаций вариантов"
                                            } else {
                                                "Comma-separated attribute axes for generating variant combinations"
                                            }}
                                        </p>
                                    </div>

                                    <div class="rounded-xl border border-border/80 overflow-hidden">
                                        <div class="bg-muted/40 px-3 py-2 text-xs font-semibold text-foreground flex items-center justify-between">
                                            <span>{if is_ru { "Таблица вариантов" } else { "Generated Variants Table" }}</span>
                                            <span class="text-[11px] text-muted-foreground font-normal">
                                                {move || loaded_product.get().as_ref().map(|p| p.variants.len()).unwrap_or(1)} " variants"
                                            </span>
                                        </div>
                                        <div class="p-3 text-xs text-muted-foreground">
                                            {if let Some(ref prod) = loaded_product.get() {
                                                let variant_rows = build_variant_row_view_models(prod);
                                                view! {
                                                    <div class="flex flex-col gap-2">
                                                        {variant_rows.into_iter().map(|row| view! {
                                                            <div class="flex items-center justify-between p-2 rounded-lg border border-border/50 bg-background">
                                                                <div class="flex flex-col">
                                                                    <span class="font-medium text-foreground text-xs">{row.options_summary}</span>
                                                                    <span class="font-mono text-[10px] text-muted-foreground">{row.sku}</span>
                                                                </div>
                                                                <div class="flex items-center gap-3">
                                                                    <span class="font-mono text-xs text-foreground font-semibold">{row.price}</span>
                                                                    <span class="text-[10px] px-2 py-0.5 rounded-full bg-secondary text-secondary-foreground">{format!("Stock: {}", row.stock)}</span>
                                                                </div>
                                                            </div>
                                                        }).collect_view()}
                                                    </div>
                                                }.into_any()
                                            } else {
                                                view! {
                                                    <p class="italic text-center py-4">
                                                        {if is_ru {
                                                            "После сохранения товара вы сможете сгенерировать полную сетку вариантов"
                                                        } else {
                                                            "Variant matrix will be generated after initial product creation"
                                                        }}
                                                    </p>
                                                }.into_any()
                                            }}
                                        </div>
                                    </div>
                                </div>
                            }.into_any(),

                            ProductKind::Bundle => view! {
                                <div class="space-y-4">
                                    <div class="space-y-1.5">
                                        <label class="text-xs font-medium text-foreground">
                                            {if is_ru { "Компоненты набора" } else { "Bundle Components" }}
                                        </label>
                                        <textarea
                                            rows="3"
                                            placeholder=if is_ru { "ID или SKU товаров, входящих в набор..." } else { "IDs or SKUs of items included in this kit..." }
                                            prop:value=move || bundle_components_str.get()
                                            on:input=move |ev| set_bundle_components_str.set(event_target_value(&ev))
                                            class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground font-mono outline-none focus:border-primary"
                                        />
                                        <p class="text-[11px] text-muted-foreground">
                                            {if is_ru {
                                                "При покупке комплекта остатки списываются с каждого входящего товара отдельно"
                                            } else {
                                                "Inventory will be deducted from each individual component upon bundle purchase"
                                            }}
                                        </p>
                                    </div>
                                </div>
                            }.into_any(),

                            ProductKind::Digital => view! {
                                <div class="space-y-4">
                                    <div class="space-y-1.5">
                                        <label class="text-xs font-medium text-foreground">
                                            {if is_ru { "URL файла для скачивания *" } else { "Downloadable File URL *" }}
                                        </label>
                                        <input
                                            type="text"
                                            placeholder="https://cdn.rustok.io/downloads/file.zip"
                                            prop:value=move || digital_file_url.get()
                                            on:input=move |ev| set_digital_file_url.set(event_target_value(&ev))
                                            class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground font-mono outline-none focus:border-primary"
                                        />
                                    </div>
                                    <div class="grid grid-cols-2 gap-4">
                                        <div class="space-y-1.5">
                                            <label class="text-xs font-medium text-foreground">
                                                {if is_ru { "Лимит скачиваний" } else { "Download Limit" }}
                                            </label>
                                            <input
                                                type="number"
                                                min="1"
                                                prop:value=move || digital_download_limit.get()
                                                on:input=move |ev| set_digital_download_limit.set(event_target_value(&ev))
                                                class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground outline-none focus:border-primary"
                                            />
                                        </div>
                                        <div class="space-y-1.5">
                                            <label class="text-xs font-medium text-foreground">
                                                {if is_ru { "Срок действия ссылки (дней)" } else { "Link Expiry (Days)" }}
                                            </label>
                                            <input
                                                type="number"
                                                min="1"
                                                prop:value=move || digital_expiry_days.get()
                                                on:input=move |ev| set_digital_expiry_days.set(event_target_value(&ev))
                                                class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground outline-none focus:border-primary"
                                            />
                                        </div>
                                    </div>
                                </div>
                            }.into_any(),
                        }}
                    </div>

                    // Pricing Card
                    <div class="bg-card rounded-2xl border border-border p-5 shadow-sm space-y-4">
                        <h2 class="text-sm font-semibold text-foreground border-b border-border pb-2.5">
                            {if is_ru { "Цены и валюта" } else { "Pricing & Valuation" }}
                        </h2>
                        <div class="grid grid-cols-1 sm:grid-cols-3 gap-4">
                            <div class="space-y-1.5">
                                <label class="text-xs font-medium text-foreground">
                                    {if is_ru { "Валюта" } else { "Currency" }}
                                </label>
                                <select
                                    prop:value=move || currency_code.get()
                                    on:change=move |ev| set_currency_code.set(event_target_value(&ev))
                                    class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground outline-none focus:border-primary"
                                >
                                    <option value="USD">"USD - US Dollar"</option>
                                    <option value="EUR">"EUR - Euro"</option>
                                    <option value="RUB">"RUB - Russian Ruble"</option>
                                </select>
                            </div>
                            <div class="space-y-1.5">
                                <label class="text-xs font-medium text-foreground">
                                    {if is_ru { "Цена продажи *" } else { "Price *" }}
                                </label>
                                <input
                                    type="number"
                                    step="0.01"
                                    placeholder="0.00"
                                    prop:value=move || amount.get()
                                    on:input=move |ev| set_amount.set(event_target_value(&ev))
                                    class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground font-mono outline-none focus:border-primary font-bold"
                                />
                            </div>
                            <div class="space-y-1.5">
                                <label class="text-xs font-medium text-foreground">
                                    {if is_ru { "Старая цена (до скидки)" } else { "Compare-at Price" }}
                                </label>
                                <input
                                    type="number"
                                    step="0.01"
                                    placeholder="0.00"
                                    prop:value=move || compare_at_amount.get()
                                    on:input=move |ev| set_compare_at_amount.set(event_target_value(&ev))
                                    class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground font-mono outline-none focus:border-primary"
                                />
                            </div>
                        </div>
                    </div>

                    // Media Gallery Card
                    <div class="bg-card rounded-2xl border border-border p-5 shadow-sm space-y-4">
                        <div class="flex items-center justify-between border-b border-border pb-2.5">
                            <h2 class="text-sm font-semibold text-foreground">
                                {if is_ru { "Медиафайлы и галерея" } else { "Media Gallery" }}
                            </h2>
                            <span class="text-xs text-muted-foreground">
                                {move || loaded_product.get().as_ref().map(|p| p.images.len()).unwrap_or(0)} " images"
                            </span>
                        </div>

                        // Existing images grid
                        {move || {
                            if let Some(ref prod) = loaded_product.get() {
                                let image_vms = build_product_image_view_models(prod);
                                if image_vms.is_empty() {
                                    view! {
                                        <p class="text-xs text-muted-foreground italic py-3 text-center">
                                            {if is_ru { "Нет загруженных изображений" } else { "No images uploaded yet" }}
                                        </p>
                                    }.into_any()
                                } else {
                                    view! {
                                        <div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
                                            {image_vms.into_iter().map(|img| {
                                                let del_id = img.id.clone();
                                                let on_del = on_delete_image.clone();
                                                view! {
                                                    <div class="relative group rounded-xl border border-border overflow-hidden bg-background">
                                                        <img
                                                            src=img.url
                                                            alt=img.alt_text.clone()
                                                            class="w-full h-28 object-cover"
                                                        />
                                                        <div class="absolute inset-0 bg-black/40 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center gap-2">
                                                            <button
                                                                type="button"
                                                                class="p-1.5 rounded-lg bg-rose-600 text-white text-xs hover:bg-rose-700"
                                                                title=if is_ru { "Удалить" } else { "Delete" }
                                                                on:click=move |_| on_del(del_id.clone())
                                                            >
                                                                "🗑"
                                                            </button>
                                                        </div>
                                                        <div class="p-1.5 text-[10px] text-muted-foreground truncate">
                                                            {if img.alt_text.is_empty() { "—".to_string() } else { img.alt_text.clone() }}
                                                        </div>
                                                    </div>
                                                }
                                            }).collect_view()}
                                        </div>
                                    }.into_any()
                                }
                            } else {
                                view! {
                                    <p class="text-xs text-muted-foreground italic py-2">
                                        {if is_ru {
                                            "Изображения можно будет добавить сразу после сохранения товара"
                                        } else {
                                            "Images can be added immediately after saving the product"
                                        }}
                                    </p>
                                }.into_any()
                            }
                        }}

                        // Add image form (available if editing)
                        <Show when=move || is_editing>
                            <div class="pt-3 border-t border-border/60 flex flex-col sm:flex-row gap-2 items-center">
                                <input
                                    type="text"
                                    placeholder="https://example.com/image.jpg"
                                    prop:value=move || new_image_url.get()
                                    on:input=move |ev| set_new_image_url.set(event_target_value(&ev))
                                    class="w-full sm:w-2/3 text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground font-mono outline-none focus:border-primary"
                                />
                                <input
                                    type="text"
                                    placeholder=if is_ru { "Alt описание" } else { "Alt text" }
                                    prop:value=move || new_image_alt.get()
                                    on:input=move |ev| set_new_image_alt.set(event_target_value(&ev))
                                    class="w-full sm:w-1/3 text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground outline-none focus:border-primary"
                                />
                                <button
                                    type="button"
                                    class="h-9 px-3.5 rounded-xl bg-secondary text-secondary-foreground text-xs font-medium hover:bg-accent transition whitespace-nowrap"
                                    on:click={
                                        let on_add = on_add_image.clone();
                                        move |_| on_add()
                                    }
                                >
                                    {if is_ru { "Добавить" } else { "Add" }}
                                </button>
                            </div>
                        </Show>
                    </div>
                </div>

                // Right Column (Sidebar 1 col)
                <div class="flex flex-col gap-6">
                    // Status & Visibility Card
                    <div class="bg-card rounded-2xl border border-border p-5 shadow-sm space-y-4">
                        <h2 class="text-sm font-semibold text-foreground border-b border-border pb-2.5">
                            {if is_ru { "Статус и публикация" } else { "Status & Visibility" }}
                        </h2>
                        <div class="space-y-1.5">
                            <label class="text-xs font-medium text-foreground">
                                {if is_ru { "Текущий статус" } else { "Status" }}
                            </label>
                            <select
                                prop:value=move || status.get()
                                on:change=move |ev| set_status.set(event_target_value(&ev))
                                class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground outline-none focus:border-primary"
                            >
                                <option value="DRAFT">{if is_ru { "Черновик (Draft)" } else { "Draft" }}</option>
                                <option value="ACTIVE">{if is_ru { "Активен (Active)" } else { "Active" }}</option>
                                <option value="ARCHIVED">{if is_ru { "В архиве (Archived)" } else { "Archived" }}</option>
                            </select>
                        </div>
                    </div>

                    // Categorization & Organization Card
                    <div class="bg-card rounded-2xl border border-border p-5 shadow-sm space-y-4">
                        <h2 class="text-sm font-semibold text-foreground border-b border-border pb-2.5">
                            {if is_ru { "Классификация" } else { "Organization" }}
                        </h2>

                        <div class="space-y-1.5">
                            <label class="text-xs font-medium text-foreground">
                                {if is_ru { "Основная категория" } else { "Primary Category" }}
                            </label>
                            <select
                                prop:value=move || primary_category_id.get()
                                on:change=move |ev| set_primary_category_id.set(event_target_value(&ev))
                                class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground outline-none focus:border-primary"
                            >
                                <option value="">{if is_ru { "Без категории" } else { "Uncategorized" }}</option>
                                {move || categories_resource.get().and_then(Result::ok).map(|cats| {
                                    cats.into_iter().map(|c| {
                                        view! {
                                            <option value=c.id.clone()>
                                                {format!("{} ({})", c.name, c.slug)}
                                            </option>
                                        }
                                    }).collect_view()
                                }).unwrap_or_default()}
                            </select>
                        </div>

                        <div class="space-y-1.5">
                            <label class="text-xs font-medium text-foreground">
                                {if is_ru { "Профиль доставки" } else { "Shipping Profile" }}
                            </label>
                            <select
                                prop:value=move || shipping_profile_slug.get()
                                on:change=move |ev| set_shipping_profile_slug.set(event_target_value(&ev))
                                class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground outline-none focus:border-primary"
                            >
                                <option value="">{if is_ru { "Стандартный" } else { "Default Profile" }}</option>
                                {move || shipping_profiles_resource.get().and_then(Result::ok).map(|profs| {
                                    profs.into_iter().map(|p| {
                                        view! {
                                            <option value=p.slug.clone()>
                                                {p.name}
                                            </option>
                                        }
                                    }).collect_view()
                                }).unwrap_or_default()}
                            </select>
                        </div>

                        <div class="space-y-1.5">
                            <label class="text-xs font-medium text-foreground">
                                {if is_ru { "Вендор / Бренд" } else { "Vendor / Brand" }}
                            </label>
                            <input
                                type="text"
                                placeholder="Acme Inc."
                                prop:value=move || vendor.get()
                                on:input=move |ev| set_vendor.set(event_target_value(&ev))
                                class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground outline-none focus:border-primary"
                            />
                        </div>

                        <div class="space-y-1.5">
                            <label class="text-xs font-medium text-foreground">
                                {if is_ru { "Теги (Tags)" } else { "Tags" }}
                            </label>
                            <div class="flex gap-1.5">
                                <input
                                    type="text"
                                    placeholder="new, sale, clothing"
                                    prop:value=move || tag_input.get()
                                    on:input=move |ev| set_tag_input.set(event_target_value(&ev))
                                    class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground outline-none focus:border-primary"
                                />
                                <button
                                    type="button"
                                    class="px-3 rounded-xl bg-secondary text-secondary-foreground text-xs hover:bg-accent"
                                    on:click=on_add_tag
                                >
                                    "+"
                                </button>
                            </div>
                            <div class="flex flex-wrap gap-1.5 mt-2">
                                {move || tags.get().into_iter().map(|tag| {
                                    let tag_cloned = tag.clone();
                                    view! {
                                        <span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs bg-secondary text-secondary-foreground border border-border">
                                            <span>{tag.clone()}</span>
                                            <button
                                                type="button"
                                                class="text-muted-foreground hover:text-foreground text-[10px]"
                                                on:click=move |_| on_remove_tag(tag_cloned.clone())
                                            >
                                                "✕"
                                            </button>
                                        </span>
                                    }
                                }).collect_view()}
                            </div>
                        </div>
                    </div>

                    // SEO & Meta Preview Card
                    <div class="bg-card rounded-2xl border border-border p-5 shadow-sm space-y-4">
                        <h2 class="text-sm font-semibold text-foreground border-b border-border pb-2.5">
                            {if is_ru { "Поисковая оптимизация (SEO)" } else { "SEO & Search Preview" }}
                        </h2>

                        <div class="space-y-1.5">
                            <label class="text-xs font-medium text-foreground">
                                {if is_ru { "Meta заголовок" } else { "Meta Title" }}
                            </label>
                            <input
                                type="text"
                                placeholder=move || title.get()
                                prop:value=move || meta_title.get()
                                on:input=move |ev| set_meta_title.set(event_target_value(&ev))
                                class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground outline-none focus:border-primary"
                            />
                        </div>

                        <div class="space-y-1.5">
                            <label class="text-xs font-medium text-foreground">
                                {if is_ru { "Meta описание" } else { "Meta Description" }}
                            </label>
                            <textarea
                                rows="3"
                                placeholder=move || description.get()
                                prop:value=move || meta_description.get()
                                on:input=move |ev| set_meta_description.set(event_target_value(&ev))
                                class="w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground outline-none focus:border-primary resize-y"
                            />
                        </div>

                        // SERP Preview Box
                        <div class="rounded-xl border border-border/70 bg-background/50 p-3 space-y-1">
                            <div class="text-[11px] text-emerald-600 dark:text-emerald-400 font-mono truncate">
                                {move || {
                                    let h = handle.get();
                                    format!("https://store.domain/products/{}", if h.is_empty() { "slug" } else { &h })
                                }}
                            </div>
                            <div class="text-xs font-semibold text-blue-600 dark:text-blue-400 truncate">
                                {move || {
                                    let mt = meta_title.get();
                                    if !mt.is_empty() {
                                        mt
                                    } else {
                                        let t = title.get();
                                        if t.is_empty() { "Product Title | Store".to_string() } else { t }
                                    }
                                }}
                            </div>
                            <div class="text-[11px] text-muted-foreground line-clamp-2">
                                {move || {
                                    let md = meta_description.get();
                                    if !md.is_empty() {
                                        md
                                    } else {
                                        let d = description.get();
                                        if d.is_empty() { "Product description will appear in search results.".to_string() } else { d }
                                    }
                                }}
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    }
}
