mod api;
mod i18n;
mod model;

use leptos::ev::{MouseEvent, SubmitEvent};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_ui_routing::read_route_query_value;
use leptos::web_sys;
use rustok_api::UiRouteContext;

use crate::i18n::t;
use crate::model::{
    SearchFacetGroup, SearchFilterPreset, SearchPreviewFilters, SearchPreviewPayload,
    SearchSuggestion,
};

#[component]
pub fn SearchView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let query = read_route_query_value(&route_context, "q").unwrap_or_default();
    let (search_input, set_search_input) = signal(query.clone());
    let preset_key = read_route_query_value(&route_context, "preset").unwrap_or_default();
    let (selected_preset, set_selected_preset) = signal(preset_key.clone());
    let locale = route_context.locale.clone();
    let badge_label = t(locale.as_deref(), "search.badge", "search");
    let title_label = t(
        locale.as_deref(),
        "search.title",
        "Search across published content and catalog",
    );
    let subtitle_label = t(
        locale.as_deref(),
        "search.subtitle",
        "This storefront surface is backed by PostgreSQL full-text search over published content and products.",
    );
    let query_label = t(locale.as_deref(), "search.form.queryLabel", "Search query");
    let query_placeholder = t(
        locale.as_deref(),
        "search.form.placeholder",
        "Search products and published content",
    );
    let submit_label = t(locale.as_deref(), "search.form.submit", "Search");
    let load_presets_error = t(
        locale.as_deref(),
        "search.error.loadPresets",
        "Failed to load presets",
    );
    let autocomplete_hint = t(
        locale.as_deref(),
        "search.form.autocompleteHint",
        "Autocomplete uses popular successful queries and matching published document titles from rustok-search.",
    );
    let loading_suggestions_label = t(
        locale.as_deref(),
        "search.suggestions.loading",
        "Loading suggestions...",
    );
    let suggestions_empty_label = t(
        locale.as_deref(),
        "search.suggestions.empty",
        "Type at least 2 characters to see autocomplete suggestions.",
    );
    let load_suggestions_error = t(
        locale.as_deref(),
        "search.error.loadSuggestions",
        "Failed to load search suggestions",
    );
    let empty_results_title = t(
        locale.as_deref(),
        "search.results.emptyTitle",
        "Enter a search query",
    );
    let empty_results_body = t(
        locale.as_deref(),
        "search.results.emptyBody",
        "Storefront search reads `?q=` from the generic module route and runs the public PostgreSQL FTS pipeline.",
    );
    let load_results_error = t(
        locale.as_deref(),
        "search.error.loadResults",
        "Failed to load storefront search results",
    );
    let entity_types = parse_csv(
        read_route_query_value(&route_context, "entity_types").as_deref().unwrap_or(""),
    );
    let source_modules = parse_csv(
        read_route_query_value(&route_context, "source_modules").as_deref().unwrap_or(""),
    );
    let statuses = parse_csv(
        read_route_query_value(&route_context, "statuses").as_deref().unwrap_or(""),
    );
    let filters = SearchPreviewFilters {
        entity_types,
        source_modules,
        statuses,
    };
    let query_for_resource = query.clone();
    let locale_for_resource = locale.clone();
    let filters_for_resource = filters.clone();
    let query_for_view = query.clone();
    let preset_for_view = preset_key.clone();
    let locale_for_suggestions = locale.clone();
    let preset_for_resource = preset_key.clone();
    let results = Resource::new_blocking(
        move || {
            (
                query_for_resource.clone(),
                locale_for_resource.clone(),
                filters_for_resource.clone(),
            )
        },
        move |(query, locale, filters)| {
            let preset_key = preset_for_resource.clone();
            async move {
                if query.trim().is_empty() {
                    Ok(None)
                } else {
                    api::fetch_storefront_search(
                        query,
                        locale,
                        (!preset_key.is_empty()).then_some(preset_key.clone()),
                        filters,
                    )
                    .await
                    .map(Some)
                }
            }
        },
    );
    let suggestions = Resource::new(
        move || (search_input.get(), locale_for_suggestions.clone()),
        move |(query, locale)| async move {
            let trimmed = query.trim().to_string();
            if trimmed.len() < 2 {
                Ok(Vec::new())
            } else {
                api::fetch_storefront_suggestions(trimmed, locale).await
            }
        },
    );
    let filter_presets = Resource::new(
        || (),
        move |_| async move { api::fetch_storefront_filter_presets().await },
    );

    view! {
        <section class="rounded-3xl border border-border bg-card p-8 shadow-sm">
            <div class="max-w-3xl space-y-3">
                <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                    {badge_label}
                </span>
                <h2 class="text-3xl font-semibold text-card-foreground">
                    {title_label}
                </h2>
                <p class="text-sm text-muted-foreground">
                    {subtitle_label}
                </p>
            </div>

            <div class="mt-8 space-y-4">
                <form
                    class="rounded-2xl border border-border bg-background p-4"
                    on:submit=move |ev| submit_search(ev, search_input.get(), selected_preset.get())
                >
                    <label class="block text-sm font-medium text-card-foreground" for="storefront-search-input">
                        {query_label.clone()}
                    </label>
                    <div class="mt-3 flex flex-col gap-3 md:flex-row">
                        <input
                            id="storefront-search-input"
                            class="min-w-0 flex-1 rounded-xl border border-border bg-card px-4 py-3 text-sm text-foreground"
                            prop:value=move || search_input.get()
                            on:input=move |ev| set_search_input.set(event_target_value(&ev))
                            placeholder=query_placeholder.clone()
                        />
                        <button
                            class="inline-flex items-center justify-center rounded-xl bg-primary px-4 py-3 text-sm font-medium text-primary-foreground"
                            type="submit"
                        >
                            {submit_label.clone()}
                        </button>
                    </div>
                    <Suspense fallback=|| view! { <div class="mt-3 h-10 animate-pulse rounded-xl bg-muted"></div> }>
                        {move || filter_presets.get().map(|result| match result {
                            Ok(presets) if !presets.is_empty() => view! {
                                <PresetChips presets selected_preset set_selected_preset query=search_input.get() />
                            }.into_any(),
                            Ok(_) => view! { <></> }.into_any(),
                            Err(err) => view! { <div class="mt-3 rounded-xl border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">{format!("{}: {err}", load_presets_error.clone())}</div> }.into_any(),
                        })}
                    </Suspense>
                    <p class="mt-3 text-xs text-muted-foreground">
                        {autocomplete_hint}
                    </p>
                </form>

                <Suspense fallback=move || {
                    let loading_suggestions_label = loading_suggestions_label.clone();
                    view! {
                        <div class="rounded-2xl border border-border bg-background p-4 text-sm text-muted-foreground">
                            {loading_suggestions_label}
                        </div>
                    }
                }>
                    {move || {
                        let suggestions = suggestions.clone();
                        let suggestions_empty_label = suggestions_empty_label.clone();
                        let load_suggestions_error = load_suggestions_error.clone();
                        Suspend::new(async move {
                            match suggestions.await {
                                Ok(items) if !items.is_empty() => view! {
                                    <SearchSuggestionList suggestions=items />
                                }.into_any(),
                                Ok(_) => view! {
                                    <div class="rounded-2xl border border-dashed border-border p-4 text-sm text-muted-foreground">
                                        {suggestions_empty_label.clone()}
                                    </div>
                                }.into_any(),
                                Err(err) => view! {
                                    <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                        {format!("{}: {err}", load_suggestions_error.clone())}
                                    </div>
                                }.into_any(),
                            }
                        })
                    }}
                </Suspense>

                <Suspense fallback=|| view! {
                    <div class="space-y-4">
                        <div class="h-28 animate-pulse rounded-2xl bg-muted"></div>
                        <div class="grid gap-4 md:grid-cols-3">
                            <div class="h-24 animate-pulse rounded-2xl bg-muted"></div>
                            <div class="h-24 animate-pulse rounded-2xl bg-muted"></div>
                            <div class="h-24 animate-pulse rounded-2xl bg-muted"></div>
                        </div>
                        <div class="h-40 animate-pulse rounded-2xl bg-muted"></div>
                    </div>
                }>
                    {move || {
                        let results = results.clone();
                        let query = query_for_view.clone();
                        let preset_key = preset_for_view.clone();
                        let empty_results_title = empty_results_title.clone();
                        let empty_results_body = empty_results_body.clone();
                        let load_results_error = load_results_error.clone();
                        Suspend::new(async move {
                            match results.await {
                                Ok(Some(payload)) => view! {
                                    <SearchResults query=query.clone() selected_preset=preset_key.clone() payload />
                                }.into_any(),
                                Ok(None) => view! {
                                    <EmptyState
                                        title=empty_results_title.clone()
                                        body=empty_results_body.clone()
                                    />
                                }.into_any(),
                                Err(err) => view! {
                                    <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                        {format!("{}: {err}", load_results_error.clone())}
                                    </div>
                                }.into_any(),
                            }
                        })
                    }}
                </Suspense>
            </div>
        </section>
    }
}

