use crate::access::{pages_can_restore_body_revision, pages_editor_capability_policy};
use crate::browser_intent::{mark_stale_draft_conflict, pages_browser_draft_store};
use crate::builder::{self, PagesBuilderFacade, PagesBuilderSaveSnapshot};
use crate::contributions::{
    build_pages_admin_contribution_registry, pages_admin_contribution_policy,
};
use crate::core;
use crate::i18n::t;
use crate::model::{PageBuilderScenarioReleaseStatus, PageDetail, PageList};
use crate::starter_admin::StarterCatalogManager;
use crate::transport;
use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
use rustok_auth_ui::ui::leptos::hooks::{use_current_user, use_tenant, use_token};
use rustok_page_builder::runtime_context::{
    PageBuilderRuntimeExampleRequest, generate_page_builder_runtime_example,
};
use rustok_page_builder::runtime_scenario_release::{
    PageBuilderScenarioBaselineChange, RuntimeScenarioReleaseBaseline,
};
use rustok_page_builder::{RuntimeContextExamplePolicy, RuntimeContextScenario};
use rustok_page_builder_admin::{
    DraftSessionOwner, PageBuilderAdmin, PageBuilderAdminFacade, PageBuilderAdminHostContext,
    PageBuilderAdminProviderStatus, SsrDraftRenderBinding, SsrDraftSessionError,
    SsrDraftSessionSnapshot, SsrDraftSessionStore,
};
use rustok_ui_core::{AdminQueryKey, UiRouteContext};
use serde_json::{Value, json};
use std::sync::Arc;

/// The server state of one open page, as a `Resource` can carry.
///
/// `PageBuilderAdminProviderStatus` is a runtime view of two values — rollout flags and an optional
/// health snapshot — and only the values cross a resource boundary; the status is derived again by
/// the render that reads them ([`PagesWorkspaceData::provider_status`]). That keeps the resource
/// payload serialisable without inventing a second provider-status type for the wire, and without
/// letting a consumer fabricate an observed snapshot: an absent health value stays absent.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct PagesWorkspaceData {
    page: PageDetail,
    baseline: Option<RuntimeScenarioReleaseBaseline>,
    release_status: PageBuilderScenarioReleaseStatus,
    provider_flags: rustok_page_builder::rollout::BuilderCapabilityFlags,
    provider_health: Option<rustok_page_builder::health::ProviderHealthSnapshot>,
}

impl PagesWorkspaceData {
    fn provider_status(&self) -> PageBuilderAdminProviderStatus {
        match self.provider_health.clone() {
            Some(health) => {
                PageBuilderAdminProviderStatus::observed(self.provider_flags.clone(), health)
            }
            None => PageBuilderAdminProviderStatus::unobserved(self.provider_flags.clone()),
        }
    }
}

#[component]
pub fn PagesAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let selected_page_query = use_route_query_value(AdminQueryKey::PageId.as_str());
    let draft_query = use_route_query_value(core::FLY_DRAFT_QUERY_KEY);
    let query_writer = use_route_query_writer();
    let token = use_token();
    let tenant = use_tenant();
    let refresh_generation = RwSignal::new(0_u64);
    let starter_generation = RwSignal::new(0_u64);
    let default_locale = route_context
        .locale
        .clone()
        .unwrap_or_else(|| "en".to_string());
    let templates_locale = default_locale.clone();

    let list_token = token;
    let list_tenant = tenant;
    // A `Resource`, not a `LocalResource`: under `ssr` a `LocalResource` is `pending().await` by
    // construction, so the standalone host answered with the `Suspense` fallback and never with
    // the page list. A `Resource` is awaited during the render, and its `JsonSerdeCodec` default
    // also means the same values hydrate instead of being fetched twice.
    let pages_resource = Resource::new(
        move || {
            (
                list_token.get(),
                list_tenant.get(),
                refresh_generation.get(),
            )
        },
        move |(token, tenant, _generation)| async move { transport::fetch_pages(token, tenant).await },
    );

    let workspace_token = token;
    let workspace_tenant = tenant;
    let workspace_resource = Resource::new(
        move || {
            (
                selected_page_query.get(),
                workspace_token.get(),
                workspace_tenant.get(),
                refresh_generation.get(),
            )
        },
        move |(page_id, token, tenant, _generation)| async move {
            let Some(page_id) = page_id.filter(|page_id| core::optional_ui_text(page_id).is_some())
            else {
                return Ok::<_, String>(None);
            };
            let Some(page) = transport::fetch_page(token.clone(), tenant.clone(), page_id.clone())
                .await
                .map_err(|error| error.to_string())?
            else {
                return Ok(None);
            };
            let baseline = transport::fetch_page_builder_scenario_baseline(
                token.clone(),
                tenant.clone(),
                page_id.clone(),
            )
            .await
            .map_err(|error| error.to_string())?;
            let release_status = transport::fetch_page_builder_scenario_release_status(
                token.clone(),
                tenant.clone(),
                page_id,
            )
            .await
            .map_err(|error| error.to_string())?;
            let rollout = crate::builder_rollout_settings::fetch_pages_builder_rollout_snapshot(
                token, tenant,
            )
            .await
            .map_err(|error| error.to_string())?;
            Ok(Some(PagesWorkspaceData {
                page,
                baseline,
                release_status,
                provider_flags: rollout.flags.clone(),
                provider_health: rollout.provider_health.clone(),
            }))
        },
    );

    let select_writer = query_writer.clone();
    let clear_writer = query_writer.clone();
    let locale = route_context.locale.clone();
    let title = t(locale.as_deref(), "pages.title", "Pages");
    let subtitle = t(
        locale.as_deref(),
        "pages.subtitle",
        "Create, publish and edit current Fly documents without a parallel JSON editor.",
    );
    let list_error = t(
        locale.as_deref(),
        "pages.error.load",
        "Failed to load pages",
    );
    let workspace_error = t(
        locale.as_deref(),
        "pages.builder.loadError",
        "Failed to load the selected page workspace",
    );

    let main_tab = RwSignal::new("pages");

    view! {
        <div class="space-y-6">
            <header class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="flex flex-wrap items-start justify-between gap-4">
                    <div>
                        <h1 class="text-2xl font-semibold text-card-foreground">{title}</h1>
                        <p class="mt-2 max-w-3xl text-sm text-muted-foreground">{subtitle}</p>
                    </div>
                    <div class="flex items-center gap-2">
                        <button
                            type="button"
                            class=move || {
                                let is_active = main_tab.get() == "pages";
                                if is_active {
                                    "rounded-lg bg-primary px-3 py-2 text-sm font-medium text-primary-foreground shadow-sm"
                                } else {
                                    "rounded-lg border border-border px-3 py-2 text-sm font-medium hover:bg-muted text-muted-foreground"
                                }
                            }
                            on:click=move |_| main_tab.set("pages")
                        >
                            "Pages"
                        </button>
                        <button
                            type="button"
                            class=move || {
                                let is_active = main_tab.get() == "templates";
                                if is_active {
                                    "rounded-lg bg-primary px-3 py-2 text-sm font-medium text-primary-foreground shadow-sm"
                                } else {
                                    "rounded-lg border border-border px-3 py-2 text-sm font-medium hover:bg-muted text-muted-foreground"
                                }
                            }
                            on:click=move |_| main_tab.set("templates")
                        >
                            "Templates & Symbols"
                        </button>
                        {move || (main_tab.get() == "pages").then(|| {
                            let writer = clear_writer.clone();
                            view! {
                                <button
                                    type="button"
                                    class="rounded-lg border border-border px-3 py-2 text-sm font-medium hover:bg-muted"
                                    on:click=move |_| writer.update(core::page_selection_query_update(None), true)
                                >
                                    "New page"
                                </button>
                            }
                        })}
                    </div>
                </div>
            </header>

            <div class=move || if main_tab.get() == "pages" { "grid gap-6 xl:grid-cols-[20rem_minmax(0,1fr)]" } else { "hidden" }>
                <aside class="space-y-4">
                    <CreatePageCard refresh_generation starter_generation default_locale=default_locale.clone() />
                    <StarterCatalogManager starter_generation default_locale=default_locale.clone() />
                    <section class="rounded-2xl border border-border bg-card p-4 shadow-sm">
                        <div class="mb-3 flex items-center justify-between gap-3">
                            <h2 class="font-semibold text-card-foreground">"Documents"</h2>
                            <span class="text-xs text-muted-foreground">"current contract"</span>
                        </div>
                        <Suspense fallback=|| view! {
                            <div class="space-y-2" aria-label="Loading pages">
                                <div class="h-16 animate-pulse rounded-xl bg-muted"></div>
                                <div class="h-16 animate-pulse rounded-xl bg-muted"></div>
                            </div>
                        }>
                            {move || {
                                let writer = select_writer.clone();
                                pages_resource.get().map(|result| match result {
                                    Ok(pages) => view! {
                                        <PagesNavigator
                                            pages
                                            selected_page_id=selected_page_query.get()
                                            on_select=Callback::new(move |page_id| {
                                                if selected_page_query.get_untracked().as_deref() != Some(page_id.as_str()) {
                                                    writer.update(core::page_selection_query_update(Some(page_id)), true);
                                                }
                                            })
                                        />
                                    }.into_any(),
                                    Err(error) => view! {
                                        <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-3 py-3 text-sm text-destructive" role="alert">
                                            {format!("{list_error}: {error}")}
                                        </div>
                                    }.into_any(),
                                })
                            }}
                        </Suspense>
                    </section>
                </aside>

                <main class="min-w-0">
                    <Suspense fallback=|| view! {
                        <div class="h-[36rem] animate-pulse rounded-2xl border border-border bg-muted" aria-label="Loading page workspace"></div>
                    }>
                        {move || {
                            workspace_resource.get().map(|result| match result {
                                Ok(Some(data)) => {
                                    // The status is derived before the fields move into the view:
                                    // it reads them, and a partially moved value cannot be read.
                                    let provider_status = data.provider_status();
                                    view! {
                                    <PageWorkspace
                                        page=data.page
                                        baseline=data.baseline
                                        release_status=data.release_status
                                        provider_status=provider_status
                                        token
                                        tenant
                                        default_locale=default_locale.clone()
                                        draft_token=draft_query.get()
                                        starter_generation
                                        refresh_generation
                                    />
                                    }
                                    .into_any()
                                }
                                Ok(None) => view! { <WorkspaceEmptyState /> }.into_any(),
                                Err(error) => view! {
                                    <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-5 py-5 text-sm text-destructive" role="alert">
                                        {format!("{workspace_error}: {error}")}
                                    </div>
                                }.into_any(),
                            })
                        }}
                    </Suspense>
                </main>
            </div>

            <div class=move || if main_tab.get() == "templates" { "block" } else { "hidden" }>
                <crate::templates_admin::TemplatesAdmin default_locale=templates_locale />
            </div>
        </div>
    }
}

