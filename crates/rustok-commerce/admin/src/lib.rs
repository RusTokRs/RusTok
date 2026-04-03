mod api;
mod model;

use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};

use crate::model::{
    ProductDetail, ProductDraft, ProductListItem, ShippingOption, ShippingOptionDraft,
    ShippingProfile, ShippingProfileDraft,
};

#[component]
pub fn CommerceAdmin() -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();
    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);

    let (editing_id, set_editing_id) = signal(Option::<String>::None);
    let (selected, set_selected) = signal(Option::<ProductDetail>::None);
    let (locale, set_locale) = signal("en".to_string());
    let (title, set_title) = signal(String::new());
    let (handle, set_handle) = signal(String::new());
    let (description, set_description) = signal(String::new());
    let (vendor, set_vendor) = signal(String::new());
    let (product_type, set_product_type) = signal(String::new());
    let (product_shipping_profile_slug, set_product_shipping_profile_slug) = signal(String::new());
    let (sku, set_sku) = signal(String::new());
    let (currency_code, set_currency_code) = signal("USD".to_string());
    let (amount, set_amount) = signal("0.00".to_string());
    let (inventory_quantity, set_inventory_quantity) = signal(0_i32);
    let (publish_now, set_publish_now) = signal(false);
    let (search, set_search) = signal(String::new());
    let (status_filter, set_status_filter) = signal(String::new());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);

    let (shipping_editing_id, set_shipping_editing_id) = signal(Option::<String>::None);
    let (selected_shipping_option, set_selected_shipping_option) =
        signal(Option::<ShippingOption>::None);
    let (shipping_name, set_shipping_name) = signal(String::new());
    let (shipping_currency_code, set_shipping_currency_code) = signal("USD".to_string());
    let (shipping_amount, set_shipping_amount) = signal("0.00".to_string());
    let (shipping_provider_id, set_shipping_provider_id) = signal("manual".to_string());
    let (shipping_allowed_profiles, set_shipping_allowed_profiles) = signal(String::new());
    let (shipping_metadata_json, set_shipping_metadata_json) = signal(String::new());
    let (shipping_search, set_shipping_search) = signal(String::new());
    let (shipping_currency_filter, set_shipping_currency_filter) = signal(String::new());
    let (shipping_provider_filter, set_shipping_provider_filter) = signal(String::new());
    let (shipping_busy, set_shipping_busy) = signal(false);
    let (shipping_error, set_shipping_error) = signal(Option::<String>::None);

    let (shipping_profile_editing_id, set_shipping_profile_editing_id) =
        signal(Option::<String>::None);
    let (selected_shipping_profile, set_selected_shipping_profile) =
        signal(Option::<ShippingProfile>::None);
    let (shipping_profile_slug, set_shipping_profile_slug) = signal(String::new());
    let (shipping_profile_name, set_shipping_profile_name) = signal(String::new());
    let (shipping_profile_description, set_shipping_profile_description) = signal(String::new());
    let (shipping_profile_metadata_json, set_shipping_profile_metadata_json) =
        signal(String::new());
    let (shipping_profile_search, set_shipping_profile_search) = signal(String::new());
    let (shipping_profile_busy, set_shipping_profile_busy) = signal(false);
    let (shipping_profile_error, set_shipping_profile_error) = signal(Option::<String>::None);

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
                locale.get(),
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

    let shipping_options = Resource::new(
        move || {
            (
                token.get(),
                tenant.get(),
                refresh_nonce.get(),
                shipping_search.get(),
                shipping_currency_filter.get(),
                shipping_provider_filter.get(),
            )
        },
        move |(token_value, tenant_value, _, search_value, currency_filter, provider_filter)| async move {
            let bootstrap = api::fetch_bootstrap(token_value.clone(), tenant_value.clone()).await?;
            api::fetch_shipping_options(
                token_value,
                tenant_value,
                bootstrap.current_tenant.id,
                text_or_none(search_value),
                text_or_none(currency_filter),
                text_or_none(provider_filter),
            )
            .await
        },
    );

    let shipping_profiles = Resource::new(
        move || {
            (
                token.get(),
                tenant.get(),
                refresh_nonce.get(),
                shipping_profile_search.get(),
            )
        },
        move |(token_value, tenant_value, _, search_value)| async move {
            let bootstrap = api::fetch_bootstrap(token_value.clone(), tenant_value.clone()).await?;
            api::fetch_shipping_profiles(
                token_value,
                tenant_value,
                bootstrap.current_tenant.id,
                text_or_none(search_value),
            )
            .await
        },
    );

    let reset_form = move || {
        set_editing_id.set(None);
        set_selected.set(None);
        set_locale.set("en".to_string());
        set_title.set(String::new());
        set_handle.set(String::new());
        set_description.set(String::new());
        set_vendor.set(String::new());
        set_product_type.set(String::new());
        set_product_shipping_profile_slug.set(String::new());
        set_sku.set(String::new());
        set_currency_code.set("USD".to_string());
        set_amount.set("0.00".to_string());
        set_inventory_quantity.set(0);
        set_publish_now.set(false);
    };

    let reset_shipping_form = move || {
        set_shipping_editing_id.set(None);
        set_selected_shipping_option.set(None);
        set_shipping_name.set(String::new());
        set_shipping_currency_code.set("USD".to_string());
        set_shipping_amount.set("0.00".to_string());
        set_shipping_provider_id.set("manual".to_string());
        set_shipping_allowed_profiles.set(String::new());
        set_shipping_metadata_json.set(String::new());
    };

    let reset_shipping_profile_form = move || {
        set_shipping_profile_editing_id.set(None);
        set_selected_shipping_profile.set(None);
        set_shipping_profile_slug.set(String::new());
        set_shipping_profile_name.set(String::new());
        set_shipping_profile_description.set(String::new());
        set_shipping_profile_metadata_json.set(String::new());
    };

    let edit_product = Callback::new(move |product_id: String| {
        let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
            set_error.set(Some("Bootstrap is still loading.".to_string()));
            return;
        };
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let locale_value = locale.get_untracked();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            match api::fetch_product(
                token_value,
                tenant_value,
                bootstrap.current_tenant.id,
                product_id,
                locale_value,
            )
            .await
            {
                Ok(Some(product)) => apply_product(
                    &product,
                    set_editing_id,
                    set_selected,
                    set_locale,
                    set_title,
                    set_handle,
                    set_description,
                    set_vendor,
                    set_product_type,
                    set_product_shipping_profile_slug,
                    set_sku,
                    set_currency_code,
                    set_amount,
                    set_inventory_quantity,
                    set_publish_now,
                ),
                Ok(None) => set_error.set(Some("Product not found.".to_string())),
                Err(err) => set_error.set(Some(format!("Failed to load product: {err}"))),
            }
            set_busy.set(false);
        });
    });

    let edit_shipping_option = Callback::new(move |shipping_option_id: String| {
        let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
            set_shipping_error.set(Some("Bootstrap is still loading.".to_string()));
            return;
        };
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        set_shipping_busy.set(true);
        set_shipping_error.set(None);
        spawn_local(async move {
            match api::fetch_shipping_option(
                token_value,
                tenant_value,
                bootstrap.current_tenant.id,
                shipping_option_id,
            )
            .await
            {
                Ok(Some(option)) => apply_shipping_option(
                    &option,
                    set_shipping_editing_id,
                    set_selected_shipping_option,
                    set_shipping_name,
                    set_shipping_currency_code,
                    set_shipping_amount,
                    set_shipping_provider_id,
                    set_shipping_allowed_profiles,
                    set_shipping_metadata_json,
                ),
                Ok(None) => set_shipping_error.set(Some("Shipping option not found.".to_string())),
                Err(err) => {
                    set_shipping_error.set(Some(format!("Failed to load shipping option: {err}")))
                }
            }
            set_shipping_busy.set(false);
        });
    });

    let edit_shipping_profile = Callback::new(move |shipping_profile_id: String| {
        let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
            set_shipping_profile_error.set(Some("Bootstrap is still loading.".to_string()));
            return;
        };
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        set_shipping_profile_busy.set(true);
        set_shipping_profile_error.set(None);
        spawn_local(async move {
            match api::fetch_shipping_profile(
                token_value,
                tenant_value,
                bootstrap.current_tenant.id,
                shipping_profile_id,
            )
            .await
            {
                Ok(Some(profile)) => apply_shipping_profile(
                    &profile,
                    set_shipping_profile_editing_id,
                    set_selected_shipping_profile,
                    set_shipping_profile_slug,
                    set_shipping_profile_name,
                    set_shipping_profile_description,
                    set_shipping_profile_metadata_json,
                ),
                Ok(None) => {
                    set_shipping_profile_error.set(Some("Shipping profile not found.".to_string()))
                }
                Err(err) => set_shipping_profile_error
                    .set(Some(format!("Failed to load shipping profile: {err}"))),
            }
            set_shipping_profile_busy.set(false);
        });
    });

    let submit_product = move |ev: SubmitEvent| {
        ev.prevent_default();
        let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
            set_error.set(Some("Bootstrap is still loading.".to_string()));
            return;
        };
        let draft = ProductDraft {
            locale: locale.get_untracked().trim().to_string(),
            title: title.get_untracked().trim().to_string(),
            handle: handle.get_untracked().trim().to_string(),
            description: description.get_untracked().trim().to_string(),
            vendor: vendor.get_untracked().trim().to_string(),
            product_type: product_type.get_untracked().trim().to_string(),
            shipping_profile_slug: text_or_none(product_shipping_profile_slug.get_untracked()),
            sku: sku.get_untracked().trim().to_string(),
            barcode: String::new(),
            currency_code: currency_code.get_untracked().trim().to_string(),
            amount: amount.get_untracked().trim().to_string(),
            compare_at_amount: String::new(),
            inventory_quantity: inventory_quantity.get_untracked(),
            publish_now: publish_now.get_untracked(),
        };
        if draft.locale.is_empty() || draft.title.is_empty() {
            set_error.set(Some("Locale and title are required.".to_string()));
            return;
        }
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let current_id = editing_id.get_untracked();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            let saved = match current_id.clone() {
                Some(product_id) => {
                    api::update_product(
                        token_value.clone(),
                        tenant_value.clone(),
                        bootstrap.current_tenant.id.clone(),
                        bootstrap.me.id.clone(),
                        product_id,
                        draft.clone(),
                    )
                    .await
                }
                None => {
                    api::create_product(
                        token_value.clone(),
                        tenant_value.clone(),
                        bootstrap.current_tenant.id.clone(),
                        bootstrap.me.id.clone(),
                        draft.clone(),
                    )
                    .await
                }
            };
            match saved {
                Ok(mut product) => {
                    if draft.publish_now && product.status != "ACTIVE" {
                        if let Ok(published) = api::publish_product(
                            token_value.clone(),
                            tenant_value.clone(),
                            bootstrap.current_tenant.id.clone(),
                            bootstrap.me.id.clone(),
                            product.id.clone(),
                        )
                        .await
                        {
                            product = published;
                        }
                    }
                    if !draft.publish_now && product.status == "ACTIVE" {
                        if let Ok(drafted) = api::change_product_status(
                            token_value.clone(),
                            tenant_value.clone(),
                            bootstrap.current_tenant.id.clone(),
                            bootstrap.me.id.clone(),
                            product.id.clone(),
                            "DRAFT",
                        )
                        .await
                        {
                            product = drafted;
                        }
                    }
                    apply_product(
                        &product,
                        set_editing_id,
                        set_selected,
                        set_locale,
                        set_title,
                        set_handle,
                        set_description,
                        set_vendor,
                        set_product_type,
                        set_product_shipping_profile_slug,
                        set_sku,
                        set_currency_code,
                        set_amount,
                        set_inventory_quantity,
                        set_publish_now,
                    );
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(format!("Failed to save product: {err}"))),
            }
            set_busy.set(false);
        });
    };

    let submit_shipping_option = move |ev: SubmitEvent| {
        ev.prevent_default();
        let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
            set_shipping_error.set(Some("Bootstrap is still loading.".to_string()));
            return;
        };
        let draft = ShippingOptionDraft {
            name: shipping_name.get_untracked().trim().to_string(),
            currency_code: shipping_currency_code.get_untracked().trim().to_string(),
            amount: shipping_amount.get_untracked().trim().to_string(),
            provider_id: shipping_provider_id.get_untracked().trim().to_string(),
            allowed_shipping_profile_slugs: shipping_allowed_profiles
                .get_untracked()
                .trim()
                .to_string(),
            metadata_json: shipping_metadata_json.get_untracked().trim().to_string(),
        };
        if draft.name.is_empty() {
            set_shipping_error.set(Some("Shipping option name is required.".to_string()));
            return;
        }
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let current_id = shipping_editing_id.get_untracked();
        set_shipping_busy.set(true);
        set_shipping_error.set(None);
        spawn_local(async move {
            let saved = match current_id.clone() {
                Some(shipping_option_id) => {
                    api::update_shipping_option(
                        token_value.clone(),
                        tenant_value.clone(),
                        bootstrap.current_tenant.id.clone(),
                        shipping_option_id,
                        draft.clone(),
                    )
                    .await
                }
                None => {
                    api::create_shipping_option(
                        token_value.clone(),
                        tenant_value.clone(),
                        bootstrap.current_tenant.id.clone(),
                        draft.clone(),
                    )
                    .await
                }
            };
            match saved {
                Ok(option) => {
                    apply_shipping_option(
                        &option,
                        set_shipping_editing_id,
                        set_selected_shipping_option,
                        set_shipping_name,
                        set_shipping_currency_code,
                        set_shipping_amount,
                        set_shipping_provider_id,
                        set_shipping_allowed_profiles,
                        set_shipping_metadata_json,
                    );
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => {
                    set_shipping_error.set(Some(format!("Failed to save shipping option: {err}")))
                }
            }
            set_shipping_busy.set(false);
        });
    };

    let submit_shipping_profile = move |ev: SubmitEvent| {
        ev.prevent_default();
        let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
            set_shipping_profile_error.set(Some("Bootstrap is still loading.".to_string()));
            return;
        };
        let draft = ShippingProfileDraft {
            slug: shipping_profile_slug.get_untracked().trim().to_string(),
            name: shipping_profile_name.get_untracked().trim().to_string(),
            description: shipping_profile_description
                .get_untracked()
                .trim()
                .to_string(),
            metadata_json: shipping_profile_metadata_json
                .get_untracked()
                .trim()
                .to_string(),
        };
        if draft.slug.is_empty() || draft.name.is_empty() {
            set_shipping_profile_error.set(Some(
                "Shipping profile slug and name are required.".to_string(),
            ));
            return;
        }
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let current_id = shipping_profile_editing_id.get_untracked();
        set_shipping_profile_busy.set(true);
        set_shipping_profile_error.set(None);
        spawn_local(async move {
            let saved = match current_id.clone() {
                Some(shipping_profile_id) => {
                    api::update_shipping_profile(
                        token_value.clone(),
                        tenant_value.clone(),
                        bootstrap.current_tenant.id.clone(),
                        shipping_profile_id,
                        draft.clone(),
                    )
                    .await
                }
                None => {
                    api::create_shipping_profile(
                        token_value.clone(),
                        tenant_value.clone(),
                        bootstrap.current_tenant.id.clone(),
                        draft.clone(),
                    )
                    .await
                }
            };
            match saved {
                Ok(profile) => {
                    apply_shipping_profile(
                        &profile,
                        set_shipping_profile_editing_id,
                        set_selected_shipping_profile,
                        set_shipping_profile_slug,
                        set_shipping_profile_name,
                        set_shipping_profile_description,
                        set_shipping_profile_metadata_json,
                    );
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_shipping_profile_error
                    .set(Some(format!("Failed to save shipping profile: {err}"))),
            }
            set_shipping_profile_busy.set(false);
        });
    };

    let toggle_publish = Callback::new(move |product: ProductListItem| {
        let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
            set_error.set(Some("Bootstrap is still loading.".to_string()));
            return;
        };
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            let result = if product.status == "ACTIVE" {
                api::change_product_status(
                    token_value,
                    tenant_value,
                    bootstrap.current_tenant.id,
                    bootstrap.me.id,
                    product.id.clone(),
                    "DRAFT",
                )
                .await
            } else {
                api::publish_product(
                    token_value,
                    tenant_value,
                    bootstrap.current_tenant.id,
                    bootstrap.me.id,
                    product.id.clone(),
                )
                .await
            };
            match result {
                Ok(_) => set_refresh_nonce.update(|value| *value += 1),
                Err(err) => set_error.set(Some(format!("Failed to change status: {err}"))),
            }
            set_busy.set(false);
        });
    });

    let archive_product = Callback::new(move |product_id: String| {
        mutate_status(
            bootstrap.get_untracked().and_then(Result::ok),
            token.get_untracked(),
            tenant.get_untracked(),
            product_id,
            "ARCHIVED",
            set_busy,
            set_error,
            set_refresh_nonce,
        )
    });

    let delete_product = Callback::new(move |product_id: String| {
        delete_item(
            bootstrap.get_untracked().and_then(Result::ok),
            token.get_untracked(),
            tenant.get_untracked(),
            product_id,
            editing_id.get_untracked(),
            reset_form,
            set_busy,
            set_error,
            set_refresh_nonce,
        )
    });

    let toggle_shipping_option = Callback::new(move |option: ShippingOption| {
        let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
            set_shipping_error.set(Some("Bootstrap is still loading.".to_string()));
            return;
        };
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        set_shipping_busy.set(true);
        set_shipping_error.set(None);
        spawn_local(async move {
            let result = if option.active {
                api::deactivate_shipping_option(
                    token_value,
                    tenant_value,
                    bootstrap.current_tenant.id,
                    option.id.clone(),
                )
                .await
            } else {
                api::reactivate_shipping_option(
                    token_value,
                    tenant_value,
                    bootstrap.current_tenant.id,
                    option.id.clone(),
                )
                .await
            };
            match result {
                Ok(updated) => {
                    if shipping_editing_id.get_untracked().as_deref() == Some(option.id.as_str()) {
                        apply_shipping_option(
                            &updated,
                            set_shipping_editing_id,
                            set_selected_shipping_option,
                            set_shipping_name,
                            set_shipping_currency_code,
                            set_shipping_amount,
                            set_shipping_provider_id,
                            set_shipping_allowed_profiles,
                            set_shipping_metadata_json,
                        );
                    }
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_shipping_error.set(Some(format!(
                    "Failed to change shipping option status: {err}"
                ))),
            }
            set_shipping_busy.set(false);
        });
    });

    let toggle_shipping_profile = Callback::new(move |profile: ShippingProfile| {
        let Some(bootstrap) = bootstrap.get_untracked().and_then(Result::ok) else {
            set_shipping_profile_error.set(Some("Bootstrap is still loading.".to_string()));
            return;
        };
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        set_shipping_profile_busy.set(true);
        set_shipping_profile_error.set(None);
        spawn_local(async move {
            let result = if profile.active {
                api::deactivate_shipping_profile(
                    token_value,
                    tenant_value,
                    bootstrap.current_tenant.id,
                    profile.id.clone(),
                )
                .await
            } else {
                api::reactivate_shipping_profile(
                    token_value,
                    tenant_value,
                    bootstrap.current_tenant.id,
                    profile.id.clone(),
                )
                .await
            };
            match result {
                Ok(updated) => {
                    if shipping_profile_editing_id.get_untracked().as_deref()
                        == Some(profile.id.as_str())
                    {
                        apply_shipping_profile(
                            &updated,
                            set_shipping_profile_editing_id,
                            set_selected_shipping_profile,
                            set_shipping_profile_slug,
                            set_shipping_profile_name,
                            set_shipping_profile_description,
                            set_shipping_profile_metadata_json,
                        );
                    }
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_shipping_profile_error.set(Some(format!(
                    "Failed to change shipping profile status: {err}"
                ))),
            }
            set_shipping_profile_busy.set(false);
        });
    });

    view! {
        <section class="space-y-6">
            <div class="rounded-3xl border border-border bg-card p-8 shadow-sm">
                <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">"commerce"</span>
                <h2 class="mt-4 text-3xl font-semibold text-card-foreground">"Commerce Control Room"</h2>
                <p class="mt-2 max-w-3xl text-sm text-muted-foreground">"Module-owned operator workspace for catalog publishing, shipping-profile registry management and delivery option compatibility. Product CRUD, profile ownership and shipping-option rules all stay inside the commerce package."</p>
            </div>

            <div class="grid gap-6 xl:grid-cols-[minmax(0,1.15fr)_minmax(0,0.85fr)]">
                <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                    <div class="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
                        <div>
                            <h3 class="text-lg font-semibold text-card-foreground">"Catalog Feed"</h3>
                            <p class="text-sm text-muted-foreground">"Search, publish, archive and open products for editing."</p>
                        </div>
                        <div class="flex flex-col gap-3 md:flex-row">
                            <input class="min-w-56 rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Search title" prop:value=move || search.get() on:input=move |ev| set_search.set(event_target_value(&ev)) />
                            <select class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" prop:value=move || status_filter.get() on:change=move |ev| set_status_filter.set(event_target_value(&ev))>
                                <option value="">"All statuses"</option>
                                <option value="DRAFT">"Draft"</option>
                                <option value="ACTIVE">"Active"</option>
                                <option value="ARCHIVED">"Archived"</option>
                            </select>
                        </div>
                    </div>
                    <div class="mt-5 space-y-3">
                        {move || match products.get() {
                            None => view! { <div class="space-y-3"><div class="h-24 animate-pulse rounded-2xl bg-muted"></div><div class="h-24 animate-pulse rounded-2xl bg-muted"></div></div> }.into_any(),
                            Some(Ok(list)) if list.items.is_empty() => view! { <div class="rounded-2xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">"No products yet."</div> }.into_any(),
                            Some(Ok(list)) => view! { <>
                                {list.items.into_iter().map(|product| {
                                    let edit_id = product.id.clone();
                                    let archive_id = product.id.clone();
                                    let delete_id = product.id.clone();
                                    let publish_item = product.clone();
                                    let shipping_profile_label = product
                                        .shipping_profile_slug
                                        .clone()
                                        .map(|value| format!("profile {value}"));
                                    let shipping_profile_for_show = shipping_profile_label.clone();
                                    view! {
                                        <article class="rounded-2xl border border-border bg-background p-5 transition hover:border-primary/40">
                                            <div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
                                                <div class="space-y-2">
                                                    <div class="flex flex-wrap items-center gap-2">
                                                        <span class=format!("inline-flex rounded-full border px-3 py-1 text-xs font-semibold {}", status_badge(product.status.as_str()))>{product.status.clone()}</span>
                                                        <span class="text-xs uppercase tracking-[0.18em] text-muted-foreground">{product.product_type.clone().unwrap_or_else(|| "general".to_string())}</span>
                                                        <Show when=move || shipping_profile_for_show.is_some()>
                                                            <span class="text-xs text-muted-foreground">{shipping_profile_label.clone().unwrap_or_default()}</span>
                                                        </Show>
                                                    </div>
                                                    <h4 class="text-base font-semibold text-card-foreground">{product.title.clone()}</h4>
                                                    <p class="text-sm text-muted-foreground">{format!("handle: {}{}", product.handle, product.vendor.as_ref().map(|value| format!(" | vendor: {value}")).unwrap_or_default())}</p>
                                                    <p class="text-xs text-muted-foreground">{product.published_at.clone().unwrap_or_else(|| product.created_at.clone())}</p>
                                                </div>
                                                <div class="flex flex-wrap gap-2">
                                                    <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| edit_product.run(edit_id.clone())>"Edit"</button>
                                                    <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| toggle_publish.run(publish_item.clone())>{if product.status == "ACTIVE" { "Move to Draft" } else { "Publish" }}</button>
                                                    <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| archive_product.run(archive_id.clone())>"Archive"</button>
                                                    <button type="button" class="inline-flex rounded-lg border border-destructive/40 px-3 py-2 text-sm font-medium text-destructive transition hover:bg-destructive/10 disabled:opacity-50" disabled=move || busy.get() || product.status == "ACTIVE" on:click=move |_| delete_product.run(delete_id.clone())>"Delete"</button>
                                                </div>
                                            </div>
                                        </article>
                                    }
                                }).collect_view()}
                            </> }.into_any(),
                            Some(Err(err)) => view! { <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{format!("Failed to load products: {err}")}</div> }.into_any(),
                        }}
                    </div>
                </section>

                <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                    <div class="flex items-center justify-between gap-3">
                        <div>
                            <h3 class="text-lg font-semibold text-card-foreground">{move || if editing_id.get().is_some() { "Product Editor" } else { "Create Product" }}</h3>
                            <p class="text-sm text-muted-foreground">"Single-SKU product editor backed by the module GraphQL mutations."</p>
                        </div>
                        <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| reset_form()>"New"</button>
                    </div>
                    <Show when=move || error.get().is_some()>
                        <div class="mt-4 rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{move || error.get().unwrap_or_default()}</div>
                    </Show>
                    <form class="mt-5 space-y-4" on:submit=submit_product>
                        <div class="grid gap-4 md:grid-cols-2">
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Locale" prop:value=move || locale.get() on:input=move |ev| set_locale.set(event_target_value(&ev)) />
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Handle" prop:value=move || handle.get() on:input=move |ev| set_handle.set(event_target_value(&ev)) />
                        </div>
                        <input class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Title" prop:value=move || title.get() on:input=move |ev| set_title.set(event_target_value(&ev)) />
                        <textarea class="min-h-28 w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Description" prop:value=move || description.get() on:input=move |ev| set_description.set(event_target_value(&ev)) />
                        <div class="grid gap-4 md:grid-cols-2">
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Vendor" prop:value=move || vendor.get() on:input=move |ev| set_vendor.set(event_target_value(&ev)) />
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Product type" prop:value=move || product_type.get() on:input=move |ev| set_product_type.set(event_target_value(&ev)) />
                        </div>
                        <div class="space-y-2">
                            <select class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" prop:value=move || product_shipping_profile_slug.get() on:change=move |ev| set_product_shipping_profile_slug.set(event_target_value(&ev))>
                                <option value="">"No shipping profile"</option>
                                {move || match shipping_profiles.get() {
                                    Some(Ok(list)) => {
                                        let current_slug = product_shipping_profile_slug.get();
                                        list.items
                                            .into_iter()
                                            .filter(|profile| profile.active || profile.slug == current_slug)
                                            .map(|profile| {
                                                let label = shipping_profile_choice_label(&profile);
                                                let slug = profile.slug;
                                                view! { <option value=slug.clone()>{label}</option> }
                                            })
                                            .collect_view()
                                            .into_any()
                                    }
                                    _ => view! { <></> }.into_any(),
                                }}
                            </select>
                            <p class="text-xs text-muted-foreground">{move || shipping_profiles.get().and_then(Result::ok).map(|list| format!("Known profiles: {}", format_known_shipping_profiles(&list.items))).unwrap_or_else(|| "Known profiles are loading from the shipping-profile registry.".to_string())}</p>
                        </div>
                        <div class="grid gap-4 md:grid-cols-3">
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Primary SKU" prop:value=move || sku.get() on:input=move |ev| set_sku.set(event_target_value(&ev)) />
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Currency" prop:value=move || currency_code.get() on:input=move |ev| set_currency_code.set(event_target_value(&ev)) />
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Price" prop:value=move || amount.get() on:input=move |ev| set_amount.set(event_target_value(&ev)) />
                        </div>
                        <div class="grid gap-4 md:grid-cols-[minmax(0,1fr)_auto] md:items-center">
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Inventory quantity" prop:value=move || inventory_quantity.get().to_string() on:input=move |ev| set_inventory_quantity.set(event_target_value(&ev).parse::<i32>().unwrap_or(0)) />
                            <label class="inline-flex items-center gap-3 rounded-2xl border border-border bg-background px-4 py-3 text-sm text-foreground">
                                <input type="checkbox" prop:checked=move || publish_now.get() on:change=move |ev| set_publish_now.set(event_target_checked(&ev)) />
                                <span>"Keep published after save"</span>
                            </label>
                        </div>
                        <button type="submit" class="inline-flex rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || busy.get()>{move || if editing_id.get().is_some() { "Save product" } else { "Create product" }}</button>
                    </form>
                    <div class="mt-5 rounded-2xl border border-border bg-background p-4 text-sm text-muted-foreground">
                        {move || selected.get().map(|product| summarize_selected(&product)).unwrap_or_else(|| "Open a product from the feed to inspect its localized copy, primary variant and shipping-profile mapping.".to_string())}
                    </div>
                </section>
            </div>

            <div class="grid gap-6 xl:grid-cols-[minmax(0,1.15fr)_minmax(0,0.85fr)]">
                <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                    <div class="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
                        <div>
                            <h3 class="text-lg font-semibold text-card-foreground">"Shipping Options"</h3>
                            <p class="text-sm text-muted-foreground">"Review delivery options, provider bindings and shipping-profile compatibility rules."</p>
                        </div>
                        <div class="grid gap-3 md:grid-cols-3">
                            <input class="min-w-40 rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Search name" prop:value=move || shipping_search.get() on:input=move |ev| set_shipping_search.set(event_target_value(&ev)) />
                            <input class="min-w-32 rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Currency" prop:value=move || shipping_currency_filter.get() on:input=move |ev| set_shipping_currency_filter.set(event_target_value(&ev)) />
                            <input class="min-w-32 rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Provider" prop:value=move || shipping_provider_filter.get() on:input=move |ev| set_shipping_provider_filter.set(event_target_value(&ev)) />
                        </div>
                    </div>
                    <div class="mt-5 space-y-3">
                        {move || match shipping_options.get() {
                            None => view! { <div class="space-y-3"><div class="h-24 animate-pulse rounded-2xl bg-muted"></div><div class="h-24 animate-pulse rounded-2xl bg-muted"></div></div> }.into_any(),
                            Some(Ok(list)) if list.items.is_empty() => view! { <div class="rounded-2xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">"No shipping options match the current filters."</div> }.into_any(),
                            Some(Ok(list)) => view! { <>
                                {list.items.into_iter().map(|option| {
                                    let edit_id = option.id.clone();
                                    let toggle_item = option.clone();
                                    view! {
                                        <article class="rounded-2xl border border-border bg-background p-5 transition hover:border-primary/40">
                                            <div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
                                                <div class="space-y-2">
                                                    <div class="flex flex-wrap items-center gap-2">
                                                        <span class=format!("inline-flex rounded-full border px-3 py-1 text-xs font-semibold {}", shipping_option_active_badge(option.active))>{if option.active { "ACTIVE" } else { "INACTIVE" }}</span>
                                                        <span class="text-xs uppercase tracking-[0.18em] text-muted-foreground">{option.provider_id.clone()}</span>
                                                    </div>
                                                    <h4 class="text-base font-semibold text-card-foreground">{option.name.clone()}</h4>
                                                    <p class="text-sm text-muted-foreground">{format!("{} {}", option.currency_code, option.amount)}</p>
                                                    <p class="text-xs text-muted-foreground">{format!("profiles: {}", format_allowed_profiles(option.allowed_shipping_profile_slugs.as_ref()))}</p>
                                                    <p class="text-xs text-muted-foreground">{option.updated_at.clone()}</p>
                                                </div>
                                                <div class="flex flex-wrap gap-2">
                                                    <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || shipping_busy.get() on:click=move |_| edit_shipping_option.run(edit_id.clone())>"Edit"</button>
                                                    <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || shipping_busy.get() on:click=move |_| toggle_shipping_option.run(toggle_item.clone())>{if option.active { "Deactivate" } else { "Reactivate" }}</button>
                                                </div>
                                            </div>
                                        </article>
                                    }
                                }).collect_view()}
                            </> }.into_any(),
                            Some(Err(err)) => view! { <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{format!("Failed to load shipping options: {err}")}</div> }.into_any(),
                        }}
                    </div>
                </section>

                <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                    <div class="flex items-center justify-between gap-3">
                        <div>
                            <h3 class="text-lg font-semibold text-card-foreground">{move || if shipping_editing_id.get().is_some() { "Shipping Option Editor" } else { "Create Shipping Option" }}</h3>
                            <p class="text-sm text-muted-foreground">"Typed operator surface over createShippingOption and updateShippingOption."</p>
                        </div>
                        <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || shipping_busy.get() on:click=move |_| reset_shipping_form()>"New"</button>
                    </div>
                    <Show when=move || shipping_error.get().is_some()>
                        <div class="mt-4 rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{move || shipping_error.get().unwrap_or_default()}</div>
                    </Show>
                    <form class="mt-5 space-y-4" on:submit=submit_shipping_option>
                        <div class="grid gap-4 md:grid-cols-2">
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Name" prop:value=move || shipping_name.get() on:input=move |ev| set_shipping_name.set(event_target_value(&ev)) />
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Provider ID" prop:value=move || shipping_provider_id.get() on:input=move |ev| set_shipping_provider_id.set(event_target_value(&ev)) />
                        </div>
                        <div class="grid gap-4 md:grid-cols-2">
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Currency" prop:value=move || shipping_currency_code.get() on:input=move |ev| set_shipping_currency_code.set(event_target_value(&ev)) />
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Amount" prop:value=move || shipping_amount.get() on:input=move |ev| set_shipping_amount.set(event_target_value(&ev)) />
                        </div>
                        <div class="space-y-3">
                            <div class="rounded-2xl border border-border bg-background p-4">
                                <div class="flex items-center justify-between gap-3">
                                    <p class="text-sm font-medium text-card-foreground">"Allowed shipping profiles"</p>
                                    <button type="button" class="inline-flex rounded-lg border border-border px-3 py-1.5 text-xs font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || shipping_busy.get() on:click=move |_| set_shipping_allowed_profiles.set(String::new())>"Allow all"</button>
                                </div>
                                <div class="mt-3 flex flex-wrap gap-2">
                                    {move || match shipping_profiles.get() {
                                        Some(Ok(list)) if !list.items.is_empty() => list.items
                                            .into_iter()
                                            .map(|profile| {
                                                let slug = profile.slug.clone();
                                                let label = shipping_profile_choice_label(&profile);
                                                let inactive_disabled_slug = slug.clone();
                                                let toggle_slug = slug.clone();
                                                let is_inactive = !profile.active;
                                                view! {
                                                    <button
                                                        type="button"
                                                        class=move || shipping_profile_chip_class(
                                                            csv_contains(shipping_allowed_profiles.get().as_str(), slug.as_str()),
                                                            is_inactive,
                                                        )
                                                        disabled=move || shipping_busy.get() || (is_inactive && !csv_contains(shipping_allowed_profiles.get().as_str(), inactive_disabled_slug.as_str()))
                                                        on:click=move |_| {
                                                            set_shipping_allowed_profiles.update(|value| {
                                                                *value = toggle_csv_slug(value.as_str(), toggle_slug.as_str());
                                                            });
                                                        }
                                                    >
                                                        {label.clone()}
                                                    </button>
                                                }
                                            })
                                            .collect_view()
                                            .into_any(),
                                        Some(Ok(_)) => view! { <p class="text-sm text-muted-foreground">"No shipping profiles exist yet. Create a profile first or keep this option available to all carts."</p> }.into_any(),
                                        Some(Err(err)) => view! { <p class="text-sm text-destructive">{format!("Failed to load registry slugs: {err}")}</p> }.into_any(),
                                        None => view! { <p class="text-sm text-muted-foreground">"Registry slugs are loading from the shipping-profile registry."</p> }.into_any(),
                                    }}
                                </div>
                            </div>
                            <p class="text-xs text-muted-foreground">{move || format!("Selected profiles: {}", format_selected_shipping_profiles(shipping_allowed_profiles.get().as_str()))}</p>
                        </div>
                        <textarea class="min-h-28 w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Metadata JSON patch" prop:value=move || shipping_metadata_json.get() on:input=move |ev| set_shipping_metadata_json.set(event_target_value(&ev)) />
                        <button type="submit" class="inline-flex rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || shipping_busy.get()>{move || if shipping_editing_id.get().is_some() { "Save shipping option" } else { "Create shipping option" }}</button>
                    </form>
                    <div class="mt-5 rounded-2xl border border-border bg-background p-4 text-sm text-muted-foreground">
                        {move || selected_shipping_option.get().map(|option| summarize_shipping_option(&option)).unwrap_or_else(|| "Open a shipping option to inspect its provider, pricing and shipping-profile compatibility set.".to_string())}
                    </div>
                    <p class="mt-3 text-xs text-muted-foreground">"Metadata is sent as an optional JSON patch. Leaving the field blank during update keeps the existing metadata payload unchanged."</p>
                </section>
            </div>

            <div class="grid gap-6 xl:grid-cols-[minmax(0,1.15fr)_minmax(0,0.85fr)]">
                <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                    <div class="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
                        <div>
                            <h3 class="text-lg font-semibold text-card-foreground">"Shipping Profiles"</h3>
                            <p class="text-sm text-muted-foreground">"Manage the typed profile registry used by products and shipping-option compatibility rules."</p>
                        </div>
                        <div class="flex flex-col gap-3 md:flex-row">
                            <input class="min-w-56 rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Search slug or name" prop:value=move || shipping_profile_search.get() on:input=move |ev| set_shipping_profile_search.set(event_target_value(&ev)) />
                        </div>
                    </div>
                    <div class="mt-5 space-y-3">
                        {move || match shipping_profiles.get() {
                            None => view! { <div class="space-y-3"><div class="h-24 animate-pulse rounded-2xl bg-muted"></div><div class="h-24 animate-pulse rounded-2xl bg-muted"></div></div> }.into_any(),
                            Some(Ok(list)) if list.items.is_empty() => view! { <div class="rounded-2xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">"No shipping profiles match the current filters."</div> }.into_any(),
                            Some(Ok(list)) => view! { <>
                                {list.items.into_iter().map(|profile| {
                                    let edit_id = profile.id.clone();
                                    let toggle_item = profile.clone();
                                    let has_description = profile.description.is_some();
                                    let description = profile.description.clone().unwrap_or_default();
                                    view! {
                                        <article class="rounded-2xl border border-border bg-background p-5 transition hover:border-primary/40">
                                            <div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
                                                <div class="space-y-2">
                                                    <div class="flex flex-wrap items-center gap-2">
                                                        <span class=format!("inline-flex rounded-full border px-3 py-1 text-xs font-semibold {}", shipping_option_active_badge(profile.active))>{if profile.active { "ACTIVE" } else { "INACTIVE" }}</span>
                                                        <span class="text-xs uppercase tracking-[0.18em] text-muted-foreground">{profile.slug.clone()}</span>
                                                    </div>
                                                    <h4 class="text-base font-semibold text-card-foreground">{profile.name.clone()}</h4>
                                                    <Show when=move || has_description>
                                                        <p class="text-sm text-muted-foreground">{description.clone()}</p>
                                                    </Show>
                                                    <p class="text-xs text-muted-foreground">{profile.updated_at.clone()}</p>
                                                </div>
                                                <div class="flex flex-wrap gap-2">
                                                    <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || shipping_profile_busy.get() on:click=move |_| edit_shipping_profile.run(edit_id.clone())>"Edit"</button>
                                                    <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || shipping_profile_busy.get() on:click=move |_| toggle_shipping_profile.run(toggle_item.clone())>{if profile.active { "Deactivate" } else { "Reactivate" }}</button>
                                                </div>
                                            </div>
                                        </article>
                                    }
                                }).collect_view()}
                            </> }.into_any(),
                            Some(Err(err)) => view! { <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{format!("Failed to load shipping profiles: {err}")}</div> }.into_any(),
                        }}
                    </div>
                </section>

                <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                    <div class="flex items-center justify-between gap-3">
                        <div>
                            <h3 class="text-lg font-semibold text-card-foreground">{move || if shipping_profile_editing_id.get().is_some() { "Shipping Profile Editor" } else { "Create Shipping Profile" }}</h3>
                            <p class="text-sm text-muted-foreground">"Typed registry editor for the slugs referenced by products and shipping options."</p>
                        </div>
                        <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || shipping_profile_busy.get() on:click=move |_| reset_shipping_profile_form()>"New"</button>
                    </div>
                    <Show when=move || shipping_profile_error.get().is_some()>
                        <div class="mt-4 rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{move || shipping_profile_error.get().unwrap_or_default()}</div>
                    </Show>
                    <form class="mt-5 space-y-4" on:submit=submit_shipping_profile>
                        <div class="grid gap-4 md:grid-cols-2">
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Slug" prop:value=move || shipping_profile_slug.get() on:input=move |ev| set_shipping_profile_slug.set(event_target_value(&ev)) />
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Name" prop:value=move || shipping_profile_name.get() on:input=move |ev| set_shipping_profile_name.set(event_target_value(&ev)) />
                        </div>
                        <textarea class="min-h-24 w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Description" prop:value=move || shipping_profile_description.get() on:input=move |ev| set_shipping_profile_description.set(event_target_value(&ev)) />
                        <textarea class="min-h-28 w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Metadata JSON patch" prop:value=move || shipping_profile_metadata_json.get() on:input=move |ev| set_shipping_profile_metadata_json.set(event_target_value(&ev)) />
                        <button type="submit" class="inline-flex rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || shipping_profile_busy.get()>{move || if shipping_profile_editing_id.get().is_some() { "Save shipping profile" } else { "Create shipping profile" }}</button>
                    </form>
                    <div class="mt-5 rounded-2xl border border-border bg-background p-4 text-sm text-muted-foreground">
                        {move || selected_shipping_profile.get().map(|profile| summarize_shipping_profile(&profile)).unwrap_or_else(|| "Open a shipping profile to inspect its slug, description and lifecycle state.".to_string())}
                    </div>
                    <p class="mt-3 text-xs text-muted-foreground">"Metadata is sent as an optional JSON patch. Leaving the field blank during update keeps the existing metadata payload unchanged."</p>
                </section>
            </div>
        </section>
    }
}

