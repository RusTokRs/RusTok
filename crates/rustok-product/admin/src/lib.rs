mod api;
mod i18n;
mod model;

use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
use rustok_api::{AdminQueryKey, UiRouteContext};
use rustok_core::locale_tags_match;
use rustok_seo::{seo_builtin_slug, SeoTargetSlug};
use rustok_seo_admin_support::SeoEntityPanel;

use crate::i18n::t;
use crate::model::{
    ProductAdminBootstrap, ProductDetail, ProductDraft, ProductPricingDetail, ShippingProfile,
};

#[component]
pub fn ProductAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let effective_locale = ui_locale.clone();
    let selected_product_query = use_route_query_value(AdminQueryKey::ProductId.as_str());
    let query_writer = use_route_query_writer();
    let token = use_token();
    let tenant = use_tenant();

    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (editing_id, set_editing_id) = signal(Option::<String>::None);
    let (selected, set_selected) = signal(Option::<ProductDetail>::None);
    let (title, set_title) = signal(String::new());
    let (handle, set_handle) = signal(String::new());
    let (description, set_description) = signal(String::new());
    let (seller_id, set_seller_id) = signal(String::new());
    let (vendor, set_vendor) = signal(String::new());
    let (product_type, set_product_type) = signal(String::new());
    let (shipping_profile_slug, set_shipping_profile_slug) = signal(String::new());
    let (sku, set_sku) = signal(String::new());
    let (barcode, set_barcode) = signal(String::new());
    let (currency_code, set_currency_code) = signal("USD".to_string());
    let (amount, set_amount) = signal("0.00".to_string());
    let (compare_at_amount, set_compare_at_amount) = signal(String::new());
    let (inventory_quantity, set_inventory_quantity) = signal(0_i32);
    let (publish_now, set_publish_now) = signal(false);
    let (search, set_search) = signal(String::new());
    let (status_filter, set_status_filter) = signal(String::new());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);
    let effective_locale_for_products = effective_locale.clone();
    let effective_locale_for_selected_pricing = effective_locale.clone();
    let effective_locale_for_initial_open = effective_locale.clone();

    let bootstrap = Resource::new(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            api::fetch_bootstrap(token_value, tenant_value).await
        },
    );

    let products = Resource::new(
        move || {
            (
                token.get(),
                tenant.get(),
                refresh_nonce.get(),
                effective_locale_for_products.clone(),
                search.get(),
                status_filter.get(),
            )
        },
        move |(token_value, tenant_value, _, locale_value, search_value, status_value)| async move {
            let bootstrap = api::fetch_bootstrap(token_value.clone(), tenant_value.clone()).await?;
            api::fetch_products(
                token_value,
                tenant_value,
                bootstrap.current_tenant.id,
                locale_value,
                text_or_none(search_value),
                text_or_none(status_value),
            )
            .await
        },
    );

    let shipping_profiles = Resource::new(
        move || (token.get(), tenant.get(), refresh_nonce.get()),
        move |(token_value, tenant_value, _)| async move {
            let bootstrap = api::fetch_bootstrap(token_value.clone(), tenant_value.clone()).await?;
            api::fetch_shipping_profiles(token_value, tenant_value, bootstrap.current_tenant.id)
                .await
        },
    );
    let selected_pricing = Resource::new(
        move || {
            (
                token.get(),
                tenant.get(),
                refresh_nonce.get(),
                effective_locale_for_selected_pricing.clone(),
                selected.get().map(|product| {
                    (
                        product.id.clone(),
                        primary_catalog_currency(Some(&product))
                            .unwrap_or_else(|| "USD".to_string()),
                    )
                }),
            )
        },
        move |(token_value, tenant_value, _, locale_value, selected_product)| async move {
            let Some((product_id, currency_code)) = selected_product else {
                return Ok(None);
            };
            let bootstrap = api::fetch_bootstrap(token_value.clone(), tenant_value.clone())
                .await
                .map_err(|err| err.to_string())?;
            api::fetch_product_pricing(
                token_value,
                tenant_value,
                bootstrap.current_tenant.id,
                product_id,
                locale_value,
                Some(currency_code),
            )
            .await
            .map_err(|err| err.to_string())
        },
    );

    let bootstrap_loading_label = t(
        ui_locale.as_deref(),
        "product.error.bootstrapLoading",
        "Bootstrap is still loading.",
    );
    let load_product_error_label = t(
        ui_locale.as_deref(),
        "product.error.loadProduct",
        "Failed to load product",
    );
    let product_not_found_label = t(
        ui_locale.as_deref(),
        "product.error.productNotFound",
        "Product not found.",
    );
    let title_required_label = t(
        ui_locale.as_deref(),
        "product.error.titleRequired",
        "Title is required.",
    );
    let locale_unavailable_label = t(
        ui_locale.as_deref(),
        "product.error.localeUnavailable",
        "Host locale is unavailable.",
    );
    let save_product_error_label = t(
        ui_locale.as_deref(),
        "product.error.saveProduct",
        "Failed to save product",
    );
    let change_status_error_label = t(
        ui_locale.as_deref(),
        "product.error.changeStatus",
        "Failed to change status",
    );
    let delete_product_error_label = t(
        ui_locale.as_deref(),
        "product.error.deleteProduct",
        "Failed to delete product",
    );
    let delete_returned_false_label = t(
        ui_locale.as_deref(),
        "product.error.deleteReturnedFalse",
        "Delete returned false.",
    );
    let initial_product_not_found_label = product_not_found_label.clone();
    let initial_load_product_error_label = load_product_error_label.clone();
    Effect::new(move |_| match selected_product_query.get() {
        Some(product_id) if !product_id.trim().is_empty() => {
            let Some(bootstrap) = bootstrap.get().and_then(Result::ok) else {
                return;
            };
            open_product_for_edit(
                bootstrap,
                token.get(),
                tenant.get(),
                effective_locale_for_initial_open.clone(),
                product_id,
                initial_product_not_found_label.clone(),
                initial_load_product_error_label.clone(),
                set_busy,
                set_error,
                set_editing_id,
                set_selected,
                set_title,
                set_handle,
                set_description,
                set_seller_id,
                set_vendor,
                set_product_type,
                set_shipping_profile_slug,
                set_sku,
                set_barcode,
                set_currency_code,
                set_amount,
                set_compare_at_amount,
                set_inventory_quantity,
                set_publish_now,
            );
        }
        _ => clear_product_form(
            set_editing_id,
            set_selected,
            set_title,
            set_handle,
            set_description,
            set_seller_id,
            set_vendor,
            set_product_type,
            set_shipping_profile_slug,
            set_sku,
            set_barcode,
            set_currency_code,
            set_amount,
            set_compare_at_amount,
            set_inventory_quantity,
            set_publish_now,
        ),
    });

    let reset_form = move || {
        set_editing_id.set(None);
        set_selected.set(None);
        set_title.set(String::new());
        set_handle.set(String::new());
        set_description.set(String::new());
        set_seller_id.set(String::new());
        set_vendor.set(String::new());
        set_product_type.set(String::new());
        set_shipping_profile_slug.set(String::new());
        set_sku.set(String::new());
        set_barcode.set(String::new());
        set_currency_code.set("USD".to_string());
        set_amount.set("0.00".to_string());
        set_compare_at_amount.set(String::new());
        set_inventory_quantity.set(0);
        set_publish_now.set(false);
        set_error.set(None);
    };

    let title_required_label_for_submit = title_required_label.clone();
    let submit_ui_locale = ui_locale.clone();
    let locale_unavailable_label_for_submit = locale_unavailable_label.clone();
    let bootstrap_loading_label_for_submit = bootstrap_loading_label.clone();
    let submit_query_writer = query_writer.clone();
    let on_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        let submit_query_writer = submit_query_writer.clone();
        if title.get_untracked().trim().is_empty() {
            set_error.set(Some(title_required_label_for_submit.clone()));
            return;
        }
        let Some(submit_locale) = submit_ui_locale.clone() else {
            set_error.set(Some(locale_unavailable_label_for_submit.clone()));
            return;
        };

        let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
            set_error.set(Some(bootstrap_loading_label_for_submit.clone()));
            return;
        };

        set_busy.set(true);
        set_error.set(None);

        let draft = ProductDraft {
            locale: submit_locale.clone(),
            title: title.get_untracked(),
            handle: handle.get_untracked(),
            description: description.get_untracked(),
            seller_id: seller_id.get_untracked(),
            vendor: vendor.get_untracked(),
            product_type: product_type.get_untracked(),
            shipping_profile_slug: text_or_none(shipping_profile_slug.get_untracked()),
            sku: sku.get_untracked(),
            barcode: barcode.get_untracked(),
            currency_code: currency_code.get_untracked(),
            amount: amount.get_untracked(),
            compare_at_amount: compare_at_amount.get_untracked(),
            inventory_quantity: inventory_quantity.get_untracked(),
            publish_now: publish_now.get_untracked(),
        };
        let product_id = editing_id.get_untracked();
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();

        let save_product_error_label = save_product_error_label.clone();
        spawn_local(async move {
            let result = match product_id {
                Some(id) => {
                    api::update_product(
                        token_value,
                        tenant_value,
                        bootstrap.current_tenant.id,
                        bootstrap.me.id,
                        id,
                        draft,
                    )
                    .await
                }
                None => {
                    api::create_product(
                        token_value,
                        tenant_value,
                        bootstrap.current_tenant.id,
                        bootstrap.me.id,
                        draft,
                    )
                    .await
                }
            };

            match result {
                Ok(product) => {
                    let product_id = product.id.clone();
                    apply_product(
                        &product,
                        Some(submit_locale.as_str()),
                        set_editing_id,
                        set_selected,
                        set_title,
                        set_handle,
                        set_description,
                        set_seller_id,
                        set_vendor,
                        set_product_type,
                        set_shipping_profile_slug,
                        set_sku,
                        set_barcode,
                        set_currency_code,
                        set_amount,
                        set_compare_at_amount,
                        set_inventory_quantity,
                        set_publish_now,
                    );
                    set_refresh_nonce.update(|value| *value += 1);
                    submit_query_writer
                        .replace_value(AdminQueryKey::ProductId.as_str(), product_id);
                }
                Err(err) => set_error.set(Some(format!("{save_product_error_label}: {err}"))),
            }

            set_busy.set(false);
        });
    };

    let ui_locale_for_list = ui_locale.clone();
    let ui_locale_for_profiles = ui_locale.clone();
    let ui_locale_for_summary = ui_locale.clone();
    let ui_locale_for_editor_title = ui_locale.clone();
    let ui_locale_for_editor_subtitle = ui_locale.clone();
    let ui_locale_for_submit = ui_locale.clone();
    let ui_locale_for_profile_panel = ui_locale.clone();
    let ui_locale_for_summary_title = ui_locale.clone();
    let pricing_module_route_base = route_context.module_route_base("pricing");
    let list_query_writer = query_writer.clone();
    let reset_query_writer = query_writer.clone();
    let delete_query_writer = query_writer.clone();
    let reset_current_product = Callback::new(move |_| {
        reset_query_writer.clear_key(AdminQueryKey::ProductId.as_str());
        reset_form();
    });

    view! {
        <section class="space-y-6">
            <header class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-3">
                    <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                        {t(ui_locale.as_deref(), "product.badge", "product")}
                    </span>
                    <h2 class="text-2xl font-semibold text-card-foreground">
                        {t(ui_locale.as_deref(), "product.title", "Product Catalog")}
                    </h2>
                    <p class="max-w-3xl text-sm text-muted-foreground">
                        {t(
                            ui_locale.as_deref(),
                            "product.subtitle",
                            "Product ownership now lives in the product module package. Commerce keeps delivery orchestration while catalog CRUD moves to the product route.",
                        )}
                    </p>
                </div>
            </header>

            <div class="grid gap-6 xl:grid-cols-[minmax(0,1.1fr)_minmax(0,0.9fr)]">
                <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                    <div class="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
                        <div>
                            <h3 class="text-lg font-semibold text-card-foreground">
                                {t(ui_locale.as_deref(), "product.list.title", "Catalog Feed")}
                            </h3>
                            <p class="text-sm text-muted-foreground">
                                {t(
                                    ui_locale.as_deref(),
                                    "product.list.subtitle",
                                    "Search, open, publish and archive products from the product-owned package.",
                                )}
                            </p>
                        </div>
                        <div class="grid gap-3 md:grid-cols-2">
                            <input
                                class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                                placeholder=t(ui_locale.as_deref(), "product.list.search", "Search title")
                                prop:value=move || search.get()
                                on:input=move |ev| set_search.set(event_target_value(&ev))
                            />
                            <select
                                class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                                prop:value=move || status_filter.get()
                                on:change=move |ev| set_status_filter.set(event_target_value(&ev))
                            >
                                <option value="">{t(ui_locale.as_deref(), "product.status.all", "All statuses")}</option>
                                <option value="DRAFT">{t(ui_locale.as_deref(), "product.status.draft", "Draft")}</option>
                                <option value="ACTIVE">{t(ui_locale.as_deref(), "product.status.active", "Active")}</option>
                                <option value="ARCHIVED">{t(ui_locale.as_deref(), "product.status.archived", "Archived")}</option>
                            </select>
                        </div>
                    </div>

                    <div class="mt-5 space-y-3">
                        {move || match products.get() {
                            None => view! {
                                <div class="rounded-2xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
                                    {t(ui_locale_for_list.as_deref(), "product.list.loading", "Loading products...")}
                                </div>
                            }.into_any(),
                            Some(Err(err)) => view! {
                                <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                    {format!("{}: {err}", t(ui_locale_for_list.as_deref(), "product.error.loadProducts", "Failed to load products"))}
                                </div>
                            }.into_any(),
                            Some(Ok(list)) if list.items.is_empty() => view! {
                                <div class="rounded-2xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
                                    {t(ui_locale_for_list.as_deref(), "product.list.empty", "No products yet.")}
                                </div>
                            }.into_any(),
                            Some(Ok(list)) => view! {
                                <>
                                    {list.items.into_iter().map(|product| {
                                        let item_locale = ui_locale_for_list.clone();
                                        let item_locale_for_chip = item_locale.clone();
                                        let item_locale_for_buttons = item_locale.clone();
                                        let item_locale_for_edit = item_locale.clone();
                                        let item_query_writer = list_query_writer.clone();
                                        let edit_id = product.id.clone();
                                        let publish_id = product.id.clone();
                                        let draft_id = product.id.clone();
                                        let archive_id = product.id.clone();
                                        let delete_id = product.id.clone();
                                        let delete_query_writer_for_item = delete_query_writer.clone();
                                        let status = product.status.clone();
                                        let shipping_profile_label = product.shipping_profile_slug.clone();
                                        let show_shipping_profile = shipping_profile_label.is_some();
                                        let shipping_profile_value = shipping_profile_label.clone().unwrap_or_default();
                                        let meta = format_product_meta(
                                            item_locale.as_deref(),
                                            product.handle.as_str(),
                                            product.vendor.as_deref(),
                                        );
                                        let bootstrap_loading_label_for_publish = bootstrap_loading_label.clone();
                                        let change_status_error_label_for_publish = change_status_error_label.clone();
                                        let bootstrap_loading_label_for_draft = bootstrap_loading_label.clone();
                                        let change_status_error_label_for_draft = change_status_error_label.clone();
                                        let bootstrap_loading_label_for_archive = bootstrap_loading_label.clone();
                                        let change_status_error_label_for_archive = change_status_error_label.clone();
                                        let bootstrap_loading_label_for_delete = bootstrap_loading_label.clone();
                                        let delete_returned_false_label_for_delete = delete_returned_false_label.clone();
                                        let delete_product_error_label_for_delete = delete_product_error_label.clone();
                                        view! {
                                            <article class="rounded-2xl border border-border bg-background p-5 transition hover:border-primary/40">
                                                <div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
                                                    <div class="space-y-2">
                                                        <div class="flex flex-wrap items-center gap-2">
                                                            <span class=format!("inline-flex rounded-full border px-3 py-1 text-xs font-semibold {}", status_badge(status.as_str()))>
                                                                {localized_product_status(item_locale.as_deref(), status.as_str())}
                                                            </span>
                                                            <span class="text-xs uppercase tracking-[0.18em] text-muted-foreground">
                                                                {product.product_type.clone().unwrap_or_else(|| t(item_locale.as_deref(), "product.common.general", "general"))}
                                                            </span>
                                                        </div>
                                                        <h4 class="text-base font-semibold text-card-foreground">{product.title.clone()}</h4>
                                                        <p class="text-sm text-muted-foreground">{meta}</p>
                                                        <Show when=move || show_shipping_profile>
                                                            <span class="inline-flex rounded-full border border-border bg-card px-3 py-1 text-xs text-muted-foreground">
                                                                {format_product_shipping_profile(item_locale_for_chip.as_deref(), shipping_profile_value.as_str())}
                                                            </span>
                                                        </Show>
                                                        <p class="text-xs text-muted-foreground">
                                                            {product.published_at.clone().unwrap_or(product.created_at.clone())}
                                                        </p>
                                                    </div>
                                                    <div class="flex flex-wrap gap-2">
                                                        <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| item_query_writer.push_value(AdminQueryKey::ProductId.as_str(), edit_id.clone())>
                                                            {t(item_locale_for_edit.as_deref(), "product.action.edit", "Edit")}
                                                        </button>
                                                        <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| mutate_status(
                                                            bootstrap.get_untracked().and_then(Result::ok),
                                                            token.get_untracked(),
                                                            tenant.get_untracked(),
                                                            publish_id.clone(),
                                                            "ACTIVE",
                                                            bootstrap_loading_label_for_publish.clone(),
                                                            change_status_error_label_for_publish.clone(),
                                                            set_busy,
                                                            set_error,
                                                            set_refresh_nonce,
                                                        )>
                                                            {t(item_locale_for_buttons.as_deref(), "product.action.publish", "Publish")}
                                                        </button>
                                                        <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| mutate_status(
                                                            bootstrap.get_untracked().and_then(Result::ok),
                                                            token.get_untracked(),
                                                            tenant.get_untracked(),
                                                            draft_id.clone(),
                                                            "DRAFT",
                                                            bootstrap_loading_label_for_draft.clone(),
                                                            change_status_error_label_for_draft.clone(),
                                                            set_busy,
                                                            set_error,
                                                            set_refresh_nonce,
                                                        )>
                                                            {t(item_locale_for_buttons.as_deref(), "product.action.moveToDraft", "Move to Draft")}
                                                        </button>
                                                        <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| mutate_status(
                                                            bootstrap.get_untracked().and_then(Result::ok),
                                                            token.get_untracked(),
                                                            tenant.get_untracked(),
                                                            archive_id.clone(),
                                                            "ARCHIVED",
                                                            bootstrap_loading_label_for_archive.clone(),
                                                            change_status_error_label_for_archive.clone(),
                                                            set_busy,
                                                            set_error,
                                                            set_refresh_nonce,
                                                        )>
                                                            {t(item_locale_for_buttons.as_deref(), "product.action.archive", "Archive")}
                                                        </button>
                                                        <button type="button" class="inline-flex rounded-lg border border-rose-200 px-3 py-2 text-sm font-medium text-rose-700 transition hover:bg-rose-50 disabled:opacity-50" disabled=move || busy.get() on:click=move |_| {
                                                            let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
                                                                set_error.set(Some(bootstrap_loading_label_for_delete.clone()));
                                                                return;
                                                            };
                                                            set_busy.set(true);
                                                            set_error.set(None);
                                                            let token_value = token.get_untracked();
                                                            let tenant_value = tenant.get_untracked();
                                                            let delete_id_value = delete_id.clone();
                                                            let delete_query_writer = delete_query_writer_for_item.clone();
                                                            let delete_returned_false_label = delete_returned_false_label_for_delete.clone();
                                                            let delete_product_error_label = delete_product_error_label_for_delete.clone();
                                                            spawn_local(async move {
                                                                match api::delete_product(
                                                                    token_value,
                                                                    tenant_value,
                                                                    bootstrap.current_tenant.id,
                                                                    bootstrap.me.id,
                                                                    delete_id_value.clone(),
                                                                ).await {
                                                                    Ok(true) => {
                                                                        if editing_id.get_untracked().as_deref() == Some(delete_id_value.as_str()) {
                                                                            delete_query_writer.clear_key(AdminQueryKey::ProductId.as_str());
                                                                            set_editing_id.set(None);
                                                                            set_selected.set(None);
                                                                            set_title.set(String::new());
                                                                            set_handle.set(String::new());
                                                                            set_description.set(String::new());
                                                                            set_vendor.set(String::new());
                                                                            set_product_type.set(String::new());
                                                                            set_shipping_profile_slug.set(String::new());
                                                                            set_sku.set(String::new());
                                                                            set_barcode.set(String::new());
                                                                            set_currency_code.set("USD".to_string());
                                                                            set_amount.set("0.00".to_string());
                                                                            set_compare_at_amount.set(String::new());
                                                                            set_inventory_quantity.set(0);
                                                                            set_publish_now.set(false);
                                                                            set_error.set(None);
                                                                        }
                                                                        set_refresh_nonce.update(|value| *value += 1);
                                                                    }
                                                                    Ok(false) => set_error.set(Some(delete_returned_false_label)),
                                                                    Err(err) => set_error.set(Some(format!("{delete_product_error_label}: {err}"))),
                                                                }
                                                                set_busy.set(false);
                                                            });
                                                        }>
                                                            {t(item_locale_for_buttons.as_deref(), "product.action.delete", "Delete")}
                                                        </button>
                                                    </div>
                                                </div>
                                            </article>
                                        }
                                    }).collect_view()}
                                </>
                            }.into_any(),
                        }}
                    </div>
                </section>

                <section class="space-y-6">
                    <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                        <div class="flex items-center justify-between gap-3">
                            <div>
                                <h3 class="text-lg font-semibold text-card-foreground">
                                    {move || if editing_id.get().is_some() {
                                        t(ui_locale_for_editor_title.as_deref(), "product.editor.editTitle", "Product Editor")
                                    } else {
                                        t(ui_locale_for_editor_title.as_deref(), "product.editor.createTitle", "Create Product")
                                    }}
                                </h3>
                                <p class="text-sm text-muted-foreground">
                                    {t(ui_locale_for_editor_subtitle.as_deref(), "product.editor.subtitle", "Single-SKU catalog editor backed by the existing commerce GraphQL contract.")}
                                </p>
                            </div>
                            <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| reset_current_product.run(())>
                                {t(ui_locale.as_deref(), "product.action.new", "New")}
                            </button>
                        </div>

                        <Show when=move || error.get().is_some()>
                            <div class="mt-4 rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                {move || error.get().unwrap_or_default()}
                            </div>
                        </Show>

                        <form class="mt-5 space-y-4" on:submit=on_submit>
                            <div class="grid gap-4 md:grid-cols-2">
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=t(ui_locale.as_deref(), "product.field.handle", "Handle") prop:value=move || handle.get() on:input=move |ev| set_handle.set(event_target_value(&ev)) />
                            </div>
                            <input class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=t(ui_locale.as_deref(), "product.field.title", "Title") prop:value=move || title.get() on:input=move |ev| set_title.set(event_target_value(&ev)) />
                            <textarea class="min-h-24 w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=t(ui_locale.as_deref(), "product.field.description", "Description") prop:value=move || description.get() on:input=move |ev| set_description.set(event_target_value(&ev)) />
                            <div class="grid gap-4 md:grid-cols-2">
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=t(ui_locale.as_deref(), "product.field.sellerId", "Seller ID") prop:value=move || seller_id.get() on:input=move |ev| set_seller_id.set(event_target_value(&ev)) />
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=t(ui_locale.as_deref(), "product.field.vendor", "Vendor") prop:value=move || vendor.get() on:input=move |ev| set_vendor.set(event_target_value(&ev)) />
                            </div>
                            <div class="grid gap-4 md:grid-cols-2">
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=t(ui_locale.as_deref(), "product.field.productType", "Product type") prop:value=move || product_type.get() on:input=move |ev| set_product_type.set(event_target_value(&ev)) />
                            </div>
                            <div class="grid gap-4 md:grid-cols-2">
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=t(ui_locale.as_deref(), "product.field.primarySku", "Primary SKU") prop:value=move || sku.get() on:input=move |ev| set_sku.set(event_target_value(&ev)) />
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=t(ui_locale.as_deref(), "product.field.barcode", "Barcode") prop:value=move || barcode.get() on:input=move |ev| set_barcode.set(event_target_value(&ev)) />
                            </div>
                            <div class="grid gap-4 md:grid-cols-3">
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=t(ui_locale.as_deref(), "product.field.currency", "Currency") prop:value=move || currency_code.get() on:input=move |ev| set_currency_code.set(event_target_value(&ev)) />
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=t(ui_locale.as_deref(), "product.field.price", "Price") prop:value=move || amount.get() on:input=move |ev| set_amount.set(event_target_value(&ev)) />
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=t(ui_locale.as_deref(), "product.field.compareAtPrice", "Compare-at price") prop:value=move || compare_at_amount.get() on:input=move |ev| set_compare_at_amount.set(event_target_value(&ev)) />
                            </div>
                            <div class="grid gap-4 md:grid-cols-[minmax(0,1fr)_140px]">
                                <select class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" prop:value=move || shipping_profile_slug.get() on:change=move |ev| set_shipping_profile_slug.set(event_target_value(&ev))>
                                    <option value="">{t(ui_locale.as_deref(), "product.field.noShippingProfile", "No shipping profile")}</option>
                                    {move || match shipping_profiles.get() {
                                        Some(Ok(list)) => list.items.into_iter().map(|profile| {
                                            let slug = profile.slug.clone();
                                            let label = shipping_profile_choice_label(ui_locale_for_profiles.as_deref(), &profile);
                                            view! { <option value=slug.clone()>{label}</option> }
                                        }).collect_view().into_any(),
                                        _ => view! { <></> }.into_any(),
                                    }}
                                </select>
                                <input type="number" class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=t(ui_locale.as_deref(), "product.field.inventoryQuantity", "Inventory quantity") prop:value=move || inventory_quantity.get().to_string() on:input=move |ev| set_inventory_quantity.set(event_target_value(&ev).parse().unwrap_or(0)) />
                            </div>
                            <label class="flex items-center gap-2 text-sm text-muted-foreground">
                                <input type="checkbox" prop:checked=move || publish_now.get() on:change=move |ev| set_publish_now.set(event_target_checked(&ev)) />
                                {t(ui_locale.as_deref(), "product.field.keepPublished", "Keep published after save")}
                            </label>
                            <button type="submit" class="inline-flex rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || busy.get()>
                                {move || if editing_id.get().is_some() {
                                    t(ui_locale_for_submit.as_deref(), "product.action.saveProduct", "Save product")
                                } else {
                                    t(ui_locale_for_submit.as_deref(), "product.action.createProduct", "Create product")
                                }}
                            </button>
                        </form>

                        <div class="mt-4 rounded-2xl border border-border bg-background p-4 text-xs text-muted-foreground">
                            {move || match shipping_profiles.get() {
                                None => t(ui_locale_for_profile_panel.as_deref(), "product.profile.loading", "Shipping profiles are loading from the registry."),
                                Some(Err(err)) => format!("{}: {err}", t(ui_locale_for_profile_panel.as_deref(), "product.profile.error", "Failed to load shipping profiles")),
                                Some(Ok(list)) => t(ui_locale_for_profile_panel.as_deref(), "product.profile.known", "Known profiles: {profiles}")
                                    .replace("{profiles}", format_known_shipping_profiles(ui_locale_for_profile_panel.as_deref(), &list.items).as_str()),
                            }}
                        </div>
                    </section>

                    <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                        <h3 class="text-lg font-semibold text-card-foreground">
                            {t(ui_locale_for_summary_title.as_deref(), "product.summary.title", "Selected product")}
                        </h3>
                        <div class="mt-4 rounded-2xl border border-border bg-background p-4 text-sm text-muted-foreground">
                            <SelectedProductSummary
                                locale=ui_locale_for_summary.clone()
                                product=selected.get()
                                pricing_state=selected_pricing.get()
                                pricing_route_base=pricing_module_route_base.clone()
                            />
                        </div>
                    </section>

                    <SeoEntityPanel
                        target_kind=SeoTargetSlug::new(seo_builtin_slug::PRODUCT).expect("builtin SEO target slug")
                        target_id=Signal::derive(move || editing_id.get())
                        locale=Signal::derive({
                            let effective_locale = effective_locale.clone();
                            move || effective_locale.clone().unwrap_or_default()
                        })
                        panel_title=t(effective_locale.as_deref(), "product.seo.title", "Product SEO")
                        panel_subtitle=t(
                            effective_locale.as_deref(),
                            "product.seo.subtitle",
                            "Explicit metadata, social tags and diagnostics for the selected product.",
                        )
                        empty_message=t(
                            effective_locale.as_deref(),
                            "product.seo.empty",
                            "Create or open a product first. The SEO panel stays attached to the product editor.",
                        )
                    />
                </section>
            </div>
        </section>
    }
}

