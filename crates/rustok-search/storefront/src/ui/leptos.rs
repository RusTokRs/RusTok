use leptos::ev::{MouseEvent, SubmitEvent};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos::web_sys;
use leptos_ui_routing::read_route_query_value;
use rustok_ui_core::UiRouteContext;

use crate::i18n::t;
use crate::model::{SearchFilterPreset, SearchPreviewPayload, SearchSuggestion};
use crate::{core, transport};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchCatalogFilterOption {
    pub value: String,
    pub label: String,
}

#[component]
pub fn SearchView(
    #[prop(optional)] category_options: Vec<SearchCatalogFilterOption>,
    #[prop(optional)] attribute_options: Vec<SearchCatalogFilterOption>,
) -> impl IntoView {
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
    let catalog_filters_label = t(
        locale.as_deref(),
        "search.filters.title",
        "Catalog filters and sorting",
    );
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
    let empty_results = core::build_search_empty_state_view_model(
        t(
            locale.as_deref(),
            "search.results.emptyTitle",
            "Enter a search query",
        ),
        t(
            locale.as_deref(),
            "search.results.emptyBody",
            "Storefront search reads `?q=` from the generic module route and runs the public PostgreSQL FTS pipeline.",
        ),
    );
    let load_results_error = t(
        locale.as_deref(),
        "search.error.loadResults",
        "Failed to load storefront search results",
    );
    let route_filters = core::parse_search_route_filters(core::SearchRouteFilterQuery {
        channel_id: read_route_query_value(&route_context, "channel_id"),
        entity_types: read_route_query_value(&route_context, "entity_types"),
        source_modules: read_route_query_value(&route_context, "source_modules"),
        statuses: read_route_query_value(&route_context, "statuses"),
        category_ids: read_route_query_value(&route_context, "category_ids"),
        attribute_code: read_route_query_value(&route_context, "attribute_code"),
        attribute_values: read_route_query_value(&route_context, "attribute_values"),
        attribute_min: read_route_query_value(&route_context, "attribute_min"),
        attribute_max: read_route_query_value(&route_context, "attribute_max"),
        sort_attribute_code: read_route_query_value(&route_context, "sort_attribute_code"),
        sort_desc: read_route_query_value(&route_context, "sort_desc"),
    });
    let (channel_id, set_channel_id) = signal(route_filters.channel_id.clone().unwrap_or_default());
    let (category_ids, set_category_ids) = signal(route_filters.category_ids.join(","));
    let (attribute_code, set_attribute_code) =
        signal(route_filters.attribute_code.clone().unwrap_or_default());
    let (attribute_values, set_attribute_values) = signal(route_filters.attribute_values.join(","));
    let (attribute_min, set_attribute_min) =
        signal(route_filters.attribute_min.clone().unwrap_or_default());
    let (attribute_max, set_attribute_max) =
        signal(route_filters.attribute_max.clone().unwrap_or_default());
    let (sort_attribute_code, set_sort_attribute_code) = signal(
        route_filters
            .sort_attribute_code
            .clone()
            .unwrap_or_default(),
    );
    let (sort_desc, set_sort_desc) = signal(route_filters.sort_desc);
    let filters = core::search_preview_filters_from_route(route_filters);
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
                match core::build_storefront_search_fetch_request(
                    &query,
                    locale,
                    preset_key.as_str(),
                    filters,
                ) {
                    Some(request) => transport::fetch_search(
                        request.query,
                        request.locale,
                        request.preset_key,
                        request.filters,
                    )
                    .await
                    .map(Some),
                    None => Ok(None),
                }
            }
        },
    );
    let suggestions = Resource::new(
        move || (search_input.get(), locale_for_suggestions.clone()),
        move |(query, locale)| async move {
            match core::build_storefront_suggestion_fetch_request(&query, locale) {
                Some(request) => transport::fetch_suggestions(request.query, request.locale).await,
                None => Ok(Vec::new()),
            }
        },
    );
    let filter_presets = Resource::new(
        || (),
        move |_| async move { transport::fetch_filter_presets().await },
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
                    on:submit=move |ev| submit_search(ev, CatalogSearchSubmission {
                        query: search_input.get(),
                        preset_key: selected_preset.get(),
                        channel_id: channel_id.get(),
                        category_ids: category_ids.get(),
                        attribute_code: attribute_code.get(),
                        attribute_values: attribute_values.get(),
                        attribute_min: attribute_min.get(),
                        attribute_max: attribute_max.get(),
                        sort_attribute_code: sort_attribute_code.get(),
                        sort_desc: sort_desc.get(),
                    })
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
                    <details class="mt-3 border border-border bg-muted/10 p-3">
                        <summary class="cursor-pointer text-sm font-medium text-card-foreground">{catalog_filters_label}</summary>
                        <div class="mt-3 grid gap-3 md:grid-cols-2">
                            <CatalogFilterField label=t(locale.as_deref(), "search.filters.channelId", "Channel ID") value=channel_id set_value=set_channel_id />
                            <CatalogFilterField label=t(locale.as_deref(), "search.filters.categoryIds", "Category IDs (CSV)") value=category_ids set_value=set_category_ids options=category_options.clone() list_id="search-storefront-category-options" />
                            <CatalogFilterField label=t(locale.as_deref(), "search.filters.attributeCode", "Attribute code") value=attribute_code set_value=set_attribute_code options=attribute_options.clone() list_id="search-storefront-attribute-options" />
                            <CatalogFilterField label=t(locale.as_deref(), "search.filters.attributeValues", "Attribute values (CSV)") value=attribute_values set_value=set_attribute_values />
                            <CatalogFilterField label=t(locale.as_deref(), "search.filters.minimum", "Minimum") value=attribute_min set_value=set_attribute_min />
                            <CatalogFilterField label=t(locale.as_deref(), "search.filters.maximum", "Maximum") value=attribute_max set_value=set_attribute_max />
                            <CatalogFilterField label=t(locale.as_deref(), "search.filters.sortAttribute", "Sort attribute code") value=sort_attribute_code set_value=set_sort_attribute_code options=attribute_options.clone() list_id="search-storefront-sort-attribute-options" />
                            <label class="flex items-center gap-2 text-sm font-medium text-card-foreground"><input type="checkbox" prop:checked=sort_desc on:change=move |ev| set_sort_desc.set(event_target_checked(&ev)) /><span>{t(locale.as_deref(), "search.filters.sortDesc", "Descending order")}</span></label>
                        </div>
                    </details>
                    <Suspense fallback=|| view! { <div class="mt-3 h-10 animate-pulse rounded-xl bg-muted"></div> }>
                        {move || filter_presets.get().map(|result| match result {
                            Ok(presets) if core::has_items(presets.as_slice()) => view! {
                                <PresetChips presets selected_preset set_selected_preset query=search_input.get() />
                            }.into_any(),
                            Ok(_) => ().into_any(),
                            Err(err) => view! { <div class="mt-3 rounded-xl border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs text-destructive">{core::error_with_context(load_presets_error.as_str(), &err.to_string())}</div> }.into_any(),
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
                        let suggestions_empty_label = suggestions_empty_label.clone();
                        let load_suggestions_error = load_suggestions_error.clone();
                        Suspend::new(async move {
                            match suggestions.await {
                                Ok(items) if core::has_items(items.as_slice()) => view! {
                                    <SearchSuggestionList suggestions=items />
                                }.into_any(),
                                Ok(_) => view! {
                                    <div class="rounded-2xl border border-dashed border-border p-4 text-sm text-muted-foreground">
                                        {suggestions_empty_label.clone()}
                                    </div>
                                }.into_any(),
                                Err(err) => view! {
                                    <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                        {core::error_with_context(load_suggestions_error.as_str(), &err.to_string())}
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
                        let query = query_for_view.clone();
                        let preset_key = preset_for_view.clone();
                        let empty_results = empty_results.clone();
                        let load_results_error = load_results_error.clone();
                        Suspend::new(async move {
                            match results.await {
                                Ok(Some(payload)) => view! {
                                    <SearchResults query=query.clone() selected_preset=preset_key.clone() payload />
                                }.into_any(),
                                Ok(None) => view! {
                                    <EmptyState state=empty_results.clone() />
                                }.into_any(),
                                Err(err) => view! {
                                    <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                        {core::error_with_context(load_results_error.as_str(), &err.to_string())}
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
                {core::build_search_suggestion_view_models(
                    suggestions,
                    &core::SearchSuggestionsLabels { open_label, search_label },
                )
                    .into_iter()
                    .map(|suggestion| {
                        let navigation = suggestion.navigation.clone();
                        view! {
                            <button
                                class="flex w-full items-start justify-between gap-4 rounded-xl border border-border px-4 py-3 text-left hover:bg-muted/30"
                                on:click=move |_| match navigation.clone() {
                                    core::SearchSuggestionNavigation::Href(href) => navigate_to_href(&href),
                                    core::SearchSuggestionNavigation::SearchQuery(query) => {
                                        navigate_to_search_query(&query, None)
                                    }
                                }
                                type="button"
                            >
                                <span class="min-w-0">
                                    <span class="block truncate text-sm font-medium text-card-foreground">
                                        {suggestion.text.clone()}
                                    </span>
                                    <span class="mt-1 block text-xs uppercase tracking-[0.16em] text-muted-foreground">
                                        {suggestion.kind_label.clone()}
                                    </span>
                                </span>
                                <span class="shrink-0 text-xs text-muted-foreground">
                                    {suggestion.action_label.clone()}
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
            {core::build_search_preset_chip_view_models(presets, selected_preset.get().as_str())
                .into_iter()
                .map(|chip| {
                    let key = chip.key.clone();
                    let class_key = chip.key.clone();
                    let query_value = query.clone();
                    view! {
                        <button
                            class=move || core::preset_chip_class(
                                selected_preset.get().as_str(),
                                class_key.as_str(),
                            )
                            on:click=move |_| {
                                let next = core::next_preset_selection(
                                    selected_preset.get().as_str(),
                                    key.as_str(),
                                );
                                set_selected_preset.set(next.clone());
                                navigate_to_search_query(&query_value, Some(next));
                            }
                            type="button"
                        >
                            {chip.label.clone()}
                        </button>
                    }
                })
                .collect_view()}
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
    let view_model = core::build_search_results_view_model(
        payload,
        selected_preset.as_str(),
        query.as_str(),
        &core::SearchResultsLabels {
            summary_template: results_summary_template,
            preset_template,
            none_label,
            locale_template,
            query_label: t(
                locale_context.as_deref(),
                "search.results.queryLabel",
                "Query",
            ),
            no_snippet: t(
                locale_context.as_deref(),
                "search.results.noSnippet",
                "No snippet returned.",
            ),
            no_target_label: t(
                locale_context.as_deref(),
                "search.results.noTarget",
                "No storefront target is available for this result yet.",
            ),
            open_result_label: t(
                locale_context.as_deref(),
                "search.results.openResult",
                "Open result",
            ),
            no_results_title: t(
                locale_context.as_deref(),
                "search.results.noResultsTitle",
                "No results",
            ),
            no_results_body: t(
                locale_context.as_deref(),
                "search.results.noResultsBody",
                "Try a different query or relax the storefront filters in the query string.",
            ),
            engine_title: t(
                locale_context.as_deref(),
                "search.features.engineTitle",
                "Engine",
            ),
            engine_body: t(
                locale_context.as_deref(),
                "search.features.engineBody",
                "Storefront uses the public published-only search surface backed by PostgreSQL FTS.",
            ),
            facet_title: t(
                locale_context.as_deref(),
                "search.features.facetsTitle",
                "Facet model",
            ),
            facet_body: t(
                locale_context.as_deref(),
                "search.features.facetsBody",
                "Entity type and source module facets come from the same search payload used by admin previews.",
            ),
        },
    );
    let item_views = view_model
        .items
        .iter()
        .map(|item| {
            view! {
                <article class="rounded-2xl border border-border bg-background p-5">
                    <div class="flex flex-wrap items-center gap-2 text-xs font-medium uppercase tracking-[0.16em] text-muted-foreground">
                        <span>{item.source_label.clone()}</span>
                        <span>"|"</span>
                        <span>{item.score_label.clone()}</span>
                    </div>
                    <h3 class="mt-3 text-lg font-semibold text-foreground">{item.title.clone()}</h3>
                    <p class="mt-2 text-sm text-muted-foreground">
                        {item.snippet.clone()}
                    </p>
                    {render_result_action(item.action.clone())}
                </article>
            }
        })
        .collect_view();
    let facet_views = view_model
        .facets
        .into_iter()
        .map(|facet| view! { <FacetCard facet /> })
        .collect_view();
    let feature_card_views = view_model
        .feature_cards
        .into_iter()
        .map(|card| view! { <FeatureCard card /> })
        .collect_view();

    view! {
        <div class="grid gap-6 lg:grid-cols-[minmax(0,1fr)_20rem]">
            <div class="space-y-6">
                <article class="rounded-2xl border border-border bg-background p-6">
                    <div class="flex flex-wrap items-center justify-between gap-3">
                        <div>
                            <div class="text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">
                                {view_model.header.query_label.clone()}
                            </div>
                            <h3 class="mt-2 text-xl font-semibold text-foreground">{view_model.header.query.clone()}</h3>
                            <p class="mt-2 text-sm text-muted-foreground">
                                {view_model.header.summary.clone()}
                            </p>
                            <p class="mt-2 text-xs text-muted-foreground">
                                {view_model.header.preset.clone()}
                            </p>
                        </div>
                        <div class="rounded-xl border border-border bg-muted/20 px-4 py-3 text-sm text-card-foreground">
                            {view_model.header.locale.clone()}
                        </div>
                    </div>
                </article>

                {if view_model.has_items {
                    view! {
                        <div class="space-y-3">
                            {item_views}
                        </div>
                    }
                    .into_any()
                } else {
                    view! {
                        <EmptyState state=view_model.no_results_empty_state.clone() />
                    }
                    .into_any()
                }}
            </div>

            <aside class="space-y-4">
                {feature_card_views}
                {facet_views}
            </aside>
        </div>
    }
}

fn render_result_action(action: core::SearchResultActionViewModel) -> impl IntoView {
    match action {
        core::SearchResultActionViewModel::NoTarget { label } => view! {
            <p class="mt-4 text-xs text-muted-foreground">
                {label}
            </p>
        }
        .into_any(),
        core::SearchResultActionViewModel::Open {
            label,
            href,
            query_log_id,
            document_id,
            position,
        } => view! {
            <a
                class="mt-4 inline-flex text-sm font-medium text-primary hover:underline"
                href=href.clone()
                on:click=move |ev| track_result_click(
                    ev,
                    query_log_id.clone(),
                    document_id.clone(),
                    href.clone(),
                    position,
                )
            >
                {label}
            </a>
        }
        .into_any(),
    }
}

fn track_result_click(
    ev: MouseEvent,
    query_log_id: Option<String>,
    document_id: String,
    href: String,
    position: i32,
) {
    let Some(window) = web_sys::window() else {
        return;
    };

    let Some(query_log_id) = query_log_id else {
        return;
    };

    ev.prevent_default();
    spawn_local(async move {
        let _ = transport::track_search_click(
            query_log_id,
            document_id,
            Some(position),
            Some(href.clone()),
        )
        .await;

        let _ = window.location().set_href(&href);
    });
}

#[component]
fn FeatureCard(card: core::SearchFeatureCardViewModel) -> impl IntoView {
    view! {
        <article class="rounded-2xl border border-border bg-background p-5">
            <div class="text-sm font-semibold text-card-foreground">{card.title}</div>
            <p class="mt-2 text-sm text-muted-foreground">{card.body}</p>
        </article>
    }
}

#[component]
fn FacetCard(facet: core::SearchFacetGroupViewModel) -> impl IntoView {
    view! {
        <article class="rounded-2xl border border-border bg-background p-5">
            <div class="text-sm font-semibold capitalize text-card-foreground">
                {facet.display_name}
            </div>
            <div class="mt-3 flex flex-wrap gap-2">
                {facet.buckets.into_iter().map(|bucket| view! {
                    <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                        {bucket.label}
                    </span>
                }).collect_view()}
            </div>
        </article>
    }
}

#[component]
fn EmptyState(state: core::SearchEmptyStateViewModel) -> impl IntoView {
    view! {
        <article class="rounded-2xl border border-dashed border-border p-8 text-center">
            <h3 class="text-lg font-semibold text-card-foreground">{state.title}</h3>
            <p class="mt-2 text-sm text-muted-foreground">{state.body}</p>
        </article>
    }
}

#[component]
fn CatalogFilterField(
    label: String,
    value: ReadSignal<String>,
    set_value: WriteSignal<String>,
    #[prop(optional)] options: Vec<SearchCatalogFilterOption>,
    #[prop(optional)] list_id: &'static str,
) -> impl IntoView {
    let list_attr = (!options.is_empty()).then_some(list_id);
    let option_nodes = if options.is_empty() {
        ().into_any()
    } else {
        view! {
            <datalist id=list_id>
                {options.into_iter().map(|option| view! { <option value=option.value>{option.label}</option> }).collect_view()}
            </datalist>
        }
        .into_any()
    };
    view! {
        <label class="grid gap-1 text-sm font-medium text-card-foreground">
            <span>{label}</span>
            <input type="text" class="w-full border border-input bg-background px-3 py-2 text-sm" prop:value=value list=list_attr on:input=move |ev| set_value.set(event_target_value(&ev)) />
            {option_nodes}
        </label>
    }
}

struct CatalogSearchSubmission {
    query: String,
    preset_key: String,
    channel_id: String,
    category_ids: String,
    attribute_code: String,
    attribute_values: String,
    attribute_min: String,
    attribute_max: String,
    sort_attribute_code: String,
    sort_desc: bool,
}

fn submit_search(ev: SubmitEvent, submission: CatalogSearchSubmission) {
    ev.prevent_default();
    navigate_to_catalog_search(&submission);
}

fn navigate_to_catalog_search(submission: &CatalogSearchSubmission) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(current_href) = window.location().href() else {
        return;
    };
    let Ok(url) = web_sys::Url::new(&current_href) else {
        return;
    };
    let route_intent =
        core::build_storefront_search_route_intent(&submission.query, Some(&submission.preset_key));

    match route_intent.query.as_deref() {
        Some(query) => url.search_params().set("q", query),
        None => url.search_params().delete("q"),
    }
    match route_intent.preset_key.as_deref() {
        Some(preset_key) => url.search_params().set("preset", preset_key),
        None => url.search_params().delete("preset"),
    }

    for (key, value) in [
        ("channel_id", submission.channel_id.as_str()),
        ("category_ids", submission.category_ids.as_str()),
        ("attribute_code", submission.attribute_code.as_str()),
        ("attribute_values", submission.attribute_values.as_str()),
        ("attribute_min", submission.attribute_min.as_str()),
        ("attribute_max", submission.attribute_max.as_str()),
        (
            "sort_attribute_code",
            submission.sort_attribute_code.as_str(),
        ),
    ] {
        if value.trim().is_empty() {
            url.search_params().delete(key);
        } else {
            url.search_params().set(key, value.trim());
        }
    }
    if submission.sort_desc {
        url.search_params().set("sort_desc", "true");
    } else {
        url.search_params().delete("sort_desc");
    }

    let _ = window.location().set_href(&url.href());
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

    let route_intent = core::build_storefront_search_route_intent(query, preset_key.as_deref());

    match route_intent.query.as_deref() {
        Some(query) => url.search_params().set("q", query),
        None => url.search_params().delete("q"),
    }

    match route_intent.preset_key.as_deref() {
        Some(preset_key) => url.search_params().set("preset", preset_key),
        None => url.search_params().delete("preset"),
    }

    let _ = window.location().set_href(&url.href());
}

fn navigate_to_href(href: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };

    let _ = window.location().set_href(href);
}
