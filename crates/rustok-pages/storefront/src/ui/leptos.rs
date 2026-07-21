use leptos::prelude::*;
use leptos_ui_routing::read_route_query_value;
use rustok_ui_core::UiRouteContext;

use crate::builder::{is_page_builder_body, PageBuilderPageBody};
use crate::core;
use crate::i18n::t;
use crate::model::{PageDetail, PageListItem, StorefrontPagesData};
use crate::transport;

#[component]
pub fn PagesView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let selected_slug =
        read_route_query_value(&route_context, "slug").unwrap_or_else(|| "home".to_string());
    let selected_locale = route_context.locale.clone();
    let badge = t(selected_locale.as_deref(), "pages.badge", "pages");
    let title = t(
        selected_locale.as_deref(),
        "pages.title",
        "Page-driven storefront experiences",
    );
    let subtitle = t(
        selected_locale.as_deref(),
        "pages.subtitle",
        "This module package renders real page data through the pages module GraphQL contract.",
    );
    let load_error = t(
        selected_locale.as_deref(),
        "pages.error.load",
        "Failed to load pages storefront data",
    );

    let pages_resource = Resource::new_blocking(
        move || (selected_slug.clone(), selected_locale.clone()),
        move |(page_slug, locale)| async move { transport::fetch_pages(page_slug, locale).await },
    );

    view! {
        <section class="rounded-3xl border border-border bg-card p-8 shadow-sm">
            <div class="max-w-3xl space-y-3">
                <span class="inline-flex items-center rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground">
                    {badge}
                </span>
                <h2 class="text-3xl font-semibold text-card-foreground">
                    {title}
                </h2>
                <p class="text-sm text-muted-foreground">
                    {subtitle}
                </p>
            </div>

            <div class="mt-8">
                <Suspense fallback=|| view! {
                    <div class="space-y-4">
                        <div class="h-32 animate-pulse rounded-2xl bg-muted"></div>
                        <div class="grid gap-3 md:grid-cols-2">
                            <div class="h-24 animate-pulse rounded-2xl bg-muted"></div>
                            <div class="h-24 animate-pulse rounded-2xl bg-muted"></div>
                        </div>
                    </div>
                }>
                    {move || {
                        let pages_resource = pages_resource;
                        let load_error = load_error.clone();
                        Suspend::new(async move {
                            match pages_resource.await {
                                Ok(data) => view! { <PagesShowcase data /> }.into_any(),
                                Err(err) => view! {
                                    <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                        {core::load_error_message(load_error.as_str(), &err.to_string())}
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
fn PagesShowcase(data: StorefrontPagesData) -> impl IntoView {
    view! {
        <div class="space-y-6">
            <SelectedPageCard page=data.selected_page />
            <PublishedPagesList items=data.pages.items total=data.pages.total />
        </div>
    }
}

#[component]
fn SelectedPageCard(page: Option<PageDetail>) -> impl IntoView {
    let locale = use_context::<UiRouteContext>().unwrap_or_default().locale;
    let Some(page) = page else {
        let empty_state = core::selected_page_empty_state(
            t(
                locale.as_deref(),
                "pages.selected.emptyTitle",
                "Requested page is not published yet",
            ),
            t(
                locale.as_deref(),
                "pages.selected.emptyBody",
                "Choose a page from the list below with `?slug=` or publish it from the pages admin package.",
            ),
        );

        return view! {
            <article class="rounded-2xl border border-dashed border-border p-6">
                <h3 class="text-lg font-semibold text-card-foreground">
                    {empty_state.title}
                </h3>
                <p class="mt-2 text-sm text-muted-foreground">
                    {empty_state.body}
                </p>
            </article>
        }
        .into_any();
    };

    let title = core::selected_page_title(
        &page,
        t(locale.as_deref(), "pages.selected.defaultTitle", "Page"),
    );
    let slug = core::selected_page_slug(
        &page,
        t(locale.as_deref(), "pages.selected.defaultSlug", "home"),
    );
    let effective_locale = core::selected_page_effective_locale(
        &page,
        t(locale.as_deref(), "pages.selected.defaultLocale", "default"),
    );
    let summary = core::summarize_page_content(
        &page,
        |content, format| summarize_content(locale.as_deref(), content, format),
        t(
            locale.as_deref(),
            "pages.body.empty",
            "No published page body is available yet.",
        ),
    );
    let builder_body = page.body.clone().filter(is_page_builder_body);
    let body_view = match builder_body {
        Some(body) => view! {
            <div class="mt-5 overflow-hidden rounded-2xl border border-border bg-background">
                <PageBuilderPageBody body />
            </div>
        }
        .into_any(),
        None => view! {
            <p class="mt-3 whitespace-pre-line text-sm leading-7 text-muted-foreground">{summary}</p>
        }
        .into_any(),
    };

    view! {
        <article class="rounded-2xl border border-border bg-background p-6">
            <div class="flex flex-wrap items-center gap-2 text-xs font-medium uppercase tracking-[0.22em] text-muted-foreground">
                <span>{core::label_value_pair(
                    &t(locale.as_deref(), "pages.selected.slugLabel", "selected slug"),
                    slug.as_str(),
                )}</span>
                <span>"·"</span>
                <span>{core::label_value_pair(
                    &t(locale.as_deref(), "pages.selected.localeLabel", "locale"),
                    effective_locale.as_str(),
                )}</span>
            </div>
            <h3 class="mt-3 text-2xl font-semibold text-foreground">{title}</h3>
            {body_view}
        </article>
    }
    .into_any()
}

#[component]
fn PublishedPagesList(items: Vec<PageListItem>, total: u64) -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let locale = route_context.locale.clone();
    let route_segment = route_context
        .route_segment
        .as_ref()
        .cloned()
        .unwrap_or_else(|| "pages".to_string());
    let module_route_base = route_context.module_route_base(route_segment.as_str());

    if items.is_empty() {
        let empty_state = core::published_pages_empty_state(t(
            locale.as_deref(),
            "pages.list.empty",
            "No published pages are available for storefront rendering yet.",
        ));

        return view! {
            <article class="rounded-2xl border border-dashed border-border p-6">
                <p class="text-sm text-muted-foreground">
                    {empty_state.body}
                </p>
            </article>
        }
        .into_any();
    }

    let header = core::published_pages_header_view(
        t(locale.as_deref(), "pages.list.title", "Published pages"),
        &t(locale.as_deref(), "pages.list.total", "{count} total"),
        total,
    );

    view! {
        <div class="space-y-3">
            <div class="flex items-center justify-between gap-3">
                <h3 class="text-lg font-semibold text-card-foreground">
                    {header.title}
                </h3>
                <span class="text-sm text-muted-foreground">
                    {header.total_label}
                </span>
            </div>
            <div class="grid gap-3 md:grid-cols-2">
                {items
                    .into_iter()
                    .map(|page| {
                        let locale = locale.clone();
                        let item_view = core::storefront_page_list_item_view(
                            page,
                            module_route_base.as_str(),
                            t(locale.as_deref(), "pages.list.missingSlug", "missing-slug"),
                            t(locale.as_deref(), "pages.list.untitled", "Untitled page"),
                            &t(locale.as_deref(), "pages.list.open", "Open"),
                            &t(locale.as_deref(), "pages.list.templateLabel", "template"),
                        );
                        view! {
                            <article class="rounded-2xl border border-border bg-background p-5">
                                <div class="text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">
                                    {item_view.status}
                                </div>
                                <h4 class="mt-2 text-base font-semibold text-foreground">
                                    {item_view.title}
                                </h4>
                                <a class="mt-2 inline-flex text-sm text-primary hover:underline" href=item_view.href>
                                    {item_view.open_label}
                                </a>
                                <p class="mt-3 text-xs text-muted-foreground">
                                    {item_view.template_label}
                                </p>
                            </article>
                        }
                    })
                    .collect_view()}
            </div>
        </div>
    }
    .into_any()
}

fn summarize_content(locale: Option<&str>, content: &str, format: &str) -> String {
    if format.eq_ignore_ascii_case("markdown") {
        return content.trim().to_string();
    }

    core::raw_body_format_summary(
        format,
        content.chars().count(),
        &t(
            locale,
            "pages.body.rawFormat",
            "Stored in `{format}` format. Raw body length: {count} characters.",
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::{summarize_content, PageDetail};
    use crate::core;
    use crate::model::PageBody;

    #[test]
    fn label_value_pair_formats_label_and_value() {
        assert_eq!(core::label_value_pair("locale", "en"), "locale: en");
    }

    #[test]
    fn open_link_label_joins_prefix_and_slug() {
        assert_eq!(core::open_link_label("Open", "home"), "Open home");
    }

    #[test]
    fn count_label_replaces_placeholder() {
        assert_eq!(core::count_label("{count} total", 5), "5 total");
    }

    #[test]
    fn page_body_is_the_only_content_summary_source() {
        let summary = core::summarize_page_content(
            &PageDetail {
                effective_locale: Some("en".to_string()),
                translation: None,
                body: Some(PageBody {
                    locale: "en".to_string(),
                    content: "Hello".to_string(),
                    format: "markdown".to_string(),
                }),
            },
            |content, format| summarize_content(Some("en"), content, format),
            "empty".to_string(),
        );

        assert_eq!(summary, "Hello");
    }

    #[test]
    fn raw_body_format_summary_uses_template_placeholders() {
        let summary = core::raw_body_format_summary(
            "rt_json_v1",
            42,
            "Stored in `{format}` format. Raw body length: {count} characters.",
        );

        assert_eq!(
            summary,
            "Stored in `rt_json_v1` format. Raw body length: 42 characters."
        );
    }

    #[test]
    fn missing_body_uses_current_empty_state() {
        let summary = core::summarize_page_content(
            &PageDetail {
                effective_locale: Some("en".to_string()),
                translation: None,
                body: None,
            },
            |content, format| summarize_content(Some("en"), content, format),
            "empty".to_string(),
        );

        assert_eq!(summary, "empty");
    }
}