#[component]
fn SearchSuggestionList(suggestions: Vec<SearchSuggestion>) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let suggestions_title = t(locale.as_deref(), "search.suggestions.title", "Suggestions");
    let suggestions_badge = t(
        locale.as_deref(),
        "search.suggestions.badge",
        "autocomplete",
    );
    let open_label = t(locale.as_deref(), "search.suggestions.open", "Open");
    let search_label = t(locale.as_deref(), "search.suggestions.search", "Search");
    view! {
        <article class="rounded-2xl border border-border bg-background p-4">
            <div class="flex items-center justify-between gap-3">
                <div class="text-sm font-semibold text-card-foreground">
                    {suggestions_title}
                </div>
                <div class="text-xs uppercase tracking-[0.16em] text-muted-foreground">
                    {suggestions_badge}
                </div>
            </div>
            <div class="mt-3 grid gap-2">
                {suggestions
                    .into_iter()
                    .map(|suggestion| {
                        let suggestion_text = suggestion.text.clone();
                        let suggestion_kind = suggestion.kind.clone();
                        let suggestion_locale = suggestion.locale.clone();
                        let href = suggestion.url.clone();
                        view! {
                            <button
                                class="flex w-full items-start justify-between gap-4 rounded-xl border border-border px-4 py-3 text-left hover:bg-muted/30"
                                on:click=move |_| {
                                    if suggestion_kind == "document" {
                                        if let Some(href) = href.clone() {
                                            navigate_to_href(&href);
                                        } else {
                                            navigate_to_search_query(&suggestion_text, None);
                                        }
                                    } else {
                                        navigate_to_search_query(&suggestion_text, None);
                                    }
                                }
                                type="button"
                            >
                                <span class="min-w-0">
                                    <span class="block truncate text-sm font-medium text-card-foreground">
                                        {suggestion_text.clone()}
                                    </span>
                                    <span class="mt-1 block text-xs uppercase tracking-[0.16em] text-muted-foreground">
                                        {format!(
                                            "{}{}",
                                            suggestion_kind,
                                            suggestion_locale
                                                .as_deref()
                                                .map(|locale| format!(" • {locale}"))
                                                .unwrap_or_default()
                                        )}
                                    </span>
                                </span>
                                <span class="shrink-0 text-xs text-muted-foreground">
                                    {if suggestion_kind == "document" { open_label.clone() } else { search_label.clone() }}
                                </span>
                            </button>
                        }
                    })
                    .collect_view()}
            </div>
        </article>
    }
}

