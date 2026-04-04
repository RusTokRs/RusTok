mod api;
mod i18n;
mod model;

use leptos::ev::{MouseEvent, SubmitEvent};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::web_sys;
use leptos_router::components::A;
use rustok_api::UiRouteContext;

use crate::i18n::t;
use crate::model::{
    LaggingSearchDocumentPayload, SearchAdminBootstrap, SearchAnalyticsPayload,
    SearchConsistencyIssuePayload, SearchDiagnosticsPayload, SearchDictionarySnapshotPayload,
    SearchFacetGroup, SearchFilterPresetPayload, SearchPreviewFilters, SearchPreviewPayload,
    SearchQueryRulePayload, SearchStopWordPayload, SearchSynonymPayload,
};

#[derive(Clone)]
struct SearchPreviewLabels {
    title: String,
    summary_template: String,
    preset_template: String,
    none_label: String,
    score_template: String,
    no_snippet: String,
    no_target_url: String,
    open_result: String,
}

#[component]
pub fn SearchAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let route_segment = route_context
        .route_segment
        .clone()
        .unwrap_or_else(|| "search".to_string());
    let initial_query = route_context.query_value("q").unwrap_or("").to_string();
    let initial_locale = route_context.locale.clone();
    let on_playground = route_context.subpath_matches("playground");
    let on_diagnostics = route_context.subpath_matches("analytics");
    let on_dictionaries = route_context.subpath_matches("dictionaries");
    let badge_label = t(ui_locale.as_deref(), "search.badge", "search");
    let title_label = t(ui_locale.as_deref(), "search.title", "Search Control Plane");
    let subtitle_label = t(
        ui_locale.as_deref(),
        "search.subtitle",
        "Module-owned admin surface for search diagnostics, scoped rebuilds, and PostgreSQL FTS inspection.",
    );
    let overview_label = t(ui_locale.as_deref(), "search.tabs.overview", "Overview");
    let playground_label = t(ui_locale.as_deref(), "search.tabs.playground", "Playground");
    let analytics_label = t(ui_locale.as_deref(), "search.tabs.analytics", "Analytics");
    let dictionaries_label = t(
        ui_locale.as_deref(),
        "search.tabs.dictionaries",
        "Dictionaries",
    );
    let preview_error_label = t(
        ui_locale.as_deref(),
        "search.error.preview",
        "Failed to run search preview",
    );
    let queue_rebuild_error_label = t(
        ui_locale.as_deref(),
        "search.error.queueRebuild",
        "Failed to queue search rebuild",
    );
    let rebuild_queued_template = t(
        ui_locale.as_deref(),
        "search.feedback.rebuildQueued",
        "Rebuild queued for {scope} scope{suffix}.",
    );
    let invalid_settings_json_label = t(
        ui_locale.as_deref(),
        "search.error.invalidSettingsJson",
        "Settings config must be valid JSON.",
    );
    let settings_saved_label = t(
        ui_locale.as_deref(),
        "search.feedback.settingsSaved",
        "Search settings saved.",
    );
    let save_settings_error_label = t(
        ui_locale.as_deref(),
        "search.error.saveSettings",
        "Failed to save search settings",
    );
    let load_bootstrap_error = t(
        ui_locale.as_deref(),
        "search.error.loadBootstrap",
        "Failed to load search bootstrap data",
    );

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
            let preview_error_label = preview_error_label.clone();
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
                        set_preview_error.set(Some(format!("{}: {err}", preview_error_label)))
                    }
                }
                set_busy.set(false);
            }
        });
    });

    let queue_rebuild = Callback::new(move |_| {
        set_rebuild_busy.set(true);
        set_rebuild_feedback.set(None);
        spawn_local({
            let token_value = token.get_untracked();
            let tenant_value = tenant.get_untracked();
            let target_type = rebuild_target_type.get_untracked();
            let target_id = optional_text(rebuild_target_id.get_untracked());
            let rebuild_queued_template = rebuild_queued_template.clone();
            let queue_rebuild_error_label = queue_rebuild_error_label.clone();
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
                        set_rebuild_feedback.set(Some(
                            rebuild_queued_template
                                .replace("{scope}", result.target_type.as_str())
                                .replace("{suffix}", suffix.as_str()),
                        ));
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                    Err(err) => set_rebuild_feedback
                        .set(Some(format!("{}: {err}", queue_rebuild_error_label))),
                }
                set_rebuild_busy.set(false);
            }
        });
    });

    view! {
        <div class="space-y-6">
            <header class="flex flex-col gap-4 rounded-2xl border border-border bg-card p-6 shadow-sm lg:flex-row lg:items-start lg:justify-between">
                <div class="space-y-2">
                    <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">{badge_label}</span>
                    <h1 class="text-2xl font-semibold text-card-foreground">{title_label}</h1>
                    <p class="max-w-3xl text-sm text-muted-foreground">
                        {subtitle_label}
                    </p>
                </div>
                <div class="flex flex-wrap gap-2">
                    <A href=format!("/modules/{route_segment}") attr:class=tab_class(!on_playground && !on_diagnostics && !on_dictionaries)>{overview_label.clone()}</A>
                    <A href=format!("/modules/{route_segment}/playground") attr:class=tab_class(on_playground)>{playground_label.clone()}</A>
                    <A href=format!("/modules/{route_segment}/analytics") attr:class=tab_class(on_diagnostics)>{analytics_label.clone()}</A>
                    <A href=format!("/modules/{route_segment}/dictionaries") attr:class=tab_class(on_dictionaries)>{dictionaries_label.clone()}</A>
                </div>
            </header>

            <Suspense fallback=move || view! { <div class="h-32 animate-pulse rounded-2xl bg-muted"></div> }>
                {move || {
                    bootstrap.get().map(|result| match result {
                        Ok(bootstrap) => {
                            if on_playground {
                                playground_view(
                                    ui_locale.clone(),
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
                                    ui_locale.clone(),
                                    bootstrap.search_diagnostics,
                                    lagging_documents,
                                    consistency_issues,
                                    search_analytics,
                                )
                                .into_any()
                            } else if on_dictionaries {
                                view! { <DictionariesView /> }.into_any()
                            } else {
                                let invalid_settings_json_label =
                                    invalid_settings_json_label.clone();
                                let settings_saved_label = settings_saved_label.clone();
                                let save_settings_error_label =
                                    save_settings_error_label.clone();
                                let settings_locale = ui_locale.clone();
                                let save_settings = Callback::new(move |_| {
                                    let invalid_settings_json_label =
                                        invalid_settings_json_label.clone();
                                    let settings_saved_label = settings_saved_label.clone();
                                    let save_settings_error_label =
                                        save_settings_error_label.clone();
                                    let config = settings_config.get_untracked();
                                    let merged_config = match merge_relevance_editor_config(
                                        settings_locale.as_deref(),
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
                                        set_settings_feedback
                                            .set(Some(invalid_settings_json_label.clone()));
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
                                        let settings_saved_label =
                                            settings_saved_label.clone();
                                        let save_settings_error_label =
                                            save_settings_error_label.clone();
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
                                                    set_settings_feedback
                                                        .set(Some(settings_saved_label));
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
                                                    format!("{}: {err}", save_settings_error_label),
                                                )),
                                            }
                                            set_settings_busy.set(false);
                                        }
                                    });
                                });
                                overview_view(
                                    ui_locale.clone(),
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
                                    save_settings,
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
                                {format!("{}: {err}", load_bootstrap_error)}
                            </div>
                        }.into_any(),
                    })
                }}
            </Suspense>
        </div>
    }
}