fn open_product_for_edit(
    bootstrap: ProductAdminBootstrap,
    token: Option<String>,
    tenant: Option<String>,
    requested_locale: Option<String>,
    product_id: String,
    product_not_found_label: String,
    load_product_error_label: String,
    set_busy: WriteSignal<bool>,
    set_error: WriteSignal<Option<String>>,
    set_editing_id: WriteSignal<Option<String>>,
    set_selected: WriteSignal<Option<ProductDetail>>,
    set_title: WriteSignal<String>,
    set_handle: WriteSignal<String>,
    set_description: WriteSignal<String>,
    set_seller_id: WriteSignal<String>,
    set_vendor: WriteSignal<String>,
    set_product_type: WriteSignal<String>,
    set_shipping_profile_slug: WriteSignal<String>,
    set_sku: WriteSignal<String>,
    set_barcode: WriteSignal<String>,
    set_currency_code: WriteSignal<String>,
    set_amount: WriteSignal<String>,
    set_compare_at_amount: WriteSignal<String>,
    set_inventory_quantity: WriteSignal<i32>,
    set_publish_now: WriteSignal<bool>,
) {
    set_busy.set(true);
    set_error.set(None);
    spawn_local(async move {
        match api::fetch_product(
            token,
            tenant,
            bootstrap.current_tenant.id,
            product_id,
            requested_locale.clone(),
        )
        .await
        {
            Ok(Some(product)) => apply_product(
                &product,
                requested_locale.as_deref(),
                set_editing_id,
                set_selected,
                set_title,
                set_handle,
                set_description,
                set_seller_id,
                set_vendor,
                set_product_type,
                set_shipping_profile_slug,
                set_sku,
                set_barcode,
                set_currency_code,
                set_amount,
                set_compare_at_amount,
                set_inventory_quantity,
                set_publish_now,
            ),
            Ok(None) => {
                clear_product_form(
                    set_editing_id,
                    set_selected,
                    set_title,
                    set_handle,
                    set_description,
                    set_seller_id,
                    set_vendor,
                    set_product_type,
                    set_shipping_profile_slug,
                    set_sku,
                    set_barcode,
                    set_currency_code,
                    set_amount,
                    set_compare_at_amount,
                    set_inventory_quantity,
                    set_publish_now,
                );
                set_error.set(Some(product_not_found_label));
            }
            Err(err) => {
                clear_product_form(
                    set_editing_id,
                    set_selected,
                    set_title,
                    set_handle,
                    set_description,
                    set_seller_id,
                    set_vendor,
                    set_product_type,
                    set_shipping_profile_slug,
                    set_sku,
                    set_barcode,
                    set_currency_code,
                    set_amount,
                    set_compare_at_amount,
                    set_inventory_quantity,
                    set_publish_now,
                );
                set_error.set(Some(format!("{load_product_error_label}: {err}")));
            }
        }
        set_busy.set(false);
    });
}