#[component]
fn CreatePageCard(
    refresh_generation: RwSignal<u64>,
    starter_generation: RwSignal<u64>,
    default_locale: String,
) -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();
    let query_writer = use_route_query_writer();
    let starter_label = t(
        Some(&default_locale),
        "pages.starter.label",
        "Starter template",
    );
    let starter_loading = t(
        Some(&default_locale),
        "pages.starter.loading",
        "Loading starters...",
    );
    let starter_unavailable = t(
        Some(&default_locale),
        "pages.starter.unavailable",
        "Starter catalog unavailable",
    );
    let starter_selection_error = t(
        Some(&default_locale),
        "pages.starter.selectionError",
        "Starter catalog is unavailable or the selection changed. Reload and try again.",
    );
    let starter_prepare_error = t(
        Some(&default_locale),
        "pages.starter.prepareError",
        "Failed to prepare starter",
    );
    let starter_layout_hint = t(
        Some(&default_locale),
        "pages.starter.layoutHint",
        "Suggested layout (not applied automatically)",
    );
    let starter_context_error = t(
        Some(&default_locale),
        "pages.starter.contextError",
        "Session or tenant changed. Reload and try again.",
    );
    let layout_label = t(Some(&default_locale), "pages.layout.label", "Page layout");
    let layout_loading = t(
        Some(&default_locale),
        "pages.layout.loading",
        "Loading layouts...",
    );
    let layout_unavailable = t(
        Some(&default_locale),
        "pages.layout.unavailable",
        "Layout catalog unavailable",
    );
    let layout_selection_error = t(
        Some(&default_locale),
        "pages.layout.selectionError",
        "Choose a layout available for this locale. Reload the catalog if needed.",
    );
    let layout_refresh_label = t(
        Some(&default_locale),
        "pages.layout.reload",
        "Reload layouts",
    );
    let layout_permissions_hint = t(
        Some(&default_locale),
        "pages.layout.permissionsHint",
        "Non-default layouts require pages:manage for builder composition.",
    );
    let layout_available_label = t(
        Some(&default_locale),
        "pages.layout.suggestionAvailable",
        "Available to you for this locale",
    );
    let layout_missing_label = t(
        Some(&default_locale),
        "pages.layout.suggestionMissing",
        "Not available to you for this locale",
    );
    let layout_context_after_create = t(
        Some(&default_locale),
        "pages.layout.contextAfterCreate",
        "Session, tenant or locale changed while creating the page. The page may have been created in the previous context; check before retrying.",
    );
    let title = RwSignal::new(String::new());
    let slug = RwSignal::new(String::new());
    let slug_touched = RwSignal::new(false);
    let locale = RwSignal::new(default_locale);
    let channel_slugs = RwSignal::new(String::new());
    let selected_starter_id = RwSignal::new("blank".to_string());
    let selected_template = RwSignal::new("default".to_string());
    let layout_generation = RwSignal::new(0_u64);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);

    // Associate the result with its tenant without serializing the auth token
    // into the SSR resource payload.
    let starter_catalog = Resource::new(
        move || (token.get(), tenant.get(), starter_generation.get()),
        move |(token, tenant, _generation)| async move {
            let result = transport::fetch_page_starters(token, tenant.clone())
                .await
                .map_err(|error| error.to_string());
            (tenant, result)
        },
    );

    let template_token = token;
    let template_tenant = tenant;
    let template_locale = locale;
    let template_keys = Resource::new(
        move || {
            (
                template_token.get(),
                template_tenant.get(),
                core::ui_text_or_default(&template_locale.get()),
                layout_generation.get(),
            )
        },
        move |(token, tenant, locale, generation)| async move {
            let result = transport::fetch_template_keys(token, tenant.clone(), locale.clone())
                .await
                .map_err(|error| error.to_string());
            (tenant, locale, generation, result)
        },
    );

    let submit_layout_selection_error = layout_selection_error.clone();
    let submit_layout_unavailable = layout_unavailable.clone();
    let submit = move |event: SubmitEvent| {
        event.prevent_default();
        if busy.get_untracked() {
            return;
        }

        let title_value = core::ui_text_or_default(&title.get_untracked());
        let explicit_slug = core::ui_text_or_default(&slug.get_untracked());
        let slug_value = if explicit_slug.is_empty() {
            core::slugify(&title_value)
        } else {
            explicit_slug
        };
        let locale_value = core::ui_text_or_default(&locale.get_untracked());
        let channels_value = channel_slugs.get_untracked();
        let starter_id_value = selected_starter_id.get_untracked();
        let template_value = selected_template.get_untracked();
        if core::missing_required_page_field(&title_value, &slug_value).is_some() {
            error.set(Some(
                "Title and slug are required to create a page.".to_string(),
            ));
            return;
        }

        let token_signal = token;
        let tenant_signal = tenant;
        let token = token.get_untracked();
        let tenant = tenant.get_untracked();
        let selected = match starter_catalog.get_untracked() {
            Some((loaded_tenant, Ok(starters))) if loaded_tenant == tenant => {
                core::selected_page_starter(&starters, &starter_id_value)
            }
            _ => None,
        };
        let Some(selected) = selected else {
            error.set(Some(starter_selection_error.clone()));
            return;
        };
        let valid_layout = matches!(template_keys.get_untracked(),
            Some((loaded_tenant, loaded_locale, generation, Ok(keys)))
                if loaded_tenant == tenant && loaded_locale == locale_value
                    && generation == layout_generation.get_untracked()
                    && core::selected_layout_available(&keys, &template_value));
        if !valid_layout {
            error.set(Some(submit_layout_selection_error.clone()));
            return;
        }
        busy.set(true);
        error.set(None);
        let writer = query_writer.clone();
        let starter_prepare_error = starter_prepare_error.clone();
        let starter_context_error = starter_context_error.clone();
        let layout_selection_error = submit_layout_selection_error.clone();
        let layout_unavailable = submit_layout_unavailable.clone();
        let layout_context_after_create = layout_context_after_create.clone();
        spawn_local(async move {
            let document = match transport::instantiate_page_starter(
                token.clone(),
                tenant.clone(),
                selected.key,
                selected.revision,
                locale_value.clone(),
            )
            .await
            {
                Ok(document) => document,
                Err(starter_error) => {
                    error.set(Some(format!("{starter_prepare_error}: {starter_error}")));
                    busy.set(false);
                    return;
                }
            };
            if token_signal.get_untracked() != token || tenant_signal.get_untracked() != tenant {
                error.set(Some(starter_context_error.clone()));
                busy.set(false);
                return;
            }
            // Recheck the exact-locale catalog just before creation; this is not a
            // transaction with the subsequent create mutation.
            let keys = match transport::fetch_template_keys(
                token.clone(),
                tenant.clone(),
                locale_value.clone(),
            )
            .await
            {
                Ok(keys) => keys,
                Err(problem) => {
                    error.set(Some(format!("{layout_unavailable}: {problem}")));
                    busy.set(false);
                    return;
                }
            };
            if token_signal.get_untracked() != token
                || tenant_signal.get_untracked() != tenant
                || core::ui_text_or_default(&locale.get_untracked()) != locale_value
            {
                error.set(Some(starter_context_error));
                busy.set(false);
                return;
            }
            if selected_template.get_untracked() != template_value
                || !core::selected_layout_available(&keys, &template_value)
            {
                error.set(Some(layout_selection_error));
                layout_generation.update(|generation| *generation = generation.wrapping_add(1));
                busy.set(false);
                return;
            }
            let draft = core::build_create_page_draft(
                core::PageDraftFormInput {
                    locale: &locale_value,
                    title: &title_value,
                    slug: &slug_value,
                    channel_slugs: &channels_value,
                    template: Some(&template_value),
                },
                document,
            );
            let result = transport::create_page(token.clone(), tenant.clone(), draft).await;
            if token_signal.get_untracked() != token
                || tenant_signal.get_untracked() != tenant
                || core::ui_text_or_default(&locale.get_untracked()) != locale_value
            {
                error.set(Some(layout_context_after_create));
                busy.set(false);
                return;
            }
            match result {
                Ok(page) => {
                    title.set(String::new());
                    slug.set(String::new());
                    slug_touched.set(false);
                    channel_slugs.set(String::new());
                    selected_starter_id.set("blank".to_string());
                    selected_template.set("default".to_string());
                    refresh_generation
                        .update(|generation| *generation = generation.wrapping_add(1));
                    writer.update(core::page_selection_query_update(Some(page.id)), true);
                }
                Err(create_error) => error.set(Some(create_error.to_string())),
            }
            busy.set(false);
        });
    };

    view! {
        <section class="rounded-2xl border border-border bg-card p-4 shadow-sm">
            <h2 class="font-semibold text-card-foreground">"Create page"</h2>
            <p class="mt-1 text-xs text-muted-foreground">
                "Creates a current Fly document. Content is edited only in the builder."
            </p>
            <form class="mt-4" on:submit=submit>
                <fieldset class="space-y-3" disabled=move || busy.get()>
                <label class="block text-sm font-medium text-card-foreground">
                    "Title"
                    <input
                        class="mt-1 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                        prop:value=move || title.get()
                        on:input=move |event| {
                            let value = event_target_value(&event);
                            title.set(value.clone());
                            if !slug_touched.get_untracked() {
                                slug.set(core::slugify(&value));
                            }
                        }
                        required
                    />
                </label>
                <label class="block text-sm font-medium text-card-foreground">
                    "Slug"
                    <input
                        class="mt-1 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                        prop:value=move || slug.get()
                        on:input=move |event| {
                            slug_touched.set(true);
                            slug.set(event_target_value(&event));
                        }
                        required
                    />
                </label>
                <div class="grid gap-3 sm:grid-cols-2 xl:grid-cols-1">
                    <label class="block text-sm font-medium text-card-foreground">
                        "Locale"
                        <input
                            class="mt-1 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                            prop:value=move || locale.get()
                            on:input=move |event| locale.set(event_target_value(&event))
                            required
                    />
                    </label>
                    <label class="block text-sm font-medium text-card-foreground">
                        "Channels"
                        <input
                            class="mt-1 w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                            prop:value=move || channel_slugs.get()
                            on:input=move |event| channel_slugs.set(event_target_value(&event))
                            placeholder="web, mobile"
                    />
                    </label>
                </div>
                <div class="space-y-1.5">
                    <label class="block text-sm font-medium text-card-foreground" for="pages-layout-select">{layout_label.clone()}</label>
                    <p class="text-xs text-muted-foreground">{layout_permissions_hint.clone()}</p>
                    <button type="button" class="text-xs underline" on:click=move |_| layout_generation.update(|generation| *generation = generation.wrapping_add(1))>{layout_refresh_label.clone()}</button>
                    <Suspense fallback=move || view! { <p class="text-xs text-muted-foreground">{layout_loading.clone()}</p> }>
                        {move || template_keys.get().map(|(loaded_tenant, loaded_locale, generation, result)| {
                            if loaded_tenant != tenant.get() || loaded_locale != core::ui_text_or_default(&locale.get()) || generation != layout_generation.get() {
                                return view! { <p role="alert" class="text-xs text-destructive">{layout_unavailable.clone()}</p> }.into_any();
                            }
                            match result {
                                Err(problem) => view! { <p role="alert" class="text-xs text-destructive">{format!("{}: {}", layout_unavailable, problem)}</p> }.into_any(),
                                Ok(keys) => {
                                    let current = selected_template.get();
                                    let valid = core::selected_layout_available(&keys, &current);
                                    view! {
                                        <select id="pages-layout-select" class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm" prop:value=move || selected_template.get() on:change=move |event| selected_template.set(event_target_value(&event))>
                                            {(!valid).then(|| view! { <option value=current selected=true disabled=true>{layout_selection_error.clone()}</option> })}
                                            {keys.into_iter().map(|key| {
                                                let selected_key = key.clone();
                                                view! { <option value=key.clone() selected=move || selected_template.get() == selected_key>{key}</option> }
                                            }).collect_view()}
                                        </select>
                                        {(!valid).then(|| view! { <p role="alert" class="text-xs text-destructive">{layout_selection_error.clone()}</p> })}
                                    }.into_any()
                                }
                            }
                        })}
                    </Suspense>
                </div>
                <div class="space-y-1.5 pt-1">
                    <label class="block text-sm font-medium text-card-foreground" for="pages-starter-select">
                        {starter_label.clone()}
                    </label>
                    <Suspense fallback=move || view! {
                        <p class="text-xs text-muted-foreground">{starter_loading.clone()}</p>
                    }>
                        {move || starter_catalog.get().map(|(loaded_tenant, result)| {
                            if loaded_tenant != tenant.get() {
                                return view! { <p role="alert" class="text-xs text-destructive">{starter_unavailable.clone()}</p> }.into_any();
                            }
                            match result {
                                Ok(starters) if starters.is_empty() => view! {
                                    <p role="alert" class="text-xs text-destructive">{starter_unavailable.clone()}</p>
                                }.into_any(),
                                Ok(starters) => {
                                    let selected = core::selected_page_starter(&starters, &selected_starter_id.get());
                                    let description = selected.as_ref().and_then(|item| item.description.clone());
                                    let layout_hint = selected.map(|item| {
                                        let availability = match template_keys.get() {
                                            Some((loaded_tenant, loaded_locale, generation, Ok(keys)))
                                                if loaded_tenant == tenant.get()
                                                    && loaded_locale == core::ui_text_or_default(&locale.get())
                                                    && generation == layout_generation.get() => Some(if core::selected_layout_available(&keys, &item.default_template_key) {
                                                        layout_available_label.clone()
                                                    } else {
                                                        layout_missing_label.clone()
                                                    }),
                                            _ => None,
                                        };
                                        let suffix = availability.map(|status| format!(" — {status}")).unwrap_or_default();
                                        format!("{}: {}{}", starter_layout_hint, item.default_template_key, suffix)
                                    });
                                    view! {
                                    <select
                                        id="pages-starter-select"
                                        class="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm"
                                        prop:value=move || selected_starter_id.get()
                                        on:change=move |event| selected_starter_id.set(event_target_value(&event))
                                        disabled=move || busy.get()
                                    >
                                        {starters.into_iter().map(|starter| {
                                            let key = starter.key.clone();
                                            let label = format!("{} ({})", starter.name, starter.category);
                                            view! { <option value=key.clone() selected=move || selected_starter_id.get() == key>{label}</option> }
                                        }).collect_view()}
                                    </select>
                                    {description.map(|value| view! { <p class="text-xs text-muted-foreground">{value}</p> })}
                                    {layout_hint.map(|value| view! { <p class="text-xs text-muted-foreground">{value}</p> })}
                                }.into_any()
                                },
                                Err(problem) => view! {
                                    <p role="alert" class="text-xs text-destructive">{format!("{}: {}", starter_unavailable, problem)}</p>
                                }.into_any(),
                            }
                        })}
                    </Suspense>
                </div>
                {move || error.get().map(|message| view! {
                    <div class="rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive" role="alert">
                        {message}
                    </div>
                })}
                <button
                    type="submit"
                    class="w-full rounded-lg bg-primary px-3 py-2 text-sm font-medium text-primary-foreground disabled:opacity-50"
                    disabled=move || busy.get() || !matches!(template_keys.get(),
                        Some((loaded_tenant, loaded_locale, generation, Ok(keys)))
                            if loaded_tenant == tenant.get() && loaded_locale == core::ui_text_or_default(&locale.get())
                                && generation == layout_generation.get()
                                && core::selected_layout_available(&keys, &selected_template.get()))
                >
                    {move || if busy.get() { "Creating..." } else { "Create and open" }}
                </button>
                </fieldset>
            </form>
        </section>
    }
}

