mod api;
mod model;

use leptos::ev::{MouseEvent, SubmitEvent};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::web_sys;
use leptos_router::components::A;
use rustok_api::UiRouteContext;

use crate::model::{
    LaggingSearchDocumentPayload, SearchAdminBootstrap, SearchAnalyticsPayload,
    SearchConsistencyIssuePayload, SearchDiagnosticsPayload, SearchDictionarySnapshotPayload,
    SearchFacetGroup, SearchFilterPresetPayload, SearchPreviewFilters, SearchPreviewPayload,
    SearchQueryRulePayload, SearchStopWordPayload, SearchSynonymPayload,
};

#[component]
pub fn SearchAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let route_segment = route_context
        .route_segment
        .clone()
        .unwrap_or_else(|| "search".to_string());
    let initial_query = route_context.query_value("q").unwrap_or("").to_string();
    let initial_locale = route_context.locale.clone();
    let on_playground = route_context.subpath_matches("playground");
    let on_diagnostics = route_context.subpath_matches("analytics");
    let on_dictionaries = route_context.subpath_matches("dictionaries");

    let token = leptos_auth::hooks::use_token();
    let tenant = leptos_auth::hooks::use_tenant();

    let (query, set_query) = signal(initial_query);
    let (entity_types, set_entity_types) = signal(String::new());
    let (source_modules, set_source_modules) = signal(String::new());
    let (statuses, set_statuses) = signal(String::new());
    let (ranking_profile, set_ranking_profile) = signal(String::new());
    let (preset_key, set_preset_key) = signal(String::new());
    let (preview, set_preview) = signal(Option::<SearchPreviewPayload>::None);
    let (preview_error, set_preview_error) = signal(Option::<String>::None);
    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (rebuild_busy, set_rebuild_busy) = signal(false);
    let (rebuild_feedback, set_rebuild_feedback) = signal(Option::<String>::None);
    let (rebuild_target_type, set_rebuild_target_type) = signal("search".to_string());
    let (rebuild_target_id, set_rebuild_target_id) = signal(String::new());
    let (busy, set_busy) = signal(false);
    let (settings_active_engine, set_settings_active_engine) = signal("postgres".to_string());
    let (settings_fallback_engine, set_settings_fallback_engine) = signal("postgres".to_string());
    let (settings_config, set_settings_config) = signal("{}".to_string());
    let (ranking_default_profile, set_ranking_default_profile) = signal("balanced".to_string());
    let (ranking_preview_profile, set_ranking_preview_profile) = signal("balanced".to_string());
    let (ranking_storefront_profile, set_ranking_storefront_profile) =
        signal("balanced".to_string());
    let (ranking_admin_global_profile, set_ranking_admin_global_profile) =
        signal("exact".to_string());
    let (preview_presets_config, set_preview_presets_config) = signal("[]".to_string());
    let (storefront_presets_config, set_storefront_presets_config) = signal("[]".to_string());
    let (settings_busy, set_settings_busy) = signal(false);
    let (settings_feedback, set_settings_feedback) = signal(Option::<String>::None);

    let bootstrap = Resource::new(
        move || (token.get(), tenant.get(), refresh_nonce.get()),
        move |(token_value, tenant_value, _)| async move {
            api::fetch_bootstrap(token_value, tenant_value).await
        },
    );
    let lagging_documents = Resource::new(
        move || (token.get(), tenant.get(), refresh_nonce.get()),
        move |(token_value, tenant_value, _)| async move {
            api::fetch_lagging_documents(token_value, tenant_value, Some(25)).await
        },
    );
    let consistency_issues = Resource::new(
        move || (token.get(), tenant.get(), refresh_nonce.get()),
        move |(token_value, tenant_value, _)| async move {
            api::fetch_consistency_issues(token_value, tenant_value, Some(25)).await
        },
    );
    let search_analytics = Resource::new(
        move || (token.get(), tenant.get(), refresh_nonce.get()),
        move |(token_value, tenant_value, _)| async move {
            api::fetch_search_analytics(token_value, tenant_value, Some(7), Some(10)).await
        },
    );
    let filter_presets = Resource::new(
        move || (token.get(), tenant.get(), refresh_nonce.get()),
        move |(token_value, tenant_value, _)| async move {
            api::fetch_filter_presets(token_value, tenant_value, "search_preview").await
        },
    );

    Effect::new(move |_| {
        if let Some(Ok(bootstrap)) = bootstrap.get() {
            set_settings_active_engine.set(bootstrap.search_settings_preview.active_engine.clone());
            set_settings_fallback_engine
                .set(bootstrap.search_settings_preview.fallback_engine.clone());
            set_settings_config.set(pretty_json_string(
                &bootstrap.search_settings_preview.config,
            ));
            if let Some(config) = parse_json_for_editor(&bootstrap.search_settings_preview.config) {
                set_ranking_default_profile.set(extract_ranking_profile_value(&config, "default"));
                set_ranking_preview_profile
                    .set(extract_ranking_profile_value(&config, "search_preview"));
                set_ranking_storefront_profile
                    .set(extract_ranking_profile_value(&config, "storefront_search"));
                set_ranking_admin_global_profile.set(extract_ranking_profile_value(
                    &config,
                    "admin_global_search",
                ));
                set_preview_presets_config
                    .set(extract_surface_presets_json(&config, "search_preview"));
                set_storefront_presets_config
                    .set(extract_surface_presets_json(&config, "storefront_search"));
            }
        }
    });

    let run_preview = Callback::new(move |ev: SubmitEvent| {
        ev.prevent_default();
        set_preview_error.set(None);
        set_busy.set(true);
        let filters = SearchPreviewFilters {
            entity_types: parse_csv(entity_types.get_untracked()),
            source_modules: parse_csv(source_modules.get_untracked()),
            statuses: parse_csv(statuses.get_untracked()),
        };
        spawn_local({
            let token_value = token.get_untracked();
            let tenant_value = tenant.get_untracked();
            let query_value = query.get_untracked();
            let ranking_profile_value = ranking_profile.get_untracked();
            let preset_key_value = optional_text(preset_key.get_untracked());
            let locale_value = initial_locale.clone();
            async move {
                match api::fetch_search_preview(
                    token_value,
                    tenant_value,
                    query_value,
                    locale_value,
                    optional_text(ranking_profile_value),
                    preset_key_value,
                    filters,
                )
                .await
                {
                    Ok(result) => set_preview.set(Some(result)),
                    Err(err) => {
                        set_preview_error.set(Some(format!("Failed to run search preview: {err}")))
                    }
                }
                set_busy.set(false);
            }
        });
    });

    let queue_rebuild = move |_| {
        set_rebuild_busy.set(true);
        set_rebuild_feedback.set(None);
        spawn_local({
            let token_value = token.get_untracked();
            let tenant_value = tenant.get_untracked();
            let target_type = rebuild_target_type.get_untracked();
            let target_id = optional_text(rebuild_target_id.get_untracked());
            async move {
                match api::trigger_search_rebuild(
                    token_value,
                    tenant_value,
                    Some(target_type.clone()),
                    target_id,
                )
                .await
                {
                    Ok(result) => {
                        let suffix = result
                            .target_id
                            .as_ref()
                            .map(|id| format!(" for target {id}"))
                            .unwrap_or_default();
                        set_rebuild_feedback.set(Some(format!(
                            "Rebuild queued for {} scope{}.",
                            result.target_type, suffix
                        )));
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                    Err(err) => set_rebuild_feedback
                        .set(Some(format!("Failed to queue search rebuild: {err}"))),
                }
                set_rebuild_busy.set(false);
            }
        });
    };

    view! {
        <div class="space-y-6">
            <header class="flex flex-col gap-4 rounded-2xl border border-border bg-card p-6 shadow-sm lg:flex-row lg:items-start lg:justify-between">
                <div class="space-y-2">
                    <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">"search"</span>
                    <h1 class="text-2xl font-semibold text-card-foreground">"Search Control Plane"</h1>
                    <p class="max-w-3xl text-sm text-muted-foreground">
                        "Module-owned admin surface for search diagnostics, scoped rebuilds, and PostgreSQL FTS inspection."
                    </p>
                </div>
                <div class="flex flex-wrap gap-2">
                    <A href=format!("/modules/{route_segment}") attr:class=tab_class(!on_playground && !on_diagnostics && !on_dictionaries)>"Overview"</A>
                    <A href=format!("/modules/{route_segment}/playground") attr:class=tab_class(on_playground)>"Playground"</A>
                    <A href=format!("/modules/{route_segment}/analytics") attr:class=tab_class(on_diagnostics)>"Analytics"</A>
                    <A href=format!("/modules/{route_segment}/dictionaries") attr:class=tab_class(on_dictionaries)>"Dictionaries"</A>
                </div>
            </header>

            <Suspense fallback=move || view! { <div class="h-32 animate-pulse rounded-2xl bg-muted"></div> }>
                {move || {
                    bootstrap.get().map(|result| match result {
                        Ok(bootstrap) => {
                            if on_playground {
                                playground_view(
                                    query,
                                    set_query,
                                    entity_types,
                                    set_entity_types,
                                    source_modules,
                                    set_source_modules,
                                    statuses,
                                    set_statuses,
                                    ranking_profile,
                                    set_ranking_profile,
                                    preset_key,
                                    set_preset_key,
                                    filter_presets,
                                    preview,
                                    preview_error,
                                    busy,
                                    run_preview,
                                ).into_any()
                            } else if on_diagnostics {
                                analytics_view(
                                    bootstrap.search_diagnostics,
                                    lagging_documents,
                                    consistency_issues,
                                    search_analytics,
                                )
                                .into_any()
                            } else if on_dictionaries {
                                view! { <DictionariesView /> }.into_any()
                            } else {
                                overview_view(
                                    bootstrap,
                                    settings_active_engine,
                                    set_settings_active_engine,
                                    settings_fallback_engine,
                                    set_settings_fallback_engine,
                                    settings_config,
                                    set_settings_config,
                                    ranking_default_profile,
                                    set_ranking_default_profile,
                                    ranking_preview_profile,
                                    set_ranking_preview_profile,
                                    ranking_storefront_profile,
                                    set_ranking_storefront_profile,
                                    ranking_admin_global_profile,
                                    set_ranking_admin_global_profile,
                                    preview_presets_config,
                                    set_preview_presets_config,
                                    storefront_presets_config,
                                    set_storefront_presets_config,
                                    settings_busy,
                                    settings_feedback,
                                    move |_| {
                                        let config = settings_config.get_untracked();
                                        let merged_config = match merge_relevance_editor_config(
                                            &config,
                                            &ranking_default_profile.get_untracked(),
                                            &ranking_preview_profile.get_untracked(),
                                            &ranking_storefront_profile.get_untracked(),
                                            &ranking_admin_global_profile.get_untracked(),
                                            &preview_presets_config.get_untracked(),
                                            &storefront_presets_config.get_untracked(),
                                        ) {
                                            Ok(config) => config,
                                            Err(err) => {
                                                set_settings_feedback.set(Some(err));
                                                return;
                                            }
                                        };
                                        if parse_json_for_editor(&merged_config).is_none() {
                                            set_settings_feedback.set(Some(
                                                "Settings config must be valid JSON.".to_string(),
                                            ));
                                            return;
                                        }

                                        set_settings_busy.set(true);
                                        set_settings_feedback.set(None);
                                        spawn_local({
                                            let token_value = token.get_untracked();
                                            let tenant_value = tenant.get_untracked();
                                            let active_engine =
                                                settings_active_engine.get_untracked();
                                            let fallback_engine =
                                                settings_fallback_engine.get_untracked();
                                            async move {
                                                match api::update_search_settings(
                                                    token_value,
                                                    tenant_value,
                                                    active_engine,
                                                    Some(fallback_engine),
                                                    merged_config,
                                                )
                                                .await
                                                {
                                                    Ok(settings) => {
                                                        set_settings_feedback.set(Some(
                                                            "Search settings saved.".to_string(),
                                                        ));
                                                        set_settings_active_engine
                                                            .set(settings.active_engine.clone());
                                                        set_settings_fallback_engine
                                                            .set(settings.fallback_engine.clone());
                                                        set_settings_config.set(pretty_json_string(
                                                            &settings.config,
                                                        ));
                                                        if let Some(config) =
                                                            parse_json_for_editor(&settings.config)
                                                        {
                                                            set_ranking_default_profile.set(
                                                                extract_ranking_profile_value(
                                                                    &config, "default",
                                                                ),
                                                            );
                                                            set_ranking_preview_profile.set(
                                                                extract_ranking_profile_value(
                                                                    &config,
                                                                    "search_preview",
                                                                ),
                                                            );
                                                            set_ranking_storefront_profile.set(
                                                                extract_ranking_profile_value(
                                                                    &config,
                                                                    "storefront_search",
                                                                ),
                                                            );
                                                            set_ranking_admin_global_profile.set(
                                                                extract_ranking_profile_value(
                                                                    &config,
                                                                    "admin_global_search",
                                                                ),
                                                            );
                                                            set_preview_presets_config.set(
                                                                extract_surface_presets_json(
                                                                    &config,
                                                                    "search_preview",
                                                                ),
                                                            );
                                                            set_storefront_presets_config.set(
                                                                extract_surface_presets_json(
                                                                    &config,
                                                                    "storefront_search",
                                                                ),
                                                            );
                                                        }
                                                        set_refresh_nonce
                                                            .update(|value| *value += 1);
                                                    }
                                                    Err(err) => set_settings_feedback.set(Some(
                                                        format!(
                                                            "Failed to save search settings: {err}"
                                                        ),
                                                    )),
                                                }
                                                set_settings_busy.set(false);
                                            }
                                        });
                                    },
                                    rebuild_target_type,
                                    set_rebuild_target_type,
                                    rebuild_target_id,
                                    set_rebuild_target_id,
                                    rebuild_busy,
                                    rebuild_feedback,
                                    queue_rebuild,
                                ).into_any()
                            }
                        }
                        Err(err) => view! {
                            <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                {format!("Failed to load search bootstrap data: {err}")}
                            </div>
                        }.into_any(),
                    })
                }}
            </Suspense>
        </div>
    }
}

