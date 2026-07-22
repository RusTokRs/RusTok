use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui_routing::{RouteQueryWriter, use_route_query_value, use_route_query_writer};
use rustok_seo_admin_support::SeoEntityPanel;
use rustok_seo_targets::{SeoTargetSlug, builtin_slug as seo_builtin_slug};
use rustok_ui_core::{AdminQueryKey, UiRouteContext};

use crate::core::{
    DeleteOutcome, DraftForm, ProductAdminEditorFormState, ProductAdminErrorCopy,
    ProductAdminOpenProductViewModel, ProductAdminProductsLoadViewModel,
    ProductAdminSelectedProductQueryState, ProductAttributeEditorState, SaveMode,
    SelectedProductSummaryViewModel, StatusOutcome, StatusTarget, build_delete_command,
    build_delete_result_view_model, build_product_admin_editor_copy,
    build_product_admin_editor_form_state, build_product_admin_editor_view_model,
    build_product_admin_error_copy, build_product_admin_list_action_labels,
    build_product_admin_list_controls_view_model, build_product_admin_list_item_view_model,
    build_product_admin_open_product_view_model, build_product_admin_seo_panel_copy,
    build_product_admin_shell_view_model, build_product_admin_summary_panel_copy,
    build_product_attribute_form_copy, build_product_detached_attribute_value_view_models,
    build_save_command, build_selected_product_summary_view_model, build_status_command,
    build_status_result_view_model, empty_product_admin_editor_form_state,
    parse_product_admin_inventory_quantity_input, pricing_preview_request_from_product,
    pricing_preview_state_from_result, product_admin_clear_product_query_intent,
    product_admin_list_actions_disabled, product_admin_open_product_query_intent,
    product_admin_products_load_view_from_result, product_admin_saved_product_query_intent,
    product_admin_selected_product_query_state, shipping_profiles_load_view_from_result,
    text_or_none,
};
use crate::model::{
    ProductAdminBootstrap, ProductDetail, ProductEffectiveFormAttribute, ProductPricingDetail,
};
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

