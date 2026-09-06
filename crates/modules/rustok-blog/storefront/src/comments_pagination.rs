use rustok_ui_core::UiRouteQueryIntent;

pub const COMMENTS_PAGE_QUERY_KEY: &str = "commentsPage";
pub const COMMENTS_PAGE_SIZE: u64 = 20;
pub const MAX_COMMENTS_PAGE: u64 = 10_000;

pub fn bounded_comments_request_page(page: u64) -> u64 {
    page.clamp(1, MAX_COMMENTS_PAGE)
}

pub fn comments_page_from_query(value: Option<String>) -> u64 {
    bounded_comments_request_page(
        value
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(1),
    )
}

pub fn comments_total_pages(total: u64) -> u64 {
    total.div_ceil(COMMENTS_PAGE_SIZE).max(1)
}

pub fn bounded_comments_page(page: u64, total: u64) -> u64 {
    bounded_comments_request_page(page).clamp(1, comments_total_pages(total))
}

pub fn comments_page_query_intent(page: u64) -> UiRouteQueryIntent {
    let page = bounded_comments_request_page(page);
    if page <= 1 {
        UiRouteQueryIntent::clear(COMMENTS_PAGE_QUERY_KEY)
    } else {
        UiRouteQueryIntent::replace(COMMENTS_PAGE_QUERY_KEY, page.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_bounds_comment_page_query() {
        assert_eq!(comments_page_from_query(None), 1);
        assert_eq!(comments_page_from_query(Some(String::new())), 1);
        assert_eq!(comments_page_from_query(Some("0".to_string())), 1);
        assert_eq!(comments_page_from_query(Some("3".to_string())), 3);
        assert_eq!(comments_page_from_query(Some("invalid".to_string())), 1);
        assert_eq!(
            comments_page_from_query(Some(u64::MAX.to_string())),
            MAX_COMMENTS_PAGE
        );
        assert_eq!(comments_total_pages(0), 1);
        assert_eq!(comments_total_pages(21), 2);
        assert_eq!(bounded_comments_page(9, 21), 2);
    }

    #[test]
    fn first_page_uses_canonical_query_clear_intent() {
        assert_eq!(
            comments_page_query_intent(1),
            UiRouteQueryIntent::clear(COMMENTS_PAGE_QUERY_KEY)
        );
        assert_eq!(
            comments_page_query_intent(2),
            UiRouteQueryIntent::replace(COMMENTS_PAGE_QUERY_KEY, "2")
        );
    }
}
