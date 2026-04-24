mod api;
mod i18n;
mod model;

use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
use rustok_api::{AdminQueryKey, UiRouteContext};

use crate::i18n::t;
use crate::model::{
    FulfillmentAdminBootstrap, ShippingOption, ShippingOptionDraft, ShippingProfile,
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
    let (search, set_search) = signal(String::new());
    let (currency_filter, set_currency_filter) = signal(String::new());
    let (provider_filter, set_provider_filter) = signal(String::new());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);

    let bootstrap = local_resource(
        move || (token.get(), tenant.get()),
        move |(token_value, tenant_value)| async move {
            api::fetch_bootstrap(token_value, tenant_value).await
        },
    );

    let shipping_options = local_resource(
        move || {
            (
                token.get(),
                tenant.get(),
                refresh_nonce.get(),
                search.get(),
                currency_filter.get(),
                provider_filter.get(),
            )
        },
        move |(token_value, tenant_value, _, search_value, currency_value, provider_value)| async move {
            let bootstrap = api::fetch_bootstrap(token_value.clone(), tenant_value.clone()).await?;
            api::fetch_shipping_options(
                token_value,
                tenant_value,
                bootstrap.current_tenant.id,
                text_or_none(search_value),
                text_or_none(currency_value),
                text_or_none(provider_value),
            )
            .await
        },
    );

    let shipping_profiles = local_resource(
        move || (token.get(), tenant.get(), refresh_nonce.get()),
        move |(token_value, tenant_value, _)| async move {
            let bootstrap = api::fetch_bootstrap(token_value.clone(), tenant_value.clone()).await?;
            api::fetch_shipping_profiles(token_value, tenant_value, bootstrap.current_tenant.id)
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
    let search_placeholder_label = t(
        ui_locale.as_deref(),
        "fulfillment.shippingOptions.searchPlaceholder",
        "Search name",
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
    let locale_unavailable_label = t(
        ui_locale.as_deref(),
        "fulfillment.error.localeUnavailable",
        "Host locale is unavailable.",
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
    let selected_profiles_template = t(
        ui_locale.as_deref(),
        "fulfillment.shippingOption.selectedProfiles",
        "Selected profiles: {profiles}",
    );
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

    let reset_form = move || {
        set_editing_id.set(None);
        set_selected.set(None);
        set_name.set(String::new());
        set_currency_code.set("USD".to_string());
        set_amount.set("0.00".to_string());
        set_provider_id.set("manual".to_string());
        set_allowed_profiles.set(Vec::new());
        set_metadata_json.set(String::new());
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
            match api::fetch_shipping_option(
                token_value,
                tenant_value,
                current_tenant.id,
                option_id,
            )
            .await
            {
                Ok(Some(option)) => apply_shipping_option(
                    &option,
                    set_editing_id,
                    set_selected,
                    set_name,
                    set_currency_code,
                    set_amount,
                    set_provider_id,
                    set_allowed_profiles,
                    set_metadata_json,
                ),
                Ok(None) => {
                    clear_shipping_option_form(
                        set_editing_id,
                        set_selected,
                        set_name,
                        set_currency_code,
                        set_amount,
                        set_provider_id,
                        set_allowed_profiles,
                        set_metadata_json,
                    );
                    set_error.set(Some(not_found_label));
                }
                Err(err) => {
                    clear_shipping_option_form(
                        set_editing_id,
                        set_selected,
                        set_name,
                        set_currency_code,
                        set_amount,
                        set_provider_id,
                        set_allowed_profiles,
                        set_metadata_json,
                    );
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
        let Some(submit_locale) = submit_ui_locale.clone() else {
            set_error.set(Some(locale_unavailable_label.clone()));
            return;
        };
        let draft = ShippingOptionDraft {
            name: name.get_untracked().trim().to_string(),
            currency_code: currency_code.get_untracked().trim().to_string(),
            amount: amount.get_untracked().trim().to_string(),
            provider_id: provider_id.get_untracked().trim().to_string(),
            allowed_shipping_profile_slugs: allowed_profiles.get_untracked(),
            metadata_json: metadata_json.get_untracked().trim().to_string(),
            locale: submit_locale,
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
                    api::update_shipping_option(
                        token_value.clone(),
                        tenant_value.clone(),
                        current_tenant.id.clone(),
                        option_id,
                        draft.clone(),
                    )
                    .await
                }
                None => {
                    api::create_shipping_option(
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
                    apply_shipping_option(
                        &option,
                        set_editing_id,
                        set_selected,
                        set_name,
                        set_currency_code,
                        set_amount,
                        set_provider_id,
                        set_allowed_profiles,
                        set_metadata_json,
                    );
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
                api::deactivate_shipping_option(
                    token_value,
                    tenant_value,
                    current_tenant.id,
                    option.id.clone(),
                )
                .await
            } else {
                api::reactivate_shipping_option(
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
                        apply_shipping_option(
                            &updated,
                            set_editing_id,
                            set_selected,
                            set_name,
                            set_currency_code,
                            set_amount,
                            set_provider_id,
                            set_allowed_profiles,
                            set_metadata_json,
                        );
                    }
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(format!("{toggle_error_label}: {err}"))),
            }
            set_busy.set(false);
        });
    });

    let ui_locale_for_list = ui_locale.clone();
    let ui_locale_for_profiles = ui_locale.clone();
    let ui_locale_for_selected_profiles = ui_locale.clone();
    let ui_locale_for_summary = ui_locale.clone();
    let initial_edit_option = edit_option.clone();
    let list_query_writer = query_writer.clone();
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
            clear_shipping_option_form(
                set_editing_id,
                set_selected,
                set_name,
                set_currency_code,
                set_amount,
                set_provider_id,
                set_allowed_profiles,
                set_metadata_json,
            );
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
                        <div class="flex flex-col gap-3 md:flex-row">
                            <input class="min-w-40 rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=search_placeholder_label.clone() prop:value=move || search.get() on:input=move |ev| set_search.set(event_target_value(&ev)) />
                            <input class="min-w-32 rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=currency_placeholder_label.clone() prop:value=move || currency_filter.get() on:input=move |ev| set_currency_filter.set(event_target_value(&ev)) />
                            <input class="min-w-32 rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=provider_placeholder_label.clone() prop:value=move || provider_filter.get() on:input=move |ev| set_provider_filter.set(event_target_value(&ev)) />
                        </div>
                    </div>
                    <div class="mt-5 space-y-3">
                        {move || match shipping_options.get() {
                            None => view! { <div class="space-y-3"><div class="h-24 animate-pulse rounded-2xl bg-muted"></div><div class="h-24 animate-pulse rounded-2xl bg-muted"></div></div> }.into_any(),
                            Some(Ok(list)) if list.items.is_empty() => view! { <div class="rounded-2xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">{no_shipping_options_label.clone()}</div> }.into_any(),
                            Some(Ok(list)) => list.items.into_iter().map(|option| {
                                let item_locale = ui_locale_for_list.clone();
                                let edit_id = option.id.clone();
                                let toggle_item = option.clone();
                                let item_query_writer = list_query_writer.clone();
                                let active_label = localized_active_label(item_locale.as_deref(), option.active);
                                let toggle_label = if option.active {
                                    t(item_locale.as_deref(), "fulfillment.action.deactivate", "Deactivate")
                                } else {
                                    t(item_locale.as_deref(), "fulfillment.action.reactivate", "Reactivate")
                                };
                                let profiles_label = t(item_locale.as_deref(), "fulfillment.shippingOption.profilesMeta", "profiles: {profiles}")
                                    .replace("{profiles}", format_allowed_profiles(item_locale.as_deref(), option.allowed_shipping_profile_slugs.as_ref()).as_str());
                                view! {
                                    <article class="rounded-2xl border border-border bg-background p-5 transition hover:border-primary/40">
                                        <div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
                                            <div class="space-y-2">
                                                <div class="flex flex-wrap items-center gap-2">
                                                    <span class=format!("inline-flex rounded-full border px-3 py-1 text-xs font-semibold {}", active_badge(option.active))>{active_label}</span>
                                                    <span class="text-xs uppercase tracking-[0.18em] text-muted-foreground">{option.provider_id.clone()}</span>
                                                    <span class="text-xs text-muted-foreground">{format!("{} {}", option.currency_code, option.amount)}</span>
                                                </div>
                                                <h4 class="text-base font-semibold text-card-foreground">{option.name.clone()}</h4>
                                                <p class="text-sm text-muted-foreground">{profiles_label}</p>
                                                <p class="text-xs text-muted-foreground">{option.updated_at.clone()}</p>
                                            </div>
                                            <div class="flex flex-wrap gap-2">
                                                <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| item_query_writer.push_value(AdminQueryKey::ShippingOptionId.as_str(), edit_id.clone())>{edit_label.clone()}</button>
                                                <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| toggle_option.run(toggle_item.clone())>{toggle_label}</button>
                                            </div>
                                        </div>
                                    </article>
                                }
                            }).collect_view().into_any(),
                            Some(Err(err)) => view! { <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{format!("{load_shipping_options_error_label}: {err}")}</div> }.into_any(),
                        }}
                    </div>
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
                            <p class="text-xs text-muted-foreground">{move || selected_profiles_template.replace("{profiles}", format_selected_profiles(ui_locale_for_selected_profiles.as_deref(), &allowed_profiles.get()).as_str())}</p>
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

fn apply_shipping_option(
    option: &ShippingOption,
    set_editing_id: WriteSignal<Option<String>>,
    set_selected: WriteSignal<Option<ShippingOption>>,
    set_name: WriteSignal<String>,
    set_currency_code: WriteSignal<String>,
    set_amount: WriteSignal<String>,
    set_provider_id: WriteSignal<String>,
    set_allowed_profiles: WriteSignal<Vec<String>>,
    set_metadata_json: WriteSignal<String>,
) {
    set_editing_id.set(Some(option.id.clone()));
    set_selected.set(Some(option.clone()));
    set_name.set(option.name.clone());
    set_currency_code.set(option.currency_code.clone());
    set_amount.set(option.amount.clone());
    set_provider_id.set(option.provider_id.clone());
    set_allowed_profiles.set(
        option
            .allowed_shipping_profile_slugs
            .clone()
            .unwrap_or_default(),
    );
    set_metadata_json.set(option.metadata.clone());
}

fn clear_shipping_option_form(
    set_editing_id: WriteSignal<Option<String>>,
    set_selected: WriteSignal<Option<ShippingOption>>,
    set_name: WriteSignal<String>,
    set_currency_code: WriteSignal<String>,
    set_amount: WriteSignal<String>,
    set_provider_id: WriteSignal<String>,
    set_allowed_profiles: WriteSignal<Vec<String>>,
    set_metadata_json: WriteSignal<String>,
) {
    set_editing_id.set(None);
    set_selected.set(None);
    set_name.set(String::new());
    set_currency_code.set("USD".to_string());
    set_amount.set("0.00".to_string());
    set_provider_id.set("manual".to_string());
    set_allowed_profiles.set(Vec::new());
    set_metadata_json.set(String::new());
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
        (true, false) => "inline-flex rounded-full border border-primary bg-primary/10 px-3 py-2 text-xs font-medium text-primary transition hover:bg-primary/15",
        (true, true) => "inline-flex rounded-full border border-amber-300 bg-amber-50 px-3 py-2 text-xs font-medium text-amber-700 transition hover:bg-amber-100",
        (false, true) => "inline-flex rounded-full border border-border bg-muted px-3 py-2 text-xs font-medium text-muted-foreground opacity-60",
        (false, false) => "inline-flex rounded-full border border-border bg-background px-3 py-2 text-xs font-medium text-foreground transition hover:bg-accent",
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

fn text_or_none(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn active_badge(active: bool) -> &'static str {
    if active {
        "border-emerald-200 bg-emerald-50 text-emerald-700"
    } else {
        "border-slate-200 bg-slate-100 text-slate-700"
    }
}