#[component]
fn PagesNavigator(
    pages: PageList,
    selected_page_id: Option<String>,
    on_select: Callback<String>,
) -> impl IntoView {
    if pages.items.is_empty() {
        return view! {
            <div class="rounded-xl border border-dashed border-border px-3 py-6 text-center text-sm text-muted-foreground">
                "No pages yet. Create the first current Fly document above."
            </div>
        }
        .into_any();
    }

    view! {
        <div class="space-y-2">
            {pages.items.into_iter().map(|page| {
                let page_id = page.id.clone();
                let selected = selected_page_id.as_deref() == Some(page.id.as_str());
                let title = page.title.clone().filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "Untitled page".to_string());
                let slug = page.slug.clone().filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| page.id.clone());
                let status = page.status.clone();
                let select = on_select;
                view! {
                    <button
                        type="button"
                        class=if selected {
                            "w-full rounded-xl border border-primary bg-primary/5 px-3 py-3 text-left"
                        } else {
                            "w-full rounded-xl border border-border bg-background px-3 py-3 text-left hover:bg-muted/50"
                        }
                        on:click=move |_| select.run(page_id.clone())
                    >
                        <div class="flex items-start justify-between gap-3">
                            <div class="min-w-0">
                                <div class="truncate text-sm font-medium text-foreground">{title}</div>
                                <div class="mt-1 truncate text-xs text-muted-foreground">{slug}</div>
                            </div>
                            <span class=format!(
                                "rounded-full px-2 py-1 text-[10px] font-semibold uppercase {}",
                                core::status_badge_class(&status)
                            )>{status.clone()}</span>
                        </div>
                    </button>
                }
            }).collect_view()}
        </div>
    }
    .into_any()
}

