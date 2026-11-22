use leptos::prelude::*;
use rustok_auth_ui::ui::leptos::hooks::{use_auth, use_tenant, use_token};
use rustok_grid::{ColumnFilters, GridPagination, RowSelection};
use rustok_grid_leptos::DataGrid;
use rustok_ui_core::UiRouteContext;

use crate::core::{
    FeedbackTone, IndexAdminOverviewViewModel, IndexSchemaRowViewModel, IndexSourceRowViewModel,
    IndexTableRowViewModel, build_index_admin_overview_view_model, filter_index_schemas,
    filter_index_sources, filter_index_tables, format_cancel_action_result,
    format_index_admin_bootstrap_error, format_replay_action_result, format_retry_action_result,
    index_schema_grid_columns, index_source_grid_columns, index_table_grid_columns,
    source_catalog_empty_message,
};
use crate::i18n::t;
use crate::model::{CancelJobInput, RetryJobInput, RetryJobKind, TriggerReplayInput};
use crate::transport;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AdminTab {
    Overview,
    Schemas,
    Storage,
    Operations,
}

/// A completed action can refresh the snapshot only for the request scope that started it.
/// Refresh on both typed success and failure: network errors do not prove that no write committed.
fn finish_scoped_action<S: Eq, T>(
    request: &S,
    current: Option<&S>,
    result: T,
    apply: impl FnOnce(T),
    refresh: impl FnOnce(),
) {
    if current == Some(request) {
        apply(result);
        refresh();
    }
}

/// Builds a resource the response resolves.
///
/// The module admin mounts inside the layout route that declares `SsrMode::Async`, so a
/// `LocalResource` here is `pending()` on the server and the reader is served the `Suspense`
/// fallback. See `DECISIONS/2026-11-13-standalone-admin-editor-delivery.md`.
fn loaded_resource<S, Fut, T>(
    source: impl Fn() -> S + Send + Sync + 'static,
    fetcher: impl Fn(S) -> Fut + Send + Sync + 'static,
) -> Resource<T>
where
    S: PartialEq + Clone + Send + Sync + 'static,
    Fut: std::future::Future<Output = T> + Send + 'static,
    T: serde::Serialize + for<'de> serde::Deserialize<'de> + Send + Sync + 'static,
{
    Resource::new(source, fetcher)
}