#[component]
fn PresetChips(
    presets: Vec<SearchFilterPreset>,
    selected_preset: ReadSignal<String>,
    set_selected_preset: WriteSignal<String>,
    query: String,
) -> impl IntoView {
    view! {
        <div class="mt-3 flex flex-wrap gap-2">
            {presets.into_iter().map(|preset| {
                let key = preset.key.clone();
                let label = preset.label.clone();
                let class_key = key.clone();
                let query_value = query.clone();
                view! {
                    <button
                        class=move || if selected_preset.get() == class_key {
                            "inline-flex items-center rounded-full border border-primary bg-primary/10 px-3 py-1 text-xs font-medium text-primary"
                        } else {
                            "inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground"
                        }
                        on:click=move |_| {
                            let next = if selected_preset.get() == key { String::new() } else { key.clone() };
                            set_selected_preset.set(next.clone());
                            navigate_to_search_query(&query_value, Some(next));
                        }
                        type="button"
                    >
                        {label}
                    </button>
                }
            }).collect_view()}
        </div>
    }
}

#[component]
fn SearchResults(
    query: String,
    selected_preset: String,
    payload: SearchPreviewPayload,
) -> impl IntoView {
    let locale_context = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let query_label = t(
        locale_context.as_deref(),
        "search.results.queryLabel",
        "Query",
    );
    let results_summary_template = t(
        locale_context.as_deref(),
        "search.results.summary",
        "{count} results in {took_ms} ms via {engine} ({ranking_profile})",
    );
    let preset_template = t(
        locale_context.as_deref(),
        "search.results.preset",
        "preset = {preset}",
    );
    let none_label = t(locale_context.as_deref(), "search.results.none", "none");
    let locale_template = t(
        locale_context.as_deref(),
        "search.results.locale",
        "locale = {locale}",
    );
    let no_results_title = t(
        locale_context.as_deref(),
        "search.results.noResultsTitle",
        "No results",
    );
    let no_results_body = t(
        locale_context.as_deref(),
        "search.results.noResultsBody",
        "Try a different query or relax the storefront filters in the query string.",
    );
    let engine_title = t(
        locale_context.as_deref(),
        "search.features.engineTitle",
        "Engine",
    );
    let engine_body = t(
        locale_context.as_deref(),
        "search.features.engineBody",
        "Storefront uses the public published-only search surface backed by PostgreSQL FTS.",
    );
    let facet_title = t(
        locale_context.as_deref(),
        "search.features.facetsTitle",
        "Facet model",
    );
    let facet_body = t(
        locale_context.as_deref(),
        "search.features.facetsBody",
        "Entity type and source module facets come from the same search payload used by admin previews.",
    );
    let locale = payload
        .items
        .first()
        .and_then(|item| item.locale.clone())
        .unwrap_or_else(|| "all".to_string());
    let SearchPreviewPayload {
        query_log_id,
        preset_key: applied_preset_key,
        items,
        total,
        took_ms,
        engine,
        ranking_profile,
        facets,
    } = payload;
    let has_items = !items.is_empty();
    let item_views = items
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            let query_log_id = query_log_id.clone();
            let href = item.url.clone();
            view! {
                <article class="rounded-2xl border border-border bg-background p-5">
                    <div class="flex flex-wrap items-center gap-2 text-xs font-medium uppercase tracking-[0.16em] text-muted-foreground">
                        <span>{item.entity_type.clone()}</span>
                        <span>"|"</span>
                        <span>{item.source_module.clone()}</span>
                        <span>"|"</span>
                        <span>{format!("score {:.3}", item.score)}</span>
                    </div>
                    <h3 class="mt-3 text-lg font-semibold text-foreground">{item.title}</h3>
                    <p class="mt-2 text-sm text-muted-foreground">
                        {item.snippet.unwrap_or_else(|| "No snippet returned.".to_string())}
                    </p>
                    {render_result_action(query_log_id, item.id.clone(), href, index)}
                </article>
            }
        })
        .collect_view();
    let facet_views = facets
        .into_iter()
        .map(|facet| view! { <FacetCard facet /> })
        .collect_view();

    view! {
        <div class="grid gap-6 lg:grid-cols-[minmax(0,1fr)_20rem]">
            <div class="space-y-6">
                <article class="rounded-2xl border border-border bg-background p-6">
                    <div class="flex flex-wrap items-center justify-between gap-3">
                        <div>
                            <div class="text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">
                                {query_label}
                            </div>
                            <h3 class="mt-2 text-xl font-semibold text-foreground">{query}</h3>
                            <p class="mt-2 text-sm text-muted-foreground">
                                {results_summary_template
                                    .replace("{count}", total.to_string().as_str())
                                    .replace("{took_ms}", took_ms.to_string().as_str())
                                    .replace("{engine}", engine.as_str())
                                    .replace("{ranking_profile}", ranking_profile.as_str())}
                            </p>
                            <p class="mt-2 text-xs text-muted-foreground">
                                {preset_template.replace(
                                    "{preset}",
                                    applied_preset_key
                                        .filter(|value| !value.is_empty())
                                        .unwrap_or_else(|| if selected_preset.is_empty() { none_label.clone() } else { selected_preset.clone() })
                                        .as_str(),
                                )}
                            </p>
                        </div>
                        <div class="rounded-xl border border-border bg-muted/20 px-4 py-3 text-sm text-card-foreground">
                            {locale_template.replace("{locale}", locale.as_str())}
                        </div>
                    </div>
                </article>

                {if has_items {
                    view! {
                        <div class="space-y-3">
                            {item_views}
                        </div>
                    }
                    .into_any()
                } else {
                    view! {
                        <EmptyState
                            title=no_results_title
                            body=no_results_body
                        />
                    }
                    .into_any()
                }}
            </div>

            <aside class="space-y-4">
                <FeatureCard
                    title=engine_title
                    body=engine_body
                />
                <FeatureCard
                    title=facet_title
                    body=facet_body
                />
                {facet_views}
            </aside>
        </div>
    }
}

