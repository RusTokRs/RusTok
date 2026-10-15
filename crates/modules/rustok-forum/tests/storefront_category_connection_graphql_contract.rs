use async_graphql::{EmptyMutation, EmptySubscription, Schema};
use rustok_forum::ForumQuery;

#[test]
fn storefront_categories_expose_total_count_in_page_info() {
    let schema = Schema::build(ForumQuery::default(), EmptyMutation, EmptySubscription).finish();
    let sdl = schema.sdl();
    assert!(sdl.contains("forumStorefrontCategories("));
    let fields = sdl
        .split("type ForumCategoryConnection {")
        .nth(1)
        .expect("category connection type")
        .split('}')
        .next()
        .expect("connection fields");
    assert!(fields.contains("pageInfo: PageInfo!"));
    assert!(fields.contains("items: [GqlForumCategory!]!"));
    assert!(!fields.contains("total:"), "there is no connection-level total field");
    assert!(sdl.contains("totalCount:"));
}
