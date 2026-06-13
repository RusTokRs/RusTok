use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui_routing::{use_route_query_value, use_route_query_writer, RouteQueryWriter};
use rustok_api::{AdminQueryKey, UiRouteContext};
use rustok_seo_admin_support::SeoEntityPanel;
use rustok_seo_targets::{builtin_slug as seo_builtin_slug, SeoTargetSlug};

use crate::core::{
    build_product_admin_delete_command, build_product_admin_delete_result_view_model,
    build_product_admin_editor_copy, build_product_admin_editor_form_state,
    build_product_admin_editor_view_model, build_product_admin_error_copy,
    build_product_admin_list_action_labels, build_product_admin_list_controls_view_model,
    build_product_admin_list_empty_view_model, build_product_admin_list_error_view_model,
    build_product_admin_list_item_view_model, build_product_admin_list_loading_view_model,
    build_product_admin_open_product_view_model,
    build_product_admin_profile_panel_error_view_model,
    build_product_admin_profile_panel_loading_view_model,
    build_product_admin_profile_panel_ready_view_model, build_product_admin_save_command,
    build_product_admin_seo_panel_copy, build_product_admin_shell_view_model,
    build_product_admin_status_mutation_command,
    build_product_admin_status_mutation_result_view_model,
    build_selected_product_summary_view_model, empty_product_admin_editor_form_state,
    parse_product_admin_inventory_quantity_input, primary_catalog_currency,
    product_admin_clear_product_query_intent, product_admin_list_actions_disabled,
    product_admin_open_product_query_intent, product_admin_pricing_preview_state_from_result,
    product_admin_saved_product_query_intent, shipping_profile_choice_label, text_or_none,
    ProductAdminDeleteOutcome, ProductAdminDraftForm, ProductAdminEditorFormState,
    ProductAdminErrorCopy, ProductAdminListStateKind, ProductAdminOpenProductViewModel,
    ProductAdminRouteQueryIntent, ProductAdminSaveMode, ProductAdminStatusMutationOutcome,
    ProductAdminStatusTarget, SelectedProductSummaryViewModel,
};
use crate::i18n::t;
use crate::model::{ProductAdminBootstrap, ProductDetail, ProductPricingDetail};
use crate::transport;

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

fn product_admin_list_state_class(kind: &ProductAdminListStateKind) -> &'static str {
    match kind {
        ProductAdminListStateKind::Loading | ProductAdminListStateKind::Empty => {
            "rounded-2xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground"
        }
        ProductAdminListStateKind::Error => {
            "rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive"
        }
    }
}

fn apply_product_admin_route_query_intent(
    query_writer: &RouteQueryWriter,
    intent: ProductAdminRouteQueryIntent,
) {
    match intent {
        ProductAdminRouteQueryIntent::Push { key, value } => query_writer.push_value(key, value),
        ProductAdminRouteQueryIntent::Replace { key, value } => {
            query_writer.replace_value(key, value);
        }
        ProductAdminRouteQueryIntent::Clear { key } => query_writer.clear_key(key),
    }
}