fn render_result_action(
    query_log_id: Option<String>,
    document_id: String,
    href: Option<String>,
    index: usize,
) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let no_target_label = t(
        locale.as_deref(),
        "search.results.noTarget",
        "No storefront target is available for this result yet.",
    );
    let open_result_label = t(
        locale.as_deref(),
        "search.results.openResult",
        "Open result",
    );
    let Some(href_value) = href else {
        return view! {
            <p class="mt-4 text-xs text-muted-foreground">
                {no_target_label}
            </p>
        }
        .into_any();
    };

    view! {
        <a
            class="mt-4 inline-flex text-sm font-medium text-primary hover:underline"
            href=href_value.clone()
            on:click=move |ev| track_result_click(ev, query_log_id.clone(), document_id.clone(), href_value.clone(), index)
        >
            {open_result_label}
        </a>
    }
    .into_any()
}

fn track_result_click(
    ev: MouseEvent,
    query_log_id: Option<String>,
    document_id: String,
    href: String,
    index: usize,
) {
    let Some(window) = web_sys::window() else {
        return;
    };

    let Some(query_log_id) = query_log_id else {
        return;
    };

    ev.prevent_default();
    spawn_local(async move {
        let _ = api::track_search_click(
            query_log_id,
            document_id,
            Some((index + 1) as i32),
            Some(href.clone()),
        )
        .await;

        let _ = window.location().set_href(&href);
    });
}