#[component]
fn WorkspaceEmptyState() -> impl IntoView {
    view! {
        <section class="rounded-2xl border border-dashed border-border bg-card px-6 py-16 text-center shadow-sm">
            <h2 class="text-xl font-semibold text-card-foreground">"Select or create a page"</h2>
            <p class="mx-auto mt-2 max-w-xl text-sm text-muted-foreground">
                "Pages has one authoring surface. Choose a document from the left or create a new one to open Fly."
            </p>
        </section>
    }
}

#[component]
fn PageWorkspace(
    page: PageDetail,
    baseline: Option<RuntimeScenarioReleaseBaseline>,
    release_status: PageBuilderScenarioReleaseStatus,
    provider_status: PageBuilderAdminProviderStatus,
    token: Signal<Option<String>>,
    tenant: Signal<Option<String>>,
    default_locale: String,
    draft_token: Option<String>,
    starter_generation: RwSignal<u64>,
    refresh_generation: RwSignal<u64>,
) -> impl IntoView {
    let query_writer = use_route_query_writer();
    let action_busy = RwSignal::new(None::<String>);
    let action_error = RwSignal::new(None::<String>);
    let title = page
        .translation
        .as_ref()
        .and_then(|translation| translation.title.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "Untitled page".to_string());
    let slug = page
        .translation
        .as_ref()
        .and_then(|translation| translation.slug.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| page.id.clone());
    let locale = page
        .translation
        .as_ref()
        .map(|translation| translation.locale.clone())
        .or_else(|| page.body.as_ref().map(|body| body.locale.clone()))
        .unwrap_or_else(|| default_locale.clone());
    let revision = builder::page_revision(&page);
    // The principal may see the page record without its authoring document. The workspace
    // then names that state instead of opening the starter document in its place.
    let document_readable = page.document_readable;
    let ui_locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let document_unavailable_title = t(
        ui_locale.as_deref(),
        "pages.document.unavailableTitle",
        "Document not available",
    );
    let document_unavailable_message = t(
        ui_locale.as_deref(),
        "pages.document.unavailable",
        core::DOCUMENT_UNAVAILABLE_MESSAGE,
    );
    let document_unavailable_value = t(
        ui_locale.as_deref(),
        "pages.document.unavailableValue",
        "not available",
    );
    let body_format = if document_readable {
        page.body
            .as_ref()
            .map(|body| body.format.clone())
            .unwrap_or_else(|| rustok_page_builder::PAGE_BUILDER_DOCUMENT_FORMAT.to_string())
    } else {
        document_unavailable_value.clone()
    };
    let revision_label = if document_readable {
        revision.clone()
    } else {
        document_unavailable_value
    };
    let channels = if page.channel_slugs.is_empty() {
        "all".to_string()
    } else {
        page.channel_slugs.join(", ")
    };
    let is_published = page.status.eq_ignore_ascii_case("published");

    let publish_page_id = page.id.clone();
    let publish_action = Callback::new(move |publish: bool| {
        let page_id = publish_page_id.clone();
        let token = token.get_untracked();
        let tenant = tenant.get_untracked();
        action_busy.set(Some(
            if publish { "publish" } else { "unpublish" }.to_string(),
        ));
        action_error.set(None);
        spawn_local(async move {
            let result = if publish {
                transport::publish_page(token, tenant, page_id).await
            } else {
                transport::unpublish_page(token, tenant, page_id).await
            };
            match result {
                Ok(_) => {
                    refresh_generation.update(|generation| *generation = generation.wrapping_add(1))
                }
                Err(error) => action_error.set(Some(error.to_string())),
            }
            action_busy.set(None);
        });
    });

    let delete_page_id = page.id.clone();
    let delete_writer = query_writer.clone();
    let delete_action = move |_| {
        let page_id = delete_page_id.clone();
        let token = token.get_untracked();
        let tenant = tenant.get_untracked();
        let writer = delete_writer.clone();
        action_busy.set(Some("delete".to_string()));
        action_error.set(None);
        spawn_local(async move {
            match transport::delete_page(token, tenant, page_id).await {
                Ok(true) => {
                    writer.update(core::page_selection_query_update(None), true);
                    refresh_generation
                        .update(|generation| *generation = generation.wrapping_add(1));
                }
                Ok(false) => action_error.set(Some("Page was not deleted".to_string())),
                Err(error) => action_error.set(Some(error.to_string())),
            }
            action_busy.set(None);
        });
    };

    let duplicate_page_id = page.id.clone();
    let duplicate_writer = query_writer.clone();
    let duplicate_action = move |_| {
        let page_id = duplicate_page_id.clone();
        let token = token.get_untracked();
        let tenant = tenant.get_untracked();
        let writer = duplicate_writer.clone();
        action_busy.set(Some("duplicate".to_string()));
        action_error.set(None);
        spawn_local(async move {
            match transport::duplicate_page(token, tenant, page_id).await {
                Ok(duplicated) => {
                    writer.update(
                        core::page_selection_query_update(Some(duplicated.id.clone())),
                        true,
                    );
                    refresh_generation
                        .update(|generation| *generation = generation.wrapping_add(1));
                }
                Err(error) => action_error.set(Some(error.to_string())),
            }
            action_busy.set(None);
        });
    };

    let page_for_builder = page.clone();
    let history_page_id = page.id.clone();
    let starter_page_id = page.id.clone();
    let starter_locale = locale.clone();
    let history_locale = locale.clone();
    let history_revision = revision.clone();
    view! {
        <div class="space-y-4">
            <section class="rounded-2xl border border-border bg-card p-5 shadow-sm">
                <div class="flex flex-wrap items-start justify-between gap-4">
                    <div class="min-w-0">
                        <div class="flex flex-wrap items-center gap-2">
                            <h2 class="truncate text-xl font-semibold text-card-foreground">{title}</h2>
                            <span class=format!(
                                "rounded-full px-2 py-1 text-[10px] font-semibold uppercase {}",
                                core::status_badge_class(&page.status)
                            )>{page.status.clone()}</span>
                        </div>
                        <p class="mt-1 text-sm text-muted-foreground">{format!("/{slug}")}</p>
                    </div>
                    <div class="flex flex-wrap gap-2">
                        <button
                            type="button"
                            class="rounded-lg border border-border px-3 py-2 text-sm font-medium hover:bg-muted disabled:opacity-50"
                            disabled=move || action_busy.get().is_some()
                            on:click=move |_| publish_action.run(!is_published)
                        >
                            {if is_published { "Unpublish" } else { "Publish" }}
                        </button>
                        <button
                            type="button"
                            class="rounded-lg border border-border px-3 py-2 text-sm font-medium hover:bg-muted disabled:opacity-50"
                            disabled=move || action_busy.get().is_some()
                            title="Copy this page into a new draft, including its current bodies and channel visibility"
                            on:click=duplicate_action
                        >
                            "Duplicate"
                        </button>
                        <button
                            type="button"
                            class="rounded-lg border border-destructive/40 px-3 py-2 text-sm font-medium text-destructive hover:bg-destructive/10 disabled:opacity-50"
                            disabled=move || action_busy.get().is_some() || is_published
                            title=if is_published {
                                "Unpublish the page before deleting it"
                            } else {
                                "Delete page"
                            }
                            on:click=delete_action
                        >
                            "Delete"
                        </button>
                    </div>
                </div>
                <dl class="mt-4 grid gap-3 text-sm sm:grid-cols-2 xl:grid-cols-5">
                    <MetadataItem label="Locale" value=locale />
                    <MetadataItem label="Template" value=page.template.clone() />
                    <MetadataItem label="Channels" value=channels />
                    <MetadataItem label="Body" value=body_format />
                    <MetadataItem label="Revision" value=revision_label />
                </dl>
                {move || action_error.get().map(|message| view! {
                    <div class="mt-4 rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive" role="alert">
                        {message}
                    </div>
                })}
            </section>


            <SaveStarterFromPage page_id=starter_page_id locale=starter_locale token tenant starter_generation />

            {if is_published && document_readable {
                view! {
                    <div class="rounded-xl border border-amber-300/60 bg-amber-50 px-4 py-3 text-sm text-amber-950" role="status">
                        "Published page: editor saves go to the unpublished body draft. The live page keeps serving the current body until the next publish; unpublishing folds the draft into the current body."
                    </div>
                }.into_any()
            } else {
                view! { <div /> }.into_any()
            }}
            {if document_readable {
                view! {
                    <div class="space-y-4">
                    <section class="rounded-2xl border border-border bg-card p-4 shadow-sm">
                        <PagesFlyBuilder
                            page=page_for_builder
                            baseline
                            release_status
                            provider_status
                            token
                            tenant
                            default_locale
                            draft_token
                            refresh_generation
                        />
                    </section>
                    <BodyRevisionHistory
                        page_id=history_page_id
                        locale=history_locale
                        expected_revision=history_revision
                        token
                        tenant
                        refresh_generation
                    />
                    </div>
                }.into_any()
            } else {
                view! {
                    <section class="rounded-2xl border border-dashed border-border bg-card px-6 py-12 text-center shadow-sm" role="status">
                        <h3 class="text-lg font-semibold text-card-foreground">{document_unavailable_title}</h3>
                        <p class="mx-auto mt-2 max-w-xl text-sm text-muted-foreground">
                            {document_unavailable_message}
                        </p>
                    </section>
                }.into_any()
            }}
            <PublishSchedulePanel
                page_id=page.id.clone()
                token
                tenant
                refresh_generation
            />
        </div>
    }
}