fn overview_view(
    bootstrap: SearchAdminBootstrap,
    settings_active_engine: ReadSignal<String>,
    set_settings_active_engine: WriteSignal<String>,
    settings_fallback_engine: ReadSignal<String>,
    set_settings_fallback_engine: WriteSignal<String>,
    settings_config: ReadSignal<String>,
    set_settings_config: WriteSignal<String>,
    ranking_default_profile: ReadSignal<String>,
    set_ranking_default_profile: WriteSignal<String>,
    ranking_preview_profile: ReadSignal<String>,
    set_ranking_preview_profile: WriteSignal<String>,
    ranking_storefront_profile: ReadSignal<String>,
    set_ranking_storefront_profile: WriteSignal<String>,
    ranking_admin_global_profile: ReadSignal<String>,
    set_ranking_admin_global_profile: WriteSignal<String>,
    preview_presets_config: ReadSignal<String>,
    set_preview_presets_config: WriteSignal<String>,
    storefront_presets_config: ReadSignal<String>,
    set_storefront_presets_config: WriteSignal<String>,
    settings_busy: ReadSignal<bool>,
    settings_feedback: ReadSignal<Option<String>>,
    save_settings: impl Fn(MouseEvent) + 'static + Copy,
    rebuild_target_type: ReadSignal<String>,
    set_rebuild_target_type: WriteSignal<String>,
    rebuild_target_id: ReadSignal<String>,
    set_rebuild_target_id: WriteSignal<String>,
    rebuild_busy: ReadSignal<bool>,
    rebuild_feedback: ReadSignal<Option<String>>,
    queue_rebuild: impl Fn(MouseEvent) + 'static + Copy,
) -> impl IntoView {
    view! {
        <section class="space-y-6">
            <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
                <InfoCard title="Active engine" value=bootstrap.search_settings_preview.active_engine detail="Effective search settings for the current tenant." />
                <InfoCard title="Fallback engine" value=bootstrap.search_settings_preview.fallback_engine detail="Used when an external engine is configured but unavailable." />
                <InfoCard title="Available engines" value=bootstrap.available_search_engines.len().to_string() detail="Only connectors installed in the runtime appear here." />
                <InfoCard title="Updated at" value=bootstrap.search_settings_preview.updated_at detail="Timestamp of the effective settings record." />
            </div>
            <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-6 2xl:grid-cols-7">
                <DiagnosticsCard diagnostics=bootstrap.search_diagnostics.clone() />
                <InfoCard title="Documents" value=bootstrap.search_diagnostics.total_documents.to_string() detail="Total search documents in rustok-search storage." />
                <InfoCard title="Public docs" value=bootstrap.search_diagnostics.public_documents.to_string() detail="Published documents visible to storefront search." />
                <InfoCard title="Stale docs" value=bootstrap.search_diagnostics.stale_documents.to_string() detail="Documents where indexed_at lags behind source updated_at." />
                <InfoCard title="Missing docs" value=bootstrap.search_diagnostics.missing_documents.to_string() detail="Source rows that should exist in search_documents but do not." />
                <InfoCard title="Orphaned docs" value=bootstrap.search_diagnostics.orphaned_documents.to_string() detail="Search documents that no longer have a matching source row." />
                <InfoCard title="Max lag" value=format!("{}s", bootstrap.search_diagnostics.max_lag_seconds) detail="Worst-case lag between source update and search projection." />
            </div>
            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-1">
                    <h2 class="text-lg font-semibold text-card-foreground">"Engine Settings"</h2>
                    <p class="text-sm text-muted-foreground">
                        "Save the effective search engine selection and JSON config for the current tenant. Only engines installed in the runtime appear here."
                    </p>
                </div>
                <div class="mt-5 grid gap-4 md:grid-cols-2">
                    <label class="block space-y-2">
                        <span class="text-sm font-medium text-card-foreground">"Active engine"</span>
                        <select class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=settings_active_engine on:change=move |ev| set_settings_active_engine.set(event_target_value(&ev))>
                            {bootstrap.available_search_engines.iter().map(|engine| view! {
                                <option value=engine.kind.clone()>{format!("{} ({})", engine.label, engine.kind)}</option>
                            }).collect_view()}
                        </select>
                    </label>
                    <label class="block space-y-2">
                        <span class="text-sm font-medium text-card-foreground">"Fallback engine"</span>
                        <select class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=settings_fallback_engine on:change=move |ev| set_settings_fallback_engine.set(event_target_value(&ev))>
                            {bootstrap.available_search_engines.iter().map(|engine| view! {
                                <option value=engine.kind.clone()>{format!("{} ({})", engine.label, engine.kind)}</option>
                            }).collect_view()}
                        </select>
                    </label>
                </div>
                <div class="mt-6 rounded-xl border border-border bg-muted/20 p-4">
                    <div class="space-y-1">
                        <h3 class="text-base font-semibold text-card-foreground">"Relevance Settings"</h3>
                        <p class="text-sm text-muted-foreground">
                            "Structured editor for ranking defaults and filter presets. These values are merged back into `search_settings.config` on save."
                        </p>
                    </div>
                    <div class="mt-4 grid gap-4 xl:grid-cols-2">
                        <label class="block space-y-2">
                            <span class="text-sm font-medium text-card-foreground">"Default ranking profile"</span>
                            <select class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=ranking_default_profile on:change=move |ev| set_ranking_default_profile.set(event_target_value(&ev))>
                                <option value="balanced">"balanced"</option>
                                <option value="exact">"exact"</option>
                                <option value="fresh">"fresh"</option>
                                <option value="catalog">"catalog"</option>
                                <option value="content">"content"</option>
                            </select>
                        </label>
                        <label class="block space-y-2">
                            <span class="text-sm font-medium text-card-foreground">"Preview ranking profile"</span>
                            <select class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=ranking_preview_profile on:change=move |ev| set_ranking_preview_profile.set(event_target_value(&ev))>
                                <option value="balanced">"balanced"</option>
                                <option value="exact">"exact"</option>
                                <option value="fresh">"fresh"</option>
                                <option value="catalog">"catalog"</option>
                                <option value="content">"content"</option>
                            </select>
                        </label>
                        <label class="block space-y-2">
                            <span class="text-sm font-medium text-card-foreground">"Storefront ranking profile"</span>
                            <select class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=ranking_storefront_profile on:change=move |ev| set_ranking_storefront_profile.set(event_target_value(&ev))>
                                <option value="balanced">"balanced"</option>
                                <option value="exact">"exact"</option>
                                <option value="fresh">"fresh"</option>
                                <option value="catalog">"catalog"</option>
                                <option value="content">"content"</option>
                            </select>
                        </label>
                        <label class="block space-y-2">
                            <span class="text-sm font-medium text-card-foreground">"Admin global ranking profile"</span>
                            <select class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=ranking_admin_global_profile on:change=move |ev| set_ranking_admin_global_profile.set(event_target_value(&ev))>
                                <option value="balanced">"balanced"</option>
                                <option value="exact">"exact"</option>
                                <option value="fresh">"fresh"</option>
                                <option value="catalog">"catalog"</option>
                                <option value="content">"content"</option>
                            </select>
                        </label>
                    </div>
                    <div class="mt-4 grid gap-4 xl:grid-cols-2">
                        <label class="block space-y-2">
                            <span class="text-sm font-medium text-card-foreground">"Preview filter presets (JSON array)"</span>
                            <textarea class="min-h-[12rem] w-full rounded-lg border border-input bg-background px-3 py-2 font-mono text-sm" prop:value=preview_presets_config on:input=move |ev| set_preview_presets_config.set(event_target_value(&ev)) />
                            <p class="text-xs text-muted-foreground">"Each item supports: key, label, entity_types, source_modules, statuses, ranking_profile."</p>
                        </label>
                        <label class="block space-y-2">
                            <span class="text-sm font-medium text-card-foreground">"Storefront filter presets (JSON array)"</span>
                            <textarea class="min-h-[12rem] w-full rounded-lg border border-input bg-background px-3 py-2 font-mono text-sm" prop:value=storefront_presets_config on:input=move |ev| set_storefront_presets_config.set(event_target_value(&ev)) />
                            <p class="text-xs text-muted-foreground">"Presets drive public tabs and default filter scopes for `storefrontSearch`."</p>
                        </label>
                    </div>
                </div>
                <label class="mt-4 block space-y-2">
                    <span class="text-sm font-medium text-card-foreground">"Engine config (JSON)"</span>
                    <textarea class="min-h-[14rem] w-full rounded-lg border border-input bg-background px-3 py-2 font-mono text-sm" prop:value=settings_config on:input=move |ev| set_settings_config.set(event_target_value(&ev)) />
                </label>
                <Show when=move || settings_feedback.get().is_some()>
                    <div class="mt-4 rounded-xl border border-border bg-muted/20 px-4 py-3 text-sm text-muted-foreground">
                        {move || settings_feedback.get().unwrap_or_default()}
                    </div>
                </Show>
                <div class="mt-4 flex justify-end">
                    <button type="button" class="inline-flex items-center justify-center rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || settings_busy.get() on:click=save_settings>
                        {move || if settings_busy.get() { "Saving..." } else { "Save Search Settings" }}
                    </button>
                </div>
            </section>
            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-1">
                    <h2 class="text-lg font-semibold text-card-foreground">"Scoped Rebuild"</h2>
                    <p class="text-sm text-muted-foreground">
                        "Queue tenant-wide or scoped rebuilds. `content` and `product` rebuild the whole slice when target ID is empty, or a single entity when target ID is provided."
                    </p>
                </div>
                <div class="mt-5 grid gap-4 md:grid-cols-[14rem_minmax(0,1fr)_auto]">
                    <label class="block space-y-2">
                        <span class="text-sm font-medium text-card-foreground">"Scope"</span>
                        <select class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=rebuild_target_type on:change=move |ev| set_rebuild_target_type.set(event_target_value(&ev))>
                            <option value="search">"search"</option>
                            <option value="content">"content"</option>
                            <option value="product">"product"</option>
                        </select>
                    </label>
                    <label class="block space-y-2">
                        <span class="text-sm font-medium text-card-foreground">"Target ID (optional)"</span>
                        <input type="text" class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" placeholder="UUID for single node/product rebuild" prop:value=rebuild_target_id on:input=move |ev| set_rebuild_target_id.set(event_target_value(&ev)) />
                    </label>
                    <div class="flex items-end">
                        <button type="button" class="inline-flex items-center justify-center rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || rebuild_busy.get() on:click=queue_rebuild>
                            {move || if rebuild_busy.get() { "Queueing..." } else { "Queue Rebuild" }}
                        </button>
                    </div>
                </div>
                <Show when=move || rebuild_feedback.get().is_some()>
                    <div class="mt-4 rounded-xl border border-border bg-muted/20 px-4 py-3 text-sm text-muted-foreground">
                        {move || rebuild_feedback.get().unwrap_or_default()}
                    </div>
                </Show>
            </section>
        </section>
    }
}