fn overview_view(
    ui_locale: Option<String>,
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
    save_settings: Callback<MouseEvent>,
    rebuild_target_type: ReadSignal<String>,
    set_rebuild_target_type: WriteSignal<String>,
    rebuild_target_id: ReadSignal<String>,
    set_rebuild_target_id: WriteSignal<String>,
    rebuild_busy: ReadSignal<bool>,
    rebuild_feedback: ReadSignal<Option<String>>,
    queue_rebuild: Callback<MouseEvent>,
) -> impl IntoView {
    let locale = ui_locale.as_deref();
    let profile_balanced = t(locale, "search.profile.balanced", "balanced");
    let profile_exact = t(locale, "search.profile.exact", "exact");
    let profile_fresh = t(locale, "search.profile.fresh", "fresh");
    let profile_catalog = t(locale, "search.profile.catalog", "catalog");
    let profile_content = t(locale, "search.profile.content", "content");
    let saving_label = t(locale, "search.action.saving", "Saving...");
    let save_settings_label = t(locale, "search.action.saveSettings", "Save Search Settings");
    let queueing_label = t(locale, "search.action.queueing", "Queueing...");
    let queue_rebuild_label = t(locale, "search.action.queueRebuild", "Queue Rebuild");
    view! {
        <section class="space-y-6">
            <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
                <InfoCard title=t(locale, "search.overview.activeEngine.title", "Active engine") value=bootstrap.search_settings_preview.active_engine detail=t(locale, "search.overview.activeEngine.detail", "Effective search settings for the current tenant.") />
                <InfoCard title=t(locale, "search.overview.fallbackEngine.title", "Fallback engine") value=bootstrap.search_settings_preview.fallback_engine detail=t(locale, "search.overview.fallbackEngine.detail", "Used when an external engine is configured but unavailable.") />
                <InfoCard title=t(locale, "search.overview.availableEngines.title", "Available engines") value=bootstrap.available_search_engines.len().to_string() detail=t(locale, "search.overview.availableEngines.detail", "Only connectors installed in the runtime appear here.") />
                <InfoCard title=t(locale, "search.overview.updatedAt.title", "Updated at") value=bootstrap.search_settings_preview.updated_at detail=t(locale, "search.overview.updatedAt.detail", "Timestamp of the effective settings record.") />
            </div>
            <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-6 2xl:grid-cols-7">
                <DiagnosticsCard diagnostics=bootstrap.search_diagnostics.clone() ui_locale=ui_locale.clone() />
                <InfoCard title=t(locale, "search.overview.documents.title", "Documents") value=bootstrap.search_diagnostics.total_documents.to_string() detail=t(locale, "search.overview.documents.detail", "Total search documents in rustok-search storage.") />
                <InfoCard title=t(locale, "search.overview.publicDocs.title", "Public docs") value=bootstrap.search_diagnostics.public_documents.to_string() detail=t(locale, "search.overview.publicDocs.detail", "Published documents visible to storefront search.") />
                <InfoCard title=t(locale, "search.overview.staleDocs.title", "Stale docs") value=bootstrap.search_diagnostics.stale_documents.to_string() detail=t(locale, "search.overview.staleDocs.detail", "Documents where indexed_at lags behind source updated_at.") />
                <InfoCard title=t(locale, "search.overview.missingDocs.title", "Missing docs") value=bootstrap.search_diagnostics.missing_documents.to_string() detail=t(locale, "search.overview.missingDocs.detail", "Source rows that should exist in search_documents but do not.") />
                <InfoCard title=t(locale, "search.overview.orphanedDocs.title", "Orphaned docs") value=bootstrap.search_diagnostics.orphaned_documents.to_string() detail=t(locale, "search.overview.orphanedDocs.detail", "Search documents that no longer have a matching source row.") />
                <InfoCard title=t(locale, "search.overview.maxLag.title", "Max lag") value=format!("{}s", bootstrap.search_diagnostics.max_lag_seconds) detail=t(locale, "search.overview.maxLag.detail", "Worst-case lag between source update and search projection.") />
            </div>
            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-1">
                    <h2 class="text-lg font-semibold text-card-foreground">{t(locale, "search.settings.title", "Engine Settings")}</h2>
                    <p class="text-sm text-muted-foreground">
                        {t(locale, "search.settings.subtitle", "Save the effective search engine selection and JSON config for the current tenant. Only engines installed in the runtime appear here.")}
                    </p>
                </div>
                <div class="mt-5 grid gap-4 md:grid-cols-2">
                    <label class="block space-y-2">
                        <span class="text-sm font-medium text-card-foreground">{t(locale, "search.settings.activeEngine", "Active engine")}</span>
                        <select class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=settings_active_engine on:change=move |ev| set_settings_active_engine.set(event_target_value(&ev))>
                            {bootstrap.available_search_engines.iter().map(|engine| view! {
                                <option value=engine.kind.clone()>{format!("{} ({})", engine.label, engine.kind)}</option>
                            }).collect_view()}
                        </select>
                    </label>
                    <label class="block space-y-2">
                        <span class="text-sm font-medium text-card-foreground">{t(locale, "search.settings.fallbackEngine", "Fallback engine")}</span>
                        <select class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=settings_fallback_engine on:change=move |ev| set_settings_fallback_engine.set(event_target_value(&ev))>
                            {bootstrap.available_search_engines.iter().map(|engine| view! {
                                <option value=engine.kind.clone()>{format!("{} ({})", engine.label, engine.kind)}</option>
                            }).collect_view()}
                        </select>
                    </label>
                </div>
                <div class="mt-6 rounded-xl border border-border bg-muted/20 p-4">
                    <div class="space-y-1">
                        <h3 class="text-base font-semibold text-card-foreground">{t(locale, "search.relevance.title", "Relevance Settings")}</h3>
                        <p class="text-sm text-muted-foreground">
                            {t(locale, "search.relevance.subtitle", "Structured editor for ranking defaults and filter presets. These values are merged back into `search_settings.config` on save.")}
                        </p>
                    </div>
                    <div class="mt-4 grid gap-4 xl:grid-cols-2">
                        <label class="block space-y-2">
                            <span class="text-sm font-medium text-card-foreground">{t(locale, "search.relevance.defaultProfile", "Default ranking profile")}</span>
                            <select class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=ranking_default_profile on:change=move |ev| set_ranking_default_profile.set(event_target_value(&ev))>
                                <option value="balanced">{profile_balanced.clone()}</option>
                                <option value="exact">{profile_exact.clone()}</option>
                                <option value="fresh">{profile_fresh.clone()}</option>
                                <option value="catalog">{profile_catalog.clone()}</option>
                                <option value="content">{profile_content.clone()}</option>
                            </select>
                        </label>
                        <label class="block space-y-2">
                            <span class="text-sm font-medium text-card-foreground">{t(locale, "search.relevance.previewProfile", "Preview ranking profile")}</span>
                            <select class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=ranking_preview_profile on:change=move |ev| set_ranking_preview_profile.set(event_target_value(&ev))>
                                <option value="balanced">{profile_balanced.clone()}</option>
                                <option value="exact">{profile_exact.clone()}</option>
                                <option value="fresh">{profile_fresh.clone()}</option>
                                <option value="catalog">{profile_catalog.clone()}</option>
                                <option value="content">{profile_content.clone()}</option>
                            </select>
                        </label>
                        <label class="block space-y-2">
                            <span class="text-sm font-medium text-card-foreground">{t(locale, "search.relevance.storefrontProfile", "Storefront ranking profile")}</span>
                            <select class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=ranking_storefront_profile on:change=move |ev| set_ranking_storefront_profile.set(event_target_value(&ev))>
                                <option value="balanced">{profile_balanced.clone()}</option>
                                <option value="exact">{profile_exact.clone()}</option>
                                <option value="fresh">{profile_fresh.clone()}</option>
                                <option value="catalog">{profile_catalog.clone()}</option>
                                <option value="content">{profile_content.clone()}</option>
                            </select>
                        </label>
                        <label class="block space-y-2">
                            <span class="text-sm font-medium text-card-foreground">{t(locale, "search.relevance.adminGlobalProfile", "Admin global ranking profile")}</span>
                            <select class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=ranking_admin_global_profile on:change=move |ev| set_ranking_admin_global_profile.set(event_target_value(&ev))>
                                <option value="balanced">{profile_balanced.clone()}</option>
                                <option value="exact">{profile_exact.clone()}</option>
                                <option value="fresh">{profile_fresh.clone()}</option>
                                <option value="catalog">{profile_catalog.clone()}</option>
                                <option value="content">{profile_content.clone()}</option>
                            </select>
                        </label>
                    </div>
                    <div class="mt-4 grid gap-4 xl:grid-cols-2">
                        <label class="block space-y-2">
                            <span class="text-sm font-medium text-card-foreground">{t(locale, "search.relevance.previewPresets", "Preview filter presets (JSON array)")}</span>
                            <textarea class="min-h-[12rem] w-full rounded-lg border border-input bg-background px-3 py-2 font-mono text-sm" prop:value=preview_presets_config on:input=move |ev| set_preview_presets_config.set(event_target_value(&ev)) />
                            <p class="text-xs text-muted-foreground">{t(locale, "search.relevance.previewPresetsHint", "Each item supports: key, label, entity_types, source_modules, statuses, ranking_profile.")}</p>
                        </label>
                        <label class="block space-y-2">
                            <span class="text-sm font-medium text-card-foreground">{t(locale, "search.relevance.storefrontPresets", "Storefront filter presets (JSON array)")}</span>
                            <textarea class="min-h-[12rem] w-full rounded-lg border border-input bg-background px-3 py-2 font-mono text-sm" prop:value=storefront_presets_config on:input=move |ev| set_storefront_presets_config.set(event_target_value(&ev)) />
                            <p class="text-xs text-muted-foreground">{t(locale, "search.relevance.storefrontPresetsHint", "Presets drive public tabs and default filter scopes for `storefrontSearch`.")}</p>
                        </label>
                    </div>
                </div>
                <label class="mt-4 block space-y-2">
                    <span class="text-sm font-medium text-card-foreground">{t(locale, "search.settings.engineConfig", "Engine config (JSON)")}</span>
                    <textarea class="min-h-[14rem] w-full rounded-lg border border-input bg-background px-3 py-2 font-mono text-sm" prop:value=settings_config on:input=move |ev| set_settings_config.set(event_target_value(&ev)) />
                </label>
                <Show when=move || settings_feedback.get().is_some()>
                    <div class="mt-4 rounded-xl border border-border bg-muted/20 px-4 py-3 text-sm text-muted-foreground">
                        {move || settings_feedback.get().unwrap_or_default()}
                    </div>
                </Show>
                <div class="mt-4 flex justify-end">
                    <button type="button" class="inline-flex items-center justify-center rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || settings_busy.get() on:click=move |ev| save_settings.run(ev)>
                        {move || if settings_busy.get() { saving_label.clone() } else { save_settings_label.clone() }}
                    </button>
                </div>
            </section>
            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-1">
                    <h2 class="text-lg font-semibold text-card-foreground">{t(locale, "search.rebuild.title", "Scoped Rebuild")}</h2>
                    <p class="text-sm text-muted-foreground">
                        {t(locale, "search.rebuild.subtitle", "Queue tenant-wide or scoped rebuilds. `content` and `product` rebuild the whole slice when target ID is empty, or a single entity when target ID is provided.")}
                    </p>
                </div>
                <div class="mt-5 grid gap-4 md:grid-cols-[14rem_minmax(0,1fr)_auto]">
                    <label class="block space-y-2">
                        <span class="text-sm font-medium text-card-foreground">{t(locale, "search.rebuild.scope", "Scope")}</span>
                        <select class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=rebuild_target_type on:change=move |ev| set_rebuild_target_type.set(event_target_value(&ev))>
                            <option value="search">{t(locale, "search.scope.search", "search")}</option>
                            <option value="content">{t(locale, "search.scope.content", "content")}</option>
                            <option value="product">{t(locale, "search.scope.product", "product")}</option>
                        </select>
                    </label>
                    <label class="block space-y-2">
                        <span class="text-sm font-medium text-card-foreground">{t(locale, "search.rebuild.targetId", "Target ID (optional)")}</span>
                        <input type="text" class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" placeholder=t(locale, "search.rebuild.targetIdPlaceholder", "UUID for single node/product rebuild") prop:value=rebuild_target_id on:input=move |ev| set_rebuild_target_id.set(event_target_value(&ev)) />
                    </label>
                    <div class="flex items-end">
                        <button type="button" class="inline-flex items-center justify-center rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || rebuild_busy.get() on:click=move |ev| queue_rebuild.run(ev)>
                            {move || if rebuild_busy.get() { queueing_label.clone() } else { queue_rebuild_label.clone() }}
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
    ui_locale: Option<String>,
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
    let locale = ui_locale.clone();
    let locale_ref = locale.as_deref();
    let profile_balanced = t(locale_ref, "search.profile.balanced", "balanced");
    let profile_exact = t(locale_ref, "search.profile.exact", "exact");
    let profile_fresh = t(locale_ref, "search.profile.fresh", "fresh");
    let profile_catalog = t(locale_ref, "search.profile.catalog", "catalog");
    let profile_content = t(locale_ref, "search.profile.content", "content");
    let auto_label = t(locale_ref, "search.common.auto", "auto");
    let running_label = t(locale_ref, "search.action.running", "Running...");
    let run_preview_label = t(locale_ref, "search.action.runPreview", "Run FTS Preview");
    let title_label = t(locale_ref, "search.playground.title", "Search Preview");
    let subtitle_label = t(
        locale_ref,
        "search.playground.subtitle",
        "Runs the current PostgreSQL FTS preview path over rustok-search documents.",
    );
    let query_label = t(locale_ref, "search.playground.query", "Query");
    let filter_preset_label = t(
        locale_ref,
        "search.playground.filterPreset",
        "Filter preset",
    );
    let load_presets_error_label = t(
        locale_ref,
        "search.error.loadPresets",
        "Failed to load presets",
    );
    let ranking_profile_label = t(
        locale_ref,
        "search.playground.rankingProfile",
        "Ranking profile",
    );
    let entity_types_label = t(
        locale_ref,
        "search.playground.entityTypes",
        "Entity types (CSV)",
    );
    let source_modules_label = t(
        locale_ref,
        "search.playground.sourceModules",
        "Source modules (CSV)",
    );
    let statuses_label = t(locale_ref, "search.playground.statuses", "Statuses (CSV)");
    let empty_label = t(
        locale_ref,
        "search.playground.empty",
        "Run a preview query to inspect FTS results, facets, and effective engine output.",
    );
    let auto_label_for_presets = auto_label.clone();
    let auto_label_for_ranking = auto_label.clone();
    let preview_labels = StoredValue::new(SearchPreviewLabels {
        title: t(locale_ref, "search.preview.title", "Preview Results"),
        summary_template: t(
            locale_ref,
            "search.preview.summary",
            "{total} results in {took_ms} ms via {engine} ({ranking_profile})",
        ),
        preset_template: t(locale_ref, "search.preview.preset", "preset = {preset}"),
        none_label: t(locale_ref, "search.common.none", "none"),
        score_template: t(locale_ref, "search.preview.score", "score {score:.3}"),
        no_snippet: t(
            locale_ref,
            "search.preview.noSnippet",
            "No snippet returned.",
        ),
        no_target_url: t(
            locale_ref,
            "search.preview.noTargetUrl",
            "No target URL is available for this result yet.",
        ),
        open_result: t(locale_ref, "search.preview.openResult", "Open result"),
    });
    view! { <section class="grid gap-6 xl:grid-cols-[minmax(0,22rem)_minmax(0,1fr)]">
        <form class="space-y-4 rounded-2xl border border-border bg-card p-6 shadow-sm" on:submit=move |ev| run_preview.run(ev)>
            <div class="space-y-1"><h2 class="text-lg font-semibold text-card-foreground">{title_label}</h2><p class="text-sm text-muted-foreground">{subtitle_label}</p></div>
            <label class="block space-y-2"><span class="text-sm font-medium text-card-foreground">{query_label}</span><input type="text" class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=query on:input=move |ev| set_query.set(event_target_value(&ev)) /></label>
            <label class="block space-y-2">
                <span class="text-sm font-medium text-card-foreground">{filter_preset_label}</span>
                <Suspense fallback=move || view! { <div class="h-10 animate-pulse rounded-lg bg-muted"></div> }>
                    {move || filter_presets.get().map(|result| match result {
                        Ok(presets) => view! {
                            <select class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=preset_key on:change=move |ev| set_preset_key.set(event_target_value(&ev))>
                                <option value="">{auto_label_for_presets.clone()}</option>
                                {presets.into_iter().map(|preset| view! { <option value=preset.key.clone()>{preset.label}</option> }).collect_view()}
                            </select>
                        }.into_any(),
                        Err(err) => view! { <div class="rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">{format!("{}: {err}", load_presets_error_label.clone())}</div> }.into_any(),
                    })}
                </Suspense>
            </label>
            <label class="block space-y-2">
                <span class="text-sm font-medium text-card-foreground">{ranking_profile_label}</span>
                <select class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=ranking_profile on:change=move |ev| set_ranking_profile.set(event_target_value(&ev))>
                    <option value="">{auto_label_for_ranking}</option>
                    <option value="balanced">{profile_balanced}</option>
                    <option value="exact">{profile_exact}</option>
                    <option value="fresh">{profile_fresh}</option>
                    <option value="catalog">{profile_catalog}</option>
                    <option value="content">{profile_content}</option>
                </select>
            </label>
            <label class="block space-y-2"><span class="text-sm font-medium text-card-foreground">{entity_types_label}</span><input type="text" class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=entity_types on:input=move |ev| set_entity_types.set(event_target_value(&ev)) /></label>
            <label class="block space-y-2"><span class="text-sm font-medium text-card-foreground">{source_modules_label}</span><input type="text" class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=source_modules on:input=move |ev| set_source_modules.set(event_target_value(&ev)) /></label>
            <label class="block space-y-2"><span class="text-sm font-medium text-card-foreground">{statuses_label}</span><input type="text" class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=statuses on:input=move |ev| set_statuses.set(event_target_value(&ev)) /></label>
            <Show when=move || preview_error.get().is_some()><div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{move || preview_error.get().unwrap_or_default()}</div></Show>
            <button type="submit" class="inline-flex w-full items-center justify-center rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || busy.get()>{move || if busy.get() { running_label.clone() } else { run_preview_label.clone() }}</button>
        </form>
        <div class="space-y-6">
            <Show when=move || preview.get().is_some() fallback=move || view! { <div class="rounded-2xl border border-dashed border-border bg-card p-10 text-center text-sm text-muted-foreground shadow-sm">{empty_label.clone()}</div> }>
                {move || {
                    let labels = preview_labels.get_value();
                    preview
                        .get()
                        .map(|payload| preview_panel(payload, labels))
                }}
            </Show>
        </div>
    </section> }
}

fn analytics_view(
    ui_locale: Option<String>,
    diagnostics: SearchDiagnosticsPayload,
    lagging_documents: Resource<Result<Vec<LaggingSearchDocumentPayload>, api::ApiError>>,
    consistency_issues: Resource<Result<Vec<SearchConsistencyIssuePayload>, api::ApiError>>,
    search_analytics: Resource<Result<SearchAnalyticsPayload, api::ApiError>>,
) -> impl IntoView {
    let locale = ui_locale.clone();
    let locale_ref = locale.as_deref();
    let analytics_title = t(locale_ref, "search.analytics.title", "Search Analytics");
    let analytics_subtitle = t(locale_ref, "search.analytics.subtitle", "CTR, abandonment, zero-result, slow-query analysis, and query-intelligence candidates over the recent query log window.");
    let load_analytics_error = t(
        locale_ref,
        "search.error.loadAnalytics",
        "Failed to load search analytics",
    );
    let lagging_title = t(
        locale_ref,
        "search.analytics.laggingTitle",
        "Lagging Documents",
    );
    let lagging_subtitle = t(
        locale_ref,
        "search.analytics.laggingSubtitle",
        "Raw diagnostics for the most stale documents in search storage.",
    );
    let load_lagging_error = t(
        locale_ref,
        "search.error.loadLagging",
        "Failed to load lagging search diagnostics",
    );
    let consistency_title = t(
        locale_ref,
        "search.analytics.consistencyTitle",
        "Consistency Issues",
    );
    let consistency_subtitle = t(locale_ref, "search.analytics.consistencySubtitle", "Missing projections and orphaned search documents compared to current content/product source state.");
    let load_consistency_error = t(
        locale_ref,
        "search.error.loadConsistency",
        "Failed to load search consistency diagnostics",
    );
    let diagnostics_locale = ui_locale.clone();
    let analytics_panel_locale = ui_locale.clone();
    let lagging_table_locale = ui_locale.clone();
    let consistency_table_locale = ui_locale.clone();
    view! {
        <section class="space-y-6">
            <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-6 2xl:grid-cols-7">
                <DiagnosticsCard diagnostics=diagnostics.clone() ui_locale=diagnostics_locale />
                <InfoCard title=t(locale_ref, "search.analytics.laggingDocs.title", "Lagging docs") value=diagnostics.stale_documents.to_string() detail=t(locale_ref, "search.analytics.laggingDocs.detail", "Documents where projection timestamps are behind source updates.") />
                <InfoCard title=t(locale_ref, "search.analytics.missingDocs.title", "Missing docs") value=diagnostics.missing_documents.to_string() detail=t(locale_ref, "search.analytics.missingDocs.detail", "Expected projection rows that are absent from search storage.") />
                <InfoCard title=t(locale_ref, "search.analytics.orphanedDocs.title", "Orphaned docs") value=diagnostics.orphaned_documents.to_string() detail=t(locale_ref, "search.analytics.orphanedDocs.detail", "Projection rows without a matching content/product source row.") />
                <InfoCard title=t(locale_ref, "search.analytics.maxLag.title", "Max lag") value=format!("{}s", diagnostics.max_lag_seconds) detail=t(locale_ref, "search.analytics.maxLag.detail", "Largest observed lag in seconds.") />
                <InfoCard title=t(locale_ref, "search.analytics.newestIndexed.title", "Newest indexed") value=diagnostics.newest_indexed_at.unwrap_or_else(|| t(locale_ref, "search.common.notIndexedYet", "not indexed yet")) detail=t(locale_ref, "search.analytics.newestIndexed.detail", "Most recent index write in rustok-search storage.") />
                <InfoCard title=t(locale_ref, "search.analytics.oldestIndexed.title", "Oldest indexed") value=diagnostics.oldest_indexed_at.unwrap_or_else(|| t(locale_ref, "search.common.notIndexedYet", "not indexed yet")) detail=t(locale_ref, "search.analytics.oldestIndexed.detail", "Oldest surviving indexed document timestamp.") />
            </div>
            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-1">
                    <h2 class="text-lg font-semibold text-card-foreground">{analytics_title}</h2>
                    <p class="text-sm text-muted-foreground">{analytics_subtitle}</p>
                </div>
                <div class="mt-5">
                    <Suspense fallback=move || view! { <div class="h-24 animate-pulse rounded-xl bg-muted"></div> }>
                        {move || search_analytics.get().map(|result| match result {
                            Ok(analytics) => analytics_panel(analytics, analytics_panel_locale.clone()).into_any(),
                            Err(err) => view! { <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{format!("{}: {err}", load_analytics_error.clone())}</div> }.into_any(),
                        })}
                    </Suspense>
                </div>
            </section>
            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-1"><h2 class="text-lg font-semibold text-card-foreground">{lagging_title}</h2><p class="text-sm text-muted-foreground">{lagging_subtitle}</p></div>
                <div class="mt-5">
                    <Suspense fallback=move || view! { <div class="h-24 animate-pulse rounded-xl bg-muted"></div> }>
                        {move || lagging_documents.get().map(|result| match result {
                            Ok(rows) => lagging_table(rows, lagging_table_locale.clone()).into_any(),
                            Err(err) => view! { <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{format!("{}: {err}", load_lagging_error.clone())}</div> }.into_any(),
                        })}
                    </Suspense>
                </div>
            </section>
            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-1"><h2 class="text-lg font-semibold text-card-foreground">{consistency_title}</h2><p class="text-sm text-muted-foreground">{consistency_subtitle}</p></div>
                <div class="mt-5">
                    <Suspense fallback=move || view! { <div class="h-24 animate-pulse rounded-xl bg-muted"></div> }>
                        {move || consistency_issues.get().map(|result| match result {
                            Ok(rows) => consistency_table(rows, consistency_table_locale.clone()).into_any(),
                            Err(err) => view! { <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">{format!("{}: {err}", load_consistency_error.clone())}</div> }.into_any(),
                        })}
                    </Suspense>
                </div>
            </section>
        </section>
    }
}

fn analytics_panel(analytics: SearchAnalyticsPayload, ui_locale: Option<String>) -> impl IntoView {
    let locale = ui_locale.as_deref();
    let summary = analytics.summary.clone();
    view! {
        <div class="space-y-6">
            <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-5">
                <InfoCard title=t(locale, "search.analytics.summary.window.title", "Window") value=format!("{}d", summary.window_days) detail=t(locale, "search.analytics.summary.window.detail", "Rolling analytics lookback window.") />
                <InfoCard title=t(locale, "search.analytics.summary.queries.title", "Queries") value=summary.total_queries.to_string() detail=t(locale, "search.analytics.summary.queries.detail", "All logged search queries in the current window.") />
                <InfoCard title=t(locale, "search.analytics.summary.ctr.title", "CTR") value=format!("{:.1}%", summary.click_through_rate * 100.0) detail=t(locale, "search.analytics.summary.ctr.detail", "Share of eligible successful queries that received at least one click.") />
                <InfoCard title=t(locale, "search.analytics.summary.abandonment.title", "Abandonment") value=format!("{:.1}%", summary.abandonment_rate * 100.0) detail=t(locale, "search.analytics.summary.abandonment.detail", "Eligible successful queries that ended without any tracked click.") />
                <InfoCard title=t(locale, "search.analytics.summary.zeroResultRate.title", "Zero-result rate") value=format!("{:.1}%", summary.zero_result_rate * 100.0) detail=t(locale, "search.analytics.summary.zeroResultRate.detail", "Share of successful queries that returned no results.") />
            </div>
            <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-5">
                <InfoCard title=t(locale, "search.analytics.summary.avgLatency.title", "Avg latency") value=format!("{:.1} ms", summary.avg_took_ms) detail=t(locale, "search.analytics.summary.avgLatency.detail", "Average PostgreSQL search execution time.") />
                <InfoCard title=t(locale, "search.analytics.summary.slowQueryRate.title", "Slow-query rate") value=format!("{:.1}%", summary.slow_query_rate * 100.0) detail=t(locale, "search.analytics.summary.slowQueryRate.detail", "Share of successful queries at or above the current slow-query threshold.") />
                <InfoCard title=t(locale, "search.analytics.summary.totalClicks.title", "Total clicks") value=summary.total_clicks.to_string() detail=t(locale, "search.analytics.summary.totalClicks.detail", "All tracked result clicks in the current window.") />
                <InfoCard title=t(locale, "search.analytics.summary.abandonedQueries.title", "Abandoned queries") value=summary.abandonment_queries.to_string() detail=t(locale, "search.analytics.summary.abandonedQueries.detail", "Successful queries older than the click-eval window with no clicks.") />
                <InfoCard title=t(locale, "search.analytics.summary.uniqueQueries.title", "Unique queries") value=summary.unique_queries.to_string() detail=t(locale, "search.analytics.summary.uniqueQueries.detail", "Distinct normalized queries observed in the window.") />
            </div>
            <div class="grid gap-6 xl:grid-cols-2">
                <section class="rounded-xl border border-border bg-background p-4">
                    <div class="space-y-1">
                        <h3 class="text-base font-semibold text-card-foreground">{t(locale, "search.analytics.topQueries.title", "Top Queries")}</h3>
                        <p class="text-sm text-muted-foreground">{t(locale, "search.analytics.topQueries.subtitle", "Most frequent successful queries across admin and storefront search.")}</p>
                    </div>
                    <div class="mt-4">{analytics_rows_table(analytics.top_queries, t(locale, "search.analytics.topQueries.empty", "No successful queries recorded yet."), ui_locale.clone())}</div>
                </section>
                <section class="rounded-xl border border-border bg-background p-4">
                    <div class="space-y-1">
                        <h3 class="text-base font-semibold text-card-foreground">{t(locale, "search.analytics.zeroResultQueries.title", "Zero-Result Queries")}</h3>
                        <p class="text-sm text-muted-foreground">{t(locale, "search.analytics.zeroResultQueries.subtitle", "Queries that repeatedly return nothing and are likely candidates for synonyms, redirects, or content gaps.")}</p>
                    </div>
                    <div class="mt-4">{analytics_rows_table(analytics.zero_result_queries, t(locale, "search.analytics.zeroResultQueries.empty", "No zero-result queries recorded in the current window."), ui_locale.clone())}</div>
                </section>
            </div>
            <div class="grid gap-6 xl:grid-cols-2">
                <section class="rounded-xl border border-border bg-background p-4">
                    <div class="space-y-1">
                        <h3 class="text-base font-semibold text-card-foreground">{t(locale, "search.analytics.slowQueries.title", "Slow Queries")}</h3>
                        <p class="text-sm text-muted-foreground">{t(locale, "search.analytics.slowQueries.subtitle", "Queries whose average execution time meets or exceeds the current slow-query threshold.")}</p>
                    </div>
                    <div class="mt-4">{analytics_rows_table(analytics.slow_queries, t(locale, "search.analytics.slowQueries.empty", "No slow queries detected in the current window."), ui_locale.clone())}</div>
                </section>
                <section class="rounded-xl border border-border bg-background p-4">
                    <div class="space-y-1">
                        <h3 class="text-base font-semibold text-card-foreground">{t(locale, "search.analytics.lowCtrQueries.title", "Low CTR Queries")}</h3>
                        <p class="text-sm text-muted-foreground">{t(locale, "search.analytics.lowCtrQueries.subtitle", "Frequent queries whose result sets are not attracting clicks.")}</p>
                    </div>
                    <div class="mt-4">{analytics_rows_table(analytics.low_ctr_queries, t(locale, "search.analytics.lowCtrQueries.empty", "No low-CTR queries detected in the current window."), ui_locale.clone())}</div>
                </section>
                <section class="rounded-xl border border-border bg-background p-4">
                    <div class="space-y-1">
                        <h3 class="text-base font-semibold text-card-foreground">{t(locale, "search.analytics.abandonmentQueries.title", "Abandonment Queries")}</h3>
                        <p class="text-sm text-muted-foreground">{t(locale, "search.analytics.abandonmentQueries.subtitle", "Successful queries that tend to end without any click.")} </p>
                    </div>
                    <div class="mt-4">{analytics_rows_table(analytics.abandonment_queries, t(locale, "search.analytics.abandonmentQueries.empty", "No abandoned high-volume queries detected in the current window."), ui_locale.clone())}</div>
                </section>
            </div>
            <section class="rounded-xl border border-border bg-background p-4">
                <div class="space-y-1">
                    <h3 class="text-base font-semibold text-card-foreground">{t(locale, "search.analytics.intelligence.title", "Query Intelligence")}</h3>
                    <p class="text-sm text-muted-foreground">{t(locale, "search.analytics.intelligence.subtitle", "Queries that most likely need synonyms, redirects, pinning, or ranking adjustments.")}</p>
                </div>
                <div class="mt-4">{intelligence_table(analytics.intelligence_candidates, ui_locale.clone())}</div>
            </section>
        </div>
    }
}

fn preview_panel(payload: SearchPreviewPayload, labels: SearchPreviewLabels) -> impl IntoView {
    let preview_summary = labels
        .summary_template
        .replace("{total}", payload.total.to_string().as_str())
        .replace("{took_ms}", payload.took_ms.to_string().as_str())
        .replace("{engine}", payload.engine.as_str())
        .replace("{ranking_profile}", payload.ranking_profile.as_str());
    let preview_preset = labels.preset_template.replace(
        "{preset}",
        payload
            .preset_key
            .as_deref()
            .unwrap_or(labels.none_label.as_str()),
    );
    view! { <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
        <div><h2 class="text-lg font-semibold text-card-foreground">{labels.title.clone()}</h2><p class="text-sm text-muted-foreground">{preview_summary}</p><p class="mt-2 text-xs text-muted-foreground">{preview_preset}</p></div>
        <div class="mt-5 grid gap-4 lg:grid-cols-3">{payload.facets.iter().map(|facet| view! { <FacetCard facet=facet.clone() /> }).collect_view()}</div>
        <div class="mt-6 space-y-3">{payload.items.into_iter().enumerate().map(|(index, item)| view! {
            <article class="rounded-xl border border-border bg-background p-4">
                <div class="flex flex-wrap items-center gap-2 text-xs font-medium uppercase tracking-[0.16em] text-muted-foreground"><span>{item.entity_type.clone()}</span><span>"|"</span><span>{item.source_module.clone()}</span><span>"|"</span><span>{labels.score_template.clone().replace("{score:.3}", format!("{:.3}", item.score).as_str())}</span></div>
                <h3 class="mt-2 text-base font-semibold text-card-foreground">{item.title}</h3>
                <p class="mt-2 text-sm text-muted-foreground">{item.snippet.unwrap_or_else(|| labels.no_snippet.clone())}</p>
                {preview_result_action(payload.query_log_id.clone(), item.id.clone(), item.url.clone(), index, labels.clone())}
            </article>
        }).collect_view()}</div>
    </section> }
}

fn analytics_rows_table(
    rows: Vec<crate::model::SearchAnalyticsQueryRowPayload>,
    empty_message: String,
    ui_locale: Option<String>,
) -> impl IntoView {
    let locale = ui_locale.as_deref();
    if rows.is_empty() {
        return view! { <div class="rounded-xl border border-dashed border-border p-10 text-center text-sm text-muted-foreground">{empty_message}</div> }.into_any();
    }

    view! { <div class="overflow-hidden rounded-xl border border-border"><table class="w-full text-sm">
        <thead class="border-b border-border bg-muted/50"><tr>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.query", "Query")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.hits", "Hits")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.zeroHits", "Zero hits")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.clicks", "Clicks")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.ctr", "CTR")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.abandonment", "Abandonment")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.avgLatency", "Avg latency")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.avgResults", "Avg results")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.lastSeen", "Last seen")}</th>
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