fn clear_product_form(
    set_editing_id: WriteSignal<Option<String>>,
    set_selected: WriteSignal<Option<ProductDetail>>,
    set_title: WriteSignal<String>,
    set_handle: WriteSignal<String>,
    set_description: WriteSignal<String>,
    set_seller_id: WriteSignal<String>,
    set_vendor: WriteSignal<String>,
    set_product_type: WriteSignal<String>,
    set_shipping_profile_slug: WriteSignal<String>,
    set_sku: WriteSignal<String>,
    set_barcode: WriteSignal<String>,
    set_currency_code: WriteSignal<String>,
    set_amount: WriteSignal<String>,
    set_compare_at_amount: WriteSignal<String>,
    set_inventory_quantity: WriteSignal<i32>,
    set_publish_now: WriteSignal<bool>,
) {
    set_editing_id.set(None);
    set_selected.set(None);
    set_title.set(String::new());
    set_handle.set(String::new());
    set_description.set(String::new());
    set_seller_id.set(String::new());
    set_vendor.set(String::new());
    set_product_type.set(String::new());
    set_shipping_profile_slug.set(String::new());
    set_sku.set(String::new());
    set_barcode.set(String::new());
    set_currency_code.set("USD".to_string());
    set_amount.set("0.00".to_string());
    set_compare_at_amount.set(String::new());
    set_inventory_quantity.set(0);
    set_publish_now.set(false);
}