fn apply_product(
    product: &ProductDetail,
    set_editing_id: WriteSignal<Option<String>>,
    set_selected: WriteSignal<Option<ProductDetail>>,
    set_locale: WriteSignal<String>,
    set_title: WriteSignal<String>,
    set_handle: WriteSignal<String>,
    set_description: WriteSignal<String>,
    set_vendor: WriteSignal<String>,
    set_product_type: WriteSignal<String>,
    set_shipping_profile_slug: WriteSignal<String>,
    set_sku: WriteSignal<String>,
    set_currency_code: WriteSignal<String>,
    set_amount: WriteSignal<String>,
    set_inventory_quantity: WriteSignal<i32>,
    set_publish_now: WriteSignal<bool>,
) {
    let translation = product.translations.first().cloned();
    let variant = product.variants.first().cloned();
    let price = variant
        .as_ref()
        .and_then(|item| item.prices.first().cloned());
    set_editing_id.set(Some(product.id.clone()));
    set_selected.set(Some(product.clone()));
    set_locale.set(
        translation
            .as_ref()
            .map(|item| item.locale.clone())
            .unwrap_or_else(|| "en".to_string()),
    );
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
    set_vendor.set(product.vendor.clone().unwrap_or_default());
    set_product_type.set(product.product_type.clone().unwrap_or_default());
    set_shipping_profile_slug.set(product.shipping_profile_slug.clone().unwrap_or_default());
    set_sku.set(variant.and_then(|item| item.sku).unwrap_or_default());
    set_currency_code.set(
        price
            .as_ref()
            .map(|item| item.currency_code.clone())
            .unwrap_or_else(|| "USD".to_string()),
    );
    set_amount.set(
        price
            .map(|item| item.amount)
            .unwrap_or_else(|| "0.00".to_string()),
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

fn apply_shipping_option(
    option: &ShippingOption,
    set_editing_id: WriteSignal<Option<String>>,
    set_selected: WriteSignal<Option<ShippingOption>>,
    set_name: WriteSignal<String>,
    set_currency_code: WriteSignal<String>,
    set_amount: WriteSignal<String>,
    set_provider_id: WriteSignal<String>,
    set_allowed_profiles: WriteSignal<String>,
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
            .unwrap_or_default()
            .join(", "),
    );
    set_metadata_json.set(option.metadata.clone());
}

fn apply_shipping_profile(
    profile: &ShippingProfile,
    set_editing_id: WriteSignal<Option<String>>,
    set_selected: WriteSignal<Option<ShippingProfile>>,
    set_slug: WriteSignal<String>,
    set_name: WriteSignal<String>,
    set_description: WriteSignal<String>,
    set_metadata_json: WriteSignal<String>,
) {
    set_editing_id.set(Some(profile.id.clone()));
    set_selected.set(Some(profile.clone()));
    set_slug.set(profile.slug.clone());
    set_name.set(profile.name.clone());
    set_description.set(profile.description.clone().unwrap_or_default());
    set_metadata_json.set(profile.metadata.clone());
}

fn mutate_status(
    bootstrap: Option<crate::model::CommerceAdminBootstrap>,
    token: Option<String>,
    tenant: Option<String>,
    product_id: String,
    status: &str,
    set_busy: WriteSignal<bool>,
    set_error: WriteSignal<Option<String>>,
    set_refresh_nonce: WriteSignal<u64>,
) {
    let Some(bootstrap) = bootstrap else {
        set_error.set(Some("Bootstrap is still loading.".to_string()));
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
            Err(err) => set_error.set(Some(format!("Failed to change status: {err}"))),
        }
        set_busy.set(false);
    });
}