fn intelligence_table(
    rows: Vec<crate::model::SearchAnalyticsInsightRowPayload>,
    ui_locale: Option<String>,
) -> impl IntoView {
    let locale = ui_locale.as_deref();
    if rows.is_empty() {
        return view! { <div class="rounded-xl border border-dashed border-border p-10 text-center text-sm text-muted-foreground">{t(locale, "search.analytics.intelligence.empty", "No query-intelligence candidates surfaced in the current window.")}</div> }.into_any();
    }

    view! { <div class="overflow-hidden rounded-xl border border-border"><table class="w-full text-sm">
        <thead class="border-b border-border bg-muted/50"><tr>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.query", "Query")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.hits", "Hits")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.zeroHits", "Zero hits")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.clicks", "Clicks")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.ctr", "CTR")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.recommendation", "Recommendation")}</th>
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
    labels: SearchPreviewLabels,
) -> impl IntoView {
    let Some(url) = url else {
        return view! { <p class="mt-4 text-xs text-muted-foreground">{labels.no_target_url}</p> }
            .into_any();
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
            {labels.open_result}
        </a>
    }
    .into_any()
}

fn lagging_table(
    rows: Vec<LaggingSearchDocumentPayload>,
    ui_locale: Option<String>,
) -> impl IntoView {
    let locale = ui_locale.as_deref();
    if rows.is_empty() {
        return view! { <div class="rounded-xl border border-dashed border-border p-12 text-center"><p class="text-sm text-muted-foreground">{t(locale, "search.analytics.lagging.empty", "No lagging documents detected. Search projection is currently caught up.")}</p></div> }.into_any();
    }
    view! { <div class="overflow-hidden rounded-xl border border-border"><table class="w-full text-sm">
        <thead class="border-b border-border bg-muted/50"><tr>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.title", "Title")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.type", "Type")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.locale", "Locale")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.lag", "Lag")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.indexed", "Indexed")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.updated", "Updated")}</th>
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