fn apply_product(
    product: &ProductDetail,
    requested_locale: Option<&str>,
    set_editing_id: WriteSignal<Option<String>>,
    set_selected: WriteSignal<Option<ProductDetail>>,
    set_title: WriteSignal<String>,
    set_handle: WriteSignal<String>,
    set_description: WriteSignal<String>,
    set_seller_id: WriteSignal<String>,
    set_vendor: WriteSignal<String>,
    set_product_type: WriteSignal<String>,
    set_shipping_profile_slug: WriteSignal<String>,
    set_sku: WriteSignal<String>,
    set_barcode: WriteSignal<String>,
    set_currency_code: WriteSignal<String>,
    set_amount: WriteSignal<String>,
    set_compare_at_amount: WriteSignal<String>,
    set_inventory_quantity: WriteSignal<i32>,
    set_publish_now: WriteSignal<bool>,
) {
    let translation = translation_for_locale(&product.translations, requested_locale);
    let variant = product.variants.first().cloned();
    let price = variant
        .as_ref()
        .and_then(|item| item.prices.first().cloned());

    set_editing_id.set(Some(product.id.clone()));
    set_selected.set(Some(product.clone()));
    set_title.set(
        translation
            .as_ref()
            .map(|item| item.title.clone())
            .unwrap_or_default(),
    );
    set_handle.set(
        translation
            .as_ref()
            .map(|item| item.handle.clone())
            .unwrap_or_default(),
    );
    set_description.set(
        translation
            .and_then(|item| item.description)
            .unwrap_or_default(),
    );
    set_seller_id.set(product.seller_id.clone().unwrap_or_default());
    set_vendor.set(product.vendor.clone().unwrap_or_default());
    set_product_type.set(product.product_type.clone().unwrap_or_default());
    set_shipping_profile_slug.set(product.shipping_profile_slug.clone().unwrap_or_default());
    set_sku.set(
        variant
            .as_ref()
            .and_then(|item| item.sku.clone())
            .unwrap_or_default(),
    );
    set_barcode.set(variant.and_then(|item| item.barcode).unwrap_or_default());
    set_currency_code.set(
        price
            .as_ref()
            .map(|item| item.currency_code.clone())
            .unwrap_or_else(|| "USD".to_string()),
    );
    set_amount.set(
        price
            .as_ref()
            .map(|item| item.amount.clone())
            .unwrap_or_else(|| "0.00".to_string()),
    );
    set_compare_at_amount.set(
        price
            .and_then(|item| item.compare_at_amount)
            .unwrap_or_default(),
    );
    set_inventory_quantity.set(
        product
            .variants
            .first()
            .map(|item| item.inventory_quantity)
            .unwrap_or(0),
    );
    set_publish_now.set(product.status == "ACTIVE");
}

