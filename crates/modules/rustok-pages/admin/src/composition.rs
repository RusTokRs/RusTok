use crate::access::pages_editor_capability_policy;
use crate::browser_intent::pages_browser_draft_store;
use crate::builder::{self, PagesBuilderFacade, PagesBuilderSaveSnapshot};
use crate::contributions::{
    build_pages_admin_contribution_registry, pages_admin_contribution_policy,
};
use crate::core;
use crate::i18n::t;
use crate::model::{PageBuilderScenarioReleaseStatus, PageDetail, PageList};
use crate::transport;
use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_current_user, use_tenant, use_token};
use leptos_ui_routing::{use_route_query_value, use_route_query_writer};
use rustok_page_builder::runtime_context::{
    PageBuilderRuntimeExampleRequest, generate_page_builder_runtime_example,
};
use rustok_page_builder::runtime_scenario_release::{
    PageBuilderScenarioBaselineChange, RuntimeScenarioReleaseBaseline,
};
use rustok_page_builder::{RuntimeContextExamplePolicy, RuntimeContextScenario};
use rustok_page_builder_admin::{
    PageBuilderAdmin, PageBuilderAdminFacade, PageBuilderAdminHostContext,
    PageBuilderAdminProviderStatus, SsrDraftSessionStore,
};
use rustok_ui_core::{AdminQueryKey, UiRouteContext};
use serde_json::{Value, json};
use std::sync::Arc;

const FLY_DRAFT_QUERY_KEY: &str = "fly_draft";

#[component]
pub fn PagesAdmin() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let selected_page_query = use_route_query_value(AdminQueryKey::PageId.as_str());
    let draft_query = use_route_query_value(FLY_DRAFT_QUERY_KEY);
    let query_writer = use_route_query_writer();
    let token = use_token();
    let tenant = use_tenant();
    let refresh_generation = RwSignal::new(0_u64);
    let default_locale = route_context
        .locale
        .clone()
        .unwrap_or_else(|| "en".to_string());

    let list_token = token;
    let list_tenant = tenant;
    let pages_resource = LocalResource::new(move || {
        let token = list_token.get();
        let tenant = list_tenant.get();
        let _generation = refresh_generation.get();
        async move { transport::fetch_pages(token, tenant).await }
    });

    let workspace_token = token;
    let workspace_tenant = tenant;
    let workspace_resource = LocalResource::new(move || {
        let page_id = selected_page_query.get();
        let token = workspace_token.get();
        let tenant = workspace_tenant.get();
        let _generation = refresh_generation.get();
        async move {
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
            let provider_status =
                crate::builder_rollout_settings::fetch_pages_builder_rollout_snapshot(
                    token, tenant,
                )
                .await
                .map_err(|error| error.to_string())?
                .provider_status();
            Ok(Some((page, baseline, release_status, provider_status)))
        }
    });

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

    view! {
        <div class="space-y-6">
            <header class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="flex flex-wrap items-start justify-between gap-4">
                    <div>
                        <h1 class="text-2xl font-semibold text-card-foreground">{title}</h1>
                        <p class="mt-2 max-w-3xl text-sm text-muted-foreground">{subtitle}</p>
                    </div>
                    <button
                        type="button"
                        class="rounded-lg border border-border px-3 py-2 text-sm font-medium hover:bg-muted"
                        on:click=move |_| clear_writer.clear_key(AdminQueryKey::PageId.as_str())
                    >
                        "New page"
                    </button>
                </div>
            </header>

            <div class="grid gap-6 xl:grid-cols-[20rem_minmax(0,1fr)]">
                <aside class="space-y-4">
                    <CreatePageCard refresh_generation default_locale=default_locale.clone() />
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
                                                writer.replace_value(AdminQueryKey::PageId.as_str(), page_id)
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
                                Ok(Some((page, baseline, release_status, provider_status))) => view! {
                                    <PageWorkspace
                                        page
                                        baseline
                                        release_status
                                        provider_status
                                        token
                                        tenant
                                        default_locale=default_locale.clone()
                                        draft_token=draft_query.get()
                                        refresh_generation
                                    />
                                }.into_any(),
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
        </div>
    }
}

