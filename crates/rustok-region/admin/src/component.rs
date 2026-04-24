use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
use rustok_api::{AdminQueryKey, UiRouteContext};

use crate::i18n::t;
use crate::model::{RegionAdminBootstrap, RegionDetail, RegionDraft, RegionListItem};

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
pub fn RegionAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let selected_region_query = use_route_query_value(AdminQueryKey::RegionId.as_str());
    let query_writer = use_route_query_writer();

    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (editing_id, set_editing_id) = signal(Option::<String>::None);
    let (selected, set_selected) = signal(Option::<RegionDetail>::None);
    let (name, set_name) = signal(String::new());
    let (currency_code, set_currency_code) = signal(String::new());
    let (tax_provider_id, set_tax_provider_id) = signal(String::new());
    let (tax_rate, set_tax_rate) = signal("0".to_string());
    let (tax_included, set_tax_included) = signal(false);
    let (country_tax_policies, set_country_tax_policies) = signal("[]".to_string());
    let (countries, set_countries) = signal(String::new());
    let (metadata, set_metadata) = signal("{}".to_string());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);

    let bootstrap = local_resource(
        move || refresh_nonce.get(),
        move |_| async move { crate::api::fetch_bootstrap().await },
    );

    let regions = local_resource(
        move || refresh_nonce.get(),
        move |_| async move { crate::api::fetch_regions().await },
    );

    let required_name_label = t(
        ui_locale.as_deref(),
        "region.error.nameRequired",
        "Name is required.",
    );
    let required_currency_label = t(
        ui_locale.as_deref(),
        "region.error.currencyRequired",
        "Currency code is required.",
    );
    let locale_unavailable_label = t(
        ui_locale.as_deref(),
        "region.error.localeUnavailable",
        "Host locale is unavailable.",
    );
    let required_countries_label = t(
        ui_locale.as_deref(),
        "region.error.countriesRequired",
        "At least one country code is required.",
    );
    let load_region_error_label = t(
        ui_locale.as_deref(),
        "region.error.loadRegion",
        "Failed to load region",
    );
    let save_region_error_label = t(
        ui_locale.as_deref(),
        "region.error.saveRegion",
        "Failed to save region",
    );
    let load_regions_error_label = t(
        ui_locale.as_deref(),
        "region.error.loadRegions",
        "Failed to load regions",
    );

    let reset_form = move || {
        set_editing_id.set(None);
        set_selected.set(None);
        set_name.set(String::new());
        set_currency_code.set(String::new());
        set_tax_provider_id.set(String::new());
        set_tax_rate.set("0".to_string());
        set_tax_included.set(false);
        set_country_tax_policies.set("[]".to_string());
        set_countries.set(String::new());
        set_metadata.set("{}".to_string());
        set_error.set(None);
    };

    let open_load_region_error_label = load_region_error_label.clone();
    let open_region = Callback::new(move |region_id: String| {
        let load_region_error_label = open_load_region_error_label.clone();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            match crate::api::fetch_region_detail(region_id).await {
                Ok(detail) => apply_region_detail(
                    &detail,
                    set_editing_id,
                    set_selected,
                    set_name,
                    set_currency_code,
                    set_tax_provider_id,
                    set_tax_rate,
                    set_tax_included,
                    set_country_tax_policies,
                    set_countries,
                    set_metadata,
                ),
                Err(err) => {
                    clear_region_form(
                        set_editing_id,
                        set_selected,
                        set_name,
                        set_currency_code,
                        set_tax_provider_id,
                        set_tax_rate,
                        set_tax_included,
                        set_country_tax_policies,
                        set_countries,
                        set_metadata,
                    );
                    set_error.set(Some(format!("{load_region_error_label}: {err}")));
                }
            }
            set_busy.set(false);
        });
    });
    let initial_open_region = open_region.clone();
    Effect::new(move |_| match selected_region_query.get() {
        Some(region_id) if !region_id.trim().is_empty() => {
            initial_open_region.run(region_id);
        }
        _ => {
            clear_region_form(
                set_editing_id,
                set_selected,
                set_name,
                set_currency_code,
                set_tax_provider_id,
                set_tax_rate,
                set_tax_included,
                set_country_tax_policies,
                set_countries,
                set_metadata,
            );
        }
    });

    let submit_ui_locale = ui_locale.clone();
    let submit_query_writer = query_writer.clone();
    let on_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        let submit_query_writer = submit_query_writer.clone();

        if name.get_untracked().trim().is_empty() {
            set_error.set(Some(required_name_label.clone()));
            return;
        }
        if currency_code.get_untracked().trim().is_empty() {
            set_error.set(Some(required_currency_label.clone()));
            return;
        }
        if countries.get_untracked().trim().is_empty() {
            set_error.set(Some(required_countries_label.clone()));
            return;
        }
        let Some(submit_locale) = submit_ui_locale.clone() else {
            set_error.set(Some(locale_unavailable_label.clone()));
            return;
        };

        let payload = RegionDraft {
            name: name.get_untracked(),
            locale: submit_locale,
            currency_code: currency_code.get_untracked(),
            tax_provider_id: tax_provider_id.get_untracked(),
            tax_rate: tax_rate.get_untracked(),
            tax_included: tax_included.get_untracked(),
            country_tax_policies: country_tax_policies.get_untracked(),
            countries: countries.get_untracked(),
            metadata: metadata.get_untracked(),
        };
        let editing_region_id = editing_id.get_untracked();
        let save_region_error_label = save_region_error_label.clone();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            let result = match editing_region_id {
                Some(region_id) => crate::api::update_region(region_id, payload).await,
                None => crate::api::create_region(payload).await,
            };

            match result {
                Ok(detail) => {
                    let detail_id = detail.region.id.clone();
                    apply_region_detail(
                        &detail,
                        set_editing_id,
                        set_selected,
                        set_name,
                        set_currency_code,
                        set_tax_provider_id,
                        set_tax_rate,
                        set_tax_included,
                        set_country_tax_policies,
                        set_countries,
                        set_metadata,
                    );
                    set_refresh_nonce.update(|value| *value += 1);
                    submit_query_writer.replace_value(AdminQueryKey::RegionId.as_str(), detail_id);
                }
                Err(err) => set_error.set(Some(format!("{save_region_error_label}: {err}"))),
            }

            set_busy.set(false);
        });
    };

    let ui_locale_for_list_heading = ui_locale.clone();
    let ui_locale_for_list = ui_locale.clone();
    let ui_locale_for_detail = ui_locale.clone();
    let ui_locale_for_empty = ui_locale.clone();
    let ui_locale_for_editor_heading = ui_locale.clone();
    let ui_locale_for_editor = ui_locale.clone();
    let list_query_writer = query_writer.clone();
    let reset_query_writer = query_writer.clone();

    view! {
        <section class="space-y-6">
            <header class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-3">
                    <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                        {t(ui_locale.as_deref(), "region.badge", "region")}
                    </span>
                    <h2 class="text-2xl font-semibold text-card-foreground">
                        {t(ui_locale.as_deref(), "region.title", "Region Operations")}
                    </h2>
                    <p class="max-w-3xl text-sm text-muted-foreground">
                        {t(ui_locale.as_deref(), "region.subtitle", "Module-owned region workspace for tenant-scoped country, currency and tax baseline management without routing operator CRUD back through the commerce umbrella.")}
                    </p>
                </div>
            </header>

            <Show when=move || error.get().is_some()>
                <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>

            <div class="grid gap-6 xl:grid-cols-[minmax(0,1.02fr)_minmax(0,0.98fr)]">
                <section class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                    <div class="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
                        <div>
                            <h3 class="text-lg font-semibold text-card-foreground">
                                {t(ui_locale.as_deref(), "region.list.title", "Regions")}
                            </h3>
                            <p class="text-sm text-muted-foreground">
                                {move || bootstrap.get().and_then(Result::ok).map(|payload: RegionAdminBootstrap| {
                                    t(ui_locale_for_list_heading.as_deref(), "region.list.subtitle", "Tenant {tenant} region policy owned by the region module.")
                                        .replace("{tenant}", payload.current_tenant.name.as_str())
                                }).unwrap_or_else(|| t(ui_locale_for_list_heading.as_deref(), "region.list.subtitleFallback", "Tenant-scoped region policy owned by the region module."))}
                            </p>
                        </div>
                        <button
                            type="button"
                            class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                            disabled=move || busy.get()
                            on:click=move |_| set_refresh_nonce.update(|value| *value += 1)
                        >
                            {t(ui_locale.as_deref(), "region.action.refresh", "Refresh")}
                        </button>
                    </div>

                    <div class="mt-5 space-y-3">
                        {move || match regions.get() {
                            None => view! { <div class="rounded-2xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">{t(ui_locale_for_list.as_deref(), "region.loading", "Loading regions...")}</div> }.into_any(),
                            Some(Err(err)) => view! { <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{format!("{load_regions_error_label}: {err}")}</div> }.into_any(),
                            Some(Ok(list)) if list.items.is_empty() => view! { <div class="rounded-2xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">{t(ui_locale_for_list.as_deref(), "region.list.empty", "No regions have been created for this tenant yet.")}</div> }.into_any(),
                            Some(Ok(list)) => view! {
                                <>
                                    {list.items.into_iter().map(|region| {
                                        let region_id = region.id.clone();
                                        let region_marker = region.id.clone();
                                        let item_locale = ui_locale_for_list.clone();
                                        let item_query_writer = list_query_writer.clone();
                                        view! {
                                            <article class=move || if editing_id.get().as_deref() == Some(region_marker.as_str()) { "rounded-2xl border border-primary/40 bg-background p-5 shadow-sm" } else { "rounded-2xl border border-border bg-background p-5 transition hover:border-primary/40" }>
                                                <div class="flex items-start justify-between gap-3">
                                                    <div class="space-y-2">
                                                        <div class="flex flex-wrap items-center gap-2">
                                                            <h4 class="text-base font-semibold text-card-foreground">{region.name.clone()}</h4>
                                                            <span class="inline-flex rounded-full border border-border px-3 py-1 text-xs text-muted-foreground">{tax_badge(item_locale.as_deref(), &region)}</span>
                                                        </div>
                                                        <p class="text-sm text-muted-foreground">{format!("{} | {}", region.currency_code, region.countries_preview)}</p>
                                                        <p class="text-xs text-muted-foreground">{list_meta(item_locale.as_deref(), &region)}</p>
                                                    </div>
                                                    <button
                                                        type="button"
                                                        class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                                                        disabled=move || busy.get()
                                                        on:click=move |_| item_query_writer.push_value(AdminQueryKey::RegionId.as_str(), region_id.clone())
                                                    >
                                                        {t(item_locale.as_deref(), "region.action.open", "Open")}
                                                    </button>
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
                                    {move || if editing_id.get().is_some() { t(ui_locale_for_editor_heading.as_deref(), "region.editor.editTitle", "Edit Region") } else { t(ui_locale_for_editor_heading.as_deref(), "region.editor.createTitle", "Create Region") }}
                                </h3>
                                <p class="text-sm text-muted-foreground">
                                    {t(ui_locale_for_editor.as_deref(), "region.editor.subtitle", "Native region CRUD lives in the region module package. Countries are entered as comma-separated ISO codes and metadata is stored as raw JSON.")}
                                </p>
                            </div>
                            <button
                                type="button"
                                class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                                disabled=move || busy.get()
                            on:click=move |_| {
                                reset_query_writer.clear_key(AdminQueryKey::RegionId.as_str());
                                reset_form();
                            }
                            >
                                {t(ui_locale.as_deref(), "region.action.new", "New")}
                            </button>
                        </div>

                        <form class="mt-5 space-y-4" on:submit=on_submit>
                            <input class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=t(ui_locale.as_deref(), "region.field.name", "Region name") prop:value=move || name.get() on:input=move |ev| set_name.set(event_target_value(&ev)) />
                            <div class="grid gap-4 md:grid-cols-2">
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=t(ui_locale.as_deref(), "region.field.currencyCode", "Currency code") prop:value=move || currency_code.get() on:input=move |ev| set_currency_code.set(event_target_value(&ev)) />
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=t(ui_locale.as_deref(), "region.field.taxProviderId", "Tax provider ID (optional)") prop:value=move || tax_provider_id.get() on:input=move |ev| set_tax_provider_id.set(event_target_value(&ev)) />
                            </div>
                            <div class="grid gap-4 md:grid-cols-2">
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=t(ui_locale.as_deref(), "region.field.taxRate", "Tax rate") prop:value=move || tax_rate.get() on:input=move |ev| set_tax_rate.set(event_target_value(&ev)) />
                            </div>
                            <label class="flex items-center gap-3 rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground">
                                <input type="checkbox" prop:checked=move || tax_included.get() on:change=move |ev| set_tax_included.set(event_target_checked(&ev)) />
                                <span>{t(ui_locale.as_deref(), "region.field.taxIncluded", "Prices already include tax")}</span>
                            </label>
                            <textarea class="min-h-32 w-full rounded-xl border border-border bg-background px-3 py-2 font-mono text-sm text-foreground outline-none transition focus:border-primary" placeholder=t(ui_locale.as_deref(), "region.field.countryTaxPolicies", "Country tax policies JSON") prop:value=move || country_tax_policies.get() on:input=move |ev| set_country_tax_policies.set(event_target_value(&ev))>{move || country_tax_policies.get()}</textarea>
                            <input class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=t(ui_locale.as_deref(), "region.field.countries", "Countries (BY, RU, KZ)") prop:value=move || countries.get() on:input=move |ev| set_countries.set(event_target_value(&ev)) />
                            <textarea class="min-h-44 w-full rounded-xl border border-border bg-background px-3 py-2 font-mono text-sm text-foreground outline-none transition focus:border-primary" prop:value=move || metadata.get() on:input=move |ev| set_metadata.set(event_target_value(&ev))>{move || metadata.get()}</textarea>
                            <button type="submit" class="inline-flex rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || busy.get()>
                                {move || if editing_id.get().is_some() { t(ui_locale.as_deref(), "region.action.save", "Save region") } else { t(ui_locale.as_deref(), "region.action.create", "Create region") }}
                            </button>
                        </form>
                    </section>

                    {move || selected.get().map(|detail| {
                        view! {
                            <section class="space-y-6 rounded-3xl border border-border bg-card p-6 shadow-sm">
                                <div class="space-y-2">
                                    <h3 class="text-lg font-semibold text-card-foreground">{t(ui_locale_for_detail.as_deref(), "region.detail.title", "Region Detail")}</h3>
                                    <p class="text-sm text-muted-foreground">{t(ui_locale_for_detail.as_deref(), "region.detail.subtitle", "Inspect country coverage, currency baseline and tax flags from the region-owned route.")}</p>
                                </div>

                                <div class="rounded-2xl border border-border bg-background p-5">
                                    <div class="flex flex-wrap items-start justify-between gap-3">
                                        <div class="space-y-2">
                                            <h4 class="text-base font-semibold text-card-foreground">{detail.region.name.clone()}</h4>
                                            <p class="text-sm text-muted-foreground">{format!("{} | {}", detail.region.currency_code, detail.region.countries.join(", "))}</p>
                                            <p class="text-xs text-muted-foreground">{detail_meta(ui_locale_for_detail.as_deref(), &detail)}</p>
                                        </div>
                                        <div class="text-right text-xs text-muted-foreground">
                                            <p>{format!("created {}", detail.region.created_at)}</p>
                                            <p>{format!("updated {}", detail.region.updated_at)}</p>
                                        </div>
                                    </div>
                                </div>

                                <div class="grid gap-4 md:grid-cols-2">
                                    <div class="rounded-2xl border border-border bg-background p-5">
                                        <h4 class="text-sm font-semibold uppercase tracking-[0.18em] text-muted-foreground">{t(ui_locale_for_detail.as_deref(), "region.section.policy", "Policy Baseline")}</h4>
                                        <div class="mt-4 space-y-2 text-sm text-muted-foreground">
                                            <p>{format!("currency: {}", detail.region.currency_code)}</p>
                                            <p>{format!("tax provider: {}", detail.region.tax_provider_id.clone().unwrap_or_else(|| "region_default".to_string()))}</p>
                                            <p>{format!("tax rate: {}", detail.region.tax_rate)}</p>
                                            <p>{if detail.region.tax_included { t(ui_locale_for_detail.as_deref(), "region.common.taxIncluded", "tax included") } else { t(ui_locale_for_detail.as_deref(), "region.common.taxExcluded", "tax excluded") }}</p>
                                        </div>
                                    </div>
                                    <div class="rounded-2xl border border-border bg-background p-5">
                                        <h4 class="text-sm font-semibold uppercase tracking-[0.18em] text-muted-foreground">{t(ui_locale_for_detail.as_deref(), "region.section.countries", "Country Coverage")}</h4>
                                        <p class="mt-4 text-sm text-muted-foreground">{detail.region.countries.join(", ")}</p>
                                    </div>
                                </div>

                                <div class="rounded-2xl border border-border bg-background p-5">
                                    <h4 class="text-sm font-semibold uppercase tracking-[0.18em] text-muted-foreground">{t(ui_locale_for_detail.as_deref(), "region.section.countryTaxPolicies", "Country Tax Policies")}</h4>
                                    <pre class="mt-4 overflow-x-auto whitespace-pre-wrap text-xs text-muted-foreground">{detail.region.country_tax_policies_pretty.clone()}</pre>
                                </div>

                                <div class="rounded-2xl border border-border bg-background p-5">
                                    <h4 class="text-sm font-semibold uppercase tracking-[0.18em] text-muted-foreground">{t(ui_locale_for_detail.as_deref(), "region.section.metadata", "Metadata")}</h4>
                                    <pre class="mt-4 overflow-x-auto whitespace-pre-wrap text-xs text-muted-foreground">{detail.region.metadata_pretty.clone()}</pre>
                                </div>
                            </section>
                        }.into_any()
                    }).unwrap_or_else(|| view! { <section class="rounded-3xl border border-dashed border-border p-10 text-center text-sm text-muted-foreground">{t(ui_locale_for_empty.as_deref(), "region.detail.empty", "Open a region to inspect policy details, country coverage and raw metadata.")}</section> }.into_any())}
                </section>
            </div>
        </section>
    }
}

fn apply_region_detail(
    detail: &RegionDetail,
    set_editing_id: WriteSignal<Option<String>>,
    set_selected: WriteSignal<Option<RegionDetail>>,
    set_name: WriteSignal<String>,
    set_currency_code: WriteSignal<String>,
    set_tax_provider_id: WriteSignal<String>,
    set_tax_rate: WriteSignal<String>,
    set_tax_included: WriteSignal<bool>,
    set_country_tax_policies: WriteSignal<String>,
    set_countries: WriteSignal<String>,
    set_metadata: WriteSignal<String>,
) {
    set_editing_id.set(Some(detail.region.id.clone()));
    set_selected.set(Some(detail.clone()));
    set_name.set(detail.region.name.clone());
    set_currency_code.set(detail.region.currency_code.clone());
    set_tax_provider_id.set(detail.region.tax_provider_id.clone().unwrap_or_default());
    set_tax_rate.set(detail.region.tax_rate.clone());
    set_tax_included.set(detail.region.tax_included);
    set_country_tax_policies.set(detail.region.country_tax_policies_pretty.clone());
    set_countries.set(detail.region.countries.join(", "));
    set_metadata.set(detail.region.metadata_pretty.clone());
}

fn clear_region_form(
    set_editing_id: WriteSignal<Option<String>>,
    set_selected: WriteSignal<Option<RegionDetail>>,
    set_name: WriteSignal<String>,
    set_currency_code: WriteSignal<String>,
    set_tax_provider_id: WriteSignal<String>,
    set_tax_rate: WriteSignal<String>,
    set_tax_included: WriteSignal<bool>,
    set_country_tax_policies: WriteSignal<String>,
    set_countries: WriteSignal<String>,
    set_metadata: WriteSignal<String>,
) {
    set_editing_id.set(None);
    set_selected.set(None);
    set_name.set(String::new());
    set_currency_code.set(String::new());
    set_tax_provider_id.set(String::new());
    set_tax_rate.set("0".to_string());
    set_tax_included.set(false);
    set_country_tax_policies.set("[]".to_string());
    set_countries.set(String::new());
    set_metadata.set("{}".to_string());
}

fn tax_badge(locale: Option<&str>, region: &RegionListItem) -> String {
    if region.tax_included {
        t(locale, "region.common.taxIncluded", "tax included")
    } else {
        t(locale, "region.common.taxExcluded", "tax excluded")
    }
}

fn list_meta(locale: Option<&str>, region: &RegionListItem) -> String {
    format!(
        "{} {} | {} {} | updated {}",
        region.country_count,
        t(locale, "region.common.countries", "countries"),
        t(locale, "region.common.taxRate", "tax rate"),
        region.tax_rate,
        region.updated_at
    )
}

fn detail_meta(locale: Option<&str>, detail: &RegionDetail) -> String {
    let tax_state = if detail.region.tax_included {
        t(locale, "region.common.taxIncluded", "tax included")
    } else {
        t(locale, "region.common.taxExcluded", "tax excluded")
    };
    format!(
        "{} {} | {} {} ({tax_state})",
        detail.region.countries.len(),
        t(locale, "region.common.countries", "countries"),
        t(locale, "region.common.taxRate", "tax rate"),
        detail.region.tax_rate
    )
}