#[component]
pub fn IndexAdmin() -> impl IntoView {
    let auth = use_auth();
    let auth_scope = Memo::new(move |_| auth.client_scope());
    let token = use_token();
    let tenant = use_tenant();
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;

    let index_bootstrap = loaded_resource(
        move || (auth_scope.get().generation, token.get(), tenant.get()),
        |(generation, token, tenant)| async move {
            // Only the credential-free generation is serialized with the resource result.
            (generation, transport::fetch_bootstrap(token, tenant).await)
        },
    );

    let (active_tab, set_active_tab) = signal(AdminTab::Overview);
    let (action_message, set_action_message) = signal::<Option<(FeedbackTone, String)>>(None);
    let (is_busy, set_is_busy) = signal(false);
    let (cancel_job_id, set_cancel_job_id) = signal(String::new());
    let retry_kind = RwSignal::new(RetryJobKind::Replay);
    let (retry_job_id, set_retry_job_id) = signal(String::new());
    let (retry_reason, set_retry_reason) = signal(String::new());

    // Never use a disposed route's signals or a session that no longer matches browser authority.
    let current_action_scope = move || {
        let scope = auth_scope.try_get_untracked()?;
        if !scope.is_ready() {
            return None;
        }
        let token = token.try_get_untracked().flatten()?;
        let tenant = tenant.try_get_untracked().flatten()?;
        transport::is_current_scope(&token, &tenant).then_some((scope, token, tenant))
    };

    Effect::new(move |_| {
        let _ = auth_scope.get();
        set_action_message.set(None);
        set_cancel_job_id.set(String::new());
        retry_kind.set(RetryJobKind::Replay);
        set_retry_job_id.set(String::new());
        set_retry_reason.set(String::new());
    });

    let on_rebuild = {
        let locale = locale.clone();
        move |generation: u64, schema_module: String, schema_entity: String, schema_version: u32| {
            let Some(request) = current_action_scope() else {
                return;
            };
            if request.0.generation != generation || is_busy.try_get_untracked() != Some(false) {
                return;
            }
            let locale_clone = locale.clone();
            set_is_busy.set(true);
            set_action_message.set(None);
            leptos::task::spawn_local(async move {
                if current_action_scope().as_ref() != Some(&request) {
                    set_is_busy.try_set(false);
                    return;
                }
                let input = TriggerReplayInput {
                    schema_module,
                    schema_entity,
                    schema_version,
                    locale: None,
                };
                let result = transport::trigger_replay(
                    Some(request.1.clone()),
                    Some(request.2.clone()),
                    input,
                )
                .await;
                finish_scoped_action(
                    &request,
                    current_action_scope().as_ref(),
                    result,
                    |result| match result {
                        Ok(res) => {
                            set_action_message.set(Some(format_replay_action_result(
                                locale_clone.as_deref(),
                                &res,
                            )));
                        }
                        Err(err) => {
                            set_action_message.set(Some((FeedbackTone::Error, err.to_string())));
                        }
                    },
                    || index_bootstrap.refetch(),
                );
                set_is_busy.try_set(false);
            });
        }
    };

    let on_cancel = {
        let locale = locale.clone();
        move |generation: u64, job_id: String| {
            let Some(request) = current_action_scope() else {
                return;
            };
            if request.0.generation != generation || is_busy.try_get_untracked() != Some(false) {
                return;
            }
            let locale_clone = locale.clone();
            set_is_busy.set(true);
            set_action_message.set(None);
            leptos::task::spawn_local(async move {
                if current_action_scope().as_ref() != Some(&request) {
                    set_is_busy.try_set(false);
                    return;
                }
                let input = CancelJobInput { job_id };
                let result =
                    transport::cancel_job(Some(request.1.clone()), Some(request.2.clone()), input)
                        .await;
                finish_scoped_action(
                    &request,
                    current_action_scope().as_ref(),
                    result,
                    |result| match result {
                        Ok(res) => {
                            set_action_message.set(Some(format_cancel_action_result(
                                locale_clone.as_deref(),
                                &res,
                            )));
                        }
                        Err(err) => {
                            set_action_message.set(Some((FeedbackTone::Error, err.to_string())));
                        }
                    },
                    || index_bootstrap.refetch(),
                );
                set_is_busy.try_set(false);
            });
        }
    };

    let on_retry = {
        let locale = locale.clone();
        move |generation: u64, input: RetryJobInput| {
            let Some(request) = current_action_scope() else {
                return;
            };
            if request.0.generation != generation || is_busy.try_get_untracked() != Some(false) {
                return;
            }
            let locale_clone = locale.clone();
            set_is_busy.set(true);
            set_action_message.set(None);
            leptos::task::spawn_local(async move {
                if current_action_scope().as_ref() != Some(&request) {
                    set_is_busy.try_set(false);
                    return;
                }
                let result =
                    transport::retry_job(Some(request.1.clone()), Some(request.2.clone()), input)
                        .await;
                finish_scoped_action(
                    &request,
                    current_action_scope().as_ref(),
                    result,
                    |result| match result {
                        Ok(res) => {
                            set_action_message.set(Some(format_retry_action_result(
                                locale_clone.as_deref(),
                                &res,
                            )));
                        }
                        Err(err) => {
                            set_action_message.set(Some((FeedbackTone::Error, err.to_string())));
                        }
                    },
                    || index_bootstrap.refetch(),
                );
                set_is_busy.try_set(false);
            });
        }
    };

    view! {
        <div class="space-y-6">
            <header class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <div class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
                    <div class="space-y-2">
                        <div class="flex items-center gap-2">
                            <span class="inline-flex items-center rounded-full border border-border px-3 py-0.5 text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                                {t(locale.as_deref(), "index.badge", "index")}
                            </span>
                            <span class="inline-flex items-center rounded-full bg-emerald-500/10 px-2.5 py-0.5 text-xs font-medium text-emerald-600 dark:text-emerald-400 border border-emerald-500/20">
                                "PostgreSQL JSONB"
                            </span>
                        </div>
                        <h1 class="text-2xl font-bold tracking-tight text-card-foreground">
                            {t(locale.as_deref(), "index.title", "Index Engine")}
                        </h1>
                        <p class="max-w-3xl text-sm text-muted-foreground">
                            {t(locale.as_deref(), "index.subtitle", "Platform-owned cross-module relational index and query engine.")}
                        </p>
                    </div>
                    <button
                        type="button"
                        disabled=move || is_busy.get()
                        class="self-start rounded-lg border border-border px-3 py-1.5 text-sm hover:bg-muted disabled:opacity-50"
                        on:click=move |_| index_bootstrap.refetch()
                    >
                        {t(locale.as_deref(), "index.action.refresh", "Refresh status")}
                    </button>
                </div>
            </header>

            // Action Feedback Banner
            {
                let banner_locale = locale.clone();
                move || action_message.get().map(|(tone, msg)| {
                    let (bg_class, text_class, border_class) = match tone {
                        FeedbackTone::Success => ("bg-emerald-500/10", "text-emerald-600 dark:text-emerald-400", "border-emerald-500/20"),
                        FeedbackTone::Neutral => ("bg-muted", "text-muted-foreground", "border-border"),
                        FeedbackTone::Error => ("bg-destructive/10", "text-destructive", "border-destructive/20"),
                    };
                    view! {
                        <div role="status" class=format!("flex items-center justify-between rounded-xl border {border_class} {bg_class} px-4 py-3 text-sm {text_class} shadow-sm transition-all")>
                            <div class="flex items-center gap-2">
                                <span class="font-semibold">{t(banner_locale.as_deref(), "index.action.status", "Action Status")}:</span>
                                <span>{msg}</span>
                            </div>
                            <button
                                type="button"
                                class="text-xs font-semibold underline hover:opacity-80 transition-opacity ml-4"
                                on:click=move |_| set_action_message.set(None)
                            >
                                {t(banner_locale.as_deref(), "index.action.dismiss", "Dismiss")}
                            </button>
                        </div>
                    }
                })
            }

            <Suspense fallback=move || view! { <div class="h-48 animate-pulse rounded-2xl bg-muted"></div> }>
                {move || {
                    let tab_locale = locale.clone();
                    let scope = auth_scope.get();
                    let current_tenant = tenant.get();
                    index_bootstrap.get().filter(|(generation, result)| {
                        scope.is_ready() && scope.generation == *generation
                            && result.as_ref().map_or(true, |bootstrap| {
                                current_tenant.as_deref() == Some(bootstrap.tenant.slug.as_str())
                            })
                    }).map(|(generation, result)| match result {
                        Ok(bootstrap) => {
                            let vm = build_index_admin_overview_view_model(
                                tab_locale.as_deref(),
                                bootstrap,
                            );
                            view! {
                                <div class="space-y-6">
                                    // Top Stat Cards
                                    <section class="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
                                        {vm
                                            .stat_cards
                                            .iter()
                                            .take(4)
                                            .cloned()
                                            .map(|card| {
                                                view! {
                                                    <div class="rounded-2xl border border-border bg-card p-5 shadow-sm transition-colors hover:border-primary/40">
                                                        <div class="text-xs font-medium uppercase tracking-wider text-muted-foreground">{card.label}</div>
                                                        <div class="mt-2 text-lg font-bold text-card-foreground">{card.value}</div>
                                                        {card.hint.map(|h| view! { <div class="mt-1 text-xs text-muted-foreground">{h}</div> })}
                                                    </div>
                                                }
                                            })
                                            .collect_view()}
                                    </section>

                                    // Tab Navigation
                                    <div class="flex border-b border-border space-x-1">
                                        <button
                                            type="button"
                                            class=move || {
                                                if active_tab.get() == AdminTab::Overview {
                                                    "border-b-2 border-primary px-4 py-2 text-sm font-semibold text-primary transition-colors"
                                                } else {
                                                    "px-4 py-2 text-sm font-medium text-muted-foreground hover:text-card-foreground transition-colors"
                                                }
                                            }
                                            on:click=move |_| set_active_tab.set(AdminTab::Overview)
                                        >
                                            {t(tab_locale.as_deref(), "index.tab.overview", "Overview")}
                                        </button>
                                        <button
                                            type="button"
                                            class=move || {
                                                if active_tab.get() == AdminTab::Schemas {
                                                    "border-b-2 border-primary px-4 py-2 text-sm font-semibold text-primary transition-colors"
                                                } else {
                                                    "px-4 py-2 text-sm font-medium text-muted-foreground hover:text-card-foreground transition-colors"
                                                }
                                            }
                                            on:click=move |_| set_active_tab.set(AdminTab::Schemas)
                                        >
                                            {t(tab_locale.as_deref(), "index.tab.schemas", "Schemas Catalog")}
                                            <span class="ml-2 inline-flex items-center rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">
                                                {vm.schemas.len()}
                                            </span>
                                        </button>
                                        <button
                                            type="button"
                                            class=move || {
                                                if active_tab.get() == AdminTab::Storage {
                                                    "border-b-2 border-primary px-4 py-2 text-sm font-semibold text-primary transition-colors"
                                                } else {
                                                    "px-4 py-2 text-sm font-medium text-muted-foreground hover:text-card-foreground transition-colors"
                                                }
                                            }
                                            on:click=move |_| set_active_tab.set(AdminTab::Storage)
                                        >
                                            {t(tab_locale.as_deref(), "index.tab.storage", "Storage Foundation")}
                                            <span class="ml-2 inline-flex items-center rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">
                                                {vm.tables.len()}
                                            </span>
                                        </button>
                                        <button
                                            type="button"
                                            class=move || {
                                                if active_tab.get() == AdminTab::Operations {
                                                    "border-b-2 border-primary px-4 py-2 text-sm font-semibold text-primary transition-colors"
                                                } else {
                                                    "px-4 py-2 text-sm font-medium text-muted-foreground hover:text-card-foreground transition-colors"
                                                }
                                            }
                                            on:click=move |_| set_active_tab.set(AdminTab::Operations)
                                        >
                                            {t(tab_locale.as_deref(), "index.tab.operations", "Replay & Operations")}
                                        </button>
                                    </div>

                                    // Tab Content
                                    {
                                        let vm_clone = vm.clone();
                                        let locale_str = tab_locale.clone();
                                        let on_rebuild = on_rebuild.clone();
                                        let on_cancel = on_cancel.clone();
                                        let on_retry = on_retry.clone();
                                        move || match active_tab.get() {
                                            AdminTab::Overview => view_overview(locale_str.as_deref(), &vm_clone),
                                            AdminTab::Schemas => view_schemas(locale_str.clone(), &vm_clone, is_busy, {
                                                let on_rebuild = on_rebuild.clone();
                                                move |module, entity, version| on_rebuild(generation, module, entity, version)
                                            }),
                                            AdminTab::Storage => view_storage(locale_str.as_deref(), &vm_clone),
                                            AdminTab::Operations => view_operations(
                                                locale_str.clone(),
                                                &vm_clone,
                                                is_busy,
                                                cancel_job_id,
                                                set_cancel_job_id,
                                                {
                                                    let on_cancel = on_cancel.clone();
                                                    move |job_id| on_cancel(generation, job_id)
                                                },
                                                retry_kind,
                                                retry_job_id,
                                                set_retry_job_id,
                                                retry_reason,
                                                set_retry_reason,
                                                {
                                                    let on_retry = on_retry.clone();
                                                    move |input| on_retry(generation, input)
                                                },
                                            ),
                                        }
                                    }
                                </div>
                            }
                            .into_any()
                        }
                        Err(err) => view! {
                            <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-5 py-4 text-sm text-destructive">
                                {format_index_admin_bootstrap_error(tab_locale.as_deref(), err)}
                            </div>
                        }
                        .into_any(),
                    })
                }}
            </Suspense>
        </div>
    }
}

