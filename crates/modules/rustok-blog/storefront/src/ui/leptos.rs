mod draft_preview;
mod published_posts_list;
mod selected_post_card;
mod showcase;

pub use draft_preview::BlogDraftPreview;

use showcase::BlogShowcase;

use leptos::prelude::*;
use leptos_ui::{RichTextHtml, TableOfContents};
use leptos_ui_routing::{read_route_query_value, use_route_query_value, use_route_query_writer};
use rustok_ui_core::{UiRouteContext, UiRouteQueryIntent};

use super::share::{BlogShareButtons, BlogStatusBadge};

use crate::i18n::t;
use crate::model::{BlogPostDetail, BlogPostListItem, StorefrontBlogData};
use crate::{comments_pagination, core, transport};

#[component]
pub fn BlogView() -> impl IntoView {
    let route_context = use_context::<UiRouteContext>().unwrap_or_default();
    let selected_locale = route_context.locale.clone();
    let comments_page_query = use_route_query_value(comments_pagination::COMMENTS_PAGE_QUERY_KEY);
    let posts_cursor_query = use_route_query_value(core::POSTS_CURSOR_QUERY_KEY);
    let search_query = use_route_query_value("q");
    let search_page_query = use_route_query_value("page");
    let tag_query = use_route_query_value("tag");
    let category_query = use_route_query_value("category");
    let shell_view = core::build_storefront_shell_view_model(selected_locale.as_deref());
    let badge = shell_view.badge;
    let title = shell_view.title;
    let subtitle = shell_view.subtitle;
    let load_error = shell_view.load_error;

    let route_context_for_fetch = route_context.clone();
    let selected_locale_for_fetch = selected_locale.clone();
    let posts_resource = Resource::new_blocking(
        move || {
            let route_state = core::build_storefront_route_state(
                read_route_query_value(&route_context_for_fetch, core::SELECTED_POST_QUERY_KEY),
                route_context_for_fetch.route_segment.as_ref().cloned(),
            )
            .with_filters(tag_query.get(), category_query.get());
            let fetch_request = core::build_storefront_fetch_request(
                &route_state,
                selected_locale_for_fetch.clone(),
            );
            core::prepare_blog_page_request(
                fetch_request,
                comments_pagination::comments_page_from_query(comments_page_query.get()),
                posts_cursor_query.get(),
                search_query.get(),
                search_page_query.get(),
            )
        },
        transport::fetch_blog_page,
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
                        <div class="h-40 animate-pulse rounded-2xl bg-muted"></div>
                        <div class="grid gap-3 md:grid-cols-2">
                            <div class="h-28 animate-pulse rounded-2xl bg-muted"></div>
                            <div class="h-28 animate-pulse rounded-2xl bg-muted"></div>
                        </div>
                    </div>
                }>
                    {move || {
                        let posts_resource = posts_resource;
                        let load_error = load_error.clone();
                        let comments_page = comments_pagination::comments_page_from_query(
                            comments_page_query.get(),
                        );
                        Suspend::new(async move {
                            match posts_resource.await {
                                Ok((data, search_total, search_page)) => view! {
                                    <BlogShowcase data comments_page search_total search_page />
                                }
                                .into_any(),
                                Err(err) => view! {
                                    <div class="rounded-2xl border border-destructive/30 bg-destructive/10 px-4 py-3 text-sm text-destructive">
                                        {core::error_with_context(load_error.as_str(), &err.to_string())}
                                    </div>
                                }
                                .into_any(),
                            }
                        })
                    }}
                </Suspense>
            </div>
        </section>
    }
}
