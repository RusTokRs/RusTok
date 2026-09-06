use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
use rustok_ui_core::{AdminQueryKey, UiRouteContext};

use crate::core::{
    CustomerAdminDisplayLabels, CustomerAdminDraftInput, CustomerAdminErrorLabels,
    CustomerAdminFormSnapshot, CustomerAdminListStateKind, CustomerAdminPageLabels,
    CustomerAdminSubmitCommandError, build_customer_admin_submit_command,
    customer_admin_detail_empty_view_model, customer_admin_detail_header_view_model,
    customer_admin_detail_section_labels, customer_admin_editor_view_model,
    customer_admin_field_labels, customer_admin_list_header_view_model,
    customer_admin_list_state_view_model, customer_admin_open_action_view_model,
    customer_admin_refresh_action_view_model, customer_admin_shell_view_model,
    customer_admin_submit_error_message, customer_admin_transport_error_message,
    customer_detail_form_snapshot, customer_detail_view_model, customer_list_item_class,
    customer_list_item_view_model, customer_list_request, empty_customer_admin_form_snapshot,
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

    let error_labels = customer_admin_error_labels(ui_locale.as_deref());

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

    let open_error_labels = error_labels.clone();
    let open_customer = Callback::new(move |customer_id: String| {
        let error_labels = open_error_labels.clone();
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
                    set_error.set(Some(customer_admin_transport_error_message(
                        &error_labels.load_customer,
                        err.to_string().as_str(),
                    )));
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
    let submit_validation_error_labels = error_labels.clone();
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
                set_error.set(Some(customer_admin_submit_error_message(
                    CustomerAdminSubmitCommandError::EmailRequired,
                    &submit_validation_error_labels,
                )));
                return;
            }
            Err(CustomerAdminSubmitCommandError::LocaleUnavailable) => {
                set_error.set(Some(customer_admin_submit_error_message(
                    CustomerAdminSubmitCommandError::LocaleUnavailable,
                    &submit_validation_error_labels,
                )));
                return;
            }
        };
        let submit_error_labels = submit_validation_error_labels.clone();
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
                Err(err) => set_error.set(Some(customer_admin_transport_error_message(
                    &submit_error_labels.save_customer,
                    err.to_string().as_str(),
                ))),
            }

            set_busy.set(false);
        });
    };

    let list_error_labels = error_labels.clone();
    let list_query_writer = query_writer.clone();
    let reset_query_writer = query_writer.clone();
    let display_labels = customer_admin_display_labels(ui_locale.as_deref());
    let list_display_labels = display_labels.clone();
    let detail_display_labels = display_labels;
    let page_labels = customer_admin_page_labels(ui_locale.as_deref());
    let shell_view = customer_admin_shell_view_model(&page_labels);
    let list_header_title_labels = page_labels.clone();
    let list_header_subtitle_labels = page_labels.clone();
    let list_state_labels = page_labels.clone();
    let refresh_disabled_labels = page_labels.clone();
    let refresh_label_labels = page_labels.clone();
    let open_action_labels = page_labels.clone();
    let editor_title_labels = page_labels.clone();
    let editor_subtitle_labels = page_labels.clone();
    let user_id_disabled_labels = page_labels.clone();
    let submit_disabled_labels = page_labels.clone();
    let submit_label_labels = page_labels.clone();
    let new_button_labels = page_labels.clone();
    let new_label_labels = page_labels.clone();
    let detail_header_labels = page_labels.clone();
    let detail_section_labels = page_labels.clone();
    let detail_empty_labels = page_labels.clone();
    let field_labels = customer_admin_field_labels(&page_labels);

    view! {
        <section class="space-y-6">
            <header class="rounded-3xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-3">
                    <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                        {shell_view.badge.clone()}
                    </span>
                    <h2 class="text-2xl font-semibold text-card-foreground">
                        {shell_view.title.clone()}
                    </h2>
                    <p class="max-w-3xl text-sm text-muted-foreground">
                        {shell_view.subtitle.clone()}
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
                                {customer_admin_list_header_view_model(
                                    None,
                                    &list_header_title_labels,
                                ).title}
                            </h3>
                            <p class="text-sm text-muted-foreground">
                                {move || customer_admin_list_header_view_model(
                                    bootstrap
                                        .get()
                                        .and_then(Result::ok)
                                        .map(|payload: CustomerAdminBootstrap| payload.current_tenant.name)
                                        .as_deref(),
                                    &list_header_subtitle_labels,
                                ).subtitle}
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
                                disabled=move || customer_admin_refresh_action_view_model(
                                    busy.get(),
                                    &refresh_disabled_labels,
                                ).disabled
                                on:click=move |_| set_refresh_nonce.update(|value| *value += 1)
                            >
                                {move || customer_admin_refresh_action_view_model(
                                    busy.get(),
                                    &refresh_label_labels,
                                ).label}
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
                                let message = customer_admin_transport_error_message(
                                    &list_error_labels.load_customers,
                                    err.to_string().as_str(),
                                );
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
                                        let item_action_disabled_labels = open_action_labels.clone();
                                        let item_action_label_labels = open_action_labels.clone();
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
                                                        disabled=move || customer_admin_open_action_view_model(
                                                            busy.get(),
                                                            &item_action_disabled_labels,
                                                        ).disabled
                                                        on:click=move |_| item_query_writer.push_value(AdminQueryKey::CustomerId.as_str(), customer_id.clone())
                                                    >
                                                        {move || customer_admin_open_action_view_model(
                                                            busy.get(),
                                                            &item_action_label_labels,
                                                        ).label}
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
                                    {move || customer_admin_editor_view_model(
                                        editing_id.get().is_some(),
                                        busy.get(),
                                        &editor_subtitle_labels,
                                    ).subtitle}
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
                                {move || customer_admin_editor_view_model(
                                    editing_id.get().is_some(),
                                    busy.get(),
                                    &new_label_labels,
                                ).new_label}
                            </button>
                        </div>

                        <form class="mt-5 space-y-4" on:submit=on_submit>
                            <input class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary disabled:opacity-60" placeholder=field_labels.user_id.clone() disabled=move || customer_admin_editor_view_model(
                                    editing_id.get().is_some(),
                                    busy.get(),
                                    &user_id_disabled_labels,
                                ).user_id_disabled prop:value=move || user_id.get() on:input=move |ev| set_user_id.set(event_target_value(&ev)) />
                            <input class="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=field_labels.email.clone() prop:value=move || email.get() on:input=move |ev| set_email.set(event_target_value(&ev)) />
                            <div class="grid gap-4 md:grid-cols-2">
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=field_labels.first_name.clone() prop:value=move || first_name.get() on:input=move |ev| set_first_name.set(event_target_value(&ev)) />
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=field_labels.last_name.clone() prop:value=move || last_name.get() on:input=move |ev| set_last_name.set(event_target_value(&ev)) />
                            </div>
                            <div class="grid gap-4 md:grid-cols-2">
                                <input class="rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground outline-none transition focus:border-primary" placeholder=field_labels.phone.clone() prop:value=move || phone.get() on:input=move |ev| set_phone.set(event_target_value(&ev)) />
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
                        let detail_header = customer_admin_detail_header_view_model(&detail_header_labels);
                        let detail_sections = customer_admin_detail_section_labels(&detail_section_labels);
                        view! {
                            <section class="space-y-6 rounded-3xl border border-border bg-card p-6 shadow-sm">
                                <div class="space-y-2">
                                    <h3 class="text-lg font-semibold text-card-foreground">{detail_header.title.clone()}</h3>
                                    <p class="text-sm text-muted-foreground">{detail_header.subtitle.clone()}</p>
                                </div>

                                <div class="rounded-2xl border border-border bg-background p-5">
                                    <div class="flex flex-wrap items-start justify-between gap-3">
                                        <div class="space-y-2">
                                            <h4 class="text-base font-semibold text-card-foreground">{detail_view.full_name.clone()}</h4>
                                            <p class="text-sm text-muted-foreground">{detail_view.email.clone()}</p>
                                            <p class="text-xs text-muted-foreground">{detail_view.meta.clone()}</p>
                                        </div>
                                        <div class="text-right text-xs text-muted-foreground">
                                            <p>{detail_view.created_at_label.clone()}</p>
                                            <p>{detail_view.updated_at_label.clone()}</p>
                                        </div>
                                    </div>
                                </div>

                                <div class="grid gap-4 md:grid-cols-2">
                                    <div class="rounded-2xl border border-border bg-background p-5">
                                        <h4 class="text-sm font-semibold uppercase tracking-[0.18em] text-muted-foreground">{detail_sections.customer.clone()}</h4>
                                        <div class="mt-4 space-y-2 text-sm text-muted-foreground">
                                            <p>{detail_view.user_link.clone()}</p>
                                            <p>{detail_view.phone.clone()}</p>
                                            <p>{detail_view.locale.clone()}</p>
                                        </div>
                                    </div>
                                    <div class="rounded-2xl border border-border bg-background p-5">
                                        <h4 class="text-sm font-semibold uppercase tracking-[0.18em] text-muted-foreground">{detail_sections.profile.clone()}</h4>
                                        {detail_view.profile.clone().map(|profile| view! {
                                            <div class="mt-4 space-y-2 text-sm text-muted-foreground">
                                                <p>{profile.identity}</p>
                                                <p>{profile.visibility}</p>
                                                <p>{profile.preferred_locale}</p>
                                                <p>{profile.tags}</p>
                                            </div>
                                        }.into_any()).unwrap_or_else(|| view! {
                                            <p class="mt-4 text-sm text-muted-foreground">{detail_sections.profile_empty.clone()}</p>
                                        }.into_any())}
                                    </div>
                                </div>

                                <div class="rounded-2xl border border-border bg-background p-5">
                                    <h4 class="text-sm font-semibold uppercase tracking-[0.18em] text-muted-foreground">{detail_sections.metadata.clone()}</h4>
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

fn customer_admin_error_labels(locale: Option<&str>) -> CustomerAdminErrorLabels {
    CustomerAdminErrorLabels {
        email_required: t(locale, "customer.error.emailRequired", "Email is required."),
        locale_unavailable: t(
            locale,
            "customer.error.localeUnavailable",
            "Host locale is unavailable.",
        ),
        load_customer: t(
            locale,
            "customer.error.loadCustomer",
            "Failed to load customer",
        ),
        save_customer: t(
            locale,
            "customer.error.saveCustomer",
            "Failed to save customer",
        ),
        load_customers: t(
            locale,
            "customer.error.loadCustomers",
            "Failed to load customers",
        ),
    }
}

fn customer_admin_page_labels(locale: Option<&str>) -> CustomerAdminPageLabels {
    CustomerAdminPageLabels {
        badge: t(locale, "customer.badge", "customer"),
        title: t(locale, "customer.title", "Customer Operations"),
        subtitle: t(
            locale,
            "customer.subtitle",
            "Module-owned customer workspace for tenant-scoped customer records, optional user linkage and profile bridge visibility without routing admin traffic back through the commerce umbrella.",
        ),
        list_title: t(locale, "customer.list.title", "Customers"),
        list_subtitle_template: t(
            locale,
            "customer.list.subtitle",
            "Tenant {tenant} customer records owned by the customer module.",
        ),
        list_subtitle_fallback: t(
            locale,
            "customer.list.subtitleFallback",
            "Tenant-scoped customer records owned by the customer module.",
        ),
        list_loading: t(locale, "customer.loading", "Loading customers..."),
        list_empty: t(
            locale,
            "customer.list.empty",
            "No customers match the current filters.",
        ),
        detail_title: t(locale, "customer.detail.title", "Customer Detail"),
        detail_subtitle: t(
            locale,
            "customer.detail.subtitle",
            "Inspect customer identity, optional user linkage and profile bridge state from the customer-owned route.",
        ),
        detail_empty: t(
            locale,
            "customer.detail.empty",
            "Open a customer to inspect the record, linked user and profile bridge state.",
        ),
        editor_subtitle: t(
            locale,
            "customer.editor.subtitle",
            "Native customer CRUD lives in the customer module package. User linkage is optional and can be set only during customer creation.",
        ),
        edit_title: t(locale, "customer.editor.editTitle", "Edit Customer"),
        create_title: t(locale, "customer.editor.createTitle", "Create Customer"),
        refresh_action: t(locale, "customer.action.refresh", "Refresh"),
        open_action: t(locale, "customer.action.open", "Open"),
        new_action: t(locale, "customer.action.new", "New"),
        save_action: t(locale, "customer.action.save", "Save customer"),
        create_action: t(locale, "customer.action.create", "Create customer"),
        user_id_field: t(locale, "customer.field.userId", "Linked user ID (optional)"),
        email_field: t(locale, "customer.field.email", "Email"),
        first_name_field: t(locale, "customer.field.firstName", "First name"),
        last_name_field: t(locale, "customer.field.lastName", "Last name"),
        phone_field: t(locale, "customer.field.phone", "Phone"),
        customer_section: t(locale, "customer.section.customer", "Customer Record"),
        profile_section: t(locale, "customer.section.profile", "Profile Bridge"),
        metadata_section: t(locale, "customer.section.metadata", "Metadata"),
        profile_empty: t(
            locale,
            "customer.profile.empty",
            "No public profile is linked to this customer yet.",
        ),
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
        phone_label: t(locale, "customer.common.phone", "phone"),
        locale_label: t(locale, "customer.common.locale", "locale"),
        user_label: t(locale, "customer.common.user", "user"),
        created_label: t(locale, "customer.common.created", "created"),
        updated_label: t(locale, "customer.common.updated", "updated"),
        visibility_label: t(locale, "customer.common.visibility", "visibility"),
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