fn consistency_table(
    rows: Vec<SearchConsistencyIssuePayload>,
    ui_locale: Option<String>,
) -> impl IntoView {
    let locale = ui_locale.as_deref();
    if rows.is_empty() {
        return view! { <div class="rounded-xl border border-dashed border-border p-12 text-center"><p class="text-sm text-muted-foreground">{t(locale, "search.analytics.consistency.empty", "No missing or orphaned search documents detected. Projection consistency is healthy.")}</p></div> }.into_any();
    }
    view! { <div class="overflow-hidden rounded-xl border border-border"><table class="w-full text-sm">
        <thead class="border-b border-border bg-muted/50"><tr>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.issue", "Issue")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.title", "Title")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.type", "Type")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.locale", "Locale")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.updated", "Updated")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.indexed", "Indexed")}</th>
        </tr></thead>
        <tbody class="divide-y divide-border">{rows.into_iter().map(|row| {
            let badge_class = if row.issue_kind == "missing" {
                "border-rose-200 bg-rose-50 text-rose-700"
            } else {
                "border-orange-200 bg-orange-50 text-orange-700"
            };
            let issue_label = if row.issue_kind == "missing" {
                t(locale, "search.issue.missing", "missing")
            } else {
                t(locale, "search.issue.orphaned", "orphaned")
            };
            view! {
                <tr class="transition-colors hover:bg-muted/30">
                    <td class="px-4 py-3 align-top">
                        <span class=format!("inline-flex rounded-full border px-2.5 py-0.5 text-xs font-semibold {badge_class}")>{issue_label}</span>
                    </td>
                    <td class="px-4 py-3 align-top"><div class="font-medium text-card-foreground">{row.title}</div><div class="mt-1 text-xs text-muted-foreground">{row.document_key}</div></td>
                    <td class="px-4 py-3 align-top text-xs text-muted-foreground">{format!("{}/{} ({})", row.source_module, row.entity_type, row.status)}</td>
                    <td class="px-4 py-3 align-top text-xs text-muted-foreground">{row.locale}</td>
                    <td class="px-4 py-3 align-top text-xs text-muted-foreground">{row.updated_at}</td>
                    <td class="px-4 py-3 align-top text-xs text-muted-foreground">{row.indexed_at.unwrap_or_else(|| t(locale, "search.common.notIndexed", "not indexed"))}</td>
                </tr>
            }
        }).collect_view()}</tbody>
    </table></div> }.into_any()
}

