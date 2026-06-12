use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
use rustok_api::{AdminQueryKey, UiRouteContext};

use crate::core::{
    build_customer_admin_submit_command, customer_admin_detail_empty_view_model,
    customer_admin_editor_view_model, customer_admin_list_state_view_model,
    customer_detail_form_snapshot, customer_detail_view_model, customer_list_item_class,
    customer_list_item_view_model, customer_list_request, empty_customer_admin_form_snapshot,
    CustomerAdminDisplayLabels, CustomerAdminDraftInput, CustomerAdminFormSnapshot,
    CustomerAdminListStateKind, CustomerAdminPageLabels, CustomerAdminSubmitCommandError,
};
use crate::i18n::t;
use crate::model::{CustomerAdminBootstrap, CustomerDetail};
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
pub fn CustomerAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let selected_customer_query = use_route_query_value(AdminQueryKey::CustomerId.as_str());
    let query_writer = use_route_query_writer();

    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (editing_id, set_editing_id) = signal(Option::<String>::None);
    let (selected, set_selected) = signal(Option::<CustomerDetail>::None);
    let (search, set_search) = signal(String::new());
    let (user_id, set_user_id) = signal(String::new());
    let (email, set_email) = signal(String::new());
    let (first_name, set_first_name) = signal(String::new());
    let (last_name, set_last_name) = signal(String::new());
    let (phone, set_phone) = signal(String::new());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);

    let bootstrap = local_resource(
        move || refresh_nonce.get(),
        move |_| async move { transport::fetch_bootstrap().await },
    );

    let customers = local_resource(
        move || (refresh_nonce.get(), search.get()),
        move |(_, search_value)| async move {
            let request = customer_list_request(search_value);
            transport::fetch_customers(request.search, request.page, request.per_page).await
        },
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
    let locale_unavailable_label = t(
        ui_locale.as_deref(),
        "customer.error.localeUnavailable",
        "Host locale is unavailable.",
    );
    let load_customers_error_label = t(
        ui_locale.as_deref(),
        "customer.error.loadCustomers",
        "Failed to load customers",
    );

    let reset_form = move || {
        apply_customer_form_snapshot(
            empty_customer_admin_form_snapshot(),
            set_editing_id,
            set_selected,
            set_user_id,
            set_email,
            set_first_name,
            set_last_name,
            set_phone,
        );
        set_error.set(None);
    };

    let open_load_customer_error_label = load_customer_error_label.clone();
    let open_customer = Callback::new(move |customer_id: String| {
        let load_customer_error_label = open_load_customer_error_label.clone();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            match transport::fetch_customer_detail(customer_id).await {
                Ok(detail) => apply_customer_form_snapshot(
                    customer_detail_form_snapshot(detail),
                    set_editing_id,
                    set_selected,
                    set_user_id,
                    set_email,
                    set_first_name,
                    set_last_name,
                    set_phone,
                ),
                Err(err) => {
                    apply_customer_form_snapshot(
                        empty_customer_admin_form_snapshot(),
                        set_editing_id,
                        set_selected,
                        set_user_id,
                        set_email,
                        set_first_name,
                        set_last_name,
                        set_phone,
                    );
                    set_error.set(Some(format!("{load_customer_error_label}: {err}")));
                }
            }
            set_busy.set(false);
        });
    });
    let initial_open_customer = open_customer;
    Effect::new(move |_| match selected_customer_query.get() {
        Some(customer_id) if !customer_id.trim().is_empty() => {
            initial_open_customer.run(customer_id);
        }
        _ => {
            apply_customer_form_snapshot(
                empty_customer_admin_form_snapshot(),
                set_editing_id,
                set_selected,
                set_user_id,
                set_email,
                set_first_name,
                set_last_name,
                set_phone,
            );
        }
    });

    let submit_ui_locale = ui_locale.clone();
    let submit_query_writer = query_writer.clone();
    let on_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        let submit_query_writer = submit_query_writer.clone();
        let command = match build_customer_admin_submit_command(
            CustomerAdminDraftInput {
                editing_customer_id: editing_id.get_untracked(),
                user_id: user_id.get_untracked(),
                email: email.get_untracked(),
                first_name: first_name.get_untracked(),
                last_name: last_name.get_untracked(),
                phone: phone.get_untracked(),
            },
            submit_ui_locale.clone(),
        ) {
            Ok(command) => command,
            Err(CustomerAdminSubmitCommandError::EmailRequired) => {
                set_error.set(Some(email_required_label.clone()));
                return;
            }
            Err(CustomerAdminSubmitCommandError::LocaleUnavailable) => {
                set_error.set(Some(locale_unavailable_label.clone()));
                return;
            }
        };
        let save_customer_error_label = save_customer_error_label.clone();
        set_busy.set(true);
        set_error.set(None);
        spawn_local(async move {
            let result = match command.customer_id {
                Some(customer_id) => transport::update_customer(customer_id, command.draft).await,
                None => transport::create_customer(command.draft).await,
            };

            match result {
                Ok(detail) => {
                    let detail_id = detail.customer.id.clone();
                    apply_customer_form_snapshot(
                        customer_detail_form_snapshot(detail),
                        set_editing_id,
                        set_selected,
                        set_user_id,
                        set_email,
                        set_first_name,
                        set_last_name,
                        set_phone,
                    );
                    set_refresh_nonce.update(|value| *value += 1);
                    submit_query_writer
                        .replace_value(AdminQueryKey::CustomerId.as_str(), detail_id);
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
    let ui_locale_for_editor = ui_locale.clone();
    let list_query_writer = query_writer.clone();
    let reset_query_writer = query_writer.clone();
    let display_labels = customer_admin_display_labels(ui_locale.as_deref());
    let list_display_labels = display_labels.clone();
    let detail_display_labels = display_labels;
    let page_labels = customer_admin_page_labels(ui_locale.as_deref());
    let list_state_labels = page_labels.clone();
    let editor_title_labels = page_labels.clone();
    let user_id_disabled_labels = page_labels.clone();
    let submit_disabled_labels = page_labels.clone();
    let submit_label_labels = page_labels.clone();
    let new_button_labels = page_labels.clone();
    let detail_empty_labels = page_labels;

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
                            None => {
                                let state = customer_admin_list_state_view_model(
                                    CustomerAdminListStateKind::Loading,
                                    &list_state_labels,
                                    None,
                                );
                                view! { <div class=state.class>{state.message}</div> }.into_any()
                            },
                            Some(Err(err)) => {
                                let message = format!("{load_customers_error_label}: {err}");
                                let state = customer_admin_list_state_view_model(
                                    CustomerAdminListStateKind::Error,
                                    &list_state_labels,
                                    Some(message.as_str()),
                                );
                                view! { <div class=state.class>{state.message}</div> }.into_any()
                            },
                            Some(Ok(list)) if list.items.is_empty() => {
                                let state = customer_admin_list_state_view_model(
                                    CustomerAdminListStateKind::Empty,
                                    &list_state_labels,
                                    None,
                                );
                                view! { <div class=state.class>{state.message}</div> }.into_any()
                            },
                            Some(Ok(list)) => view! {
                                <>
                                    {list.items.into_iter().map(|customer| {
                                        let row = customer_list_item_view_model(&customer, &list_display_labels);
                                        let customer_id = row.id.clone();
                                        let customer_marker = row.id.clone();
                                        let item_locale = ui_locale_for_list.clone();
                                        let item_query_writer = list_query_writer.clone();
                                        view! {
                                            <article class=move || customer_list_item_class(editing_id.get().as_deref() == Some(customer_marker.as_str()))>
                                                <div class="flex items-start justify-between gap-3">
                                                    <div class="space-y-2">
                                                        <div class="flex flex-wrap items-center gap-2">
                                                            <h4 class="text-base font-semibold text-card-foreground">{row.full_name.clone()}</h4>
                                                            <span class="inline-flex rounded-full border border-border px-3 py-1 text-xs text-muted-foreground">{row.linked_badge.clone()}</span>
                                                        </div>
                                                        <p class="text-sm text-muted-foreground">{row.email.clone()}</p>
                                                        <p class="text-xs text-muted-foreground">{row.meta.clone()}</p>
                                                    </div>
                                                    <button
                                                        type="button"
                                                        class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                                                        disabled=move || busy.get()
                                                        on:click=move |_| item_query_writer.push_value(AdminQueryKey::CustomerId.as_str(), customer_id.clone())
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
                                    {move || customer_admin_editor_view_model(
                                        editing_id.get().is_some(),
                                        busy.get(),
                                        &editor_title_labels,
                                    ).title}
                                </h3>
                                <p class="text-sm text-muted-foreground">
                                    {t(ui_locale_for_editor.as_deref(), "customer.editor.subtitle", "Native customer CRUD lives in the customer module package. User linkage is optional and can be set only during customer creation.")}
                                </p>
                            </div>
                            <button
                                type="button"
                                class="inline-flex rounded-lg border border-border px-3 py-2 text-sm font-medium text-foreground transition hover:bg-accent disabled:opacity-50"
                                disabled=move || customer_admin_editor_view_model(
                                    editing_id.get().is_some(),
                                    busy.get(),
                                    &new_button_labels,
                                ).new_disabled
                                on:click=move |_| {
                                    reset_query_writer.clear_key(AdminQueryKey::CustomerId.as_str());
                                    reset_form();
                                }
                            >
                                {t(ui_locale.as_deref(), "customer.action.new", "New")}
                            </button>
                        </div>

                        <form class="mt-5 space-y-4" on:submit=on_submit>
                            <input class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary disabled:opacity-60" placeholder=t(ui_locale.as_deref(), "customer.field.userId", "Linked user ID (optional)") disabled=move || customer_admin_editor_view_model(
                                    editing_id.get().is_some(),
                                    busy.get(),
                                    &user_id_disabled_labels,
                                ).user_id_disabled prop:value=move || user_id.get() on:input=move |ev| set_user_id.set(event_target_value(&ev)) />
                            <input class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=t(ui_locale.as_deref(), "customer.field.email", "Email") prop:value=move || email.get() on:input=move |ev| set_email.set(event_target_value(&ev)) />
                            <div class="grid gap-4 md:grid-cols-2">
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=t(ui_locale.as_deref(), "customer.field.firstName", "First name") prop:value=move || first_name.get() on:input=move |ev| set_first_name.set(event_target_value(&ev)) />
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=t(ui_locale.as_deref(), "customer.field.lastName", "Last name") prop:value=move || last_name.get() on:input=move |ev| set_last_name.set(event_target_value(&ev)) />
                            </div>
                            <div class="grid gap-4 md:grid-cols-2">
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=t(ui_locale.as_deref(), "customer.field.phone", "Phone") prop:value=move || phone.get() on:input=move |ev| set_phone.set(event_target_value(&ev)) />
                            </div>
                            <button type="submit" class="inline-flex rounded-xl bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || customer_admin_editor_view_model(
                                    editing_id.get().is_some(),
                                    busy.get(),
                                    &submit_disabled_labels,
                                ).submit_disabled>
                                {move || customer_admin_editor_view_model(
                                    editing_id.get().is_some(),
                                    busy.get(),
                                    &submit_label_labels,
                                ).submit_label}
                            </button>
                        </form>
                    </section>

                    {move || selected.get().map(|detail| {
                        let detail_view = customer_detail_view_model(&detail, &detail_display_labels);
                        view! {
                            <section class="space-y-6 rounded-3xl border border-border bg-card p-6 shadow-sm">
                                <div class="space-y-2">
                                    <h3 class="text-lg font-semibold text-card-foreground">{t(ui_locale_for_detail.as_deref(), "customer.detail.title", "Customer Detail")}</h3>
                                    <p class="text-sm text-muted-foreground">{t(ui_locale_for_detail.as_deref(), "customer.detail.subtitle", "Inspect customer identity, optional user linkage and profile bridge state from the customer-owned route.")}</p>
                                </div>

                                <div class="rounded-2xl border border-border bg-background p-5">
                                    <div class="flex flex-wrap items-start justify-between gap-3">
                                        <div class="space-y-2">
                                            <h4 class="text-base font-semibold text-card-foreground">{detail_view.full_name.clone()}</h4>
                                            <p class="text-sm text-muted-foreground">{detail_view.email.clone()}</p>
                                            <p class="text-xs text-muted-foreground">{detail_view.meta.clone()}</p>
                                        </div>
                                        <div class="text-right text-xs text-muted-foreground">
                                            <p>{format!("created {}", detail_view.created_at)}</p>
                                            <p>{format!("updated {}", detail_view.updated_at)}</p>
                                        </div>
                                    </div>
                                </div>

                                <div class="grid gap-4 md:grid-cols-2">
                                    <div class="rounded-2xl border border-border bg-background p-5">
                                        <h4 class="text-sm font-semibold uppercase tracking-[0.18em] text-muted-foreground">{t(ui_locale_for_detail.as_deref(), "customer.section.customer", "Customer Record")}</h4>
                                        <div class="mt-4 space-y-2 text-sm text-muted-foreground">
                                            <p>{detail_view.user_link.clone()}</p>
                                            <p>{detail_view.phone.clone()}</p>
                                            <p>{detail_view.locale.clone()}</p>
                                        </div>
                                    </div>
                                    <div class="rounded-2xl border border-border bg-background p-5">
                                        <h4 class="text-sm font-semibold uppercase tracking-[0.18em] text-muted-foreground">{t(ui_locale_for_profile.as_deref(), "customer.section.profile", "Profile Bridge")}</h4>
                                        {detail_view.profile.clone().map(|profile| view! {
                                            <div class="mt-4 space-y-2 text-sm text-muted-foreground">
                                                <p>{profile.identity}</p>
                                                <p>{profile.visibility}</p>
                                                <p>{profile.preferred_locale}</p>
                                                <p>{profile.tags}</p>
                                            </div>
                                        }.into_any()).unwrap_or_else(|| view! {
                                            <p class="mt-4 text-sm text-muted-foreground">{t(ui_locale_for_profile.as_deref(), "customer.profile.empty", "No public profile is linked to this customer yet.")}</p>
                                        }.into_any())}
                                    </div>
                                </div>

                                <div class="rounded-2xl border border-border bg-background p-5">
                                    <h4 class="text-sm font-semibold uppercase tracking-[0.18em] text-muted-foreground">{t(ui_locale_for_detail.as_deref(), "customer.section.metadata", "Metadata")}</h4>
                                    <pre class="mt-4 overflow-x-auto whitespace-pre-wrap text-xs text-muted-foreground">{detail_view.metadata_pretty.clone()}</pre>
                                </div>
                            </section>
                        }.into_any()
                    }).unwrap_or_else(|| {
                        let empty = customer_admin_detail_empty_view_model(&detail_empty_labels);
                        view! { <section class=empty.class>{empty.message}</section> }.into_any()
                    })}
                </section>
            </div>
        </section>
    }
}

fn customer_admin_page_labels(locale: Option<&str>) -> CustomerAdminPageLabels {
    CustomerAdminPageLabels {
        list_loading: t(locale, "customer.loading", "Loading customers..."),
        list_empty: t(
            locale,
            "customer.list.empty",
            "No customers match the current filters.",
        ),
        detail_empty: t(
            locale,
            "customer.detail.empty",
            "Open a customer to inspect the record, linked user and profile bridge state.",
        ),
        edit_title: t(locale, "customer.editor.editTitle", "Edit Customer"),
        create_title: t(locale, "customer.editor.createTitle", "Create Customer"),
        save_action: t(locale, "customer.action.save", "Save customer"),
        create_action: t(locale, "customer.action.create", "Create customer"),
    }
}

fn customer_admin_display_labels(locale: Option<&str>) -> CustomerAdminDisplayLabels {
    CustomerAdminDisplayLabels {
        linked_user: t(locale, "customer.common.linked", "linked user"),
        standalone_customer: t(locale, "customer.common.unlinked", "standalone customer"),
        not_linked_to_platform_user: t(
            locale,
            "customer.common.unlinked",
            "not linked to a platform user",
        ),
        no_phone: t(locale, "customer.common.noPhone", "no phone"),
        no_locale: t(locale, "customer.common.noLocale", "no locale"),
        no_tags: t(locale, "customer.profile.noTags", "no tags"),
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_customer_form_snapshot(
    snapshot: CustomerAdminFormSnapshot,
    set_editing_id: WriteSignal<Option<String>>,
    set_selected: WriteSignal<Option<CustomerDetail>>,
    set_user_id: WriteSignal<String>,
    set_email: WriteSignal<String>,
    set_first_name: WriteSignal<String>,
    set_last_name: WriteSignal<String>,
    set_phone: WriteSignal<String>,
) {
    set_editing_id.set(snapshot.editing_customer_id);
    set_selected.set(snapshot.selected_detail);
    set_user_id.set(snapshot.user_id);
    set_email.set(snapshot.email);
    set_first_name.set(snapshot.first_name);
    set_last_name.set(snapshot.last_name);
    set_phone.set(snapshot.phone);
}
