mod published_posts_list;
mod selected_post_card;
mod showcase;

use showcase::BlogShowcase;

use leptos::prelude::*;
use leptos_auth::hooks::use_token;
use leptos_ui::{RichTextHtml, TableOfContents};
use leptos_ui_routing::{read_route_query_value, use_route_query_value, use_route_query_writer};
#[cfg(target_arch = "wasm32")]
use rustok_comments_storefront_support::CommentComposer;
use rustok_ui_core::{UiRouteContext, UiRouteQueryIntent};

use super::comments_list::PublicCommentsList;
use super::share::{BlogShareButtons, BlogStatusBadge};

use crate::i18n::{comment_composer_copy, t};
use crate::model::{
    BlogCommentsAvailability, BlogPostAuthorProfile, BlogPostDetail, BlogPostList,
    BlogPostListItem, StorefrontBlogData,
};
use crate::{comments_pagination, core, transport};
use rustok_search_storefront::model::{SearchPreviewFilters, SearchPreviewPayload};

const BLOG_SEARCH_PAGE_SIZE: usize = 9;

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
            let mut fetch_request = core::build_storefront_fetch_request(
                &route_state,
                selected_locale_for_fetch.clone(),
            );
            fetch_request.posts_after = posts_cursor_query
                .get()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            let q = search_query
                .get()
                .map(|q| q.trim().to_owned())
                .filter(|q| !q.is_empty());
            let search_page = search_page_query
                .get()
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(1)
                .clamp(1, 100);
            if q.is_some() {
                // The Blog keyset cursor belongs to the ordinary feed, not
                // to a relevance-ranked Search page. An old postsCursor in
                // a bookmarked URL must never prevent a valid FTS request.
                fetch_request.posts_after = None;
            }
            (
                fetch_request,
                comments_pagination::comments_page_from_query(comments_page_query.get()),
                q,
                search_page,
            )
        },
        move |(request, comments_page, query, search_page)| async move {
            let mut data = transport::fetch_blog(request.clone(), comments_page)
                .await
                .map_err(|error| error.to_string())?;
            let Some(query) = query else {
                return Ok::<_, String>((data, None, search_page));
            };
            let mut filters = SearchPreviewFilters::default();
            filters.entity_types.push("blog_post".to_owned());
            filters.source_modules.push("blog".to_owned());
            filters.statuses.push("published".to_owned());
            if let Some(category_id) = request.category_id {
                filters.category_ids.push(category_id);
            }
            let result = rustok_search_storefront::transport::fetch_search_page(
                query,
                request.locale,
                None,
                filters,
                request.tag,
                BLOG_SEARCH_PAGE_SIZE,
                (search_page - 1) * BLOG_SEARCH_PAGE_SIZE,
            )
            .await
            .map_err(|error| error.to_string())?;
            let total = result.total;
            data.posts = map_blog_search_results(result)?;
            Ok((data, Some(total), search_page))
        },
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

/// Convert Search's public projection into Blog's canonical presentation
/// model without requerying/locally filtering a limited Blog page. An invalid
/// owner projection is an error, never silently shown as zero matches.
fn map_blog_search_results(result: SearchPreviewPayload) -> Result<BlogPostList, String> {
    let mut items = Vec::with_capacity(result.items.len());
    for item in result.items {
        if item.source_module != "blog" || item.entity_type != "blog_post" {
            return Err("Search returned an unrelated Blog result".to_owned());
        }
        let payload: serde_json::Value = serde_json::from_str(&item.payload)
            .map_err(|_| "Search returned malformed Blog metadata".to_owned())?;
        let get_string = |key: &str| {
            payload
                .get(key)
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned)
        };
        let slug = get_string("slug")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "Search result has no canonical Blog slug".to_owned())?;
        let tags = payload
            .get("tags")
            .and_then(serde_json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| value.as_str().map(ToOwned::to_owned))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let author_profile = match (get_string("author_id"), get_string("author_name")) {
            (Some(user_id), Some(display_name)) if !display_name.trim().is_empty() => {
                Some(BlogPostAuthorProfile {
                    user_id,
                    handle: String::new(),
                    display_name,
                    tags: Vec::new(),
                    avatar_media_id: None,
                })
            }
            _ => None,
        };
        items.push(BlogPostListItem {
            id: item.id,
            title: item.title,
            effective_locale: item.locale.unwrap_or_else(|| "en".to_string()),
            slug: Some(slug),
            excerpt: get_string("excerpt"),
            status: "published".to_string(),
            published_at: get_string("published_at"),
            tags,
            featured_image_url: get_string("featured_image_url"),
            author_profile,
            category_id: get_string("category_id"),
            category_name: get_string("category_name"),
        });
    }
    Ok(BlogPostList {
        items,
        next_cursor: None,
    })
}

#[cfg(test)]
mod blog_search_mapping_tests {
    use super::*;

    #[test]
    fn search_result_projection_keeps_cross_page_fulltext_metadata() {
        let result = serde_json::from_value::<SearchPreviewPayload>(serde_json::json!({
            "queryLogId": null,
            "presetKey": null,
            "total": 51,
            "tookMs": 3,
            "engine": "postgres",
            "rankingProfile": "balanced",
            "facets": [],
            "items": [{
                "id": "c293edb7-4f5b-432b-9011-096354ba8ba6",
                "entityType": "blog_post",
                "sourceModule": "blog",
                "title": "Article with searchable body",
                "snippet": "A matching word from the body",
                "score": 0.9,
                "locale": "ru",
                "url": "/modules/blog?slug=search-guide",
                "payload": serde_json::json!({
                    "slug": "search-guide",
                    "excerpt": "Excerpt",
                    "category_id": "4c66bc80-d348-44a9-bce6-a43d74832adf",
                    "category_name": "Tutorials",
                    "featured_image_url": "/media/example.webp",
                    "tags": ["rust", "search"],
                    "published_at": "2026-10-28T13:00:00Z"
                }).to_string()
            }]
        }))
        .expect("Search GraphQL response must deserialize");
        let page = map_blog_search_results(result).expect("Blog page projection");
        assert!(
            page.next_cursor.is_none(),
            "Search uses offsets, not Blog cursors"
        );
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].slug.as_deref(), Some("search-guide"));
        assert_eq!(page.items[0].effective_locale, "ru");
        assert_eq!(page.items[0].tags, ["rust", "search"]);
        assert_eq!(page.items[0].category_name.as_deref(), Some("Tutorials"));
    }
}