fn translation_for_locale(
    translations: &[crate::model::ProductTranslation],
    requested_locale: Option<&str>,
) -> Option<crate::model::ProductTranslation> {
    requested_locale.and_then(|requested_locale| {
        translations
            .iter()
            .find(|translation| locale_tags_match(&translation.locale, requested_locale))
            .cloned()
    })
}

fn mutate_status(
    bootstrap: Option<ProductAdminBootstrap>,
    token: Option<String>,
    tenant: Option<String>,
    product_id: String,
    status: &str,
    bootstrap_loading_label: String,
    change_status_error_label: String,
    set_busy: WriteSignal<bool>,
    set_error: WriteSignal<Option<String>>,
    set_refresh_nonce: WriteSignal<u64>,
) {
    let Some(bootstrap) = bootstrap else {
        set_error.set(Some(bootstrap_loading_label));
        return;
    };
    let status = status.to_string();
    set_busy.set(true);
    set_error.set(None);
    spawn_local(async move {
        match api::change_product_status(
            token,
            tenant,
            bootstrap.current_tenant.id,
            bootstrap.me.id,
            product_id,
            status.as_str(),
        )
        .await
        {
            Ok(_) => set_refresh_nonce.update(|value| *value += 1),
            Err(err) => set_error.set(Some(format!("{change_status_error_label}: {err}"))),
        }
        set_busy.set(false);
    });
}

