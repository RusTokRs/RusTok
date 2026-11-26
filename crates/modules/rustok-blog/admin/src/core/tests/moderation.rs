use crate::core::moderation_page_view;

#[test]
fn moderation_pagination_bounds_empty_zero_and_extreme_owner_values() {
    let empty = moderation_page_view(0, 0, 0);
    assert_eq!(empty.current_page, 1);
    assert_eq!(empty.total_pages, 1);
    assert!(!empty.can_previous && !empty.can_next);
    let last = moderation_page_view(u64::MAX, 1, u64::MAX);
    assert_eq!(last.current_page, u64::MAX);
    assert_eq!(last.total_pages, u64::MAX);
    assert!(last.can_previous && !last.can_next);
    let second = moderation_page_view(2, 20, 21);
    assert_eq!(second.current_page, 2);
    assert_eq!(second.total_pages, 2);
    assert!(second.can_previous && !second.can_next);
}
