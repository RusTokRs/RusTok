use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
use rustok_grid::{ColumnFilters, GridPagination, RowSelection};
use rustok_grid_leptos::DataGrid;
use rustok_ui_core::{AdminQueryKey, UiRouteContext};

use crate::core::{shipping_option_list_request, shipping_profile_list_request};
use crate::core::{filter_shipping_options, shipping_option_grid_columns};
use crate::i18n::t;
use crate::model::{
    FulfillmentAdminBootstrap, ShippingOption, ShippingOptionDraft, ShippingProfile,
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
pub fn FulfillmentAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let selected_option_query = use_route_query_value(AdminQueryKey::ShippingOptionId.as_str());
    let query_writer = use_route_query_writer();
    let token = use_token();
    let tenant = use_tenant();
    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);

    let (editing_id, set_editing_id) = signal(Option::<String>::None);
    let (selected, set_selected) = signal(Option::<ShippingOption>::None);
    let (name, set_name) = signal(String::new());
    let (currency_code, set_currency_code) = signal("USD".to_string());
    let (amount, set_amount) = signal("0.00".to_string());
    let (provider_id, set_provider_id) = signal("manual".to_string());
    let (allowed_profiles, set_allowed_profiles) = signal(Vec::<String>::new());
    let (metadata_json, set_metadata_json) = signal(String::new());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);

    let bootstrap = local_resource(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            transport::fetch_bootstrap(token_value, tenant_value).await
        },
    );

    let shipping_options = local_resource(
        move || (token.get(), tenant.get(), refresh_nonce.get()),
        move |(token_value, tenant_value, _)| async move {
            let bootstrap =
                transport::fetch_bootstrap(token_value.clone(), tenant_value.clone()).await?;
            let request = shipping_option_list_request("", "", "");
            transport::fetch_shipping_options(transport::FetchShippingOptionsRequest {
                token: token_value,
                tenant_slug: tenant_value,
                tenant_id: bootstrap.current_tenant.id,
                filter: request,
            })
            .await
        },
    );

    let shipping_profiles = local_resource(
        move || (token.get(), tenant.get(), refresh_nonce.get()),
        move |(token_value, tenant_value, _)| async move {
            let bootstrap =
                transport::fetch_bootstrap(token_value.clone(), tenant_value.clone()).await?;
            let request = shipping_profile_list_request();
            transport::fetch_shipping_profiles(
                token_value,
                tenant_value,
                bootstrap.current_tenant.id,
                request.page,
                request.per_page,
            )
            .await
        },
    );

    let bootstrap_loading_label = t(
        ui_locale.as_deref(),
        "fulfillment.error.bootstrapLoading",
        "Bootstrap is still loading.",
    );
    let edit_label = t(ui_locale.as_deref(), "fulfillment.action.edit", "Edit");
    let new_label = t(ui_locale.as_deref(), "fulfillment.action.new", "New");
    let name_placeholder_label = t(ui_locale.as_deref(), "fulfillment.field.name", "Name");
    let currency_placeholder_label = t(
        ui_locale.as_deref(),
        "fulfillment.field.currency",
        "Currency",
    );
    let price_placeholder_label = t(ui_locale.as_deref(), "fulfillment.field.price", "Price");
    let provider_placeholder_label = t(
        ui_locale.as_deref(),
        "fulfillment.field.providerId",
        "Provider ID",
    );
    let metadata_placeholder_label = t(
        ui_locale.as_deref(),
        "fulfillment.field.metadataJsonPatch",
        "Metadata JSON patch",
    );
    let title_label = t(
        ui_locale.as_deref(),
        "fulfillment.title",
        "Fulfillment Control Room",
    );
    let subtitle_label = t(
        ui_locale.as_deref(),
        "fulfillment.subtitle",
        "Module-owned operator workspace for shipping-option lifecycle and compatibility rules.",
    );
    let shipping_options_title_label = t(
        ui_locale.as_deref(),
        "fulfillment.shippingOptions.title",
        "Shipping Options",
    );
    let shipping_options_subtitle_label = t(
        ui_locale.as_deref(),
        "fulfillment.shippingOptions.subtitle",
        "Review delivery options, provider bindings and shipping-profile compatibility rules.",
    );
    let no_shipping_options_label = t(
        ui_locale.as_deref(),
        "fulfillment.shippingOptions.empty",
        "No shipping options match the current filters.",
    );
    let load_shipping_options_error_label = t(
        ui_locale.as_deref(),
        "fulfillment.error.loadShippingOptions",
        "Failed to load shipping options",
    );
    let editor_label = t(
        ui_locale.as_deref(),
        "fulfillment.shippingOption.editor",
        "Shipping Option Editor",
    );
    let create_label = t(
        ui_locale.as_deref(),
        "fulfillment.shippingOption.create",
        "Create Shipping Option",
    );
    let editor_subtitle_label = t(
        ui_locale.as_deref(),
        "fulfillment.shippingOption.subtitle",
        "Typed operator surface over createShippingOption and updateShippingOption.",
    );
    let required_label = t(
        ui_locale.as_deref(),
        "fulfillment.error.shippingOptionNameRequired",
        "Shipping option name is required.",
    );
    let not_found_label = t(
        ui_locale.as_deref(),
        "fulfillment.error.shippingOptionNotFound",
        "Shipping option not found.",
    );
    let load_shipping_option_error_label = t(
        ui_locale.as_deref(),
        "fulfillment.error.loadShippingOption",
        "Failed to load shipping option",
    );
    let save_error_label = t(
        ui_locale.as_deref(),
        "fulfillment.error.saveShippingOption",
        "Failed to save shipping option",
    );

    let toggle_error_label = t(
        ui_locale.as_deref(),
        "fulfillment.error.changeShippingOptionStatus",
        "Failed to change shipping option status",
    );
    let allowed_profiles_label = t(
        ui_locale.as_deref(),
        "fulfillment.shippingOption.allowedProfiles",
        "Allowed shipping profiles",
    );
    let allow_all_label = t(
        ui_locale.as_deref(),
        "fulfillment.shippingOption.allowAll",
        "Allow all",
    );
    let no_profiles_label = t(
        ui_locale.as_deref(),
        "fulfillment.shippingOption.noProfiles",
        "No shipping profiles exist yet. Create a profile first or keep this option available to all carts.",
    );
    let registry_loading_label = t(
        ui_locale.as_deref(),
        "fulfillment.shippingOption.registryLoading",
        "Registry slugs are loading from the shipping-profile registry.",
    );
    let load_registry_error_label = t(
        ui_locale.as_deref(),
        "fulfillment.error.loadRegistrySlugs",
        "Failed to load registry slugs",
    );
    let selected_profiles_locale = ui_locale.clone();
    let save_button_label = t(
        ui_locale.as_deref(),
        "fulfillment.action.saveShippingOption",
        "Save shipping option",
    );
    let create_button_label = t(
        ui_locale.as_deref(),
        "fulfillment.action.createShippingOption",
        "Create shipping option",
    );
    let summary_empty_label = t(
        ui_locale.as_deref(),
        "fulfillment.summary.shippingOption.empty",
        "Open a shipping option to inspect its provider, pricing and shipping-profile compatibility set.",
    );
    let metadata_hint_label = t(
        ui_locale.as_deref(),
        "fulfillment.metadata.hint",
        "Metadata is sent as an optional JSON patch. Leaving the field blank during update keeps the existing metadata payload unchanged.",
    );

    let form_signals = ShippingOptionFormSignals {
        editing_id: set_editing_id,
        selected: set_selected,
        name: set_name,
        currency_code: set_currency_code,
        amount: set_amount,
        provider_id: set_provider_id,
        allowed_profiles: set_allowed_profiles,
        metadata_json: set_metadata_json,
    };

    let reset_form = move || {
        form_signals.clear();
    };

    let edit_bootstrap_loading_label = bootstrap_loading_label.clone();
    let edit_option = Callback::new(move |option_id: String| {
        let Some(FulfillmentAdminBootstrap { current_tenant }) =
            bootstrap.get_untracked().and_then(Result::ok)
        else {
            set_error.set(Some(edit_bootstrap_loading_label.clone()));
            return;
        };
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let load_error_label = load_shipping_option_error_label.clone();
        let not_found_label = not_found_label.clone();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            match transport::fetch_shipping_option(
                token_value,
                tenant_value,
                current_tenant.id,
                option_id,
            )
            .await
            {
                Ok(Some(option)) => form_signals.apply(&option),
                Ok(None) => {
                    form_signals.clear();
                    set_error.set(Some(not_found_label));
                }
                Err(err) => {
                    form_signals.clear();
                    set_error.set(Some(format!("{load_error_label}: {err}")));
                }
            }
            set_busy.set(false);
        });
    });

    let submit_bootstrap_loading_label = bootstrap_loading_label.clone();
    let submit_ui_locale = ui_locale.clone();
    let submit_query_writer = query_writer.clone();
    let submit_option = move |ev: SubmitEvent| {
        ev.prevent_default();
        let submit_query_writer = submit_query_writer.clone();
        let Some(FulfillmentAdminBootstrap { current_tenant }) =
            bootstrap.get_untracked().and_then(Result::ok)
        else {
            set_error.set(Some(submit_bootstrap_loading_label.clone()));
            return;
        };
        let submit_locale = submit_ui_locale.clone().unwrap_or_else(|| "en".to_string());

        let draft = ShippingOptionDraft {
            name: name.get_untracked().trim().to_string(),
            currency_code: currency_code.get_untracked().trim().to_string(),
            amount: amount.get_untracked().trim().to_string(),
            provider_id: provider_id.get_untracked().trim().to_string(),
            allowed_shipping_profile_slugs: allowed_profiles.get_untracked(),
            metadata_json: metadata_json.get_untracked().trim().to_string(),
            locale: submit_locale,
            existing_translations: selected
                .get_untracked()
                .map(|option| option.translations.clone())
                .unwrap_or_default(),
            expected_translation_revision: selected
                .get_untracked()
                .map(|option| option.translation_revision.clone()),
        };
        if draft.name.is_empty() {
            set_error.set(Some(required_label.clone()));
            return;
        }
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let current_id = editing_id.get_untracked();
        let save_error_label = save_error_label.clone();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            let result = match current_id {
                Some(option_id) => {
                    transport::update_shipping_option(
                        token_value.clone(),
                        tenant_value.clone(),
                        current_tenant.id.clone(),
                        option_id,
                        draft.clone(),
                    )
                    .await
                }
                None => {
                    transport::create_shipping_option(
                        token_value.clone(),
                        tenant_value.clone(),
                        current_tenant.id.clone(),
                        draft.clone(),
                    )
                    .await
                }
            };
            match result {
                Ok(option) => {
                    let option_id = option.id.clone();
                    form_signals.apply(&option);
                    set_refresh_nonce.update(|value| *value += 1);
                    submit_query_writer
                        .replace_value(AdminQueryKey::ShippingOptionId.as_str(), option_id);
                }
                Err(err) => set_error.set(Some(format!("{save_error_label}: {err}"))),
            }
            set_busy.set(false);
        });
    };

    let toggle_bootstrap_loading_label = bootstrap_loading_label.clone();
    let toggle_option = Callback::new(move |option: ShippingOption| {
        let Some(FulfillmentAdminBootstrap { current_tenant }) =
            bootstrap.get_untracked().and_then(Result::ok)
        else {
            set_error.set(Some(toggle_bootstrap_loading_label.clone()));
            return;
        };
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let toggle_error_label = toggle_error_label.clone();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            let result = if option.active {
                transport::deactivate_shipping_option(
                    token_value,
                    tenant_value,
                    current_tenant.id,
                    option.id.clone(),
                )
                .await
            } else {
                transport::reactivate_shipping_option(
                    token_value,
                    tenant_value,
                    current_tenant.id,
                    option.id.clone(),
                )
                .await
            };
            match result {
                Ok(updated) => {
                    if editing_id.get_untracked().as_deref() == Some(option.id.as_str()) {
                        form_signals.apply(&updated);
                    }
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(format!("{toggle_error_label}: {err}"))),
            }
            set_busy.set(false);
        });
    });

    let is_ru = ui_locale.as_deref().map(|l| l.starts_with("ru")).unwrap_or(false);
    let columns = shipping_option_grid_columns(ui_locale.as_deref());
    let filters = RwSignal::new(ColumnFilters::new());
    let selection = RwSignal::new(RowSelection::new());
    let pagination = RwSignal::new(GridPagination::new(1, 10, 0));

    let filtered_shipping_options = Memo::new(move |_| {
        let raw = shipping_options
            .get()
            .and_then(Result::ok)
            .map(|list| list.items)
            .unwrap_or_default();
        let current_filters = filters.get();
        if current_filters.is_empty() {
            raw
        } else {
            filter_shipping_options(&raw, &current_filters)
        }
    });

    let on_filters_change = Callback::new(move |new_filters: ColumnFilters| {
        filters.set(new_filters);
    });

    let on_row_click = {
        let click_writer = query_writer.clone();
        Callback::new(move |item: ShippingOption| {
            click_writer.push_value(AdminQueryKey::ShippingOptionId.as_str(), item.id);
        })
    };

    let cell_locale = ui_locale.clone();
    let cell_action_writer = query_writer.clone();
    let cell_edit_label = edit_label.clone();
    let cell_toggle_option = toggle_option;
    let cell_busy = busy;

    let cell_renderer = Callback::new(move |(item, col_id): (ShippingOption, String)| {
        match col_id.as_str() {
            "name" => {
                let name = item.name.clone();
                let id = item.id.clone();
                view! {
                    <div class="flex flex-col min-w-0">
                        <span class="text-xs font-semibold text-foreground truncate">{name}</span>
                        <span class="text-[10px] font-mono text-muted-foreground truncate">{id}</span>
                    </div>
                }
                .into_any()
            }
            "provider_id" => {
                let provider = item.provider_id.clone();
                view! {
                    <span class="inline-flex rounded-full border border-border bg-muted px-2 py-0.5 text-[10px] font-medium uppercase tracking-wider text-muted-foreground">
                        {provider}
                    </span>
                }
                .into_any()
            }
            "price" => {
                let price_str = format!("{} {}", item.currency_code, item.amount);
                view! {
                    <span class="text-xs font-medium text-foreground whitespace-nowrap text-right">
                        {price_str}
                    </span>
                }
                .into_any()
            }
            "profiles" => {
                let profiles_str = format_allowed_profiles(cell_locale.as_deref(), item.allowed_shipping_profile_slugs.as_ref());
                let title_str = profiles_str.clone();
                view! {
                    <span class="text-xs text-muted-foreground truncate" title=title_str>
                        {profiles_str}
                    </span>
                }
                .into_any()
            }
            "status" => {
                let badge_cls = active_badge(item.active);
                let label = localized_active_label(cell_locale.as_deref(), item.active);
                view! {
                    <span class=format!("inline-flex items-center px-2 py-0.5 rounded-full text-[11px] font-semibold border {badge_cls}")>
                        {label}
                    </span>
                }
                .into_any()
            }
            "updated_at" => {
                let date_str = item.updated_at.split('T').next().unwrap_or(&item.updated_at);
                view! {
                    <span class="text-xs text-muted-foreground whitespace-nowrap">
                        {date_str.to_string()}
                    </span>
                }
                .into_any()
            }
            "actions" => {
                let edit_id = item.id.clone();
                let toggle_item = item.clone();
                let item_writer = cell_action_writer.clone();
                let edit_btn_label = cell_edit_label.clone();
                let toggle_btn_label = if item.active {
                    t(cell_locale.as_deref(), "fulfillment.action.deactivate", "Deactivate")
                } else {
                    t(cell_locale.as_deref(), "fulfillment.action.reactivate", "Reactivate")
                };
                let toggle_fn = cell_toggle_option;
                let is_busy = cell_busy;
                view! {
                    <div class="flex items-center justify-center gap-1.5">
                        <button
                            type="button"
                            class="inline-flex items-center justify-center h-6 px-2 rounded-md text-[11px] font-medium border border-border bg-background text-foreground hover:bg-accent transition disabled:opacity-50"
                            disabled=move || is_busy.get()
                            on:click=move |ev| {
                                ev.stop_propagation();
                                item_writer.push_value(AdminQueryKey::ShippingOptionId.as_str(), edit_id.clone());
                            }
                        >
                            {edit_btn_label}
                        </button>
                        <button
                            type="button"
                            class="inline-flex items-center justify-center h-6 px-2 rounded-md text-[11px] font-medium border border-border bg-background text-foreground hover:bg-accent transition disabled:opacity-50"
                            disabled=move || is_busy.get()
                            on:click=move |ev| {
                                ev.stop_propagation();
                                toggle_fn.run(toggle_item.clone());
                            }
                        >
                            {toggle_btn_label}
                        </button>
                    </div>
                }
                .into_any()
            }
            _ => ().into_any(),
        }
    });

    let ui_locale_for_profiles = ui_locale.clone();
    let ui_locale_for_selected_profiles = ui_locale.clone();
    let ui_locale_for_summary = ui_locale.clone();
    let initial_edit_option = edit_option;
    let reset_current_option = Callback::new(move |_| {
        query_writer.clear_key(AdminQueryKey::ShippingOptionId.as_str());
        reset_form();
    });
    Effect::new(move |_| match selected_option_query.get() {
        Some(option_id) if !option_id.trim().is_empty() => {
            if bootstrap.get().and_then(Result::ok).is_none() {
                return;
            }
            initial_edit_option.run(option_id);
        }
        _ => {
            form_signals.clear();
        }
    });

    view! {
        <section class="space-y-6">
            <div class="rounded-3xl border border-border bg-card p-8 shadow-sm">
                <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">{t(ui_locale.as_deref(), "fulfillment.badge", "fulfillment")}</span>
                <h2 class="mt-4 text-3xl font-semibold text-card-foreground">{title_label.clone()}</h2>
                <p class="mt-2 max-w-3xl text-sm text-muted-foreground">{subtitle_label.clone()}</p>
            </div>

            <div class="grid gap-6 xl:grid-cols-[minmax(0,1.15fr)_minmax(0,0.85fr)]">
                <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                    <div class="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
                        <div>
                            <h3 class="text-lg font-semibold text-card-foreground">{shipping_options_title_label.clone()}</h3>
                            <p class="text-sm text-muted-foreground">{shipping_options_subtitle_label.clone()}</p>
                        </div>
                    </div>

                    // Selection toolbar when items selected
                    <Show when=move || !selection.get().is_empty()>
                        <div class="mt-4 flex items-center justify-between gap-3 bg-primary/5 border border-primary/20 rounded-xl px-4 py-2 animate-in fade-in duration-150">
                            <div class="flex items-center gap-2">
                                <span class="w-2 h-2 rounded-full bg-primary animate-pulse" />
                                <span class="text-xs font-semibold text-foreground">
                                    {move || format!("{} {}", selection.get().count(), if is_ru { "выбрано" } else { "selected" })}
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

                    <div class="mt-5">
                        <DataGrid
                            columns=columns
                            data=Signal::derive(move || filtered_shipping_options.get())
                            key_fn=|item: &ShippingOption| item.id.clone()
                            cell_renderer=cell_renderer
                            is_loading=Signal::derive(move || busy.get() || shipping_options.get().is_none())
                            empty_message=no_shipping_options_label.clone()
                            selection=selection
                            pagination=pagination
                            filters=filters
                            on_filter_change=on_filters_change
                            on_row_click=on_row_click
                        />
                    </div>

                    <Show when=move || shipping_options.get().and_then(Result::err).is_some()>
                        <div class="mt-4 rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                            {format!("{load_shipping_options_error_label}: {}", shipping_options.get().and_then(Result::err).map(|e| e.to_string()).unwrap_or_default())}
                        </div>
                    </Show>
                </section>

                <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                    <div class="flex items-center justify-between gap-3">
                        <div>
                            <h3 class="text-lg font-semibold text-card-foreground">{move || if editing_id.get().is_some() { editor_label.clone() } else { create_label.clone() }}</h3>
                            <p class="text-sm text-muted-foreground">{editor_subtitle_label.clone()}</p>
                        </div>
                        <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| reset_current_option.run(())>{new_label.clone()}</button>
                    </div>
                    <Show when=move || error.get().is_some()>
                        <div class="mt-4 rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{move || error.get().unwrap_or_default()}</div>
                    </Show>
                    <form class="mt-5 space-y-4" on:submit=submit_option>
                        <div class="grid gap-4 md:grid-cols-2">
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=name_placeholder_label.clone() prop:value=move || name.get() on:input=move |ev| set_name.set(event_target_value(&ev)) />
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=provider_placeholder_label.clone() prop:value=move || provider_id.get() on:input=move |ev| set_provider_id.set(event_target_value(&ev)) />
                        </div>
                        <div class="grid gap-4 md:grid-cols-2">
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=currency_placeholder_label.clone() prop:value=move || currency_code.get() on:input=move |ev| set_currency_code.set(event_target_value(&ev)) />
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=price_placeholder_label.clone() prop:value=move || amount.get() on:input=move |ev| set_amount.set(event_target_value(&ev)) />
                        </div>
                        <div class="space-y-3">
                            <div class="flex items-center justify-between gap-3">
                                <p class="text-sm font-medium text-card-foreground">{allowed_profiles_label.clone()}</p>
                                <button type="button" class="inline-flex rounded-lg border border-border px-3 py-1.5 text-xs font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| set_allowed_profiles.set(Vec::new())>{allow_all_label.clone()}</button>
                            </div>
                            <div class="flex flex-wrap gap-2">
                                {move || match shipping_profiles.get() {
                                    Some(Ok(list)) if !list.is_empty() => list.into_iter().map(|profile| {
                                        let profile_locale = ui_locale_for_profiles.clone();
                                        let slug = profile.slug.clone();
                                        let inactive_disabled_slug = slug.clone();
                                        let toggle_slug = slug.clone();
                                        let is_inactive = !profile.active;
                                        let label = profile_choice_label(profile_locale.as_deref(), &profile);
                                        view! {
                                            <button
                                                type="button"
                                                class=move || profile_chip_class(
                                                    slug_selected(&allowed_profiles.get(), slug.as_str()),
                                                    is_inactive,
                                                )
                                                disabled=move || busy.get() || (is_inactive && !slug_selected(&allowed_profiles.get(), inactive_disabled_slug.as_str()))
                                                on:click=move |_| {
                                                    set_allowed_profiles.update(|value| toggle_slug_selection(value, toggle_slug.as_str()));
                                                }
                                            >
                                                {label}
                                            </button>
                                        }
                                    }).collect_view().into_any(),
                                    Some(Ok(_)) => view! { <p class="text-sm text-muted-foreground">{no_profiles_label.clone()}</p> }.into_any(),
                                    Some(Err(err)) => view! { <p class="text-sm text-destructive">{format!("{load_registry_error_label}: {err}")}</p> }.into_any(),
                                    None => view! { <p class="text-sm text-muted-foreground">{registry_loading_label.clone()}</p> }.into_any(),
                                }}
                            </div>
                            <p class="text-xs text-muted-foreground">{move || crate::i18n::format(
                                selected_profiles_locale.as_deref(),
                                "fulfillment.shippingOption.selectedProfiles",
                                Some(&rustok_ui_i18n::fluent_args!("profiles" => format_selected_profiles(ui_locale_for_selected_profiles.as_deref(), &allowed_profiles.get()).to_string())),
                                "Selected profiles: {profiles}",
                            )}</p>
                        </div>
                        <textarea class="min-h-28 w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=metadata_placeholder_label.clone() prop:value=move || metadata_json.get() on:input=move |ev| set_metadata_json.set(event_target_value(&ev)) />
                        <button type="submit" class="inline-flex rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || busy.get()>{move || if editing_id.get().is_some() { save_button_label.clone() } else { create_button_label.clone() }}</button>
                    </form>
                    <div class="mt-5 rounded-2xl border border-border bg-background p-4 text-sm text-muted-foreground">
                        {move || selected.get().map(|option| summarize_shipping_option(ui_locale_for_summary.as_deref(), &option)).unwrap_or_else(|| summary_empty_label.clone())}
                    </div>
                    <p class="mt-3 text-xs text-muted-foreground">{metadata_hint_label.clone()}</p>
                </section>
            </div>
        </section>
    }
}

