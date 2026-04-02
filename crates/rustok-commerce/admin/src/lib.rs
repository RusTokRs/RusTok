mod api;
mod model;

use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};

use crate::model::{ProductDetail, ProductDraft, ProductListItem};

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
    let (sku, set_sku) = signal(String::new());
    let (currency_code, set_currency_code) = signal("USD".to_string());
    let (amount, set_amount) = signal("0.00".to_string());
    let (inventory_quantity, set_inventory_quantity) = signal(0_i32);
    let (publish_now, set_publish_now) = signal(false);
    let (search, set_search) = signal(String::new());
    let (status_filter, set_status_filter) = signal(String::new());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);

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

    let reset_form = move || {
        set_editing_id.set(None);
        set_selected.set(None);
        set_title.set(String::new());
        set_handle.set(String::new());
        set_description.set(String::new());
        set_vendor.set(String::new());
        set_product_type.set(String::new());
        set_sku.set(String::new());
        set_currency_code.set("USD".to_string());
        set_amount.set("0.00".to_string());
        set_inventory_quantity.set(0);
        set_publish_now.set(false);
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
            shipping_profile_slug: None,
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

    view! {
        <section class="space-y-6">
            <div class="rounded-3xl border border-border bg-card p-8 shadow-sm">
                <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">"commerce"</span>
                <h2 class="mt-4 text-3xl font-semibold text-card-foreground">"Catalog Control Room"</h2>
                <p class="mt-2 max-w-3xl text-sm text-muted-foreground">"Module-owned product workspace for the ecommerce family. Product CRUD and publish flow now live inside the package instead of the host."</p>
            </div>
            <div class="grid gap-6 xl:grid-cols-[minmax(0,1.15fr)_minmax(0,0.85fr)]">
                <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                    <div class="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
                        <div><h3 class="text-lg font-semibold text-card-foreground">"Catalog Feed"</h3><p class="text-sm text-muted-foreground">"Search, publish, archive and open products for editing."</p></div>
                        <div class="flex flex-col gap-3 md:flex-row">
                            <input class="min-w-56 rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Search title" prop:value=move || search.get() on:input=move |ev| set_search.set(event_target_value(&ev)) />
                            <select class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" prop:value=move || status_filter.get() on:change=move |ev| set_status_filter.set(event_target_value(&ev))>
                                <option value="">"All statuses"</option><option value="DRAFT">"Draft"</option><option value="ACTIVE">"Active"</option><option value="ARCHIVED">"Archived"</option>
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
                                    view! {
                                        <article class="rounded-2xl border border-border bg-background p-5 transition hover:border-primary/40">
                                            <div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
                                                <div class="space-y-2">
                                                    <div class="flex flex-wrap items-center gap-2"><span class=format!("inline-flex rounded-full border px-3 py-1 text-xs font-semibold {}", status_badge(product.status.as_str()))>{product.status.clone()}</span><span class="text-xs uppercase tracking-[0.18em] text-muted-foreground">{product.product_type.clone().unwrap_or_else(|| "general".to_string())}</span></div>
                                                    <h4 class="text-base font-semibold text-card-foreground">{product.title.clone()}</h4>
                                                    <p class="text-sm text-muted-foreground">{format!("handle: {}{}", product.handle, product.vendor.as_ref().map(|v| format!(" • vendor: {v}")).unwrap_or_default())}</p>
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
                        <div><h3 class="text-lg font-semibold text-card-foreground">{move || if editing_id.get().is_some() { "Product Editor" } else { "Create Product" }}</h3><p class="text-sm text-muted-foreground">"Single-SKU oriented editor on top of module GraphQL mutations."</p></div>
                        <button type="button" class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| reset_form()>"New"</button>
                    </div>
                    <Show when=move || error.get().is_some()><div class="mt-4 rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{move || error.get().unwrap_or_default()}</div></Show>
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
                        <div class="grid gap-4 md:grid-cols-3">
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Primary SKU" prop:value=move || sku.get() on:input=move |ev| set_sku.set(event_target_value(&ev)) />
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Currency" prop:value=move || currency_code.get() on:input=move |ev| set_currency_code.set(event_target_value(&ev)) />
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Price" prop:value=move || amount.get() on:input=move |ev| set_amount.set(event_target_value(&ev)) />
                        </div>
                        <div class="grid gap-4 md:grid-cols-[minmax(0,1fr)_auto] md:items-center">
                            <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder="Inventory quantity" prop:value=move || inventory_quantity.get().to_string() on:input=move |ev| set_inventory_quantity.set(event_target_value(&ev).parse::<i32>().unwrap_or(0)) />
                            <label class="inline-flex items-center gap-3 rounded-2xl border border-border bg-background px-4 py-3 text-sm text-foreground"><input type="checkbox" prop:checked=move || publish_now.get() on:change=move |ev| set_publish_now.set(event_target_checked(&ev)) /><span>"Keep published after save"</span></label>
                        </div>
                        <button type="submit" class="inline-flex rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || busy.get()>{move || if editing_id.get().is_some() { "Save product" } else { "Create product" }}</button>
                    </form>
                    <div class="mt-5 rounded-2xl border border-border bg-background p-4 text-sm text-muted-foreground">
                        {move || selected.get().map(|product| summarize_selected(&product)).unwrap_or_else(|| "Open a product from the feed to inspect its localized copy and primary variant.".to_string())}
                    </div>
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
    format!(
        "{title} • status {} • primary variant price {price} • inventory {inventory}",
        product.status
    )
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