fn delete_item(
    bootstrap: Option<crate::model::CommerceAdminBootstrap>,
    token: Option<String>,
    tenant: Option<String>,
    product_id: String,
    editing_id: Option<String>,
    reset_form: impl Fn() + 'static,
    set_busy: WriteSignal<bool>,
    set_error: WriteSignal<Option<String>>,
    set_refresh_nonce: WriteSignal<u64>,
) {
    let Some(bootstrap) = bootstrap else {
        set_error.set(Some("Bootstrap is still loading.".to_string()));
        return;
    };
    set_busy.set(true);
    set_error.set(None);
    spawn_local(async move {
        match api::delete_product(
            token,
            tenant,
            bootstrap.current_tenant.id,
            bootstrap.me.id,
            product_id.clone(),
        )
        .await
        {
            Ok(true) => {
                if editing_id.as_deref() == Some(product_id.as_str()) {
                    reset_form();
                }
                set_refresh_nonce.update(|value| *value += 1);
            }
            Ok(false) => set_error.set(Some("Delete returned false.".to_string())),
            Err(err) => set_error.set(Some(format!("Failed to delete product: {err}"))),
        }
        set_busy.set(false);
    });
}

fn summarize_selected(product: &ProductDetail) -> String {
    let title = product
        .translations
        .first()
        .map(|item| item.title.as_str())
        .unwrap_or("Untitled");
    let variant = product.variants.first();
    let price = variant
        .and_then(|item| item.prices.first())
        .map(|price| format!("{} {}", price.currency_code, price.amount))
        .unwrap_or_else(|| "no pricing".to_string());
    let inventory = variant.map(|item| item.inventory_quantity).unwrap_or(0);
    let shipping_profile = product
        .shipping_profile_slug
        .as_deref()
        .unwrap_or("unassigned");
    format!(
        "{title} | status {} | primary variant price {price} | inventory {inventory} | shipping profile {shipping_profile}",
        product.status
    )
}