#[component]
fn CreatePageCard(refresh_generation: RwSignal<u64>, default_locale: String) -> impl IntoView {
    let token = use_token();
    let tenant = use_tenant();
    let query_writer = use_route_query_writer();
    let title = RwSignal::new(String::new());
    let slug = RwSignal::new(String::new());
    let slug_touched = RwSignal::new(false);
    let locale = RwSignal::new(default_locale);
    let channel_slugs = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);

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
        let draft = core::build_create_page_draft(
            core::PageDraftFormInput {
                locale: &locale_value,
                title: &title_value,
                slug: &slug_value,
                channel_slugs: &channels_value,
            },
            core::default_project_data(&title_value),
        );
        if let Some(field) = core::missing_required_page_field(&draft) {
            error.set(Some(format!("Required page field is missing: {field:?}")));
            return;
        }

        busy.set(true);
        error.set(None);
        let token = token.get_untracked();
        let tenant = tenant.get_untracked();
        let writer = query_writer.clone();
        spawn_local(async move {
            match transport::create_page(token, tenant, draft).await {
                Ok(page) => {
                    title.set(String::new());
                    slug.set(String::new());
                    slug_touched.set(false);
                    channel_slugs.set(String::new());
                    refresh_generation
                        .update(|generation| *generation = generation.wrapping_add(1));
                    writer.replace_value(AdminQueryKey::PageId.as_str(), page.id);
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
            <form class="mt-4 space-y-3" on:submit=submit>
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
                {move || error.get().map(|message| view! {
                    <div class="rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive" role="alert">
                        {message}
                    </div>
                })}
                <button
                    type="submit"
                    class="w-full rounded-lg bg-primary px-3 py-2 text-sm font-medium text-primary-foreground disabled:opacity-50"
                    disabled=move || busy.get()
                >
                    {move || if busy.get() { "Creating..." } else { "Create and open" }}
                </button>
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
    let body_format = page
        .body
        .as_ref()
        .map(|body| body.format.clone())
        .unwrap_or_else(|| rustok_page_builder::PAGE_BUILDER_DOCUMENT_FORMAT.to_string());
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
                    writer.clear_key(AdminQueryKey::PageId.as_str());
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
                    writer.replace_value(AdminQueryKey::PageId.as_str(), duplicated.id.clone());
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
                    <MetadataItem label="Revision" value=revision />
                </dl>
                {move || action_error.get().map(|message| view! {
                    <div class="mt-4 rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive" role="alert">
                        {message}
                    </div>
                })}
            </section>


            {if is_published {
                view! {
                    <div class="rounded-xl border border-amber-300/60 bg-amber-50 px-4 py-3 text-sm text-amber-950" role="status">
                        "Published page: editor saves go to the unpublished body draft. The live page keeps serving the current body until the next publish; unpublishing folds the draft into the current body."
                    </div>
                }.into_any()
            } else {
                view! { <div /> }.into_any()
            }}
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
    let history_token = token;
    let history_tenant = tenant;
    let history_page_id = page_id.clone();
    let history_locale = locale.clone();
    let history = LocalResource::new(move || {
        let page_id = history_page_id.clone();
        let locale = history_locale.clone();
        let token = history_token.get();
        let tenant = history_tenant.get();
        let _generation = refresh_generation.get();
        async move {
            transport::fetch_page_body_revision_history(token, tenant, page_id, locale)
                .await
                .map_err(|error| error.to_string())
        }
    });

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
                                        let restore = restore.clone();
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
                                                <button
                                                    type="button"
                                                    class="rounded-lg border border-border px-3 py-1.5 text-xs font-medium hover:bg-muted disabled:opacity-50"
                                                    disabled=move || busy.get()
                                                    title="Restore this body revision into the working copy"
                                                    on:click=move |_| restore.run(revision_id.clone())
                                                >
                                                    "Restore"
                                                </button>
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
    let schedule = LocalResource::new(move || {
        let page_id = fetch_page_id.clone();
        let token = schedule_token.get();
        let tenant = schedule_tenant.get();
        let _generation = refresh_generation.get();
        async move {
            transport::fetch_page_publish_schedule(token, tenant, page_id)
                .await
                .map_err(|error| error.to_string())
        }
    });

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
                    on:click=schedule_action
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
        pages_editor_capability_policy(
            user.as_ref().map(|user| user.role.as_str()),
            contribution_assembly.as_ref(),
        )
    };
    let restored_draft = draft_token
        .as_deref()
        .and_then(|token| {
            pages_browser_draft_store()
                .load(token, &page.id)
                .ok()
                .flatten()
        })
        .filter(|draft| draft.controller.revision_id() == revision_id);
    let (controller, runtime_context) = match restored_draft {
        Some(draft) => (Ok(draft.controller), draft.runtime_context),
        None => (
            builder::controller_from_project(&page.id, &revision_id, &seed.project_data_text),
            generated_context,
        ),
    };

    match controller {
        Ok(controller) => {
            let page_id = page.id.clone();
            let browser_endpoint = format!("/api/admin/pages/{page_id}/builder/intents");
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
                .on_runtime_scenario_baseline(on_baseline);
            if let Some(baseline) = baseline {
                host = host.with_runtime_scenario_baseline(baseline);
            }
            provide_context(host);
            view! {
                <div class="space-y-2">
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