fn playground_view(
    query: ReadSignal<String>,
    set_query: WriteSignal<String>,
    entity_types: ReadSignal<String>,
    set_entity_types: WriteSignal<String>,
    source_modules: ReadSignal<String>,
    set_source_modules: WriteSignal<String>,
    statuses: ReadSignal<String>,
    set_statuses: WriteSignal<String>,
    ranking_profile: ReadSignal<String>,
    set_ranking_profile: WriteSignal<String>,
    preset_key: ReadSignal<String>,
    set_preset_key: WriteSignal<String>,
    filter_presets: Resource<Result<Vec<SearchFilterPresetPayload>, api::ApiError>>,
    preview: ReadSignal<Option<SearchPreviewPayload>>,
    preview_error: ReadSignal<Option<String>>,
    busy: ReadSignal<bool>,
    run_preview: Callback<SubmitEvent>,
) -> impl IntoView {
    view! { <section class="grid gap-6 xl:grid-cols-[minmax(0,22rem)_minmax(0,1fr)]">
        <form class="space-y-4 rounded-2xl border border-border bg-card p-6 shadow-sm" on:submit=move |ev| run_preview.run(ev)>
            <div class="space-y-1"><h2 class="text-lg font-semibold text-card-foreground">"Search Preview"</h2><p class="text-sm text-muted-foreground">"Runs the current PostgreSQL FTS preview path over rustok-search documents."</p></div>
            <label class="block space-y-2"><span class="text-sm font-medium text-card-foreground">"Query"</span><input type="text" class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=query on:input=move |ev| set_query.set(event_target_value(&ev)) /></label>
            <label class="block space-y-2">
                <span class="text-sm font-medium text-card-foreground">"Filter preset"</span>
                <Suspense fallback=move || view! { <div class="h-10 animate-pulse rounded-lg bg-muted"></div> }>
                    {move || filter_presets.get().map(|result| match result {
                        Ok(presets) => view! {
                            <select class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=preset_key on:change=move |ev| set_preset_key.set(event_target_value(&ev))>
                                <option value="">"auto"</option>
                                {presets.into_iter().map(|preset| view! { <option value=preset.key.clone()>{preset.label}</option> }).collect_view()}
                            </select>
                        }.into_any(),
                        Err(err) => view! { <div class="rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">{format!("Failed to load presets: {err}")}</div> }.into_any(),
                    })}
                </Suspense>
            </label>
            <label class="block space-y-2">
                <span class="text-sm font-medium text-card-foreground">"Ranking profile"</span>
                <select class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=ranking_profile on:change=move |ev| set_ranking_profile.set(event_target_value(&ev))>
                    <option value="">"auto"</option>
                    <option value="balanced">"balanced"</option>
                    <option value="exact">"exact"</option>
                    <option value="fresh">"fresh"</option>
                    <option value="catalog">"catalog"</option>
                    <option value="content">"content"</option>
                </select>
            </label>
            <label class="block space-y-2"><span class="text-sm font-medium text-card-foreground">"Entity types (CSV)"</span><input type="text" class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=entity_types on:input=move |ev| set_entity_types.set(event_target_value(&ev)) /></label>
            <label class="block space-y-2"><span class="text-sm font-medium text-card-foreground">"Source modules (CSV)"</span><input type="text" class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=source_modules on:input=move |ev| set_source_modules.set(event_target_value(&ev)) /></label>
            <label class="block space-y-2"><span class="text-sm font-medium text-card-foreground">"Statuses (CSV)"</span><input type="text" class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=statuses on:input=move |ev| set_statuses.set(event_target_value(&ev)) /></label>
            <Show when=move || preview_error.get().is_some()><div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{move || preview_error.get().unwrap_or_default()}</div></Show>
            <button type="submit" class="inline-flex w-full items-center justify-center rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || busy.get()>{move || if busy.get() { "Running..." } else { "Run FTS Preview" }}</button>
        </form>
        <div class="space-y-6">
            <Show when=move || preview.get().is_some() fallback=move || view! { <div class="rounded-2xl border border-dashed border-border bg-card p-10 text-center text-sm text-muted-foreground shadow-sm">"Run a preview query to inspect FTS results, facets, and effective engine output."</div> }>
                {move || preview.get().map(preview_panel)}
            </Show>
        </div>
    </section> }
}