#[component]
fn SelectedProductSummary(
    locale: Option<String>,
    product: Option<ProductDetail>,
    pricing_state: Option<Result<Option<ProductPricingDetail>, String>>,
    pricing_route_base: String,
) -> impl IntoView {
    let Some(product) = product else {
        return view! {
            <p>
                {t(
                    locale.as_deref(),
                    "product.summary.empty",
                    "Open a product to inspect its localized copy, catalog snapshot and pricing module preview.",
                )}
            </p>
        }
        .into_any();
    };

    let title = translation_for_locale(&product.translations, locale.as_deref())
        .map(|item| item.title)
        .or_else(|| product.translations.first().map(|item| item.title.clone()))
        .unwrap_or_else(|| t(locale.as_deref(), "product.summary.untitled", "Untitled"));
    let inventory = product
        .variants
        .first()
        .map(|item| item.inventory_quantity)
        .unwrap_or(0);
    let shipping_profile = product.shipping_profile_slug.clone().unwrap_or_else(|| {
        t(
            locale.as_deref(),
            "product.summary.unassigned",
            "unassigned",
        )
    });
    let catalog_snapshot = format_catalog_snapshot_price(locale.as_deref(), Some(&product));

    let pricing_preview = match pricing_state {
        None => t(
            locale.as_deref(),
            "product.summary.pricingLoading",
            "Loading pricing module preview...",
        ),
        Some(Err(err)) => format!(
            "{}: {err}",
            t(
                locale.as_deref(),
                "product.summary.pricingError",
                "Pricing module preview failed",
            )
        ),
        Some(Ok(None)) => t(
            locale.as_deref(),
            "product.summary.pricingUnavailable",
            "Pricing module preview is unavailable.",
        ),
        Some(Ok(Some(pricing))) => format_pricing_preview(locale.as_deref(), Some(&pricing)),
    };
    let pricing_href = build_admin_pricing_href(pricing_route_base.as_str(), &product);

    view! {
        <div class="space-y-3">
            <p class="font-medium text-card-foreground">{title}</p>
            <p>
                {format!(
                    "{} {} | {} {inventory} | {} {shipping_profile}",
                    t(locale.as_deref(), "product.summary.status", "status"),
                    localized_product_status(locale.as_deref(), product.status.as_str()),
                    t(locale.as_deref(), "product.summary.inventory", "inventory"),
                    t(locale.as_deref(), "product.summary.shippingProfile", "shipping profile"),
                )}
            </p>
            <p>
                {format!(
                    "{}: {}",
                    t(
                        locale.as_deref(),
                        "product.summary.catalogSnapshot",
                        "catalog snapshot",
                    ),
                    catalog_snapshot,
                )}
            </p>
            <p>
                {format!(
                    "{}: {}",
                    t(
                        locale.as_deref(),
                        "product.summary.pricingPreview",
                        "pricing module preview",
                    ),
                    pricing_preview,
                )}
            </p>
            <div class="pt-1">
                <a
                    class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent"
                    href=pricing_href
                >
                    {t(
                        locale.as_deref(),
                        "product.summary.openPricing",
                        "Open pricing module",
                    )}
                </a>
            </div>
        </div>
    }
    .into_any()
}

