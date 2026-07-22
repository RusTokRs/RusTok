use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
use rustok_api::context::{
    ChannelResolutionOutcome, ChannelResolutionSource, ChannelResolutionStage,
};
use rustok_ui_core::{AdminQueryKey, UiRouteContext, normalize_ui_text};

use crate::core::{
    ChannelPolicySelectionCleanup, PolicyRuleFormState, channel_policy_selection_cleanup,
    channel_selection_exists, policy_rule_active_update_payload, policy_rule_create_form_state,
    policy_rule_edit_form_state, reorder_policy_rule_ids,
};
use crate::i18n::t;
use crate::model::{
    BindChannelModulePayload, BindChannelOauthAppPayload, ChannelAdminBootstrap, ChannelDetail,
    ChannelResolutionPolicySetDetail, CreateChannelPayload, CreateChannelTargetPayload,
    CreateResolutionPolicySetPayload, ReorderResolutionRulesPayload,
};
use crate::transport;

mod channel_card;
mod policy_set_card;
mod policy_workbench;
mod runtime_context;

use channel_card::ChannelCard;
use policy_set_card::PolicySetCard;
use policy_workbench::PolicyWorkbench;
use runtime_context::RuntimeContext;

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
pub fn ChannelAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let selected_channel_query = use_route_query_value(AdminQueryKey::ChannelId.as_str());
    let query_writer = use_route_query_writer();
    let token = leptos_auth::hooks::use_token();
    let tenant = leptos_auth::hooks::use_tenant();
    let badge_label = t(ui_locale.as_deref(), "channel.badge", "Experimental Core");
    let title_label = t(ui_locale.as_deref(), "channel.title", "Channel Management");
    let subtitle_label = t(
        ui_locale.as_deref(),
        "channel.subtitle",
        "Channels define platform-level external delivery context, targets, enabled module surfaces, and bound OAuth apps.",
    );
    let route_label = t(ui_locale.as_deref(), "channel.route", "Route: {route}");
    let create_title = t(
        ui_locale.as_deref(),
        "channel.create.title",
        "Create Channel",
    );
    let create_subtitle = t(
        ui_locale.as_deref(),
        "channel.create.subtitle",
        "Start small: create the channel first, then attach targets and bindings below.",
    );
    let slug_placeholder = t(
        ui_locale.as_deref(),
        "channel.create.slugPlaceholder",
        "slug",
    );
    let name_placeholder = t(
        ui_locale.as_deref(),
        "channel.create.namePlaceholder",
        "name",
    );
    let creating_label = t(
        ui_locale.as_deref(),
        "channel.create.creating",
        "Creating...",
    );
    let create_label = t(ui_locale.as_deref(), "channel.create.submit", "Create");
    let empty_channels_label = t(
        ui_locale.as_deref(),
        "channel.empty.channels",
        "No channels configured yet.",
    );
    let load_bootstrap_error = t(
        ui_locale.as_deref(),
        "channel.error.loadBootstrap",
        "Failed to load channel bootstrap",
    );

    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (feedback, set_feedback) = signal(Option::<String>::None);
    let (error, set_error) = signal(Option::<String>::None);
    let create_slug = RwSignal::new(String::new());
    let create_name = RwSignal::new(String::new());
    let create_busy = RwSignal::new(false);

    let bootstrap = local_resource(
        move || (token.get(), tenant.get(), refresh_nonce.get()),
        move |(token_value, tenant_value, _)| async move {
            transport::fetch_bootstrap(token_value, tenant_value).await
        },
    );
    let create_channel_query_writer = query_writer.clone();

    Effect::new(move |_| {
        let selected_channel_id = selected_channel_query.get();
        match (selected_channel_id.as_deref(), bootstrap.get()) {
            (Some(channel_id), Some(Ok(ref bootstrap)))
                if !channel_selection_exists(bootstrap, channel_id) =>
            {
                query_writer.clear_key(AdminQueryKey::ChannelId.as_str());
            }
            _ => {}
        }
    });

    let on_create = move |ev: SubmitEvent| {
        ev.prevent_default();
        create_busy.set(true);
        set_feedback.set(None);
        set_error.set(None);
        let ui_locale = ui_locale.clone();
        let create_channel_query_writer = create_channel_query_writer.clone();

        spawn_local({
            let token_value = token.get_untracked();
            let tenant_value = tenant.get_untracked();
            let slug = create_slug.get_untracked();
            let name = create_name.get_untracked();
            async move {
                let result = transport::create_channel(
                    token_value,
                    tenant_value,
                    &CreateChannelPayload {
                        tenant_id: None,
                        slug,
                        name,
                        settings: Some(serde_json::json!({})),
                    },
                )
                .await;

                match result {
                    Ok(channel) => {
                        set_feedback.set(Some(
                            t(
                                ui_locale.as_deref(),
                                "channel.feedback.created",
                                "Channel `{slug}` created.",
                            )
                            .replace("{slug}", channel.slug.as_str()),
                        ));
                        create_slug.set(String::new());
                        create_name.set(String::new());
                        create_channel_query_writer
                            .replace_value(AdminQueryKey::ChannelId.as_str(), channel.id.clone());
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                    Err(err) => set_error.set(Some(err.to_string())),
                }

                create_busy.set(false);
            }
        });
    };

    let route_segment = route_context
        .route_segment
        .clone()
        .unwrap_or_else(|| "channels".to_string());

    view! {
        <div class="space-y-6">
            <header class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
                    <div class="space-y-2">
                        <span class="inline-flex items-center rounded-full border border-amber-300 bg-amber-50 px-3 py-1 text-xs font-semibold uppercase tracking-wide text-amber-700">
                            {badge_label.clone()}
                        </span>
                        <h1 class="text-2xl font-semibold text-card-foreground">{title_label.clone()}</h1>
                        <p class="max-w-3xl text-sm text-muted-foreground">
                            {subtitle_label.clone()}
                        </p>
                    </div>
                    <div class="rounded-xl border border-border bg-background px-4 py-3 text-sm text-muted-foreground">
                        {route_label.replace("{route}", format!("/modules/{route_segment}").as_str())}
                    </div>
                </div>
            </header>

            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-1">
                    <h2 class="text-lg font-semibold text-card-foreground">{create_title.clone()}</h2>
                    <p class="text-sm text-muted-foreground">
                        {create_subtitle.clone()}
                    </p>
                </div>
                <form class="mt-5 grid gap-4 lg:grid-cols-[1fr_1fr_auto]" on:submit=on_create>
                    <input
                        type="text"
                        class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                        placeholder=slug_placeholder.clone()
                        prop:value=create_slug
                        on:input=move |ev| create_slug.set(event_target_value(&ev))
                    />
                    <input
                        type="text"
                        class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                        placeholder=name_placeholder.clone()
                        prop:value=create_name
                        on:input=move |ev| create_name.set(event_target_value(&ev))
                    />
                    <button
                        type="submit"
                        class="inline-flex h-10 items-center justify-center rounded-lg bg-primary px-4 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50"
                        disabled=move || create_busy.get()
                    >
                        {move || if create_busy.get() { creating_label.clone() } else { create_label.clone() }}
                    </button>
                </form>
                <Show when=move || feedback.get().is_some()>
                    <div class="mt-4 rounded-xl border border-emerald-300 bg-emerald-50 px-4 py-3 text-sm text-emerald-700">
                        {move || feedback.get().unwrap_or_default()}
                    </div>
                </Show>
                <Show when=move || error.get().is_some()>
                    <div class="mt-4 rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                        {move || error.get().unwrap_or_default()}
                    </div>
                </Show>
            </section>

            <Suspense fallback=move || view! { <div class="h-48 animate-pulse rounded-2xl bg-muted"></div> }>
                {move || {
                    bootstrap.get().map(|result| match result {
                        Ok(bootstrap) => view! {
                            <div class="space-y-6">
                                <RuntimeContext bootstrap=bootstrap.clone() />
                                <PolicyWorkbench
                                    policy_sets=bootstrap.policy_sets.clone()
                                    channels=bootstrap.channels.clone()
                                    oauth_apps=bootstrap.oauth_apps.clone()
                                    token=token.get()
                                    tenant=tenant.get()
                                    set_feedback=set_feedback
                                    set_error=set_error
                                    set_refresh_nonce=set_refresh_nonce
                                />
                                {if bootstrap.channels.is_empty() {
                                    view! {
                                        <div class="rounded-2xl border border-dashed border-border bg-card p-8 text-center text-sm text-muted-foreground">
                                            {empty_channels_label.clone()}
                                        </div>
                                    }.into_any()
                                } else {
                                    view! {
                                        <div class="space-y-4">
                                            {bootstrap.channels.into_iter().map(|channel| view! {
                                                <ChannelCard
                                                    channel=channel
                                                    available_modules=bootstrap.available_modules.clone()
                                                    oauth_apps=bootstrap.oauth_apps.clone()
                                                    token=token.get()
                                                    tenant=tenant.get()
                                                    set_feedback=set_feedback
                                                    set_error=set_error
                                                    set_refresh_nonce=set_refresh_nonce
                                                />
                                            }).collect_view()}
                                        </div>
                                    }.into_any()
                                }}
                            </div>
                        }.into_any(),
                        Err(err) => view! {
                            <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-5 py-4 text-sm text-destructive">
                                {format!("{}: {err}", load_bootstrap_error.clone())}
                            </div>
                        }.into_any(),
                    })
                }}
            </Suspense>
        </div>
    }
}