fn summarize_shipping_option(option: &ShippingOption) -> String {
    format!(
        "{} | {} {} | provider {} | profiles {}",
        option.name,
        option.currency_code,
        option.amount,
        option.provider_id,
        format_allowed_profiles(option.allowed_shipping_profile_slugs.as_ref())
    )
}

fn summarize_shipping_profile(profile: &ShippingProfile) -> String {
    format!(
        "{} ({}) | {} | {}",
        profile.name,
        profile.slug,
        if profile.active { "active" } else { "inactive" },
        profile
            .description
            .clone()
            .unwrap_or_else(|| "no description".to_string())
    )
}

fn format_allowed_profiles(profiles: Option<&Vec<String>>) -> String {
    match profiles {
        Some(values) if !values.is_empty() => values.join(", "),
        _ => "all".to_string(),
    }
}

fn format_known_shipping_profiles(profiles: &[ShippingProfile]) -> String {
    let slugs = profiles
        .iter()
        .filter(|profile| profile.active)
        .map(|profile| profile.slug.as_str())
        .collect::<Vec<_>>();
    if slugs.is_empty() {
        "none yet".to_string()
    } else {
        slugs.join(", ")
    }
}

fn format_selected_shipping_profiles(value: &str) -> String {
    let slugs = csv_values(value);
    if slugs.is_empty() {
        "all carts".to_string()
    } else {
        slugs.join(", ")
    }
}