fn primary_catalog_currency(product: Option<&ProductDetail>) -> Option<String> {
    product.and_then(|item| {
        item.variants
            .first()
            .and_then(|variant| variant.prices.first())
            .map(|price| price.currency_code.clone())
    })
}

fn format_catalog_snapshot_price(locale: Option<&str>, product: Option<&ProductDetail>) -> String {
    product
        .and_then(|item| item.variants.first())
        .and_then(|variant| variant.prices.first())
        .map(|price| {
            format_scoped_price(
                locale,
                &price.currency_code,
                &price.amount,
                price.compare_at_amount.as_deref(),
                None,
            )
        })
        .unwrap_or_else(|| t(locale, "product.summary.noPricing", "no pricing"))
}

fn format_pricing_preview(locale: Option<&str>, pricing: Option<&ProductPricingDetail>) -> String {
    let Some(pricing) = pricing else {
        return t(
            locale,
            "product.summary.pricingUnavailable",
            "Pricing module preview is unavailable.",
        );
    };

    let Some(variant) = pricing.variants.first() else {
        return t(locale, "product.summary.noPricing", "no pricing");
    };

    if let Some(price) = variant.effective_price.as_ref() {
        let mut label = format_scoped_price(
            locale,
            &price.currency_code,
            &price.amount,
            price.compare_at_amount.as_deref(),
            price.discount_percent.as_deref(),
        );
        if let Some(scope) = format_pricing_scope(
            locale,
            price.price_list_id.as_deref(),
            price.channel_slug.as_deref(),
            price.channel_id.as_deref(),
        ) {
            label.push_str(format!(" | {scope}").as_str());
        }
        return label;
    }

    variant
        .prices
        .first()
        .map(|price| {
            format_scoped_price(
                locale,
                &price.currency_code,
                &price.amount,
                price.compare_at_amount.as_deref(),
                price.discount_percent.as_deref(),
            )
        })
        .unwrap_or_else(|| t(locale, "product.summary.noPricing", "no pricing"))
}

