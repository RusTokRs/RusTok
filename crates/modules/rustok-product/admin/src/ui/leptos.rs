use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui_routing::{RouteQueryWriter, use_route_query_value, use_route_query_writer};
use rustok_grid::{ColumnFilters, GridPagination, RowSelection};
use rustok_grid_leptos::prelude::*;
use rustok_seo_panel::SeoEntityPanel;
use rustok_seo_targets::{SeoTargetSlug, builtin_slug as seo_builtin_slug};
use rustok_ui_core::{AdminQueryKey, UiRouteContext};

use crate::catalog_transport;
use crate::core::{
    DeleteOutcome, DraftForm, ProductAdminEditorFormState, ProductAdminErrorCopy,
    ProductAdminOpenProductViewModel, ProductAdminProductsLoadViewModel,
    ProductAdminSelectedProductQueryState, ProductAttributeEditorState, SaveMode,
    SelectedProductSummaryViewModel, StatusOutcome, StatusTarget, VariantRowViewModel,
    build_delete_command, build_delete_result_view_model, build_product_admin_editor_copy,
    build_product_admin_editor_form_state, build_product_admin_editor_view_model,
    build_product_admin_error_copy, build_product_admin_list_action_labels,
    build_product_admin_list_controls_view_model, build_product_admin_list_item_view_model,
    build_product_admin_open_product_view_model, build_product_admin_seo_panel_copy,
    build_product_admin_shell_view_model, build_product_admin_summary_panel_copy,
    build_product_attribute_form_copy, build_product_attribute_values_section_copy,
    build_product_detached_attribute_value_view_models, build_product_image_view_models,
    build_product_media_panel_copy, build_product_variants_panel_copy, build_save_command,
    build_selected_product_summary_view_model, build_status_command,
    build_status_result_view_model, build_variant_row_view_models,
    empty_product_admin_editor_form_state, parse_product_admin_inventory_quantity_input,
    pricing_preview_request_from_product, pricing_preview_state_from_result,
    product_admin_clear_product_query_intent, product_admin_list_actions_disabled,
    product_admin_open_product_query_intent, product_admin_products_load_view_from_result,
    product_admin_saved_product_query_intent, product_admin_selected_product_query_state,
    shipping_profiles_load_view_from_result, text_or_none,
};
use crate::model::{
    BindCategoryAttributeDraft, BindSchemaAttributeDraft, CatalogCategorySummary,
    CategoryAttributeGroupDraft, ProductAdminBootstrap, ProductAttributeSchemaGroupDraft,
    ProductAttributeSchemaSummary, ProductAttributeSummary, ProductAttributeValueItem,
    ProductDetail, ProductEffectiveForm, ProductEffectiveFormAttribute, ProductImageDraft,
    ProductPricingDetail, SetCategorySchemaModeDraft, SetVariantAxesDraft, UpdateProductImageDraft,
    VariantAxisDraft, VariantDraft, VariantPriceDraft,
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
    saved_option_ids: Vec<String>,
    missing_option_suffix: String,
) -> impl IntoView {
    let attribute_id = attribute.attribute_id.clone();
    let value_type = attribute.value_type.clone();
    // Dictionary options are resolved from the schema, but a stored value can
    // reference an option that is no longer reachable from it (removed or
    // deactivated). Such references are surfaced explicitly so that an operator
    // still sees the current value and, for multi-selects, can clear it.
    let options = attribute.options.clone();
    let dangling_options = saved_option_ids
        .iter()
        .filter(|option_id| {
            !option_id.is_empty() && !options.iter().any(|option| &option.id == *option_id)
        })
        .map(|option_id| {
            (
                option_id.clone(),
                format!("{option_id} {missing_option_suffix}"),
            )
        })
        .collect::<Vec<(String, String)>>();
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
            let options = options.clone();
            let dangling_options = dangling_options.clone();
            view! {
                <select class="w-full rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground outline-none focus:border-primary" prop:value=move || editor_state.get().selected_option(&read_id) on:change=move |event| editor_state.update(|state| state.set_select(write_id.clone(), event_target_value(&event)))>
                    <option value="">{empty_option_label}</option>
                    {options.into_iter().map(|option| view! { <option value=option.id>{option.label}</option> }).collect_view()}
                    {dangling_options.into_iter().map(|(option_id, label)| view! { <option value=option_id>{label}</option> }).collect_view()}
                </select>
            }.into_any()
        }
        "multiselect" => {
            let rows = options
                .into_iter()
                .map(|option| (option.id.clone(), option.label.clone()))
                .chain(dangling_options.into_iter())
                .collect::<Vec<(String, String)>>();
            view! {
                <div class="grid gap-2">
                    {rows.into_iter().map(|(option_id, label)| {
                        let read_id = attribute_id.clone();
                        let write_id = attribute_id.clone();
                        let read_option_id = option_id.clone();
                        let write_option_id = option_id.clone();
                        view! {
                            <label class="flex items-center gap-2 text-sm text-foreground">
                                <input type="checkbox" prop:checked=move || editor_state.get().option_selected(&read_id, &read_option_id) on:change=move |event| editor_state.update(|state| state.set_multiselect_option(write_id.clone(), write_option_id.clone(), event_target_checked(&event))) />
                                <span>{label}</span>
                            </label>
                        }
                    }).collect_view()}
                </div>
            }.into_any()
        }
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