fn view_overview(locale: Option<&str>, vm: &IndexAdminOverviewViewModel) -> AnyView {
    view! {
        <div class="space-y-6">
            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm">
                <h2 class="text-lg font-semibold text-card-foreground">
                    {t(locale, "index.contract.title", "Engine Architecture & Contract")}
                </h2>
                <p class="mt-2 text-sm leading-relaxed text-muted-foreground">
                    {vm.module_description.clone()}
                </p>

                <div class="mt-6 grid gap-4 sm:grid-cols-3">
                    <div class="rounded-xl border border-border/70 bg-muted/40 p-4">
                        <div class="text-xs font-semibold text-muted-foreground uppercase">{t(locale, "index.info.engine", "Storage Engine")}</div>
                        <div class="mt-1 text-sm font-medium text-card-foreground">{vm.engine_type.clone()}</div>
                    </div>
                    <div class="rounded-xl border border-border/70 bg-muted/40 p-4">
                        <div class="text-xs font-semibold text-muted-foreground uppercase">{t(locale, "index.info.version", "Module Version")}</div>
                        <div class="mt-1 text-sm font-medium text-card-foreground">{vm.module_version.clone()}</div>
                    </div>
                    <div class="rounded-xl border border-border/70 bg-muted/40 p-4">
                        <div class="text-xs font-semibold text-muted-foreground uppercase">{t(locale, "index.info.partitionStatus", "Partition Policy")}</div>
                        <div class="mt-1 text-sm font-medium text-card-foreground font-mono text-xs">{vm.partition_status.clone()}</div>
                    </div>
                </div>
            </section>
        </div>
    }
    .into_any()
}