#[component]
fn SaveStarterFromPage(
    page_id: String,
    locale: String,
    token: Signal<Option<String>>,
    tenant: Signal<Option<String>>,
    starter_generation: RwSignal<u64>,
) -> impl IntoView {
    let user = use_current_user();
    let allowed = Signal::derive(move || {
        user.get().is_some_and(|user| {
            crate::access::effective_permissions_grant(&user.permissions, "pages:manage")
        })
    });
    let ui_locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let text = |key, fallback| t(ui_locale.as_deref(), key, fallback);
    let heading = text("pages.starter.saveTitle", "Save saved page as starter");
    let warning = text(
        "pages.starter.savedWarning",
        "Uses the saved working copy in the selected locale, not unsaved editor changes. Existing starter keys cannot be replaced here.",
    );
    let key_label = text("pages.starter.key", "Starter key");
    let name_label = text("pages.starter.name", "Starter name");
    let category_label = text("pages.starter.category", "Category");
    let locale_label = text("pages.starter.sourceLocale", "Source locale");
    let saved_label = text("pages.starter.saved", "Saved from");
    let submit_label = text("pages.starter.saveAction", "Save starter");
    let context_error = text(
        "pages.starter.contextError",
        "Session or tenant changed. Reload and try again.",
    );
    let key = RwSignal::new(String::new());
    let name = RwSignal::new(String::new());
    let category = RwSignal::new("custom".to_string());
    let source_locale = RwSignal::new(locale);
    let busy = RwSignal::new(false);
    let feedback = RwSignal::new(None::<(bool, String)>);
    let submit = Callback::new(move |event: SubmitEvent| {
        event.prevent_default();
        if busy.get_untracked() || !allowed.get_untracked() {
            return;
        }
        let token_value = token.get_untracked();
        let tenant_value = tenant.get_untracked();
        let page_id = page_id.clone();
        let locale = source_locale.get_untracked();
        let key_value = key.get_untracked();
        let name_value = name.get_untracked();
        let category_value = category.get_untracked();
        let saved_label = saved_label.clone();
        let context_error = context_error.clone();
        busy.set(true);
        feedback.set(None);
        spawn_local(async move {
            let result = transport::save_starter_from_page(
                token_value.clone(),
                tenant_value.clone(),
                page_id,
                locale,
                key_value,
                name_value,
                category_value,
            )
            .await;
            if token.get_untracked() != token_value || tenant.get_untracked() != tenant_value {
                feedback.set(Some((false, context_error)));
            } else {
                match result {
                    Ok(snapshot) => {
                        feedback.set(Some((
                            true,
                            format!(
                                "{}: {} / {} / {}",
                                saved_label,
                                snapshot.source_locale,
                                snapshot.source_state,
                                snapshot.source_revision
                            ),
                        )));
                        starter_generation.update(|value| *value = value.wrapping_add(1));
                    }
                    Err(error) => feedback.set(Some((false, error.to_string()))),
                }
            }
            busy.set(false);
        });
    });
    view! {
            <section hidden=move || !allowed.get() class="rounded-2xl border border-border bg-card p-4 shadow-sm">
                <h3 class="font-semibold">{heading.clone()}</h3>
                <p class="mt-1 text-sm text-muted-foreground">{warning.clone()}</p>
                <form class="mt-3 grid gap-3 sm:grid-cols-2" on:submit=move |event| submit.run(event)>
                    <label class="text-sm">{locale_label.clone()}<input class="mt-1 w-full rounded-lg border border-input bg-background px-3 py-2" required prop:value=move || source_locale.get() on:input=move |event| source_locale.set(event_target_value(&event)) /></label>
                    <label class="text-sm">{key_label.clone()}<input class="mt-1 w-full rounded-lg border border-input bg-background px-3 py-2" required prop:value=move || key.get() on:input=move |event| key.set(event_target_value(&event)) /></label>
                    <label class="text-sm">{name_label.clone()}<input class="mt-1 w-full rounded-lg border border-input bg-background px-3 py-2" required prop:value=move || name.get() on:input=move |event| name.set(event_target_value(&event)) /></label>
                    <label class="text-sm">{category_label.clone()}<input class="mt-1 w-full rounded-lg border border-input bg-background px-3 py-2" required prop:value=move || category.get() on:input=move |event| category.set(event_target_value(&event)) /></label>
                    <button type="submit" class="rounded-lg bg-primary px-3 py-2 text-sm text-primary-foreground disabled:opacity-50" disabled=move || busy.get()>{submit_label.clone()}</button>
                </form>
                {move || feedback.get().map(|(ok, message)| view! { <p role=if ok { "status" } else { "alert" } class="mt-2 text-sm">{message}</p> })}
            </section>
    }
}

#[component]
fn MetadataItem(label: &'static str, value: String) -> impl IntoView {
    view! {
        <div class="min-w-0 rounded-lg bg-muted/40 px-3 py-2">
            <dt class="text-xs font-medium uppercase tracking-wide text-muted-foreground">{label}</dt>
            <dd class="mt-1 truncate text-sm text-foreground">{value}</dd>
        </div>
    }
}