/// Non-mounted single-screen reference composition.
///
/// The host mounts `ui::root::ProductAdmin` (routed pages), not this component.
/// It stays in the tree because it is the canonical catalog-controls reference
/// pinned by the product catalog verification suite, and because the shared
/// sections below it are mounted by the routed pages. Feature work belongs on
/// the routed pages and the shared sections; do not grow this composition.
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

    let reload_current_product = {
        let effective_locale_for_reload = effective_locale.clone();
        let error_copy_for_reload = error_copy.clone();
        move |product_id: String| {
            if let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) {
                open_product_for_edit(
                    bootstrap,
                    token.get_untracked(),
                    tenant.get_untracked(),
                    effective_locale_for_reload.clone(),
                    product_id,
                    error_copy_for_reload.clone(),
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
        }
    };

    let on_variant_mutated = {
        let reload = reload_current_product.clone();
        Callback::new(move |product_id: String| {
            set_refresh_nonce.update(|value| *value += 1);
            reload(product_id);
        })
    };

    let on_media_mutated = {
        let reload = reload_current_product.clone();
        Callback::new(move |product_id: String| {
            set_refresh_nonce.update(|value| *value += 1);
            reload(product_id);
        })
    };

    let submit_ui_locale = ui_locale.clone();
    let submit_query_writer = query_writer.clone();
    let error_copy_for_submit_base = error_copy.clone();
    let on_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        let submit_query_writer = submit_query_writer.clone();
        let submit_locale = submit_ui_locale.clone();
        let submit_revision = selected.get_untracked().map(|product| product.revision);
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
                revision: submit_revision,
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

                    <Show when=move || selected.get().is_some()>
                        <ProductVariantsPanel
                            locale=ui_locale.clone()
                            product=selected.get()
                            token=token
                            tenant=tenant
                            bootstrap=bootstrap
                            busy=busy
                            set_busy=set_busy
                            set_error=set_error
                            on_variant_mutated=on_variant_mutated
                        />
                        <ProductMediaPanel
                            locale=ui_locale.clone()
                            product=selected.get()
                            token=token
                            tenant=tenant
                            bootstrap=bootstrap
                            busy=busy
                            set_busy=set_busy
                            set_error=set_error
                            on_media_mutated=on_media_mutated
                        />
                        {
                            let product_id = selected.get().map(|p| p.id).unwrap_or_default();
                            view! {
                                <rustok_product_relations_admin::ProductRelationsPanel
                                    locale=ui_locale.clone().unwrap_or_else(|| "en".to_string())
                                    product_id=product_id
                                    token=token
                                    tenant=tenant
                                    busy=busy
                                    set_busy=set_busy
                                    set_error=set_error
                                />
                            }
                        }
                    </Show>

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

#[component]
fn ProductVariantsPanel(
    locale: Option<String>,
    product: Option<ProductDetail>,
    token: Signal<Option<String>>,
    tenant: Signal<Option<String>>,
    bootstrap: LocalResource<Result<ProductAdminBootstrap, rustok_graphql::GraphqlHttpError>>,
    busy: ReadSignal<bool>,
    set_busy: WriteSignal<bool>,
    set_error: WriteSignal<Option<String>>,
    on_variant_mutated: Callback<String>,
) -> impl IntoView {
    let copy = build_product_variants_panel_copy(locale.as_deref());
    let (show_add, set_show_add) = signal(false);
    let (sku, set_sku) = signal(String::new());
    let (barcode, set_barcode) = signal(String::new());
    let (selected_axis_values, set_selected_axis_values) = signal(Vec::<(String, String)>::new());
    let (amount, set_amount) = signal(String::new());
    let (inventory_quantity, set_inventory_quantity) = signal(0_i32);
    let (inventory_policy, set_inventory_policy) = signal("DENY".to_string());
    let (editing_variant_id, set_editing_variant_id) = signal(Option::<String>::None);
    let (edit_sku, set_edit_sku) = signal(String::new());
    let (edit_price, set_edit_price) = signal(String::new());
    let (edit_stock, set_edit_stock) = signal(0_i32);

    let Some(product) = product else {
        return view! { <div /> }.into_any();
    };

    let product_id = product.id.clone();
    let variant_rows = build_variant_row_view_models(&product);
    let default_currency = product
        .variants
        .first()
        .and_then(|v| v.prices.first())
        .map(|p| p.currency_code.clone())
        .unwrap_or_else(|| "USD".to_string());
    let default_currency_for_form = default_currency.clone();

    let on_add_submit = {
        let product_id_val = product_id.clone();
        let default_currency_for_add = default_currency.clone();
        let on_mutated = on_variant_mutated;
        move |ev: SubmitEvent| {
            ev.prevent_default();
            let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
                return;
            };
            let token_val = token.get_untracked();
            let tenant_val = tenant.get_untracked();
            let product_id_val = product_id_val.clone();

            let price_val = amount.get_untracked().trim().to_string();
            let price_val = if price_val.is_empty() {
                "0.00".to_string()
            } else {
                price_val
            };

            let axis_values = selected_axis_values
                .get_untracked()
                .into_iter()
                .map(
                    |(attribute_id, option_id)| crate::model::VariantAxisValueDraft {
                        attribute_id,
                        option_id,
                    },
                )
                .collect();

            let draft = VariantDraft {
                sku: text_or_none(sku.get_untracked()),
                barcode: text_or_none(barcode.get_untracked()),
                shipping_profile_slug: None,
                axis_values,
                prices: vec![VariantPriceDraft {
                    currency_code: default_currency_for_add.clone(),
                    amount: price_val,
                    compare_at_amount: None,
                }],
                inventory_quantity: Some(inventory_quantity.get_untracked()),
                inventory_policy: Some(inventory_policy.get_untracked()),
            };

            set_busy.set(true);
            set_error.set(None);
            spawn_local(async move {
                match transport::create_product_variant(
                    token_val,
                    tenant_val,
                    bootstrap.current_tenant.id,
                    bootstrap.me.id,
                    product_id_val.clone(),
                    draft,
                )
                .await
                {
                    Ok(_) => {
                        set_sku.set(String::new());
                        set_barcode.set(String::new());
                        set_selected_axis_values.set(Vec::new());
                        set_amount.set(String::new());
                        set_inventory_quantity.set(0);
                        set_show_add.set(false);
                        on_mutated.run(product_id_val);
                    }
                    Err(err) => set_error.set(Some(err.to_string())),
                }
                set_busy.set(false);
            });
        }
    };

    view! {
        <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
            <div class="flex items-center justify-between gap-3">
                <div>
                    <h3 class="text-lg font-semibold text-card-foreground">{copy.title}</h3>
                    <p class="text-sm text-muted-foreground">{copy.subtitle}</p>
                </div>
                <button
                    type="button"
                    class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                    disabled=move || busy.get()
                    on:click=move |_| set_show_add.update(|v| *v = !*v)
                >
                    {copy.add}
                </button>
            </div>

            <Show when=move || show_add.get()>
                <form class="mt-4 rounded-2xl border border-border/70 bg-background/50 p-4 space-y-3" on:submit={
                    let on_submit = on_add_submit.clone();
                    move |ev| on_submit(ev)
                }>
                    <div class="grid gap-3 md:grid-cols-2">
                        <input
                            class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                            placeholder="SKU"
                            prop:value=move || sku.get()
                            on:input=move |ev| set_sku.set(event_target_value(&ev))
                        />
                        <input
                            class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                            placeholder="Barcode"
                            prop:value=move || barcode.get()
                            on:input=move |ev| set_barcode.set(event_target_value(&ev))
                        />
                    </div>
                    {if !product.variant_axes.is_empty() {
                        view! {
                            <div class="grid gap-3 md:grid-cols-2">
                                {product.variant_axes.iter().map(|axis| {
                                    let attr_id = axis.attribute_id.clone();
                                    let attr_name = axis.name.clone();
                                    let allowed = axis.allowed_values.clone();
                                    view! {
                                        <div>
                                            <label class="block text-xs font-medium text-muted-foreground mb-1">{attr_name.clone()}</label>
                                            <select
                                                class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                                                on:change={
                                                    let attr_id = attr_id.clone();
                                                    move |ev| {
                                                        let val = event_target_value(&ev);
                                                        set_selected_axis_values.update(|entries| {
                                                            entries.retain(|(aid, _)| aid != &attr_id);
                                                            if !val.is_empty() {
                                                                entries.push((attr_id.clone(), val));
                                                            }
                                                        });
                                                    }
                                                }
                                            >
                                                <option value="">{format!("Select {}", attr_name)}</option>
                                                {allowed.into_iter().map(|opt| {
                                                    view! {
                                                        <option value=opt.option_id>{opt.value}</option>
                                                    }
                                                }).collect_view()}
                                            </select>
                                        </div>
                                    }
                                }).collect_view()}
                            </div>
                        }.into_any()
                    } else {
                        view! { <div /> }.into_any()
                    }}
                    <div class="grid gap-3 md:grid-cols-3">
                        <input
                            class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                            placeholder=format!("Price ({})", default_currency_for_form)
                            prop:value=move || amount.get()
                            on:input=move |ev| set_amount.set(event_target_value(&ev))
                        />
                        <input
                            type="number"
                            class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                            placeholder="Stock quantity"
                            prop:value=move || inventory_quantity.get().to_string()
                            on:input=move |ev| set_inventory_quantity.set(parse_product_admin_inventory_quantity_input(&event_target_value(&ev)))
                        />
                        <select
                            class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                            prop:value=move || inventory_policy.get()
                            on:change=move |ev| set_inventory_policy.set(event_target_value(&ev))
                        >
                            <option value="DENY">"Deny backorders"</option>
                            <option value="CONTINUE">"Continue selling out of stock"</option>
                        </select>
                    </div>
                    <div class="flex justify-end gap-2 pt-2">
                        <button
                            type="button"
                            class="rounded-lg border border-border px-3 py-1.5 text-xs font-medium text-foreground transition hover:bg-accent"
                            on:click=move |_| set_show_add.set(false)
                        >
                            "Cancel"
                        </button>
                        <button
                            type="submit"
                            class="rounded-lg bg-primary px-4 py-1.5 text-xs font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50"
                            disabled=move || busy.get()
                        >
                            "Save variant"
                        </button>
                    </div>
                </form>
            </Show>

            {
                let columns = crate::core::product_variant_grid_columns(locale.as_deref());
                let filters = RwSignal::new(ColumnFilters::default());
                let selection = RwSignal::new(RowSelection::default());
                let pagination = RwSignal::new(GridPagination::new(1, 10, variant_rows.len() as u64));

                let filtered_rows = {
                    let rows = variant_rows.clone();
                    Memo::new(move |_| {
                        let f = filters.get();
                        crate::core::filter_product_variants(&rows, &f, None)
                    })
                };

                Effect::new(move |_| {
                    let total = filtered_rows.get().len() as u64;
                    pagination.update(|p| p.set_total(total));
                });

                let paged_rows = Memo::new(move |_| {
                    let list = filtered_rows.get();
                    let p = pagination.get();
                    let start = (p.page.saturating_sub(1)) * p.page_size;
                    list.into_iter().skip(start).take(p.page_size).collect::<Vec<_>>()
                });

                let on_filters_change = Callback::new(move |new_filters: ColumnFilters| {
                    filters.set(new_filters);
                });

                let cell_renderer = {
                    let product_id_for_del = product_id.clone();
                    let product_id_for_edit = product_id.clone();
                    let default_curr = default_currency.clone();
                    let on_mutated_del = on_variant_mutated;
                    let on_mutated_edit = on_variant_mutated;

                    Callback::new(move |(row, col_id): (VariantRowViewModel, String)| {
                        let variant_id = row.id.clone();
                        let can_delete = row.can_delete;
                        let is_editing_this = {
                            let v_id = variant_id.clone();
                            Signal::derive(move || editing_variant_id.get().as_deref() == Some(&v_id))
                        };

                        match col_id.as_str() {
                            "sku" => {
                                let sku_val = row.sku.clone();
                                view! {
                                    <div class="font-mono text-xs text-foreground">
                                        {move || if is_editing_this.get() {
                                            view! {
                                                <input
                                                    class="rounded border border-border bg-background px-2 py-1 text-xs outline-none focus:border-primary w-28"
                                                    prop:value=move || edit_sku.get()
                                                    on:input=move |ev| set_edit_sku.set(event_target_value(&ev))
                                                />
                                            }.into_any()
                                        } else {
                                            view! { <span>{sku_val.clone()}</span> }.into_any()
                                        }}
                                    </div>
                                }.into_any()
                            }
                            "options_summary" => view! {
                                <span class="text-foreground font-medium">{row.options_summary.clone()}</span>
                            }.into_any(),
                            "price" => {
                                let price_val = row.price.clone();
                                view! {
                                    <div class="text-foreground">
                                        {move || if is_editing_this.get() {
                                            view! {
                                                <input
                                                    class="rounded border border-border bg-background px-2 py-1 text-xs outline-none focus:border-primary w-20"
                                                    prop:value=move || edit_price.get()
                                                    on:input=move |ev| set_edit_price.set(event_target_value(&ev))
                                                />
                                            }.into_any()
                                        } else {
                                            view! { <span>{price_val.clone()}</span> }.into_any()
                                        }}
                                    </div>
                                }.into_any()
                            }
                            "stock" => {
                                let stock_val = row.stock.clone();
                                view! {
                                    <div class="text-foreground">
                                        {move || if is_editing_this.get() {
                                            view! {
                                                <input
                                                    type="number"
                                                    class="rounded border border-border bg-background px-2 py-1 text-xs outline-none focus:border-primary w-16"
                                                    prop:value=move || edit_stock.get().to_string()
                                                    on:input=move |ev| set_edit_stock.set(parse_product_admin_inventory_quantity_input(&event_target_value(&ev)))
                                                />
                                            }.into_any()
                                        } else {
                                            view! { <span>{stock_val.clone()}</span> }.into_any()
                                        }}
                                    </div>
                                }.into_any()
                            }
                            "inventory_policy" => view! {
                                <span class="inline-flex rounded-full px-2 py-0.5 text-xs font-medium border border-border bg-muted/50 text-muted-foreground">
                                    {row.inventory_policy.clone()}
                                </span>
                            }.into_any(),
                            "actions" => {
                                let v_id = variant_id.clone();
                                let s = row.sku.clone();
                                let p = row.price.clone();
                                let stock_for_edit = row.stock.parse::<i32>().unwrap_or_default();
                                let on_start_edit = move |_| {
                                    set_edit_sku.set(s.clone());
                                    set_edit_price.set(p.split_whitespace().next().unwrap_or("").to_string());
                                    set_edit_stock.set(stock_for_edit);
                                    set_editing_variant_id.set(Some(v_id.clone()));
                                };

                                let v_id_save = variant_id.clone();
                                let p_id_save = product_id_for_edit.clone();
                                let default_curr_save = default_curr.clone();
                                let on_save_edit = move |_| {
                                    let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
                                        return;
                                    };
                                    let token_val = token.get_untracked();
                                    let tenant_val = tenant.get_untracked();
                                    let var_id = v_id_save.clone();
                                    let prod_id = p_id_save.clone();

                                    let draft = VariantDraft {
                                        sku: text_or_none(edit_sku.get_untracked()),
                                        barcode: None,
                                        shipping_profile_slug: None,
                                        axis_values: Vec::new(),
                                        prices: vec![VariantPriceDraft {
                                            currency_code: default_curr_save.clone(),
                                            amount: edit_price.get_untracked(),
                                            compare_at_amount: None,
                                        }],
                                        inventory_quantity: Some(edit_stock.get_untracked()),
                                        inventory_policy: None,
                                    };

                                    set_busy.set(true);
                                    set_error.set(None);
                                    spawn_local(async move {
                                        match transport::update_product_variant(
                                            token_val,
                                            tenant_val,
                                            bootstrap.current_tenant.id,
                                            bootstrap.me.id,
                                            var_id,
                                            draft,
                                        ).await {
                                            Ok(_) => {
                                                set_editing_variant_id.set(None);
                                                on_mutated_edit.run(prod_id);
                                            }
                                            Err(err) => set_error.set(Some(err.to_string())),
                                        }
                                        set_busy.set(false);
                                    });
                                };

                                let v_id_del = variant_id.clone();
                                let p_id_del = product_id_for_del.clone();
                                let on_delete = move |_| {
                                    let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
                                        return;
                                    };
                                    let token_val = token.get_untracked();
                                    let tenant_val = tenant.get_untracked();
                                    let variant_id_val = v_id_del.clone();
                                    let product_id_val = p_id_del.clone();

                                    set_busy.set(true);
                                    set_error.set(None);
                                    spawn_local(async move {
                                        match transport::delete_product_variant(
                                            token_val,
                                            tenant_val,
                                            bootstrap.current_tenant.id,
                                            bootstrap.me.id,
                                            variant_id_val,
                                        ).await {
                                            Ok(_) => on_mutated_del.run(product_id_val),
                                            Err(err) => set_error.set(Some(err.to_string())),
                                        }
                                        set_busy.set(false);
                                    });
                                };

                                view! {
                                    <div class="inline-flex items-center gap-1 justify-end w-full">
                                        {move || if is_editing_this.get() {
                                            view! {
                                                <button
                                                    type="button"
                                                    class="inline-flex rounded-lg bg-primary px-2.5 py-1 text-xs font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50"
                                                    disabled=move || busy.get()
                                                    on:click=on_save_edit.clone()
                                                >
                                                    "Save"
                                                </button>
                                                <button
                                                    type="button"
                                                    class="inline-flex rounded-lg border border-border px-2 py-1 text-xs font-medium text-foreground transition hover:bg-accent"
                                                    on:click=move |_| set_editing_variant_id.set(None)
                                                >
                                                    "Cancel"
                                                </button>
                                            }.into_any()
                                        } else {
                                            view! {
                                                <button
                                                    type="button"
                                                    class="inline-flex rounded-lg border border-border px-2 py-1 text-xs font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                                                    disabled=move || busy.get()
                                                    on:click=on_start_edit.clone()
                                                >
                                                    "Edit"
                                                </button>
                                                <button
                                                    type="button"
                                                    class="inline-flex rounded-lg border border-rose-200 px-2.5 py-1 text-xs font-medium text-rose-700 transition hover:bg-rose-50 disabled:opacity-40 disabled:hover:bg-transparent"
                                                    disabled=move || !can_delete || busy.get()
                                                    title=if can_delete { "" } else { "Cannot delete the only variant of a product" }
                                                    on:click=on_delete.clone()
                                                >
                                                    "Delete"
                                                </button>
                                            }.into_any()
                                        }}
                                    </div>
                                }.into_any()
                            }
                            _ => ().into_any(),
                        }
                    })
                };

                view! {
                    <div class="mt-4">
                        <DataGrid
                            columns=columns
                            data=Signal::derive(move || paged_rows.get())
                            key_fn=|row: &VariantRowViewModel| row.id.clone()
                            cell_renderer=cell_renderer
                            empty_message=copy.empty.clone()
                            selection=selection
                            pagination=pagination
                            filters=filters
                            on_filter_change=on_filters_change
                            on_row_click=Callback::new(|_| ())
                        />
                    </div>
                }
            }
        </section>
    }.into_any()
}