#[component]
fn TypedProductAttributeField(
    attribute: ProductEffectiveFormAttribute,
    editor_state: RwSignal<ProductAttributeEditorState>,
    required_label: String,
    empty_option_label: String,
    boolean_true_label: String,
    boolean_false_label: String,
) -> impl IntoView {
    let attribute_id = attribute.attribute_id.clone();
    let value_type = attribute.value_type.clone();
    let input = match value_type.as_str() {
        "text" => {
            let read_id = attribute_id.clone();
            let write_id = attribute_id.clone();
            view! {
                <input class="w-full rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground outline-none focus:border-primary" prop:value=move || editor_state.get().text(&read_id) on:input=move |event| editor_state.update(|state| state.set_text(write_id.clone(), event_target_value(&event))) />
            }.into_any()
        }
        "textarea" | "richtext" => {
            let read_id = attribute_id.clone();
            let write_id = attribute_id.clone();
            view! {
                <textarea class="min-h-24 w-full rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground outline-none focus:border-primary" prop:value=move || editor_state.get().text(&read_id) on:input=move |event| editor_state.update(|state| state.set_text(write_id.clone(), event_target_value(&event))) />
            }.into_any()
        }
        "integer" | "decimal" => {
            let read_id = attribute_id.clone();
            let write_id = attribute_id.clone();
            view! {
                <input type="number" step=if value_type == "integer" { "1" } else { "any" } class="w-full rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground outline-none focus:border-primary" prop:value=move || editor_state.get().text(&read_id) on:input=move |event| editor_state.update(|state| state.set_text(write_id.clone(), event_target_value(&event))) />
            }.into_any()
        }
        "boolean" => {
            let read_id = attribute_id.clone();
            let write_id = attribute_id.clone();
            view! {
                <select class="w-full rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground outline-none focus:border-primary" prop:value=move || editor_state.get().boolean_value(&read_id) on:change=move |event| editor_state.update(|state| state.set_boolean(write_id.clone(), event_target_value(&event)))>
                    <option value="">{empty_option_label.clone()}</option>
                    <option value="true">{boolean_true_label}</option>
                    <option value="false">{boolean_false_label}</option>
                </select>
            }.into_any()
        }
        "date" => {
            let read_id = attribute_id.clone();
            let write_id = attribute_id.clone();
            view! {
                <input type="date" class="w-full rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground outline-none focus:border-primary" prop:value=move || editor_state.get().text(&read_id) on:input=move |event| editor_state.update(|state| state.set_text(write_id.clone(), event_target_value(&event))) />
            }.into_any()
        }
        "datetime" => {
            let read_id = attribute_id.clone();
            let write_id = attribute_id.clone();
            view! {
                <input type="text" class="w-full rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground outline-none focus:border-primary" prop:value=move || editor_state.get().text(&read_id) on:input=move |event| editor_state.update(|state| state.set_text(write_id.clone(), event_target_value(&event))) />
            }.into_any()
        }
        "select" => {
            let read_id = attribute_id.clone();
            let write_id = attribute_id.clone();
            view! {
                <select class="w-full rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground outline-none focus:border-primary" prop:value=move || editor_state.get().selected_option(&read_id) on:change=move |event| editor_state.update(|state| state.set_select(write_id.clone(), event_target_value(&event)))>
                    <option value="">{empty_option_label}</option>
                    {attribute.options.into_iter().map(|option| view! { <option value=option.id>{option.label}</option> }).collect_view()}
                </select>
            }.into_any()
        }
        "multiselect" => view! {
            <div class="grid gap-2">
                {attribute.options.into_iter().map(|option| {
                    let read_id = attribute_id.clone();
                    let write_id = attribute_id.clone();
                    let read_option_id = option.id.clone();
                    let write_option_id = option.id.clone();
                    view! {
                        <label class="flex items-center gap-2 text-sm text-foreground">
                            <input type="checkbox" prop:checked=move || editor_state.get().option_selected(&read_id, &read_option_id) on:change=move |event| editor_state.update(|state| state.set_multiselect_option(write_id.clone(), write_option_id.clone(), event_target_checked(&event))) />
                            <span>{option.label}</span>
                        </label>
                    }
                }).collect_view()}
            </div>
        }.into_any(),
        "json" => {
            let read_id = attribute_id.clone();
            let write_id = attribute_id.clone();
            view! {
                <textarea class="min-h-24 w-full rounded-lg border border-border bg-background px-3 py-2 font-mono text-sm text-foreground outline-none focus:border-primary" prop:value=move || editor_state.get().json(&read_id) on:input=move |event| editor_state.update(|state| state.set_json(write_id.clone(), event_target_value(&event))) />
            }.into_any()
        }
        _ => ().into_any(),
    };

    view! {
        <label class="grid gap-2 text-sm text-foreground">
            <span class="flex items-center gap-2 font-medium">
                {attribute.label}
                <Show when=move || attribute.is_required>
                    <span class="text-xs font-normal text-destructive">{required_label.clone()}</span>
                </Show>
            </span>
            {input}
        </label>
    }
}