#[derive(Clone, Copy)]
struct ShippingOptionFormSignals {
    editing_id: WriteSignal<Option<String>>,
    selected: WriteSignal<Option<ShippingOption>>,
    name: WriteSignal<String>,
    currency_code: WriteSignal<String>,
    amount: WriteSignal<String>,
    provider_id: WriteSignal<String>,
    allowed_profiles: WriteSignal<Vec<String>>,
    metadata_json: WriteSignal<String>,
}

impl ShippingOptionFormSignals {
    fn apply(self, option: &ShippingOption) {
        self.editing_id.set(Some(option.id.clone()));
        self.selected.set(Some(option.clone()));
        self.name.set(option.name.clone());
        self.currency_code.set(option.currency_code.clone());
        self.amount.set(option.amount.clone());
        self.provider_id.set(option.provider_id.clone());
        self.allowed_profiles.set(
            option
                .allowed_shipping_profile_slugs
                .clone()
                .unwrap_or_default(),
        );
        self.metadata_json.set(option.metadata.clone());
    }

    fn clear(self) {
        self.editing_id.set(None);
        self.selected.set(None);
        self.name.set(String::new());
        self.currency_code.set("USD".to_string());
        self.amount.set("0.00".to_string());
        self.provider_id.set("manual".to_string());
        self.allowed_profiles.set(Vec::new());
        self.metadata_json.set(String::new());
    }
}