#[component]
fn ProductMediaPanel(
    locale: Option<String>,
    product: Option<ProductDetail>,
    token: Signal<Option<String>>,
    tenant: Signal<Option<String>>,
    bootstrap: LocalResource<Result<ProductAdminBootstrap, rustok_graphql::GraphqlHttpError>>,
    busy: ReadSignal<bool>,
    set_busy: WriteSignal<bool>,
    set_error: WriteSignal<Option<String>>,
    on_media_mutated: Callback<String>,
) -> impl IntoView {
    let copy = build_product_media_panel_copy(locale.as_deref());
    let (show_add, set_show_add) = signal(false);
    let (media_id, set_media_id) = signal(String::new());
    let (alt_text, set_alt_text) = signal(String::new());
    let (position, set_position) = signal(0_i32);
    let (editing_image_id, set_editing_image_id) = signal(Option::<String>::None);
    let (edit_alt_text, set_edit_alt_text) = signal(String::new());

    let Some(product) = product else {
        return view! { <div /> }.into_any();
    };

    let product_id = product.id.clone();
    let image_models = build_product_image_view_models(&product);
    let current_image_ids: Vec<String> = product.images.iter().map(|img| img.id.clone()).collect();

    let on_add_submit = {
        let product_id_val = product_id.clone();
        let on_mutated = on_media_mutated;
        let locale_for_add = locale.clone();
        move |ev: SubmitEvent| {
            ev.prevent_default();
            let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
                return;
            };
            let token_val = token.get_untracked();
            let tenant_val = tenant.get_untracked();
            let product_id_val = product_id_val.clone();

            let media_id_val = media_id.get_untracked().trim().to_string();
            if media_id_val.is_empty() {
                set_error.set(Some("Media ID is required.".to_string()));
                return;
            }

            let draft = ProductImageDraft {
                media_id: media_id_val,
                position: Some(position.get_untracked()),
                alt_text: text_or_none(alt_text.get_untracked()),
                locale: locale_for_add.clone(),
            };

            set_busy.set(true);
            set_error.set(None);
            spawn_local(async move {
                match transport::add_product_image(
                    token_val,
                    tenant_val,
                    bootstrap.current_tenant.id,
                    bootstrap.me.id,
                    product_id_val.clone(),
                    draft,
                )
                .await
                {
                    Ok(_) => {
                        set_media_id.set(String::new());
                        set_alt_text.set(String::new());
                        set_position.set(0);
                        set_show_add.set(false);
                        on_mutated.run(product_id_val);
                    }
                    Err(err) => set_error.set(Some(err.to_string())),
                }
                set_busy.set(false);
            });
        }
    };

    view! {
        <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
            <div class="flex items-center justify-between gap-3">
                <div>
                    <h3 class="text-lg font-semibold text-card-foreground">{copy.title}</h3>
                    <p class="text-sm text-muted-foreground">{copy.subtitle}</p>
                </div>
                <button
                    type="button"
                    class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                    disabled=move || busy.get()
                    on:click=move |_| set_show_add.update(|v| *v = !*v)
                >
                    {copy.add}
                </button>
            </div>

            <Show when=move || show_add.get()>
                <form class="mt-4 rounded-2xl border border-border/70 bg-background/50 p-4 space-y-3" on:submit={
                    let on_submit = on_add_submit.clone();
                    move |ev| on_submit(ev)
                }>
                    <div class="grid gap-3 md:grid-cols-3">
                        <input
                            class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary font-mono text-xs"
                            placeholder="Media UUID"
                            prop:value=move || media_id.get()
                            on:input=move |ev| set_media_id.set(event_target_value(&ev))
                        />
                        <input
                            class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                            placeholder="Alt text"
                            prop:value=move || alt_text.get()
                            on:input=move |ev| set_alt_text.set(event_target_value(&ev))
                        />
                        <input
                            type="number"
                            class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                            placeholder="Position"
                            prop:value=move || position.get().to_string()
                            on:input=move |ev| set_position.set(parse_product_admin_inventory_quantity_input(&event_target_value(&ev)))
                        />
                    </div>
                    <div class="flex justify-end gap-2 pt-2">
                        <button
                            type="button"
                            class="rounded-lg border border-border px-3 py-1.5 text-xs font-medium text-foreground transition hover:bg-accent"
                            on:click=move |_| set_show_add.set(false)
                        >
                            "Cancel"
                        </button>
                        <button
                            type="submit"
                            class="rounded-lg bg-primary px-4 py-1.5 text-xs font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50"
                            disabled=move || busy.get()
                        >
                            "Add image"
                        </button>
                    </div>
                </form>
            </Show>

            <div class="mt-4">
                {if image_models.is_empty() {
                    view! {
                        <p class="text-sm text-muted-foreground py-4 text-center">{copy.empty}</p>
                    }.into_any()
                } else {
                    view! {
                        <div class="grid gap-3 sm:grid-cols-2 md:grid-cols-3">
                            {image_models.into_iter().enumerate().map(|(idx, item)| {
                                let image_id = item.id.clone();
                                let img_pos = item.position;
                                let product_id_for_del = product_id.clone();
                                let product_id_for_update = product_id.clone();
                                let product_id_for_reorder = product_id.clone();
                                let current_ids = current_image_ids.clone();
                                let on_mutated_del = on_media_mutated;
                                let on_mutated_update = on_media_mutated;
                                let on_mutated_reorder = on_media_mutated;
                                let locale_for_update = locale.clone();

                                let i_id = image_id.clone();
                                let is_editing_this = {
                                    let id = image_id.clone();
                                    move || editing_image_id.get().as_deref() == Some(&id)
                                };

                                let on_start_edit = {
                                    let id = image_id.clone();
                                    let alt = item.alt_text.clone();
                                    move |_| {
                                        set_edit_alt_text.set(alt.clone());
                                        set_editing_image_id.set(Some(id.clone()));
                                    }
                                };

                                let on_save_edit = {
                                    let id = image_id.clone();
                                    let p_id = product_id_for_update.clone();
                                    move |_| {
                                        let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
                                            return;
                                        };
                                        let token_val = token.get_untracked();
                                        let tenant_val = tenant.get_untracked();
                                        let target_img_id = id.clone();
                                        let prod_id = p_id.clone();

                                        let draft = UpdateProductImageDraft {
                                            position: Some(img_pos),
                                            alt_text: text_or_none(edit_alt_text.get_untracked()),
                                            locale: locale_for_update.clone(),
                                        };

                                        set_busy.set(true);
                                        set_error.set(None);
                                        spawn_local(async move {
                                            match transport::update_product_image(
                                                token_val,
                                                tenant_val,
                                                bootstrap.current_tenant.id,
                                                bootstrap.me.id,
                                                prod_id.clone(),
                                                target_img_id,
                                                draft,
                                            ).await {
                                                Ok(_) => {
                                                    set_editing_image_id.set(None);
                                                    on_mutated_update.run(prod_id);
                                                }
                                                Err(err) => set_error.set(Some(err.to_string())),
                                            }
                                            set_busy.set(false);
                                        });
                                    }
                                };

                                let on_remove = move |_| {
                                    let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
                                        return;
                                    };
                                    let token_val = token.get_untracked();
                                    let tenant_val = tenant.get_untracked();
                                    let img_id = i_id.clone();
                                    let prod_id = product_id_for_del.clone();

                                    set_busy.set(true);
                                    set_error.set(None);
                                    spawn_local(async move {
                                        match transport::delete_product_image(
                                            token_val,
                                            tenant_val,
                                            bootstrap.current_tenant.id,
                                            bootstrap.me.id,
                                            prod_id.clone(),
                                            img_id,
                                        ).await {
                                            Ok(_) => on_mutated_del.run(prod_id),
                                            Err(err) => set_error.set(Some(err.to_string())),
                                        }
                                        set_busy.set(false);
                                    });
                                };

                                let on_move_up = {
                                    let current_ids = current_ids.clone();
                                    let prod_id = product_id_for_reorder.clone();
                                    move |_| {
                                        if idx == 0 { return; }
                                        let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
                                            return;
                                        };
                                        let token_val = token.get_untracked();
                                        let tenant_val = tenant.get_untracked();
                                        let mut new_ids = current_ids.clone();
                                        new_ids.swap(idx, idx - 1);
                                        let p_id = prod_id.clone();

                                        set_busy.set(true);
                                        set_error.set(None);
                                        spawn_local(async move {
                                            match transport::reorder_product_images(
                                                token_val,
                                                tenant_val,
                                                bootstrap.current_tenant.id,
                                                bootstrap.me.id,
                                                p_id.clone(),
                                                new_ids,
                                            ).await {
                                                Ok(_) => on_mutated_reorder.run(p_id),
                                                Err(err) => set_error.set(Some(err.to_string())),
                                            }
                                            set_busy.set(false);
                                        });
                                    }
                                };

                                let on_move_down = {
                                    let current_ids = current_ids.clone();
                                    let prod_id = product_id_for_reorder.clone();
                                    move |_| {
                                        if idx + 1 >= current_ids.len() { return; }
                                        let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
                                            return;
                                        };
                                        let token_val = token.get_untracked();
                                        let tenant_val = tenant.get_untracked();
                                        let mut new_ids = current_ids.clone();
                                        new_ids.swap(idx, idx + 1);
                                        let p_id = prod_id.clone();

                                        set_busy.set(true);
                                        set_error.set(None);
                                        spawn_local(async move {
                                            match transport::reorder_product_images(
                                                token_val,
                                                tenant_val,
                                                bootstrap.current_tenant.id,
                                                bootstrap.me.id,
                                                p_id.clone(),
                                                new_ids,
                                            ).await {
                                                Ok(_) => on_mutated_reorder.run(p_id),
                                                Err(err) => set_error.set(Some(err.to_string())),
                                            }
                                            set_busy.set(false);
                                        });
                                    }
                                };

                                view! {
                                    <div class="rounded-2xl border border-border bg-background p-3 flex flex-col gap-2">
                                        <div class="h-32 w-full rounded-xl bg-muted/40 overflow-hidden flex items-center justify-center border border-border/40">
                                            {if item.url.is_empty() {
                                                view! { <span class="text-xs text-muted-foreground">"No preview"</span> }.into_any()
                                            } else {
                                                view! { <img src=item.url.clone() alt=item.alt_text.clone() class="h-full w-full object-cover" /> }.into_any()
                                            }}
                                        </div>
                                        <div class="flex-1">
                                            <p class="text-xs font-mono text-muted-foreground truncate" title=item.media_id.clone()>
                                                {format!("Media: {}", item.media_id)}
                                            </p>
                                            {move || if is_editing_this() {
                                                view! {
                                                    <div class="flex items-center gap-1 mt-1">
                                                        <input
                                                            class="rounded border border-border bg-background px-2 py-0.5 text-xs outline-none focus:border-primary w-full"
                                                            prop:value=move || edit_alt_text.get()
                                                            on:input=move |ev| set_edit_alt_text.set(event_target_value(&ev))
                                                        />
                                                        <button
                                                            type="button"
                                                            class="rounded bg-primary px-2 py-0.5 text-xs text-primary-foreground font-medium hover:bg-primary/90"
                                                            on:click=on_save_edit.clone()
                                                        >
                                                            "Save"
                                                        </button>
                                                        <button
                                                            type="button"
                                                            class="rounded border border-border px-1.5 py-0.5 text-xs text-foreground hover:bg-accent"
                                                            on:click=move |_| set_editing_image_id.set(None)
                                                        >
                                                            "✕"
                                                        </button>
                                                    </div>
                                                }.into_any()
                                            } else {
                                                view! {
                                                    <p class="text-xs text-foreground truncate cursor-pointer hover:underline" title=item.alt_text.clone() on:click=on_start_edit.clone()>
                                                        {if item.alt_text.is_empty() { "— (click to edit alt text)" } else { &item.alt_text }}
                                                    </p>
                                                }.into_any()
                                            }}
                                        </div>
                                        <div class="flex items-center justify-between gap-1 pt-1 border-t border-border/50">
                                            <div class="flex gap-1">
                                                <button
                                                    type="button"
                                                    class="rounded p-1 text-xs text-muted-foreground hover:bg-accent hover:text-foreground disabled:opacity-30"
                                                    disabled=move || item.is_first || busy.get()
                                                    title="Move up"
                                                    on:click=on_move_up
                                                >
                                                    "↑"
                                                </button>
                                                <button
                                                    type="button"
                                                    class="rounded p-1 text-xs text-muted-foreground hover:bg-accent hover:text-foreground disabled:opacity-30"
                                                    disabled=move || item.is_last || busy.get()
                                                    title="Move down"
                                                    on:click=on_move_down
                                                >
                                                    "↓"
                                                </button>
                                            </div>
                                            <button
                                                type="button"
                                                class="rounded px-2 py-0.5 text-xs text-rose-600 hover:bg-rose-50 border border-rose-200 transition disabled:opacity-50"
                                                disabled=move || busy.get()
                                                on:click=on_remove
                                            >
                                                "Remove"
                                            </button>
                                        </div>
                                    </div>
                                }
                            }).collect_view()}
                        </div>
                    }.into_any()
                }}
            </div>
        </section>
    }.into_any()
}