#[component]
fn InfoPill(label: String, value: String) -> impl IntoView {
    view! {
        <div class="rounded-xl border border-border bg-background px-4 py-3">
            <div class="text-xs font-medium uppercase tracking-wide text-muted-foreground">{label}</div>
            <div class="mt-1 text-sm font-medium text-card-foreground">{value}</div>
        </div>
    }
}

#[component]
fn EmptyState(title: String, body: String) -> impl IntoView {
    view! {
        <div class="rounded-lg border border-dashed border-border px-3 py-4 text-sm">
            <div class="font-medium text-card-foreground">{title}</div>
            <div class="mt-1 text-muted-foreground">{body}</div>
        </div>
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_policy_rule_form_state(
    priority: RwSignal<i32>,
    is_active: RwSignal<bool>,
    action_channel_id: RwSignal<String>,
    host_equals: RwSignal<String>,
    host_suffix: RwSignal<String>,
    locale: RwSignal<String>,
    surface: RwSignal<String>,
    oauth_app_id: RwSignal<String>,
    state: &PolicyRuleFormState,
) {
    priority.set(state.priority);
    is_active.set(state.is_active);
    action_channel_id.set(state.action_channel_id.clone());
    host_equals.set(state.host_equals.clone());
    host_suffix.set(state.host_suffix.clone());
    locale.set(state.locale.clone());
    surface.set(state.surface.clone());
    oauth_app_id.set(state.oauth_app_id.clone());
}

fn policy_rule_summary(
    rule: &crate::model::ChannelResolutionRuleRecord,
    channels: &[ChannelDetail],
) -> String {
    let action_channel = channels
        .iter()
        .find(|channel| channel.channel.id == rule.action_channel_id)
        .map(|channel| channel.channel.slug.clone())
        .unwrap_or_else(|| short_id(rule.action_channel_id.as_str()));
    let predicates = rule
        .definition
        .predicates
        .iter()
        .map(|predicate| match predicate {
            crate::model::ChannelResolutionPredicateRecord::HostEquals(value) => {
                format!("host = {value}")
            }
            crate::model::ChannelResolutionPredicateRecord::HostSuffix(value) => {
                format!("host suffix = {value}")
            }
            crate::model::ChannelResolutionPredicateRecord::OAuthAppEquals(value) => {
                format!("oauth app = {}", short_id(value.as_str()))
            }
            crate::model::ChannelResolutionPredicateRecord::SurfaceIs(value) => {
                format!("surface = {value}")
            }
            crate::model::ChannelResolutionPredicateRecord::LocaleEquals(value) => {
                format!("locale = {value}")
            }
        })
        .collect::<Vec<_>>()
        .join(" + ");

    format!("{predicates} -> {action_channel}")
}

fn short_id(value: &str) -> String {
    value.chars().take(8).collect()
}

fn optional_text(value: String) -> Option<String> {
    normalize_ui_text(value.as_str())
}

fn resolution_source_label(source: &ChannelResolutionSource, locale: Option<&str>) -> String {
    match source {
        ChannelResolutionSource::HeaderId => t(locale, "channel.source.headerId", "Header ID"),
        ChannelResolutionSource::HeaderSlug => {
            t(locale, "channel.source.headerSlug", "Header Slug")
        }
        ChannelResolutionSource::Query => t(locale, "channel.source.query", "Query"),
        ChannelResolutionSource::Host => t(locale, "channel.source.host", "Host"),
        ChannelResolutionSource::Policy => t(locale, "channel.source.policy", "Policy"),
        ChannelResolutionSource::Default => t(locale, "channel.source.default", "Default"),
    }
}

fn resolution_source_description(source: &ChannelResolutionSource, locale: Option<&str>) -> String {
    match source {
        ChannelResolutionSource::HeaderId => t(
            locale,
            "channel.sourceDescription.headerId",
            "The current request explicitly selected this channel through the X-Channel-ID header.",
        ),
        ChannelResolutionSource::HeaderSlug => t(
            locale,
            "channel.sourceDescription.headerSlug",
            "The current request explicitly selected this channel through the X-Channel-Slug header.",
        ),
        ChannelResolutionSource::Query => t(
            locale,
            "channel.sourceDescription.query",
            "The current request selected this channel through the query parameter fallback.",
        ),
        ChannelResolutionSource::Host => t(
            locale,
            "channel.sourceDescription.host",
            "The current request matched this channel through host-based target resolution.",
        ),
        ChannelResolutionSource::Policy => t(
            locale,
            "channel.sourceDescription.policy",
            "The current request matched a tenant-scoped typed channel resolution policy.",
        ),
        ChannelResolutionSource::Default => t(
            locale,
            "channel.sourceDescription.default",
            "No explicit channel selector matched, so the tenant's explicit default channel was used.",
        ),
    }
}

fn resolution_stage_label(stage: &ChannelResolutionStage, locale: Option<&str>) -> String {
    match stage {
        ChannelResolutionStage::HeaderId => t(locale, "channel.trace.stage.headerId", "Header ID"),
        ChannelResolutionStage::HeaderSlug => {
            t(locale, "channel.trace.stage.headerSlug", "Header Slug")
        }
        ChannelResolutionStage::Query => t(locale, "channel.trace.stage.query", "Query"),
        ChannelResolutionStage::Host => t(locale, "channel.trace.stage.host", "Host"),
        ChannelResolutionStage::Policy => t(locale, "channel.trace.stage.policy", "Policy"),
        ChannelResolutionStage::Default => t(locale, "channel.trace.stage.default", "Default"),
    }
}

fn resolution_outcome_label(outcome: &ChannelResolutionOutcome, locale: Option<&str>) -> String {
    match outcome {
        ChannelResolutionOutcome::Matched => t(locale, "channel.trace.outcome.matched", "Matched"),
        ChannelResolutionOutcome::Miss => t(locale, "channel.trace.outcome.miss", "Miss"),
        ChannelResolutionOutcome::Rejected => {
            t(locale, "channel.trace.outcome.rejected", "Rejected")
        }
    }
}

fn resolution_outcome_badge_class(outcome: &ChannelResolutionOutcome) -> &'static str {
    match outcome {
        ChannelResolutionOutcome::Matched => {
            "inline-flex items-center rounded-full border border-emerald-200 bg-emerald-50 px-2 py-1 font-medium text-emerald-700"
        }
        ChannelResolutionOutcome::Miss => {
            "inline-flex items-center rounded-full border border-amber-200 bg-amber-50 px-2 py-1 font-medium text-amber-700"
        }
        ChannelResolutionOutcome::Rejected => {
            "inline-flex items-center rounded-full border border-rose-200 bg-rose-50 px-2 py-1 font-medium text-rose-700"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::policy_rule_edit_form_state;
    use crate::model::{
        ChannelDetail, ChannelRecord, ChannelResolutionActionRecord,
        ChannelResolutionPredicateRecord, ChannelResolutionRuleDefinitionRecord,
        ChannelResolutionRuleRecord,
    };

    #[test]
    fn policy_rule_edit_form_state_prefills_predicates_and_action_channel() {
        let rule = ChannelResolutionRuleRecord {
            id: "rule_01".to_string(),
            policy_set_id: "policy_set_01".to_string(),
            priority: 30,
            is_active: false,
            action_channel_id: "channel_01".to_string(),
            definition: ChannelResolutionRuleDefinitionRecord {
                predicates: vec![
                    ChannelResolutionPredicateRecord::HostEquals("shop.example.test".to_string()),
                    ChannelResolutionPredicateRecord::OAuthAppEquals(
                        "550e8400-e29b-41d4-a716-446655440000".to_string(),
                    ),
                    ChannelResolutionPredicateRecord::SurfaceIs("http".to_string()),
                    ChannelResolutionPredicateRecord::LocaleEquals("ru-by".to_string()),
                ],
                action: ChannelResolutionActionRecord::ResolveToChannel {
                    channel_id: "channel_01".to_string(),
                },
            },
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let channels = vec![ChannelDetail {
            channel: ChannelRecord {
                id: "channel_01".to_string(),
                tenant_id: "tenant_01".to_string(),
                slug: "web".to_string(),
                name: "Web".to_string(),
                is_active: true,
                is_default: true,
                status: "experimental".to_string(),
                settings: serde_json::json!({}),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
            },
            targets: Vec::new(),
            module_bindings: Vec::new(),
            oauth_apps: Vec::new(),
        }];

        let form_state = policy_rule_edit_form_state(&rule, &channels);

        assert_eq!(form_state.priority, 30);
        assert!(!form_state.is_active);
        assert_eq!(form_state.action_channel_id, "channel_01");
        assert_eq!(form_state.host_equals, "shop.example.test");
        assert_eq!(
            form_state.oauth_app_id,
            "550e8400-e29b-41d4-a716-446655440000"
        );
        assert_eq!(form_state.surface, "http");
        assert_eq!(form_state.locale, "ru-by");
    }
}