#[component]
pub fn ProductAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let effective_locale = ui_locale.clone();
    let editor_copy = build_product_admin_editor_copy(effective_locale.as_deref());
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

    let bootstrap = local_resource(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            transport::fetch_bootstrap(token_value, tenant_value).await
        },
    );

    let products = local_resource(
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
            let bootstrap =
                transport::fetch_bootstrap(token_value.clone(), tenant_value.clone()).await?;
            transport::fetch_products(
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

    let shipping_profiles = local_resource(
        move || (token.get(), tenant.get(), refresh_nonce.get()),
        move |(token_value, tenant_value, _)| async move {
            let bootstrap =
                transport::fetch_bootstrap(token_value.clone(), tenant_value.clone()).await?;
            transport::fetch_shipping_profiles(
                token_value,
                tenant_value,
                bootstrap.current_tenant.id,
            )
            .await
        },
    );
    let selected_pricing = local_resource(
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
            let bootstrap = transport::fetch_bootstrap(token_value.clone(), tenant_value.clone())
                .await
                .map_err(|err| err.to_string())?;
            transport::fetch_product_pricing(
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

    let error_copy = build_product_admin_error_copy(ui_locale.as_deref());
    let initial_error_copy = error_copy.clone();
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
                initial_error_copy.clone(),
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
        set_error.set(None);
    };

    let submit_ui_locale = ui_locale.clone();
    let submit_query_writer = query_writer.clone();
    let error_copy_for_submit_base = error_copy.clone();
    let on_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        let submit_query_writer = submit_query_writer.clone();
        let submit_locale = submit_ui_locale.clone();
        let command = build_product_admin_save_command(
            ProductAdminDraftForm {
                locale: submit_locale.clone(),
                title: title.get_untracked(),
                handle: handle.get_untracked(),
                description: description.get_untracked(),
                seller_id: seller_id.get_untracked(),
                vendor: vendor.get_untracked(),
                product_type: product_type.get_untracked(),
                shipping_profile_slug: shipping_profile_slug.get_untracked(),
                sku: sku.get_untracked(),
                barcode: barcode.get_untracked(),
                currency_code: currency_code.get_untracked(),
                amount: amount.get_untracked(),
                compare_at_amount: compare_at_amount.get_untracked(),
                inventory_quantity: inventory_quantity.get_untracked(),
                publish_now: publish_now.get_untracked(),
            },
            editing_id.get_untracked(),
            bootstrap.get_untracked().and_then(Result::ok).as_ref(),
        );

        let command = match command {
            Ok(command) => command,
            Err(err) => {
                set_error.set(Some(err.message(submit_ui_locale.as_deref())));
                return;
            }
        };

        set_busy.set(true);
        set_error.set(None);

        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();

        let error_copy_for_submit = error_copy_for_submit_base.clone();
        spawn_local(async move {
            let submit_locale = command.draft.locale.clone();
            let result = match command.mode {
                ProductAdminSaveMode::Update { product_id } => {
                    transport::update_product(
                        token_value,
                        tenant_value,
                        command.tenant_id,
                        command.actor_id,
                        product_id,
                        command.draft,
                    )
                    .await
                }
                ProductAdminSaveMode::Create => {
                    transport::create_product(
                        token_value,
                        tenant_value,
                        command.tenant_id,
                        command.actor_id,
                        command.draft,
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
                    apply_product_admin_route_query_intent(
                        &submit_query_writer,
                        product_admin_saved_product_query_intent(product_id),
                    );
                }
                Err(err) => set_error.set(Some(error_copy_for_submit.save_product_failure(err))),
            }

            set_busy.set(false);
        });
    };

    let ui_locale_for_list = ui_locale.clone();
    let ui_locale_for_profiles = ui_locale.clone();
    let ui_locale_for_summary = ui_locale.clone();
    let ui_locale_for_editor = ui_locale.clone();
    let ui_locale_for_submit = ui_locale.clone();
    let ui_locale_for_profile_panel = ui_locale.clone();
    let ui_locale_for_summary_title = ui_locale.clone();
    let pricing_module_route_base = route_context.module_route_base("pricing");
    let list_query_writer = query_writer.clone();
    let reset_query_writer = query_writer.clone();
    let delete_query_writer = query_writer.clone();
    let reset_current_product = Callback::new(move |_| {
        apply_product_admin_route_query_intent(
            &reset_query_writer,
            product_admin_clear_product_query_intent(),
        );
        reset_form();
    });

    view! {
        <section class="space-y-6">
            <header class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                {
                    let shell = build_product_admin_shell_view_model(ui_locale.as_deref());
                    view! {
                        <div class="space-y-3">
                            <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                                {shell.badge}
                            </span>
                            <h2 class="text-2xl font-semibold text-card-foreground">
                                {shell.title}
                            </h2>
                            <p class="max-w-3xl text-sm text-muted-foreground">
                                {shell.subtitle}
                            </p>
                        </div>
                    }
                }
            </header>

            <div class="grid gap-6 xl:grid-cols-[minmax(0,1.1fr)_minmax(0,0.9fr)]">
                <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                    <div class="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
                        {
                            let controls = build_product_admin_list_controls_view_model(ui_locale.as_deref());
                            let controls_title = controls.title;
                            let controls_subtitle = controls.subtitle;
                            let search_placeholder = controls.search_placeholder;
                            let status_options = controls.status_options;

                            view! {
                                <div>
                                    <h3 class="text-lg font-semibold text-card-foreground">
                                        {controls_title}
                                    </h3>
                                    <p class="text-sm text-muted-foreground">
                                        {controls_subtitle}
                                    </p>
                                </div>
                                <div class="grid gap-3 md:grid-cols-2">
                                    <input
                                        class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                                        placeholder=search_placeholder
                                        prop:value=move || search.get()
                                        on:input=move |ev| set_search.set(event_target_value(&ev))
                                    />
                                    <select
                                        class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                                        prop:value=move || status_filter.get()
                                        on:change=move |ev| set_status_filter.set(event_target_value(&ev))
                                    >
                                        {status_options.into_iter().map(|option| {
                                            view! {
                                                <option value=option.value>{option.label}</option>
                                            }
                                        }).collect_view()}
                                    </select>
                                </div>
                            }
                        }
                    </div>

                    <div class="mt-5 space-y-3">
                        {move || match products.get() {
                            None => {
                                let state = build_product_admin_list_loading_view_model(
                                    ui_locale_for_list.as_deref(),
                                );
                                view! {
                                    <div class=product_admin_list_state_class(&state.kind)>
                                        {state.message}
                                    </div>
                                }.into_any()
                            },
                            Some(Err(err)) => {
                                let state = build_product_admin_list_error_view_model(
                                    ui_locale_for_list.as_deref(),
                                    err,
                                );
                                view! {
                                    <div class=product_admin_list_state_class(&state.kind)>
                                        {state.message}
                                    </div>
                                }.into_any()
                            },
                            Some(Ok(list)) if list.items.is_empty() => {
                                let state = build_product_admin_list_empty_view_model(
                                    ui_locale_for_list.as_deref(),
                                );
                                view! {
                                    <div class=product_admin_list_state_class(&state.kind)>
                                        {state.message}
                                    </div>
                                }.into_any()
                            },
                            Some(Ok(list)) => view! {
                                <>
                                    {list.items.into_iter().map(|product| {
                                        let item_locale = ui_locale_for_list.clone();
                                        let item_locale_for_buttons = item_locale.clone();
                                        let _item_locale_for_edit = item_locale.clone();
                                        let item_query_writer = list_query_writer.clone();
                                        let edit_id = product.id.clone();
                                        let publish_id = product.id.clone();
                                        let draft_id = product.id.clone();
                                        let archive_id = product.id.clone();
                                        let delete_id = product.id.clone();
                                        let delete_query_writer_for_item = delete_query_writer.clone();
                                        let item_view_model = build_product_admin_list_item_view_model(
                                            item_locale.as_deref(),
                                            &product,
                                        );
                                        let item_status_badge_class = item_view_model.status_badge_class;
                                        let item_status_label = item_view_model.status_label.clone();
                                        let item_type_label = item_view_model.type_label.clone();
                                        let item_title = item_view_model.title.clone();
                                        let item_meta_label = item_view_model.meta_label.clone();
                                        let item_shipping_profile_label = item_view_model.shipping_profile_label.clone();
                                        let show_shipping_profile = item_shipping_profile_label.is_some();
                                        let item_timestamp_label = item_view_model.timestamp_label.clone();
                                        let action_labels = build_product_admin_list_action_labels(
                                            item_locale_for_buttons.as_deref(),
                                        );
                                        let edit_label = action_labels.edit.clone();
                                        let publish_label = action_labels.publish.clone();
                                        let draft_label = action_labels.move_to_draft.clone();
                                        let archive_label = action_labels.archive.clone();
                                        let delete_label = action_labels.delete.clone();
                                        let item_locale_for_publish = item_locale_for_buttons.clone();
                                        let item_locale_for_draft = item_locale_for_buttons.clone();
                                        let item_locale_for_archive = item_locale_for_buttons.clone();
                                        let item_locale_for_delete = item_locale_for_buttons.clone();
                                        view! {
                                            <article class="rounded-2xl border border-border bg-background p-5 transition hover:border-primary/40">
                                                <div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
                                                    <div class="space-y-2">
                                                        <div class="flex flex-wrap items-center gap-2">
                                                            <span class=format!("inline-flex rounded-full border px-3 py-1 text-xs font-semibold {}", item_status_badge_class)>
                                                                {item_status_label.clone()}
                                                            </span>
                                                            <span class="text-xs uppercase tracking-[0.18em] text-muted-foreground">
                                                                {item_type_label.clone()}
                                                            </span>
                                                        </div>
                                                        <h4 class="text-base font-semibold text-card-foreground">{item_title.clone()}</h4>
                                                        <p class="text-sm text-muted-foreground">{item_meta_label.clone()}</p>
                                                        <Show when=move || show_shipping_profile>
                                                            <span class="inline-flex rounded-full border border-border bg-card px-3 py-1 text-xs text-muted-foreground">
                                                                {item_shipping_profile_label.clone().unwrap_or_default()}
                                                            </span>
                                                        </Show>
                                                        <p class="text-xs text-muted-foreground">
                                                            {item_timestamp_label.clone()}
                                                        </p>
                                                    </div>
                                                    <div class="flex flex-wrap gap-2">
                                                        <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || product_admin_list_actions_disabled(busy.get()) on:click=move |_| apply_product_admin_route_query_intent(
                                                            &item_query_writer,
                                                            product_admin_open_product_query_intent(edit_id.clone()),
                                                        )>
                                                            {edit_label.clone()}
                                                        </button>
                                                        <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || product_admin_list_actions_disabled(busy.get()) on:click=move |_| mutate_status(
                                                            bootstrap.get_untracked().and_then(Result::ok),
                                                            token.get_untracked(),
                                                            tenant.get_untracked(),
                                                            publish_id.clone(),
                                                            ProductAdminStatusTarget::Active,
                                                            item_locale_for_publish.clone(),
                                                            set_busy,
                                                            set_error,
                                                            set_refresh_nonce,
                                                        )>
                                                            {publish_label.clone()}
                                                        </button>
                                                        <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || product_admin_list_actions_disabled(busy.get()) on:click=move |_| mutate_status(
                                                            bootstrap.get_untracked().and_then(Result::ok),
                                                            token.get_untracked(),
                                                            tenant.get_untracked(),
                                                            draft_id.clone(),
                                                            ProductAdminStatusTarget::Draft,
                                                            item_locale_for_draft.clone(),
                                                            set_busy,
                                                            set_error,
                                                            set_refresh_nonce,
                                                        )>
                                                            {draft_label.clone()}
                                                        </button>
                                                        <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || product_admin_list_actions_disabled(busy.get()) on:click=move |_| mutate_status(
                                                            bootstrap.get_untracked().and_then(Result::ok),
                                                            token.get_untracked(),
                                                            tenant.get_untracked(),
                                                            archive_id.clone(),
                                                            ProductAdminStatusTarget::Archived,
                                                            item_locale_for_archive.clone(),
                                                            set_busy,
                                                            set_error,
                                                            set_refresh_nonce,
                                                        )>
                                                            {archive_label.clone()}
                                                        </button>
                                                        <button type="button" class="inline-flex rounded-lg border border-rose-200 px-3 py-2 text-sm font-medium text-rose-700 transition hover:bg-rose-50 disabled:opacity-50" disabled=move || product_admin_list_actions_disabled(busy.get()) on:click=move |_| mutate_delete(
                                                            bootstrap.get_untracked().and_then(Result::ok),
                                                            token.get_untracked(),
                                                            tenant.get_untracked(),
                                                            delete_id.clone(),
                                                            item_locale_for_delete.clone(),
                                                            delete_query_writer_for_item.clone(),
                                                            editing_id,
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
                                                            set_busy,
                                                            set_error,
                                                            set_refresh_nonce,
                                                        )>
                                                            {delete_label.clone()}
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
                                    {
                                        let ui_locale_for_editor = ui_locale_for_editor.clone();
                                        move || build_product_admin_editor_view_model(
                                            ui_locale_for_editor.as_deref(),
                                            editing_id.get().as_deref(),
                                        ).title
                                    }
                                </h3>
                                <p class="text-sm text-muted-foreground">
                                    {
                                        let ui_locale_for_editor = ui_locale_for_editor.clone();
                                        move || build_product_admin_editor_view_model(
                                            ui_locale_for_editor.as_deref(),
                                            editing_id.get().as_deref(),
                                        ).subtitle
                                    }
                                </p>
                            </div>
                            <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| reset_current_product.run(())>
                                {editor_copy.new_action_label.clone()}
                            </button>
                        </div>

                        <Show when=move || error.get().is_some()>
                            <div class="mt-4 rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                {move || error.get().unwrap_or_default()}
                            </div>
                        </Show>

                        <form class="mt-5 space-y-4" on:submit=on_submit>
                            <div class="grid gap-4 md:grid-cols-2">
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=editor_copy.handle_placeholder.clone() prop:value=move || handle.get() on:input=move |ev| set_handle.set(event_target_value(&ev)) />
                            </div>
                            <input class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=editor_copy.title_placeholder.clone() prop:value=move || title.get() on:input=move |ev| set_title.set(event_target_value(&ev)) />
                            <textarea class="min-h-24 w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=editor_copy.description_placeholder.clone() prop:value=move || description.get() on:input=move |ev| set_description.set(event_target_value(&ev)) />
                            <div class="grid gap-4 md:grid-cols-2">
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=editor_copy.seller_id_placeholder.clone() prop:value=move || seller_id.get() on:input=move |ev| set_seller_id.set(event_target_value(&ev)) />
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=editor_copy.vendor_placeholder.clone() prop:value=move || vendor.get() on:input=move |ev| set_vendor.set(event_target_value(&ev)) />
                            </div>
                            <div class="grid gap-4 md:grid-cols-2">
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=editor_copy.product_type_placeholder.clone() prop:value=move || product_type.get() on:input=move |ev| set_product_type.set(event_target_value(&ev)) />
                            </div>
                            <div class="grid gap-4 md:grid-cols-2">
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=editor_copy.primary_sku_placeholder.clone() prop:value=move || sku.get() on:input=move |ev| set_sku.set(event_target_value(&ev)) />
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=editor_copy.barcode_placeholder.clone() prop:value=move || barcode.get() on:input=move |ev| set_barcode.set(event_target_value(&ev)) />
                            </div>
                            <div class="grid gap-4 md:grid-cols-3">
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=editor_copy.currency_placeholder.clone() prop:value=move || currency_code.get() on:input=move |ev| set_currency_code.set(event_target_value(&ev)) />
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=editor_copy.price_placeholder.clone() prop:value=move || amount.get() on:input=move |ev| set_amount.set(event_target_value(&ev)) />
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=editor_copy.compare_at_price_placeholder.clone() prop:value=move || compare_at_amount.get() on:input=move |ev| set_compare_at_amount.set(event_target_value(&ev)) />
                            </div>
                            <div class="grid gap-4 md:grid-cols-[minmax(0,1fr)_140px]">
                                <select class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" prop:value=move || shipping_profile_slug.get() on:change=move |ev| set_shipping_profile_slug.set(event_target_value(&ev))>
                                    <option value="">{editor_copy.no_shipping_profile_label.clone()}</option>
                                    {move || match shipping_profiles.get() {
                                        Some(Ok(list)) => list.items.into_iter().map(|profile| {
                                            let slug = profile.slug.clone();
                                            let label = shipping_profile_choice_label(ui_locale_for_profiles.as_deref(), &profile);
                                            view! { <option value=slug.clone()>{label}</option> }
                                        }).collect_view().into_any(),
                                        _ => ().into_any(),
                                    }}
                                </select>
                                <input type="number" class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=editor_copy.inventory_quantity_placeholder.clone() prop:value=move || inventory_quantity.get().to_string() on:input=move |ev| set_inventory_quantity.set(parse_product_admin_inventory_quantity_input(
                                    &event_target_value(&ev),
                                )) />
                            </div>
                            <label class="flex items-center gap-2 text-sm text-muted-foreground">
                                <input type="checkbox" prop:checked=move || publish_now.get() on:change=move |ev| set_publish_now.set(event_target_checked(&ev)) />
                                {editor_copy.keep_published_label.clone()}
                            </label>
                            <button type="submit" class="inline-flex rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || busy.get()>
                                {move || build_product_admin_editor_view_model(
                                    ui_locale_for_submit.as_deref(),
                                    editing_id.get().as_deref(),
                                ).submit_label}
                            </button>
                        </form>

                        <div class="mt-4 rounded-2xl border border-border bg-background p-4 text-xs text-muted-foreground">
                            {move || match shipping_profiles.get() {
                                None => build_product_admin_profile_panel_loading_view_model(
                                    ui_locale_for_profile_panel.as_deref(),
                                )
                                .into_message(),
                                Some(Err(err)) => build_product_admin_profile_panel_error_view_model(
                                    ui_locale_for_profile_panel.as_deref(),
                                    err,
                                )
                                .into_message(),
                                Some(Ok(list)) => build_product_admin_profile_panel_ready_view_model(
                                    ui_locale_for_profile_panel.as_deref(),
                                    &list.items,
                                )
                                .into_message(),
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

                    {
                        let seo_copy = build_product_admin_seo_panel_copy(effective_locale.as_deref());
                        view! {
                            <SeoEntityPanel
                                target_kind=SeoTargetSlug::new(seo_builtin_slug::PRODUCT).expect("builtin SEO target slug")
                                target_id=Signal::derive(move || editing_id.get())
                                locale=Signal::derive({
                                    let effective_locale = effective_locale.clone();
                                    move || effective_locale.clone().unwrap_or_default()
                                })
                                show_control_plane_widgets=true
                                panel_title=seo_copy.title
                                panel_subtitle=seo_copy.subtitle
                                empty_message=seo_copy.empty_message
                            />
                        }
                    }
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
    error_copy: ProductAdminErrorCopy,
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
        let result = transport::fetch_product(
            token,
            tenant,
            bootstrap.current_tenant.id,
            product_id,
            requested_locale.clone(),
        )
        .await;

        match build_product_admin_open_product_view_model(
            requested_locale.as_deref(),
            &error_copy,
            result,
        ) {
            ProductAdminOpenProductViewModel::Ready {
                product,
                form_state,
            } => {
                set_selected.set(Some(product));
                apply_product_editor_form_state(
                    form_state,
                    set_editing_id,
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
            ProductAdminOpenProductViewModel::Empty {
                form_state,
                error_message,
            } => {
                set_selected.set(None);
                apply_product_editor_form_state(
                    form_state,
                    set_editing_id,
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
                set_error.set(Some(error_message));
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
    set_selected.set(None);
    apply_product_editor_form_state(
        empty_product_admin_editor_form_state(),
        set_editing_id,
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
    set_selected.set(Some(product.clone()));
    apply_product_editor_form_state(
        build_product_admin_editor_form_state(product, requested_locale),
        set_editing_id,
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

fn apply_product_editor_form_state(
    state: ProductAdminEditorFormState,
    set_editing_id: WriteSignal<Option<String>>,
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
    set_editing_id.set(state.editing_id);
    set_title.set(state.title);
    set_handle.set(state.handle);
    set_description.set(state.description);
    set_seller_id.set(state.seller_id);
    set_vendor.set(state.vendor);
    set_product_type.set(state.product_type);
    set_shipping_profile_slug.set(state.shipping_profile_slug);
    set_sku.set(state.sku);
    set_barcode.set(state.barcode);
    set_currency_code.set(state.currency_code);
    set_amount.set(state.amount);
    set_compare_at_amount.set(state.compare_at_amount);
    set_inventory_quantity.set(state.inventory_quantity);
    set_publish_now.set(state.publish_now);
}

fn mutate_status(
    bootstrap: Option<ProductAdminBootstrap>,
    token: Option<String>,
    tenant: Option<String>,
    product_id: String,
    status: ProductAdminStatusTarget,
    locale: Option<String>,
    set_busy: WriteSignal<bool>,
    set_error: WriteSignal<Option<String>>,
    set_refresh_nonce: WriteSignal<u64>,
) {
    let command =
        match build_product_admin_status_mutation_command(bootstrap.as_ref(), product_id, status) {
            Ok(command) => command,
            Err(err) => {
                set_error.set(Some(err.message(locale.as_deref())));
                return;
            }
        };

    set_busy.set(true);
    set_error.set(None);
    spawn_local(async move {
        let outcome = match transport::change_product_status(
            token,
            tenant,
            command.tenant_id,
            command.actor_id,
            command.product_id,
            command.status.as_graphql_status(),
        )
        .await
        {
            Ok(_) => ProductAdminStatusMutationOutcome::Changed,
            Err(err) => ProductAdminStatusMutationOutcome::TransportError(err.to_string()),
        };
        let view_model =
            build_product_admin_status_mutation_result_view_model(locale.as_deref(), outcome);

        if view_model.refresh {
            set_refresh_nonce.update(|value| *value += 1);
        }
        match view_model.error_message {
            Some(message) => set_error.set(Some(message)),
            None => set_error.set(None),
        }
        set_busy.set(false);
    });
}

fn mutate_delete(
    bootstrap: Option<ProductAdminBootstrap>,
    token: Option<String>,
    tenant: Option<String>,
    product_id: String,
    locale: Option<String>,
    query_writer: RouteQueryWriter,
    editing_id: ReadSignal<Option<String>>,
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
    set_busy: WriteSignal<bool>,
    set_error: WriteSignal<Option<String>>,
    set_refresh_nonce: WriteSignal<u64>,
) {
    let command = match build_product_admin_delete_command(bootstrap.as_ref(), product_id) {
        Ok(command) => command,
        Err(err) => {
            set_error.set(Some(err.message(locale.as_deref())));
            return;
        }
    };

    set_busy.set(true);
    set_error.set(None);
    spawn_local(async move {
        let deleted_product_id = command.product_id.clone();
        let outcome = match transport::delete_product(
            token,
            tenant,
            command.tenant_id,
            command.actor_id,
            command.product_id,
        )
        .await
        {
            Ok(true) => ProductAdminDeleteOutcome::Deleted,
            Ok(false) => ProductAdminDeleteOutcome::NotDeleted,
            Err(err) => ProductAdminDeleteOutcome::TransportError(err.to_string()),
        };
        let view_model = build_product_admin_delete_result_view_model(
            locale.as_deref(),
            deleted_product_id.as_str(),
            editing_id.get_untracked().as_deref(),
            outcome,
        );

        if view_model.clear_selection {
            apply_product_admin_route_query_intent(
                &query_writer,
                product_admin_clear_product_query_intent(),
            );
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
        }

        match view_model.error_message {
            Some(message) => set_error.set(Some(message)),
            None => set_error.set(None),
        }
        if view_model.refresh {
            set_refresh_nonce.update(|value| *value += 1);
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
    let pricing_state = product_admin_pricing_preview_state_from_result(pricing_state.as_ref());

    match build_selected_product_summary_view_model(
        locale.as_deref(),
        product.as_ref(),
        pricing_state,
        pricing_route_base.as_str(),
    ) {
        SelectedProductSummaryViewModel::Empty { message } => view! {
            <p>{message}</p>
        }
        .into_any(),
        SelectedProductSummaryViewModel::Ready {
            title,
            status_line,
            catalog_snapshot_label,
            pricing_preview_label,
            pricing_href,
            open_pricing_label,
        } => view! {
            <div class="space-y-3">
                <p class="font-medium text-card-foreground">{title}</p>
                <p>{status_line}</p>
                <p>{catalog_snapshot_label}</p>
                <p>{pricing_preview_label}</p>
                <div class="pt-1">
                    <a
                        class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent"
                        href=pricing_href
                    >
                        {open_pricing_label}
                    </a>
                </div>
            </div>
        }
        .into_any(),
    }
}