fn analytics_view(
    diagnostics: SearchDiagnosticsPayload,
    lagging_documents: Resource<Result<Vec<LaggingSearchDocumentPayload>, api::ApiError>>,
    consistency_issues: Resource<Result<Vec<SearchConsistencyIssuePayload>, api::ApiError>>,
    search_analytics: Resource<Result<SearchAnalyticsPayload, api::ApiError>>,
) -> impl IntoView {
    view! {
        <section class="space-y-6">
            <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-6 2xl:grid-cols-7">
                <DiagnosticsCard diagnostics=diagnostics.clone() />
                <InfoCard title="Lagging docs" value=diagnostics.stale_documents.to_string() detail="Documents where projection timestamps are behind source updates." />
                <InfoCard title="Missing docs" value=diagnostics.missing_documents.to_string() detail="Expected projection rows that are absent from search storage." />
                <InfoCard title="Orphaned docs" value=diagnostics.orphaned_documents.to_string() detail="Projection rows without a matching content/product source row." />
                <InfoCard title="Max lag" value=format!("{}s", diagnostics.max_lag_seconds) detail="Largest observed lag in seconds." />
                <InfoCard title="Newest indexed" value=diagnostics.newest_indexed_at.unwrap_or_else(|| "not indexed yet".to_string()) detail="Most recent index write in rustok-search storage." />
                <InfoCard title="Oldest indexed" value=diagnostics.oldest_indexed_at.unwrap_or_else(|| "not indexed yet".to_string()) detail="Oldest surviving indexed document timestamp." />
            </div>
            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-1">
                    <h2 class="text-lg font-semibold text-card-foreground">"Search Analytics"</h2>
                    <p class="text-sm text-muted-foreground">"CTR, abandonment, zero-result, slow-query analysis, and query-intelligence candidates over the recent query log window."</p>
                </div>
                <div class="mt-5">
                    <Suspense fallback=move || view! { <div class="h-24 animate-pulse rounded-xl bg-muted"></div> }>
                        {move || search_analytics.get().map(|result| match result {
                            Ok(analytics) => analytics_panel(analytics).into_any(),
                            Err(err) => view! { <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{format!("Failed to load search analytics: {err}")}</div> }.into_any(),
                        })}
                    </Suspense>
                </div>
            </section>
            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-1"><h2 class="text-lg font-semibold text-card-foreground">"Lagging Documents"</h2><p class="text-sm text-muted-foreground">"Raw diagnostics for the most stale documents in search storage."</p></div>
                <div class="mt-5">
                    <Suspense fallback=move || view! { <div class="h-24 animate-pulse rounded-xl bg-muted"></div> }>
                        {move || lagging_documents.get().map(|result| match result {
                            Ok(rows) => lagging_table(rows).into_any(),
                            Err(err) => view! { <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{format!("Failed to load lagging search diagnostics: {err}")}</div> }.into_any(),
                        })}
                    </Suspense>
                </div>
            </section>
            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-1"><h2 class="text-lg font-semibold text-card-foreground">"Consistency Issues"</h2><p class="text-sm text-muted-foreground">"Missing projections and orphaned search documents compared to current content/product source state."</p></div>
                <div class="mt-5">
                    <Suspense fallback=move || view! { <div class="h-24 animate-pulse rounded-xl bg-muted"></div> }>
                        {move || consistency_issues.get().map(|result| match result {
                            Ok(rows) => consistency_table(rows).into_any(),
                            Err(err) => view! { <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{format!("Failed to load search consistency diagnostics: {err}")}</div> }.into_any(),
                        })}
                    </Suspense>
                </div>
            </section>
        </section>
    }
}

