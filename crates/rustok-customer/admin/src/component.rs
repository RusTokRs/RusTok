use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use rustok_api::UiRouteContext;

use crate::i18n::t;
use crate::model::{CustomerAdminBootstrap, CustomerDetail, CustomerDraft, CustomerListItem};

#[component]
pub fn CustomerAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let initial_locale = ui_locale.clone().unwrap_or_else(|| "en".to_string());

    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (editing_id, set_editing_id) = signal(Option::<String>::None);
    let (selected, set_selected) = signal(Option::<CustomerDetail>::None);
    let (search, set_search) = signal(String::new());
    let (user_id, set_user_id) = signal(String::new());
    let (email, set_email) = signal(String::new());
    let (first_name, set_first_name) = signal(String::new());
    let (last_name, set_last_name) = signal(String::new());
    let (phone, set_phone) = signal(String::new());
    let (locale, set_locale) = signal(initial_locale.clone());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);

    let bootstrap = Resource::new(
        move || refresh_nonce.get(),
        move |_| async move { crate::api::fetch_bootstrap().await },
    );

    let customers = Resource::new(
        move || (refresh_nonce.get(), search.get()),
        move |(_, search_value)| async move { crate::api::fetch_customers(search_value, 1, 24).await },
    );

    let email_required_label = t(
        ui_locale.as_deref(),
        "customer.error.emailRequired",
        "Email is required.",
    );
    let load_customer_error_label = t(
        ui_locale.as_deref(),
        "customer.error.loadCustomer",
        "Failed to load customer",
    );
    let save_customer_error_label = t(
        ui_locale.as_deref(),
        "customer.error.saveCustomer",
        "Failed to save customer",
    );
    let load_customers_error_label = t(
        ui_locale.as_deref(),
        "customer.error.loadCustomers",
        "Failed to load customers",
    );

    let reset_form_initial_locale = initial_locale.clone();
    let reset_form = move || {
        set_editing_id.set(None);
        set_selected.set(None);
        set_user_id.set(String::new());
        set_email.set(String::new());
        set_first_name.set(String::new());
        set_last_name.set(String::new());
        set_phone.set(String::new());
        set_locale.set(reset_form_initial_locale.clone());
        set_error.set(None);
    };

    let open_load_customer_error_label = load_customer_error_label.clone();
    let open_customer = Callback::new(move |customer_id: String| {
        let load_customer_error_label = open_load_customer_error_label.clone();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            match crate::api::fetch_customer_detail(customer_id).await {
                Ok(detail) => apply_customer_detail(
                    &detail,
                    set_editing_id,
                    set_selected,
                    set_user_id,
                    set_email,
                    set_first_name,
                    set_last_name,
                    set_phone,
                    set_locale,
                ),
                Err(err) => set_error.set(Some(format!("{load_customer_error_label}: {err}"))),
            }
            set_busy.set(false);
        });
    });

    let on_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        if email.get_untracked().trim().is_empty() {
            set_error.set(Some(email_required_label.clone()));
            return;
        }

        let payload = CustomerDraft {
            user_id: user_id.get_untracked(),
            email: email.get_untracked(),
            first_name: first_name.get_untracked(),
            last_name: last_name.get_untracked(),
            phone: phone.get_untracked(),
            locale: locale.get_untracked(),
        };
        let editing_customer_id = editing_id.get_untracked();
        let save_customer_error_label = save_customer_error_label.clone();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            let result = match editing_customer_id {
                Some(customer_id) => crate::api::update_customer(customer_id, payload).await,
                None => crate::api::create_customer(payload).await,
            };

            match result {
                Ok(detail) => {
                    apply_customer_detail(
                        &detail,
                        set_editing_id,
                        set_selected,
                        set_user_id,
                        set_email,
                        set_first_name,
                        set_last_name,
                        set_phone,
                        set_locale,
                    );
                    set_refresh_nonce.update(|value| *value += 1);
                }
                Err(err) => set_error.set(Some(format!("{save_customer_error_label}: {err}"))),
            }

            set_busy.set(false);
        });
    };

    let ui_locale_for_list_heading = ui_locale.clone();
    let ui_locale_for_list = ui_locale.clone();
    let ui_locale_for_detail = ui_locale.clone();
    let ui_locale_for_profile = ui_locale.clone();
    let ui_locale_for_empty = ui_locale.clone();
    let ui_locale_for_editor_heading = ui_locale.clone();
    let ui_locale_for_editor = ui_locale.clone();

    view! {
        <section class="space-y-6">
            <header class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-3">
                    <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                        {t(ui_locale.as_deref(), "customer.badge", "customer")}
                    </span>
                    <h2 class="text-2xl font-semibold text-card-foreground">
                        {t(ui_locale.as_deref(), "customer.title", "Customer Operations")}
                    </h2>
                    <p class="max-w-3xl text-sm text-muted-foreground">
                        {t(ui_locale.as_deref(), "customer.subtitle", "Module-owned customer workspace for tenant-scoped customer records, optional user linkage and profile bridge visibility without routing admin traffic back through the commerce umbrella.")}
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
                                {t(ui_locale.as_deref(), "customer.list.title", "Customers")}
                            </h3>
                            <p class="text-sm text-muted-foreground">
                                {move || bootstrap.get().and_then(Result::ok).map(|payload: CustomerAdminBootstrap| {
                                    t(ui_locale_for_list_heading.as_deref(), "customer.list.subtitle", "Tenant {tenant} customer records owned by the customer module.")
                                        .replace("{tenant}", payload.current_tenant.name.as_str())
                                }).unwrap_or_else(|| t(ui_locale_for_list_heading.as_deref(), "customer.list.subtitleFallback", "Tenant-scoped customer records owned by the customer module."))}
                            </p>
                        </div>
                        <div class="flex flex-wrap items-center gap-3">
                            <input
                                class="min-w-64 rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary"
                                placeholder=t(ui_locale.as_deref(), "customer.list.search", "Search email, name or phone")
                                prop:value=move || search.get()
                                on:input=move |ev| set_search.set(event_target_value(&ev))
                            />
                            <button
                                type="button"
                                class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                                disabled=move || busy.get()
                                on:click=move |_| set_refresh_nonce.update(|value| *value += 1)
                            >
                                {t(ui_locale.as_deref(), "customer.action.refresh", "Refresh")}
                            </button>
                        </div>
                    </div>

                    <div class="mt-5 space-y-3">
                        {move || match customers.get() {
                            None => view! { <div class="rounded-2xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">{t(ui_locale_for_list.as_deref(), "customer.loading", "Loading customers...")}</div> }.into_any(),
                            Some(Err(err)) => view! { <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{format!("{load_customers_error_label}: {err}")}</div> }.into_any(),
                            Some(Ok(list)) if list.items.is_empty() => view! { <div class="rounded-2xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">{t(ui_locale_for_list.as_deref(), "customer.list.empty", "No customers match the current filters.")}</div> }.into_any(),
                            Some(Ok(list)) => view! {
                                <>
                                    {list.items.into_iter().map(|customer| {
                                        let customer_id = customer.id.clone();
                                        let customer_marker = customer.id.clone();
                                        let item_locale = ui_locale_for_list.clone();
                                        view! {
                                            <article class=move || if editing_id.get() == Some(customer_marker.clone()) { "rounded-2xl border border-primary/40 bg-background p-5 shadow-sm" } else { "rounded-2xl border border-border bg-background p-5 transition hover:border-primary/40" }>
                                                <div class="flex items-start justify-between gap-3">
                                                    <div class="space-y-2">
                                                        <div class="flex flex-wrap items-center gap-2">
                                                            <h4 class="text-base font-semibold text-card-foreground">{customer.full_name.clone()}</h4>
                                                            <span class="inline-flex rounded-full border border-border px-3 py-1 text-xs text-muted-foreground">{linked_badge(item_locale.as_deref(), &customer)}</span>
                                                        </div>
                                                        <p class="text-sm text-muted-foreground">{customer.email.clone()}</p>
                                                        <p class="text-xs text-muted-foreground">{list_meta(item_locale.as_deref(), &customer)}</p>
                                                    </div>
                                                    <button
                                                        type="button"
                                                        class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                                                        disabled=move || busy.get()
                                                        on:click=move |_| open_customer.run(customer_id.clone())
                                                    >
                                                        {t(item_locale.as_deref(), "customer.action.open", "Open")}
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
                                    {move || if editing_id.get().is_some() { t(ui_locale_for_editor_heading.as_deref(), "customer.editor.editTitle", "Edit Customer") } else { t(ui_locale_for_editor_heading.as_deref(), "customer.editor.createTitle", "Create Customer") }}
                                </h3>
                                <p class="text-sm text-muted-foreground">
                                    {t(ui_locale_for_editor.as_deref(), "customer.editor.subtitle", "Native customer CRUD lives in the customer module package. User linkage is optional and can be set only during customer creation.")}
                                </p>
                            </div>
                            <button
                                type="button"
                                class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                                disabled=move || busy.get()
                                on:click=move |_| reset_form()
                            >
                                {t(ui_locale.as_deref(), "customer.action.new", "New")}
                            </button>
                        </div>

                        <form class="mt-5 space-y-4" on:submit=on_submit>
                            <input class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary disabled:opacity-60" placeholder=t(ui_locale.as_deref(), "customer.field.userId", "Linked user ID (optional)") disabled=move || editing_id.get().is_some() || busy.get() prop:value=move || user_id.get() on:input=move |ev| set_user_id.set(event_target_value(&ev)) />
                            <input class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=t(ui_locale.as_deref(), "customer.field.email", "Email") prop:value=move || email.get() on:input=move |ev| set_email.set(event_target_value(&ev)) />
                            <div class="grid gap-4 md:grid-cols-2">
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=t(ui_locale.as_deref(), "customer.field.firstName", "First name") prop:value=move || first_name.get() on:input=move |ev| set_first_name.set(event_target_value(&ev)) />
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=t(ui_locale.as_deref(), "customer.field.lastName", "Last name") prop:value=move || last_name.get() on:input=move |ev| set_last_name.set(event_target_value(&ev)) />
                            </div>
                            <div class="grid gap-4 md:grid-cols-2">
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=t(ui_locale.as_deref(), "customer.field.phone", "Phone") prop:value=move || phone.get() on:input=move |ev| set_phone.set(event_target_value(&ev)) />
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=t(ui_locale.as_deref(), "customer.field.locale", "Locale") prop:value=move || locale.get() on:input=move |ev| set_locale.set(event_target_value(&ev)) />
                            </div>
                            <button type="submit" class="inline-flex rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || busy.get()>
                                {move || if editing_id.get().is_some() { t(ui_locale.as_deref(), "customer.action.save", "Save customer") } else { t(ui_locale.as_deref(), "customer.action.create", "Create customer") }}
                            </button>
                        </form>
                    </section>

                    {move || selected.get().map(|detail| {
                        view! {
                            <section class="space-y-6 rounded-3xl border border-border bg-card p-6 shadow-sm">
                                <div class="space-y-2">
                                    <h3 class="text-lg font-semibold text-card-foreground">{t(ui_locale_for_detail.as_deref(), "customer.detail.title", "Customer Detail")}</h3>
                                    <p class="text-sm text-muted-foreground">{t(ui_locale_for_detail.as_deref(), "customer.detail.subtitle", "Inspect customer identity, optional user linkage and profile bridge state from the customer-owned route.")}</p>
                                </div>

                                <div class="rounded-2xl border border-border bg-background p-5">
                                    <div class="flex flex-wrap items-start justify-between gap-3">
                                        <div class="space-y-2">
                                            <h4 class="text-base font-semibold text-card-foreground">{detail.customer.full_name.clone()}</h4>
                                            <p class="text-sm text-muted-foreground">{detail.customer.email.clone()}</p>
                                            <p class="text-xs text-muted-foreground">{detail_meta(ui_locale_for_detail.as_deref(), &detail)}</p>
                                        </div>
                                        <div class="text-right text-xs text-muted-foreground">
                                            <p>{format!("created {}", detail.customer.created_at)}</p>
                                            <p>{format!("updated {}", detail.customer.updated_at)}</p>
                                        </div>
                                    </div>
                                </div>

                                <div class="grid gap-4 md:grid-cols-2">
                                    <div class="rounded-2xl border border-border bg-background p-5">
                                        <h4 class="text-sm font-semibold uppercase tracking-[0.18em] text-muted-foreground">{t(ui_locale_for_detail.as_deref(), "customer.section.customer", "Customer Record")}</h4>
                                        <div class="mt-4 space-y-2 text-sm text-muted-foreground">
                                            <p>{detail.customer.user_id.clone().unwrap_or_else(|| t(ui_locale_for_detail.as_deref(), "customer.common.unlinked", "not linked to a platform user"))}</p>
                                            <p>{detail.customer.phone.clone().unwrap_or_else(|| t(ui_locale_for_detail.as_deref(), "customer.common.noPhone", "no phone"))}</p>
                                            <p>{detail.customer.locale.clone().unwrap_or_else(|| t(ui_locale_for_detail.as_deref(), "customer.common.noLocale", "no locale"))}</p>
                                        </div>
                                    </div>
                                    <div class="rounded-2xl border border-border bg-background p-5">
                                        <h4 class="text-sm font-semibold uppercase tracking-[0.18em] text-muted-foreground">{t(ui_locale_for_profile.as_deref(), "customer.section.profile", "Profile Bridge")}</h4>
                                        {detail.profile.clone().map(|profile| view! {
                                            <div class="mt-4 space-y-2 text-sm text-muted-foreground">
                                                <p>{format!("{} @{}", profile.display_name, profile.handle)}</p>
                                                <p>{format!("visibility: {}", profile.visibility)}</p>
                                                <p>{profile.preferred_locale.unwrap_or_else(|| t(ui_locale_for_profile.as_deref(), "customer.common.noLocale", "no locale"))}</p>
                                                <p>{if profile.tags.is_empty() { t(ui_locale_for_profile.as_deref(), "customer.profile.noTags", "no tags") } else { profile.tags.join(", ") }}</p>
                                            </div>
                                        }.into_any()).unwrap_or_else(|| view! {
                                            <p class="mt-4 text-sm text-muted-foreground">{t(ui_locale_for_profile.as_deref(), "customer.profile.empty", "No public profile is linked to this customer yet.")}</p>
                                        }.into_any())}
                                    </div>
                                </div>

                                <div class="rounded-2xl border border-border bg-background p-5">
                                    <h4 class="text-sm font-semibold uppercase tracking-[0.18em] text-muted-foreground">{t(ui_locale_for_detail.as_deref(), "customer.section.metadata", "Metadata")}</h4>
                                    <pre class="mt-4 overflow-x-auto whitespace-pre-wrap text-xs text-muted-foreground">{detail.customer.metadata_pretty.clone()}</pre>
                                </div>
                            </section>
                        }.into_any()
                    }).unwrap_or_else(|| view! { <section class="rounded-3xl border border-dashed border-border p-10 text-center text-sm text-muted-foreground">{t(ui_locale_for_empty.as_deref(), "customer.detail.empty", "Open a customer to inspect the record, linked user and profile bridge state.")}</section> }.into_any())}
                </section>
            </div>
        </section>
    }
}

fn apply_customer_detail(
    detail: &CustomerDetail,
    set_editing_id: WriteSignal<Option<String>>,
    set_selected: WriteSignal<Option<CustomerDetail>>,
    set_user_id: WriteSignal<String>,
    set_email: WriteSignal<String>,
    set_first_name: WriteSignal<String>,
    set_last_name: WriteSignal<String>,
    set_phone: WriteSignal<String>,
    set_locale: WriteSignal<String>,
) {
    set_editing_id.set(Some(detail.customer.id.clone()));
    set_selected.set(Some(detail.clone()));
    set_user_id.set(detail.customer.user_id.clone().unwrap_or_default());
    set_email.set(detail.customer.email.clone());
    set_first_name.set(detail.customer.first_name.clone().unwrap_or_default());
    set_last_name.set(detail.customer.last_name.clone().unwrap_or_default());
    set_phone.set(detail.customer.phone.clone().unwrap_or_default());
    set_locale.set(
        detail
            .customer
            .locale
            .clone()
            .unwrap_or_else(|| "en".to_string()),
    );
}

fn linked_badge(locale: Option<&str>, customer: &CustomerListItem) -> String {
    if customer.user_id.is_some() {
        t(locale, "customer.common.linked", "linked user")
    } else {
        t(locale, "customer.common.unlinked", "standalone customer")
    }
}

fn list_meta(locale: Option<&str>, customer: &CustomerListItem) -> String {
    let phone = customer
        .phone
        .clone()
        .unwrap_or_else(|| t(locale, "customer.common.noPhone", "no phone"));
    let locale_value = customer
        .locale
        .clone()
        .unwrap_or_else(|| t(locale, "customer.common.noLocale", "no locale"));
    format!(
        "phone: {phone} | locale: {locale_value} | updated {}",
        customer.updated_at
    )
}

fn detail_meta(locale: Option<&str>, detail: &CustomerDetail) -> String {
    let linked = detail
        .customer
        .user_id
        .clone()
        .unwrap_or_else(|| t(locale, "customer.common.unlinked", "standalone customer"));
    let locale_value = detail
        .customer
        .locale
        .clone()
        .unwrap_or_else(|| t(locale, "customer.common.noLocale", "no locale"));
    format!("user: {linked} | locale: {locale_value}")
}