fn shipping_profile_choice_label(profile: &ShippingProfile) -> String {
    if profile.active {
        format!("{} ({})", profile.name, profile.slug)
    } else {
        format!("{} ({}, inactive)", profile.name, profile.slug)
    }
}

fn shipping_profile_chip_class(selected: bool, inactive: bool) -> &'static str {
    match (selected, inactive) {
        (true, false) => "inline-flex rounded-full border border-primary bg-primary/10 px-3 py-2 text-xs font-medium text-primary transition hover:bg-primary/15",
        (true, true) => "inline-flex rounded-full border border-amber-300 bg-amber-50 px-3 py-2 text-xs font-medium text-amber-700 transition hover:bg-amber-100",
        (false, true) => "inline-flex rounded-full border border-border bg-muted px-3 py-2 text-xs font-medium text-muted-foreground opacity-60",
        (false, false) => "inline-flex rounded-full border border-border bg-background px-3 py-2 text-xs font-medium text-foreground transition hover:bg-accent",
    }
}

fn toggle_csv_slug(current: &str, slug: &str) -> String {
    let slug = slug.trim();
    if slug.is_empty() {
        return current.trim().to_string();
    }
    let mut values = csv_values(current);
    if let Some(position) = values.iter().position(|value| value == slug) {
        values.remove(position);
    } else {
        values.push(slug.to_string());
        values.sort();
        values.dedup();
    }
    values.join(", ")
}

fn csv_contains(current: &str, slug: &str) -> bool {
    csv_values(current).iter().any(|value| value == slug)
}

fn csv_values(current: &str) -> Vec<String> {
    current
        .split(',')
        .map(str::trim)
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

fn status_badge(status: &str) -> &'static str {
    match status {
        "ACTIVE" => "border-emerald-200 bg-emerald-50 text-emerald-700",
        "ARCHIVED" => "border-slate-200 bg-slate-100 text-slate-700",
        _ => "border-amber-200 bg-amber-50 text-amber-700",
    }
}

fn shipping_option_active_badge(active: bool) -> &'static str {
    if active {
        "border-emerald-200 bg-emerald-50 text-emerald-700"
    } else {
        "border-slate-200 bg-slate-100 text-slate-700"
    }
}