fn analytics_panel(analytics: SearchAnalyticsPayload) -> impl IntoView {
    let summary = analytics.summary.clone();
    view! {
        <div class="space-y-6">
            <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-5">
                <InfoCard title="Window" value=format!("{}d", summary.window_days) detail="Rolling analytics lookback window." />
                <InfoCard title="Queries" value=summary.total_queries.to_string() detail="All logged search queries in the current window." />
                <InfoCard title="CTR" value=format!("{:.1}%", summary.click_through_rate * 100.0) detail="Share of eligible successful queries that received at least one click." />
                <InfoCard title="Abandonment" value=format!("{:.1}%", summary.abandonment_rate * 100.0) detail="Eligible successful queries that ended without any tracked click." />
                <InfoCard title="Zero-result rate" value=format!("{:.1}%", summary.zero_result_rate * 100.0) detail="Share of successful queries that returned no results." />
            </div>
            <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-5">
                <InfoCard title="Avg latency" value=format!("{:.1} ms", summary.avg_took_ms) detail="Average PostgreSQL search execution time." />
                <InfoCard title="Slow-query rate" value=format!("{:.1}%", summary.slow_query_rate * 100.0) detail="Share of successful queries at or above the current slow-query threshold." />
                <InfoCard title="Total clicks" value=summary.total_clicks.to_string() detail="All tracked result clicks in the current window." />
                <InfoCard title="Abandoned queries" value=summary.abandonment_queries.to_string() detail="Successful queries older than the click-eval window with no clicks." />
                <InfoCard title="Unique queries" value=summary.unique_queries.to_string() detail="Distinct normalized queries observed in the window." />
            </div>
            <div class="grid gap-6 xl:grid-cols-2">
                <section class="rounded-xl border border-border bg-background p-4">
                    <div class="space-y-1">
                        <h3 class="text-base font-semibold text-card-foreground">"Top Queries"</h3>
                        <p class="text-sm text-muted-foreground">"Most frequent successful queries across admin and storefront search."</p>
                    </div>
                    <div class="mt-4">{analytics_rows_table(analytics.top_queries, "No successful queries recorded yet.")}</div>
                </section>
                <section class="rounded-xl border border-border bg-background p-4">
                    <div class="space-y-1">
                        <h3 class="text-base font-semibold text-card-foreground">"Zero-Result Queries"</h3>
                        <p class="text-sm text-muted-foreground">"Queries that repeatedly return nothing and are likely candidates for synonyms, redirects, or content gaps."</p>
                    </div>
                    <div class="mt-4">{analytics_rows_table(analytics.zero_result_queries, "No zero-result queries recorded in the current window.")}</div>
                </section>
            </div>
            <div class="grid gap-6 xl:grid-cols-2">
                <section class="rounded-xl border border-border bg-background p-4">
                    <div class="space-y-1">
                        <h3 class="text-base font-semibold text-card-foreground">"Slow Queries"</h3>
                        <p class="text-sm text-muted-foreground">"Queries whose average execution time meets or exceeds the current slow-query threshold."</p>
                    </div>
                    <div class="mt-4">{analytics_rows_table(analytics.slow_queries, "No slow queries detected in the current window.")}</div>
                </section>
                <section class="rounded-xl border border-border bg-background p-4">
                    <div class="space-y-1">
                        <h3 class="text-base font-semibold text-card-foreground">"Low CTR Queries"</h3>
                        <p class="text-sm text-muted-foreground">"Frequent queries whose result sets are not attracting clicks."</p>
                    </div>
                    <div class="mt-4">{analytics_rows_table(analytics.low_ctr_queries, "No low-CTR queries detected in the current window.")}</div>
                </section>
                <section class="rounded-xl border border-border bg-background p-4">
                    <div class="space-y-1">
                        <h3 class="text-base font-semibold text-card-foreground">"Abandonment Queries"</h3>
                        <p class="text-sm text-muted-foreground">"Successful queries that tend to end without any click." </p>
                    </div>
                    <div class="mt-4">{analytics_rows_table(analytics.abandonment_queries, "No abandoned high-volume queries detected in the current window.")}</div>
                </section>
            </div>
            <section class="rounded-xl border border-border bg-background p-4">
                <div class="space-y-1">
                    <h3 class="text-base font-semibold text-card-foreground">"Query Intelligence"</h3>
                    <p class="text-sm text-muted-foreground">"Queries that most likely need synonyms, redirects, pinning, or ranking adjustments."</p>
                </div>
                <div class="mt-4">{intelligence_table(analytics.intelligence_candidates)}</div>
            </section>
        </div>
    }
}

fn preview_panel(payload: SearchPreviewPayload) -> impl IntoView {
    view! { <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
        <div><h2 class="text-lg font-semibold text-card-foreground">"Preview Results"</h2><p class="text-sm text-muted-foreground">{format!("{} results in {} ms via {} ({})", payload.total, payload.took_ms, payload.engine, payload.ranking_profile)}</p><p class="mt-2 text-xs text-muted-foreground">{format!("preset = {}", payload.preset_key.unwrap_or_else(|| "none".to_string()))}</p></div>
        <div class="mt-5 grid gap-4 lg:grid-cols-3">{payload.facets.iter().map(|facet| view! { <FacetCard facet=facet.clone() /> }).collect_view()}</div>
        <div class="mt-6 space-y-3">{payload.items.into_iter().enumerate().map(|(index, item)| view! {
            <article class="rounded-xl border border-border bg-background p-4">
                <div class="flex flex-wrap items-center gap-2 text-xs font-medium uppercase tracking-[0.16em] text-muted-foreground"><span>{item.entity_type.clone()}</span><span>"|"</span><span>{item.source_module.clone()}</span><span>"|"</span><span>{format!("score {:.3}", item.score)}</span></div>
                <h3 class="mt-2 text-base font-semibold text-card-foreground">{item.title}</h3>
                <p class="mt-2 text-sm text-muted-foreground">{item.snippet.unwrap_or_else(|| "No snippet returned.".to_string())}</p>
                {preview_result_action(payload.query_log_id.clone(), item.id.clone(), item.url.clone(), index)}
            </article>
        }).collect_view()}</div>
    </section> }
}

fn analytics_rows_table(
    rows: Vec<crate::model::SearchAnalyticsQueryRowPayload>,
    empty_message: &'static str,
) -> impl IntoView {
    if rows.is_empty() {
        return view! { <div class="rounded-xl border border-dashed border-border p-10 text-center text-sm text-muted-foreground">{empty_message}</div> }.into_any();
    }

    view! { <div class="overflow-hidden rounded-xl border border-border"><table class="w-full text-sm">
        <thead class="border-b border-border bg-muted/50"><tr>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Query"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Hits"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Zero hits"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Clicks"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"CTR"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Abandonment"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Avg latency"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Avg results"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Last seen"</th>
        </tr></thead>
        <tbody class="divide-y divide-border">{rows.into_iter().map(|row| view! {
            <tr class="transition-colors hover:bg-muted/30">
                <td class="px-4 py-3 align-top"><div class="font-medium text-card-foreground">{row.query}</div></td>
                <td class="px-4 py-3 align-top text-xs text-muted-foreground">{row.hits}</td>
                <td class="px-4 py-3 align-top text-xs text-muted-foreground">{row.zero_result_hits}</td>
                <td class="px-4 py-3 align-top text-xs text-muted-foreground">{row.clicks}</td>
                <td class="px-4 py-3 align-top text-xs text-muted-foreground">{format!("{:.1}%", row.click_through_rate * 100.0)}</td>
                <td class="px-4 py-3 align-top text-xs text-muted-foreground">{format!("{:.1}%", row.abandonment_rate * 100.0)}</td>
                <td class="px-4 py-3 align-top text-xs text-muted-foreground">{format!("{:.1} ms", row.avg_took_ms)}</td>
                <td class="px-4 py-3 align-top text-xs text-muted-foreground">{format!("{:.1}", row.avg_results)}</td>
                <td class="px-4 py-3 align-top text-xs text-muted-foreground">{row.last_seen_at}</td>
            </tr>
        }).collect_view()}</tbody>
    </table></div> }.into_any()
}

fn intelligence_table(rows: Vec<crate::model::SearchAnalyticsInsightRowPayload>) -> impl IntoView {
    if rows.is_empty() {
        return view! { <div class="rounded-xl border border-dashed border-border p-10 text-center text-sm text-muted-foreground">"No query-intelligence candidates surfaced in the current window."</div> }.into_any();
    }

    view! { <div class="overflow-hidden rounded-xl border border-border"><table class="w-full text-sm">
        <thead class="border-b border-border bg-muted/50"><tr>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Query"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Hits"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Zero hits"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Clicks"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"CTR"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Recommendation"</th>
        </tr></thead>
        <tbody class="divide-y divide-border">{rows.into_iter().map(|row| view! {
            <tr class="transition-colors hover:bg-muted/30">
                <td class="px-4 py-3 align-top"><div class="font-medium text-card-foreground">{row.query}</div></td>
                <td class="px-4 py-3 align-top text-xs text-muted-foreground">{row.hits}</td>
                <td class="px-4 py-3 align-top text-xs text-muted-foreground">{row.zero_result_hits}</td>
                <td class="px-4 py-3 align-top text-xs text-muted-foreground">{row.clicks}</td>
                <td class="px-4 py-3 align-top text-xs text-muted-foreground">{format!("{:.1}%", row.click_through_rate * 100.0)}</td>
                <td class="px-4 py-3 align-top text-xs text-muted-foreground">{row.recommendation}</td>
            </tr>
        }).collect_view()}</tbody>
    </table></div> }.into_any()
}

