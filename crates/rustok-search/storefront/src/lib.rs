mod api;
mod model;

use leptos::ev::MouseEvent;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::web_sys;
use rustok_api::UiRouteContext;

use crate::model::{SearchFacetGroup, SearchPreviewFilters, SearchPreviewPayload};

#[component]
pub fn SearchView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let query = route_context.query_value("q").unwrap_or("").to_string();
    let locale = route_context.locale.clone();
    let entity_types = parse_csv(route_context.query_value("entity_types").unwrap_or(""));
    let source_modules = parse_csv(route_context.query_value("source_modules").unwrap_or(""));
    let statuses = parse_csv(route_context.query_value("statuses").unwrap_or(""));
    let filters = SearchPreviewFilters {
        entity_types,
        source_modules,
        statuses,
    };
    let query_for_resource = query.clone();
    let locale_for_resource = locale.clone();
    let filters_for_resource = filters.clone();
    let query_for_view = query.clone();
    let results = Resource::new_blocking(
        move || {
            (
                query_for_resource.clone(),
                locale_for_resource.clone(),
                filters_for_resource.clone(),
            )
        },
        move |(query, locale, filters)| async move {
            if query.trim().is_empty() {
                Ok(None)
            } else {
                api::fetch_storefront_search(query, locale, filters)
                    .await
                    .map(Some)
            }
        },
    );

    view! {
        <section class="rounded-3xl border border-border bg-card p-8 shadow-sm">
            <div class="max-w-3xl space-y-3">
                <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                    "search"
                </span>
                <h2 class="text-3xl font-semibold text-card-foreground">
                    "Search across published content and catalog"
                </h2>
                <p class="text-sm text-muted-foreground">
                    "This storefront surface is backed by PostgreSQL full-text search over published content and products."
                </p>
            </div>

            <div class="mt-8">
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
                        Suspend::new(async move {
                            match results.await {
                                Ok(Some(payload)) => view! {
                                    <SearchResults query=query.clone() payload />
                                }.into_any(),
                                Ok(None) => view! {
                                    <EmptyState
                                        title="Enter a search query"
                                        body="Storefront search reads `?q=` from the generic module route and runs the public PostgreSQL FTS pipeline."
                                    />
                                }.into_any(),
                                Err(err) => view! {
                                    <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                        {format!("Failed to load storefront search results: {err}")}
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
fn SearchResults(query: String, payload: SearchPreviewPayload) -> impl IntoView {
    let locale = payload
        .items
        .first()
        .and_then(|item| item.locale.clone())
        .unwrap_or_else(|| "all".to_string());
    let SearchPreviewPayload {
        query_log_id,
        items,
        total,
        took_ms,
        engine,
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
                                "Query"
                            </div>
                            <h3 class="mt-2 text-xl font-semibold text-foreground">{query}</h3>
                            <p class="mt-2 text-sm text-muted-foreground">
                                {format!("{total} results in {took_ms} ms via {engine}")}
                            </p>
                        </div>
                        <div class="rounded-xl border border-border bg-muted/20 px-4 py-3 text-sm text-card-foreground">
                            {format!("locale = {locale}")}
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
                            title="No results"
                            body="Try a different query or relax the storefront filters in the query string."
                        />
                    }
                    .into_any()
                }}
            </div>

            <aside class="space-y-4">
                <FeatureCard
                    title="Engine"
                    body="Storefront uses the public published-only search surface backed by PostgreSQL FTS."
                />
                <FeatureCard
                    title="Facet model"
                    body="Entity type and source module facets come from the same search payload used by admin previews."
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
    let Some(href_value) = href else {
        return view! {
            <p class="mt-4 text-xs text-muted-foreground">
                "No storefront target is available for this result yet."
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
            "Open result"
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
fn EmptyState(title: &'static str, body: &'static str) -> impl IntoView {
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
