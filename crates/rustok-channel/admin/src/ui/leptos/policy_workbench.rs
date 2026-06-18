use super::*;

#[component]
pub(super) fn PolicyWorkbench(
    policy_sets: Vec<ChannelResolutionPolicySetDetail>,
    channels: Vec<ChannelDetail>,
    oauth_apps: Vec<crate::model::AvailableOauthAppItem>,
    token: Option<String>,
    tenant: Option<String>,
    set_feedback: WriteSignal<Option<String>>,
    set_error: WriteSignal<Option<String>>,
    set_refresh_nonce: WriteSignal<u64>,
) -> impl IntoView {
    let ui_locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let create_slug = RwSignal::new(String::new());
    let create_name = RwSignal::new(String::new());
    let create_is_active = RwSignal::new(policy_sets.is_empty());
    let create_busy = RwSignal::new(false);
    let section_title = t(
        ui_locale.as_deref(),
        "channel.policies.title",
        "Resolution Policies",
    );
    let section_subtitle = t(
        ui_locale.as_deref(),
        "channel.policies.subtitle",
        "Tenant-scoped typed rules run after built-in host resolution and before the explicit default channel.",
    );
    let empty_title = t(
        ui_locale.as_deref(),
        "channel.policies.emptyTitle",
        "No policy sets yet.",
    );
    let empty_body = t(
        ui_locale.as_deref(),
        "channel.policies.emptyBody",
        "Create the first policy set when channel selection should depend on locale, OAuth app, or richer host matching instead of only explicit selectors and host targets.",
    );
    let selected_policy_set_query = use_route_query_value(AdminQueryKey::PolicySetId.as_str());
    let selected_policy_rule_query = use_route_query_value(AdminQueryKey::PolicyRuleId.as_str());
    let policy_query_writer = use_route_query_writer();
    let policy_sets_for_selection = policy_sets.clone();
    let create_policy_ctx = StoredValue::new((token.clone(), tenant.clone(), ui_locale.clone()));

    Effect::new(move |_| {
        let selected_policy_set_id = selected_policy_set_query.get();
        let selected_policy_rule_id = selected_policy_rule_query.get();

        match channel_policy_selection_cleanup(
            &policy_sets_for_selection,
            selected_policy_set_id.as_deref(),
            selected_policy_rule_id.as_deref(),
        ) {
            ChannelPolicySelectionCleanup::None => {}
            ChannelPolicySelectionCleanup::ClearRule => {
                policy_query_writer.clear_key(AdminQueryKey::PolicyRuleId.as_str());
            }
            ChannelPolicySelectionCleanup::ClearPolicySetAndRule => {
                policy_query_writer.update(
                    vec![
                        (AdminQueryKey::PolicySetId.as_str().to_string(), None),
                        (AdminQueryKey::PolicyRuleId.as_str().to_string(), None),
                    ],
                    false,
                );
            }
        }
    });

    let on_create = move |ev: SubmitEvent| {
        ev.prevent_default();
        create_busy.set(true);
        set_feedback.set(None);
        set_error.set(None);
        let (token, tenant, ui_locale) = create_policy_ctx.get_value();

        spawn_local({
            async move {
                let result = transport::create_resolution_policy_set(
                    token,
                    tenant,
                    &CreateResolutionPolicySetPayload {
                        slug: create_slug.get_untracked(),
                        name: create_name.get_untracked(),
                        is_active: create_is_active.get_untracked(),
                    },
                )
                .await;

                match result {
                    Ok(policy_set) => {
                        set_feedback.set(Some(
                            t(
                                ui_locale.as_deref(),
                                "channel.policies.feedback.created",
                                "Policy set `{slug}` created.",
                            )
                            .replace("{slug}", policy_set.slug.as_str()),
                        ));
                        create_slug.set(String::new());
                        create_name.set(String::new());
                        create_is_active.set(false);
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                    Err(err) => set_error.set(Some(err.to_string())),
                }

                create_busy.set(false);
            }
        });
    };

    view! {
        <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
            <div class="space-y-1">
                <h2 class="text-lg font-semibold text-card-foreground">{section_title}</h2>
                <p class="text-sm text-muted-foreground">{section_subtitle}</p>
            </div>

            <div class="mt-5 space-y-4">
                {if policy_sets.is_empty() {
                    view! {
                        <EmptyState title=empty_title body=empty_body />
                    }.into_any()
                } else {
                    view! {
                        <div class="space-y-4">
                            {policy_sets.into_iter().map(|policy_set| view! {
                                <PolicySetCard
                                    policy_set=policy_set
                                    channels=channels.clone()
                                    oauth_apps=oauth_apps.clone()
                                    token=token.clone()
                                    tenant=tenant.clone()
                                    set_feedback=set_feedback
                                    set_error=set_error
                                    set_refresh_nonce=set_refresh_nonce
                                />
                            }).collect_view()}
                        </div>
                    }.into_any()
                }}
            </div>

            <form class="mt-6 grid gap-3 rounded-xl border border-border bg-background p-4 lg:grid-cols-[1fr_1fr_auto_auto]" on:submit=on_create>
                <input
                    type="text"
                    class="w-full rounded-lg border border-input bg-card px-3 py-2 text-sm"
                    placeholder=t(ui_locale.as_deref(), "channel.policies.slugPlaceholder", "policy slug")
                    prop:value=create_slug
                    on:input=move |ev| create_slug.set(event_target_value(&ev))
                />
                <input
                    type="text"
                    class="w-full rounded-lg border border-input bg-card px-3 py-2 text-sm"
                    placeholder=t(ui_locale.as_deref(), "channel.policies.namePlaceholder", "policy set name")
                    prop:value=create_name
                    on:input=move |ev| create_name.set(event_target_value(&ev))
                />
                <label class="flex items-center gap-2 text-sm text-muted-foreground">
                    <input
                        type="checkbox"
                        prop:checked=create_is_active
                        on:change=move |ev| create_is_active.set(event_target_checked(&ev))
                    />
                    {t(ui_locale.as_deref(), "channel.policies.active", "Activate now")}
                </label>
                <button
                    type="submit"
                    class="inline-flex h-10 items-center justify-center rounded-lg bg-primary px-4 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50"
                    disabled=move || create_busy.get()
                >
                    {move || if create_busy.get() {
                        t(ui_locale.as_deref(), "channel.policies.creating", "Creating...")
                    } else {
                        t(ui_locale.as_deref(), "channel.policies.create", "Create Policy Set")
                    }}
                </button>
            </form>
        </section>
    }
}