fn preview_result_action(
    query_log_id: Option<String>,
    document_id: String,
    url: Option<String>,
    index: usize,
) -> impl IntoView {
    let Some(url) = url else {
        return view! { <p class="mt-4 text-xs text-muted-foreground">"No target URL is available for this result yet."</p> }.into_any();
    };

    let token = leptos_auth::hooks::use_token();
    let tenant = leptos_auth::hooks::use_tenant();

    view! {
        <a
            class="mt-4 inline-flex text-sm font-medium text-primary hover:underline"
            href=url.clone()
            on:click=move |ev| {
                let Some(query_log_id) = query_log_id.clone() else {
                    return;
                };
                let Some(window) = web_sys::window() else {
                    return;
                };
                ev.prevent_default();
                let token_value = token.get_untracked();
                let tenant_value = tenant.get_untracked();
                let document_id = document_id.clone();
                let url = url.clone();
                spawn_local(async move {
                    let _ = api::track_search_click(
                        token_value,
                        tenant_value,
                        query_log_id,
                        document_id,
                        Some((index + 1) as i32),
                        Some(url.clone()),
                    )
                    .await;
                    let _ = window.location().set_href(&url);
                });
            }
        >
            "Open result"
        </a>
    }
    .into_any()
}

fn lagging_table(rows: Vec<LaggingSearchDocumentPayload>) -> impl IntoView {
    if rows.is_empty() {
        return view! { <div class="rounded-xl border border-dashed border-border p-12 text-center"><p class="text-sm text-muted-foreground">"No lagging documents detected. Search projection is currently caught up."</p></div> }.into_any();
    }
    view! { <div class="overflow-hidden rounded-xl border border-border"><table class="w-full text-sm">
        <thead class="border-b border-border bg-muted/50"><tr>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Title"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Type"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Locale"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Lag"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Indexed"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Updated"</th>
        </tr></thead>
        <tbody class="divide-y divide-border">{rows.into_iter().map(|row| view! {
            <tr class="transition-colors hover:bg-muted/30">
                <td class="px-4 py-3 align-top"><div class="font-medium text-card-foreground">{row.title}</div><div class="mt-1 text-xs text-muted-foreground">{row.document_key}</div></td>
                <td class="px-4 py-3 align-top text-xs text-muted-foreground">{format!("{}/{} ({})", row.source_module, row.entity_type, row.status)}</td>
                <td class="px-4 py-3 align-top text-xs text-muted-foreground">{row.locale}</td>
                <td class="px-4 py-3 align-top"><span class="inline-flex rounded-full border border-amber-200 bg-amber-50 px-2.5 py-0.5 text-xs font-semibold text-amber-700">{format!("{}s", row.lag_seconds)}</span></td>
                <td class="px-4 py-3 align-top text-xs text-muted-foreground">{row.indexed_at}</td>
                <td class="px-4 py-3 align-top text-xs text-muted-foreground">{row.updated_at}</td>
            </tr>
        }).collect_view()}</tbody>
    </table></div> }.into_any()
}

fn consistency_table(rows: Vec<SearchConsistencyIssuePayload>) -> impl IntoView {
    if rows.is_empty() {
        return view! { <div class="rounded-xl border border-dashed border-border p-12 text-center"><p class="text-sm text-muted-foreground">"No missing or orphaned search documents detected. Projection consistency is healthy."</p></div> }.into_any();
    }
    view! { <div class="overflow-hidden rounded-xl border border-border"><table class="w-full text-sm">
        <thead class="border-b border-border bg-muted/50"><tr>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Issue"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Title"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Type"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Locale"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Updated"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Indexed"</th>
        </tr></thead>
        <tbody class="divide-y divide-border">{rows.into_iter().map(|row| {
            let badge_class = if row.issue_kind == "missing" {
                "border-rose-200 bg-rose-50 text-rose-700"
            } else {
                "border-orange-200 bg-orange-50 text-orange-700"
            };
            view! {
                <tr class="transition-colors hover:bg-muted/30">
                    <td class="px-4 py-3 align-top">
                        <span class=format!("inline-flex rounded-full border px-2.5 py-0.5 text-xs font-semibold {badge_class}")>{row.issue_kind.clone()}</span>
                    </td>
                    <td class="px-4 py-3 align-top"><div class="font-medium text-card-foreground">{row.title}</div><div class="mt-1 text-xs text-muted-foreground">{row.document_key}</div></td>
                    <td class="px-4 py-3 align-top text-xs text-muted-foreground">{format!("{}/{} ({})", row.source_module, row.entity_type, row.status)}</td>
                    <td class="px-4 py-3 align-top text-xs text-muted-foreground">{row.locale}</td>
                    <td class="px-4 py-3 align-top text-xs text-muted-foreground">{row.updated_at}</td>
                    <td class="px-4 py-3 align-top text-xs text-muted-foreground">{row.indexed_at.unwrap_or_else(|| "not indexed".to_string())}</td>
                </tr>
            }
        }).collect_view()}</tbody>
    </table></div> }.into_any()
}

