use crate::core::BlogStorefrontFetchRequest;
use crate::model::{BlogPostAuthorProfile, BlogPostList, BlogPostListItem};
use rustok_search_storefront::model::{SearchPreviewFilters, SearchPreviewPayload};
use rustok_ui_core::UiRouteQueryIntent;

pub const BLOG_SEARCH_PAGE_SIZE: usize = 9;
const MAX_SEARCH_PAGE: usize = 100;

pub struct SearchPageView {
    pub previous: Option<UiRouteQueryIntent>,
    pub next: Option<UiRouteQueryIntent>,
}

pub fn search_page_view(page: usize, total: u64) -> SearchPageView {
    let page = page.clamp(1, MAX_SEARCH_PAGE);
    SearchPageView {
        previous: (page > 1).then(|| {
            if page == 2 {
                UiRouteQueryIntent::clear("page")
            } else {
                UiRouteQueryIntent::replace("page", (page - 1).to_string())
            }
        }),
        next: (page < MAX_SEARCH_PAGE && ((page * BLOG_SEARCH_PAGE_SIZE) as u64) < total)
            .then(|| UiRouteQueryIntent::replace("page", (page + 1).to_string())),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlogPageRequest {
    pub feed: BlogStorefrontFetchRequest,
    pub comments_page: u64,
    pub query: Option<String>,
    pub search_page: usize,
}

pub fn prepare_blog_page_request(
    mut feed: BlogStorefrontFetchRequest,
    comments_page: u64,
    cursor: Option<String>,
    query: Option<String>,
    search_page: Option<String>,
) -> BlogPageRequest {
    let query = query
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    feed.posts_after = if query.is_some() {
        None
    } else {
        cursor
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    };
    let search_page = search_page
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(1)
        .clamp(1, MAX_SEARCH_PAGE);
    BlogPageRequest {
        feed,
        comments_page,
        query,
        search_page,
    }
}

pub fn blog_search_filters(request: &BlogStorefrontFetchRequest) -> SearchPreviewFilters {
    let mut filters = SearchPreviewFilters::default();
    filters.entity_types.push("blog_post".to_string());
    filters.source_modules.push("blog".to_string());
    filters.statuses.push("published".to_string());
    if let Some(category_id) = &request.category_id {
        filters.category_ids.push(category_id.clone());
    }
    filters
}

/// Convert Search's public projection into Blog's canonical presentation
/// model without requerying/locally filtering a limited Blog page. An invalid
/// owner projection is an error, never silently shown as zero matches.
pub fn map_blog_search_results(result: SearchPreviewPayload) -> Result<BlogPostList, String> {
    let mut items = Vec::with_capacity(result.items.len());
    for item in result.items {
        if item.source_module != "blog" || item.entity_type != "blog_post" {
            return Err("Search returned an unrelated Blog result".to_owned());
        }
        let payload: serde_json::Value = serde_json::from_str(&item.payload)
            .map_err(|_| "Search returned malformed Blog metadata".to_owned())?;
        let get_string = |key: &str| -> Result<Option<String>, String> {
            match payload.get(key) {
                None | Some(serde_json::Value::Null) => Ok(None),
                Some(serde_json::Value::String(value)) => Ok(Some(value.clone())),
                Some(_) => Err(format!("Search returned invalid Blog field: {key}")),
            }
        };
        let slug = get_string("slug")?
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "Search result has no canonical Blog slug".to_owned())?;
        let tags = match payload.get("tags") {
            None | Some(serde_json::Value::Null) => Vec::new(),
            Some(serde_json::Value::Array(values)) => values
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(ToOwned::to_owned)
                        .ok_or_else(|| "Search returned invalid Blog tags".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?,
            Some(_) => return Err("Search returned invalid Blog tags".to_owned()),
        };
        let author_profile = match (get_string("author_id")?, get_string("author_name")?) {
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
            effective_locale: item
                .locale
                .and_then(|locale| rustok_api::RuntimeLocale::new(&locale).ok())
                .map(|locale| locale.as_str().to_owned())
                .ok_or_else(|| "Search result has no effective Blog locale".to_owned())?,
            slug: Some(slug),
            excerpt: get_string("excerpt")?,
            status: "published".to_string(),
            published_at: get_string("published_at")?,
            tags,
            featured_image_url: get_string("featured_image_url")?,
            author_profile,
            category_id: get_string("category_id")?,
            category_name: get_string("category_name")?,
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

    fn projection_fixture() -> SearchPreviewPayload {
        serde_json::from_value::<SearchPreviewPayload>(serde_json::json!({
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
        .expect("Search GraphQL response must deserialize")
    }

    #[test]
    fn malformed_optional_projection_fields_cannot_be_silently_dropped() {
        for metadata in [
            serde_json::json!({"slug":"article", "tags":["valid", 12]}),
            serde_json::json!({"slug":"article", "tags":"invalid"}),
            serde_json::json!({"slug":"article", "category_id":12}),
            serde_json::json!({"slug":"article", "author_name":false}),
        ] {
            let mut result = projection_fixture();
            result.items[0].payload = metadata.to_string();
            assert!(map_blog_search_results(result).is_err(), "{metadata}");
        }
    }

    #[test]
    fn search_result_projection_keeps_cross_page_fulltext_metadata() {
        let result = projection_fixture();
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

#[cfg(test)]
mod request_tests {
    use super::*;
    #[test]
    fn search_navigation_obeys_the_same_bounds_as_request_preparation() {
        assert!(search_page_view(0, 0).previous.is_none());
        assert!(search_page_view(1, 9).next.is_none());
        assert_eq!(
            search_page_view(2, 19).previous,
            Some(UiRouteQueryIntent::clear("page"))
        );
        assert_eq!(
            search_page_view(2, 19).next,
            Some(UiRouteQueryIntent::replace("page", "3"))
        );
        assert!(search_page_view(100, u64::MAX).next.is_none());
        assert!(search_page_view(usize::MAX, u64::MAX).next.is_none());
    }
    fn feed() -> BlogStorefrontFetchRequest {
        BlogStorefrontFetchRequest {
            post_slug: String::new(),
            locale: Some("ru".into()),
            tag: Some("rust".into()),
            category_id: Some("category".into()),
            posts_after: None,
        }
    }
    #[test]
    fn search_discards_feed_cursor_and_clamps_offset_inputs() {
        let request = prepare_blog_page_request(
            feed(),
            1,
            Some("cursor".into()),
            Some(" query ".into()),
            Some("18446744073709551616".into()),
        );
        assert_eq!(request.query.as_deref(), Some("query"));
        assert!(request.feed.posts_after.is_none());
        assert_eq!(request.search_page, 1);
        let request = prepare_blog_page_request(
            feed(),
            1,
            Some(" cursor ".into()),
            Some(" ".into()),
            Some("999".into()),
        );
        assert_eq!(request.feed.posts_after.as_deref(), Some("cursor"));
        assert_eq!(request.search_page, 100);
        let filters = blog_search_filters(&request.feed);
        assert_eq!(filters.category_ids, ["category"]);
        assert_eq!(filters.source_modules, ["blog"]);
    }
}