/// Mounted editor section that renders the product's effective typed schema.
///
/// The section loads the effective form and the saved values through the
/// canonical transport facade, renders one typed field per schema attribute
/// (grouped, disabled attributes excluded), persists only dirty patches, and
/// keeps detached values visible with an explicit clear action.
#[component]
pub fn ProductAttributeValuesSection(
    product_id: String,
    locale: Option<String>,
    on_saved: Callback<()>,
) -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();
    let form_copy = build_product_attribute_form_copy(locale.as_deref());
    let form_copy_for_failure = form_copy.clone();
    let form_loading = form_copy.loading.clone();
    let form_select_category = form_copy.select_category.clone();
    let form_no_attributes = form_copy.no_attributes.clone();
    let form_ungrouped_label = form_copy.ungrouped_label.clone();
    let form_required_label = form_copy.required_label.clone();
    let form_empty_option_label = form_copy.empty_option_label.clone();
    let form_boolean_true_label = form_copy.boolean_true_label.clone();
    let form_boolean_false_label = form_copy.boolean_false_label.clone();
    let form_detached_title = form_copy.detached_title.clone();
    let form_detached_values_label = form_copy.detached_values_label.clone();
    let form_clear_detached_label = form_copy.clear_detached_label.clone();
    let form_detached_empty_label = form_copy.detached_empty_label.clone();
    let section_copy = build_product_attribute_values_section_copy(locale.as_deref());
    let section_title = section_copy.title.clone();
    let section_subtitle = section_copy.subtitle.clone();
    let section_save_label = section_copy.save.clone();
    let section_saving_label = section_copy.saving.clone();
    let section_saved_label = section_copy.saved.clone();
    let section_nothing_dirty = section_copy.nothing_dirty.clone();
    let section_missing_option = section_copy.missing_option.clone();
    let error_copy = build_product_admin_error_copy(locale.as_deref());

    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);
    let (notice, set_notice) = signal(Option::<String>::None);
    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let editor_state = RwSignal::new(ProductAttributeEditorState::default());

    let loaded_product_id = product_id.clone();
    let loaded_locale = locale.clone();
    let detached_locale = locale.clone();
    let form_resource = LocalResource::new(move || {
        let tok = token.get();
        let ten = tenant.get();
        let pid = loaded_product_id.clone();
        let loc = loaded_locale.clone().unwrap_or_default();
        let _ = refresh_nonce.get();
        async move {
            let bootstrap = catalog_transport::fetch_bootstrap(tok.clone(), ten.clone())
                .await
                .map_err(|failure| failure.to_string())?;
            let form = catalog_transport::fetch_effective_product_form(
                tok.clone(),
                ten.clone(),
                bootstrap.current_tenant.id.clone(),
                Some(pid.clone()),
                None,
                loc.clone(),
            )
            .await
            .map_err(|failure| failure.to_string())?;
            let values = catalog_transport::fetch_product_attribute_values(
                tok,
                ten,
                bootstrap.current_tenant.id,
                pid,
                loc,
            )
            .await
            .map_err(|failure| failure.to_string())?;
            Ok::<(Option<ProductEffectiveForm>, Vec<ProductAttributeValueItem>), String>((
                form, values,
            ))
        }
    });

    Effect::new(move |_| {
        if let Some(Ok((_, values))) = form_resource.get() {
            editor_state.set(ProductAttributeEditorState::from_values(values));
        }
    });

    let save_product_id = product_id.clone();
    let save_locale = locale.clone();
    let on_save = move |_| {
        let attribute_types = form_resource
            .get_untracked()
            .and_then(Result::ok)
            .and_then(|(form, _)| form)
            .map(|form| {
                form.attributes
                    .into_iter()
                    .map(|attribute| (attribute.attribute_id, attribute.value_type))
                    .collect::<std::collections::HashMap<_, _>>()
            })
            .unwrap_or_default();
        let patches = match editor_state
            .get_untracked()
            .patches(save_locale.as_deref(), &attribute_types)
        {
            Ok(patches) => patches,
            Err(message) => {
                set_error.set(Some(message));
                return;
            }
        };
        if patches.is_empty() {
            set_notice.set(Some(section_nothing_dirty.clone()));
            return;
        }

        set_busy.set(true);
        set_error.set(None);
        set_notice.set(None);

        let tok = token.get_untracked();
        let ten = tenant.get_untracked();
        let pid = save_product_id.clone();
        let loc = save_locale.clone().unwrap_or_default();
        let saved_label = section_saved_label.clone();
        let save_error_copy = error_copy.clone();
        spawn_local(async move {
            let result = async {
                let bootstrap =
                    catalog_transport::fetch_bootstrap(tok.clone(), ten.clone()).await?;
                catalog_transport::save_product_attribute_values(
                    tok,
                    ten,
                    bootstrap.current_tenant.id,
                    bootstrap.me.id,
                    pid,
                    loc,
                    patches,
                )
                .await
            }
            .await;

            set_busy.set(false);
            match result {
                Ok(values) => {
                    editor_state.set(ProductAttributeEditorState::from_values(values));
                    set_notice.set(Some(saved_label));
                    set_refresh_nonce.update(|value| *value += 1);
                    on_saved.run(());
                }
                Err(failure) => set_error.set(Some(save_error_copy.save_product_failure(failure))),
            }
        });
    };

    let clear_product_id = product_id.clone();
    let clear_locale = locale.clone();
    let clear_error_copy = error_copy.clone();
    let on_clear_detached = move |_| {
        let attribute_ids = form_resource
            .get_untracked()
            .and_then(Result::ok)
            .map(|(_, values)| values)
            .unwrap_or_default()
            .into_iter()
            .filter(|value| value.detached)
            .map(|value| value.attribute_id)
            .collect::<Vec<_>>();
        if attribute_ids.is_empty() {
            return;
        }

        set_busy.set(true);
        set_error.set(None);
        set_notice.set(None);

        let tok = token.get_untracked();
        let ten = tenant.get_untracked();
        let pid = clear_product_id.clone();
        let loc = clear_locale.clone().unwrap_or_default();
        let clear_error = clear_error_copy.clone();
        spawn_local(async move {
            let result = async {
                let bootstrap =
                    catalog_transport::fetch_bootstrap(tok.clone(), ten.clone()).await?;
                catalog_transport::clear_detached_product_attribute_values(
                    tok,
                    ten,
                    bootstrap.current_tenant.id,
                    bootstrap.me.id,
                    pid,
                    loc,
                    attribute_ids,
                )
                .await
            }
            .await;

            set_busy.set(false);
            match result {
                Ok(values) => {
                    editor_state.set(ProductAttributeEditorState::from_values(values));
                    set_refresh_nonce.update(|value| *value += 1);
                    on_saved.run(());
                }
                Err(failure) => set_error.set(Some(clear_error.save_product_failure(failure))),
            }
        });
    };

    let has_detached_values = move || {
        form_resource
            .get()
            .and_then(Result::ok)
            .map(|(_, values)| values.iter().any(|value| value.detached))
            .unwrap_or(false)
    };

    view! {
        <section class="space-y-4 rounded-2xl border border-border bg-card p-5 shadow-sm">
            <div class="flex flex-wrap items-start justify-between gap-3">
                <div class="space-y-1">
                    <h3 class="text-sm font-semibold text-foreground">{section_title.clone()}</h3>
                    <p class="text-xs text-muted-foreground">{section_subtitle.clone()}</p>
                </div>
                <div class="flex items-center gap-2">
                    <button
                        type="button"
                        class="inline-flex h-9 items-center justify-center rounded-xl bg-primary px-4 text-xs font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50"
                        disabled=move || busy.get()
                        on:click=on_save
                    >
                        {move || if busy.get() {
                            section_saving_label.clone()
                        } else {
                            section_save_label.clone()
                        }}
                    </button>
                </div>
            </div>

            <Show when=move || error.get().is_some()>
                <div class="rounded-xl border border-rose-200 bg-rose-500/10 px-3 py-2 text-xs text-rose-600 dark:border-rose-900 dark:text-rose-400">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>
            <Show when=move || notice.get().is_some()>
                <div class="rounded-xl border border-emerald-200 bg-emerald-500/10 px-3 py-2 text-xs text-emerald-700 dark:border-emerald-900 dark:text-emerald-400">
                    {move || notice.get().unwrap_or_default()}
                </div>
            </Show>

            {move || match form_resource.get() {
                None => {
                    let loading = form_loading.clone();
                    view! { <p class="text-xs text-muted-foreground">{loading}</p> }.into_any()
                }
                Some(Err(detail)) => {
                    let load_failure = form_copy_for_failure.load_failure(detail);
                    view! { <p class="text-xs text-destructive">{load_failure}</p> }.into_any()
                }
                Some(Ok((form, values))) => match form {
                    None => {
                        let select_category = form_select_category.clone();
                        view! { <p class="text-xs text-muted-foreground">{select_category}</p> }.into_any()
                    }
                    Some(form) if form.attributes.is_empty() => {
                        let no_attributes = form_no_attributes.clone();
                        view! { <p class="text-xs text-muted-foreground">{no_attributes}</p> }.into_any()
                    }
                    Some(form) => {
                        let mut groups: Vec<(String, Vec<crate::model::ProductEffectiveFormAttribute>)> = Vec::new();
                        for attribute in form.attributes.into_iter().filter(|item| !item.is_disabled) {
                            let group = attribute
                                .group_label
                                .clone()
                                .or_else(|| attribute.group_code.clone())
                                .unwrap_or_else(|| form_ungrouped_label.clone());
                            if let Some((_, attributes)) = groups.iter_mut().find(|(code, _)| code == &group) {
                                attributes.push(attribute);
                            } else {
                                groups.push((group, vec![attribute]));
                            }
                        }
                        let required_label = form_required_label.clone();
                        let empty_option_label = form_empty_option_label.clone();
                        let boolean_true_label = form_boolean_true_label.clone();
                        let boolean_false_label = form_boolean_false_label.clone();
                        view! {
                            <div class="space-y-5">
                                {groups.into_iter().map(|(group, attributes)| view! {
                                    <section class="space-y-3">
                                        <h4 class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">{group}</h4>
                                        <div class="grid gap-4 md:grid-cols-2">
                                            {attributes.into_iter().map(|attribute| view! {
                                                {
                                                    let saved_option_ids = values
                                                        .iter()
                                                        .filter(|value| {
                                                            value.attribute_id == attribute.attribute_id
                                                                && !value.detached
                                                        })
                                                        .flat_map(|value| {
                                                            value
                                                                .option_ids
                                                                .clone()
                                                                .unwrap_or_else(|| {
                                                                    value
                                                                        .option_id
                                                                        .clone()
                                                                        .into_iter()
                                                                        .collect()
                                                                })
                                                        })
                                                        .collect::<Vec<String>>();
                                                    view! {
                                                        <TypedProductAttributeField
                                                            attribute=attribute
                                                            editor_state=editor_state
                                                            required_label=required_label.clone()
                                                            empty_option_label=empty_option_label.clone()
                                                            boolean_true_label=boolean_true_label.clone()
                                                            boolean_false_label=boolean_false_label.clone()
                                                            saved_option_ids=saved_option_ids
                                                            missing_option_suffix=section_missing_option.clone()
                                                        />
                                                    }
                                                }
                                            }).collect_view()}
                                        </div>
                                    </section>
                                }).collect_view()}
                            </div>
                        }.into_any()
                    }
                },
            }}

            <Show when=has_detached_values>
                <div class="rounded-xl border border-dashed border-border bg-muted/30 p-3">
                    <div class="flex flex-wrap items-center justify-between gap-3">
                        <div>
                            <h4 class="text-xs font-semibold text-foreground">{form_detached_title.clone()}</h4>
                            <p class="text-[11px] text-muted-foreground">{form_detached_values_label.clone()}</p>
                        </div>
                        <button
                            type="button"
                            class="rounded-lg border border-border px-3 py-2 text-xs font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                            disabled=move || busy.get()
                            on:click=on_clear_detached
                        >
                            {form_clear_detached_label.clone()}
                        </button>
                    </div>
                    <div class="mt-3 grid gap-2">
                        {move || {
                            let values = form_resource
                                .get()
                                .and_then(Result::ok)
                                .map(|(_, values)| {
                                    build_product_detached_attribute_value_view_models(
                                        detached_locale.as_deref(),
                                        &values,
                                    )
                                })
                                .unwrap_or_default();
                            if values.is_empty() {
                                let empty_label = form_detached_empty_label.clone();
                                view! { <p class="text-xs text-muted-foreground">{empty_label}</p> }.into_any()
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
            </Show>
        </section>
    }.into_any()
}

/// One editable variant axis row of the mounted editor.
#[derive(Clone, Debug, PartialEq)]
struct VariantAxisRow {
    attribute_id: String,
    code: String,
    label: String,
    allowed_option_ids: Vec<String>,
    options: Vec<VariantAxisOption>,
}

#[derive(Clone, Debug, PartialEq)]
struct VariantAxisOption {
    id: String,
    label: String,
}

/// Mounted editor section for the ADR variant-axis model.
///
/// The section seeds itself from the product's saved axes plus the effective
/// category schema. Only attributes whose `variant_axis_policy` is not
/// `forbidden` and that carry options are offered, because the owner rejects an
/// axis configuration the schema does not admit. The operator orders the axes,
/// chooses the allowed option subset per axis, and persists the whole
/// configuration through the idempotent `set_variant_axes` command. Saving an
/// empty configuration clears the axes, which the owner accepts.
#[component]
pub fn ProductVariantAxesSection(
    product_id: String,
    locale: Option<String>,
    on_saved: Callback<()>,
) -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();
    let is_ru = locale.as_deref() == Some("ru");
    let error_copy = build_product_admin_error_copy(locale.as_deref());
    let save_error_copy = error_copy.clone();

    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);
    let (notice, set_notice) = signal(Option::<String>::None);
    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (rows, set_rows) = signal(Vec::<VariantAxisRow>::new());
    let (add_selection, set_add_selection) = signal(String::new());
    let seeded = RwSignal::new(false);

    let title = if is_ru {
        "Оси вариантов"
    } else {
        "Variant axes"
    }
    .to_string();
    let subtitle = if is_ru {
        "Оси задают идентичность комбинаций. Доступны атрибуты схемы категории, для которых политика оси не запрещена."
    } else {
        "Axes define the combination identity. Only category-schema attributes whose axis policy is not forbidden are offered."
    }
    .to_string();
    let save_label = if is_ru {
        "Сохранить оси"
    } else {
        "Save axes"
    }
    .to_string();
    let saving_label = if is_ru {
        "Сохранение..."
    } else {
        "Saving..."
    }
    .to_string();
    let saved_label = if is_ru {
        "Оси вариантов сохранены"
    } else {
        "Variant axes saved"
    }
    .to_string();
    let cleared_label = if is_ru {
        "Оси очищены"
    } else {
        "Variant axes cleared"
    }
    .to_string();
    let empty_label = if is_ru {
        "Оси не заданы. Добавьте атрибут, чтобы включить комбинации вариантов."
    } else {
        "No axes configured. Add an attribute to enable variant combinations."
    }
    .to_string();
    let no_candidates_label = if is_ru {
        "В схеме категории нет атрибутов с опциями, допускающих использование в осях. Настройте схему категории."
    } else {
        "The category schema has no option-backed attribute that allows axis use. Configure the category schema."
    }
    .to_string();
    let add_label = if is_ru {
        "Добавить ось"
    } else {
        "Add axis"
    }
    .to_string();
    let select_attribute_label = if is_ru { "Атрибут" } else { "Attribute" }.to_string();
    let allowed_values_label = if is_ru {
        "Допустимые значения"
    } else {
        "Allowed values"
    }
    .to_string();

    let loaded_product_id = product_id.clone();
    let loaded_locale = locale.clone();
    let data_resource = LocalResource::new(move || {
        let tok = token.get();
        let ten = tenant.get();
        let pid = loaded_product_id.clone();
        let loc = loaded_locale.clone();
        let _ = refresh_nonce.get();
        async move {
            let bootstrap = catalog_transport::fetch_bootstrap(tok.clone(), ten.clone())
                .await
                .map_err(|failure| failure.to_string())?;
            let detail = catalog_transport::fetch_product(
                tok.clone(),
                ten.clone(),
                bootstrap.current_tenant.id.clone(),
                pid.clone(),
                loc.clone(),
            )
            .await
            .map_err(|failure| failure.to_string())?;
            let form = catalog_transport::fetch_effective_product_form(
                tok,
                ten,
                bootstrap.current_tenant.id,
                Some(pid),
                None,
                loc.unwrap_or_default(),
            )
            .await
            .map_err(|failure| failure.to_string())?;
            Ok::<(Option<ProductDetail>, Option<ProductEffectiveForm>), String>((detail, form))
        }
    });

    let seed_rows = move |detail: Option<ProductDetail>,
                          form: Option<ProductEffectiveForm>|
          -> Vec<VariantAxisRow> {
        let options_by_attribute = form
            .as_ref()
            .map(|form| {
                form.attributes
                    .iter()
                    .map(|attribute| {
                        (
                            attribute.attribute_id.clone(),
                            attribute
                                .options
                                .iter()
                                .map(|option| VariantAxisOption {
                                    id: option.id.clone(),
                                    label: option.label.clone(),
                                })
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect::<std::collections::HashMap<_, _>>()
            })
            .unwrap_or_default();

        let mut next_rows = detail
            .as_ref()
            .map(|detail| {
                detail
                    .variant_axes
                    .iter()
                    .map(|axis| {
                        let saved_options = axis
                            .allowed_values
                            .iter()
                            .map(|value| VariantAxisOption {
                                id: value.option_id.clone(),
                                label: if value.value.is_empty() {
                                    value.option_id.clone()
                                } else {
                                    value.value.clone()
                                },
                            })
                            .collect::<Vec<_>>();
                        let options = options_by_attribute
                            .get(&axis.attribute_id)
                            .cloned()
                            .unwrap_or(saved_options);
                        VariantAxisRow {
                            attribute_id: axis.attribute_id.clone(),
                            code: axis.code.clone(),
                            label: axis.name.clone(),
                            allowed_option_ids: axis
                                .allowed_values
                                .iter()
                                .map(|value| value.option_id.clone())
                                .collect(),
                            options,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        if next_rows.is_empty() {
            if let Some(form) = form.as_ref() {
                next_rows = form
                    .attributes
                    .iter()
                    .filter(|attribute| {
                        attribute.default_variant_axis
                            && attribute.variant_axis_policy != "forbidden"
                            && !attribute.options.is_empty()
                    })
                    .map(|attribute| VariantAxisRow {
                        attribute_id: attribute.attribute_id.clone(),
                        code: attribute.code.clone(),
                        label: attribute.label.clone(),
                        allowed_option_ids: attribute
                            .options
                            .iter()
                            .map(|option| option.id.clone())
                            .collect(),
                        options: attribute
                            .options
                            .iter()
                            .map(|option| VariantAxisOption {
                                id: option.id.clone(),
                                label: option.label.clone(),
                            })
                            .collect(),
                    })
                    .collect();
            }
        }

        next_rows
    };

    Effect::new(move |_| {
        if seeded.get() {
            return;
        }
        let Some(Ok((detail, form))) = data_resource.get() else {
            return;
        };
        set_rows.set(seed_rows(detail, form));
        seeded.set(true);
    });

    let available_candidates = move || -> Vec<(String, String)> {
        let used = rows
            .get()
            .into_iter()
            .map(|row| row.attribute_id)
            .collect::<Vec<_>>();
        data_resource
            .get()
            .and_then(Result::ok)
            .and_then(|(_, form)| form)
            .map(|form| {
                form.attributes
                    .into_iter()
                    .filter(|attribute| {
                        attribute.variant_axis_policy != "forbidden"
                            && !attribute.options.is_empty()
                            && !used.contains(&attribute.attribute_id)
                    })
                    .map(|attribute| (attribute.attribute_id, attribute.label))
                    .collect()
            })
            .unwrap_or_default()
    };

    let move_axis = move |attribute_id: &str, delta: i32| {
        set_rows.update(|rows| {
            let Some(index) = rows.iter().position(|row| row.attribute_id == attribute_id) else {
                return;
            };
            let target = index as i32 + delta;
            if target < 0 || target as usize >= rows.len() {
                return;
            }
            rows.swap(index, target as usize);
        });
    };

    let remove_axis = move |attribute_id: &str| {
        set_rows.update(|rows| rows.retain(|row| row.attribute_id != attribute_id));
    };

    let toggle_option = move |attribute_id: &str, option_id: &str| {
        set_rows.update(|rows| {
            let Some(row) = rows.iter_mut().find(|row| row.attribute_id == attribute_id) else {
                return;
            };
            match row
                .allowed_option_ids
                .iter()
                .position(|value| value == option_id)
            {
                Some(position) => {
                    row.allowed_option_ids.remove(position);
                }
                None => row.allowed_option_ids.push(option_id.to_string()),
            }
        });
    };

    let add_axis = move |_| {
        let attribute_id = add_selection.get_untracked();
        if attribute_id.is_empty() {
            return;
        }
        let attribute = data_resource
            .get_untracked()
            .and_then(Result::ok)
            .and_then(|(_, form)| form)
            .and_then(|form| {
                form.attributes
                    .into_iter()
                    .find(|attribute| attribute.attribute_id == attribute_id)
            });
        let Some(attribute) = attribute else {
            return;
        };
        let row = VariantAxisRow {
            attribute_id: attribute.attribute_id,
            code: attribute.code,
            label: attribute.label,
            allowed_option_ids: attribute
                .options
                .iter()
                .map(|option| option.id.clone())
                .collect(),
            options: attribute
                .options
                .into_iter()
                .map(|option| VariantAxisOption {
                    id: option.id,
                    label: option.label,
                })
                .collect(),
        };
        set_rows.update(move |rows| {
            if rows
                .iter()
                .any(|existing| existing.attribute_id == row.attribute_id)
            {
                return;
            }
            rows.push(row);
        });
        set_add_selection.set(String::new());
    };

    let save_product_id = product_id.clone();
    let on_save = move |_| {
        let axes = rows
            .get_untracked()
            .into_iter()
            .enumerate()
            .map(|(index, row)| VariantAxisDraft {
                attribute_id: row.attribute_id,
                position: Some(index as i32),
                allowed_option_ids: row.allowed_option_ids,
            })
            .collect::<Vec<_>>();
        let cleared = axes.is_empty();

        set_busy.set(true);
        set_error.set(None);
        set_notice.set(None);

        let tok = token.get_untracked();
        let ten = tenant.get_untracked();
        let pid = save_product_id.clone();
        let saved = saved_label.clone();
        let cleared_label = cleared_label.clone();
        let save_error = save_error_copy.clone();
        spawn_local(async move {
            let result = async {
                let bootstrap =
                    catalog_transport::fetch_bootstrap(tok.clone(), ten.clone()).await?;
                catalog_transport::set_variant_axes(
                    tok,
                    ten,
                    bootstrap.current_tenant.id,
                    bootstrap.me.id,
                    pid,
                    SetVariantAxesDraft { axes },
                )
                .await
            }
            .await;

            set_busy.set(false);
            match result {
                Ok(_) => {
                    set_notice.set(Some(if cleared { cleared_label } else { saved }));
                    seeded.set(false);
                    set_refresh_nonce.update(|value| *value += 1);
                    on_saved.run(());
                }
                Err(failure) => set_error.set(Some(save_error.save_product_failure(failure))),
            }
        });
    };

    let candidates_for_add = available_candidates;

    view! {
        <section class="space-y-3 rounded-xl border border-border/80 bg-muted/10 p-3">
            <div class="flex flex-wrap items-start justify-between gap-2">
                <div class="space-y-1">
                    <h4 class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">{title.clone()}</h4>
                    <p class="text-[11px] text-muted-foreground">{subtitle.clone()}</p>
                </div>
                <button
                    type="button"
                    class="inline-flex h-8 items-center justify-center rounded-lg bg-primary px-3 text-[11px] font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50"
                    disabled=move || busy.get()
                    on:click=on_save
                >
                    {move || if busy.get() { saving_label.clone() } else { save_label.clone() }}
                </button>
            </div>

            <Show when=move || error.get().is_some()>
                <div class="rounded-lg border border-rose-200 bg-rose-500/10 px-3 py-2 text-[11px] text-rose-600 dark:border-rose-900 dark:text-rose-400">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>
            <Show when=move || notice.get().is_some()>
                <div class="rounded-lg border border-emerald-200 bg-emerald-500/10 px-3 py-2 text-[11px] text-emerald-700 dark:border-emerald-900 dark:text-emerald-400">
                    {move || notice.get().unwrap_or_default()}
                </div>
            </Show>

            <Show when=move || rows.get().is_empty()>
                <p class="text-[11px] text-muted-foreground">{empty_label.clone()}</p>
            </Show>

            {move || {
                let row_count = rows.get().len();
                rows.get()
                    .into_iter()
                    .enumerate()
                    .map(|(index, row)| {
                        let up_id = row.attribute_id.clone();
                        let down_id = row.attribute_id.clone();
                        let remove_id = row.attribute_id.clone();
                        let toggle_attribute = row.attribute_id.clone();
                        let selected = row.allowed_option_ids.clone();
                        let options = row.options.clone();
                        let can_move_up = index > 0;
                        let can_move_down = index + 1 < row_count;
                        view! {
                            <div class="space-y-2 rounded-lg border border-border/80 bg-background p-3">
                                <div class="flex flex-wrap items-center justify-between gap-2">
                                    <div class="flex items-center gap-2 text-xs font-semibold text-foreground">
                                        <span>"#" {index + 1}</span>
                                        <span>{row.label.clone()}</span>
                                        <span class="font-mono text-[10px] text-muted-foreground">{row.code.clone()}</span>
                                    </div>
                                    <div class="flex items-center gap-1">
                                        <button
                                            type="button"
                                            class="h-6 w-6 rounded border border-border text-[11px] text-foreground transition hover:bg-accent disabled:opacity-40"
                                            disabled=!can_move_up
                                            title=if is_ru { "Выше" } else { "Move up" }
                                            on:click=move |_| move_axis(&up_id, -1)
                                        >"↑"</button>
                                        <button
                                            type="button"
                                            class="h-6 w-6 rounded border border-border text-[11px] text-foreground transition hover:bg-accent disabled:opacity-40"
                                            disabled=!can_move_down
                                            title=if is_ru { "Ниже" } else { "Move down" }
                                            on:click=move |_| move_axis(&down_id, 1)
                                        >"↓"</button>
                                        <button
                                            type="button"
                                            class="h-6 w-6 rounded border border-rose-200 text-[11px] text-rose-600 transition hover:bg-rose-500/10 dark:border-rose-900 dark:text-rose-400"
                                            title=if is_ru { "Удалить ось" } else { "Remove axis" }
                                            on:click=move |_| remove_axis(&remove_id)
                                        >"✕"</button>
                                    </div>
                                </div>
                                <div class="space-y-1">
                                    <p class="text-[10px] uppercase tracking-wide text-muted-foreground">{allowed_values_label.clone()}</p>
                                    <div class="flex flex-wrap gap-2">
                                        {options
                                            .into_iter()
                                            .map(|option| {
                                                let option_id = option.id.clone();
                                                let toggle_attribute = toggle_attribute.clone();
                                                let is_selected = selected.contains(&option.id);
                                                let toggle_option = toggle_option;
                                                view! {
                                                    <label class="inline-flex items-center gap-1.5 rounded-lg border border-border/70 bg-muted/20 px-2 py-1 text-[11px] text-foreground">
                                                        <input
                                                            type="checkbox"
                                                            prop:checked=is_selected
                                                            on:change=move |_| toggle_option(&toggle_attribute, &option_id)
                                                        />
                                                        <span>{option.label}</span>
                                                    </label>
                                                }
                                            })
                                            .collect_view()}
                                    </div>
                                </div>
                            </div>
                        }
                    })
                    .collect_view()
            }}

            <div class="flex flex-wrap items-end gap-2">
                <label class="grid gap-1 text-[11px] text-foreground">
                    <span class="font-medium">{select_attribute_label.clone()}</span>
                    <select
                        class="min-w-[220px] rounded-lg border border-border bg-background px-2 py-2 text-xs text-foreground outline-none transition focus:border-primary"
                        prop:value=move || add_selection.get()
                        on:change=move |ev| set_add_selection.set(event_target_value(&ev))
                    >
                        <option value="">{if is_ru { "— выберите атрибут —" } else { "— select attribute —" }}</option>
                        {move || candidates_for_add()
                            .into_iter()
                            .map(|(attribute_id, label)| view! {
                                <option value=attribute_id>{label}</option>
                            })
                            .collect_view()}
                    </select>
                </label>
                <button
                    type="button"
                    class="h-9 rounded-lg border border-border px-3 text-xs font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                    disabled=move || add_selection.get().is_empty()
                    on:click=add_axis
                >
                    {add_label.clone()}
                </button>
                <Show when=move || candidates_for_add().is_empty() && rows.get().is_empty()>
                    <span class="text-[11px] text-muted-foreground">{no_candidates_label.clone()}</span>
                </Show>
            </div>
        </section>
    }.into_any()
}

/// Applies the shared result handling of the mounted schema-authoring commands.
///
/// Every authoring command resolves to `Result<bool, String>` at the facade
/// boundary, so the card keeps exactly one place that turns an accepted, a
/// rejected, and a failed command into operator-visible state.
fn apply_schema_authoring_result(
    busy: WriteSignal<bool>,
    error: WriteSignal<Option<String>>,
    notice: WriteSignal<Option<String>>,
    refresh_nonce: WriteSignal<u64>,
    error_copy: ProductAdminErrorCopy,
    is_ru: bool,
    run: std::pin::Pin<Box<dyn std::future::Future<Output = Result<bool, String>> + 'static>>,
) {
    busy.set(true);
    error.set(None);
    notice.set(None);
    spawn_local(async move {
        let result = run.await;
        busy.set(false);
        match result {
            Ok(accepted) => {
                if accepted {
                    notice.set(Some(
                        if is_ru {
                            "Схема обновлена"
                        } else {
                            "Schema authoring updated"
                        }
                        .to_string(),
                    ));
                } else {
                    error.set(Some(
                        if is_ru {
                            "Владелец отклонил изменение схемы"
                        } else {
                            "The owner rejected the schema change"
                        }
                        .to_string(),
                    ));
                }
                refresh_nonce.update(|value| *value += 1);
            }
            Err(detail) => error.set(Some(error_copy.save_product_failure(detail))),
        }
    });
}

/// Mounted authoring card for the category-schema layer.
///
/// Category schemas used to be configurable only through raw GraphQL/REST. The
/// card drives the five owner commands that were unreachable from the UI:
/// schema mode, schema-scoped groups, category-scoped groups, schema attribute
/// bindings, and category attribute bindings. Every section resolves its own
/// owner reads through the canonical transport facade and surfaces the command
/// result instead of discarding it.
#[component]
pub fn ProductSchemaAuthoringCard(locale: Option<String>) -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();
    let locale_store = StoredValue::new(locale);
    let is_ru = locale_store.get_value().as_deref() == Some("ru");
    let error_copy_store = StoredValue::new(build_product_admin_error_copy(
        locale_store.get_value().as_deref(),
    ));

    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);
    let (notice, set_notice) = signal(Option::<String>::None);
    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);

    // Schema mode
    let (mode_category, set_mode_category) = signal(String::new());
    let (mode_value, set_mode_value) = signal("inherit".to_string());
    let (mode_schema, set_mode_schema) = signal(String::new());
    let (mode_clone_category, set_clone_category) = signal(String::new());

    // Schema-scoped groups and bindings
    let (schema_target, set_schema_target) = signal(String::new());
    let (schema_group_code, set_schema_group_code) = signal(String::new());
    let (schema_group_label, set_schema_group_label) = signal(String::new());
    let (schema_group_position, set_schema_group_position) = signal("0".to_string());
    let (schema_attribute, set_schema_attribute) = signal(String::new());
    let (schema_binding_group, set_schema_binding_group) = signal(String::new());
    let (schema_binding_required, set_schema_binding_required) = signal(false);
    let (schema_binding_disabled, set_schema_binding_disabled) = signal(false);
    let (schema_binding_position, set_schema_binding_position) = signal("0".to_string());

    // Category-scoped groups and bindings
    let (category_target, set_category_target) = signal(String::new());
    let (category_group_code, set_category_group_code) = signal(String::new());
    let (category_group_label, set_category_group_label) = signal(String::new());
    let (category_group_position, set_category_group_position) = signal("0".to_string());
    let (category_attribute, set_category_attribute) = signal(String::new());
    let (category_binding_group, set_category_binding_group) = signal(String::new());
    let (category_binding_kind, set_category_binding_kind) = signal("addition".to_string());
    let (category_binding_disabled, set_category_binding_disabled) = signal(false);
    let (category_binding_position, set_category_binding_position) = signal("0".to_string());

    let categories_resource = LocalResource::new(move || {
        let tok = token.get();
        let ten = tenant.get();
        let loc = locale_store.get_value().unwrap_or_default();
        let _ = refresh_nonce.get();
        async move {
            let bootstrap = catalog_transport::fetch_bootstrap(tok.clone(), ten.clone())
                .await
                .map_err(|failure| failure.to_string())?;
            catalog_transport::fetch_catalog_categories(tok, ten, bootstrap.current_tenant.id, loc)
                .await
                .map(|list| list.items)
                .map_err(|failure| failure.to_string())
        }
    });

    let schemas_resource = LocalResource::new(move || {
        let tok = token.get();
        let ten = tenant.get();
        let loc = locale_store.get_value().unwrap_or_default();
        let _ = refresh_nonce.get();
        async move {
            let bootstrap = catalog_transport::fetch_bootstrap(tok.clone(), ten.clone())
                .await
                .map_err(|failure| failure.to_string())?;
            catalog_transport::fetch_attribute_schemas(tok, ten, bootstrap.current_tenant.id, loc)
                .await
                .map(|list| list.items)
                .map_err(|failure| failure.to_string())
        }
    });

    let attributes_resource = LocalResource::new(move || {
        let tok = token.get();
        let ten = tenant.get();
        let loc = locale_store.get_value().unwrap_or_default();
        let _ = refresh_nonce.get();
        async move {
            let bootstrap = catalog_transport::fetch_bootstrap(tok.clone(), ten.clone())
                .await
                .map_err(|failure| failure.to_string())?;
            catalog_transport::fetch_product_attributes(tok, ten, bootstrap.current_tenant.id, loc)
                .await
                .map(|list| list.items)
                .map_err(|failure| failure.to_string())
        }
    });

    let categories = move || -> Vec<CatalogCategorySummary> {
        categories_resource
            .get()
            .and_then(Result::ok)
            .unwrap_or_default()
    };
    let schemas = move || -> Vec<ProductAttributeSchemaSummary> {
        schemas_resource
            .get()
            .and_then(Result::ok)
            .unwrap_or_default()
    };
    let attributes = move || -> Vec<ProductAttributeSummary> {
        attributes_resource
            .get()
            .and_then(Result::ok)
            .unwrap_or_default()
    };

    let apply_mode = move |_| {
        let category_id = mode_category.get_untracked();
        if category_id.is_empty() {
            set_error.set(Some(
                if is_ru {
                    "Выберите категорию"
                } else {
                    "Select a category"
                }
                .to_string(),
            ));
            return;
        }
        let mode = mode_value.get_untracked();
        let schema_id = mode_schema.get_untracked();
        let clone_from_category_id = mode_clone_category.get_untracked();
        let draft = SetCategorySchemaModeDraft {
            category_id,
            mode: mode.clone(),
            schema_id: if mode == "use_schema" && !schema_id.is_empty() {
                Some(schema_id)
            } else {
                None
            },
            clone_from_category_id: if mode == "clone_from_category"
                && !clone_from_category_id.is_empty()
            {
                Some(clone_from_category_id)
            } else {
                None
            },
        };
        let tok = token.get_untracked();
        let ten = tenant.get_untracked();
        apply_schema_authoring_result(
            set_busy,
            set_error,
            set_notice,
            set_refresh_nonce,
            error_copy_store.get_value(),
            is_ru,
            Box::pin(async move {
                let bootstrap = catalog_transport::fetch_bootstrap(tok.clone(), ten.clone())
                    .await
                    .map_err(|failure| failure.to_string())?;
                catalog_transport::set_category_schema_mode(
                    tok,
                    ten,
                    bootstrap.current_tenant.id,
                    bootstrap.me.id,
                    draft,
                )
                .await
                .map_err(|failure| failure.to_string())
            }),
        );
    };

    let create_schema_group = move |_| {
        let schema_id = schema_target.get_untracked();
        let code = schema_group_code.get_untracked();
        if schema_id.is_empty() || code.trim().is_empty() {
            set_error.set(Some(
                if is_ru {
                    "Выберите схему и укажите код группы"
                } else {
                    "Select a schema and provide the group code"
                }
                .to_string(),
            ));
            return;
        }
        let draft = ProductAttributeSchemaGroupDraft {
            schema_id,
            code: code.trim().to_string(),
            label: {
                let label = schema_group_label.get_untracked();
                if label.trim().is_empty() {
                    code.trim().to_string()
                } else {
                    label.trim().to_string()
                }
            },
            position: schema_group_position
                .get_untracked()
                .trim()
                .parse::<i32>()
                .unwrap_or(0),
        };
        let tok = token.get_untracked();
        let ten = tenant.get_untracked();
        let loc = locale_store.get_value().unwrap_or_default();
        apply_schema_authoring_result(
            set_busy,
            set_error,
            set_notice,
            set_refresh_nonce,
            error_copy_store.get_value(),
            is_ru,
            Box::pin(async move {
                let bootstrap = catalog_transport::fetch_bootstrap(tok.clone(), ten.clone())
                    .await
                    .map_err(|failure| failure.to_string())?;
                catalog_transport::create_product_attribute_schema_group(
                    tok,
                    ten,
                    bootstrap.current_tenant.id,
                    bootstrap.me.id,
                    loc,
                    draft,
                )
                .await
                .map_err(|failure| failure.to_string())
            }),
        );
    };

    let bind_schema = move |_| {
        let schema_id = schema_target.get_untracked();
        let attribute_id = schema_attribute.get_untracked();
        if schema_id.is_empty() || attribute_id.is_empty() {
            set_error.set(Some(
                if is_ru {
                    "Выберите схему и атрибут"
                } else {
                    "Select a schema and an attribute"
                }
                .to_string(),
            ));
            return;
        }
        let group_code = schema_binding_group.get_untracked();
        let draft = BindSchemaAttributeDraft {
            schema_id,
            attribute_id,
            group_code: if group_code.trim().is_empty() {
                None
            } else {
                Some(group_code.trim().to_string())
            },
            is_required: schema_binding_required.get_untracked(),
            is_disabled: schema_binding_disabled.get_untracked(),
            position: schema_binding_position
                .get_untracked()
                .trim()
                .parse::<i32>()
                .unwrap_or(0),
        };
        let tok = token.get_untracked();
        let ten = tenant.get_untracked();
        apply_schema_authoring_result(
            set_busy,
            set_error,
            set_notice,
            set_refresh_nonce,
            error_copy_store.get_value(),
            is_ru,
            Box::pin(async move {
                let bootstrap = catalog_transport::fetch_bootstrap(tok.clone(), ten.clone())
                    .await
                    .map_err(|failure| failure.to_string())?;
                catalog_transport::bind_schema_attribute(
                    tok,
                    ten,
                    bootstrap.current_tenant.id,
                    bootstrap.me.id,
                    draft,
                )
                .await
                .map_err(|failure| failure.to_string())
            }),
        );
    };

    let create_category_group = move |_| {
        let category_id = category_target.get_untracked();
        let code = category_group_code.get_untracked();
        if category_id.is_empty() || code.trim().is_empty() {
            set_error.set(Some(
                if is_ru {
                    "Выберите категорию и укажите код группы"
                } else {
                    "Select a category and provide the group code"
                }
                .to_string(),
            ));
            return;
        }
        let draft = CategoryAttributeGroupDraft {
            category_id,
            code: code.trim().to_string(),
            label: {
                let label = category_group_label.get_untracked();
                if label.trim().is_empty() {
                    code.trim().to_string()
                } else {
                    label.trim().to_string()
                }
            },
            position: category_group_position
                .get_untracked()
                .trim()
                .parse::<i32>()
                .unwrap_or(0),
        };
        let tok = token.get_untracked();
        let ten = tenant.get_untracked();
        let loc = locale_store.get_value().unwrap_or_default();
        apply_schema_authoring_result(
            set_busy,
            set_error,
            set_notice,
            set_refresh_nonce,
            error_copy_store.get_value(),
            is_ru,
            Box::pin(async move {
                let bootstrap = catalog_transport::fetch_bootstrap(tok.clone(), ten.clone())
                    .await
                    .map_err(|failure| failure.to_string())?;
                catalog_transport::create_category_attribute_group(
                    tok,
                    ten,
                    bootstrap.current_tenant.id,
                    bootstrap.me.id,
                    loc,
                    draft,
                )
                .await
                .map_err(|failure| failure.to_string())
            }),
        );
    };

    let bind_category = move |_| {
        let category_id = category_target.get_untracked();
        let attribute_id = category_attribute.get_untracked();
        if category_id.is_empty() || attribute_id.is_empty() {
            set_error.set(Some(
                if is_ru {
                    "Выберите категорию и атрибут"
                } else {
                    "Select a category and an attribute"
                }
                .to_string(),
            ));
            return;
        }
        let group_code = category_binding_group.get_untracked();
        let draft = BindCategoryAttributeDraft {
            category_id,
            attribute_id,
            group_code: if group_code.trim().is_empty() {
                None
            } else {
                Some(group_code.trim().to_string())
            },
            binding_kind: category_binding_kind.get_untracked(),
            is_required: None,
            is_disabled: category_binding_disabled.get_untracked(),
            position: Some(
                category_binding_position
                    .get_untracked()
                    .trim()
                    .parse::<i32>()
                    .unwrap_or(0),
            ),
        };
        let tok = token.get_untracked();
        let ten = tenant.get_untracked();
        apply_schema_authoring_result(
            set_busy,
            set_error,
            set_notice,
            set_refresh_nonce,
            error_copy_store.get_value(),
            is_ru,
            Box::pin(async move {
                let bootstrap = catalog_transport::fetch_bootstrap(tok.clone(), ten.clone())
                    .await
                    .map_err(|failure| failure.to_string())?;
                catalog_transport::bind_category_attribute(
                    tok,
                    ten,
                    bootstrap.current_tenant.id,
                    bootstrap.me.id,
                    draft,
                )
                .await
                .map_err(|failure| failure.to_string())
            }),
        );
    };

    let select_class = "w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground outline-none focus:border-primary";
    let input_class = "w-full text-xs rounded-xl border border-border bg-background px-3 py-2 text-foreground outline-none focus:border-primary";
    let label_class = "grid gap-1.5 text-xs font-medium text-foreground";
    let button_class = "inline-flex h-9 items-center justify-center rounded-xl bg-primary px-3 text-xs font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50";

    view! {
        <section class="space-y-5 rounded-2xl border border-border bg-card p-5 shadow-sm">
            <div class="space-y-1">
                <h2 class="text-sm font-semibold text-foreground">
                    {if is_ru { "Схема категории: режим, группы, привязки" } else { "Category schema: mode, groups, bindings" }}
                </h2>
                <p class="text-xs text-muted-foreground">
                    {if is_ru {
                        "Команды владельца, которые раньше были достижимы только через GraphQL/REST."
                    } else {
                        "Owner commands that used to be reachable only through GraphQL/REST."
                    }}
                </p>
            </div>

            <Show when=move || error.get().is_some()>
                <div class="rounded-xl border border-rose-200 bg-rose-500/10 px-3 py-2 text-xs text-rose-600 dark:border-rose-900 dark:text-rose-400">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>
            <Show when=move || notice.get().is_some()>
                <div class="rounded-xl border border-emerald-200 bg-emerald-500/10 px-3 py-2 text-xs text-emerald-700 dark:border-emerald-900 dark:text-emerald-400">
                    {move || notice.get().unwrap_or_default()}
                </div>
            </Show>

            // 1. Category schema mode
            <div class="space-y-3 rounded-xl border border-border/80 bg-muted/10 p-4">
                <h3 class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    {if is_ru { "Режим схемы категории" } else { "Category schema mode" }}
                </h3>
                <div class="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
                    <label class=label_class>
                        <span>{if is_ru { "Категория" } else { "Category" }}</span>
                        <select
                            class=select_class
                            prop:value=move || mode_category.get()
                            on:change=move |ev| set_mode_category.set(event_target_value(&ev))
                        >
                            <option value="">{if is_ru { "— выберите —" } else { "— select —" }}</option>
                            {move || categories().into_iter().map(|category| view! {
                                <option value=category.id.clone()>{category.name.clone()}</option>
                            }).collect_view()}
                        </select>
                    </label>
                    <label class=label_class>
                        <span>{if is_ru { "Режим" } else { "Mode" }}</span>
                        <select
                            class=select_class
                            prop:value=move || mode_value.get()
                            on:change=move |ev| set_mode_value.set(event_target_value(&ev))
                        >
                            <option value="inherit">"inherit"</option>
                            <option value="use_schema">"use_schema"</option>
                            <option value="clone_from_category">"clone_from_category"</option>
                            <option value="custom">"custom"</option>
                        </select>
                    </label>
                    <label class=label_class>
                        <span>{if is_ru { "Схема (для use_schema)" } else { "Schema (for use_schema)" }}</span>
                        <select
                            class=select_class
                            prop:value=move || mode_schema.get()
                            on:change=move |ev| set_mode_schema.set(event_target_value(&ev))
                        >
                            <option value="">{if is_ru { "— не выбрана —" } else { "— none —" }}</option>
                            {move || schemas().into_iter().map(|schema| view! {
                                <option value=schema.id.clone()>{schema.name.clone()}</option>
                            }).collect_view()}
                        </select>
                    </label>
                    <label class=label_class>
                        <span>{if is_ru { "Копировать из категории" } else { "Clone from category" }}</span>
                        <select
                            class=select_class
                            prop:value=move || mode_clone_category.get()
                            on:change=move |ev| set_clone_category.set(event_target_value(&ev))
                        >
                            <option value="">{if is_ru { "— не выбрана —" } else { "— none —" }}</option>
                            {move || categories().into_iter().map(|category| view! {
                                <option value=category.id.clone()>{category.name.clone()}</option>
                            }).collect_view()}
                        </select>
                    </label>
                </div>
                <button
                    type="button"
                    class=button_class
                    disabled=move || busy.get()
                    on:click=apply_mode
                >
                    {if is_ru { "Применить режим" } else { "Apply mode" }}
                </button>
            </div>

            // 2. Schema-scoped groups and attribute bindings
            <div class="space-y-3 rounded-xl border border-border/80 bg-muted/10 p-4">
                <h3 class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    {if is_ru { "Схема: группы и привязки атрибутов" } else { "Schema: groups and attribute bindings" }}
                </h3>
                <div class="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
                    <label class=label_class>
                        <span>{if is_ru { "Схема" } else { "Schema" }}</span>
                        <select
                            class=select_class
                            prop:value=move || schema_target.get()
                            on:change=move |ev| set_schema_target.set(event_target_value(&ev))
                        >
                            <option value="">{if is_ru { "— выберите —" } else { "— select —" }}</option>
                            {move || schemas().into_iter().map(|schema| view! {
                                <option value=schema.id.clone()>{schema.name.clone()}</option>
                            }).collect_view()}
                        </select>
                    </label>
                    <label class=label_class>
                        <span>{if is_ru { "Код группы" } else { "Group code" }}</span>
                        <input
                            type="text"
                            class=input_class
                            placeholder="specs"
                            prop:value=move || schema_group_code.get()
                            on:input=move |ev| set_schema_group_code.set(event_target_value(&ev))
                        />
                    </label>
                    <label class=label_class>
                        <span>{if is_ru { "Название группы" } else { "Group label" }}</span>
                        <input
                            type="text"
                            class=input_class
                            placeholder="Specs"
                            prop:value=move || schema_group_label.get()
                            on:input=move |ev| set_schema_group_label.set(event_target_value(&ev))
                        />
                    </label>
                    <label class=label_class>
                        <span>{if is_ru { "Позиция" } else { "Position" }}</span>
                        <input
                            type="number"
                            class=input_class
                            prop:value=move || schema_group_position.get()
                            on:input=move |ev| set_schema_group_position.set(event_target_value(&ev))
                        />
                    </label>
                </div>
                <button
                    type="button"
                    class=button_class
                    disabled=move || busy.get()
                    on:click=create_schema_group
                >
                    {if is_ru { "Создать группу схемы" } else { "Create schema group" }}
                </button>

                <div class="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
                    <label class=label_class>
                        <span>{if is_ru { "Атрибут" } else { "Attribute" }}</span>
                        <select
                            class=select_class
                            prop:value=move || schema_attribute.get()
                            on:change=move |ev| set_schema_attribute.set(event_target_value(&ev))
                        >
                            <option value="">{if is_ru { "— выберите —" } else { "— select —" }}</option>
                            {move || attributes().into_iter().map(|attribute| view! {
                                <option value=attribute.id.clone()>{attribute.label.clone()}</option>
                            }).collect_view()}
                        </select>
                    </label>
                    <label class=label_class>
                        <span>{if is_ru { "Группа (код)" } else { "Group (code)" }}</span>
                        <input
                            type="text"
                            class=input_class
                            placeholder="specs"
                            prop:value=move || schema_binding_group.get()
                            on:input=move |ev| set_schema_binding_group.set(event_target_value(&ev))
                        />
                    </label>
                    <label class=label_class>
                        <span>{if is_ru { "Позиция" } else { "Position" }}</span>
                        <input
                            type="number"
                            class=input_class
                            prop:value=move || schema_binding_position.get()
                            on:input=move |ev| set_schema_binding_position.set(event_target_value(&ev))
                        />
                    </label>
                    <div class="flex items-end gap-4 text-xs text-foreground">
                        <label class="inline-flex items-center gap-2">
                            <input
                                type="checkbox"
                                prop:checked=move || schema_binding_required.get()
                                on:change=move |ev| set_schema_binding_required.set(event_target_checked(&ev))
                            />
                            <span>{if is_ru { "Обязательный" } else { "Required" }}</span>
                        </label>
                        <label class="inline-flex items-center gap-2">
                            <input
                                type="checkbox"
                                prop:checked=move || schema_binding_disabled.get()
                                on:change=move |ev| set_schema_binding_disabled.set(event_target_checked(&ev))
                            />
                            <span>{if is_ru { "Отключён" } else { "Disabled" }}</span>
                        </label>
                    </div>
                </div>
                <button
                    type="button"
                    class=button_class
                    disabled=move || busy.get()
                    on:click=bind_schema
                >
                    {if is_ru { "Привязать атрибут к схеме" } else { "Bind attribute to schema" }}
                </button>
            </div>

            // 3. Category-scoped groups and attribute bindings
            <div class="space-y-3 rounded-xl border border-border/80 bg-muted/10 p-4">
                <h3 class="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    {if is_ru { "Категория: группы и привязки атрибутов" } else { "Category: groups and attribute bindings" }}
                </h3>
                <div class="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
                    <label class=label_class>
                        <span>{if is_ru { "Категория" } else { "Category" }}</span>
                        <select
                            class=select_class
                            prop:value=move || category_target.get()
                            on:change=move |ev| set_category_target.set(event_target_value(&ev))
                        >
                            <option value="">{if is_ru { "— выберите —" } else { "— select —" }}</option>
                            {move || categories().into_iter().map(|category| view! {
                                <option value=category.id.clone()>{category.name.clone()}</option>
                            }).collect_view()}
                        </select>
                    </label>
                    <label class=label_class>
                        <span>{if is_ru { "Код группы" } else { "Group code" }}</span>
                        <input
                            type="text"
                            class=input_class
                            placeholder="details"
                            prop:value=move || category_group_code.get()
                            on:input=move |ev| set_category_group_code.set(event_target_value(&ev))
                        />
                    </label>
                    <label class=label_class>
                        <span>{if is_ru { "Название группы" } else { "Group label" }}</span>
                        <input
                            type="text"
                            class=input_class
                            placeholder="Details"
                            prop:value=move || category_group_label.get()
                            on:input=move |ev| set_category_group_label.set(event_target_value(&ev))
                        />
                    </label>
                    <label class=label_class>
                        <span>{if is_ru { "Позиция" } else { "Position" }}</span>
                        <input
                            type="number"
                            class=input_class
                            prop:value=move || category_group_position.get()
                            on:input=move |ev| set_category_group_position.set(event_target_value(&ev))
                        />
                    </label>
                </div>
                <button
                    type="button"
                    class=button_class
                    disabled=move || busy.get()
                    on:click=create_category_group
                >
                    {if is_ru { "Создать группу категории" } else { "Create category group" }}
                </button>

                <div class="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
                    <label class=label_class>
                        <span>{if is_ru { "Атрибут" } else { "Attribute" }}</span>
                        <select
                            class=select_class
                            prop:value=move || category_attribute.get()
                            on:change=move |ev| set_category_attribute.set(event_target_value(&ev))
                        >
                            <option value="">{if is_ru { "— выберите —" } else { "— select —" }}</option>
                            {move || attributes().into_iter().map(|attribute| view! {
                                <option value=attribute.id.clone()>{attribute.label.clone()}</option>
                            }).collect_view()}
                        </select>
                    </label>
                    <label class=label_class>
                        <span>{if is_ru { "Группа (код)" } else { "Group (code)" }}</span>
                        <input
                            type="text"
                            class=input_class
                            placeholder="details"
                            prop:value=move || category_binding_group.get()
                            on:input=move |ev| set_category_binding_group.set(event_target_value(&ev))
                        />
                    </label>
                    <label class=label_class>
                        <span>{if is_ru { "Вид привязки" } else { "Binding kind" }}</span>
                        <select
                            class=select_class
                            prop:value=move || category_binding_kind.get()
                            on:change=move |ev| set_category_binding_kind.set(event_target_value(&ev))
                        >
                            <option value="addition">"addition"</option>
                            <option value="override">"override"</option>
                            <option value="removal">"removal"</option>
                        </select>
                    </label>
                    <div class="grid gap-3">
                        <label class=label_class>
                            <span>{if is_ru { "Позиция" } else { "Position" }}</span>
                            <input
                                type="number"
                                class=input_class
                                prop:value=move || category_binding_position.get()
                                on:input=move |ev| set_category_binding_position.set(event_target_value(&ev))
                            />
                        </label>
                        <label class="inline-flex items-center gap-2 text-xs text-foreground">
                            <input
                                type="checkbox"
                                prop:checked=move || category_binding_disabled.get()
                                on:change=move |ev| set_category_binding_disabled.set(event_target_checked(&ev))
                            />
                            <span>{if is_ru { "Отключён" } else { "Disabled" }}</span>
                        </label>
                    </div>
                </div>
                <button
                    type="button"
                    class=button_class
                    disabled=move || busy.get()
                    on:click=bind_category
                >
                    {if is_ru { "Привязать атрибут к категории" } else { "Bind attribute to category" }}
                </button>
            </div>
        </section>
    }.into_any()
}
