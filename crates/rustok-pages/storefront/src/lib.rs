mod api;
mod i18n;
mod model;

use leptos::prelude::*;
use leptos_ui_routing::read_route_query_value;
use rustok_api::UiRouteContext;

use crate::i18n::t;
use crate::model::{PageBlock, PageDetail, PageListItem, StorefrontPagesData};

#[component]
pub fn PagesView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let selected_slug = read_route_query_value(&route_context, "slug")
        .unwrap_or_else(|| "home".to_string());
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
        move |(page_slug, locale)| async move { api::fetch_storefront_pages(page_slug, locale).await },
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
                        let pages_resource = pages_resource.clone();
                        let load_error = load_error.clone();
                        Suspend::new(async move {
                            match pages_resource.await {
                                Ok(data) => view! { <PagesShowcase data /> }.into_any(),
                                Err(err) => view! {
                                    <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                        {format!("{}: {err}", load_error)}
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
        return view! {
            <article class="rounded-2xl border border-dashed border-border p-6">
                <h3 class="text-lg font-semibold text-card-foreground">
                    {t(
                        locale.as_deref(),
                        "pages.selected.emptyTitle",
                        "Requested page is not published yet",
                    )}
                </h3>
                <p class="mt-2 text-sm text-muted-foreground">
                    {t(
                        locale.as_deref(),
                        "pages.selected.emptyBody",
                        "Choose a page from the list below with `?slug=` or publish it from the pages admin package.",
                    )}
                </p>
            </article>
        }
        .into_any();
    };

    let title = page
        .translation
        .as_ref()
        .and_then(|translation| translation.title.clone())
        .unwrap_or_else(|| t(locale.as_deref(), "pages.selected.defaultTitle", "Page"));
    let slug = page
        .translation
        .as_ref()
        .and_then(|translation| translation.slug.clone())
        .unwrap_or_else(|| t(locale.as_deref(), "pages.selected.defaultSlug", "home"));
    let effective_locale = page
        .effective_locale
        .clone()
        .unwrap_or_else(|| t(locale.as_deref(), "pages.selected.defaultLocale", "default"));
    let summary = summarize_page_content(locale.as_deref(), &page);

    view! {
        <article class="rounded-2xl border border-border bg-background p-6">
            <div class="flex flex-wrap items-center gap-2 text-xs font-medium uppercase tracking-[0.22em] text-muted-foreground">
                <span>{format!(
                    "{}: {slug}",
                    t(locale.as_deref(), "pages.selected.slugLabel", "selected slug")
                )}</span>
                <span>"·"</span>
                <span>{format!(
                    "{}: {effective_locale}",
                    t(locale.as_deref(), "pages.selected.localeLabel", "locale")
                )}</span>
            </div>
            <h3 class="mt-3 text-2xl font-semibold text-foreground">{title}</h3>
            <p class="mt-3 whitespace-pre-line text-sm leading-7 text-muted-foreground">{summary}</p>
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
        return view! {
            <article class="rounded-2xl border border-dashed border-border p-6">
                <p class="text-sm text-muted-foreground">
                    {t(
                        locale.as_deref(),
                        "pages.list.empty",
                        "No published pages are available for storefront rendering yet.",
                    )}
                </p>
            </article>
        }
        .into_any();
    }

    view! {
        <div class="space-y-3">
            <div class="flex items-center justify-between gap-3">
                <h3 class="text-lg font-semibold text-card-foreground">
                    {t(locale.as_deref(), "pages.list.title", "Published pages")}
                </h3>
                <span class="text-sm text-muted-foreground">
                    {t(locale.as_deref(), "pages.list.total", "{count} total")
                        .replace("{count}", &total.to_string())}
                </span>
            </div>
            <div class="grid gap-3 md:grid-cols-2">
                {items
                    .into_iter()
                    .map(|page| {
                        let locale = locale.clone();
                        let slug = page.slug.unwrap_or_else(|| {
                            t(locale.as_deref(), "pages.list.missingSlug", "missing-slug")
                        });
                        let href = format!("{module_route_base}?slug={slug}");
                        view! {
                            <article class="rounded-2xl border border-border bg-background p-5">
                                <div class="text-xs font-medium uppercase tracking-[0.2em] text-muted-foreground">
                                    {page.status}
                                </div>
                                <h4 class="mt-2 text-base font-semibold text-foreground">
                                    {page.title.unwrap_or_else(|| {
                                        t(locale.as_deref(), "pages.list.untitled", "Untitled page")
                                    })}
                                </h4>
                                <a class="mt-2 inline-flex text-sm text-primary hover:underline" href=href>
                                    {format!("{} {slug}", t(locale.as_deref(), "pages.list.open", "Open"))}
                                </a>
                                <p class="mt-3 text-xs text-muted-foreground">
                                    {format!(
                                        "{}: {}",
                                        t(locale.as_deref(), "pages.list.templateLabel", "template"),
                                        page.template
                                    )}
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

    t(
        locale,
        "pages.body.rawFormat",
        "Stored in `{format}` format. Raw body length: {count} characters.",
    )
    .replace("{format}", format)
    .replace("{count}", &content.chars().count().to_string())
}

fn summarize_page_content(locale: Option<&str>, page: &PageDetail) -> String {
    if let Some(body) = page.body.as_ref() {
        return summarize_content(locale, body.content.as_str(), body.format.as_str());
    }

    if !page.blocks.is_empty() {
        return summarize_legacy_blocks(locale, &page.blocks);
    }

    t(
        locale,
        "pages.body.empty",
        "No page body or legacy blocks yet.",
    )
}

fn summarize_legacy_blocks(locale: Option<&str>, blocks: &[PageBlock]) -> String {
    let block_count = blocks.len();
    let first_block = blocks
        .first()
        .map(|block| humanize_block_type(locale, block.block_type.as_str()))
        .unwrap_or_else(|| t(locale, "pages.block.default", "block"));

    if block_count == 1 {
        return t(
            locale,
            "pages.body.legacySingle",
            "Legacy block-driven page with 1 {block_type} block.",
        )
        .replace("{block_type}", first_block.as_str());
    }

    t(
        locale,
        "pages.body.legacyMany",
        "Legacy block-driven page with {count} blocks. First block: {block_type}.",
    )
    .replace("{count}", &block_count.to_string())
    .replace("{block_type}", first_block.as_str())
}

fn humanize_block_type(locale: Option<&str>, block_type: &str) -> String {
    match block_type {
        "hero" => t(locale, "pages.block.hero", "hero"),
        "text" => t(locale, "pages.block.text", "text"),
        "image" => t(locale, "pages.block.image", "image"),
        "gallery" => t(locale, "pages.block.gallery", "gallery"),
        "cta" => t(locale, "pages.block.cta", "CTA"),
        "features" => t(locale, "pages.block.features", "features"),
        "testimonials" => t(locale, "pages.block.testimonials", "testimonials"),
        "pricing" => t(locale, "pages.block.pricing", "pricing"),
        "faq" => t(locale, "pages.block.faq", "FAQ"),
        "contact" => t(locale, "pages.block.contact", "contact"),
        "product_grid" => t(locale, "pages.block.productGrid", "product grid"),
        "newsletter" => t(locale, "pages.block.newsletter", "newsletter"),
        "video" => t(locale, "pages.block.video", "video"),
        "html" => t(locale, "pages.block.html", "HTML"),
        "spacer" => t(locale, "pages.block.spacer", "spacer"),
        _ => t(locale, "pages.block.content", "content"),
    }
}

#[cfg(test)]
mod tests {
    use super::{summarize_page_content, PageBlock, PageDetail};
    use crate::model::PageBody;

    #[test]
    fn page_body_takes_precedence_over_legacy_blocks() {
        let summary = summarize_page_content(
            Some("en"),
            &PageDetail {
                effective_locale: Some("en".to_string()),
                translation: None,
                body: Some(PageBody {
                    locale: "en".to_string(),
                    content: "Hello".to_string(),
                    format: "markdown".to_string(),
                }),
                blocks: vec![PageBlock {
                    id: "1".to_string(),
                    block_type: "text".to_string(),
                    position: 0,
                }],
            },
        );

        assert_eq!(summary, "Hello");
    }

    #[test]
    fn legacy_blocks_are_summarized_when_body_is_missing() {
        let summary = summarize_page_content(
            Some("en"),
            &PageDetail {
                effective_locale: Some("en".to_string()),
                translation: None,
                body: None,
                blocks: vec![PageBlock {
                    id: "1".to_string(),
                    block_type: "text".to_string(),
                    position: 0,
                }],
            },
        );

        assert_eq!(summary, "Legacy block-driven page with 1 text block.");
    }
}