fn format_scoped_price(
    locale: Option<&str>,
    currency_code: &str,
    amount: &str,
    compare_at_amount: Option<&str>,
    discount_percent: Option<&str>,
) -> String {
    let mut label = if let Some(compare_at_amount) = compare_at_amount {
        format!(
            "{} {} ({})",
            currency_code,
            amount,
            t(locale, "product.summary.compareAt", "compare-at {value}")
                .replace("{value}", compare_at_amount),
        )
    } else {
        format!("{currency_code} {amount}")
    };

    if let Some(discount_percent) = discount_percent.filter(|value| !value.trim().is_empty()) {
        label.push_str(format!(" (-{discount_percent}%)").as_str());
    }

    label
}

fn format_pricing_scope(
    locale: Option<&str>,
    price_list_id: Option<&str>,
    channel_slug: Option<&str>,
    channel_id: Option<&str>,
) -> Option<String> {
    let price_list_id = price_list_id.filter(|value| !value.trim().is_empty());
    let channel_slug = channel_slug.filter(|value| !value.trim().is_empty());
    let channel_id = channel_id.filter(|value| !value.trim().is_empty());

    if price_list_id.is_none() && channel_slug.is_none() && channel_id.is_none() {
        return None;
    }

    let mut parts = Vec::new();
    if let Some(price_list_id) = price_list_id {
        parts.push(t(locale, "product.summary.priceList", "price list") + " " + price_list_id);
    }
    match (channel_slug, channel_id) {
        (Some(channel_slug), Some(channel_id)) => parts.push(
            t(locale, "product.summary.channel", "channel")
                + " "
                + channel_slug
                + " ("
                + channel_id
                + ")",
        ),
        (Some(channel_slug), None) => {
            parts.push(t(locale, "product.summary.channel", "channel") + " " + channel_slug)
        }
        (None, Some(channel_id)) => {
            parts.push(t(locale, "product.summary.channel", "channel") + " " + channel_id)
        }
        (None, None) => {}
    }

    Some(parts.join(" | "))
}

fn build_admin_pricing_href(module_route_base: &str, product: &ProductDetail) -> String {
    let mut params = vec![format!("id={}", product.id)];
    if let Some(currency_code) =
        primary_catalog_currency(Some(product)).filter(|value| !value.trim().is_empty())
    {
        params.push(format!("currency={currency_code}"));
    }
    params.push("quantity=1".to_string());
    format!("{module_route_base}?{}", params.join("&"))
}

fn format_known_shipping_profiles(locale: Option<&str>, profiles: &[ShippingProfile]) -> String {
    let slugs = profiles
        .iter()
        .filter(|profile| profile.active)
        .map(|profile| profile.slug.as_str())
        .collect::<Vec<_>>();
    if slugs.is_empty() {
        t(locale, "product.common.noneYet", "none yet")
    } else {
        slugs.join(", ")
    }
}

fn shipping_profile_choice_label(locale: Option<&str>, profile: &ShippingProfile) -> String {
    if profile.active {
        format!("{} ({})", profile.name, profile.slug)
    } else {
        format!(
            "{} ({}, {})",
            profile.name,
            profile.slug,
            t(locale, "product.common.inactive", "inactive")
        )
    }
}

fn localized_product_status(locale: Option<&str>, status: &str) -> String {
    match status {
        "ACTIVE" => t(locale, "product.status.active", "Active"),
        "ARCHIVED" => t(locale, "product.status.archived", "Archived"),
        _ => t(locale, "product.status.draft", "Draft"),
    }
}

fn format_product_meta(locale: Option<&str>, handle: &str, vendor: Option<&str>) -> String {
    let handle_label = t(locale, "product.summary.handle", "handle");
    let vendor_label = t(locale, "product.summary.vendor", "vendor");
    match vendor.filter(|value| !value.is_empty()) {
        Some(vendor) => format!("{handle_label}: {handle} | {vendor_label}: {vendor}"),
        None => format!("{handle_label}: {handle}"),
    }
}

fn format_product_shipping_profile(locale: Option<&str>, slug: &str) -> String {
    t(locale, "product.summary.profileChip", "profile {slug}").replace("{slug}", slug)
}

fn text_or_none(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn status_badge(status: &str) -> &'static str {
    match status {
        "ACTIVE" => "border-emerald-200 bg-emerald-50 text-emerald-700",
        "ARCHIVED" => "border-slate-200 bg-slate-100 text-slate-700",
        _ => "border-amber-200 bg-amber-50 text-amber-700",
    }
}