#[component]
fn FeatureCard<T, U>(title: T, body: U) -> impl IntoView
where
    T: IntoView + 'static,
    U: IntoView + 'static,
{
    view! {
        <article class="rounded-2xl border border-border bg-background p-5">
            <div class="text-sm font-semibold text-card-foreground">{title}</div>
            <p class="mt-2 text-sm text-muted-foreground">{body}</p>
        </article>
    }
}

#[component]
fn FacetCard(facet: SearchFacetGroup) -> impl IntoView {
    view! {
        <article class="rounded-2xl border border-border bg-background p-5">
            <div class="text-sm font-semibold capitalize text-card-foreground">
                {facet.name.replace('_', " ")}
            </div>
            <div class="mt-3 flex flex-wrap gap-2">
                {facet.buckets.into_iter().map(|bucket| view! {
                    <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                        {format!("{} ({})", bucket.value, bucket.count)}
                    </span>
                }).collect_view()}
            </div>
        </article>
    }
}

#[component]
fn EmptyState(title: String, body: String) -> impl IntoView {
    view! {
        <article class="rounded-2xl border border-dashed border-border p-8 text-center">
            <h3 class="text-lg font-semibold text-card-foreground">{title}</h3>
            <p class="mt-2 text-sm text-muted-foreground">{body}</p>
        </article>
    }
}

fn parse_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn submit_search(ev: SubmitEvent, query: String, preset_key: String) {
    ev.prevent_default();
    navigate_to_search_query(&query, Some(preset_key));
}

fn navigate_to_search_query(query: &str, preset_key: Option<String>) {
    let Some(window) = web_sys::window() else {
        return;
    };

    let Ok(current_href) = window.location().href() else {
        return;
    };

    let Ok(url) = web_sys::Url::new(&current_href) else {
        return;
    };

    if query.trim().is_empty() {
        let _ = url.search_params().delete("q");
    } else {
        url.search_params().set("q", query.trim());
    }

    match preset_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => url.search_params().set("preset", value),
        None => {
            let _ = url.search_params().delete("preset");
        }
    }

    let _ = window.location().set_href(&url.href());
}

fn navigate_to_href(href: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };

    let _ = window.location().set_href(href);
}