fn view_schemas<F>(
    locale: Option<String>,
    vm: &IndexAdminOverviewViewModel,
    is_busy: ReadSignal<bool>,
    on_rebuild: F,
) -> AnyView
where
    F: Fn(String, String, u32) + Clone + Send + Sync + 'static,
{
    let is_ru = locale
        .as_deref()
        .map(|l| l.starts_with("ru"))
        .unwrap_or(false);
    let columns = index_schema_grid_columns(locale.as_deref());
    let search = RwSignal::new(String::new());
    let filters = RwSignal::new(ColumnFilters::new());
    let selection = RwSignal::new(RowSelection::new());
    let pagination = RwSignal::new(GridPagination::new(1, 10, vm.schemas.len() as u64));

    let all_schemas = vm.schemas.clone();
    let filtered_schemas = Memo::new({
        let all_schemas = all_schemas.clone();
        move |_| {
            let s_val = search.get();
            let search_term = if s_val.trim().is_empty() {
                None
            } else {
                Some(s_val.as_str())
            };
            let list = filter_index_schemas(&all_schemas, &filters.get(), search_term);
            pagination.update(|p| p.total = list.len() as u64);
            list
        }
    });

    let paged_schemas = Memo::new(move |_| {
        let list = filtered_schemas.get();
        let p = pagination.get();
        let start = (p.page.saturating_sub(1)) * p.page_size;
        list.into_iter()
            .skip(start)
            .take(p.page_size)
            .collect::<Vec<_>>()
    });

    let on_filters_change = Callback::new(move |new_filters: ColumnFilters| {
        filters.set(new_filters);
    });

    let cell_locale = locale.clone();
    let cell_on_rebuild = on_rebuild.clone();

    let cell_renderer = Callback::new(
        move |(schema, col_id): (IndexSchemaRowViewModel, String)| match col_id.as_str() {
            "name" => {
                let name = schema.qualified_name.clone();
                view! {
                    <span class="font-semibold text-card-foreground">{name}</span>
                }
                .into_any()
            }
            "version" => {
                let version = schema.version.clone();
                view! {
                    <span class="inline-flex items-center rounded-full bg-primary/10 px-2.5 py-0.5 text-xs font-medium text-primary">
                        {version}
                    </span>
                }
                .into_any()
            }
            "fingerprint" => {
                let fp = schema.fingerprint.clone();
                view! {
                    <span class="font-mono text-xs text-muted-foreground">{fp}</span>
                }
                .into_any()
            }
            "fields" => {
                let count = schema.fields_count.clone();
                view! {
                    <span class="text-card-foreground font-mono">{count}</span>
                }
                .into_any()
            }
            "links" => {
                let count = schema.links_count.clone();
                view! {
                    <span class="text-card-foreground font-mono">{count}</span>
                }
                .into_any()
            }
            "owner" => {
                let owner = schema.owner_module.clone();
                view! {
                    <span class="text-xs font-mono text-muted-foreground">{owner}</span>
                }
                .into_any()
            }
            "readiness" => {
                view! { <span class="text-xs">{schema.readiness.clone()}</span> }.into_any()
            }
            "actions" => {
                let module = schema.module.clone();
                let entity = schema.entity.clone();
                let version = schema.raw_version;
                let can_replay = schema.can_replay;
                let on_rebuild = cell_on_rebuild.clone();
                let btn_locale = cell_locale.clone();
                view! {
                    <div class="flex items-center justify-end">
                        <button
                            type="button"
                            disabled=move || is_busy.get() || !can_replay
                            class="inline-flex items-center gap-1.5 rounded-lg border border-primary/20 bg-primary/10 px-3 py-1.5 text-xs font-semibold text-primary shadow-sm hover:bg-primary/20 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                            on:click={
                                let on_rebuild = on_rebuild.clone();
                                let module = module.clone();
                                let entity = entity.clone();
                                move |_| on_rebuild(module.clone(), entity.clone(), version)
                            }
                        >
                            {
                                let btn_locale = btn_locale.clone();
                                move || {
                                    if is_busy.get() {
                                        t(btn_locale.as_deref(), "index.action.rebuilding", "Rebuilding...")
                                    } else {
                                        t(btn_locale.as_deref(), "index.action.rebuild", "Rebuild")
                                    }
                                }
                            }
                        </button>
                    </div>
                }
                .into_any()
            }
            _ => ().into_any(),
        },
    );

    let empty_msg = t(
        locale.as_deref(),
        "index.schema.empty",
        "No schemas registered for this tenant yet.",
    )
    .to_string();

    view! {
        <section class="rounded-2xl border border-border bg-card shadow-sm overflow-hidden p-6 space-y-4">
            <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4 border-b border-border pb-4">
                <div>
                    <h2 class="text-lg font-semibold text-card-foreground">
                        {t(locale.as_deref(), "index.tab.schemas", "Schemas Catalog")}
                    </h2>
                    <p class="text-sm text-muted-foreground mt-0.5">
                        {if is_ru { "Каталог зарегистрированных схем сущностей для поиска и аналитики." } else { "Catalog of registered entity schemas for search and relational analytics." }}
                    </p>
                </div>
            </div>

            // Search filter input
            <div class="flex flex-col sm:flex-row gap-4">
                <div class="flex-1">
                    <input
                        type="text"
                        placeholder={if is_ru { "Поиск по схеме, владельцу, хэшу..." } else { "Search by schema, owner, fingerprint..." }}
                        class="w-full px-3 py-2 border rounded-xl shadow-sm focus:ring-primary focus:border-primary text-sm bg-background border-border text-foreground placeholder:text-muted-foreground"
                        prop:value=move || search.get()
                        on:input=move |ev| search.set(event_target_value(&ev))
                    />
                </div>
            </div>

            // Selection toolbar
            <Show when=move || !selection.get().is_empty()>
                <div class="flex items-center justify-between gap-3 px-4 py-2.5 rounded-xl border border-primary/20 bg-primary/5 text-sm">
                    <div class="flex items-center gap-2">
                        <span class="font-medium text-foreground">
                            {move || format!("{} {} {}", selection.get().count(), if is_ru { "выбрано" } else { "selected" }, if is_ru { "схем" } else { "schemas" })}
                        </span>
                    </div>
                    <div class="flex items-center gap-2">
                        <button
                            type="button"
                            class="h-6 px-2.5 rounded-lg text-xs text-muted-foreground hover:text-foreground transition border border-border bg-background"
                            on:click=move |_| selection.update(|s| s.clear())
                        >
                            {if is_ru { "Снять выбор" } else { "Clear" }}
                        </button>
                    </div>
                </div>
            </Show>

            // Modern DataGrid
            <DataGrid
                columns=columns
                data=Signal::derive(move || paged_schemas.get())
                key_fn=IndexSchemaRowViewModel::key
                cell_renderer=cell_renderer
                is_loading=Signal::derive(move || is_busy.get())
                empty_message=empty_msg
                selection=selection
                pagination=pagination
                filters=filters
                on_filter_change=on_filters_change
                on_row_click=Callback::new(|_| ())
            />
        </section>
    }
    .into_any()
}