#[component]
fn BodyRevisionHistory(
    page_id: String,
    locale: String,
    expected_revision: String,
    token: Signal<Option<String>>,
    tenant: Signal<Option<String>>,
    refresh_generation: RwSignal<u64>,
) -> impl IntoView {
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    // The journal is readable with `pages:publish` too; restoring needs the
    // save grant, so the action is offered only with it.
    let current_user = use_current_user();
    let can_restore = Signal::derive(move || {
        current_user
            .get()
            .is_some_and(|user| pages_can_restore_body_revision(&user.permissions))
    });
    let history_token = token;
    let history_tenant = tenant;
    let history_page_id = page_id.clone();
    let history_locale = locale.clone();
    // The page and locale are part of the source so the resource refetches when the editor moves to
    // another page, exactly as the closure it replaces did when it read them.
    let history = Resource::new(
        move || {
            (
                history_page_id.clone(),
                history_locale.clone(),
                history_token.get(),
                history_tenant.get(),
                refresh_generation.get(),
            )
        },
        move |(page_id, locale, token, tenant, _generation)| async move {
            transport::fetch_page_body_revision_history(token, tenant, page_id, locale)
                .await
                .map_err(|error| error.to_string())
        },
    );

    let restore_page_id = page_id;
    let restore_expected = expected_revision;
    let restore = Callback::new(move |revision_id: String| {
        let page_id = restore_page_id.clone();
        let expected_revision = restore_expected.clone();
        let token = token.get_untracked();
        let tenant = tenant.get_untracked();
        busy.set(true);
        error.set(None);
        spawn_local(async move {
            match transport::restore_page_body_revision(
                token,
                tenant,
                page_id,
                expected_revision,
                revision_id,
            )
            .await
            {
                Ok(_) => {
                    refresh_generation.update(|generation| *generation = generation.wrapping_add(1))
                }
                Err(error_message) => error.set(Some(error_message.to_string())),
            }
            busy.set(false);
        });
    });

    view! {
        <section class="rounded-2xl border border-border bg-card p-4 shadow-sm">
            <div class="mb-3 flex items-center justify-between gap-3">
                <h2 class="font-semibold text-card-foreground">"Body revision history"</h2>
                <span class="text-xs text-muted-foreground">
                    "append-only journal, newest first"
                </span>
            </div>
            {move || error.get().map(|message| view! {
                <div class="mb-3 rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive" role="alert">
                    {message}
                </div>
            })}
            <Suspense fallback=|| view! {
                <div class="space-y-2" aria-label="Loading body revision history">
                    <div class="h-10 animate-pulse rounded-lg bg-muted"></div>
                    <div class="h-10 animate-pulse rounded-lg bg-muted"></div>
                </div>
            }>
                {move || {
                    let can_restore = can_restore.get();
                    history.get().map(|result| match result {
                        Ok(revisions) if revisions.is_empty() => view! {
                            <p class="text-sm text-muted-foreground">
                                "No journaled body revisions for this locale yet."
                            </p>
                        }.into_any(),
                        Ok(revisions) => view! {
                            <ul class="space-y-2">
                                {revisions
                                    .into_iter()
                                    .map(|revision| {
                                        let revision_id = revision.id.clone();
                                        view! {
                                            <li class="flex flex-wrap items-center justify-between gap-3 rounded-lg bg-muted/40 px-3 py-2 text-sm">
                                                <div class="min-w-0">
                                                    <div class="truncate font-medium text-foreground">
                                                        {format!("{} · {}", revision.created_at, revision.source)}
                                                    </div>
                                                    <div class="truncate text-xs text-muted-foreground">
                                                        {format!(
                                                            "revision {} · {}",
                                                            revision.body_revision,
                                                            revision.created_by.as_deref().unwrap_or("system"),
                                                        )}
                                                    </div>
                                                </div>
                                                {can_restore.then(|| view! {
                                                    <button
                                                        type="button"
                                                        class="rounded-lg border border-border px-3 py-1.5 text-xs font-medium hover:bg-muted disabled:opacity-50"
                                                        disabled=move || busy.get()
                                                        title="Restore this body revision into the working copy"
                                                        on:click=move |_| restore.run(revision_id.clone())
                                                    >
                                                        "Restore"
                                                    </button>
                                                })}
                                            </li>
                                        }
                                    })
                                    .collect_view()}
                            </ul>
                        }.into_any(),
                        Err(message) => view! {
                            <div class="rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive" role="alert">
                                {message}
                            </div>
                        }.into_any(),
                    })
                }}
            </Suspense>
        </section>
    }
}

#[component]
fn PublishSchedulePanel(
    page_id: String,
    token: Signal<Option<String>>,
    tenant: Signal<Option<String>>,
    refresh_generation: RwSignal<u64>,
) -> impl IntoView {
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let publish_at_input = RwSignal::new(String::new());
    let schedule_token = token;
    let schedule_tenant = tenant;
    let fetch_page_id = page_id.clone();
    let schedule = Resource::new(
        move || {
            (
                fetch_page_id.clone(),
                schedule_token.get(),
                schedule_tenant.get(),
                refresh_generation.get(),
            )
        },
        move |(page_id, token, tenant, _generation)| async move {
            transport::fetch_page_publish_schedule(token, tenant, page_id)
                .await
                .map_err(|error| error.to_string())
        },
    );

    let schedule_page_id = page_id.clone();
    let schedule_action = Callback::new(move |_| {
        let page_id = schedule_page_id.clone();
        let token = token.get_untracked();
        let tenant = tenant.get_untracked();
        let value = publish_at_input.get_untracked().trim().to_string();
        if value.is_empty() {
            error.set(Some("Choose a publication time first".to_string()));
            return;
        }
        // datetime-local values carry no timezone; the panel documents UTC semantics.
        let publish_at = if value.len() == 16 {
            format!("{value}:00Z")
        } else if value.len() == 19 {
            format!("{value}Z")
        } else {
            value
        };
        busy.set(true);
        error.set(None);
        spawn_local(async move {
            match transport::schedule_page_publish(token, tenant, page_id, publish_at).await {
                Ok(_) => {
                    refresh_generation.update(|generation| *generation = generation.wrapping_add(1))
                }
                Err(failure) => error.set(Some(failure.to_string())),
            }
            busy.set(false);
        });
    });

    let cancel_page_id = page_id;
    let cancel_action = Callback::new(move |_| {
        let page_id = cancel_page_id.clone();
        let token = token.get_untracked();
        let tenant = tenant.get_untracked();
        busy.set(true);
        error.set(None);
        spawn_local(async move {
            match transport::cancel_page_publish(token, tenant, page_id).await {
                Ok(_) => {
                    refresh_generation.update(|generation| *generation = generation.wrapping_add(1))
                }
                Err(failure) => error.set(Some(failure.to_string())),
            }
            busy.set(false);
        });
    });

    view! {
        <section class="rounded-2xl border border-border bg-card p-4 shadow-sm">
            <div class="mb-3 flex items-center justify-between gap-3">
                <h2 class="font-semibold text-card-foreground">"Scheduled publish"</h2>
                <span class="text-xs text-muted-foreground">
                    "the reviewed command captured here is replayed unchanged at the due time (UTC)"
                </span>
            </div>
            {move || error.get().map(|message| view! {
                <div class="mb-3 rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive" role="alert">
                    {message}
                </div>
            })}
            <Suspense fallback=|| view! {
                <div class="h-10 animate-pulse rounded-lg bg-muted" aria-label="Loading publish schedule"></div>
            }>
                {move || {
                    schedule.get().map(|result| match result {
                        Ok(None) => view! {
                            <p class="text-sm text-muted-foreground">
                                "No publication scheduled for this page."
                            </p>
                        }.into_any(),
                        Ok(Some(job)) => {
                            let pending = job.state == "scheduled";
                            let detail = match (&job.last_error_code, &job.last_error_message) {
                                (Some(code), Some(message)) => {
                                    format!("attempts {} · {}: {}", job.attempts, code, message)
                                }
                                _ => format!("attempts {}", job.attempts),
                            };
                            view! {
                                <div class="rounded-lg bg-muted/40 px-3 py-2 text-sm">
                                    <div class="font-medium text-foreground">
                                        {format!("{} · publish at {}", job.state, job.publish_at)}
                                    </div>
                                    <div class="mt-1 text-xs text-muted-foreground">{detail}</div>
                                    <Show when=move || pending>
                                        <button
                                            type="button"
                                            class="mt-2 rounded-lg border border-destructive/40 px-3 py-1.5 text-xs font-medium text-destructive hover:bg-destructive/10 disabled:opacity-50"
                                            disabled=move || busy.get()
                                            title="Cancel the pending scheduled publication"
                                            on:click=move |_| cancel_action.run(())
                                        >
                                            "Cancel schedule"
                                        </button>
                                    </Show>
                                </div>
                            }.into_any()
                        }
                        Err(message) => view! {
                            <div class="rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive" role="alert">
                                {message}
                            </div>
                        }.into_any(),
                    })
                }}
            </Suspense>
            <div class="mt-3 flex flex-wrap items-center gap-2">
                <input
                    type="datetime-local"
                    class="rounded-lg border border-border bg-background px-3 py-2 text-sm"
                    placeholder="Publication time (UTC)"
                    on:input=move |event| publish_at_input.set(event_target_value(&event))
                />
                <button
                    type="button"
                    class="rounded-lg border border-border px-3 py-2 text-sm font-medium hover:bg-muted disabled:opacity-50"
                    disabled=move || busy.get()
                    title="Schedule the reviewed publish of the current page state"
                    on:click=move |_| schedule_action.run(())
                >
                    "Schedule publish"
                </button>
            </div>
        </section>
    }
}