fn summarize_shipping_option(locale: Option<&str>, option: &ShippingOption) -> String {
    format!(
        "{} | {} {} | {} {} | {} {}",
        option.name,
        option.currency_code,
        option.amount,
        t(
            locale,
            "fulfillment.summary.shippingOption.provider",
            "provider"
        ),
        option.provider_id,
        t(
            locale,
            "fulfillment.summary.shippingOption.profiles",
            "profiles"
        ),
        format_allowed_profiles(locale, option.allowed_shipping_profile_slugs.as_ref())
    )
}

fn format_allowed_profiles(locale: Option<&str>, profiles: Option<&Vec<String>>) -> String {
    match profiles {
        Some(values) if !values.is_empty() => values.join(", "),
        _ => t(locale, "fulfillment.common.all", "all"),
    }
}

fn format_selected_profiles(locale: Option<&str>, values: &[String]) -> String {
    let slugs = normalize_slug_list(values);
    if slugs.is_empty() {
        t(locale, "fulfillment.common.allCarts", "all carts")
    } else {
        slugs.join(", ")
    }
}

fn profile_choice_label(locale: Option<&str>, profile: &ShippingProfile) -> String {
    if profile.active {
        format!("{} ({})", profile.name, profile.slug)
    } else {
        format!(
            "{} ({}, {})",
            profile.name,
            profile.slug,
            t(locale, "fulfillment.common.inactive", "inactive")
        )
    }
}