#[component]
fn DictionariesView() -> impl IntoView {
    let token = leptos_auth::hooks::use_token();
    let tenant = leptos_auth::hooks::use_tenant();
    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (feedback, set_feedback) = signal(Option::<String>::None);
    let (busy, set_busy) = signal(false);
    let (synonym_term, set_synonym_term) = signal(String::new());
    let (synonym_values, set_synonym_values) = signal(String::new());
    let (stop_word_value, set_stop_word_value) = signal(String::new());
    let (pin_query_text, set_pin_query_text) = signal(String::new());
    let (pin_document_id, set_pin_document_id) = signal(String::new());
    let (pin_position, set_pin_position) = signal("1".to_string());

    let snapshot = Resource::new(
        move || (token.get(), tenant.get(), refresh_nonce.get()),
        move |(token_value, tenant_value, _)| async move {
            api::fetch_dictionary_snapshot(token_value, tenant_value).await
        },
    );

    let submit_synonym = Callback::new(move |ev: SubmitEvent| {
        ev.prevent_default();
        set_busy.set(true);
        set_feedback.set(None);
        spawn_local({
            let token_value = token.get_untracked();
            let tenant_value = tenant.get_untracked();
            let term = synonym_term.get_untracked();
            let synonyms = parse_csv(synonym_values.get_untracked());
            async move {
                match api::upsert_search_synonym(token_value, tenant_value, term, synonyms).await {
                    Ok(_) => {
                        set_feedback.set(Some("Synonym dictionary updated.".to_string()));
                        set_synonym_term.set(String::new());
                        set_synonym_values.set(String::new());
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                    Err(err) => {
                        set_feedback.set(Some(format!("Failed to save synonym: {err}")));
                    }
                }
                set_busy.set(false);
            }
        });
    });

    let submit_stop_word = Callback::new(move |ev: SubmitEvent| {
        ev.prevent_default();
        set_busy.set(true);
        set_feedback.set(None);
        spawn_local({
            let token_value = token.get_untracked();
            let tenant_value = tenant.get_untracked();
            let value = stop_word_value.get_untracked();
            async move {
                match api::add_search_stop_word(token_value, tenant_value, value).await {
                    Ok(_) => {
                        set_feedback.set(Some("Stop-word dictionary updated.".to_string()));
                        set_stop_word_value.set(String::new());
                        set_refresh_nonce.update(|nonce| *nonce += 1);
                    }
                    Err(err) => {
                        set_feedback.set(Some(format!("Failed to add stop word: {err}")));
                    }
                }
                set_busy.set(false);
            }
        });
    });

    let submit_pin_rule = Callback::new(move |ev: SubmitEvent| {
        ev.prevent_default();
        let pinned_position = match optional_text(pin_position.get_untracked()) {
            Some(value) => match value.parse::<i32>() {
                Ok(parsed) => Some(parsed),
                Err(_) => {
                    set_feedback.set(Some(
                        "Pinned position must be a positive integer.".to_string(),
                    ));
                    return;
                }
            },
            None => Some(1),
        };

        set_busy.set(true);
        set_feedback.set(None);
        spawn_local({
            let token_value = token.get_untracked();
            let tenant_value = tenant.get_untracked();
            let query_text = pin_query_text.get_untracked();
            let document_id = pin_document_id.get_untracked();
            async move {
                match api::upsert_search_pin_rule(
                    token_value,
                    tenant_value,
                    query_text,
                    document_id,
                    pinned_position,
                )
                .await
                {
                    Ok(_) => {
                        set_feedback.set(Some("Pinned result rule updated.".to_string()));
                        set_pin_query_text.set(String::new());
                        set_pin_document_id.set(String::new());
                        set_pin_position.set("1".to_string());
                        set_refresh_nonce.update(|nonce| *nonce += 1);
                    }
                    Err(err) => {
                        set_feedback.set(Some(format!("Failed to save pinned result rule: {err}")));
                    }
                }
                set_busy.set(false);
            }
        });
    });

    let delete_synonym = Callback::new(move |synonym_id: String| {
        set_busy.set(true);
        set_feedback.set(None);
        spawn_local({
            let token_value = token.get_untracked();
            let tenant_value = tenant.get_untracked();
            async move {
                match api::delete_search_synonym(token_value, tenant_value, synonym_id).await {
                    Ok(_) => {
                        set_feedback.set(Some("Synonym removed.".to_string()));
                        set_refresh_nonce.update(|nonce| *nonce += 1);
                    }
                    Err(err) => {
                        set_feedback.set(Some(format!("Failed to remove synonym: {err}")));
                    }
                }
                set_busy.set(false);
            }
        });
    });

    let delete_stop_word = Callback::new(move |stop_word_id: String| {
        set_busy.set(true);
        set_feedback.set(None);
        spawn_local({
            let token_value = token.get_untracked();
            let tenant_value = tenant.get_untracked();
            async move {
                match api::delete_search_stop_word(token_value, tenant_value, stop_word_id).await {
                    Ok(_) => {
                        set_feedback.set(Some("Stop word removed.".to_string()));
                        set_refresh_nonce.update(|nonce| *nonce += 1);
                    }
                    Err(err) => {
                        set_feedback.set(Some(format!("Failed to remove stop word: {err}")));
                    }
                }
                set_busy.set(false);
            }
        });
    });

    let delete_query_rule = Callback::new(move |query_rule_id: String| {
        set_busy.set(true);
        set_feedback.set(None);
        spawn_local({
            let token_value = token.get_untracked();
            let tenant_value = tenant.get_untracked();
            async move {
                match api::delete_search_query_rule(token_value, tenant_value, query_rule_id).await
                {
                    Ok(_) => {
                        set_feedback.set(Some("Pinned rule removed.".to_string()));
                        set_refresh_nonce.update(|nonce| *nonce += 1);
                    }
                    Err(err) => {
                        set_feedback.set(Some(format!("Failed to remove pinned rule: {err}")));
                    }
                }
                set_busy.set(false);
            }
        });
    });

    view! {
        <section class="space-y-6">
            <div class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-1">
                    <h2 class="text-lg font-semibold text-card-foreground">"Search Dictionaries"</h2>
                    <p class="text-sm text-muted-foreground">
                        "Tenant-owned stop words, synonyms, and exact-query pin rules. These dictionaries apply to both admin preview and storefront search on the shared backend contract."
                    </p>
                </div>
                <Show when=move || feedback.get().is_some()>
                    <div class="mt-4 rounded-xl border border-border bg-muted/20 px-4 py-3 text-sm text-muted-foreground">
                        {move || feedback.get().unwrap_or_default()}
                    </div>
                </Show>
            </div>

            <div class="grid gap-6 xl:grid-cols-3">
                <form class="space-y-4 rounded-2xl border border-border bg-card p-6 shadow-sm" on:submit=move |ev| submit_synonym.run(ev)>
                    <div class="space-y-1">
                        <h3 class="text-base font-semibold text-card-foreground">"Synonyms"</h3>
                        <p class="text-sm text-muted-foreground">"Expand exact tokens into equivalent search terms."</p>
                    </div>
                    <label class="block space-y-2">
                        <span class="text-sm font-medium text-card-foreground">"Canonical term"</span>
                        <input type="text" class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=synonym_term on:input=move |ev| set_synonym_term.set(event_target_value(&ev)) />
                    </label>
                    <label class="block space-y-2">
                        <span class="text-sm font-medium text-card-foreground">"Synonyms (CSV)"</span>
                        <input type="text" class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=synonym_values on:input=move |ev| set_synonym_values.set(event_target_value(&ev)) />
                    </label>
                    <button type="submit" class="inline-flex w-full items-center justify-center rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || busy.get()>
                        {move || if busy.get() { "Saving..." } else { "Save Synonym Group" }}
                    </button>
                </form>

                <form class="space-y-4 rounded-2xl border border-border bg-card p-6 shadow-sm" on:submit=move |ev| submit_stop_word.run(ev)>
                    <div class="space-y-1">
                        <h3 class="text-base font-semibold text-card-foreground">"Stop Words"</h3>
                        <p class="text-sm text-muted-foreground">"Remove low-signal tokens before FTS execution."</p>
                    </div>
                    <label class="block space-y-2">
                        <span class="text-sm font-medium text-card-foreground">"Stop word"</span>
                        <input type="text" class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=stop_word_value on:input=move |ev| set_stop_word_value.set(event_target_value(&ev)) />
                    </label>
                    <button type="submit" class="inline-flex w-full items-center justify-center rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || busy.get()>
                        {move || if busy.get() { "Saving..." } else { "Add Stop Word" }}
                    </button>
                </form>

                <form class="space-y-4 rounded-2xl border border-border bg-card p-6 shadow-sm" on:submit=move |ev| submit_pin_rule.run(ev)>
                    <div class="space-y-1">
                        <h3 class="text-base font-semibold text-card-foreground">"Pinned Results"</h3>
                        <p class="text-sm text-muted-foreground">"Pin an existing search document for an exact normalized query."</p>
                    </div>
                    <label class="block space-y-2">
                        <span class="text-sm font-medium text-card-foreground">"Query text"</span>
                        <input type="text" class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=pin_query_text on:input=move |ev| set_pin_query_text.set(event_target_value(&ev)) />
                    </label>
                    <label class="block space-y-2">
                        <span class="text-sm font-medium text-card-foreground">"Document ID"</span>
                        <input type="text" class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=pin_document_id on:input=move |ev| set_pin_document_id.set(event_target_value(&ev)) />
                    </label>
                    <label class="block space-y-2">
                        <span class="text-sm font-medium text-card-foreground">"Pinned position"</span>
                        <input type="number" min="1" class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=pin_position on:input=move |ev| set_pin_position.set(event_target_value(&ev)) />
                    </label>
                    <button type="submit" class="inline-flex w-full items-center justify-center rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || busy.get()>
                        {move || if busy.get() { "Saving..." } else { "Save Pin Rule" }}
                    </button>
                </form>
            </div>

            <Suspense fallback=move || view! { <div class="h-32 animate-pulse rounded-2xl bg-muted"></div> }>
                {move || snapshot.get().map(|result| match result {
                    Ok(snapshot) => dictionaries_tables(snapshot, busy, delete_synonym, delete_stop_word, delete_query_rule).into_any(),
                    Err(err) => view! {
                        <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                            {format!("Failed to load search dictionaries: {err}")}
                        </div>
                    }.into_any(),
                })}
            </Suspense>
        </section>
    }
}

fn dictionaries_tables(
    snapshot: SearchDictionarySnapshotPayload,
    busy: ReadSignal<bool>,
    delete_synonym: Callback<String>,
    delete_stop_word: Callback<String>,
    delete_query_rule: Callback<String>,
) -> impl IntoView {
    view! {
        <div class="space-y-6">
            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-1">
                    <h3 class="text-base font-semibold text-card-foreground">"Synonym Groups"</h3>
                    <p class="text-sm text-muted-foreground">"Each group expands all included terms as equivalent tokens."</p>
                </div>
                <div class="mt-5">{synonyms_table(snapshot.synonyms, busy, delete_synonym)}</div>
            </section>
            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-1">
                    <h3 class="text-base font-semibold text-card-foreground">"Stop Words"</h3>
                    <p class="text-sm text-muted-foreground">"Terms removed from the effective FTS query."</p>
                </div>
                <div class="mt-5">{stop_words_table(snapshot.stop_words, busy, delete_stop_word)}</div>
            </section>
            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-1">
                    <h3 class="text-base font-semibold text-card-foreground">"Pinned Query Rules"</h3>
                    <p class="text-sm text-muted-foreground">"Exact normalized queries that promote specific documents to chosen positions."</p>
                </div>
                <div class="mt-5">{query_rules_table(snapshot.query_rules, busy, delete_query_rule)}</div>
            </section>
        </div>
    }
}

fn synonyms_table(
    rows: Vec<SearchSynonymPayload>,
    busy: ReadSignal<bool>,
    delete_synonym: Callback<String>,
) -> impl IntoView {
    if rows.is_empty() {
        return view! { <div class="rounded-xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">"No synonym groups configured yet."</div> }.into_any();
    }

    view! { <div class="overflow-hidden rounded-xl border border-border"><table class="w-full text-sm">
        <thead class="border-b border-border bg-muted/50"><tr>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Term"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Synonyms"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Updated"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Actions"</th>
        </tr></thead>
        <tbody class="divide-y divide-border">{rows.into_iter().map(|row| {
            let synonym_id = row.id.clone();
            view! {
                <tr class="transition-colors hover:bg-muted/30">
                    <td class="px-4 py-3 align-top"><div class="font-medium text-card-foreground">{row.term}</div></td>
                    <td class="px-4 py-3 align-top text-xs text-muted-foreground">{row.synonyms.join(", ")}</td>
                    <td class="px-4 py-3 align-top text-xs text-muted-foreground">{row.updated_at}</td>
                    <td class="px-4 py-3 align-top">
                        <button type="button" class="inline-flex rounded-lg border border-border px-3 py-1 text-xs font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| delete_synonym.run(synonym_id.clone())>"Delete"</button>
                    </td>
                </tr>
            }
        }).collect_view()}</tbody>
    </table></div> }.into_any()
}

fn stop_words_table(
    rows: Vec<SearchStopWordPayload>,
    busy: ReadSignal<bool>,
    delete_stop_word: Callback<String>,
) -> impl IntoView {
    if rows.is_empty() {
        return view! { <div class="rounded-xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">"No stop words configured yet."</div> }.into_any();
    }

    view! { <div class="overflow-hidden rounded-xl border border-border"><table class="w-full text-sm">
        <thead class="border-b border-border bg-muted/50"><tr>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Value"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Updated"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Actions"</th>
        </tr></thead>
        <tbody class="divide-y divide-border">{rows.into_iter().map(|row| {
            let stop_word_id = row.id.clone();
            view! {
                <tr class="transition-colors hover:bg-muted/30">
                    <td class="px-4 py-3 align-top"><div class="font-medium text-card-foreground">{row.value}</div></td>
                    <td class="px-4 py-3 align-top text-xs text-muted-foreground">{row.updated_at}</td>
                    <td class="px-4 py-3 align-top">
                        <button type="button" class="inline-flex rounded-lg border border-border px-3 py-1 text-xs font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| delete_stop_word.run(stop_word_id.clone())>"Delete"</button>
                    </td>
                </tr>
            }
        }).collect_view()}</tbody>
    </table></div> }.into_any()
}

fn query_rules_table(
    rows: Vec<SearchQueryRulePayload>,
    busy: ReadSignal<bool>,
    delete_query_rule: Callback<String>,
) -> impl IntoView {
    if rows.is_empty() {
        return view! { <div class="rounded-xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">"No pinned query rules configured yet."</div> }.into_any();
    }

    view! { <div class="overflow-hidden rounded-xl border border-border"><table class="w-full text-sm">
        <thead class="border-b border-border bg-muted/50"><tr>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Query"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Target"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Position"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Updated"</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">"Actions"</th>
        </tr></thead>
        <tbody class="divide-y divide-border">{rows.into_iter().map(|row| {
            let query_rule_id = row.id.clone();
            view! {
                <tr class="transition-colors hover:bg-muted/30">
                    <td class="px-4 py-3 align-top">
                        <div class="font-medium text-card-foreground">{row.query_text}</div>
                        <div class="mt-1 text-xs text-muted-foreground">{row.query_normalized}</div>
                    </td>
                    <td class="px-4 py-3 align-top">
                        <div class="font-medium text-card-foreground">{row.title}</div>
                        <div class="mt-1 text-xs text-muted-foreground">{format!("{} / {} / {}", row.document_id, row.source_module, row.entity_type)}</div>
                    </td>
                    <td class="px-4 py-3 align-top text-xs text-muted-foreground">{row.pinned_position}</td>
                    <td class="px-4 py-3 align-top text-xs text-muted-foreground">{row.updated_at}</td>
                    <td class="px-4 py-3 align-top">
                        <button type="button" class="inline-flex rounded-lg border border-border px-3 py-1 text-xs font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| delete_query_rule.run(query_rule_id.clone())>"Delete"</button>
                    </td>
                </tr>
            }
        }).collect_view()}</tbody>
    </table></div> }.into_any()
}

#[component]
fn DiagnosticsCard(diagnostics: SearchDiagnosticsPayload) -> impl IntoView {
    let badge_class = match diagnostics.state.as_str() {
        "healthy" => "border-emerald-200 bg-emerald-50 text-emerald-700",
        "inconsistent" => "border-rose-200 bg-rose-50 text-rose-700",
        "lagging" => "border-amber-200 bg-amber-50 text-amber-700",
        _ => "border-slate-200 bg-slate-50 text-slate-700",
    };
    view! { <article class="rounded-2xl border border-border bg-card p-5 shadow-sm">
        <div class="text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">"Index state"</div>
        <div class="mt-3"><span class=format!("inline-flex rounded-full border px-3 py-1 text-xs font-semibold {badge_class}")>{diagnostics.state}</span></div>
        <p class="mt-3 text-sm text-muted-foreground">{format!("Newest indexed: {}", diagnostics.newest_indexed_at.unwrap_or_else(|| "not indexed yet".to_string()))}</p>
    </article> }
}

#[component]
fn InfoCard<T, U>(title: T, value: U, detail: &'static str) -> impl IntoView
where
    T: IntoView + 'static,
    U: IntoView + 'static,
{
    view! { <article class="rounded-2xl border border-border bg-card p-5 shadow-sm"><div class="text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">{title}</div><div class="mt-2 text-lg font-semibold text-card-foreground">{value}</div><p class="mt-2 text-sm text-muted-foreground">{detail}</p></article> }
}

#[component]
fn FacetCard(facet: SearchFacetGroup) -> impl IntoView {
    view! { <article class="rounded-xl border border-border bg-background p-4"><div class="text-sm font-semibold capitalize text-card-foreground">{facet.name.replace('_', " ")}</div><div class="mt-3 flex flex-wrap gap-2">{facet.buckets.into_iter().map(|bucket| view! { <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">{format!("{} ({})", bucket.value, bucket.count)}</span> }).collect_view()}</div></article> }
}

fn parse_csv(value: String) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn optional_text(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn pretty_json_string(value: &str) -> String {
    parse_json_for_editor(value)
        .and_then(|json| serde_json::to_string_pretty(&json).ok())
        .unwrap_or_else(|| value.to_string())
}

fn parse_json_for_editor(value: &str) -> Option<serde_json::Value> {
    serde_json::from_str(value).ok()
}

fn extract_ranking_profile_value(config: &serde_json::Value, surface: &str) -> String {
    config
        .get("ranking_profiles")
        .and_then(|value| value.get(surface))
        .and_then(serde_json::Value::as_str)
        .unwrap_or(match surface {
            "admin_global_search" => "exact",
            _ => "balanced",
        })
        .to_string()
}

fn extract_surface_presets_json(config: &serde_json::Value, surface: &str) -> String {
    config
        .get("filter_presets")
        .and_then(|value| value.get(surface))
        .and_then(|value| serde_json::to_string_pretty(value).ok())
        .unwrap_or_else(|| "[]".to_string())
}

fn merge_relevance_editor_config(
    config_text: &str,
    ranking_default: &str,
    ranking_preview: &str,
    ranking_storefront: &str,
    ranking_admin_global: &str,
    preview_presets: &str,
    storefront_presets: &str,
) -> Result<String, String> {
    let mut config = parse_json_for_editor(config_text)
        .ok_or_else(|| "Settings config must be valid JSON.".to_string())?;
    let object = config
        .as_object_mut()
        .ok_or_else(|| "Settings config root must be a JSON object.".to_string())?;

    object.insert(
        "ranking_profiles".to_string(),
        serde_json::json!({
            "default": ranking_default,
            "search_preview": ranking_preview,
            "storefront_search": ranking_storefront,
            "admin_global_search": ranking_admin_global,
        }),
    );
    object.insert(
        "filter_presets".to_string(),
        serde_json::json!({
            "search_preview": parse_json_array_for_editor("Preview filter presets", preview_presets)?,
            "storefront_search": parse_json_array_for_editor("Storefront filter presets", storefront_presets)?,
        }),
    );

    serde_json::to_string_pretty(&config)
        .map_err(|err| format!("Failed to serialize merged search settings config: {err}"))
}

fn parse_json_array_for_editor(label: &str, value: &str) -> Result<serde_json::Value, String> {
    let parsed: serde_json::Value =
        serde_json::from_str(value).map_err(|err| format!("{label} must be valid JSON: {err}"))?;
    if !parsed.is_array() {
        return Err(format!("{label} must be a JSON array."));
    }
    Ok(parsed)
}

fn tab_class(active: bool) -> &'static str {
    if active {
        "inline-flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90"
    } else {
        "inline-flex items-center gap-2 rounded-lg border border-border px-4 py-2 text-sm font-medium text-foreground transition hover:bg-accent hover:text-accent-foreground"
    }
}