#[component]
pub fn ProductAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let effective_locale = ui_locale.clone();
    let editor_copy = build_product_admin_editor_copy(effective_locale.as_deref());
    let attribute_form_copy = build_product_attribute_form_copy(effective_locale.as_deref());
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
    let (primary_category_id, set_primary_category_id) = signal(String::new());
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
    let attribute_editor_state = RwSignal::new(ProductAttributeEditorState::default());
    let effective_locale_for_products = effective_locale.clone();
    let effective_locale_for_categories = effective_locale.clone();
    let effective_locale_for_effective_form = effective_locale.clone();
    let effective_locale_for_attribute_values = effective_locale.clone();
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
    let catalog_categories = local_resource(
        move || {
            (
                token.get(),
                tenant.get(),
                refresh_nonce.get(),
                effective_locale_for_categories.clone(),
            )
        },
        move |(token_value, tenant_value, _, locale_value)| async move {
            let bootstrap =
                transport::fetch_bootstrap(token_value.clone(), tenant_value.clone()).await?;
            let locale = locale_value.unwrap_or_default();
            transport::fetch_catalog_categories(
                token_value,
                tenant_value,
                bootstrap.current_tenant.id,
                locale,
            )
            .await
        },
    );
    let effective_form = local_resource(
        move || {
            let category_id = text_or_none(primary_category_id.get());
            let selected_product = selected.get();
            let product_id = selected_product
                .as_ref()
                .filter(|product| product.primary_category_id.as_deref() == category_id.as_deref())
                .map(|product| product.id.clone());
            (
                token.get(),
                tenant.get(),
                refresh_nonce.get(),
                effective_locale_for_effective_form.clone(),
                product_id,
                category_id,
            )
        },
        move |(token_value, tenant_value, _, locale_value, product_id, category_id)| async move {
            if product_id.is_none() && category_id.is_none() {
                return Ok(None);
            }
            let bootstrap =
                transport::fetch_bootstrap(token_value.clone(), tenant_value.clone()).await?;
            transport::fetch_effective_product_form(
                token_value,
                tenant_value,
                bootstrap.current_tenant.id,
                product_id,
                category_id,
                locale_value.unwrap_or_default(),
            )
            .await
        },
    );
    let attribute_values = local_resource(
        move || {
            (
                token.get(),
                tenant.get(),
                refresh_nonce.get(),
                effective_locale_for_attribute_values.clone(),
                selected.get().map(|product| product.id),
            )
        },
        move |(token_value, tenant_value, _, locale_value, product_id)| async move {
            let Some(product_id) = product_id else {
                return Ok(Vec::new());
            };
            let bootstrap = transport::fetch_bootstrap(token_value.clone(), tenant_value.clone())
                .await
                .map_err(|error| error.to_string())?;
            transport::fetch_product_attribute_values(
                token_value,
                tenant_value,
                bootstrap.current_tenant.id,
                product_id,
                locale_value.unwrap_or_default(),
            )
            .await
            .map_err(|error| error.to_string())
        },
    );
    Effect::new(move |_| {
        let _selected_product_id = selected.get().map(|product| product.id);
        attribute_editor_state.set(ProductAttributeEditorState::default());
    });
    Effect::new(move |_| {
        if let Some(Ok(values)) = attribute_values.get() {
            attribute_editor_state.set(ProductAttributeEditorState::from_values(values));
        }
    });
    let selected_pricing = local_resource(
        move || {
            (
                token.get(),
                tenant.get(),
                refresh_nonce.get(),
                effective_locale_for_selected_pricing.clone(),
                selected
                    .get()
                    .map(|product| pricing_preview_request_from_product(&product)),
            )
        },
        move |(token_value, tenant_value, _, locale_value, selected_product)| async move {
            let Some(request) = selected_product else {
                return Ok(None);
            };
            let bootstrap = transport::fetch_bootstrap(token_value.clone(), tenant_value.clone())
                .await
                .map_err(|err| err.to_string())?;
            transport::fetch_product_pricing(
                token_value,
                tenant_value,
                bootstrap.current_tenant.id,
                request.product_id,
                locale_value,
                Some(request.currency_code),
            )
            .await
            .map_err(|err| err.to_string())
        },
    );

    let error_copy = build_product_admin_error_copy(ui_locale.as_deref());
    let initial_error_copy = error_copy.clone();
    Effect::new(move |_| {
        match product_admin_selected_product_query_state(selected_product_query.get()) {
            ProductAdminSelectedProductQueryState::Open { product_id } => {
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
                    set_primary_category_id,
                    set_sku,
                    set_barcode,
                    set_currency_code,
                    set_amount,
                    set_compare_at_amount,
                    set_inventory_quantity,
                    set_publish_now,
                );
            }
            ProductAdminSelectedProductQueryState::Clear => clear_product_form(
                set_editing_id,
                set_selected,
                set_title,
                set_handle,
                set_description,
                set_seller_id,
                set_vendor,
                set_product_type,
                set_shipping_profile_slug,
                set_primary_category_id,
                set_sku,
                set_barcode,
                set_currency_code,
                set_amount,
                set_compare_at_amount,
                set_inventory_quantity,
                set_publish_now,
            ),
        }
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
            set_primary_category_id,
            set_sku,
            set_barcode,
            set_currency_code,
            set_amount,
            set_compare_at_amount,
            set_inventory_quantity,
            set_publish_now,
        );
        attribute_editor_state.set(ProductAttributeEditorState::default());
        set_error.set(None);
    };

    let submit_ui_locale = ui_locale.clone();
    let submit_query_writer = query_writer.clone();
    let error_copy_for_submit_base = error_copy.clone();
    let on_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        let submit_query_writer = submit_query_writer.clone();
        let submit_locale = submit_ui_locale.clone();
        let command = build_save_command(
            DraftForm {
                locale: submit_locale.clone(),
                title: title.get_untracked(),
                handle: handle.get_untracked(),
                description: description.get_untracked(),
                seller_id: seller_id.get_untracked(),
                vendor: vendor.get_untracked(),
                product_type: product_type.get_untracked(),
                shipping_profile_slug: shipping_profile_slug.get_untracked(),
                primary_category_id: primary_category_id.get_untracked(),
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
        let attribute_types = effective_form
            .get_untracked()
            .and_then(Result::ok)
            .flatten()
            .map(|form| {
                form.attributes
                    .into_iter()
                    .map(|attribute| (attribute.attribute_id, attribute.value_type))
                    .collect::<std::collections::HashMap<_, _>>()
            })
            .unwrap_or_default();
        let attribute_patches = match attribute_editor_state
            .get_untracked()
            .patches(submit_ui_locale.as_deref(), &attribute_types)
        {
            Ok(patches) => patches,
            Err(message) => {
                set_error.set(Some(message));
                return;
            }
        };

        set_busy.set(true);
        set_error.set(None);

        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let attribute_tenant_id = command.tenant_id.clone();
        let attribute_actor_id = command.actor_id.clone();

        let error_copy_for_submit = error_copy_for_submit_base.clone();
        spawn_local(async move {
            let submit_locale = command.draft.locale.clone();
            let result = match command.mode {
                SaveMode::Update { product_id } => {
                    transport::update_product(
                        token_value.clone(),
                        tenant_value.clone(),
                        command.tenant_id,
                        command.actor_id,
                        product_id,
                        command.draft,
                    )
                    .await
                }
                SaveMode::Create => {
                    transport::create_product(
                        token_value.clone(),
                        tenant_value.clone(),
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
                    let attribute_result = if attribute_patches.is_empty() {
                        Ok(Vec::new())
                    } else {
                        transport::save_product_attribute_values(
                            token_value,
                            tenant_value,
                            attribute_tenant_id,
                            attribute_actor_id,
                            product_id.clone(),
                            submit_locale.clone(),
                            attribute_patches,
                        )
                        .await
                        .map_err(|error| error.to_string())
                    };
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
                        set_primary_category_id,
                        set_sku,
                        set_barcode,
                        set_currency_code,
                        set_amount,
                        set_compare_at_amount,
                        set_inventory_quantity,
                        set_publish_now,
                    );
                    match attribute_result {
                        Ok(values) if !values.is_empty() => attribute_editor_state
                            .set(ProductAttributeEditorState::from_values(values)),
                        Ok(_) => {}
                        Err(detail) => {
                            set_error.set(Some(error_copy_for_submit.save_product_failure(detail)))
                        }
                    }
                    set_refresh_nonce.update(|value| *value += 1);
                    submit_query_writer
                        .apply_query_intent(product_admin_saved_product_query_intent(product_id));
                }
                Err(err) => set_error.set(Some(error_copy_for_submit.save_product_failure(err))),
            }

            set_busy.set(false);
        });
    };

    let clear_detached_ui_locale = ui_locale.clone();
    let error_copy_for_detached = error_copy.clone();
    let clear_detached_values = move |attribute_ids: Vec<String>| {
        let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
            set_error.set(Some(error_copy_for_detached.bootstrap_loading.clone()));
            return;
        };
        let Some(product_id) = selected.get_untracked().map(|product| product.id) else {
            return;
        };
        set_busy.set(true);
        set_error.set(None);
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let locale = clear_detached_ui_locale.clone().unwrap_or_default();
        let error_copy = error_copy_for_detached.clone();
        spawn_local(async move {
            match transport::clear_detached_product_attribute_values(
                token_value,
                tenant_value,
                bootstrap.current_tenant.id,
                bootstrap.me.id,
                product_id,
                locale,
                attribute_ids,
            )
            .await
            {
                Ok(values) => {
                    attribute_editor_state.set(ProductAttributeEditorState::from_values(values));
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(error_copy.save_product_failure(err))),
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
    let pricing_module_route_base = route_context.module_route_base("pricing");
    let list_query_writer = query_writer.clone();
    let reset_query_writer = query_writer.clone();
    let delete_query_writer = query_writer.clone();
    let reset_current_product = Callback::new(move |_| {
        reset_query_writer.apply_query_intent(product_admin_clear_product_query_intent());
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
                        {move || match product_admin_products_load_view_from_result(
                            ui_locale_for_list.as_deref(),
                            products.get(),
                        ) {
                            ProductAdminProductsLoadViewModel::State(state) => {
                                view! {
                                    <div class=state.container_class>
                                        {state.message}
                                    </div>
                                }.into_any()
                            },
                            ProductAdminProductsLoadViewModel::Ready(items) => view! {
                                <>
                                    {items.into_iter().map(|product| {
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
                                        let item_shipping_profile_label =
                                            item_view_model.shipping_profile_label.clone();
                                        let show_shipping_profile =
                                            item_view_model.show_shipping_profile;
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
                                                            <span class=item_status_badge_class>
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
                                                                {item_shipping_profile_label.clone()}
                                                            </span>
                                                        </Show>
                                                        <p class="text-xs text-muted-foreground">
                                                            {item_timestamp_label.clone()}
                                                        </p>
                                                    </div>
                                                    <div class="flex flex-wrap gap-2">
                                                        <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || product_admin_list_actions_disabled(busy.get()) on:click=move |_| item_query_writer.apply_query_intent(product_admin_open_product_query_intent(edit_id.clone()))>
                                                            {edit_label.clone()}
                                                        </button>
                                                        <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || product_admin_list_actions_disabled(busy.get()) on:click=move |_| mutate_status(
                                                            bootstrap.get_untracked().and_then(Result::ok),
                                                            token.get_untracked(),
                                                            tenant.get_untracked(),
                                                            publish_id.clone(),
                                                            StatusTarget::Active,
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
                                                            StatusTarget::Draft,
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
                                                            StatusTarget::Archived,
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
                                                            set_primary_category_id,
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
                                <select class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" prop:value=move || primary_category_id.get() on:change=move |ev| set_primary_category_id.set(event_target_value(&ev))>
                                    <option value="">{editor_copy.primary_category_placeholder.clone()}</option>
                                    {move || catalog_categories
                                        .get()
                                        .and_then(Result::ok)
                                        .map(|list| {
                                            list.items
                                                .into_iter()
                                                .filter(|category| category.kind == "structural")
                                                .map(|category| {
                                                    let label = if category.path.trim().is_empty() {
                                                        category.name
                                                    } else {
                                                        format!("{} / {}", category.path, category.name)
                                                    };
                                                    view! { <option value=category.id>{label}</option> }
                                                })
                                                .collect_view()
                                        })
                                        .unwrap_or_default()
                                    }
                                </select>
                            </div>
                            <div class="border-y border-border py-4">
                                {
                                    let ui_locale = ui_locale.clone();
                                    move || {
                                        let attribute_form_copy = attribute_form_copy.clone();
                                        let ui_locale = ui_locale.clone();
                                    match effective_form.get() {
                                    None => {
                                        let loading = attribute_form_copy.loading.clone();
                                        view! {
                                            <p class="text-xs text-muted-foreground">{loading}</p>
                                        }.into_any()
                                    },
                                    Some(Err(err)) => {
                                        let load_failure = attribute_form_copy.load_failure(err);
                                        view! {
                                            <p class="text-xs text-destructive">{load_failure}</p>
                                        }.into_any()
                                    },
                                    Some(Ok(None)) => {
                                        let select_category = attribute_form_copy.select_category.clone();
                                        view! {
                                            <p class="text-xs text-muted-foreground">{select_category}</p>
                                        }.into_any()
                                    },
                                    Some(Ok(Some(form))) if form.attributes.is_empty() => {
                                        let no_attributes = attribute_form_copy.no_attributes.clone();
                                        view! {
                                            <p class="text-xs text-muted-foreground">{no_attributes}</p>
                                        }.into_any()
                                    },
                                    Some(Ok(Some(form))) => {
                                        let detached_count = form.detached_attribute_ids.len();
                                        let detached_title = attribute_form_copy.detached_title.clone();
                                        let detached_values_label = attribute_form_copy.detached_values(detached_count);
                                        let clear_detached_label = attribute_form_copy.clear_detached_label.clone();
                                        let detached_empty_label = attribute_form_copy.detached_empty_label.clone();
                                        let ui_locale = ui_locale.clone();
                                        let clear_detached_values = clear_detached_values.clone();
                                        let mut groups: Vec<(String, Vec<ProductEffectiveFormAttribute>)> = Vec::new();
                                        for attribute in form.attributes.into_iter().filter(|item| !item.is_disabled) {
                                            let group = attribute
                                                .group_label
                                                .clone()
                                                .or_else(|| attribute.group_code.clone())
                                                .unwrap_or_else(|| attribute_form_copy.ungrouped_label.clone());
                                            if let Some((_, attributes)) = groups.iter_mut().find(|(code, _)| code == &group) {
                                                attributes.push(attribute);
                                            } else {
                                                groups.push((group, vec![attribute]));
                                            }
                                        }
                                        view! {
                                            <div class="space-y-6">
                                                {groups.into_iter().map(|(group, attributes)| view! {
                                                    <section class="space-y-3">
                                                        <h4 class="text-sm font-semibold text-foreground">{group}</h4>
                                                        <div class="grid gap-4 md:grid-cols-2">
                                                            {attributes.into_iter().map(|attribute| view! {
                                                                <TypedProductAttributeField
                                                            attribute=attribute
                                                            editor_state=attribute_editor_state
                                                                    required_label=attribute_form_copy.required_label.clone()
                                                                    empty_option_label=attribute_form_copy.empty_option_label.clone()
                                                                    boolean_true_label=attribute_form_copy.boolean_true_label.clone()
                                                                    boolean_false_label=attribute_form_copy.boolean_false_label.clone()
                                                                />
                                                            }).collect_view()}
                                                        </div>
                                                    </section>
                                                }).collect_view()}
                                                <Show when=move || { detached_count > 0 }>
                                                    {
                                                        let detached_title = detached_title.clone();
                                                        let detached_values_label = detached_values_label.clone();
                                                        let clear_detached_label = clear_detached_label.clone();
                                                        let clear_detached_values = clear_detached_values.clone();
                                                        let ui_locale = ui_locale.clone();
                                                        let detached_empty_label = detached_empty_label.clone();
                                                        view! {
                                                            <div class="rounded-xl border border-dashed border-border bg-muted/30 p-3">
                                                                <div class="flex flex-wrap items-center justify-between gap-3">
                                                                    <div>
                                                                        <h4 class="text-sm font-semibold text-foreground">{detached_title.clone()}</h4>
                                                                        <p class="text-xs text-muted-foreground">{detached_values_label.clone()}</p>
                                                                    </div>
                                                                    <button
                                                                        type="button"
                                                                        class="rounded-lg border border-border px-3 py-2 text-xs font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                                                                        disabled=move || busy.get()
                                                                        on:click={
                                                                            let clear_detached_values = clear_detached_values.clone();
                                                                            move |_| {
                                                                                let ids = attribute_values
                                                                                    .get()
                                                                                    .and_then(Result::ok)
                                                                                    .unwrap_or_default()
                                                                                    .into_iter()
                                                                                    .filter(|value| value.detached)
                                                                                    .map(|value| value.attribute_id)
                                                                                    .collect::<Vec<_>>();
                                                                                clear_detached_values(ids);
                                                                            }
                                                                        }
                                                                    >
                                                                        {clear_detached_label.clone()}
                                                                    </button>
                                                                </div>
                                                                <div class="mt-3 grid gap-2">
                                                                    {move || {
                                                                        let ui_locale = ui_locale.clone();
                                                                        let detached_empty_label = detached_empty_label.clone();
                                                                        let values = attribute_values
                                                                            .get()
                                                                            .and_then(Result::ok)
                                                                            .map(|values| build_product_detached_attribute_value_view_models(ui_locale.as_deref(), &values))
                                                                            .unwrap_or_default();
                                                                        if values.is_empty() {
                                                                            view! { <p class="text-xs text-muted-foreground">{detached_empty_label}</p> }.into_any()
                                                                        } else {
                                                                            view! {
                                                                                <div class="grid gap-2">
                                                                                    {values.into_iter().map(|value| view! {
                                                                                        <div class="rounded-lg border border-border bg-background px-3 py-2 text-xs">
                                                                                            <p class="font-medium text-foreground">{value.label}</p>
                                                                                            <p class="mt-1 break-all text-muted-foreground">{value.value}</p>
                                                                                        </div>
                                                                                    }).collect_view()}
                                                                                </div>
                                                                            }.into_any()
                                                                        }
                                                                    }}
                                                                </div>
                                                            </div>
                                                        }
                                                    }
                                                </Show>
                                            </div>
                                        }.into_any()
                                    }
                                    }
                                }}
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
                                    {move || shipping_profiles_load_view_from_result(
                                        ui_locale_for_profiles.as_deref(),
                                        shipping_profiles.get(),
                                    )
                                        .options
                                        .into_iter()
                                        .map(|option| view! { <option value=option.value>{option.label}</option> })
                                        .collect_view()
                                    }
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
                            {move || shipping_profiles_load_view_from_result(
                                ui_locale_for_profile_panel.as_deref(),
                                shipping_profiles.get(),
                            ).panel.into_message()}
                        </div>
                    </section>

                    <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                        {
                            let summary_copy = build_product_admin_summary_panel_copy(ui_locale.as_deref());
                            view! {
                        <h3 class="text-lg font-semibold text-card-foreground">
                            {summary_copy.title}
                        </h3>
                            }
                        }
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
    set_primary_category_id: WriteSignal<String>,
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
                set_selected.set(Some(*product));
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
                    set_primary_category_id,
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
                    set_primary_category_id,
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
    set_primary_category_id: WriteSignal<String>,
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
        set_primary_category_id,
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
    set_primary_category_id: WriteSignal<String>,
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
        set_primary_category_id,
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
    set_primary_category_id: WriteSignal<String>,
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
    set_primary_category_id.set(state.primary_category_id);
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
    status: StatusTarget,
    locale: Option<String>,
    set_busy: WriteSignal<bool>,
    set_error: WriteSignal<Option<String>>,
    set_refresh_nonce: WriteSignal<u64>,
) {
    let command = match build_status_command(bootstrap.as_ref(), product_id, status) {
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
            Ok(_) => StatusOutcome::Changed,
            Err(err) => StatusOutcome::TransportError(err.to_string()),
        };
        let view_model = build_status_result_view_model(locale.as_deref(), outcome);

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
    set_primary_category_id: WriteSignal<String>,
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
    let command = match build_delete_command(bootstrap.as_ref(), product_id) {
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
            Ok(true) => DeleteOutcome::Deleted,
            Ok(false) => DeleteOutcome::NotDeleted,
            Err(err) => DeleteOutcome::TransportError(err.to_string()),
        };
        let view_model = build_delete_result_view_model(
            locale.as_deref(),
            deleted_product_id.as_str(),
            editing_id.get_untracked().as_deref(),
            outcome,
        );

        if view_model.clear_selection {
            query_writer.apply_query_intent(product_admin_clear_product_query_intent());
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
                set_primary_category_id,
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
    let pricing_state = pricing_preview_state_from_result(pricing_state.as_ref());

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