fn view_storage(locale: Option<&str>, vm: &IndexAdminOverviewViewModel) -> AnyView {
    let is_ru = locale.map(|l| l.starts_with("ru")).unwrap_or(false);
    let columns = index_table_grid_columns(locale);
    let search = RwSignal::new(String::new());
    let filters = RwSignal::new(ColumnFilters::new());
    let selection = RwSignal::new(RowSelection::new());
    let pagination = RwSignal::new(GridPagination::new(1, 20, vm.tables.len() as u64));

    let all_tables = vm.tables.clone();
    let filtered_tables = Memo::new({
        let all_tables = all_tables.clone();
        move |_| {
            let s_val = search.get();
            let search_term = if s_val.trim().is_empty() {
                None
            } else {
                Some(s_val.as_str())
            };
            let list = filter_index_tables(&all_tables, &filters.get(), search_term);
            pagination.update(|p| p.total = list.len() as u64);
            list
        }
    });

    let paged_tables = Memo::new(move |_| {
        let list = filtered_tables.get();
        let p = pagination.get();
        let start = (p.page.saturating_sub(1)) * p.page_size;
        list.into_iter()
            .skip(start)
            .take(p.page_size)
            .collect::<Vec<_>>()
    });

    let cell_renderer = Callback::new(move |(table, col_id): (IndexTableRowViewModel, String)| {
        match col_id.as_str() {
            "name" => view! {
                <span class="font-mono text-xs font-semibold text-card-foreground">
                    {table.name}
                </span>
            }
            .into_any(),
            "role" => view! {
                <span class="text-sm text-muted-foreground">
                    {table.role}
                </span>
            }
            .into_any(),
            _ => ().into_any(),
        }
    });

    view! {
        <section class="rounded-2xl border border-border bg-card p-6 shadow-sm space-y-4">
            <div class="flex flex-col md:flex-row md:items-center justify-between gap-4">
                <div>
                    <h2 class="text-lg font-semibold text-card-foreground">
                        {t(locale, "index.tab.storage", "Storage Foundation")}
                    </h2>
                    <p class="text-xs text-muted-foreground mt-0.5">
                        {if is_ru { "Системные таблицы и структуры данных индексатора" } else { "System tables and partition layout for search indexing" }}
                    </p>
                </div>
                <div class="w-full md:w-64">
                    <input
                        type="text"
                        placeholder={if is_ru { "Поиск таблиц..." } else { "Search tables..." }}
                        class="w-full px-3 py-1.5 text-xs rounded-xl border border-border bg-background text-foreground shadow-sm focus:ring-1 focus:ring-primary outline-none"
                        prop:value=move || search.get()
                        on:input=move |ev| search.set(event_target_value(&ev))
                    />
                </div>
            </div>

            <Show when=move || !selection.get().is_empty()>
                <div class="flex items-center justify-between gap-3 px-4 py-2 rounded-xl border border-primary/20 bg-primary/5 text-xs">
                    <span class="font-medium text-foreground">
                        {move || format!("{} {} {}", selection.get().count(), if is_ru { "выбрано" } else { "selected" }, if is_ru { "таблиц" } else { "tables" })}
                    </span>
                    <button
                        type="button"
                        class="h-6 px-2 rounded-md text-xs text-muted-foreground hover:text-foreground border border-border bg-background"
                        on:click=move |_| selection.update(|s| s.clear())
                    >
                        {if is_ru { "Снять выбор" } else { "Clear" }}
                    </button>
                </div>
            </Show>

            <DataGrid
                columns=columns
                data=Signal::derive(move || paged_tables.get())
                key_fn=|table: &IndexTableRowViewModel| table.name.clone()
                cell_renderer=cell_renderer
                empty_message={t(locale, "index.storage.empty", "No storage tables found.").to_string()}
                selection=selection
                pagination=pagination
                filters=filters
                on_filter_change=Callback::new(move |f| filters.set(f))
                on_row_click=Callback::new(|_| ())
            />
        </section>
    }
    .into_any()
}