#[component]
fn PagesFlyBuilder(
    page: PageDetail,
    baseline: Option<RuntimeScenarioReleaseBaseline>,
    release_status: PageBuilderScenarioReleaseStatus,
    provider_status: PageBuilderAdminProviderStatus,
    token: Signal<Option<String>>,
    tenant: Signal<Option<String>>,
    default_locale: String,
    draft_token: Option<String>,
    refresh_generation: RwSignal<u64>,
) -> impl IntoView {
    let current_user = use_current_user();
    let seed = core::edit_form_seed_from_page(&page, &default_locale);
    let revision_id = builder::page_revision(&page);
    let project_data = serde_json::from_str::<Value>(&seed.project_data_text)
        .unwrap_or_else(|_| core::default_project_data(&seed.title));
    let generated_context =
        generate_page_builder_runtime_example(PageBuilderRuntimeExampleRequest {
            project_data: project_data.clone(),
            policy: RuntimeContextExamplePolicy::default(),
        })
        .ok()
        .map(|response| response.example.input_context)
        .unwrap_or_else(|| json!({}));
    let scenarios = Arc::new(vec![
        RuntimeContextScenario::new("empty", "Empty", json!({})),
        RuntimeContextScenario::new("generated", "Generated example", generated_context.clone()),
    ]);
    let contribution_assembly = Arc::new(build_pages_admin_contribution_registry(
        &pages_admin_contribution_policy(),
    ));
    let editor_policy = {
        let user = current_user.get();
        let effective_permissions = user
            .as_ref()
            .map(|user| user.permissions.as_slice())
            .unwrap_or(&[]);
        pages_editor_capability_policy(effective_permissions, contribution_assembly.as_ref())
    };
    let draft_owner = current_user
        .get()
        .zip(tenant.get())
        .map(|(user, tenant_slug)| DraftSessionOwner {
            tenant_id: tenant_slug,
            user_id: user.id,
        });
    // Never render a saved page under a draft token that failed to load. The browser
    // would keep sending that token and could otherwise silently seed a new session.
    let route_draft = match load_route_draft(
        pages_browser_draft_store(),
        draft_token.as_deref(),
        &page.id,
        draft_owner.as_ref(),
    ) {
        Ok(Some(draft)) => Some(draft),
        Ok(None) if draft_token.is_none() => None,
        result => {
            let ui_locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
            let label = t(
                ui_locale.as_deref(),
                "pages.builder.draftLoadError",
                "The draft could not be loaded. The editor is stopped to avoid replacing it. Open the saved page in a private window using an address without fly_draft.",
            );
            let message = match result {
                Err(error) => format!("{label}: {error}"),
                Ok(_) => label,
            };
            return view! {
                <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive" role="alert" data-pages-draft-load-error="true">{message}</div>
            }.into_any();
        }
    };
    // Bind browser requests to exactly the snapshot rendered below, not an
    // independently cached generation from sessionStorage.
    provide_context(SsrDraftRenderBinding {
        session: route_draft
            .as_ref()
            .map(|draft| (draft.token.clone(), draft.generation)),
    });
    let stale_draft = route_draft
        .as_ref()
        .is_some_and(|draft| draft.controller.revision_id() != revision_id);
    let recovery_project = if stale_draft {
        route_draft
            .as_ref()
            .map(|draft| draft_recovery_project(&draft.controller))
    } else {
        None
    };
    let (controller, runtime_context) = match route_draft {
        Some(mut draft) => (
            mark_stale_draft_conflict(&mut draft.controller, &revision_id)
                .map(|_| draft.controller),
            draft.runtime_context,
        ),
        None => (
            builder::controller_from_project(&page.id, &revision_id, &seed.project_data_text)
                .map_err(crate::browser_intent::PagesBrowserIntentError::from),
            generated_context,
        ),
    };

    match controller {
        Ok(controller) => {
            let page_id = page.id.clone();
            let browser_endpoint = format!("/api/admin/pages/{page_id}/builder/intents");
            // The preview is served by the host as its own document, so its policy is the policy
            // of the page rather than the policy of this console. The address names the session
            // the editor is showing, so a preview of an edited draft is a preview of that draft.
            // The token is the one the route already put in the address: it is the editor's own
            // session, and the route refuses it unless its owner is the verified caller.
            // A draft token is `[A-Za-z0-9_-]` by construction (`SsrDraftSessionStore` refuses
            // anything else), so it needs no escaping to travel in the address, and the route
            // hands it back to the same owner-checked load the editor used.
            let preview_endpoint = match draft_token.as_deref() {
                Some(token) => format!("/api/admin/pages/{page_id}/preview?draft={token}"),
                None => format!("/api/admin/pages/{page_id}/preview"),
            };
            let snapshot_default_locale = default_locale.clone();
            let facade_token = token;
            let facade_tenant = tenant;
            let facade: Arc<dyn PageBuilderAdminFacade> = Arc::new(
                PagesBuilderFacade::new(
                    move || PagesBuilderSaveSnapshot {
                        token: facade_token.get_untracked(),
                        tenant_slug: facade_tenant.get_untracked(),
                        page_id: page_id.clone(),
                        default_locale: snapshot_default_locale.clone(),
                        // The facade save path is a legacy unattributed write, not a draft
                        // owner: the owner of a draft session is the verified caller of the
                        // intent endpoint (`dispatch_named_intent`), which is the only path
                        // that creates or commits a session. Threading this snapshot's owner
                        // into the facade would add a second, unverified attribution without
                        // changing which sessions can be read or committed. The route draft
                        // above is loaded with the owner built from the current user and the
                        // tenant, so a session belonging to another caller is not restored.
                        owner: None,
                    },
                    move |_page, _project_data| {
                        refresh_generation.update(|generation| {
                            *generation = generation.wrapping_add(1)
                        })
                    },
                )
                .with_provider_status(provider_status),
            );

            let persistence_error = RwSignal::new(None::<String>);
            let server_status = RwSignal::new(release_status);
            let baseline_page_id = page.id.clone();
            let baseline_token = token;
            let baseline_tenant = tenant;
            let on_baseline = Callback::new(
                move |change: PageBuilderScenarioBaselineChange| {
                    let page_id = baseline_page_id.clone();
                    let token = baseline_token.get_untracked();
                    let tenant = baseline_tenant.get_untracked();
                    let expected_baseline_hash =
                        server_status.get_untracked().baseline_hash.clone();
                    let baseline = change.baseline;
                    let promotion_note = change.promotion_note;
                    spawn_local(async move {
                        let write_result = match baseline {
                            Some(baseline) => transport::save_page_builder_scenario_baseline(
                                token.clone(),
                                tenant.clone(),
                                page_id.clone(),
                                baseline,
                                expected_baseline_hash,
                                promotion_note,
                            )
                            .await
                            .map(|_| ()),
                            None => transport::delete_page_builder_scenario_baseline(
                                token.clone(),
                                tenant.clone(),
                                page_id.clone(),
                                expected_baseline_hash,
                            )
                            .await
                            .map(|_| ()),
                        };
                        match write_result {
                            Ok(()) => match transport::fetch_page_builder_scenario_release_status(
                                token,
                                tenant,
                                page_id,
                            )
                            .await
                            {
                                Ok(status) => {
                                    server_status.set(status);
                                    persistence_error.set(None);
                                }
                                Err(error) => persistence_error.set(Some(format!(
                                    "Baseline was written but server status could not be verified: {error}"
                                ))),
                            },
                            Err(error) => persistence_error.set(Some(error.to_string())),
                        }
                    });
                },
            );

            let media_provider_token = token;
            let media_provider_tenant = tenant;
            let asset_provider = transport::pages_media_asset_provider(
                move || media_provider_token.get_untracked(),
                move || media_provider_tenant.get_untracked(),
            );

            let mut host = PageBuilderAdminHostContext::new(controller)
                .with_facade(facade)
                .with_asset_provider(asset_provider)
                .with_contribution_assembly(contribution_assembly)
                .with_editor_capability_policy(editor_policy)
                .with_runtime_context(runtime_context)
                .with_runtime_scenarios(scenarios)
                .with_browser_intent_endpoint(browser_endpoint)
                .with_preview_endpoint(preview_endpoint)
                .on_runtime_scenario_baseline(on_baseline);
            // Derive from the session id, not the access token: the id is stable across
            // refresh rotation, so the embedded CSRF token stays valid for the session. The
            // endpoint that verifies the header uses this same helper, so a token without a
            // session id embeds nothing instead of embedding a value the endpoint would reject.
            if let Some(csrf) = token
                .get_untracked()
                .as_deref()
                .and_then(fly_browser::csrf_secret_from_access_token)
                .map(|session_id| fly_browser::derive_csrf_token(&session_id))
            {
                host = host.with_browser_csrf_token(csrf);
            }
            if let Some(baseline) = baseline {
                host = host.with_runtime_scenario_baseline(baseline);
            }
            provide_context(host);
            view! {
                <div class="space-y-2">
                    {recovery_project.map(|project| {
                        let ui_locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
                        let message = t(ui_locale.as_deref(), "pages.builder.staleDraft", "The saved page changed in another session. This draft is preserved but cannot be saved over the newer page. Copy the project JSON now; it can expire. Open the saved page in a private window using an address without fly_draft and merge your changes manually.");
                        let label = t(ui_locale.as_deref(), "pages.builder.recoveryProject", "Draft project JSON to copy");
                        view! {
                            <section class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive" role="alert" data-pages-stale-draft="true">
                                <p>{message}</p>
                                {match project {
                                    Ok(json) => view! {
                                        <div class="mt-3 font-medium">{label.clone()}</div>
                                        <pre class="mt-1 max-h-96 overflow-auto whitespace-pre-wrap rounded-lg border border-border bg-background p-2 font-mono text-xs text-foreground" tabindex="0" aria-label=label>{json}</pre>
                                    }.into_any(),
                                    Err(error) => view! { <p>{error}</p> }.into_any(),
                                }}
                            </section>
                        }
                    })}
                    <ServerReleaseStatus status=server_status />
                    <PageBuilderAdmin />
                    {move || persistence_error.get().map(|error| view! {
                        <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive" role="alert">
                            {format!("Failed to persist scenario baseline: {error}")}
                        </div>
                    })}
                </div>
            }
            .into_any()
        }
        Err(error) => view! {
            <div class="rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive" role="alert">
                {error.to_string()}
            </div>
        }
        .into_any(),
    }
}

