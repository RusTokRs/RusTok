use leptos::ev::{KeyboardEvent, MouseEvent};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_auth::hooks::{use_tenant, use_token};
use leptos_router::components::A;
use leptos_router::hooks::{use_location, use_navigate};
use leptos_router::NavigateOptions;
use leptos_use::use_debounce_fn;
use serde::{Deserialize, Serialize};

use crate::features::auth::UserMenu;
use crate::shared::api::queries::ADMIN_GLOBAL_SEARCH_QUERY;
use crate::shared::api::request;
use crate::shared::ui::LanguageToggle;
use crate::{t_string, use_i18n};

#[derive(Clone, Copy, PartialEq)]
struct Breadcrumb {
    label_key: &'static str,
    href: Option<&'static str>,
}

#[derive(Clone, Debug, Deserialize)]
struct AdminGlobalSearchResponse {
    #[serde(rename = "adminGlobalSearch")]
    admin_global_search: AdminGlobalSearchPayload,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AdminGlobalSearchPayload {
    items: Vec<AdminGlobalSearchItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AdminGlobalSearchItem {
    id: String,
    #[serde(rename = "entityType")]
    entity_type: String,
    #[serde(rename = "sourceModule")]
    source_module: String,
    title: String,
    snippet: Option<String>,
    score: f64,
    locale: Option<String>,
    #[serde(default)]
    url: Option<String>,
    payload: String,
}

#[derive(Clone, Debug, Serialize)]
struct AdminGlobalSearchVariables {
    input: AdminGlobalSearchInput,
}

#[derive(Clone, Debug, Serialize)]
struct AdminGlobalSearchInput {
    query: String,
    limit: i32,
    offset: i32,
}

#[cfg(feature = "ssr")]
const MAX_ADMIN_SEARCH_QUERY_LEN: usize = 256;

#[server(prefix = "/api/fn", endpoint = "admin/global-search")]
async fn admin_global_search_native(
    query: String,
    limit: i32,
    offset: i32,
) -> Result<AdminGlobalSearchPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{has_effective_permission, AuthContext, TenantContext};
        use rustok_core::Permission;
        use std::time::Instant;

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ) {
            return Err(ServerFnError::new("settings:read required"));
        }

        let query = normalize_admin_search_query(&query)?;
        let limit = limit.clamp(1, 20) as usize;
        let offset = offset.max(0) as usize;
        let transform =
            rustok_search::SearchDictionaryService::transform_query(&app_ctx.db, tenant.id, &query)
                .await
                .map_err(ServerFnError::new)?;
        let settings =
            rustok_search::SearchSettingsService::load_effective(&app_ctx.db, Some(tenant.id))
                .await
                .map_err(ServerFnError::new)?;
        let ranking_profile = rustok_search::SearchRankingProfile::resolve(
            &settings.config,
            "admin_global_search",
            None,
            None,
        )
        .map_err(|err| ServerFnError::new(err.to_string()))?;

        let search_query = rustok_search::SearchQuery {
            tenant_id: Some(tenant.id),
            locale: None,
            original_query: transform.original_query,
            query: transform.effective_query,
            ranking_profile,
            preset_key: None,
            limit,
            offset,
            published_only: false,
            entity_types: Vec::new(),
            source_modules: Vec::new(),
            statuses: Vec::new(),
        };
        let engine = rustok_search::PgSearchEngine::new(app_ctx.db.clone());
        let started_at = Instant::now();
        let result = rustok_search::SearchEngine::search(&engine, search_query.clone()).await;
        let result = match result {
            Ok(result) => {
                rustok_search::SearchDictionaryService::apply_query_rules(
                    &app_ctx.db,
                    &search_query,
                    result,
                )
                .await
            }
            Err(error) => Err(error),
        }
        .map_err(ServerFnError::new)?;

        let query_log_id = record_admin_search_query_log(
            &app_ctx.db,
            &search_query,
            result.engine.as_str(),
            result.total,
            result.took_ms.max(started_at.elapsed().as_millis() as u64),
        )
        .await;