fn localized_active_label(locale: Option<&str>, active: bool) -> String {
    if active {
        t(locale, "fulfillment.common.active", "ACTIVE")
    } else {
        t(locale, "fulfillment.common.inactive", "INACTIVE")
    }
}

fn profile_chip_class(selected: bool, inactive: bool) -> &'static str {
    match (selected, inactive) {
        (true, false) => {
            "inline-flex rounded-full border border-primary bg-primary/10 px-3 py-2 text-xs font-medium text-primary transition hover:bg-primary/15"
        }
        (true, true) => {
            "inline-flex rounded-full border border-amber-300 bg-amber-50 px-3 py-2 text-xs font-medium text-amber-700 transition hover:bg-amber-100"
        }
        (false, true) => {
            "inline-flex rounded-full border border-border bg-muted px-3 py-2 text-xs font-medium text-muted-foreground opacity-60"
        }
        (false, false) => {
            "inline-flex rounded-full border border-border bg-background px-3 py-2 text-xs font-medium text-foreground transition hover:bg-accent"
        }
    }
}

fn toggle_slug_selection(current: &mut Vec<String>, slug: &str) {
    let slug = slug.trim();
    if slug.is_empty() {
        return;
    }
    let mut values = normalize_slug_list(current);
    if let Some(position) = values.iter().position(|value| value == slug) {
        values.remove(position);
    } else {
        values.push(slug.to_string());
        values.sort();
        values.dedup();
    }
    *current = values;
}

fn slug_selected(current: &[String], slug: &str) -> bool {
    normalize_slug_list(current)
        .iter()
        .any(|value| value == slug)
}

fn normalize_slug_list(current: &[String]) -> Vec<String> {
    current
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn active_badge(active: bool) -> &'static str {
    if active {
        "border-emerald-200 bg-emerald-50 text-emerald-700"
    } else {
        "border-slate-200 bg-slate-100 text-slate-700"
    }
}