/// Export the complete draft project, not only the currently selected Fly page.
fn draft_recovery_project(
    controller: &rustok_page_builder_admin::AdminCanvasController,
) -> Result<String, String> {
    let project = fly::GrapesJsCodec::encode_value(controller.editor().document())
        .map_err(|error| format!("Could not encode draft project: {error}"))?;
    serde_json::to_string_pretty(&project)
        .map_err(|error| format!("Could not serialize draft project: {error}"))
}

/// The draft session the route names, if the store still holds it.
///
/// `Ok(None)` means the route names no draft or the session is gone. An error is returned to
/// the caller instead of being read as "no draft": the editor would otherwise open the saved
/// page in silence while the store is down or the session belongs to another user.
fn load_route_draft(
    store: &dyn SsrDraftSessionStore,
    draft_token: Option<&str>,
    page_id: &str,
    owner: Option<&DraftSessionOwner>,
) -> Result<Option<SsrDraftSessionSnapshot>, SsrDraftSessionError> {
    match draft_token {
        Some(token) => store.load(token, page_id, owner),
        None => Ok(None),
    }
}

#[component]
fn ServerReleaseStatus(status: RwSignal<PageBuilderScenarioReleaseStatus>) -> impl IntoView {
    let class_status = status;
    let text_status = status;
    view! {
        <div
            class=move || {
                let status = class_status.get();
                if !status.allowed || status.status == "broken" || status.status == "baseline_invalid" {
                    "rounded-xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive"
                } else if status.status == "requires_review" {
                    "rounded-xl border border-amber-300/50 bg-amber-50 px-4 py-3 text-sm text-amber-900"
                } else {
                    "rounded-xl border border-border bg-muted/40 px-4 py-3 text-sm text-muted-foreground"
                }
            }
            role="status"
        >
            {move || {
                let status = text_status.get();
                format!(
                    "Server release gate: {} · allowed={} · {} visual · {} breaking{}",
                    status.status,
                    status.allowed,
                    status.visual_changes,
                    status.breaking_changes,
                    status
                        .baseline_hash
                        .as_deref()
                        .map(|hash| format!(" · baseline {hash}"))
                        .unwrap_or_default(),
                )
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_page_builder_admin::{AdminCanvasController, InMemorySsrDraftSessionStore};

    fn owner(user_id: &str) -> DraftSessionOwner {
        DraftSessionOwner {
            tenant_id: "tenant".to_string(),
            user_id: user_id.to_string(),
        }
    }

    fn controller() -> AdminCanvasController {
        AdminCanvasController::new(
            "home",
            "rev-1",
            json!({
                "pages": [{
                    "id": "home",
                    "component": { "id": "root", "type": "wrapper" }
                }]
            }),
        )
        .expect("controller")
    }

    #[test]
    fn workspace_resource_payload_round_trips_without_inventing_provider_health() {
        use crate::model::PageBody;
        use rustok_page_builder::health::{ProviderHealthSnapshot, ProviderSloObservations};
        use rustok_page_builder::rollout::BuilderCapabilityFlags;
        use rustok_page_builder_admin::PageBuilderAdminProviderState;

        let mut data = PagesWorkspaceData {
            page: PageDetail {
                id: "home".to_string(),
                version: 2,
                status: "draft".to_string(),
                template: "default".to_string(),
                updated_at: "2026-11-13T00:00:00Z".to_string(),
                available_locales: vec!["en".to_string()],
                channel_slugs: vec!["web".to_string()],
                translation: None,
                body: Some(PageBody {
                    locale: "en".to_string(),
                    content: String::new(),
                    format: "grapesjs".to_string(),
                    content_json: Some(json!({"pages": [{"id": "home"}]})),
                    updated_at: "2026-11-13T00:00:00Z".to_string(),
                    state: "draft".to_string(),
                }),
                document_readable: true,
            },
            baseline: None,
            release_status: PageBuilderScenarioReleaseStatus {
                page_id: "home".to_string(),
                baseline_present: false,
                allowed: false,
                status: "pending".to_string(),
                baseline_id: None,
                baseline_hash: None,
                previous_baseline_hash: None,
                promoted_by: None,
                promotion_note: None,
                promoted_at: None,
                visual_changes: 0,
                breaking_changes: 0,
                diagnostics: json!([]),
            },
            provider_flags: BuilderCapabilityFlags::default(),
            provider_health: None,
        };
        let unavailable = ProviderHealthSnapshot::evaluate(ProviderSloObservations {
            preview_p95_ms: 1_000,
            publish_p95_ms: 1_000,
            sanitize_failure_rate: 0.0,
            runtime_error_rate: 0.03,
        });
        for (health, state) in [
            (None, PageBuilderAdminProviderState::Unobserved),
            (
                Some(unavailable),
                PageBuilderAdminProviderState::Unavailable,
            ),
        ] {
            data.provider_health = health.clone();
            let encoded = serde_json::to_vec(&data).expect("serialize resource payload");
            let restored: PagesWorkspaceData =
                serde_json::from_slice(&encoded).expect("deserialize resource payload");
            assert_eq!(
                serde_json::to_value(&restored.page).unwrap(),
                serde_json::to_value(&data.page).unwrap()
            );
            assert_eq!(restored.release_status, data.release_status);
            assert_eq!(restored.provider_flags, data.provider_flags);
            assert_eq!(restored.provider_health, health);
            assert_eq!(restored.provider_status().state(), state);
        }
    }

    #[test]
    fn stale_draft_recovery_exports_the_entire_project_without_rebasing() {
        let mut controller = AdminCanvasController::new(
            "home",
            "rev-1",
            json!({"pages": [
                {"id": "home", "component": {"id": "root", "type": "wrapper"}},
                {"id": "about", "component": {"id": "about-root", "type": "wrapper"}}
            ]}),
        )
        .expect("controller");
        let before = controller.editor().revision().project_hash;
        assert!(mark_stale_draft_conflict(&mut controller, "rev-2").unwrap());
        let export: Value =
            serde_json::from_str(&draft_recovery_project(&controller).unwrap()).unwrap();
        assert_eq!(export["pages"].as_array().unwrap().len(), 2);
        assert_eq!(controller.editor().revision().project_hash, before);
        assert_eq!(controller.revision_id(), "rev-1");
    }

    #[test]
    fn route_draft_reports_a_refused_session_instead_of_no_draft() {
        let store = InMemorySsrDraftSessionStore::editor_default();
        let author = owner("author");
        let session = store
            .commit(None, None, controller(), Some(&author))
            .expect("draft session");

        let restored = load_route_draft(&store, Some(&session.token), "home", Some(&author))
            .expect("the author's own draft")
            .expect("live session");
        assert_eq!(restored.token, session.token);

        let reader = owner("reader");
        let error = load_route_draft(&store, Some(&session.token), "home", Some(&reader))
            .expect_err("the draft of another user");
        assert!(
            matches!(error, SsrDraftSessionError::OwnerMismatch),
            "unexpected error: {error}"
        );

        let error = load_route_draft(&store, Some(&session.token), "about", Some(&author))
            .expect_err("the draft of another page");
        assert!(
            matches!(error, SsrDraftSessionError::PageMismatch { .. }),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn route_without_a_live_draft_is_not_an_error() {
        let store = InMemorySsrDraftSessionStore::editor_default();

        let without_token = load_route_draft(&store, None, "home", None).expect("no token");
        assert!(without_token.is_none());
        let unknown = load_route_draft(&store, Some("non-existent-token"), "home", None)
            .expect("a token the store does not hold");
        assert!(unknown.is_none());
    }
}