fn view_operations<F, G>(
    locale: Option<String>,
    vm: &IndexAdminOverviewViewModel,
    is_busy: ReadSignal<bool>,
    cancel_job_id: ReadSignal<String>,
    set_cancel_job_id: WriteSignal<String>,
    on_cancel: F,
    retry_kind: RwSignal<RetryJobKind>,
    retry_job_id: ReadSignal<String>,
    set_retry_job_id: WriteSignal<String>,
    retry_reason: ReadSignal<String>,
    set_retry_reason: WriteSignal<String>,
    on_retry: G,
) -> AnyView
where
    F: Fn(String) + Clone + 'static,
    G: Fn(RetryJobInput) + Clone + 'static,
{
    let empty_locale = locale.clone();
    let btn_locale = locale.clone();
    let retry_btn_locale = locale.clone();
    let is_ru = locale
        .as_deref()
        .map(|l| l.starts_with("ru"))
        .unwrap_or(false);
    let source_columns = index_source_grid_columns(locale.as_deref());
    let source_search = RwSignal::new(String::new());
    let source_filters = RwSignal::new(ColumnFilters::new());
    let source_selection = RwSignal::new(RowSelection::new());
    let can_manage_jobs = vm.can_manage_jobs;
    let source_catalog_available = vm.source_catalog_available;
    let source_pagination = RwSignal::new(GridPagination::new(1, 20, vm.sources.len() as u64));

    let all_sources = vm.sources.clone();
    let filtered_sources = Memo::new({
        let all_sources = all_sources.clone();
        move |_| {
            let s_val = source_search.get();
            let search_term = if s_val.trim().is_empty() {
                None
            } else {
                Some(s_val.as_str())
            };
            let list = filter_index_sources(&all_sources, &source_filters.get(), search_term);
            source_pagination.update(|p| p.total = list.len() as u64);
            list
        }
    });

    let paged_sources = Memo::new(move |_| {
        let list = filtered_sources.get();
        let p = source_pagination.get();
        let start = (p.page.saturating_sub(1)) * p.page_size;
        list.into_iter()
            .skip(start)
            .take(p.page_size)
            .collect::<Vec<_>>()
    });

    let source_cell_renderer = Callback::new(
        move |(src, col_id): (IndexSourceRowViewModel, String)| {
            match col_id.as_str() {
            "name" => view! {
                <span class="font-mono text-xs font-semibold text-card-foreground">
                    {src.name}
                </span>
            }
            .into_any(),
            "entity" => view! {
                <span class="text-sm text-card-foreground">
                    {src.entity}
                </span>
            }
            .into_any(),
            "owner" => view! {
                <span class="inline-flex items-center rounded-full bg-muted px-2.5 py-0.5 text-xs font-medium text-muted-foreground font-mono">
                    {src.owner_module}
                </span>
            }
            .into_any(),
            _ => ().into_any(),
        }
        },
    );

    view! {
        <div class="space-y-6">
            // Operator Controls: Cancel
            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm space-y-6">
                <div class="border-b border-border pb-4">
                    <h2 class="text-lg font-semibold text-card-foreground">
                        {t(locale.as_deref(), "index.action.operatorControls", "Operator Controls")}
                    </h2>
                    <p class="mt-1 text-sm text-muted-foreground">
                        {t(locale.as_deref(), "index.action.operatorControlsDesc", "Trigger bounded replay runs, cancel or retry existing jobs.")}
                    </p>
                    {vm.operator_unavailable.clone().map(|message| view! {
                        <p class="mt-2 text-sm text-muted-foreground">{message}</p>
                    })}
                </div>

                // Cancel Job Form
                <div class="grid gap-4 sm:grid-cols-3 items-end">
                    <div class="sm:col-span-2 space-y-1.5">
                        <label class="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                            {t(locale.as_deref(), "index.action.jobId", "Job ID (UUID)")}
                        </label>
                        <input
                            type="text"
                            placeholder={t(locale.as_deref(), "index.action.jobIdPlaceholder", "Enter Job UUID to cancel")}
                            prop:value=move || cancel_job_id.get()
                            on:input=move |ev| set_cancel_job_id.set(event_target_value(&ev))
                            class="w-full rounded-xl border border-border bg-background px-4 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary font-mono"
                        />
                    </div>
                    <div>
                        <button
                            type="button"
                            disabled=move || !can_manage_jobs || is_busy.get() || cancel_job_id.get().trim().is_empty()
                            class="w-full inline-flex items-center justify-center rounded-xl bg-destructive/10 text-destructive border border-destructive/20 px-4 py-2 text-sm font-semibold hover:bg-destructive/20 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                            on:click={
                                let on_cancel = on_cancel.clone();
                                move |_| {
                                    let id = cancel_job_id.get().trim().to_string();
                                    if !id.is_empty() {
                                        on_cancel(id);
                                    }
                                }
                            }
                        >
                            {
                                let btn_locale = btn_locale.clone();
                                move || {
                                    if is_busy.get() {
                                        t(btn_locale.as_deref(), "index.action.cancelling", "Cancelling...")
                                    } else {
                                        t(btn_locale.as_deref(), "index.action.cancel", "Cancel replay")
                                    }
                                }
                            }
                        </button>
                    </div>
                </div>
            </section>

            // Retry / Requeue Operator Form
            <section class="rounded-2xl border border-amber-500/20 bg-amber-500/5 p-6 shadow-sm space-y-6">
                <div class="border-b border-amber-500/20 pb-4">
                    <h2 class="text-lg font-semibold text-card-foreground">
                        {t(locale.as_deref(), "index.action.retryControls", "Retry / Requeue Failed Job")}
                    </h2>
                    <p class="mt-1 text-sm text-muted-foreground">
                        {t(locale.as_deref(), "index.action.retryControlsDesc", "Requeue the original failed replay or reconciliation job. A reason is required.")}
                    </p>
                </div>

                <div class="grid gap-4 sm:grid-cols-3">
                    <label class="space-y-1.5">
                        <span class="block text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                            {t(locale.as_deref(), "index.action.retryKind", "Job kind")}
                        </span>
                        <select
                            disabled=move || !can_manage_jobs || is_busy.get()
                            prop:value=move || retry_kind.get().as_str()
                            on:change=move |event| match event_target_value(&event).as_str() {
                                "replay" => retry_kind.set(RetryJobKind::Replay),
                                "reconciliation" => retry_kind.set(RetryJobKind::Reconciliation),
                                _ => (),
                            }
                            class="w-full rounded-xl border border-border bg-background px-4 py-2 text-sm text-foreground"
                        >
                            <option value="replay">{t(locale.as_deref(), "index.action.retryKindReplay", "Replay")}</option>
                            <option value="reconciliation">{t(locale.as_deref(), "index.action.retryKindReconciliation", "Reconciliation")}</option>
                        </select>
                    </label>
                    <div class="space-y-1.5">
                        <label class="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                            {t(locale.as_deref(), "index.action.retryJobId", "Job ID to Retry")}
                        </label>
                        <input
                            type="text"
                            placeholder={t(locale.as_deref(), "index.action.retryJobIdPlaceholder", "Enter Job UUID to retry / requeue")}
                            prop:value=move || retry_job_id.get()
                            on:input=move |ev| set_retry_job_id.set(event_target_value(&ev))
                            class="w-full rounded-xl border border-border bg-background px-4 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary font-mono"
                        />
                    </div>
                    <div class="space-y-1.5">
                        <label class="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                            {t(locale.as_deref(), "index.action.retryReason", "Reason (required)")}
                        </label>
                        <input
                            type="text"
                            placeholder={t(locale.as_deref(), "index.action.retryReasonPlaceholder", "Reason for manual retry")}
                            prop:value=move || retry_reason.get()
                            on:input=move |ev| set_retry_reason.set(event_target_value(&ev))
                            class="w-full rounded-xl border border-border bg-background px-4 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
                        />
                    </div>
                </div>
                <div>
                    <button
                        type="button"
                        disabled=move || !can_manage_jobs || is_busy.get() || retry_job_id.get().trim().is_empty() || retry_reason.get().trim().is_empty()
                        class="inline-flex items-center justify-center rounded-xl bg-amber-500/10 text-amber-700 dark:text-amber-400 border border-amber-500/30 px-5 py-2 text-sm font-semibold hover:bg-amber-500/20 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                        on:click={
                            let on_retry = on_retry.clone();
                            move |_| {
                                let id = retry_job_id.get().trim().to_string();
                                let reason = retry_reason.get().trim().to_string();
                                if !id.is_empty() && !reason.is_empty() {
                                    on_retry(RetryJobInput { job_id: id, kind: retry_kind.get(), reason });
                                }
                            }
                        }
                    >
                        {
                            let retry_btn_locale = retry_btn_locale.clone();
                            move || {
                                if is_busy.get() {
                                    t(retry_btn_locale.as_deref(), "index.action.retrying", "Retrying...")
                                } else {
                                    t(retry_btn_locale.as_deref(), "index.action.retry", "Retry / Requeue Job")
                                }
                            }
                        }
                    </button>
                </div>
            </section>

            // Inbox Queue & Lag metrics
            <section class="space-y-3">
                <h3 class="text-sm font-semibold text-muted-foreground uppercase tracking-wider">
                    {t(locale.as_deref(), "index.inbox.title", "Inbox Queue & Lag")}
                </h3>
                <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
                    {vm.inbox_summary.iter().cloned().map(|card| view! {
                        <div class="rounded-2xl border border-border bg-card p-5 shadow-sm">
                            <div class="text-xs font-medium uppercase tracking-wider text-muted-foreground">{card.label}</div>
                            <div class="mt-2 text-lg font-bold text-card-foreground font-mono">{card.value}</div>
                            {card.hint.map(|h| view! { <div class="mt-1 text-xs text-amber-600 dark:text-amber-400 font-mono">{h}</div> })}
                        </div>
                    }).collect_view()}
                </div>
            </section>

            // Jobs & Recovery metrics
            <section class="space-y-3">
                <h3 class="text-sm font-semibold text-muted-foreground uppercase tracking-wider">
                    {t(locale.as_deref(), "index.jobs.title", "Jobs & Recovery")}
                </h3>
                <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
                    {vm.job_summary.iter().cloned().map(|card| view! {
                        <div class="rounded-2xl border border-border bg-card p-5 shadow-sm">
                            <div class="text-xs font-medium uppercase tracking-wider text-muted-foreground">{card.label}</div>
                            <div class="mt-2 text-lg font-bold text-card-foreground font-mono">{card.value}</div>
                        </div>
                    }).collect_view()}
                </div>
            </section>

            <section class="rounded-2xl border border-border bg-card p-6 space-y-3">
                <h2 class="text-lg font-semibold">{t(locale.as_deref(), "index.jobs.recent", "Recent Replay and Reconciliation Jobs")}</h2>
                <ul class="space-y-2">
                    {vm.recent_jobs.iter().cloned().map(|job| view! {
                        <li class="rounded-lg border border-border px-3 py-2 text-sm">
                            <div class="font-medium">
                                {job.schema} " · " {job.locale.unwrap_or_else(|| t(locale.as_deref(), "index.jobs.schemaWide", "All locales"))}
                                " · " {job.kind} " · " {job.state.clone()}
                            </div>
                            <div class="font-mono text-xs text-muted-foreground">{job.job_id}</div>
                            <div class="flex flex-wrap gap-2 text-xs text-muted-foreground">
                                <span>{t(locale.as_deref(), "index.jobs.failures", "Failures")} ": " {job.failure_count}</span>
                                {job.available_at.map(|at| view! { <span>{t(locale.as_deref(), "index.jobs.availableAt", "Available at")} ": " {at}</span> })}
                                {(job.state == "running" && job.cancel_requested).then(|| view! {
                                    <span>{t(locale.as_deref(), "index.jobs.cancelling", "Cancellation requested")}</span>
                                })}
                                <span>{job.last_error_code}</span>
                            </div>
                        </li>
                    }).collect_view()}
                </ul>
            </section>

            // Query Diagnostics metrics
            <section class="space-y-3">
                <h3 class="text-sm font-semibold text-muted-foreground uppercase tracking-wider">
                    {t(locale.as_deref(), "index.query.title", "Query Diagnostics")}
                </h3>
                <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
                    {vm.query_diagnostics_summary.iter().cloned().map(|card| view! {
                        <div class="rounded-2xl border border-border bg-card p-5 shadow-sm">
                            <div class="text-xs font-medium uppercase tracking-wider text-muted-foreground">{card.label}</div>
                            <div class="mt-2 text-sm font-bold text-card-foreground font-mono">{card.value}</div>
                        </div>
                    }).collect_view()}
                </div>
            </section>

            // Operations summary cards
            <section class="grid gap-4 sm:grid-cols-3">
                {vm.operations_summary
                    .iter()
                    .cloned()
                    .map(|card| {
                        view! {
                            <div class="rounded-2xl border border-border bg-card p-5 shadow-sm">
                                <div class="text-xs font-medium uppercase tracking-wider text-muted-foreground">{card.label}</div>
                                <div class="mt-2 text-sm font-bold text-card-foreground font-mono">{card.value}</div>
                            </div>
                        }
                    })
                    .collect_view()}
            </section>

            // Registered Replay Sources DataGrid
            <section class="rounded-2xl border border-border bg-card p-6 shadow-sm space-y-4">
                <div class="flex flex-col md:flex-row md:items-center justify-between gap-4">
                    <div>
                        <h2 class="text-lg font-semibold text-card-foreground">
                            {t(locale.as_deref(), "index.source.title", "Selected Replay Sources")}
                        </h2>
                        <p class="text-xs text-muted-foreground mt-0.5">
                            {if is_ru { "Зарегистрированные источники реплея и стриминга" } else { "Registered streaming and snapshot replay pipelines" }}
                        </p>
                    </div>
                    <div class="w-full md:w-64">
                        <input
                            type="text"
                            placeholder={if is_ru { "Поиск источников..." } else { "Search sources..." }}
                            class="w-full px-3 py-1.5 text-xs rounded-xl border border-border bg-background text-foreground shadow-sm focus:ring-1 focus:ring-primary outline-none"
                            prop:value=move || source_search.get()
                            on:input=move |ev| source_search.set(event_target_value(&ev))
                        />
                    </div>
                </div>

                <Show when=move || !source_selection.get().is_empty()>
                    <div class="flex items-center justify-between gap-3 px-4 py-2 rounded-xl border border-primary/20 bg-primary/5 text-xs">
                        <span class="font-medium text-foreground">
                            {move || format!("{} {} {}", source_selection.get().count(), if is_ru { "выбрано" } else { "selected" }, if is_ru { "источников" } else { "sources" })}
                        </span>
                        <button
                            type="button"
                            class="h-6 px-2 rounded-md text-xs text-muted-foreground hover:text-foreground border border-border bg-background"
                            on:click=move |_| source_selection.update(|s| s.clear())
                        >
                            {if is_ru { "Снять выбор" } else { "Clear" }}
                        </button>
                    </div>
                </Show>

                <DataGrid
                    columns=source_columns
                    data=Signal::derive(move || paged_sources.get())
                    key_fn=IndexSourceRowViewModel::key
                    cell_renderer=source_cell_renderer
                    empty_message={source_catalog_empty_message(empty_locale.as_deref(), source_catalog_available)}
                    selection=source_selection
                    pagination=source_pagination
                    filters=source_filters
                    on_filter_change=Callback::new(move |f| source_filters.set(f))
                    on_row_click=Callback::new(|_| ())
                />
            </section>
        </div>
    }
    .into_any()
}

#[cfg(test)]
mod refresh_tests {
    use std::cell::Cell;

    use super::finish_scoped_action;

    #[test]
    fn completed_action_refreshes_once_but_not_after_scope_changes() {
        let refreshed = Cell::new(0);
        let displayed = Cell::new(None);
        let apply = |result| displayed.set(Some(result));
        let refresh = || refreshed.set(refreshed.get() + 1);
        finish_scoped_action(&"first", Some(&"first"), "retry", apply, refresh);
        finish_scoped_action(&"first", Some(&"second"), "stale result", apply, refresh);
        finish_scoped_action(&"first", None, "no active scope", apply, refresh);
        assert_eq!(refreshed.get(), 1);
        assert_eq!(displayed.get(), Some("retry"));
    }
}