        Ok(AdminGlobalSearchPayload {
            items: result
                .items
                .into_iter()
                .map(|item| {
                    let url = derive_admin_search_result_url(&item);
                    AdminGlobalSearchItem {
                        id: item.id.to_string(),
                        entity_type: item.entity_type,
                        source_module: item.source_module,
                        title: item.title,
                        snippet: item.snippet,
                        score: item.score,
                        locale: item.locale,
                        url,
                        payload: serde_json::json!({
                            "queryLogId": query_log_id,
                            "payload": item.payload,
                        })
                        .to_string(),
                    }
                })
                .collect(),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (query, limit, offset);
        Err(ServerFnError::new(
            "admin/global-search requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn normalize_admin_search_query(value: &str) -> Result<String, ServerFnError> {
    let trimmed = value.trim();
    if trimmed.len() > MAX_ADMIN_SEARCH_QUERY_LEN {
        return Err(ServerFnError::new(format!(
            "Search query exceeds the maximum length of {MAX_ADMIN_SEARCH_QUERY_LEN} characters"
        )));
    }
    if trimmed.chars().any(|ch| ch.is_control()) {
        return Err(ServerFnError::new(
            "Search query contains unsupported control characters",
        ));
    }
    Ok(trimmed.to_string())
}

#[cfg(feature = "ssr")]
async fn record_admin_search_query_log(
    db: &sea_orm::DatabaseConnection,
    search_query: &rustok_search::SearchQuery,
    engine: &str,
    result_count: u64,
    took_ms: u64,
) -> Option<i64> {
    let tenant_id = search_query.tenant_id?;
    let engine = rustok_search::SearchEngineKind::try_from_str(engine)?;

    rustok_search::SearchAnalyticsService::record_query(
        db,
        rustok_search::SearchQueryLogRecord {
            tenant_id,
            surface: "admin_global_search".to_string(),
            query: search_query.original_query.clone(),
            locale: search_query.locale.clone(),
            engine,
            result_count,
            took_ms,
            status: "success".to_string(),
            entity_types: search_query.entity_types.clone(),
            source_modules: search_query.source_modules.clone(),
            statuses: search_query.statuses.clone(),
        },
    )
    .await
    .ok()
    .flatten()
}

#[cfg(feature = "ssr")]
fn derive_admin_search_result_url(item: &rustok_search::SearchResultItem) -> Option<String> {
    match item.entity_type.as_str() {
        "node" => {
            let module_slug = if item.source_module.trim().is_empty() {
                "content"
            } else {
                item.source_module.as_str()
            };
            Some(format!("/modules/{module_slug}?id={}", item.id))
        }
        "product" => Some(format!("/modules/search/playground?focusId={}", item.id)),
        _ => None,
    }
}

#[component]
pub fn Header() -> impl IntoView {
    let i18n = use_i18n();
    let location = use_location();

    let breadcrumbs = Memo::new(move |_| resolve_breadcrumbs(&location.pathname.get()));
    let title_key = Memo::new(move |_| resolve_title_key(&location.pathname.get()));

    Effect::new(move |_| {
        let title = format!(
            "{} вЂ” {}",
            t_string!(i18n, app.brand.title),
            resolve_label(i18n, title_key.get())
        );
        set_document_title(&title);
    });

    view! {
        <header class="h-14 bg-background border-b border-border flex items-center justify-between px-6 shrink-0">
            <div class="flex items-center gap-2 text-sm text-muted-foreground">
                <span class="font-medium text-foreground">{t_string!(i18n, app.brand.title)}</span>
                <span>"/"</span>
                {{
                    let crumbs = breadcrumbs.get();
                    let last_index = crumbs.len().saturating_sub(1);
                    crumbs
                        .into_iter()
                        .enumerate()
                        .map(|(index, crumb)| {
                            let label_key = crumb.label_key;
                            let is_last = index == last_index;
                            let content = if let Some(href) = crumb.href {
                                view! { <A href=href attr:class="hover:text-foreground transition-colors">{move || resolve_label(i18n, label_key)}</A> }.into_any()
                            } else {
                                view! { <span class="text-foreground">{move || resolve_label(i18n, label_key)}</span> }.into_any()
                            };
                            view! {
                                <div class="flex items-center gap-2">
                                    {content}
                                    <Show when=move || !is_last>
                                        <span>"/"</span>
                                    </Show>
                                </div>
                            }
                        })
                        .collect_view()
                }}
            </div>

            <div class="flex items-center gap-3">
                <HeaderGlobalSearch />
                <LanguageToggle />
                <UserMenu />
            </div>
        </header>
    }
}

#[component]
fn HeaderGlobalSearch() -> impl IntoView {
    let i18n = use_i18n();
    let token = use_token();
    let tenant = use_tenant();
    let navigate = use_navigate();

    let (query, set_query) = signal(String::new());
    let (debounced_query, set_debounced_query) = signal(String::new());
    let (results, set_results) = signal(Vec::<AdminGlobalSearchItem>::new());
    let (is_open, set_is_open) = signal(false);
    let (is_loading, set_is_loading) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);
    let request_seq = RwSignal::new(0_u64);

    let debounce_search = use_debounce_fn(
        move || set_debounced_query.set(query.get_untracked()),
        180.0,
    );

    Effect::new(move |_| {
        let _ = query.get();
        debounce_search();
    });

    Effect::new(move |_| {
        let search_value = debounced_query.get();
        let is_panel_open = is_open.get();
        let token_value = token.get();
        let tenant_value = tenant.get();

        if !is_panel_open || search_value.trim().len() < 2 {
            set_results.set(Vec::new());
            set_error.set(None);
            set_is_loading.set(false);
            return;
        }

        let current_request = request_seq.get_untracked() + 1;
        request_seq.set(current_request);
        set_is_loading.set(true);
        set_error.set(None);

        spawn_local(async move {
            let response = match admin_global_search_native(search_value.clone(), 8, 0).await {
                Ok(payload) => Ok(payload),
                Err(_) => request::<AdminGlobalSearchVariables, AdminGlobalSearchResponse>(
                    ADMIN_GLOBAL_SEARCH_QUERY,
                    AdminGlobalSearchVariables {
                        input: AdminGlobalSearchInput {
                            query: search_value.clone(),
                            limit: 8,
                            offset: 0,
                        },
                    },
                    token_value,
                    tenant_value,
                )
                .await
                .map(|payload| payload.admin_global_search),
            };

            if request_seq.get_untracked() != current_request {
                return;
            }

            match response {
                Ok(payload) => {
                    set_results.set(payload.items);
                    set_error.set(None);
                }
                Err(err) => {
                    set_results.set(Vec::new());
                    set_error.set(Some(format!("Global admin search failed: {err}")));
                }
            }

            set_is_loading.set(false);
        });
    });

    let navigate_open = navigate.clone();
    let open_full_search = Callback::new(move |_| {
        let href = build_search_fallback_href(query.get_untracked().as_str(), None);
        set_is_open.set(false);
        navigate_open(&href, NavigateOptions::default());
    });
    let navigate_results = navigate.clone();
    let navigate_to_result = Callback::new(move |href: String| {
        set_is_open.set(false);
        navigate_results(&href, NavigateOptions::default());
    });
    let navigate_keydown = navigate.clone();

    let on_keydown = move |ev: KeyboardEvent| match ev.key().as_str() {
        "Enter" => {
            ev.prevent_default();
            if let Some(first) = results.get_untracked().first().cloned() {
                let href = resolve_admin_href(&first, query.get_untracked().as_str());
                set_is_open.set(false);
                navigate_keydown(&href, NavigateOptions::default());
            } else if query.get_untracked().trim().len() >= 2 {
                open_full_search.run(());
            }
        }
        "Escape" => set_is_open.set(false),
        _ => {}
    };

    view! {
        <div class="relative hidden lg:block">
            <input
                type="search"
                prop:value=query
                placeholder=t_string!(i18n, app.search.placeholder)
                class="h-9 w-72 rounded-md border border-input bg-background px-3 py-2 text-sm text-foreground shadow-sm transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring xl:w-96"
                on:focus=move |_| set_is_open.set(true)
                on:blur=move |_| set_is_open.set(false)
                on:input=move |ev| {
                    set_query.set(event_target_value(&ev));
                    set_is_open.set(true);
                }
                on:keydown=on_keydown
            />

            <Show
                when=move || {
                    is_open.get()
                        && query.get().trim().len() >= 2
                        && (is_loading.get()
                            || error.get().is_some()
                            || !results.get().is_empty()
                            || query.get().trim().len() >= 2)
                }
            >
                <div class="absolute right-0 top-11 z-50 w-[30rem] overflow-hidden rounded-2xl border border-border bg-card shadow-xl">
                    <button
                        type="button"
                        class="flex w-full items-center justify-between border-b border-border px-4 py-3 text-left text-sm hover:bg-accent hover:text-accent-foreground"
                        on:mousedown=move |ev: MouseEvent| {
                            ev.prevent_default();
                            open_full_search.run(());
                        }
                    >
                        <span class="font-medium text-card-foreground">{t_string!(i18n, app.search.openFull)}</span>
                        <span class="text-xs text-muted-foreground">{move || query.get()}</span>
                    </button>

                    <div class="max-h-[24rem] overflow-y-auto">
                        <Show when=move || is_loading.get()>
                            <div class="px-4 py-3 text-sm text-muted-foreground">
                                {t_string!(i18n, app.search.loading)}
                            </div>
                        </Show>

                        <Show when=move || error.get().is_some()>
                            {move || {
                                error
                                    .get()
                                    .map(|message| {
                                        view! {
                                            <div class="border-t border-border px-4 py-3 text-sm text-destructive">
                                                {message}
                                            </div>
                                        }
                                    })
                            }}
                        </Show>

                        <Show when=move || !is_loading.get() && error.get().is_none() && results.get().is_empty()>
                            <div class="border-t border-border px-4 py-4">
                                <div class="text-sm font-medium text-card-foreground">
                                    {t_string!(i18n, app.search.noResults)}
                                </div>
                                <p class="mt-1 text-xs text-muted-foreground">
                                    {t_string!(i18n, app.search.noResultsBody)}
                                </p>
                            </div>
                        </Show>

                        {move || {
                            results
                                .get()
                                .into_iter()
                                .map(move |item| {
                                    let href = resolve_admin_href(&item, query.get_untracked().as_str());
                                    let navigate_to_result = navigate_to_result.clone();
                                    let on_select = Callback::new({
                                        let href = href.clone();
                                        move |_| navigate_to_result.run(href.clone())
                                    });
                                    view! {
                                        <HeaderGlobalSearchResultRow
                                            item=item
                                            href=href
                                            on_select=on_select
                                        />
                                    }
                                })
                                .collect_view()
                        }}
                    </div>
                </div>
            </Show>
        </div>
    }
}

#[component]
fn HeaderGlobalSearchResultRow(
    item: AdminGlobalSearchItem,
    href: String,
    on_select: Callback<()>,
) -> impl IntoView {
    let subtitle = admin_result_subtitle(&item);

    view! {
        <button
            type="button"
            class="flex w-full flex-col gap-1 border-t border-border px-4 py-3 text-left hover:bg-accent hover:text-accent-foreground"
            on:mousedown=move |ev: MouseEvent| {
                ev.prevent_default();
                let _ = &href;
                on_select.run(());
            }
        >
            <div class="flex items-center justify-between gap-3">
                <span class="text-sm font-medium text-card-foreground">{item.title.clone()}</span>
                <span class="text-[11px] uppercase tracking-[0.16em] text-muted-foreground">
                    {format!("{} • {}", item.source_module, item.entity_type)}
                </span>
            </div>
            <p class="text-xs text-muted-foreground">{subtitle}</p>
            <div class="text-[11px] text-muted-foreground">{format!("score {:.3}", item.score)}</div>
        </button>
    }
}

/// Resolve a navigation label key to its translation.
/// Uses compile-time checked keys via t_string! for known routes.
fn resolve_label(i18n: leptos_i18n::I18nContext<crate::i18n::Locale>, key: &str) -> String {
    let s: &str = match key {
        "app.nav.dashboard" => t_string!(i18n, app.nav.dashboard),
        "app.nav.users" => t_string!(i18n, app.nav.users),
        "app.nav.profile" => t_string!(i18n, app.nav.profile),
        "app.nav.security" => t_string!(i18n, app.nav.security),
        "app.nav.modules" => t_string!(i18n, app.nav.modules),
        "app.nav.search" => t_string!(i18n, app.nav.search),
        "users.detail.title" => t_string!(i18n, users.detail.title),
        _ => key,
    };
    s.to_string()
}

fn resolve_breadcrumbs(pathname: &str) -> Vec<Breadcrumb> {
    match pathname {
        "/" | "/dashboard" => vec![Breadcrumb {
            label_key: "app.nav.dashboard",
            href: Some("/dashboard"),
        }],
        "/users" => vec![Breadcrumb {
            label_key: "app.nav.users",
            href: Some("/users"),
        }],
        "/profile" => vec![Breadcrumb {
            label_key: "app.nav.profile",
            href: Some("/profile"),
        }],
        "/security" => vec![Breadcrumb {
            label_key: "app.nav.security",
            href: Some("/security"),
        }],
        "/modules" => vec![Breadcrumb {
            label_key: "app.nav.modules",
            href: Some("/modules"),
        }],
        _ if pathname.starts_with("/modules/search") => vec![Breadcrumb {
            label_key: "app.nav.search",
            href: Some("/modules/search"),
        }],
        _ if pathname.starts_with("/users/") => vec![
            Breadcrumb {
                label_key: "app.nav.users",
                href: Some("/users"),
            },
            Breadcrumb {
                label_key: "users.detail.title",
                href: None,
            },
        ],
        _ => vec![Breadcrumb {
            label_key: "app.nav.dashboard",
            href: Some("/dashboard"),
        }],
    }
}

fn resolve_title_key(pathname: &str) -> &'static str {
    match pathname {
        "/" | "/dashboard" => "app.nav.dashboard",
        "/users" => "app.nav.users",
        "/profile" => "app.nav.profile",
        "/security" => "app.nav.security",
        "/modules" => "app.nav.modules",
        _ if pathname.starts_with("/modules/search") => "app.nav.search",
        _ if pathname.starts_with("/users/") => "users.detail.title",
        _ => "app.nav.dashboard",
    }
}

fn build_search_fallback_href(query: &str, item: Option<&AdminGlobalSearchItem>) -> String {
    let mut params: Vec<(&str, String)> = Vec::new();

    if !query.trim().is_empty() {
        params.push(("q", query.trim().to_string()));
    }

    if let Some(item) = item {
        params.push(("focusId", item.id.clone()));
        params.push(("entityType", item.entity_type.clone()));
        params.push(("sourceModule", item.source_module.clone()));
    }

    let encoded = serde_urlencoded::to_string(params)
        .ok()
        .filter(|value| !value.is_empty())
        .map(|value| format!("?{value}"))
        .unwrap_or_default();

    format!("/modules/search/playground{encoded}")
}

fn resolve_admin_href(item: &AdminGlobalSearchItem, query: &str) -> String {
    match item.entity_type.as_str() {
        "node" => {
            let module_slug = if item.source_module.trim().is_empty() {
                "content"
            } else {
                item.source_module.as_str()
            };
            format!("/modules/{module_slug}?id={}", item.id)
        }
        "product" => build_search_fallback_href(query, Some(item)),
        _ => build_search_fallback_href(query, Some(item)),
    }
}

fn admin_result_subtitle(item: &AdminGlobalSearchItem) -> String {
    item.snippet
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            let mut segments = vec![item.source_module.clone(), item.entity_type.clone()];
            if let Some(locale) = item.locale.clone().filter(|value| !value.trim().is_empty()) {
                segments.push(locale);
            }
            if !item.payload.trim().is_empty() {
                segments.push("indexed payload ready".to_string());
            }
            segments.join(" • ")
        })
}

fn set_document_title(_title: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                document.set_title(_title);
            }
        }
    }
}