#[component]
fn DictionariesView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let ui_locale = route_context.locale.clone();
    let locale = ui_locale.as_deref();
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
    let saving_label = t(locale, "search.action.saving", "Saving...");
    let saving_label_for_synonyms = saving_label.clone();
    let saving_label_for_stop_words = saving_label.clone();
    let saving_label_for_pin_rules = saving_label.clone();
    let save_synonym_group_label = t(
        locale,
        "search.dictionary.action.saveSynonymGroup",
        "Save Synonym Group",
    );
    let add_stop_word_label = t(
        locale,
        "search.dictionary.action.addStopWord",
        "Add Stop Word",
    );
    let save_pin_rule_label = t(locale, "search.dictionary.action.savePinRule", "Save Pin Rule");
    let synonym_updated_label = t(
        locale,
        "search.dictionary.feedback.synonymUpdated",
        "Synonym dictionary updated.",
    );
    let synonym_save_error_label = t(
        locale,
        "search.dictionary.error.saveSynonym",
        "Failed to save synonym",
    );
    let stop_word_updated_label = t(
        locale,
        "search.dictionary.feedback.stopWordUpdated",
        "Stop-word dictionary updated.",
    );
    let stop_word_save_error_label = t(
        locale,
        "search.dictionary.error.addStopWord",
        "Failed to add stop word",
    );
    let pin_position_error_label = t(
        locale,
        "search.dictionary.error.invalidPinnedPosition",
        "Pinned position must be a positive integer.",
    );
    let pin_rule_updated_label = t(
        locale,
        "search.dictionary.feedback.pinRuleUpdated",
        "Pinned result rule updated.",
    );
    let pin_rule_save_error_label = t(
        locale,
        "search.dictionary.error.savePinRule",
        "Failed to save pinned result rule",
    );
    let synonym_removed_label = t(
        locale,
        "search.dictionary.feedback.synonymRemoved",
        "Synonym removed.",
    );
    let synonym_remove_error_label = t(
        locale,
        "search.dictionary.error.removeSynonym",
        "Failed to remove synonym",
    );
    let stop_word_removed_label = t(
        locale,
        "search.dictionary.feedback.stopWordRemoved",
        "Stop word removed.",
    );
    let stop_word_remove_error_label = t(
        locale,
        "search.dictionary.error.removeStopWord",
        "Failed to remove stop word",
    );
    let pin_rule_removed_label = t(
        locale,
        "search.dictionary.feedback.pinRuleRemoved",
        "Pinned rule removed.",
    );
    let pin_rule_remove_error_label = t(
        locale,
        "search.dictionary.error.removePinRule",
        "Failed to remove pinned rule",
    );
    let load_dictionaries_error_label = t(
        locale,
        "search.error.loadDictionaries",
        "Failed to load search dictionaries",
    );

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
            let synonym_updated_label = synonym_updated_label.clone();
            let synonym_save_error_label = synonym_save_error_label.clone();
            async move {
                match api::upsert_search_synonym(token_value, tenant_value, term, synonyms).await {
                    Ok(_) => {
                        set_feedback.set(Some(synonym_updated_label));
                        set_synonym_term.set(String::new());
                        set_synonym_values.set(String::new());
                        set_refresh_nonce.update(|value| *value += 1);
                    }
                    Err(err) => {
                        set_feedback.set(Some(format!("{}: {err}", synonym_save_error_label)));
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
            let stop_word_updated_label = stop_word_updated_label.clone();
            let stop_word_save_error_label = stop_word_save_error_label.clone();
            async move {
                match api::add_search_stop_word(token_value, tenant_value, value).await {
                    Ok(_) => {
                        set_feedback.set(Some(stop_word_updated_label));
                        set_stop_word_value.set(String::new());
                        set_refresh_nonce.update(|nonce| *nonce += 1);
                    }
                    Err(err) => {
                        set_feedback.set(Some(format!("{}: {err}", stop_word_save_error_label)));
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
                    set_feedback.set(Some(pin_position_error_label.clone()));
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
            let pin_rule_updated_label = pin_rule_updated_label.clone();
            let pin_rule_save_error_label = pin_rule_save_error_label.clone();
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
                        set_feedback.set(Some(pin_rule_updated_label));
                        set_pin_query_text.set(String::new());
                        set_pin_document_id.set(String::new());
                        set_pin_position.set("1".to_string());
                        set_refresh_nonce.update(|nonce| *nonce += 1);
                    }
                    Err(err) => {
                        set_feedback.set(Some(format!("{}: {err}", pin_rule_save_error_label)));
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
            let synonym_removed_label = synonym_removed_label.clone();
            let synonym_remove_error_label = synonym_remove_error_label.clone();
            async move {
                match api::delete_search_synonym(token_value, tenant_value, synonym_id).await {
                    Ok(_) => {
                        set_feedback.set(Some(synonym_removed_label));
                        set_refresh_nonce.update(|nonce| *nonce += 1);
                    }
                    Err(err) => {
                        set_feedback.set(Some(format!("{}: {err}", synonym_remove_error_label)));
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
            let stop_word_removed_label = stop_word_removed_label.clone();
            let stop_word_remove_error_label = stop_word_remove_error_label.clone();
            async move {
                match api::delete_search_stop_word(token_value, tenant_value, stop_word_id).await {
                    Ok(_) => {
                        set_feedback.set(Some(stop_word_removed_label));
                        set_refresh_nonce.update(|nonce| *nonce += 1);
                    }
                    Err(err) => {
                        set_feedback.set(Some(format!("{}: {err}", stop_word_remove_error_label)));
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
            let pin_rule_removed_label = pin_rule_removed_label.clone();
            let pin_rule_remove_error_label = pin_rule_remove_error_label.clone();
            async move {
                match api::delete_search_query_rule(token_value, tenant_value, query_rule_id).await
                {
                    Ok(_) => {
                        set_feedback.set(Some(pin_rule_removed_label));
                        set_refresh_nonce.update(|nonce| *nonce += 1);
                    }
                    Err(err) => {
                        set_feedback.set(Some(format!("{}: {err}", pin_rule_remove_error_label)));
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
                    <h2 class="text-lg font-semibold text-card-foreground">{t(locale, "search.dictionary.title", "Search Dictionaries")}</h2>
                    <p class="text-sm text-muted-foreground">
                        {t(locale, "search.dictionary.subtitle", "Tenant-owned stop words, synonyms, and exact-query pin rules. These dictionaries apply to both admin preview and storefront search on the shared backend contract.")}
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
                        <h3 class="text-base font-semibold text-card-foreground">{t(locale, "search.dictionary.synonyms.title", "Synonyms")}</h3>
                        <p class="text-sm text-muted-foreground">{t(locale, "search.dictionary.synonyms.subtitle", "Expand exact tokens into equivalent search terms.")}</p>
                    </div>
                    <label class="block space-y-2">
                        <span class="text-sm font-medium text-card-foreground">{t(locale, "search.dictionary.synonyms.term", "Canonical term")}</span>
                        <input type="text" class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=synonym_term on:input=move |ev| set_synonym_term.set(event_target_value(&ev)) />
                    </label>
                    <label class="block space-y-2">
                        <span class="text-sm font-medium text-card-foreground">{t(locale, "search.dictionary.synonyms.values", "Synonyms (CSV)")}</span>
                        <input type="text" class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=synonym_values on:input=move |ev| set_synonym_values.set(event_target_value(&ev)) />
                    </label>
                    <button type="submit" class="inline-flex w-full items-center justify-center rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || busy.get()>
                        {move || if busy.get() { saving_label_for_synonyms.clone() } else { save_synonym_group_label.clone() }}
                    </button>
                </form>

                <form class="space-y-4 rounded-2xl border border-border bg-card p-6 shadow-sm" on:submit=move |ev| submit_stop_word.run(ev)>
                    <div class="space-y-1">
                        <h3 class="text-base font-semibold text-card-foreground">{t(locale, "search.dictionary.stopWords.title", "Stop Words")}</h3>
                        <p class="text-sm text-muted-foreground">{t(locale, "search.dictionary.stopWords.subtitle", "Remove low-signal tokens before FTS execution.")}</p>
                    </div>
                    <label class="block space-y-2">
                        <span class="text-sm font-medium text-card-foreground">{t(locale, "search.dictionary.stopWords.value", "Stop word")}</span>
                        <input type="text" class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=stop_word_value on:input=move |ev| set_stop_word_value.set(event_target_value(&ev)) />
                    </label>
                    <button type="submit" class="inline-flex w-full items-center justify-center rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || busy.get()>
                        {move || if busy.get() { saving_label_for_stop_words.clone() } else { add_stop_word_label.clone() }}
                    </button>
                </form>

                <form class="space-y-4 rounded-2xl border border-border bg-card p-6 shadow-sm" on:submit=move |ev| submit_pin_rule.run(ev)>
                    <div class="space-y-1">
                        <h3 class="text-base font-semibold text-card-foreground">{t(locale, "search.dictionary.pinRules.title", "Pinned Results")}</h3>
                        <p class="text-sm text-muted-foreground">{t(locale, "search.dictionary.pinRules.subtitle", "Pin an existing search document for an exact normalized query.")}</p>
                    </div>
                    <label class="block space-y-2">
                        <span class="text-sm font-medium text-card-foreground">{t(locale, "search.dictionary.pinRules.queryText", "Query text")}</span>
                        <input type="text" class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=pin_query_text on:input=move |ev| set_pin_query_text.set(event_target_value(&ev)) />
                    </label>
                    <label class="block space-y-2">
                        <span class="text-sm font-medium text-card-foreground">{t(locale, "search.dictionary.pinRules.documentId", "Document ID")}</span>
                        <input type="text" class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=pin_document_id on:input=move |ev| set_pin_document_id.set(event_target_value(&ev)) />
                    </label>
                    <label class="block space-y-2">
                        <span class="text-sm font-medium text-card-foreground">{t(locale, "search.dictionary.pinRules.position", "Pinned position")}</span>
                        <input type="number" min="1" class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=pin_position on:input=move |ev| set_pin_position.set(event_target_value(&ev)) />
                    </label>
                    <button type="submit" class="inline-flex w-full items-center justify-center rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition hover:bg-primary/90 disabled:opacity-50" disabled=move || busy.get()>
                        {move || if busy.get() { saving_label_for_pin_rules.clone() } else { save_pin_rule_label.clone() }}
                    </button>
                </form>
            </div>

            <Suspense fallback=move || view! { <div class="h-32 animate-pulse rounded-2xl bg-muted"></div> }>
                {move || snapshot.get().map(|result| match result {
                    Ok(snapshot) => dictionaries_tables(snapshot, busy, delete_synonym, delete_stop_word, delete_query_rule, ui_locale.clone()).into_any(),
                    Err(err) => view! {
                        <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                            {format!("{}: {err}", load_dictionaries_error_label)}
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
    ui_locale: Option<String>,
) -> impl IntoView {
    let locale = ui_locale.as_deref();
    view! {
        <div class="space-y-6">
            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-1">
                    <h3 class="text-base font-semibold text-card-foreground">{t(locale, "search.dictionary.synonymGroups.title", "Synonym Groups")}</h3>
                    <p class="text-sm text-muted-foreground">{t(locale, "search.dictionary.synonymGroups.subtitle", "Each group expands all included terms as equivalent tokens.")}</p>
                </div>
                <div class="mt-5">{synonyms_table(snapshot.synonyms, busy, delete_synonym, ui_locale.clone())}</div>
            </section>
            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-1">
                    <h3 class="text-base font-semibold text-card-foreground">{t(locale, "search.dictionary.stopWords.title", "Stop Words")}</h3>
                    <p class="text-sm text-muted-foreground">{t(locale, "search.dictionary.stopWords.tableSubtitle", "Terms removed from the effective FTS query.")}</p>
                </div>
                <div class="mt-5">{stop_words_table(snapshot.stop_words, busy, delete_stop_word, ui_locale.clone())}</div>
            </section>
            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="space-y-1">
                    <h3 class="text-base font-semibold text-card-foreground">{t(locale, "search.dictionary.pinRules.tableTitle", "Pinned Query Rules")}</h3>
                    <p class="text-sm text-muted-foreground">{t(locale, "search.dictionary.pinRules.tableSubtitle", "Exact normalized queries that promote specific documents to chosen positions.")}</p>
                </div>
                <div class="mt-5">{query_rules_table(snapshot.query_rules, busy, delete_query_rule, ui_locale.clone())}</div>
            </section>
        </div>
    }
}

fn synonyms_table(
    rows: Vec<SearchSynonymPayload>,
    busy: ReadSignal<bool>,
    delete_synonym: Callback<String>,
    ui_locale: Option<String>,
) -> impl IntoView {
    let locale = ui_locale.as_deref();
    if rows.is_empty() {
        return view! { <div class="rounded-xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">{t(locale, "search.dictionary.synonymGroups.empty", "No synonym groups configured yet.")}</div> }.into_any();
    }

    view! { <div class="overflow-hidden rounded-xl border border-border"><table class="w-full text-sm">
        <thead class="border-b border-border bg-muted/50"><tr>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.term", "Term")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.synonyms", "Synonyms")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.updated", "Updated")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.actions", "Actions")}</th>
        </tr></thead>
        <tbody class="divide-y divide-border">{rows.into_iter().map(|row| {
            let synonym_id = row.id.clone();
            view! {
                <tr class="transition-colors hover:bg-muted/30">
                    <td class="px-4 py-3 align-top"><div class="font-medium text-card-foreground">{row.term}</div></td>
                    <td class="px-4 py-3 align-top text-xs text-muted-foreground">{row.synonyms.join(", ")}</td>
                    <td class="px-4 py-3 align-top text-xs text-muted-foreground">{row.updated_at}</td>
                    <td class="px-4 py-3 align-top">
                        <button type="button" class="inline-flex rounded-lg border border-border px-3 py-1 text-xs font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| delete_synonym.run(synonym_id.clone())>{t(locale, "search.action.delete", "Delete")}</button>
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
    ui_locale: Option<String>,
) -> impl IntoView {
    let locale = ui_locale.as_deref();
    if rows.is_empty() {
        return view! { <div class="rounded-xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">{t(locale, "search.dictionary.stopWords.empty", "No stop words configured yet.")}</div> }.into_any();
    }

    view! { <div class="overflow-hidden rounded-xl border border-border"><table class="w-full text-sm">
        <thead class="border-b border-border bg-muted/50"><tr>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.value", "Value")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.updated", "Updated")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.actions", "Actions")}</th>
        </tr></thead>
        <tbody class="divide-y divide-border">{rows.into_iter().map(|row| {
            let stop_word_id = row.id.clone();
            view! {
                <tr class="transition-colors hover:bg-muted/30">
                    <td class="px-4 py-3 align-top"><div class="font-medium text-card-foreground">{row.value}</div></td>
                    <td class="px-4 py-3 align-top text-xs text-muted-foreground">{row.updated_at}</td>
                    <td class="px-4 py-3 align-top">
                        <button type="button" class="inline-flex rounded-lg border border-border px-3 py-1 text-xs font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| delete_stop_word.run(stop_word_id.clone())>{t(locale, "search.action.delete", "Delete")}</button>
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
    ui_locale: Option<String>,
) -> impl IntoView {
    let locale = ui_locale.as_deref();
    if rows.is_empty() {
        return view! { <div class="rounded-xl border border-dashed border-border p-8 text-center text-sm text-muted-foreground">{t(locale, "search.dictionary.pinRules.empty", "No pinned query rules configured yet.")}</div> }.into_any();
    }

    view! { <div class="overflow-hidden rounded-xl border border-border"><table class="w-full text-sm">
        <thead class="border-b border-border bg-muted/50"><tr>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.query", "Query")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.target", "Target")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.position", "Position")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.updated", "Updated")}</th>
            <th class="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground">{t(locale, "search.table.actions", "Actions")}</th>
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
                        <button type="button" class="inline-flex rounded-lg border border-border px-3 py-1 text-xs font-medium text-foreground transition hover:bg-accent disabled:opacity-50" disabled=move || busy.get() on:click=move |_| delete_query_rule.run(query_rule_id.clone())>{t(locale, "search.action.delete", "Delete")}</button>
                    </td>
                </tr>
            }
        }).collect_view()}</tbody>
    </table></div> }.into_any()
}

#[component]
fn DiagnosticsCard(
    diagnostics: SearchDiagnosticsPayload,
    ui_locale: Option<String>,
) -> impl IntoView {
    let locale = ui_locale.as_deref();
    let badge_class = match diagnostics.state.as_str() {
        "healthy" => "border-emerald-200 bg-emerald-50 text-emerald-700",
        "inconsistent" => "border-rose-200 bg-rose-50 text-rose-700",
        "lagging" => "border-amber-200 bg-amber-50 text-amber-700",
        _ => "border-slate-200 bg-slate-50 text-slate-700",
    };
    let state_label = match diagnostics.state.as_str() {
        "healthy" => t(locale, "search.state.healthy", "healthy"),
        "inconsistent" => t(locale, "search.state.inconsistent", "inconsistent"),
        "lagging" => t(locale, "search.state.lagging", "lagging"),
        other => other.to_string(),
    };
    view! { <article class="rounded-2xl border border-border bg-card p-5 shadow-sm">
        <div class="text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">{t(locale, "search.diagnostics.indexState", "Index state")}</div>
        <div class="mt-3"><span class=format!("inline-flex rounded-full border px-3 py-1 text-xs font-semibold {badge_class}")>{state_label}</span></div>
        <p class="mt-3 text-sm text-muted-foreground">{format!("{}: {}", t(locale, "search.diagnostics.newestIndexed", "Newest indexed"), diagnostics.newest_indexed_at.unwrap_or_else(|| t(locale, "search.common.notIndexedYet", "not indexed yet")))}</p>
    </article> }
}

#[component]
fn InfoCard<T, U>(title: T, value: U, detail: String) -> impl IntoView
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
    locale: Option<&str>,
    config_text: &str,
    ranking_default: &str,
    ranking_preview: &str,
    ranking_storefront: &str,
    ranking_admin_global: &str,
    preview_presets: &str,
    storefront_presets: &str,
) -> Result<String, String> {
    let mut config = parse_json_for_editor(config_text)
        .ok_or_else(|| t(locale, "search.error.invalidSettingsJson", "Settings config must be valid JSON."))?;
    let object = config
        .as_object_mut()
        .ok_or_else(|| t(locale, "search.error.settingsConfigRootObject", "Settings config root must be a JSON object."))?;

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
            "search_preview": parse_json_array_for_editor(locale, t(locale, "search.relevance.previewPresets", "Preview filter presets").as_str(), preview_presets)?,
            "storefront_search": parse_json_array_for_editor(locale, t(locale, "search.relevance.storefrontPresets", "Storefront filter presets").as_str(), storefront_presets)?,
        }),
    );

    serde_json::to_string_pretty(&config)
        .map_err(|err| format!("{}: {err}", t(locale, "search.error.serializeMergedSettings", "Failed to serialize merged search settings config")))
}

fn parse_json_array_for_editor(
    locale: Option<&str>,
    label: &str,
    value: &str,
) -> Result<serde_json::Value, String> {
    let parsed: serde_json::Value =
        serde_json::from_str(value).map_err(|err| {
            t(
                locale,
                "search.error.editorArrayJson",
                "{label} must be valid JSON: {err}",
            )
            .replace("{label}", label)
            .replace("{err}", err.to_string().as_str())
        })?;
    if !parsed.is_array() {
        return Err(
            t(
                locale,
                "search.error.editorArrayType",
                "{label} must be a JSON array.",
            )
            .replace("{label}", label),
        );
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
